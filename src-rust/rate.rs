//! Offline nominal demand and measured-reference screening, never admission.
//! Prepared actions and guarded-mailbox calls are distinct domains. The report
//! makes their unqualified mapping explicit instead of inventing native capacity.
use crate::{
    bundle,
    compiled::{self, Action, PreparedPlan},
    contract::{v2::Geometry, Command, EntryGate, Feed},
    geometry, json,
    part21::Limits,
    Diagnostic, Result,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const REFERENCE_LIMIT: usize = 1024 * 1024;
/// A diagnostic smoothing window, not a planner queue size or an admission limit.
const WINDOW: usize = 64;

fn fail(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("rate-reference", "REFERENCE_EVIDENCE", message)
        .with("inputRole", "admission-reference")
}
fn insist(value: bool, message: &str) -> Result<()> {
    if value {
        Ok(())
    } else {
        Err(fail(message))
    }
}
fn uint(value: &Value) -> Result<u64> {
    value
        .as_u64()
        .ok_or_else(|| fail("Missing or noninteger observation count/time"))
}
fn number(value: &Value) -> Result<f64> {
    value
        .as_f64()
        .filter(|v| v.is_finite())
        .ok_or_else(|| fail("Missing or nonfinite observation value"))
}
fn hash(value: &Value, length: usize) -> Result<String> {
    let value = value
        .as_str()
        .ok_or_else(|| fail("Missing source/hash identity"))?;
    insist(
        value.len() == length
            && value
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)),
        "Invalid source/hash identity",
    )?;
    Ok(value.into())
}

#[derive(Clone, Debug, Serialize)]
pub struct Sample {
    case: String,
    machine: String,
    shape: String,
    observations: u64,
    motion_calls: u64,
    state_calls: u64,
    elapsed_ns: u64,
    observed_all_calls_per_second: f64,
    service_ns: Value,
    mailbox_sha256: String,
    geometry_trace_sha256: String,
}

/// Construction checks a complete Stage 0 summary. It does not authenticate a
/// measurement author or rerun the separately archived raw trace auditor.
#[derive(Clone, Debug, Serialize)]
pub struct Reference {
    schema: &'static str,
    input_sha256: String,
    source: String,
    binaries: BTreeMap<String, String>,
    instrumentation: String,
    samples: Vec<Sample>,
    native_executor_qualified: bool,
    capacity_limit_calls_per_second: Option<f64>,
    verification_scope: &'static str,
}
impl Reference {
    pub fn parse(text: &str) -> Result<Self> {
        insist(
            text.len() <= REFERENCE_LIMIT,
            "Reference report exceeds its byte limit",
        )?;
        let limits = Limits {
            input_bytes: REFERENCE_LIMIT,
            nesting: 24,
            values: 100_000,
            ..Limits::default()
        };
        let report = json::parse(text, &limits).map_err(|mut error| {
            error.stage = "rate-reference".into();
            error.with("inputRole", "admission-reference")
        })?;
        insist(
            report["schema"] == "nextnc-stage0/admission-simulation/1" && report["passed"] == true,
            "Require a completed, passing reference admission matrix",
        )?;
        let source = hash(&report["source"], 40)?;
        let mut binaries = BTreeMap::new();
        for name in [
            "bin/milltask",
            "lib/librs274.so",
            "rtlib/motmod.so",
            "rtlib/motion_shim.so",
        ] {
            binaries.insert(name.into(), hash(&report["binaries"][name], 64)?);
        }
        let instrumentation = report["instrumentation"]
            .as_str()
            .ok_or_else(|| fail("Missing instrumentation identity"))?;
        let mut fields = BTreeMap::new();
        for line in instrumentation.lines() {
            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| fail("Invalid instrumentation identity"))?;
            insist(
                fields.insert(key, value).is_none(),
                "Duplicate instrumentation identity",
            )?;
        }
        insist(
            fields.len() == 3 && fields.get("source").copied() == Some(source.as_str()),
            "Instrumentation source differs",
        )?;
        for key in ["original_sha256", "instrumented_sha256"] {
            hash(
                &Value::String(
                    fields
                        .get(key)
                        .ok_or_else(|| fail("Missing instrumentation hash"))?
                        .to_string(),
                ),
                64,
            )?;
        }
        let rows = report["results"]
            .as_array()
            .ok_or_else(|| fail("Missing case results"))?;
        insist(
            rows.len() == 21,
            "Reference matrix must contain all 21 repeated cases",
        )?;
        let mut expected = BTreeSet::new();
        for machine in ["mill", "lathe"] {
            for shape in ["line", "line_state", "arc", "helix"] {
                if machine == "lathe" && shape == "helix" {
                    continue;
                }
                for repeat in 1..=3 {
                    expected.insert(format!("admission_{machine}_{shape}_{repeat}"));
                }
            }
        }
        let mut samples = Vec::with_capacity(21);
        for row in rows {
            let name = row["case"]
                .as_str()
                .ok_or_else(|| fail("Missing case name"))?;
            insist(expected.remove(name), "Unknown or duplicate reference case")?;
            let sample = (|| -> Result<Sample> {
                insist(
                    row["passed"] == true
                        && row["source"] == source
                        && row["in_position"] == true
                        && row["shim_done"] == true
                        && row["final_task_state"] == 4
                        && row.get("error").is_none()
                        && row.get("cleanup_failed").is_none()
                        && row.get("abort_error").is_none(),
                    "Case completion or cleanup is unqualified",
                )?;
                insist(
                    row["messages"].as_array().is_some_and(Vec::is_empty),
                    "Reference case contains controller messages",
                )?;
                insist(
                    (0.0..=1e-7).contains(&number(&row["endpoint_error_mm"])?),
                    "Reference endpoint check failed",
                )?;
                for key in ["enable", "armed", "active", "draining", "fault"] {
                    insist(
                        row["shaper"][key] == false,
                        "Reference requires disabled/inactive shaping",
                    )?;
                }
                let observation = &row["admission"];
                let audit = &observation["trace_audit"];
                insist(
                    observation["schema"] == "nextnc-stage0/admission-observation/1"
                        && observation["case"] == name
                        && observation["source"] == source
                        && observation["hardware"] == "NOT_TESTED"
                        && observation["findings"]
                            .as_array()
                            .is_some_and(Vec::is_empty),
                    "Invalid observation scope or audit",
                )?;
                insist(
                    audit["schema"] == "nextnc-stage0/task-trace/1"
                        && audit["case"] == name
                        && audit["source"] == source
                        && audit["result"] == "PASS"
                        && audit["hardware"] == "NOT_TESTED",
                    "Missing matching geometry audit",
                )?;
                let mailbox_sha256 = hash(&row["admission_trace_sha256"], 64)?;
                let geometry_trace_sha256 = hash(&row["trace_sha256"], 64)?;
                insist(
                    observation["input_sha256"] == mailbox_sha256
                        && audit["trace_sha256"] == geometry_trace_sha256,
                    "Reference trace identities differ",
                )?;
                let machine = observation["machine"]
                    .as_str()
                    .ok_or_else(|| fail("Missing machine"))?;
                let shape = observation["shape"]
                    .as_str()
                    .ok_or_else(|| fail("Missing shape"))?;
                insist(
                    name.starts_with(&format!("admission_{machine}_{shape}_")),
                    "Case shape/machine differs",
                )?;
                let position = row["final_position"]
                    .as_array()
                    .ok_or_else(|| fail("Missing final position"))?;
                insist(position.len() == 9, "Invalid final pose dimension")?;
                let mut endpoint = [0.0_f64; 9];
                match (machine, shape) {
                    ("mill", "line") => endpoint[0] = 0.01024,
                    ("mill", "line_state") => endpoint[0] = 0.00512,
                    ("mill", "arc" | "helix") => endpoint[0] = 0.002,
                    ("lathe", "line") => endpoint[2] = -0.01024,
                    ("lathe", "line_state") => endpoint[2] = -0.00512,
                    ("lathe", "arc") => {
                        endpoint[0] = 0.008;
                        endpoint[2] = 0.002;
                    }
                    _ => return Err(fail("Unknown machine/shape combination")),
                }
                for (actual, expected) in position.iter().zip(endpoint) {
                    insist(
                        (number(actual)? - expected).abs() <= 1e-7,
                        "Final pose disagrees with the reference fixture",
                    )?;
                }
                let window = &observation["motion_window"];
                let observations = uint(&observation["observations"])?;
                let first = uint(&window["first_record"])?;
                let last = uint(&window["last_record"])?;
                let calls = uint(&window["all_commands"])?;
                let elapsed = uint(&window["elapsed_ns"])?;
                insist(
                    observations <= 100_000
                        && first <= last
                        && last < observations
                        && calls == last - first + 1
                        && elapsed > 0
                        && elapsed <= 120_000_000_000,
                    "Invalid observation window",
                )?;
                let lines = uint(&window["counts"]["line"])?;
                let arcs = uint(&window["counts"]["arc"])?;
                let states = uint(&window["counts"]["state"])?;
                let counts = match shape {
                    "line" => (1024, 0, 0),
                    "line_state" => (512, 0, 511),
                    "arc" | "helix" => (1, 256, 0),
                    _ => return Err(fail("Unknown fixture shape")),
                };
                insist(
                    (lines, arcs, states) == counts && calls == lines + arcs + states,
                    "Reference primitive/state expansion differs from its audited fixture",
                )?;
                let trace_records = uint(&audit["records"])?;
                insist(
                    trace_records >= lines + arcs && trace_records <= observations,
                    "Geometry trace record count contradicts observations",
                )?;
                let rate = calls as f64 * 1e9 / elapsed as f64;
                insist(
                    (number(&window["observed_commands_per_second"])? - rate).abs() <= rate * 1e-12,
                    "Reported rate contradicts measured counts/time",
                )?;
                let completed_seconds = number(&row["duration_s"])?;
                insist(
                    (0.0..=120.0).contains(&completed_seconds)
                        && completed_seconds >= elapsed as f64 / 1e9,
                    "Completion duration contradicts observation window",
                )?;
                let mut service_count = 0;
                for category in ["line", "arc", "state"] {
                    let distribution = &observation["service_ns"][category];
                    let count = uint(&distribution["samples"])?;
                    insist(
                        count <= observations,
                        "Service sample count exceeds observations",
                    )?;
                    service_count += count;
                    insist(
                        count >= uint(&window["counts"][category])?,
                        "Missing service observations",
                    )?;
                    if category != "state" {
                        insist(
                            count == uint(&window["counts"][category])?,
                            "Motion observations outside the first/last motion window",
                        )?;
                    }
                    if count > 0 {
                        let values = [
                            uint(&distribution["min"])?,
                            uint(&distribution["median"])?,
                            uint(&distribution["p95"])?,
                            uint(&distribution["max"])?,
                        ];
                        insist(
                            values.windows(2).all(|v| v[0] <= v[1]) && values[3] <= 120_000_000_000,
                            "Contradictory service distribution",
                        )?;
                    }
                }
                insist(
                    service_count == observations,
                    "Service counts do not cover every observation",
                )?;
                Ok(Sample {
                    case: name.into(),
                    machine: machine.into(),
                    shape: shape.into(),
                    observations,
                    motion_calls: lines + arcs,
                    state_calls: states,
                    elapsed_ns: elapsed,
                    observed_all_calls_per_second: rate,
                    service_ns: observation["service_ns"].clone(),
                    mailbox_sha256,
                    geometry_trace_sha256,
                })
            })()
            .map_err(|error| error.with("referenceCase", name))?;
            samples.push(sample);
        }
        insist(expected.is_empty(), "Incomplete repeated reference matrix")?;
        samples.sort_by(|a, b| a.case.cmp(&b.case));
        Ok(Self { schema: "nextnc-native/checked-reference/1", input_sha256: bundle::digest(text.as_bytes()), source, binaries,
            instrumentation: instrumentation.into(), samples, native_executor_qualified: false, capacity_limit_calls_per_second: None,
            verification_scope: "Checks complete summary, identities, counts, timing arithmetic and recorded audits; does not authenticate the measurement author or reexecute the separately archived raw-trace audit" })
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Counts {
    pub prepared_commands: usize,
    pub motion_commands: usize,
    pub line_commands: usize,
    pub planar_circular_commands: usize,
    pub helical_commands: usize,
    pub multi_turn_commands: usize,
    pub state_commands: usize,
    pub state_by_kind: BTreeMap<&'static str, usize>,
    pub reviewed_waypoints: usize,
    pub rapid_motions: usize,
    pub per_revolution_motions: usize,
    pub at_speed_gates: usize,
    pub zero_length_feed_motions: usize,
    pub numeric_duration_unavailable: usize,
    pub source_geometry_paths: usize,
}
#[derive(Clone, Debug, Serialize)]
pub struct Window {
    pub first_command: usize,
    pub last_command: usize,
    pub prepared_commands: usize,
    pub motion_commands: usize,
    pub state_commands: usize,
    pub nominal_feed_and_dwell_seconds: f64,
    pub nominal_prepared_commands_per_second: f64,
}
#[derive(Clone, Debug, Serialize)]
pub struct Warning {
    code: &'static str,
    message: &'static str,
}
#[derive(Clone, Debug, Serialize)]
pub struct Report {
    schema: &'static str,
    policy: &'static str,
    program_fingerprint: crate::profile::Fingerprint,
    machine: String,
    counts: Counts,
    known_feed_and_dwell_seconds: Option<f64>,
    densest_nominal_window: Option<Window>,
    densest_window_source_uses: Option<[Option<Value>; 2]>,
    diagnostic_window_commands: usize,
    reference: Option<Reference>,
    observed_reference_all_call_rate_range: Option<[f64; 2]>,
    warnings: Vec<Warning>,
    native_capacity_commands_per_second: Option<f64>,
    predicted_native_mailbox_calls: Option<usize>,
    predicted_downstream_pieces: Option<usize>,
    execution_authorized: bool,
    interpretation: [&'static str; 6],
}
impl Report {
    pub fn counts(&self) -> &Counts {
        &self.counts
    }
    pub fn nominal_window(&self) -> Option<&Window> {
        self.densest_nominal_window.as_ref()
    }
}
#[derive(Clone, Copy)]
struct Item {
    command: usize,
    seconds: f64,
    motion: bool,
}

fn candidate(items: &VecDeque<Item>) -> Option<Window> {
    let duration: f64 = items.iter().map(|i| i.seconds).sum();
    let motions = items.iter().filter(|i| i.motion).count();
    let rate = items.len() as f64 / duration;
    if motions == 0 || duration <= 0.0 || !duration.is_finite() || !rate.is_finite() {
        return None;
    }
    Some(Window {
        first_command: items.front()?.command,
        last_command: items.back()?.command,
        prepared_commands: items.len(),
        motion_commands: motions,
        state_commands: items.len() - motions,
        nominal_feed_and_dwell_seconds: duration,
        nominal_prepared_commands_per_second: rate,
    })
}
fn consider(items: &VecDeque<Item>, best: &mut Option<Window>) {
    if let Some(window) = candidate(items) {
        if best.as_ref().is_none_or(|b| {
            window.nominal_prepared_commands_per_second > b.nominal_prepared_commands_per_second
        }) {
            *best = Some(window);
        }
    }
}

/// Every action is counted; geometry expansion is read from the actual audited
/// plan. Unknown motion durations split windows instead of borrowing time from a
/// different operation. State/procedure waits are intentionally not estimated.
pub fn describe(plan: &PreparedPlan, reference: Option<&Reference>) -> Result<Report> {
    let mut counts = Counts::default();
    let mut time = 0.0;
    let mut time_finite = true;
    let mut window = VecDeque::with_capacity(WINDOW);
    let mut best = None;
    for section in compiled::array(&plan.program().model["sections"])? {
        counts.source_geometry_paths += compiled::array(&section["paths"])?
            .iter()
            .filter(|p| p["kind"] != "dwell")
            .count();
    }
    for (index, record) in plan.commands().iter().enumerate() {
        counts.prepared_commands += 1;
        let (duration, motion) = match record.action {
            Action::Motion(m) => {
                counts.motion_commands += 1;
                counts.at_speed_gates += usize::from(m.entry_gate != EntryGate::None);
                match m.geometry {
                    Geometry::Line { .. } => counts.line_commands += 1,
                    Geometry::Circular {
                        axial_rise_mm,
                        sweep_radians,
                        ..
                    } => {
                        if axial_rise_mm == 0.0 {
                            counts.planar_circular_commands += 1;
                        } else {
                            counts.helical_commands += 1;
                        }
                        counts.multi_turn_commands +=
                            usize::from(sweep_radians > std::f64::consts::TAU);
                    }
                }
                let seconds = match m.feed {
                    Feed::Rapid => {
                        counts.rapid_motions += 1;
                        None
                    }
                    Feed::PerRevolution { .. } => {
                        counts.per_revolution_motions += 1;
                        None
                    }
                    Feed::PerSecond(mm_per_second) => {
                        let length = geometry::validate_with_floor(
                            m.geometry,
                            1e-7 * compiled::scale(plan.program()),
                        )?
                        .length_mm;
                        let seconds = length / mm_per_second;
                        if !seconds.is_finite() || (length > 0.0 && seconds == 0.0) {
                            counts.numeric_duration_unavailable += 1;
                            None
                        } else {
                            counts.zero_length_feed_motions += usize::from(length == 0.0);
                            Some(seconds)
                        }
                    }
                };
                (seconds, true)
            }
            Action::Waypoint { .. } => {
                counts.reviewed_waypoints += 1;
                (None, false)
            }
            action => {
                counts.state_commands += 1;
                let (name, seconds) = match action {
                    Action::ResetModes => ("reset-modes", 0.0),
                    Action::ClearTemporaryOffsets => ("clear-temporary-offsets", 0.0),
                    Action::ResetSpindleDemand => ("reset-spindle-demand", 0.0),
                    Action::SelectWorkOffset(_) => ("select-work-offset", 0.0),
                    Action::RestoreFeedPerMinute => ("restore-feed-per-minute", 0.0),
                    Action::Event(e) => match e {
                        Command::Spindle(_) => ("spindle", 0.0),
                        Command::Coolant(_) => ("coolant", 0.0),
                        Command::ChangeTool { .. } => ("change-tool", 0.0),
                        Command::ToolOffset { .. } => ("tool-offset", 0.0),
                        Command::Dwell { seconds } => ("dwell", seconds),
                        Command::Fence => ("fence", 0.0),
                        Command::End => ("end", 0.0),
                        Command::Motion(_) => {
                            return Err(fail("Unexpected revision-1 motion action"))
                        }
                    },
                    _ => return Err(fail("Unexpected action")),
                };
                *counts.state_by_kind.entry(name).or_default() += 1;
                (Some(seconds), false)
            }
        };
        if let Some(seconds) = duration {
            time += seconds;
            time_finite &= time.is_finite();
            if window.len() == WINDOW {
                window.pop_front();
            }
            window.push_back(Item {
                command: index,
                seconds,
                motion,
            });
            if window.len() == WINDOW {
                consider(&window, &mut best);
            }
        } else {
            consider(&window, &mut best);
            window.clear();
        }
    }
    consider(&window, &mut best);
    let rates = reference.map(|r| {
        r.samples
            .iter()
            .filter(|s| s.machine == plan.program().report.machine)
            .fold([f64::INFINITY, 0.0_f64], |range, s| {
                [
                    range[0].min(s.observed_all_calls_per_second),
                    range[1].max(s.observed_all_calls_per_second),
                ]
            })
    });
    let mut warnings = Vec::new();
    if let (Some(demand), Some(range)) = (&best, rates) {
        if demand.nominal_prepared_commands_per_second > range[1] {
            warnings.push(Warning { code: "NOMINAL_DENSITY_ABOVE_REFERENCE", message: "Nominal prepared-command density exceeds every recorded reference all-call rate for this machine. This screens potential delivery pressure, not native starvation: state lowering, planner dynamics, queues and procedure waits are not equivalent or qualified." });
        }
    }
    if counts.per_revolution_motions > 0 {
        warnings.push(Warning { code: "SYNCHRONIZED_DURATION_UNKNOWN", message: "Per-revolution/CSS timing requires qualified live spindle synchronization. No nominal-RPM conversion was used." });
    }
    if counts.zero_length_feed_motions > 0
        || counts.numeric_duration_unavailable > 0
        || !time_finite
    {
        warnings.push(Warning { code: "NOMINAL_DURATION_UNRESOLVED", message: "Zero-length source uses or numeric duration limits prevent a finite rate estimate for some intervals. Their commands remain counted and preserved." });
    }
    let densest_window_source_uses = best.as_ref().map(|w| {
        [
            plan.provenance_for(w.first_command),
            plan.provenance_for(w.last_command),
        ]
    });
    Ok(Report { schema: "nextnc-native/command-demand/1", policy: compiled::POLICY,
        program_fingerprint: plan.program().report.fingerprint.clone(), machine: plan.program().report.machine.clone(),
        counts, known_feed_and_dwell_seconds: time_finite.then_some(time), densest_nominal_window: best, densest_window_source_uses,
        diagnostic_window_commands: WINDOW, reference: reference.cloned(), observed_reference_all_call_rate_range: rates, warnings,
        native_capacity_commands_per_second: None, predicted_native_mailbox_calls: None, predicted_downstream_pieces: None, execution_authorized: false,
        interpretation: [
            "Demand counts actual prepared actions, including state commands and polyline expansion; each analytic circle/helix stays one motion command, including multiple turns",
            "Nominal windows use path length/programmed mm/s plus explicit dwell time. State/procedure and at-speed waits, acceleration, jerk and shaping are not time predictions; this is earliest ideal demand screening",
            "The 64-command sliding diagnostic window is not a planner queue size. Unknown rapid, live-start and per-revolution durations split windows; finite shorter runs are also considered",
            "Prepared actions and reference guarded-mailbox calls are different domains. Their comparison is advisory screening, never a utilization or capacity bound; native state lowering and downstream piece expansion remain unqualified",
            "Reference delivery includes producer/planner delays and instrumentation overhead; service-call time is retained separately. Low or absent warnings do not establish feasible motion, sufficient admission or machine readiness",
            "The report changes no command, feed, event, tolerance, artifact selection or execution permission. Measurement identity is not cached as permission to run",
        ] })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;
    const REFERENCE: &str = include_str!("../tests-rust/fixtures/admission/reference.json");
    fn fixture(name: &str) -> Result<PreparedPlan> {
        let base = format!("tests-rust/fixtures/benchmark/{name}");
        let source =
            std::fs::read_to_string(format!("{base}.stpnc")).map_err(|e| fail(e.to_string()))?;
        let setup = std::fs::read_to_string(format!("{base}.plan.json"))
            .map_err(|e| fail(e.to_string()))?;
        compiled::prepare(&source, &setup, &Limits::default())
    }
    #[test]
    fn complete_reference_retains_identity_state_calls_and_workload_variation() -> TestResult {
        let r = Reference::parse(REFERENCE)?;
        assert_eq!(
            r.input_sha256,
            "f5ce23faeb6cf46d262091c8a531e14b9b9124910884d607f20ecdf4312edd5c"
        );
        assert_eq!(r.samples.len(), 21);
        assert_eq!(r.binaries.len(), 4);
        assert!(!r.native_executor_qualified && r.capacity_limit_calls_per_second.is_none());
        let state = r
            .samples
            .iter()
            .find(|s| s.case == "admission_mill_line_state_1")
            .ok_or("state case")?;
        assert_eq!((state.motion_calls, state.state_calls), (512, 511));
        assert!(state.observed_all_calls_per_second > 990.0);
        let helix = r
            .samples
            .iter()
            .find(|s| s.case == "admission_mill_helix_1")
            .ok_or("helix case")?;
        assert!(helix.observed_all_calls_per_second < 6.0);
        assert!(number(&helix.service_ns["arc"]["median"])? < 1_000_000.0);
        Ok(())
    }
    #[test]
    fn incomplete_contradictory_or_unsupported_reference_is_not_a_rate_limit() -> TestResult {
        let original: Value = serde_json::from_str(REFERENCE)?;
        for (pointer, value) in [
            ("/passed", json!(false)),
            ("/schema", json!("nextnc-stage0/simulation/1")),
            ("/binaries/bin~1milltask", Value::Null),
            ("/results/0/passed", json!(false)),
            ("/results/0/in_position", json!(false)),
            ("/results/0/shim_done", json!(false)),
            ("/results/0/final_task_state", json!(1)),
            ("/results/0/final_position/0", json!(99.0)),
            ("/results/0/endpoint_error_mm", json!(0.001)),
            ("/results/0/shaper/active", json!(true)),
            ("/results/0/source", json!("0".repeat(40))),
            ("/results/0/admission/input_sha256", json!("0".repeat(64))),
            ("/results/0/admission/trace_audit/result", json!("FAIL")),
            ("/results/0/admission/trace_audit/records", json!(1)),
            ("/results/0/admission/shape", json!("arc")),
            ("/results/0/admission/observations", json!(100_001)),
            ("/results/0/admission/motion_window/elapsed_ns", json!(0)),
            ("/results/0/admission/motion_window/all_commands", json!(1)),
            ("/results/0/admission/motion_window/counts/state", json!(1)),
            (
                "/results/0/admission/motion_window/observed_commands_per_second",
                json!(99999.0),
            ),
            ("/results/0/admission/service_ns/line/samples", json!(9999)),
            (
                "/results/0/admission/service_ns/line/median",
                json!(99999999999_u64),
            ),
            ("/results/0/duration_s", json!(-1.0)),
            ("/results/0/duration_s", json!(121.0)),
        ] {
            let mut changed = original.clone();
            *changed.pointer_mut(pointer).ok_or(pointer)? = value;
            assert!(Reference::parse(&changed.to_string()).is_err(), "{pointer}");
        }
        let mut changed = original.clone();
        changed["results"].as_array_mut().ok_or("rows")?.pop();
        assert!(Reference::parse(&changed.to_string()).is_err());
        let mut changed = original;
        changed["results"][1] = changed["results"][0].clone();
        assert!(Reference::parse(&changed.to_string()).is_err());
        assert!(Reference::parse(&" ".repeat(REFERENCE_LIMIT + 1)).is_err());
        assert!(Reference::parse(
            &REFERENCE.replace("\"passed\": true", "\"passed\": true, \"passed\": true")
        )
        .is_err());
        Ok(())
    }
    #[test]
    fn count_actual_polyline_expansion_and_state_work_without_modifying_the_plan() -> TestResult {
        let p = fixture("mill-mm-polyline-8")?;
        let before = p.commands().to_vec();
        let reference = Reference::parse(REFERENCE)?;
        let r = describe(&p, Some(&reference))?;
        assert_eq!(r.counts.source_geometry_paths, 1);
        assert_eq!(r.counts.motion_commands, 8);
        assert_eq!(r.counts.line_commands, 8);
        assert_eq!(
            r.counts.prepared_commands,
            r.counts.motion_commands + r.counts.state_commands + r.counts.reviewed_waypoints
        );
        assert_eq!(
            r.counts.state_by_kind.values().sum::<usize>(),
            r.counts.state_commands
        );
        assert!(r.counts.state_by_kind["spindle"] > 0 && r.counts.state_by_kind["coolant"] > 0);
        // Independent source fixture: eight collinear 0.00001 mm moves at 2 mm/s.
        assert!((r.known_feed_and_dwell_seconds.ok_or("duration")? - 0.00004).abs() < 1e-12);
        let peak = r.densest_nominal_window.as_ref().ok_or("density")?;
        assert!(peak.prepared_commands > peak.motion_commands && peak.state_commands > 0);
        assert!(peak.nominal_prepared_commands_per_second > 200_000.0);
        assert!(r
            .warnings
            .iter()
            .any(|w| w.code == "NOMINAL_DENSITY_ABOVE_REFERENCE"));
        let range = r.observed_reference_all_call_rate_range.ok_or("range")?;
        assert!(range[0] < 6.0 && range[1] > 999.0 && range[1] < 1001.0);
        assert!(
            r.native_capacity_commands_per_second.is_none()
                && r.predicted_native_mailbox_calls.is_none()
                && r.predicted_downstream_pieces.is_none()
        );
        assert!(!r.execution_authorized);
        assert_eq!(before, p.commands());
        let unmeasured = describe(&p, None)?;
        assert!(unmeasured.observed_reference_all_call_rate_range.is_none());
        assert!(!unmeasured
            .warnings
            .iter()
            .any(|w| w.code == "NOMINAL_DENSITY_ABOVE_REFERENCE"));
        Ok(())
    }
    #[test]
    fn analytic_length_includes_multiple_turns_rise_and_unit_conversion() -> TestResult {
        for (name, radius, rise, speed) in [
            ("mill-mm-arcs-8", 2.0_f64, 0.25_f64, 2.0),
            (
                "mill-inch-arcs-8",
                0.125 * 25.4,
                0.25 * 25.4,
                5.0 * 25.4 / 60.0,
            ),
        ] {
            let p = fixture(name)?;
            let r = describe(&p, None)?;
            assert_eq!(r.counts.motion_commands, 8);
            assert_eq!(r.counts.source_geometry_paths, 8);
            assert_eq!(
                (
                    r.counts.planar_circular_commands,
                    r.counts.helical_commands,
                    r.counts.multi_turn_commands
                ),
                (6, 2, 2)
            );
            // Six quarter circles and two 2.25-turn helices, independently of
            // the geometry helper's length result or producer serialization.
            let expected = (6.0 * radius * std::f64::consts::FRAC_PI_2
                + 2.0 * (radius * 4.5 * std::f64::consts::PI).hypot(rise))
                / speed;
            assert!((r.known_feed_and_dwell_seconds.ok_or("duration")? - expected).abs() < 1e-11);
        }
        Ok(())
    }
    #[test]
    fn synchronized_feed_has_counts_but_no_invented_rpm_duration() -> TestResult {
        for name in [
            "lathe-mm-arcs-8",
            "lathe-inch-arcs-8",
            "lathe-mm-polyline-8",
        ] {
            let r = describe(&fixture(name)?, Some(&Reference::parse(REFERENCE)?))?;
            assert_eq!(r.counts.per_revolution_motions, 8);
            assert!(r.densest_nominal_window.is_none());
            assert!(r
                .warnings
                .iter()
                .any(|w| w.code == "SYNCHRONIZED_DURATION_UNKNOWN"));
            assert!(!r
                .warnings
                .iter()
                .any(|w| w.code == "NOMINAL_DENSITY_ABOVE_REFERENCE"));
        }
        Ok(())
    }
    #[test]
    fn windows_include_state_and_dwell_but_never_hide_untimed_motion() -> TestResult {
        let items = VecDeque::from([
            Item {
                command: 3,
                seconds: 1.0,
                motion: true,
            },
            Item {
                command: 4,
                seconds: 0.0,
                motion: false,
            },
            Item {
                command: 5,
                seconds: 2.0,
                motion: false,
            },
            Item {
                command: 6,
                seconds: 3.0,
                motion: true,
            },
        ]);
        let c = candidate(&items).ok_or("window")?;
        assert_eq!(
            (c.prepared_commands, c.motion_commands, c.state_commands),
            (4, 2, 2)
        );
        assert_eq!(c.nominal_feed_and_dwell_seconds, 6.0);
        assert_eq!(c.nominal_prepared_commands_per_second, 4.0 / 6.0);
        assert!(candidate(&VecDeque::from([Item {
            command: 0,
            seconds: 0.0,
            motion: true
        }]))
        .is_none());
        assert!(candidate(&VecDeque::from([Item {
            command: 0,
            seconds: 1.0,
            motion: false
        }]))
        .is_none());
        for machine in ["mill-mm", "lathe-inch"] {
            let f: Value = serde_json::from_str(&std::fs::read_to_string(format!(
                "tests-rust/fixtures/legacy/{machine}.json"
            ))?)?;
            let p = compiled::prepare(
                f["text"].as_str().ok_or("source")?,
                &f["plan"].to_string(),
                &Limits::default(),
            )?;
            let r = describe(&p, None)?;
            if let Some(w) = r.densest_nominal_window {
                assert!(w.prepared_commands <= WINDOW);
                for record in &p.commands()[w.first_command..=w.last_command] {
                    assert!(!matches!(
                        record.action,
                        Action::Waypoint { .. }
                            | Action::Motion(crate::contract::v2::Motion {
                                feed: Feed::Rapid | Feed::PerRevolution { .. },
                                ..
                            })
                    ));
                }
            }
        }
        Ok(())
    }
}
