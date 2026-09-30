use nextnc_native::{
    bundle, capabilities, compiled,
    part21::{parse, Limits},
    plan, profile, publication, shape, tool_table, Diagnostic,
};
use std::{io::Read, path::Path};
fn read_bytes(path: &str, limit: usize) -> nextnc_native::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(Path::new(path))
        .and_then(|f| {
            f.take((limit as u64).saturating_add(1))
                .read_to_end(&mut bytes)
        })
        .map_err(|e| Diagnostic::new("read", "IO", e.to_string()).with("file", path))?;
    if bytes.len() > limit {
        return Err(Diagnostic::new(
            "read",
            "INPUT_SIZE",
            "Input exceeds byte limit",
        ));
    }
    Ok(bytes)
}
fn read(path: &str, limit: usize) -> nextnc_native::Result<String> {
    String::from_utf8(read_bytes(path, limit)?)
        .map_err(|e| Diagnostic::new("read", "UTF8", e.to_string()).with("file", path))
}
fn run() -> nextnc_native::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let valid = match args.get(1).map(String::as_str) {
        Some("check-syntax" | "inspect" | "verify-bundle") => args.len() == 3,
        Some("preflight" | "prepare") => args.len() >= 4 && (args.len() - 4) % 2 == 0,
        Some("publish") => args.len() >= 6 && (args.len() - 4) % 2 == 0,
        _ => false,
    };
    if !valid {
        return Err(Diagnostic::new("cli","USAGE","Usage: nextnc-native {check-syntax|inspect} input.stpnc; nextnc-native {preflight|prepare|publish} input.stpnc plan.json [--tool-table tool.tbl] [--target capabilities.json] [--store directory (required for publish)]; nextnc-native verify-bundle artifact.nncb. Preparation only; no execution."));
    }
    let limits = Limits::default();
    if args[1] == "verify-bundle" {
        let artifact = bundle::load(read_bytes(&args[2], limits.bundle_bytes)?, &limits)?;
        println!(
            "{}",
            serde_json::json!({"status":"native-bundle-verified","executable":false,"artifactSHA256":artifact.sha256(),"identity":artifact.identity(),"audit":artifact.prepared().audit(),"liveBinding":"not_checked"})
        );
        return Ok(());
    }
    if args[1] == "publish" {
        let parsed = option_paths(&args[4..], true)?;
        let root = parsed.store.ok_or_else(|| {
            Diagnostic::new(
                "cli",
                "USAGE",
                "publish requires --store with an empty directory or existing artifact store",
            )
        })?;
        let mut store = publication::Store::open(Path::new(&root), limits.clone())?;
        let (ticket, snapshot) = store.begin(publication::Paths {
            source: (&args[2]).into(),
            setup: (&args[3]).into(),
            tool_table: parsed.table.map(Into::into),
            target: parsed.target.map(Into::into),
        })?;
        let artifact = match bundle::compile(snapshot.inputs(), &limits) {
            Ok(a) => a,
            Err(e) => {
                let _ = store.reject(&ticket, &e);
                return Err(e);
            }
        };
        let selected = store.commit(&ticket, &artifact)?;
        println!(
            "{}",
            serde_json::json!({"status":"native-artifact-published","executable":false,
            "artifact":store.root().join("objects").join(format!("{}.nncb",artifact.sha256())),"artifactSHA256":artifact.sha256(),
            "identity":artifact.identity(),"audit":artifact.prepared().audit(),"selection":selected,"selectionRetainedAfterExit":false,
            "liveBinding":"not_checked"})
        );
        return Ok(());
    }
    let text = read(&args[2], limits.input_bytes)?;
    if args[1] == "prepare" {
        let prepared = compiled::prepare(&text, &read(&args[3], limits.input_bytes)?, &limits)?;
        let (table, target) = options(&args[4..])?;
        let table_report =
            tool_table::check(table.as_deref(), prepared.program(), prepared.setup())?;
        let target_report = capabilities::check_prepared(target.as_deref(), &prepared, &limits)?;
        println!(
            "{}",
            serde_json::json!({"status":"native-commands-audited","executable":false,
            "policy":compiled::POLICY,"programFingerprint":prepared.program().report.fingerprint,
            "audit":prepared.audit(),"toolTable":table_report,"targetCapabilities":target_report,
            "notChecked":["native bundle serialization, publication and selection lifecycle","runtime capabilities and live coordinate binding","physical clearance and machine readiness"]})
        );
        return Ok(());
    }
    let doc = parse(&text, &limits)?;
    if args[1] == "inspect" {
        let program = profile::decode_document(&doc, &limits)?;
        println!(
            "{}",
            serde_json::json!({"status":"source-profile-checked","executable":false,"program":program})
        );
        return Ok(());
    }
    if args[1] == "preflight" {
        let program = profile::decode_document(&doc, &limits)?;
        let plan = plan::parse(&read(&args[3], limits.input_bytes)?, &program, &limits)?;
        let (table, target) = options(&args[4..])?;
        let table_report = tool_table::check(table.as_deref(), &program, &plan)?;
        let target_report = capabilities::check(target.as_deref(), &program, &limits)?;
        println!(
            "{}",
            serde_json::json!({"status":"source-and-setup-plan-checked","executable":false,"program":program,"plan":plan,"toolTable":table_report,"targetCapabilities":target_report,"notChecked":["native command completeness and policy audit","runtime capabilities and live coordinate binding","physical clearance and machine readiness"]})
        );
        return Ok(());
    }
    let report = shape::validate(&doc)?;
    println!(
        "{}",
        serde_json::json!({"status":"syntax-and-shape-checked","executable":false,"inputSHA256":doc.input_sha256,"shape":report})
    );
    Ok(())
}
#[derive(Default)]
struct OptionPaths {
    table: Option<String>,
    target: Option<String>,
    store: Option<String>,
}
fn option_paths(args: &[String], allow_store: bool) -> nextnc_native::Result<OptionPaths> {
    let mut result = OptionPaths::default();
    for pair in args.chunks_exact(2) {
        let slot = match pair[0].as_str() {
            "--tool-table" => &mut result.table,
            "--target" => &mut result.target,
            "--store" if allow_store => &mut result.store,
            _ => {
                return Err(Diagnostic::new(
                    "cli",
                    "USAGE",
                    "Unknown preparation option",
                ))
            }
        };
        if slot.is_some() {
            return Err(Diagnostic::new(
                "cli",
                "USAGE",
                "Duplicate preparation option",
            ));
        }
        *slot = Some(pair[1].clone());
    }
    Ok(result)
}
fn options(args: &[String]) -> nextnc_native::Result<(Option<String>, Option<String>)> {
    let paths = option_paths(args, false)?;
    Ok((
        paths
            .table
            .as_deref()
            .map(|p| read(p, 1024 * 1024))
            .transpose()?,
        paths
            .target
            .as_deref()
            .map(|p| read(p, 1024 * 1024))
            .transpose()?,
    ))
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{}", serde_json::json!({"status":"failed","error":e}));
        std::process::exit(1);
    }
}
