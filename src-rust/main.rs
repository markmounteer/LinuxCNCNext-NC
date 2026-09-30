use nextnc_native::{
    part21::{parse, Limits},
    shape, Diagnostic,
};
use std::{io::Read, path::Path};
fn run() -> nextnc_native::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 || args[1] != "check-syntax" {
        return Err(Diagnostic::new("cli","USAGE","Usage: nextnc-native check-syntax input.stpnc (syntax/shape only; not executable qualification)"));
    }
    let limits = Limits::default();
    let mut bytes = Vec::new();
    std::fs::File::open(Path::new(&args[2]))
        .and_then(|f| {
            f.take(limits.input_bytes as u64 + 1)
                .read_to_end(&mut bytes)
        })
        .map_err(|e| Diagnostic::new("read", "IO", e.to_string()))?;
    if bytes.len() > limits.input_bytes {
        return Err(Diagnostic::new(
            "read",
            "INPUT_SIZE",
            "Input exceeds byte limit",
        ));
    }
    let text =
        std::str::from_utf8(&bytes).map_err(|e| Diagnostic::new("read", "UTF8", e.to_string()))?;
    let doc = parse(text, &limits)?;
    let report = shape::validate(&doc)?;
    println!(
        "{}",
        serde_json::json!({"status":"syntax-and-shape-checked","executable":false,"inputSHA256":doc.input_sha256,"shape":report})
    );
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{}", serde_json::json!({"status":"failed","error":e}));
        std::process::exit(1);
    }
}
