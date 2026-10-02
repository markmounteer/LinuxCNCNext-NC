//! Development-only: create same-build bundles for the C ABI conformance driver.
use nextnc_native::{
    bundle::{self, Inputs},
    part21::Limits,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let output = args
        .next()
        .ok_or("usage: abi_fixtures NEW_OUTPUT_DIR [FIXTURE_DIR]")?;
    let root = args.next().map_or_else(
        || std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../nextnc-task/tests/fixtures"),
        std::path::PathBuf::from,
    );
    if args.next().is_some() {
        return Err("usage: abi_fixtures NEW_OUTPUT_DIR [FIXTURE_DIR]".into());
    }
    let output = std::path::Path::new(&output);
    std::fs::create_dir(output)?;
    let mut paths = std::fs::read_dir(&root)?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort();
    let mut count = 0;
    for path in paths
        .iter()
        .filter(|p| p.extension().is_some_and(|e| e == "stpnc"))
    {
        let name = path
            .file_stem()
            .and_then(|n| n.to_str())
            .ok_or("fixture name")?;
        let source = std::fs::read_to_string(path)?;
        let setup = std::fs::read_to_string(root.join(format!("{name}.plan.json")))?;
        let artifact = bundle::compile(
            Inputs {
                source: &source,
                setup: &setup,
                tool_table: None,
                target: None,
            },
            &Limits::default(),
        )?;
        std::fs::write(output.join(format!("{name}.nncb")), artifact.bytes())?;
        count += 1;
    }
    println!("{count} same-build ABI fixture bundles");
    Ok(())
}
