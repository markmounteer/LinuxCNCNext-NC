use nextnc_native::{
    bundle, capabilities, compiled,
    diagnostics::{self, Trace},
    error_budget,
    part21::{parse, Limits},
    plan, profile, publication, rate, shape, tool_table, Diagnostic, Result,
};
use serde_json::{json, Value};
use std::path::Path;

const OPTION_LIMIT: usize = 1024 * 1024;
fn run(args: &[String], trace: &mut Trace) -> Result<Value> {
    if args.len() == 2 && args[1] == "identity" {
        return Ok(json!({"schema":"nextnc/compiler-identity/v1",
            "compiler_sha256":bundle::COMPILER_SHA256,
            "schema_sha256":bundle::schema_sha256(),
            "policy_sha256":bundle::policy_sha256(), "executable":false}));
    }
    let limits = Limits::default();
    let parsed = trace.stage("arguments", |t| {
        // Protect named input paths before argument validation can fail. A
        // malformed option must not allow latest.json to overwrite its input.
        let command = args.get(1).map(String::as_str).unwrap_or("");
        if ["check-syntax", "inspect", "preflight", "prepare", "publish", "verify-bundle", "analyze-rate"].contains(&command) {
            if let Some(input) = args.get(2) {
                let artifact = matches!(command, "verify-bundle" | "analyze-rate");
                t.note_input(if artifact { "artifact" } else { "source" }, Path::new(input), if artifact { limits.bundle_bytes } else { limits.input_bytes })?;
            }
        }
        if command == "analyze-rate" {
            if let Some(input) = args.get(3) { t.note_input("admission-reference", Path::new(input), rate::REFERENCE_LIMIT)?; }
        }
        if ["preflight", "prepare", "publish"].contains(&command) {
            if let Some(input) = args.get(3) { t.note_input("setup", Path::new(input), limits.input_bytes)?; }
            for pair in args.get(4..).unwrap_or_default().chunks_exact(2) {
                let role = match pair[0].as_str() { "--tool-table" => "tool-table", "--target" => "target", _ => continue };
                t.note_input(role, Path::new(&pair[1]), OPTION_LIMIT)?;
            }
        }
        let valid = match args.get(1).map(String::as_str) {
            Some("check-syntax" | "inspect" | "verify-bundle") => args.len() == 3,
            Some("analyze-rate") => args.len() == 4,
            Some("preflight" | "prepare") => args.len() >= 4 && (args.len() - 4).is_multiple_of(2),
            Some("publish") => args.len() >= 6 && (args.len() - 4).is_multiple_of(2),
            _ => false,
        };
        if !valid {
            return Err(Diagnostic::new("cli","USAGE","Usage: nextnc-native {check-syntax|inspect} input.stpnc; nextnc-native {preflight|prepare|publish} input.stpnc plan.json [--tool-table tool.tbl] [--target capabilities.json] [--store directory (required for publish)]; nextnc-native verify-bundle artifact.nncb; nextnc-native analyze-rate artifact.nncb admission-simulation.json. Preparation only; no execution."));
        }
        let paths = option_paths(args.get(4..).unwrap_or_default(), args[1] == "publish")?;
        if args[1] == "publish" && paths.store.is_none() {
            return Err(Diagnostic::new("cli", "USAGE", "publish requires --store with an empty directory or existing artifact store"));
        }
        Ok(paths)
    })?;
    if matches!(args[1].as_str(), "verify-bundle" | "analyze-rate") {
        let bytes = trace.stage("artifact-read", |t| {
            t.read_bytes("artifact", Path::new(&args[2]), limits.bundle_bytes)
        })?;
        let artifact = trace.stage("bundle-validation", |_| bundle::load(bytes, &limits))?;
        if args[1] == "analyze-rate" {
            let text = trace.stage("reference-read", |t| {
                t.read_text(
                    "admission-reference",
                    Path::new(&args[3]),
                    rate::REFERENCE_LIMIT,
                )
            })?;
            let reference =
                trace.stage("reference-validation", |_| rate::Reference::parse(&text))?;
            let demand = trace.stage("command-demand", |_| {
                rate::describe(artifact.prepared(), Some(&reference))
            })?;
            return Ok(
                json!({"status":"native-command-demand-analyzed","executable":false,"artifactSHA256":artifact.sha256(),
                "identity":artifact.identity(),"commandDemand":demand,"liveBinding":"not_checked"}),
            );
        }
        let budget = trace.stage("error-budget", |_| {
            error_budget::describe(artifact.prepared())
        })?;
        return Ok(
            json!({"status":"native-bundle-verified","executable":false,"artifactSHA256":artifact.sha256(),"identity":artifact.identity(),"audit":artifact.prepared().audit(),"errorBudget":budget,"liveBinding":"not_checked"}),
        );
    }
    if args[1] == "publish" {
        let root = parsed
            .store
            .as_deref()
            .ok_or_else(|| Diagnostic::new("cli", "USAGE", "Missing store"))?;
        let mut store = trace.stage("open-store", |_| {
            publication::Store::open(Path::new(root), limits.clone())
        })?;
        let (ticket, snapshot) = trace.stage("input-snapshot", |_| {
            store.begin(publication::Paths {
                source: (&args[2]).into(),
                setup: (&args[3]).into(),
                tool_table: parsed.table.map(Into::into),
                target: parsed.target.map(Into::into),
            })
        })?;
        let inputs = snapshot.inputs();
        trace.observed("source", inputs.source.as_bytes());
        trace.observed("setup", inputs.setup.as_bytes());
        if let Some(text) = inputs.tool_table {
            trace.observed("tool-table", text.as_bytes());
        }
        if let Some(text) = inputs.target {
            trace.observed("target", text.as_bytes());
        }
        let artifact = match trace.stage("native-bundle", |_| bundle::compile(inputs, &limits)) {
            Ok(a) => a,
            Err(e) => {
                let _ = store.reject(&ticket, &e);
                return Err(e);
            }
        };
        let budget = match trace.stage("error-budget", |_| {
            error_budget::describe(artifact.prepared())
        }) {
            Ok(report) => report,
            Err(e) => {
                let _ = store.reject(&ticket, &e);
                return Err(e);
            }
        };
        let selected = trace.stage("publication", |_| store.commit(&ticket, &artifact))?;
        return Ok(
            json!({"status":"native-artifact-published","executable":false,
            "artifact":store.root().join("objects").join(format!("{}.nncb",artifact.sha256())),"artifactSHA256":artifact.sha256(),
            "identity":artifact.identity(),"audit":artifact.prepared().audit(),"errorBudget":budget,"selection":selected,"selectionRetainedAfterExit":false,
            "liveBinding":"not_checked"}),
        );
    }
    let text = trace.stage("source-read", |t| {
        t.read_text("source", Path::new(&args[2]), limits.input_bytes)
    })?;
    if args[1] == "prepare" {
        let setup = trace.stage("setup-read", |t| {
            t.read_text("setup", Path::new(&args[3]), limits.input_bytes)
        })?;
        let prepared = trace.stage("native-compile", |_| {
            compiled::prepare(&text, &setup, &limits)
        })?;
        let (table, target) = trace.stage("option-inputs", |t| options(&parsed, t))?;
        let table_report = trace.checked_report("tool-table", || {
            tool_table::check(table.as_deref(), prepared.program(), prepared.setup())
        })?;
        let target_report = trace.checked_report("target-capabilities", || {
            capabilities::check_prepared(target.as_deref(), &prepared, &limits)
        })?;
        let budget = trace.stage("error-budget", |_| error_budget::describe(&prepared))?;
        return Ok(
            json!({"status":"native-commands-audited","executable":false,
            "policy":compiled::POLICY,"programFingerprint":prepared.program().report.fingerprint,
            "audit":prepared.audit(),"toolTable":table_report,"targetCapabilities":target_report,"errorBudget":budget,
            "notChecked":["native bundle serialization, publication and selection lifecycle","runtime capabilities and live coordinate binding","physical clearance and machine readiness"]}),
        );
    }
    let doc = trace.stage("syntax", |_| parse(&text, &limits))?;
    if args[1] == "inspect" {
        let program = trace.stage("source-profile", |_| {
            profile::decode_document(&doc, &limits)
        })?;
        return Ok(json!({"status":"source-profile-checked","executable":false,"program":program}));
    }
    if args[1] == "preflight" {
        let program = trace.stage("source-profile", |_| {
            profile::decode_document(&doc, &limits)
        })?;
        let setup = trace.stage("setup-read", |t| {
            t.read_text("setup", Path::new(&args[3]), limits.input_bytes)
        })?;
        let plan = trace.stage("setup-plan", |_| plan::parse(&setup, &program, &limits))?;
        let (table, target) = trace.stage("option-inputs", |t| options(&parsed, t))?;
        let table_report = trace.checked_report("tool-table", || {
            tool_table::check(table.as_deref(), &program, &plan)
        })?;
        let target_report = trace.checked_report("target-capabilities", || {
            capabilities::check(target.as_deref(), &program, &limits)
        })?;
        return Ok(
            json!({"status":"source-and-setup-plan-checked","executable":false,"program":program,"plan":plan,"toolTable":table_report,"targetCapabilities":target_report,"notChecked":["native command completeness and policy audit","runtime capabilities and live coordinate binding","physical clearance and machine readiness"]}),
        );
    }
    let report = trace.stage("shape", |_| shape::validate(&doc))?;
    Ok(
        json!({"status":"syntax-and-shape-checked","executable":false,"inputSHA256":doc.input_sha256,"shape":report}),
    )
}
#[derive(Default)]
struct OptionPaths {
    table: Option<String>,
    target: Option<String>,
    store: Option<String>,
}
fn option_paths(args: &[String], allow_store: bool) -> Result<OptionPaths> {
    let mut result = OptionPaths::default();
    for pair in args.chunks_exact(2) {
        let slot = match pair[0].as_str() {
            "--tool-table" => &mut result.table,
            "--target" => &mut result.target,
            "--store" if allow_store => &mut result.store,
            _ => {
                return Err(
                    Diagnostic::new("cli", "USAGE", "Unknown preparation option")
                        .with("option", pair[0].clone()),
                )
            }
        };
        if slot.is_some() {
            return Err(
                Diagnostic::new("cli", "USAGE", "Duplicate preparation option")
                    .with("option", pair[0].clone()),
            );
        }
        *slot = Some(pair[1].clone());
    }
    Ok(result)
}
fn options(paths: &OptionPaths, trace: &mut Trace) -> Result<(Option<String>, Option<String>)> {
    Ok((
        paths
            .table
            .as_deref()
            .map(|p| trace.read_text("tool-table", Path::new(p), OPTION_LIMIT))
            .transpose()?,
        paths
            .target
            .as_deref()
            .map(|p| trace.read_text("target", Path::new(p), OPTION_LIMIT))
            .transpose()?,
    ))
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let mut trace = Trace::new(args.get(1).map(String::as_str).unwrap_or(""));
    match run(&args, &mut trace) {
        Ok(mut outcome) => {
            let report = diagnostics::archive(&trace, &outcome, None);
            outcome["diagnostics"] = report;
            println!("{outcome}");
        }
        Err(e) => {
            let mut outcome = json!({"status":"failed","error":e});
            let report = diagnostics::archive(&trace, &outcome, Some(&e));
            outcome["diagnostics"] = report;
            eprintln!("{outcome}");
            std::process::exit(1);
        }
    }
}
