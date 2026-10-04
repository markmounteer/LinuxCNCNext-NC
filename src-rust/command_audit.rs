//! Independent obligations over prepared commands, never builder state/caches.
//! The preserved JavaScript command corpus is an additional, external oracle.
use crate::{
    compiled::{self, Action, Frame, Phase, Record, Site, Span},
    geometry,
    part21::Limits,
    plan::{Transition, ValidatedPlan, Waypoint},
    profile::Program,
    Diagnostic, Result,
};
use motion_command::{
    v2::{Geometry, GeometryCapabilities},
    Capabilities, Capability, Command, Coolant, EntryGate, Feed, Machine, Plane, Rotation, Spindle,
    Termination,
};
use serde::Serialize;
use serde_json::Value;
use std::{collections::BTreeSet, f64::consts::TAU};

#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub schema: &'static str,
    pub status: &'static str,
    pub commands: usize,
    pub motions: usize,
    pub source_uses: usize,
    pub reviewed_waypoints: usize,
    pub events: usize,
    pub required_capabilities: Vec<String>,
    pub execution_authorized: bool,
}

fn fail(rule: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("native-command-audit", rule, message)
}
fn insist(ok: bool, rule: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(fail(
            rule,
            "Prepared command violates a source/setup/policy obligation",
        ))
    }
}
fn structural_capabilities() -> GeometryCapabilities {
    // This is a structural vocabulary check, not an installed capability claim.
    let mut base = Capabilities::default();
    for cap in [
        Capability::Linear,
        Capability::PlanarArc,
        Capability::Blend,
        Capability::AtSpeed,
        Capability::FeedPerRevolution,
        Capability::Spindle,
        Capability::Css,
        Capability::Coolant,
        Capability::ToolChange,
        Capability::ToolOffset,
        Capability::Dwell,
        Capability::Completion,
    ] {
        base = base.with(cap);
    }
    GeometryCapabilities {
        base,
        helix: true,
        multiple_turns: true,
    }
}

struct Audit<'a> {
    commands: &'a [Record],
    spans: &'a [Span],
    span: usize,
    at: usize,
    end: usize,
    spindle: Option<Spindle>,
    requested_spindle: Option<Value>,
    coolant: Option<Coolant>,
    tool: Option<u32>,
    gate: bool,
    unit: f64,
    machine: Machine,
    report: Report,
    required: BTreeSet<String>,
    current: Option<usize>,
    expected_phase: Phase,
}
impl Audit<'_> {
    fn phase(&mut self, expected: Phase) -> Result<()> {
        self.current = None;
        self.expected_phase = expected;
        insist(self.at == self.end, "EXTRA_COMMAND")?;
        let s = self
            .spans
            .get(self.span)
            .ok_or_else(|| fail("MISSING_SPAN", "Missing operation/path span"))?;
        insist(
            s.phase == expected
                && s.commands.start == self.at
                && s.commands.end >= self.at
                && s.commands.end <= self.commands.len(),
            "SPAN_COVERAGE",
        )?;
        self.end = s.commands.end;
        self.span += 1;
        Ok(())
    }
    fn next(&mut self, site: Site, ordinal: Option<usize>) -> Result<Action> {
        self.current = (self.at < self.end).then_some(self.at);
        if self.at >= self.end {
            return Err(fail(
                "MISSING_COMMAND",
                "Required command missing from its phase",
            ));
        }
        let record = self
            .commands
            .get(self.at)
            .ok_or_else(|| fail("MISSING_COMMAND", "Missing command"))?;
        insist(
            record.site == site && record.ordinal == ordinal,
            "SOURCE_USE",
        )?;
        self.at += 1;
        if let Action::Event(command) = record.action {
            insist(!matches!(command, Command::Motion(_)), "CONTRACT_REVISION")?;
            command
                .check_structure(self.machine, structural_capabilities().base)
                .map_err(|e| fail("EVENT_STRUCTURE", format!("{e:?}")))?;
            self.report.events += 1;
            let capability = match command {
                Command::Spindle(Spindle::Css { .. }) => {
                    self.required.insert("css".into());
                    "spindle"
                }
                Command::Spindle(_) => "spindle",
                Command::Coolant(_) => "coolant",
                Command::ChangeTool { .. } => "tool-change",
                Command::ToolOffset { .. } => "tool-offset",
                Command::Dwell { .. } => "dwell",
                _ => "completion",
            };
            self.required.insert(capability.into());
        }
        Ok(record.action)
    }
    fn policy(&mut self, expected: Action) -> Result<()> {
        let got = self.next(Site::Policy, None)?;
        insist(got == expected, "POLICY_ORDER")
    }
    fn stop(&mut self) -> Result<()> {
        self.policy(Action::Event(Command::Spindle(Spindle::Stop)))?;
        self.policy(Action::Event(Command::Coolant(Coolant::Off)))?;
        self.policy(Action::ResetSpindleDemand)?;
        self.spindle = Some(Spindle::Stop);
        self.requested_spindle = None;
        self.coolant = Some(Coolant::Off);
        self.gate = false;
        Ok(())
    }
    fn waypoints(&mut self, waypoints: &[Waypoint], frame: Frame, site: Site) -> Result<()> {
        for (i, p) in waypoints.iter().enumerate() {
            let got = self.next(site, Some(i))?;
            insist(
                got == Action::Waypoint {
                    frame,
                    axis: p.axis,
                    value_mm: p.value * self.unit,
                },
                "REVIEWED_WAYPOINT",
            )?;
            insist(
                p.axis < 3 && (self.machine != Machine::LatheXz || p.axis != 1),
                "WAYPOINT_AXIS",
            )?;
            if site != Site::Link {
                insist(
                    self.spindle == Some(Spindle::Stop) && self.coolant == Some(Coolant::Off),
                    "RETRACT_APPROACH_STATE",
                )?;
            }
            self.report.reviewed_waypoints += 1;
            self.required.insert("linear".into());
        }
        Ok(())
    }
    fn process(&mut self, s: &Value, c: &Value) -> Result<()> {
        // Compare exact source dimensions directly. No emitter conversion helper
        // or state snapshot participates in this independent state fold.
        let speed = compiled::number(&s["speed"])?;
        let clockwise = s["clockwise"]
            .as_bool()
            .ok_or_else(|| fail("SOURCE_STATE", "Missing spindle direction"))?;
        let target = if s["mode"] == "css" {
            Spindle::Css {
                surface_mm_per_second: speed * self.unit / 60.0,
                maximum_rpm: compiled::number(&s["maximumRPM"])?,
                clockwise,
            }
        } else {
            Spindle::Rpm {
                rpm: speed,
                clockwise,
            }
        };
        if self.requested_spindle.as_ref() != Some(s) {
            if let Some(Spindle::Rpm { clockwise: old, .. } | Spindle::Css { clockwise: old, .. }) =
                self.spindle
            {
                if old != clockwise {
                    insist(
                        self.next(Site::Source, None)?
                            == Action::Event(Command::Spindle(Spindle::Stop)),
                        "REVERSAL_STOP",
                    )?;
                }
            }
            insist(
                self.next(Site::Source, None)? == Action::Event(Command::Spindle(target)),
                "SPINDLE_SOURCE",
            )?;
            self.spindle = Some(target);
            self.requested_spindle = Some(s.clone());
            self.gate = true;
        }
        let target = match c.as_str() {
            Some("off") => Coolant::Off,
            Some("flood") => Coolant::Flood,
            Some("mist") => Coolant::Mist,
            _ => return Err(fail("SOURCE_STATE", "Unsupported coolant")),
        };
        if self.coolant != Some(target) {
            insist(
                self.next(Site::Source, None)? == Action::Event(Command::Coolant(Coolant::Off)),
                "COOLANT_OFF",
            )?;
            if target != Coolant::Off {
                insist(
                    self.next(Site::Source, None)? == Action::Event(Command::Coolant(target)),
                    "COOLANT_SOURCE",
                )?;
            }
            self.coolant = Some(target);
        }
        Ok(())
    }
    fn motion(
        &mut self,
        path: &Value,
        section: &Value,
        position: [f64; 3],
        ordinal: usize,
        termination: Termination,
    ) -> Result<[f64; 3]> {
        let Action::Motion(m) = self.next(Site::Source, Some(ordinal))? else {
            return Err(fail("MOTION_USE", "Source motion missing/replaced"));
        };
        m.check_structure(
            motion_command::v2::CONTRACT_VERSION,
            self.machine,
            structural_capabilities(),
        )
        .map_err(|e| fail("MOTION_STRUCTURE", format!("{e:?}")))?;
        geometry::validate_with_floor(m.geometry, 1e-7 * self.unit)?;
        let expected_feed = if path["kind"] == "rapid" {
            Feed::Rapid
        } else if path["feed"]["mode"] == "perRevolution" {
            Feed::PerRevolution {
                mm_per_rev: compiled::number(&path["feed"]["value"])? * self.unit,
                spindle: 0,
            }
        } else {
            Feed::PerSecond(compiled::number(&path["feed"]["value"])? * self.unit / 60.0)
        };
        insist(
            m.feed == expected_feed
                && m.termination
                    == if expected_feed == Feed::Rapid {
                        Termination::ExactPath
                    } else {
                        termination
                    },
            "FEED_TERMINATION",
        )?;
        insist(
            m.movement == compiled::movement(&path["movement"])?
                && m.tolerance == compiled::tolerance(&section["tolerance"], self.unit)?,
            "SOURCE_INTENT",
        )?;
        let gate = if m.feed != Feed::Rapid && self.gate {
            self.gate = false;
            EntryGate::SpindlesAtSpeed
        } else {
            EntryGate::None
        };
        insist(m.entry_gate == gate, "ENTRY_GATE")?;
        if gate != EntryGate::None {
            self.required.insert("at-speed".into());
        }
        if matches!(m.feed, Feed::PerRevolution { .. }) {
            self.required.insert("feed-per-revolution".into());
        }
        if matches!(m.termination, Termination::Blend { .. }) {
            self.required.insert("blend".into());
        }
        let (start, end) = match m.geometry {
            Geometry::Line { start, end } | Geometry::Circular { start, end, .. } => (start, end),
        };
        insist(
            geometry::coordinates(start) == position.map(|v| v * self.unit),
            "COMMANDED_START",
        )?;
        let end_source;
        match m.geometry {
            Geometry::Line { .. } => {
                insist(
                    path["kind"] == "linear" || path["kind"] == "rapid",
                    "GEOMETRY_KIND",
                )?;
                end_source = compiled::xyz(&path["points"][ordinal])?;
                self.required.insert("linear".into());
            }
            Geometry::Circular {
                center,
                plane,
                rotation,
                sweep_radians,
                axial_rise_mm,
                ..
            } => {
                insist(
                    path["kind"] == "arc" || path["kind"] == "circular",
                    "GEOMETRY_KIND",
                )?;
                let (wanted_plane, normal) = match path["plane"].as_str().unwrap_or("XZ") {
                    "XY" => (Plane::Xy, 2),
                    "XZ" => (Plane::Xz, 1),
                    "YZ" => (Plane::Yz, 0),
                    _ => return Err(fail("SOURCE_PLANE", "Unexpected source plane")),
                };
                insist(
                    plane == wanted_plane
                        && (rotation == Rotation::Clockwise) == (path["clockwise"] == true),
                    "PLANE_DIRECTION",
                )?;
                end_source = if path["fullCircle"] == true {
                    position
                } else {
                    compiled::xyz(&path["end"])?
                };
                let source_center = compiled::xyz(&path["center"])?;
                let source_start = compiled::xyz(&path["start"])?;
                let actual_center = geometry::coordinates(center);
                for axis in 0..3 {
                    let wanted = if axis == normal {
                        position[axis]
                    } else if path["kind"] == "circular" {
                        source_center[axis]
                    } else {
                        position[axis] + (source_center[axis] - source_start[axis])
                    };
                    insist(actual_center[axis] == wanted * self.unit, "ARC_CENTER")?;
                }
                if path["kind"] == "circular" {
                    insist(
                        sweep_radians == compiled::number(&path["sweepRadians"])?,
                        "SOURCE_SWEEP",
                    )?;
                } else if path["fullCircle"] == true {
                    insist(sweep_radians == TAU, "FULL_CIRCLE")?;
                } else {
                    insist(sweep_radians > 0.0 && sweep_radians < TAU, "PARTIAL_ARC")?;
                }
                // The independent geometry oracle checks direction/sweep against
                // the endpoint, not just the builder's trigonometric formula.
                insist(
                    axial_rise_mm == end_source[normal] * self.unit - position[normal] * self.unit,
                    "AXIAL_RISE",
                )?;
                if axial_rise_mm != 0.0 {
                    self.required.insert("helix".into());
                }
                if sweep_radians > TAU {
                    self.required.insert("multiple-turns".into());
                }
                self.required.insert("planar-arc".into());
            }
        }
        insist(
            geometry::coordinates(end) == end_source.map(|v| v * self.unit),
            "SOURCE_ENDPOINT",
        )?;
        self.report.motions += 1;
        self.report.source_uses += 1;
        Ok(end_source)
    }
}

/// Public for independent artifact verification and corruption tests. A passed
/// audit does not construct a plan, authorize execution, or trust live bindings.
pub fn audit(
    program: &Program,
    setup: &ValidatedPlan,
    commands: &[Record],
    spans: &[Span],
    limits: &Limits,
) -> Result<Report> {
    let mut a = Audit {
        commands,
        spans,
        span: 0,
        at: 0,
        end: 0,
        spindle: None,
        requested_spindle: None,
        coolant: None,
        tool: None,
        gate: false,
        unit: compiled::scale(program),
        machine: compiled::machine(program),
        required: BTreeSet::new(),
        current: None,
        expected_phase: Phase::Header,
        report: Report {
            schema: "nextnc-native/command-audit/1",
            status: "passed",
            commands: commands.len(),
            motions: 0,
            source_uses: 0,
            reviewed_waypoints: 0,
            events: 0,
            required_capabilities: Vec::new(),
            execution_authorized: false,
        },
    };
    let result = (|| -> Result<()> {
        limits.check()?;
        insist(
            commands.len() <= limits.output_commands,
            "OUTPUT_COMMAND_LIMIT",
        )?;
        // Revalidate the source binding of setup. Public audit APIs must not accept
        // a ValidatedPlan copied from a different source job.
        plan_binding(program, setup)?;
        a.phase(Phase::Header)?;
        a.policy(Action::ResetModes)?;
        a.policy(Action::ClearTemporaryOffsets)?;
        a.stop()?;
        for (si, section) in compiled::array(&program.model["sections"])?
            .iter()
            .enumerate()
        {
            a.phase(Phase::Transition(si))?;
            match &setup.transitions()[si] {
                Transition::Retract { retract, approach } => {
                    if si > 0 {
                        a.stop()?;
                    }
                    a.waypoints(retract, Frame::Machine, Site::Retract)?;
                    let mapping = &setup.mappings()[si];
                    let change_required = a.tool != Some(mapping.tool);
                    if change_required {
                        a.policy(Action::Event(Command::ToolOffset { offset: 0 }))?;
                        a.policy(Action::Event(Command::ChangeTool { tool: mapping.tool }))?;
                        // A tool-change procedure invalidates every assumed mode.
                        a.spindle = None;
                        a.coolant = None;
                        a.tool = Some(mapping.tool);
                    }
                    a.policy(Action::ResetModes)?;
                    if change_required {
                        a.stop()?;
                    }
                    a.policy(Action::ClearTemporaryOffsets)?;
                    a.policy(Action::SelectWorkOffset(mapping.work_offset))?;
                    a.policy(Action::Event(Command::ToolOffset {
                        offset: mapping.offset,
                    }))?;
                    a.waypoints(approach, Frame::Work, Site::Approach)?;
                }
                Transition::Link { moves } => a.waypoints(moves, Frame::Work, Site::Link)?,
                Transition::Continue => (),
            }
            a.phase(Phase::Entry(si))?;
            a.process(&section["initialSpindle"], &section["initialCoolant"])?;
            let mut position = compiled::xyz(&section["start"])?;
            for (pi, path) in compiled::array(&section["paths"])?.iter().enumerate() {
                a.phase(Phase::Path {
                    section: si,
                    path: pi,
                })?;
                a.process(&path["spindle"], &path["coolant"])?;
                if path["kind"] == "dwell" {
                    let wanted = Action::Event(Command::Dwell {
                        seconds: compiled::number(&path["seconds"])?,
                    });
                    insist(a.next(Site::Source, Some(0))? == wanted, "DWELL_SOURCE")?;
                    a.report.source_uses += 1;
                } else if let Some(points) = path["points"].as_array() {
                    for vertex in 1..points.len() {
                        position = a.motion(
                            path,
                            section,
                            position,
                            vertex,
                            setup.path_controls()[si].termination(),
                        )?;
                    }
                } else {
                    position = a.motion(
                        path,
                        section,
                        position,
                        0,
                        setup.path_controls()[si].termination(),
                    )?;
                }
            }
        }
        a.phase(Phase::End)?;
        a.stop()?;
        a.waypoints(setup.end(), Frame::Machine, Site::End)?;
        a.policy(Action::Event(Command::ToolOffset { offset: 0 }))?;
        a.policy(Action::RestoreFeedPerMinute)?;
        a.policy(Action::Event(Command::End))?;
        insist(
            a.at == a.end && a.end == commands.len() && a.span == spans.len(),
            "WHOLE_JOB_COVERAGE",
        )?;
        Ok(())
    })();
    if let Err(e) = result {
        let mut e = e
            .with("command", a.current.unwrap_or(a.at).saturating_add(1))
            .with("commandIndexBase", 1)
            .with("phase", format!("{:?}", a.expected_phase));
        if let Some(r) = a.current.and_then(|i| commands.get(i)) {
            e = e.with(
                "provenance",
                compiled::source_context(program, a.expected_phase, *r),
            );
        }
        return Err(e);
    }
    a.report.required_capabilities = a.required.into_iter().collect();
    Ok(a.report)
}
fn plan_binding(program: &Program, setup: &ValidatedPlan) -> Result<()> {
    let sections = compiled::array(&program.model["sections"])?;
    insist(
        setup.source()["programFingerprint"] == program.report.fingerprint.value
            && setup.mappings().len() == sections.len()
            && setup.transitions().len() == sections.len()
            && setup.path_controls().len() == sections.len(),
        "PLAN_BINDING",
    )
}
