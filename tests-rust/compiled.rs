use nextnc_native::{
    command_audit,
    compiled::{self, Action, Frame, Phase, PreparedPlan},
    contract::{self, v2::Geometry, Command, Coolant, Feed, Spindle},
    geometry, json,
    part21::Limits,
    profile,
};
use serde_json::{json, Value};
type TestResult = Result<(), Box<dyn std::error::Error>>;

fn fixture(name: &str) -> Result<Value, Box<dyn std::error::Error>> {
    Ok(json::parse(
        &std::fs::read_to_string(format!("tests-rust/fixtures/legacy/{name}.json"))?,
        &Limits::default(),
    )?)
}
fn prepare(f: &Value) -> Result<PreparedPlan, Box<dyn std::error::Error>> {
    Ok(compiled::prepare(
        f["text"].as_str().ok_or("missing source")?,
        &f["plan"].to_string(),
        &Limits::default(),
    )?)
}
fn point(v: &Value, unit: f64) -> Result<[f64; 3], Box<dyn std::error::Error>> {
    Ok([
        v[0].as_f64().ok_or("x")? * unit,
        v[1].as_f64().ok_or("y")? * unit,
        v[2].as_f64().ok_or("z")? * unit,
    ])
}
fn feed(f: Feed) -> Value {
    match f {
        Feed::Rapid => json!(["rapid"]),
        Feed::PerSecond(v) => json!(["mm/s", v]),
        Feed::PerRevolution {
            mm_per_rev,
            spindle,
        } => json!(["mm/rev", mm_per_rev, spindle]),
    }
}
fn coolant(c: Coolant) -> &'static str {
    match c {
        Coolant::Off => "off",
        Coolant::Flood => "flood",
        Coolant::Mist => "mist",
    }
}

// These projections are intentionally test-only. They compare ordered physical
// intent with the frozen, independently audited legacy output, including every
// reviewed waypoint, state event, dwell, source vertex and source process feed.
fn native_projection(p: &PreparedPlan) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let mut out = Vec::new();
    for r in p.commands() {
        match r.action {
            Action::ResetModes => out.push(json!(["initialize"])),
            Action::ClearTemporaryOffsets => out.push(json!(["clearTemporaryOffsets"])),
            Action::ResetSpindleDemand => out.push(json!(["spindleMode", "rpm", 0, null])),
            Action::SelectWorkOffset(w) => out.push(json!(["workOffset", w])),
            Action::RestoreFeedPerMinute => out.push(json!(["restoreFeedPerMinute"])),
            Action::Waypoint {
                frame,
                axis,
                value_mm,
            } => out.push(json!([
                "waypoint",
                if frame == Frame::Machine {
                    "machine"
                } else {
                    "work"
                },
                axis,
                value_mm
            ])),
            Action::Event(Command::Spindle(s)) => match s {
                Spindle::Stop => out.push(json!(["spindleStop"])),
                Spindle::Rpm { rpm, clockwise } => {
                    out.push(json!(["spindleMode", "rpm", rpm, null]));
                    out.push(json!(["spindleStart", clockwise]));
                }
                Spindle::Css {
                    surface_mm_per_second,
                    maximum_rpm,
                    clockwise,
                } => {
                    out.push(json!([
                        "spindleMode",
                        "css",
                        surface_mm_per_second,
                        maximum_rpm
                    ]));
                    out.push(json!(["spindleStart", clockwise]));
                }
            },
            Action::Event(Command::Coolant(c)) => out.push(json!(["coolant", coolant(c)])),
            Action::Event(Command::ChangeTool { tool }) => out.push(json!(["toolChange", tool])),
            Action::Event(Command::ToolOffset { offset }) => {
                out.push(json!(["toolOffset", offset]))
            }
            Action::Event(Command::Dwell { seconds }) => out.push(json!(["dwell", seconds])),
            Action::Event(Command::End) => out.push(json!(["end"])),
            Action::Motion(m) => match m.geometry {
                Geometry::Line { start, end } => out.push(json!([
                    "line",
                    geometry::coordinates(start),
                    geometry::coordinates(end),
                    feed(m.feed)
                ])),
                Geometry::Circular {
                    start,
                    end,
                    center,
                    plane,
                    rotation,
                    ..
                } => out.push(json!([
                    "arc",
                    geometry::coordinates(start),
                    geometry::coordinates(end),
                    geometry::coordinates(center),
                    format!("{plane:?}"),
                    rotation == contract::Rotation::Clockwise,
                    feed(m.feed)
                ])),
            },
            _ => return Err("unexpected unoptimized action".into()),
        }
    }
    Ok(out)
}
fn legacy_projection(f: &Value) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let unit = if f["program"]["model"]["units"] == "inch" {
        25.4
    } else {
        1.0
    };
    let css_scale = if unit == 25.4 { 304.8 } else { 1000.0 };
    let mut out = Vec::new();
    for row in f["translation"]["sourceMap"]
        .as_array()
        .ok_or("source map")?
    {
        let c = &row["command"];
        let kind = c["type"].as_str().ok_or("command type")?;
        match kind {
            "comment" | "plane" => (),
            "feed" => {
                if c["value"].is_null() {
                    out.push(json!(["restoreFeedPerMinute"]));
                }
            }
            "initialize" | "clearTemporaryOffsets" | "spindleStop" | "end" => {
                out.push(json!([kind]))
            }
            "cancelToolOffset" => out.push(json!(["toolOffset", 0])),
            "toolChange" => out.push(json!([kind, c["tool"]])),
            "toolOffset" => out.push(json!([kind, c["offset"]])),
            "workOffset" => {
                let w = nextnc_native::plan::WORK_OFFSETS
                    .iter()
                    .position(|s| Some(*s) == c["value"].as_str())
                    .ok_or("WCS")?
                    + 1;
                out.push(json!([kind, w]));
            }
            "spindleMode" => out.push(json!([
                kind,
                c["mode"],
                c["speed"].as_f64().ok_or("speed")?
                    * if c["mode"] == "css" {
                        css_scale / 60.0
                    } else {
                        1.0
                    },
                c["maximumRPM"]
            ])),
            "spindleStart" => out.push(json!([kind, c["clockwise"]])),
            "coolant" => out.push(json!([kind, c["value"]])),
            "dwell" => out.push(json!([kind, c["seconds"]])),
            "rapid" | "linear" | "arc" => {
                if row["phase"] != "toolpath" {
                    let axes = c["axes"].as_object().ok_or("waypoint axes")?;
                    assert_eq!(axes.len(), 1);
                    let (name, value) = axes.iter().next().ok_or("waypoint")?;
                    let axis = ["x", "y", "z"]
                        .iter()
                        .position(|x| x == name)
                        .ok_or("axis")?;
                    out.push(json!([
                        "waypoint",
                        c["frame"],
                        axis,
                        value.as_f64().ok_or("coordinate")? * unit
                    ]));
                    continue;
                }
                let state = &row["modalState"];
                let f = if kind == "rapid" {
                    Feed::Rapid
                } else {
                    let v = state["feedRate"].as_f64().ok_or("feed")? * unit;
                    if state["feedMode"] == "perRevolution" {
                        Feed::PerRevolution {
                            mm_per_rev: v,
                            spindle: 0,
                        }
                    } else {
                        Feed::PerSecond(v / 60.0)
                    }
                };
                let m = &row["motion"];
                let start = point(&m["start"], unit)?;
                let end = point(&m["end"], unit)?;
                if kind == "arc" {
                    let source_start = point(&m["start"], 1.0)?;
                    let offset = point(&m["centerOffset"], 1.0)?;
                    let center =
                        std::array::from_fn::<_, 3, _>(|i| (source_start[i] + offset[i]) * unit);
                    let plane = match c["plane"].as_str().ok_or("plane")? {
                        "XY" => "Xy",
                        "XZ" => "Xz",
                        "YZ" => "Yz",
                        _ => return Err("plane".into()),
                    };
                    out.push(json!([
                        "arc",
                        start,
                        end,
                        center,
                        plane,
                        c["clockwise"],
                        feed(f)
                    ]));
                } else {
                    out.push(json!(["line", start, end, feed(f)]));
                }
            }
            _ => return Err(format!("unhandled oracle command {kind}").into()),
        }
    }
    Ok(out)
}
fn equivalent(a: &Value, b: &Value) {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            let a = a.as_f64().unwrap_or(f64::NAN);
            let b = b.as_f64().unwrap_or(f64::NAN);
            assert_eq!(a, b);
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len());
            if a.first() == Some(&json!("spindleMode")) && a.get(1) == Some(&json!("css")) {
                // Only CSS passes through an intermediate metres/feet-per-minute
                // conversion in the legacy output. All geometry is compared exactly.
                let x = a[2].as_f64().unwrap_or(f64::NAN);
                let y = b[2].as_f64().unwrap_or(f64::NAN);
                assert!((x - y).abs() <= f64::EPSILON * 8.0 * x.abs().max(y.abs()).max(1.0));
                equivalent(&a[0], &b[0]);
                equivalent(&a[1], &b[1]);
                equivalent(&a[3], &b[3]);
                return;
            }
            for (a, b) in a.iter().zip(b) {
                equivalent(a, b);
            }
        }
        _ => assert_eq!(a, b),
    }
}
#[test]
fn complete_ordered_motion_events_and_transitions_match_frozen_legacy_oracle() -> TestResult {
    for name in ["lathe-mm", "lathe-inch", "mill-mm", "mill-inch"] {
        let f = fixture(name)?;
        let p = prepare(&f)?;
        let native = native_projection(&p)?;
        let legacy = legacy_projection(&f)?;
        assert_eq!(native.len(), legacy.len(), "{name}");
        for (a, b) in native.iter().zip(&legacy) {
            equivalent(a, b);
        }
        assert!(!p.audit().execution_authorized);
        for i in 0..p.commands().len() {
            let (s, r) = p.source_for(i).ok_or("source lookup")?;
            assert!(s.commands.contains(&i));
            assert_eq!(r, &p.commands()[i]);
            let provenance = p.provenance_for(i).ok_or("provenance")?;
            assert!(!provenance["phase"].is_null());
            assert!(
                provenance.to_string().len() < 10_000,
                "unbounded source metadata clone"
            );
        }
        assert!(p.source_for(p.commands().len()).is_none());
        assert_eq!(p.commands(), prepare(&f)?.commands());
    }
    Ok(())
}

#[test]
fn prepared_requirements_include_policy_and_setup_beyond_source_capabilities() -> TestResult {
    let text = include_str!("fixtures/native/synthetic-1.stpnc");
    let source = profile::decode(text, &Limits::default())?;
    let prepared = compiled::prepare(
        text,
        &synthetic_setup(&source)?.to_string(),
        &Limits::default(),
    )?;
    let mut manifest = json!({"schema":"nextnc-native/target-capabilities/1","machine":source.report.machine,
        "motionContractVersion":2,"capabilities":source.report.required_capabilities,"evidenceSha256":"0".repeat(64)});
    nextnc_native::capabilities::check(Some(&manifest.to_string()), &source, &Limits::default())?;
    let error = nextnc_native::capabilities::check_prepared(
        Some(&manifest.to_string()),
        &prepared,
        &Limits::default(),
    )
    .err()
    .ok_or("source-only capabilities accepted")?;
    assert_eq!(error.code, "UNSUPPORTED_CAPABILITIES");
    assert_eq!(error.context["unsupported"], json!(["at-speed", "linear"]));
    manifest["capabilities"] = json!(prepared.audit().required_capabilities);
    nextnc_native::capabilities::check_prepared(
        Some(&manifest.to_string()),
        &prepared,
        &Limits::default(),
    )?;
    Ok(())
}
fn synthetic_setup(p: &profile::Program) -> Result<Value, Box<dyn std::error::Error>> {
    let axes = if p.report.machine == "mill" {
        vec![0, 1, 2]
    } else {
        vec![0, 2]
    };
    let names = ["x", "y", "z"];
    let retract: Vec<_> = axes.iter().map(|i| json!({names[*i]:0})).collect();
    let mut tools = serde_json::Map::new();
    let mut work = serde_json::Map::new();
    let mut sections = Vec::new();
    for s in p.model["sections"].as_array().ok_or("sections")? {
        let tool = s["tool"]["number"].as_u64().ok_or("tool")?;
        let offset = s["tool"]["offset"].as_u64().ok_or("offset")?;
        tools.insert(
            format!("{tool}:{offset}"),
            json!({"tool":tool.max(1),"offset":offset.max(1)}),
        );
        work.insert(s["workOffset"].to_string(), json!("G54"));
        let approach: Vec<_> = axes
            .iter()
            .map(|i| json!({names[*i]:s["start"][*i]}))
            .collect();
        sections.push(json!({"mode":"retract","retract":retract,"approach":approach}));
    }
    Ok(
        json!({"schema":"linuxcnc-next-nc/execution-plan/4","machine":p.report.machine,"units":p.report.units,
        "programFingerprint":p.report.fingerprint.value,"tools":tools,"workOffsets":work,"sections":sections,"end":retract}),
    )
}
#[test]
fn all_native_engine_and_synthetic_profiles_prepare_without_losing_analytic_geometry() -> TestResult
{
    let mut count = 0;
    let mut helical = 0;
    let mut multi = 0;
    for entry in std::fs::read_dir("tests-rust/fixtures/native")? {
        let file = entry?.path();
        if file.extension().and_then(|s| s.to_str()) != Some("stpnc") {
            continue;
        }
        let text = std::fs::read_to_string(&file)?;
        let source = profile::decode(&text, &Limits::default())?;
        let setup_text = synthetic_setup(&source)?.to_string();
        let p = compiled::prepare(&text, &setup_text, &Limits::default())
            .map_err(|e| format!("{}: {e}", file.display()))?;
        let artifact = nextnc_native::bundle::compile(
            nextnc_native::bundle::Inputs {
                source: &text,
                setup: &setup_text,
                tool_table: None,
                target: None,
            },
            &Limits::default(),
        )?;
        assert_eq!(
            artifact.prepared().commands(),
            p.commands(),
            "native bundle {}",
            file.display()
        );
        assert_eq!(artifact.prepared().spans(), p.spans());
        let circular_source: usize = source.model["sections"]
            .as_array()
            .ok_or("sections")?
            .iter()
            .map(|s| {
                s["paths"].as_array().map_or(0, |ps| {
                    ps.iter().filter(|p| p["kind"] == "circular").count()
                })
            })
            .sum();
        let mut circular = 0;
        for r in p.commands() {
            if let Action::Motion(m) = r.action {
                if let Geometry::Circular {
                    sweep_radians,
                    axial_rise_mm,
                    ..
                } = m.geometry
                {
                    circular += 1;
                    if axial_rise_mm != 0.0 {
                        helical += 1;
                    }
                    if sweep_radians > std::f64::consts::TAU {
                        multi += 1;
                    }
                }
            }
        }
        assert_eq!(circular, circular_source);
        count += 1;
    }
    assert_eq!(count, 130);
    assert!(helical > 0 && multi > 0);
    Ok(())
}
#[test]
fn command_limit_is_enforced_before_plan_construction() -> TestResult {
    let f = fixture("mill-mm")?;
    let p = prepare(&f)?;
    let source = f["text"].as_str().ok_or("source")?;
    let plan = f["plan"].to_string();
    for limit in [1, p.commands().len() - 1] {
        let limits = Limits {
            output_commands: limit,
            ..Limits::default()
        };
        assert_eq!(
            compiled::prepare(source, &plan, &limits)
                .err()
                .ok_or("limit accepted")?
                .code,
            "OUTPUT_COMMAND_LIMIT"
        );
    }
    compiled::prepare(
        source,
        &plan,
        &Limits {
            output_commands: p.commands().len(),
            ..Limits::default()
        },
    )?;
    Ok(())
}

#[test]
fn repeated_and_backtracking_vertices_remain_distinct_ordered_uses() -> TestResult {
    let text = include_str!("fixtures/legacy/mill-mm.stpnc");
    let original = "#263=POLYLINE('',(#225,#262));";
    assert!(text.contains(original));
    let text = text.replace(original, "#263=POLYLINE('',(#225,#225,#262,#225,#262));");
    let source = profile::decode(&text, &Limits::default())?;
    let p = compiled::prepare(
        &text,
        &synthetic_setup(&source)?.to_string(),
        &Limits::default(),
    )?;
    let span = p
        .spans()
        .iter()
        .find(|s| {
            if let Phase::Path { section, path } = s.phase {
                source.model["sections"][section]["paths"][path]["points"]
                    .as_array()
                    .is_some_and(|a| a.len() == 5)
            } else {
                false
            }
        })
        .ok_or("expanded path span")?;
    let uses: Vec<_> = p.commands()[span.commands.clone()]
        .iter()
        .filter_map(|r| {
            if let Action::Motion(m) = r.action {
                Some((r.ordinal, m.geometry))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(uses.len(), 4);
    let a = geometry::point([4.0, 4.0, 3.0]);
    let b = geometry::point([3.0, 3.0, 1.0]);
    for ((ordinal, geometry), (i, (start, end))) in uses
        .iter()
        .zip([(a, a), (a, b), (b, a), (a, b)].into_iter().enumerate())
    {
        assert_eq!(*ordinal, Some(i + 1));
        assert_eq!(*geometry, Geometry::Line { start, end });
    }
    Ok(())
}
#[test]
fn independent_audit_rejects_each_missing_or_replaced_command_and_changed_source_use() -> TestResult
{
    let f = fixture("mill-mm")?;
    let p = prepare(&f)?;
    for index in 0..p.commands().len() {
        let mut commands = p.commands().to_vec();
        commands.remove(index);
        let mut spans = p.spans().to_vec();
        for span in &mut spans {
            if span.commands.start > index {
                span.commands.start -= 1;
            }
            if span.commands.end > index {
                span.commands.end -= 1;
            }
        }
        assert!(
            command_audit::audit(
                p.program(),
                p.setup(),
                &commands,
                &spans,
                &Limits::default()
            )
            .is_err(),
            "missing {index}"
        );
        let mut commands = p.commands().to_vec();
        commands[index].action = if commands[index].action == Action::RestoreFeedPerMinute {
            Action::ClearTemporaryOffsets
        } else {
            Action::RestoreFeedPerMinute
        };
        assert!(
            command_audit::audit(
                p.program(),
                p.setup(),
                &commands,
                p.spans(),
                &Limits::default()
            )
            .is_err(),
            "replaced {index}"
        );
        let mut commands = p.commands().to_vec();
        commands[index].ordinal = Some(usize::MAX);
        assert!(
            command_audit::audit(
                p.program(),
                p.setup(),
                &commands,
                p.spans(),
                &Limits::default()
            )
            .is_err(),
            "origin {index}"
        );
    }
    let index = p
        .commands()
        .iter()
        .position(|r| matches!(r.action, Action::Motion(_)))
        .ok_or("motion")?;
    for mutation in 0..5 {
        let mut commands = p.commands().to_vec();
        if let Action::Motion(ref mut m) = commands[index].action {
            match mutation {
                0 => m.feed = Feed::PerSecond(999.0),
                1 => m.entry_gate = contract::EntryGate::None,
                2 => {
                    m.termination = contract::Termination::Blend {
                        max_deviation_mm: 0.01,
                    }
                }
                3 => m.movement = contract::v2::Movement::Ramp,
                _ => match &mut m.geometry {
                    Geometry::Line { end, .. } | Geometry::Circular { end, .. } => end.x += 0.01,
                },
            }
        }
        assert!(
            command_audit::audit(
                p.program(),
                p.setup(),
                &commands,
                p.spans(),
                &Limits::default()
            )
            .is_err(),
            "motion mutation {mutation}"
        );
    }
    let mut spans = p.spans().to_vec();
    spans[0].phase = Phase::End;
    assert!(command_audit::audit(
        p.program(),
        p.setup(),
        p.commands(),
        &spans,
        &Limits::default()
    )
    .is_err());
    Ok(())
}
