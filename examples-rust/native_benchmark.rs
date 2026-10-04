//! Development-only timing worker. It uses the real compiler and bundle auditor.
//! Each process runs one phase; no execution or controller interface is present.
use nextnc_native::{
    bundle,
    compiled::{self, Action, PreparedPlan},
    contract::{v2::Geometry, Command},
    part21::Limits,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{error::Error, fs, io::Write, path::Path, time::Instant};

type Outcome<T> = Result<T, Box<dyn Error>>;
fn summary(plan: &PreparedPlan, expected: &Value) -> Outcome<Value> {
    let (mut lines, mut circular, mut helices, mut turns, mut dwells) = (0, 0, 0, 0, 0);
    let mut hash = Sha256::new();
    for r in plan.commands() {
        hash.update(format!("{r:?}\n"));
        match r.action {
            Action::Motion(m) => match m.geometry {
                Geometry::Line { .. } => lines += 1,
                Geometry::Circular {
                    sweep_radians,
                    axial_rise_mm,
                    ..
                } => {
                    circular += 1;
                    helices += usize::from(axial_rise_mm != 0.0);
                    turns += usize::from(sweep_radians > std::f64::consts::TAU);
                }
            },
            Action::Event(Command::Dwell { .. }) => dwells += 1,
            _ => (),
        }
    }
    for s in plan.spans() {
        hash.update(format!("{s:?}\n"));
    }
    let metrics = json!({"motions":lines+circular,"lines":lines,"circular":circular,"helices":helices,"multiTurns":turns,"dwells":dwells,
        "fingerprint":plan.program().report.fingerprint.value,"machine":plan.program().report.machine,"units":plan.program().report.units});
    if &metrics != expected || plan.audit().execution_authorized {
        return Err(
            format!("Fixture semantics changed: expected {expected}, observed {metrics}").into(),
        );
    }
    Ok(
        json!({"semantics":metrics,"commands":plan.commands().len(),"spans":plan.spans().len(),
        "orderedCommandSpanSHA256":format!("{:x}",hash.finalize()),"audit":plan.audit()}),
    )
}
fn read_checked(root: &Path, case: &Value, role: &str) -> Outcome<Vec<u8>> {
    let file = case[role].as_str().ok_or("missing fixture file")?;
    if Path::new(file).components().count() != 1 {
        return Err("Fixture file must be a basename".into());
    }
    let bytes = fs::read(root.join(file))?;
    if json!(bundle::digest(&bytes)) != case[format!("{role}SHA256")] {
        return Err("Fixture input hash mismatch".into());
    }
    Ok(bytes)
}
fn main() -> Outcome<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 5 {
        return Err(
            "usage: native_benchmark CASE_JSON {prepare|compile|load|create} SAMPLES BUNDLE_FILE"
                .into(),
        );
    }
    let case_path = Path::new(&args[1]);
    let root = case_path.parent().ok_or("fixture directory")?;
    let case: Value = serde_json::from_slice(&fs::read(case_path)?)?;
    let phase = &args[2];
    let count: usize = args[3].parse()?;
    if count == 0
        || count > 101
        || !["prepare", "compile", "load", "create"].contains(&phase.as_str())
    {
        return Err("invalid phase/sample count".into());
    }
    let limits = Limits::default();
    let source = String::from_utf8(read_checked(root, &case, "source")?)?;
    let setup = String::from_utf8(read_checked(root, &case, "setup")?)?;
    let inputs = bundle::Inputs {
        source: &source,
        setup: &setup,
        tool_table: None,
        target: None,
    };
    if phase == "create" {
        let artifact = bundle::compile(inputs, &limits)?;
        summary(artifact.prepared(), &case["expected"])?;
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&args[4])?;
        f.write_all(artifact.bytes())?;
        f.sync_all()?;
        println!(
            "{}",
            json!({"sha256":artifact.sha256(),"bytes":artifact.bytes().len()})
        );
        return Ok(());
    }
    let data = if phase == "load" {
        Some(fs::read(&args[4])?)
    } else {
        None
    };
    let mut samples = Vec::new();
    let mut reference = None;
    let mut artifact_hash = None;
    let mut artifact_bytes = None;
    for i in 0..count {
        // Supply an owned buffer before timing load. This copy is excluded from
        // core latency but remains included in process memory/end-to-end time.
        let owned = data.clone();
        let start = Instant::now();
        let (elapsed, result, sha, len) = if phase == "prepare" {
            let p = compiled::prepare(&source, &setup, &limits)?;
            let elapsed = start.elapsed().as_nanos();
            (
                elapsed,
                summary(std::hint::black_box(&p), &case["expected"])?,
                None,
                None,
            )
        } else {
            let a = if let Some(bytes) = owned {
                bundle::load(bytes, &limits)?
            } else {
                bundle::compile(inputs, &limits)?
            };
            let elapsed = start.elapsed().as_nanos();
            if a.identity() != &bundle::Identity::of(inputs) {
                return Err("Bundle input identity mismatch".into());
            }
            (
                elapsed,
                summary(std::hint::black_box(a.prepared()), &case["expected"])?,
                Some(a.sha256().to_string()),
                Some(a.bytes().len()),
            )
        };
        if i > 0
            && (reference.as_ref() != Some(&result)
                || artifact_hash != sha
                || artifact_bytes != len)
        {
            return Err("Repeated preparation changed command or bundle identity".into());
        }
        reference = Some(result);
        artifact_hash = sha;
        artifact_bytes = len;
        samples.push(
            json!({"index":i,"temperature":if i==0{"first_call"}else{"warm"},"coreWallNs":elapsed}),
        );
    }
    println!(
        "{}",
        json!({"schema":"nextnc-native/benchmark-worker/1","case":case["name"],"phase":phase,
        "compilerSHA256":bundle::COMPILER_SHA256,"schemaSHA256":bundle::schema_sha256(),"policySHA256":bundle::policy_sha256(),
        "inputIdentity":bundle::Identity::of(inputs),"sourceBytes":source.len(),"setupBytes":setup.len(),
        "artifactSHA256":artifact_hash,"artifactBytes":artifact_bytes,"result":reference,"samples":samples,"executable":false})
    );
    std::io::stdout().flush()?;
    // The parent reads OS peak RSS while this child is still alive, then releases it.
    let mut ack = String::new();
    std::io::stdin().read_line(&mut ack)?;
    if ack.trim() != "done" {
        return Err("Missing measurement acknowledgment".into());
    }
    Ok(())
}
