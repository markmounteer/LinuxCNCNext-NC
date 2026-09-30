//! Immutable, unoptimized preparation. No command here is admitted to a controller.
//! Work coordinates and partial machine waypoints still require live task binding.
use crate::{geometry, part21::Limits, plan, profile, Diagnostic, Result};
use motion_command::{
    v2::{Geometry, Motion, Movement, Tolerance},
    Command, Coolant, EntryGate, Feed, Machine, Plane, Rotation, Spindle, Termination,
};
use serde_json::{json, Value};
use std::{f64::consts::TAU, ops::Range};

pub const POLICY: &str = "nextnc-native/unoptimized-exact-path/1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Frame {
    Machine,
    Work,
}

/// Task-owned operations remain distinct from resolved motion. Their future
/// adapter must acknowledge completion; none is an arbitrary HAL write.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    Motion(Motion),
    /// Shared process event vocabulary. The revision-1 Motion variant is forbidden.
    Event(Command),
    /// Establish absolute Cartesian/radius coordinates, exact-path policy, no
    /// canned cycle/cutter compensation, and the normal feed-per-minute context.
    /// Native motions themselves always carry dimensioned feeds and planes.
    ResetModes,
    ClearTemporaryOffsets,
    /// Restore stopped spindle demand to RPM mode with zero speed (also clears CSS).
    ResetSpindleDemand,
    SelectWorkOffset(u8),
    RestoreFeedPerMinute,
    /// Reviewed single-axis rapid intent, NOT a resolved segment. Other axes and
    /// the initial position are unknown until the task binds current coordinates.
    Waypoint {
        frame: Frame,
        axis: usize,
        value_mm: f64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Site {
    Policy,
    Retract,
    Approach,
    Link,
    End,
    Source,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Record {
    pub action: Action,
    pub site: Site,
    /// Zero-based reviewed waypoint, or one-based polyline segment. Circular
    /// and dwell source uses have ordinal zero. State events have no ordinal.
    pub ordinal: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Header,
    Transition(usize),
    Entry(usize),
    Path { section: usize, path: usize },
    End,
}

/// Repeated operation/path metadata is stored once, not cloned into each command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub phase: Phase,
    pub commands: Range<usize>,
}

#[derive(Clone, Debug)]
pub struct PreparedPlan {
    pub(crate) program: profile::Program,
    pub(crate) setup: plan::ValidatedPlan,
    pub(crate) commands: Vec<Record>,
    pub(crate) spans: Vec<Span>,
    pub(crate) audit: crate::command_audit::Report,
}

impl PreparedPlan {
    pub fn commands(&self) -> &[Record] {
        &self.commands
    }
    pub fn spans(&self) -> &[Span] {
        &self.spans
    }
    pub fn program(&self) -> &profile::Program {
        &self.program
    }
    pub fn setup(&self) -> &plan::ValidatedPlan {
        &self.setup
    }
    pub fn audit(&self) -> &crate::command_audit::Report {
        &self.audit
    }
    /// Logarithmic lookup with no replay from program start and no state snapshots
    /// that could be mistaken for restart permission. Empty spans are harmless.
    pub fn source_for(&self, command: usize) -> Option<(&Span, &Record)> {
        let record = self.commands.get(command)?;
        let index = self.spans.partition_point(|s| s.commands.end <= command);
        let span = self.spans.get(index)?;
        span.commands.contains(&command).then_some((span, record))
    }
    /// Bounded provenance: never clones an entire polyline or operation.
    /// Command indexes are zero-based; source line/vertex labels are explicit.
    pub fn provenance_for(&self, command: usize) -> Option<Value> {
        let (span, record) = self.source_for(command)?;
        Some(source_context(&self.program, span.phase, *record))
    }
}

pub(crate) fn source_context(program: &profile::Program, phase: Phase, record: Record) -> Value {
    let mut result = json!({"phase":format!("{phase:?}"),"site":format!("{:?}",record.site),"ordinal":record.ordinal,
        "coordinateUnits":"mm; X radius on a lathe","policy":POLICY});
    let section = match phase {
        Phase::Transition(s) | Phase::Entry(s) | Phase::Path { section: s, .. } => Some(s),
        _ => None,
    };
    if let Some(si) = section {
        let s = &program.provenance["sections"][si];
        let model = &program.model["sections"][si];
        result["section"] = json!(si.saturating_add(1));
        result["operation"] = model["name"].clone();
        result["workingstep"] = s["workingstep"].clone();
        result["operationSource"] = s["operation"].clone();
        result["operationSequence"] = s["sequenceRelationship"].clone();
        if let Phase::Path { path, .. } = phase {
            let p = &s["paths"][path];
            result["path"] = json!(path.saturating_add(1));
            result["toolpath"] = p["toolpath"].clone();
            result["pathSequence"] = p["sequenceRelationship"].clone();
            match record.action {
                Action::Motion(_) => {
                    for name in ["curve", "arc", "circular", "movement"] {
                        if !p[name].is_null() {
                            result[name] = p[name].clone();
                        }
                    }
                    result["tolerance"] = s["tolerance"].clone();
                    result["feed"] = p["process"]["feed"].clone();
                    if let Some(vertex) = record.ordinal.filter(|n| *n > 0) {
                        result["fromVertex"] = json!(vertex);
                        result["toVertex"] = json!(vertex.saturating_add(1));
                        result["from"] = p["vertices"][vertex - 1].clone();
                        result["to"] = p["vertices"][vertex].clone();
                    }
                }
                Action::Event(Command::Dwell { .. }) => result["property"] = p["dwell"].clone(),
                Action::Event(Command::Spindle(_)) => {
                    result["property"] = p["process"]["spindle"].clone()
                }
                Action::Event(Command::Coolant(_)) => {
                    result["property"] = p["process"]["coolant"].clone();
                    result["coolantType"] = p["process"]["coolantType"].clone();
                }
                _ => (),
            }
        } else if record.site == Site::Source {
            let key = if matches!(record.action, Action::Event(Command::Spindle(_))) {
                "spindle"
            } else {
                "coolant"
            };
            result["property"] = s["process"][key].clone();
        }
        let pointer = match record.site {
            Site::Retract => Some(format!("/sections/{si}/retract")),
            Site::Approach => Some(format!("/sections/{si}/approach")),
            Site::Link => Some(format!("/sections/{si}/moves")),
            _ => match record.action {
                Action::Event(Command::ChangeTool { .. } | Command::ToolOffset { offset: 1.. }) => {
                    Some(format!(
                        "/tools/{}:{}",
                        model["tool"]["number"], model["tool"]["offset"]
                    ))
                }
                Action::SelectWorkOffset(_) => {
                    Some(format!("/workOffsets/{}", model["workOffset"]))
                }
                _ => None,
            },
        };
        if let Some(mut p) = pointer {
            if let Some(n) = record.ordinal {
                p.push_str(&format!("/{n}"));
            }
            result["planPointer"] = json!(p);
        }
    } else if record.site == Site::End {
        result["planPointer"] = json!(format!("/end/{}", record.ordinal.unwrap_or(0)));
    }
    result
}

pub(crate) fn fail(code: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("native-compile", code, message)
}
pub(crate) fn array(v: &Value) -> Result<&Vec<Value>> {
    v.as_array()
        .ok_or_else(|| fail("MODEL", "Expected validated array"))
}
pub(crate) fn number(v: &Value) -> Result<f64> {
    v.as_f64()
        .filter(|n| n.is_finite())
        .ok_or_else(|| fail("MODEL", "Expected finite validated number"))
}
pub(crate) fn xyz(v: &Value) -> Result<[f64; 3]> {
    let a = array(v)?;
    if a.len() != 3 {
        return Err(fail("MODEL", "Expected XYZ point"));
    }
    Ok([number(&a[0])?, number(&a[1])?, number(&a[2])?])
}
pub(crate) fn scale(program: &profile::Program) -> f64 {
    if program.report.units == "inch" {
        25.4
    } else {
        1.0
    }
}
pub(crate) fn machine(program: &profile::Program) -> Machine {
    if program.report.machine == "mill" {
        Machine::MillXyz
    } else {
        Machine::LatheXz
    }
}
fn plane(path: &Value) -> Result<Plane> {
    match path["plane"].as_str().unwrap_or("XZ") {
        "XY" => Ok(Plane::Xy),
        "XZ" => Ok(Plane::Xz),
        "YZ" => Ok(Plane::Yz),
        _ => Err(fail("MODEL", "Unexpected plane")),
    }
}
fn spindle(v: &Value, unit: f64) -> Result<Spindle> {
    let clockwise = v["clockwise"]
        .as_bool()
        .ok_or_else(|| fail("MODEL", "Missing spindle direction"))?;
    match v["mode"].as_str() {
        Some("rpm") => Ok(Spindle::Rpm {
            rpm: number(&v["speed"])?,
            clockwise,
        }),
        Some("css") => Ok(Spindle::Css {
            surface_mm_per_second: number(&v["speed"])? * unit / 60.0,
            maximum_rpm: number(&v["maximumRPM"])?,
            clockwise,
        }),
        _ => Err(fail("MODEL", "Unexpected spindle mode")),
    }
}
fn coolant(v: &Value) -> Result<Coolant> {
    match v.as_str() {
        Some("off") => Ok(Coolant::Off),
        Some("flood") => Ok(Coolant::Flood),
        Some("mist") => Ok(Coolant::Mist),
        _ => Err(fail("MODEL", "Unexpected coolant")),
    }
}
fn direction(s: Spindle) -> Option<bool> {
    match s {
        Spindle::Stop => None,
        Spindle::Rpm { clockwise, .. } | Spindle::Css { clockwise, .. } => Some(clockwise),
    }
}
pub(crate) fn movement(v: &Value) -> Result<Movement> {
    use Movement::*;
    Ok(match v.as_str().unwrap_or("unspecified") {
        "unspecified" => Unspecified,
        "rapid" => Rapid,
        "cutting" => Cutting,
        "finish-cutting" => FinishCutting,
        "lead-in" => LeadIn,
        "lead-out" => LeadOut,
        "link-transition" => LinkTransition,
        "link-direct" => LinkDirect,
        "ramp-helix" => RampHelix,
        "ramp-profile" => RampProfile,
        "ramp-zig-zag" => RampZigZag,
        "ramp" => Ramp,
        "plunge" => Plunge,
        "predrill" => Predrill,
        "extended" => Extended,
        "reduced" => Reduced,
        "high-feed" => HighFeed,
        _ => return Err(fail("MODEL", "Unexpected movement class")),
    })
}
pub(crate) fn tolerance(v: &Value, unit: f64) -> Result<Tolerance> {
    match v["provenance"].as_str() {
        None | Some("missing") => Ok(Tolerance::Missing),
        Some("source-declared") => Ok(Tolerance::SourceDeclaredMm(number(&v["value"])? * unit)),
        Some("fusion:operation:tolerance") => {
            Ok(Tolerance::FusionOperationMm(number(&v["value"])? * unit))
        }
        _ => Err(fail("MODEL", "Unexpected tolerance provenance")),
    }
}
fn feed(path: &Value, unit: f64) -> Result<Feed> {
    if path["kind"] == "rapid" {
        return Ok(Feed::Rapid);
    }
    let value = number(&path["feed"]["value"])? * unit;
    match path["feed"]["mode"].as_str() {
        Some("perMinute") => Ok(Feed::PerSecond(value / 60.0)),
        Some("perRevolution") => Ok(Feed::PerRevolution {
            mm_per_rev: value,
            spindle: 0,
        }),
        _ => Err(fail("MODEL", "Unexpected dimensioned feed")),
    }
}

struct Builder<'a> {
    limits: &'a Limits,
    commands: Vec<Record>,
    spans: Vec<Span>,
    spindle: Spindle,
    requested_spindle: Option<Value>,
    coolant: Coolant,
    selected_tool: Option<u32>,
    gate_pending: bool,
    phase: Phase,
}
impl Builder<'_> {
    fn emit(&mut self, action: Action, site: Site, ordinal: Option<usize>) -> Result<()> {
        if self.commands.len() >= self.limits.output_commands {
            return Err(fail(
                "OUTPUT_COMMAND_LIMIT",
                "Complete job exceeds output-command limit; no plan was produced",
            )
            .with("limit", self.limits.output_commands)
            .with("command", self.commands.len() + 1)
            .with("phase", format!("{:?}", self.phase)));
        }
        self.commands
            .try_reserve(1)
            .map_err(|_| fail("RESOURCE", "Cannot allocate command storage"))?;
        self.commands.push(Record {
            action,
            site,
            ordinal,
        });
        Ok(())
    }
    fn policy(&mut self, action: Action) -> Result<()> {
        self.emit(action, Site::Policy, None)
    }
    fn event(&mut self, event: Command, site: Site) -> Result<()> {
        self.emit(Action::Event(event), site, None)
    }
    fn span(&mut self, phase: Phase, start: usize) {
        self.spans.push(Span {
            phase,
            commands: start..self.commands.len(),
        });
    }
    fn stop(&mut self) -> Result<()> {
        self.event(Command::Spindle(Spindle::Stop), Site::Policy)?;
        self.event(Command::Coolant(Coolant::Off), Site::Policy)?;
        self.policy(Action::ResetSpindleDemand)?;
        self.spindle = Spindle::Stop;
        self.requested_spindle = None;
        self.coolant = Coolant::Off;
        self.gate_pending = false;
        Ok(())
    }
    fn process(&mut self, s: &Value, c: &Value, unit: f64) -> Result<()> {
        let target = spindle(s, unit)?;
        // Source changes remain ordered events even if unit conversion rounds
        // two distinct source speeds onto the same binary64 canonical value.
        if self.requested_spindle.as_ref() != Some(s) {
            if direction(self.spindle).is_some() && direction(self.spindle) != direction(target) {
                self.event(Command::Spindle(Spindle::Stop), Site::Source)?;
            }
            self.event(Command::Spindle(target), Site::Source)?;
            self.spindle = target;
            self.requested_spindle = Some(s.clone());
            self.gate_pending = true;
        }
        let target = coolant(c)?;
        if target != self.coolant {
            self.event(Command::Coolant(Coolant::Off), Site::Source)?;
            if target != Coolant::Off {
                self.event(Command::Coolant(target), Site::Source)?;
            }
            self.coolant = target;
        }
        Ok(())
    }
    fn waypoints(
        &mut self,
        points: &[plan::Waypoint],
        frame: Frame,
        site: Site,
        unit: f64,
    ) -> Result<()> {
        for (ordinal, p) in points.iter().enumerate() {
            self.emit(
                Action::Waypoint {
                    frame,
                    axis: p.axis,
                    value_mm: p.value * unit,
                },
                site,
                Some(ordinal),
            )?;
        }
        Ok(())
    }
    fn motion(
        &mut self,
        geometry: Geometry,
        path: &Value,
        tolerance: Tolerance,
        unit: f64,
        ordinal: usize,
    ) -> Result<()> {
        let feed = feed(path, unit)?;
        let entry_gate = if feed != Feed::Rapid && self.gate_pending {
            self.gate_pending = false;
            EntryGate::SpindlesAtSpeed
        } else {
            EntryGate::None
        };
        self.emit(
            Action::Motion(Motion {
                geometry,
                feed,
                termination: Termination::ExactPath,
                entry_gate,
                movement: movement(&path["movement"])?,
                tolerance,
            }),
            Site::Source,
            Some(ordinal),
        )
    }
}

/// Decode internally: a caller cannot alter the public inspection model and then
/// present it as validated input. Construction is private until all audits pass.
pub fn prepare(text: &str, plan_text: &str, limits: &Limits) -> Result<PreparedPlan> {
    let program = profile::decode(text, limits)?;
    let setup = plan::parse(plan_text, &program, limits)?;
    let unit = scale(&program);
    let mut b = Builder {
        limits,
        commands: Vec::new(),
        spans: Vec::new(),
        spindle: Spindle::Stop,
        requested_spindle: None,
        coolant: Coolant::Off,
        selected_tool: None,
        gate_pending: false,
        phase: Phase::Header,
    };
    b.policy(Action::ResetModes)?;
    b.policy(Action::ClearTemporaryOffsets)?;
    b.stop()?;
    b.span(Phase::Header, 0);
    for (si, section) in array(&program.model["sections"])?.iter().enumerate() {
        b.phase = Phase::Transition(si);
        let start = b.commands.len();
        match &setup.transitions()[si] {
            plan::Transition::Retract { retract, approach } => {
                if si > 0 {
                    b.stop()?;
                }
                b.waypoints(retract, Frame::Machine, Site::Retract, unit)?;
                let map = &setup.mappings()[si];
                let changed = b.selected_tool != Some(map.tool);
                if changed {
                    b.event(Command::ToolOffset { offset: 0 }, Site::Policy)?;
                    b.event(Command::ChangeTool { tool: map.tool }, Site::Policy)?;
                    b.selected_tool = Some(map.tool);
                }
                b.policy(Action::ResetModes)?;
                if changed {
                    b.stop()?;
                }
                b.policy(Action::ClearTemporaryOffsets)?;
                b.policy(Action::SelectWorkOffset(map.work_offset))?;
                b.event(Command::ToolOffset { offset: map.offset }, Site::Policy)?;
                b.waypoints(approach, Frame::Work, Site::Approach, unit)?;
            }
            plan::Transition::Link { moves } => {
                b.waypoints(moves, Frame::Work, Site::Link, unit)?
            }
            plan::Transition::Continue => (),
        }
        b.span(Phase::Transition(si), start);
        b.phase = Phase::Entry(si);
        let start = b.commands.len();
        b.process(&section["initialSpindle"], &section["initialCoolant"], unit)?;
        b.span(Phase::Entry(si), start);
        let mut position = xyz(&section["start"])?;
        let tolerance = tolerance(&section["tolerance"], unit)?;
        for (pi, path) in array(&section["paths"])?.iter().enumerate() {
            b.phase = Phase::Path {
                section: si,
                path: pi,
            };
            let start = b.commands.len();
            b.process(&path["spindle"], &path["coolant"], unit)?;
            match path["kind"].as_str() {
                Some("dwell") => b.emit(
                    Action::Event(Command::Dwell {
                        seconds: number(&path["seconds"])?,
                    }),
                    Site::Source,
                    Some(0),
                )?,
                Some("rapid" | "linear") => {
                    for (vertex, point) in array(&path["points"])?.iter().enumerate().skip(1) {
                        let end = xyz(point)?;
                        b.motion(
                            Geometry::Line {
                                start: geometry::point(position.map(|x| x * unit)),
                                end: geometry::point(end.map(|x| x * unit)),
                            },
                            path,
                            tolerance,
                            unit,
                            vertex,
                        )?;
                        position = end;
                    }
                }
                Some("arc" | "circular") => {
                    let native = path["kind"] == "circular";
                    let plane = plane(path)?;
                    let (u, v, n) = geometry::basis(plane);
                    let source_start = xyz(&path["start"])?;
                    let source_center = xyz(&path["center"])?;
                    let full = path["fullCircle"] == true;
                    let end = if full { position } else { xyz(&path["end"])? };
                    // Revision 1 uses IJK relative to its reconstructed source
                    // start, even if a full circle's start differs by roundoff.
                    let center = if native {
                        source_center
                    } else {
                        std::array::from_fn(|i| {
                            if i == n {
                                position[i]
                            } else {
                                position[i] + (source_center[i] - source_start[i])
                            }
                        })
                    };
                    let clockwise = path["clockwise"]
                        .as_bool()
                        .ok_or_else(|| fail("MODEL", "Missing arc direction"))?;
                    let sweep = if native {
                        number(&path["sweepRadians"])?
                    } else if full {
                        TAU
                    } else {
                        let a = (position[v] - center[v]).atan2(position[u] - center[u]);
                        let z = (end[v] - center[v]).atan2(end[u] - center[u]);
                        (if clockwise { a - z } else { z - a }).rem_euclid(TAU)
                    };
                    let start_mm = position.map(|x| x * unit);
                    let end_mm = end.map(|x| x * unit);
                    let mut center_mm = center.map(|x| x * unit);
                    center_mm[n] = start_mm[n];
                    b.motion(
                        Geometry::Circular {
                            start: geometry::point(start_mm),
                            end: geometry::point(end_mm),
                            center: geometry::point(center_mm),
                            plane,
                            rotation: if clockwise {
                                Rotation::Clockwise
                            } else {
                                Rotation::Counterclockwise
                            },
                            sweep_radians: sweep,
                            axial_rise_mm: end_mm[n] - start_mm[n],
                        },
                        path,
                        tolerance,
                        unit,
                        0,
                    )?;
                    position = end;
                }
                _ => return Err(fail("MODEL", "Unexpected path kind")),
            }
            b.span(
                Phase::Path {
                    section: si,
                    path: pi,
                },
                start,
            );
        }
    }
    let start = b.commands.len();
    b.phase = Phase::End;
    b.stop()?;
    b.waypoints(setup.end(), Frame::Machine, Site::End, unit)?;
    b.event(Command::ToolOffset { offset: 0 }, Site::Policy)?;
    b.policy(Action::RestoreFeedPerMinute)?;
    b.event(Command::End, Site::Policy)?;
    b.span(Phase::End, start);
    let audit = crate::command_audit::audit(&program, &setup, &b.commands, &b.spans, limits)?;
    Ok(PreparedPlan {
        program,
        setup,
        commands: b.commands,
        spans: b.spans,
        audit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rounding_must_not_erase_a_source_spindle_event() -> Result<()> {
        let limits = Limits::default();
        let mut builder = Builder {
            limits: &limits,
            commands: Vec::new(),
            spans: Vec::new(),
            spindle: Spindle::Stop,
            requested_spindle: None,
            coolant: Coolant::Off,
            selected_tool: None,
            gate_pending: false,
            phase: Phase::Header,
        };
        // Find adjacent valid CSS source values that collapse in the canonical
        // conversion. The original translator still emits their two source events.
        let mut pair = None;
        for bits in 1.5_f64.to_bits()..1.5_f64.to_bits() + 1000 {
            let x = f64::from_bits(bits);
            let y = f64::from_bits(bits + 1);
            if x * 25.4 / 60.0 == y * 25.4 / 60.0 {
                pair = Some((x, y));
                break;
            }
        }
        let (x, y) = pair.ok_or_else(|| fail("TEST", "No conversion collision found"))?;
        for speed in [x, y] {
            builder.process(
                &json!({"mode":"css","speed":speed,"maximumRPM":1800,"clockwise":true}),
                &json!("off"),
                25.4,
            )?;
        }
        assert_eq!(builder.commands.len(), 2);
        assert_eq!(builder.commands[0].action, builder.commands[1].action);
        Ok(())
    }
}
