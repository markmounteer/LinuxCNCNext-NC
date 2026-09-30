use nextnc_native::{
    capabilities, compiled,
    part21::{parse, Limits},
    plan, profile, shape, tool_table, Diagnostic,
};
use std::{io::Read, path::Path};
fn read(path: &str, limit: usize) -> nextnc_native::Result<String> {
    let mut bytes = Vec::new();
    std::fs::File::open(Path::new(path))
        .and_then(|f| f.take(limit as u64 + 1).read_to_end(&mut bytes))
        .map_err(|e| Diagnostic::new("read", "IO", e.to_string()).with("file", path))?;
    if bytes.len() > limit {
        return Err(Diagnostic::new(
            "read",
            "INPUT_SIZE",
            "Input exceeds byte limit",
        ));
    }
    String::from_utf8(bytes)
        .map_err(|e| Diagnostic::new("read", "UTF8", e.to_string()).with("file", path))
}
fn run() -> nextnc_native::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let valid = match args.get(1).map(String::as_str) {
        Some("check-syntax" | "inspect") => args.len() == 3,
        Some("preflight" | "prepare") => args.len() >= 4 && (args.len() - 4) % 2 == 0,
        _ => false,
    };
    if !valid {
        return Err(Diagnostic::new("cli","USAGE","Usage: nextnc-native {check-syntax|inspect} input.stpnc; nextnc-native {preflight|prepare} input.stpnc plan.json [--tool-table tool.tbl] [--target capabilities.json]. Preparation only; no execution."));
    }
    let limits = Limits::default();
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
fn options(args: &[String]) -> nextnc_native::Result<(Option<String>, Option<String>)> {
    let mut table = None;
    let mut target = None;
    for pair in args.chunks_exact(2) {
        let slot = match pair[0].as_str() {
            "--tool-table" => &mut table,
            "--target" => &mut target,
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
        *slot = Some(read(&pair[1], 1024 * 1024)?);
    }
    Ok((table, target))
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{}", serde_json::json!({"status":"failed","error":e}));
        std::process::exit(1);
    }
}
