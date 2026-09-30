use nextnc_native::{json, part21::Limits};
use std::{path::PathBuf, process::Command};
#[test]
fn standalone_publication_and_bundle_verification_need_no_node_or_live_state(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!(
        "nextnc-publish-cli-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    std::fs::create_dir(&root)?;
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let f = json::parse(
            &std::fs::read_to_string("tests-rust/fixtures/legacy/mill-inch.json")?,
            &Limits::default(),
        )?;
        let source = root.join("same-name.stpnc");
        let setup = root.join("plan.json");
        let table = root.join("tool.tbl");
        let store = root.join("store");
        std::fs::write(&source, f["text"].as_str().ok_or("source")?)?;
        std::fs::write(&setup, f["plan"].to_string())?;
        std::fs::write(&table, f["toolTable"].as_str().ok_or("table")?)?;
        let run = || {
            Command::new(env!("CARGO_BIN_EXE_nextnc-native"))
                .env_clear()
                .arg("publish")
                .arg(&source)
                .arg(&setup)
                .arg("--tool-table")
                .arg(&table)
                .arg("--store")
                .arg(&store)
                .output()
        };
        let first = run()?;
        assert!(
            first.status.success(),
            "{}",
            String::from_utf8_lossy(&first.stderr)
        );
        let first: serde_json::Value = serde_json::from_slice(&first.stdout)?;
        assert_eq!(first["executable"], false);
        assert_eq!(first["selectionRetainedAfterExit"], false);
        let artifact = PathBuf::from(first["artifact"].as_str().ok_or("artifact")?);
        let original = std::fs::read(&artifact)?;
        let second = run()?;
        assert!(
            second.status.success(),
            "{}",
            String::from_utf8_lossy(&second.stderr)
        );
        let second: serde_json::Value = serde_json::from_slice(&second.stdout)?;
        assert_eq!(first["artifactSHA256"], second["artifactSHA256"]);
        assert_ne!(
            first["selection"]["generation"],
            second["selection"]["generation"]
        );
        let verified = Command::new(env!("CARGO_BIN_EXE_nextnc-native"))
            .env_clear()
            .arg("verify-bundle")
            .arg(&artifact)
            .output()?;
        assert!(
            verified.status.success(),
            "{}",
            String::from_utf8_lossy(&verified.stderr)
        );
        let verified: serde_json::Value = serde_json::from_slice(&verified.stdout)?;
        assert_eq!(verified["artifactSHA256"], first["artifactSHA256"]);
        std::fs::write(&table, "T99 P99\n")?;
        let failure = run()?;
        assert!(!failure.status.success());
        assert!(failure.stdout.is_empty());
        let journal: serde_json::Value =
            serde_json::from_slice(&std::fs::read(store.join("selection.json"))?)?;
        assert_eq!(journal["status"], "failed");
        assert!(journal["artifactSHA256"].is_null());
        assert_eq!(std::fs::read(&artifact)?, original);
        assert_eq!(
            std::fs::read_to_string(source)?,
            f["text"].as_str().ok_or("source")?
        );
        Ok(())
    })();
    let canonical = std::fs::canonicalize(&root)?;
    let temp = std::fs::canonicalize(std::env::temp_dir())?;
    if canonical.parent() == Some(temp.as_path()) {
        std::fs::remove_dir_all(canonical)?;
    } else {
        return Err("unexpected test root".into());
    }
    result
}
#[test]
fn standalone_cli_has_no_node_path_and_publishes_only_complete_preflight_json(
) -> Result<(), Box<dyn std::error::Error>> {
    let root: PathBuf = std::env::temp_dir().join(format!(
        "nextnc-native-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    std::fs::create_dir(&root)?;
    let outcome = (|| -> Result<(), Box<dyn std::error::Error>> {
        let fixture = json::parse(
            &std::fs::read_to_string("tests-rust/fixtures/legacy/lathe-mm.json")?,
            &Limits::default(),
        )?;
        let input = root.join("input.stpnc");
        let plan = root.join("plan.json");
        let table = root.join("tool.tbl");
        std::fs::write(&input, fixture["text"].as_str().ok_or("missing source")?)?;
        std::fs::write(&plan, fixture["plan"].to_string())?;
        std::fs::write(
            &table,
            fixture["toolTable"].as_str().ok_or("missing table")?,
        )?;
        let before = [
            std::fs::read(&input)?,
            std::fs::read(&plan)?,
            std::fs::read(&table)?,
        ];
        let run = |mode: &str| {
            Command::new(env!("CARGO_BIN_EXE_nextnc-native"))
                .env_clear()
                .arg(mode)
                .arg(&input)
                .arg(&plan)
                .arg("--tool-table")
                .arg(&table)
                .output()
        };
        let success = run("preflight")?;
        assert!(
            success.status.success(),
            "{}",
            String::from_utf8_lossy(&success.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&success.stdout)?;
        assert_eq!(report["executable"], false);
        assert_eq!(report["toolTable"]["status"], "passed");
        assert_eq!(report["targetCapabilities"]["status"], "not_checked");
        assert_eq!(report["diagnostics"]["status"], "unavailable");
        assert_eq!(
            report["diagnostics"]["error"]["code"],
            "DIRECTORY_UNAVAILABLE"
        );
        let prepared = run("prepare")?;
        assert!(
            prepared.status.success(),
            "{}",
            String::from_utf8_lossy(&prepared.stderr)
        );
        let prepared: serde_json::Value = serde_json::from_slice(&prepared.stdout)?;
        assert_eq!(prepared["status"], "native-commands-audited");
        assert_eq!(prepared["executable"], false);
        assert_eq!(prepared["audit"]["status"], "passed");
        assert!(prepared["audit"]["commands"]
            .as_u64()
            .is_some_and(|n| n > 0));
        assert_eq!(
            before,
            [
                std::fs::read(&input)?,
                std::fs::read(&plan)?,
                std::fs::read(&table)?
            ]
        );
        std::fs::write(&table, "T9 P9\n")?;
        let failure = run("preflight")?;
        assert!(!failure.status.success());
        assert!(failure.stdout.is_empty());
        let error: serde_json::Value = serde_json::from_slice(&failure.stderr)?;
        assert_eq!(error["error"]["code"], "TOOL_TABLE_MISSING");
        let failure = run("prepare")?;
        assert!(!failure.status.success());
        assert!(failure.stdout.is_empty());
        assert_eq!(
            std::fs::read_dir(&root)?.count(),
            3,
            "CLI created an unrequested output or cache"
        );
        Ok(())
    })();
    std::fs::remove_dir_all(&root)?;
    outcome
}

#[test]
fn persistent_cli_reports_cover_all_commands_and_preserve_failure_when_archiving_fails(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!(
        "nextnc-diagnostic-cli-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    std::fs::create_dir(&root)?;
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let fixture: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
            "tests-rust/fixtures/legacy/mill-mm.json",
        )?)?;
        let source = root.join("source.stpnc");
        let setup = root.join("plan.json");
        let table = root.join("tool.tbl");
        let target = root.join("target.json");
        let store = root.join("store");
        let diagnostics = root.join("diagnostics");
        std::fs::write(&source, fixture["text"].as_str().ok_or("source")?)?;
        std::fs::write(&setup, fixture["plan"].to_string())?;
        std::fs::write(&table, fixture["toolTable"].as_str().ok_or("table")?)?;
        let run = |command: &str,
                   input: &std::path::Path,
                   args: &[&std::path::Path],
                   archive: &std::path::Path| {
            Command::new(env!("CARGO_BIN_EXE_nextnc-native"))
                .env_clear()
                .env("NEXTNC_DIAGNOSTICS", archive)
                .arg(command)
                .arg(input)
                .args(args)
                .output()
        };
        let flag_table = std::path::Path::new("--tool-table");
        let flag_store = std::path::Path::new("--store");
        let flag_target = std::path::Path::new("--target");
        let mut artifact = None;
        for command in ["check-syntax", "inspect", "preflight", "prepare", "publish"] {
            let mut args = Vec::new();
            if ["preflight", "prepare", "publish"].contains(&command) {
                args.extend([setup.as_path(), flag_table, table.as_path()]);
            }
            if command == "publish" {
                args.extend([flag_store, store.as_path()]);
            }
            let output = run(command, &source, &args, &diagnostics)?;
            assert!(
                output.status.success(),
                "{command}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stderr.is_empty());
            let outcome: serde_json::Value = serde_json::from_slice(&output.stdout)?;
            assert_eq!(outcome["diagnostics"]["status"], "saved", "{outcome}");
            let record: serde_json::Value = serde_json::from_slice(&std::fs::read(
                outcome["diagnostics"]["json"].as_str().ok_or("report")?,
            )?)?;
            assert_eq!(record["command"], command);
            assert_eq!(record["status"], "passed");
            assert_eq!(record["executionAuthorized"], false);
            let stages = record["validationStages"].as_array().ok_or("stages")?;
            assert!(stages
                .iter()
                .all(|s| s["status"] == "passed" || s["status"] == "not_checked"));
            assert!(record["inputs"][0]["sha256"]
                .as_str()
                .is_some_and(|s| s.len() == 64));
            assert!(record["result"]["program"]["model"].is_null());
            if command == "publish" {
                artifact = outcome["artifact"].as_str().map(PathBuf::from);
            }
        }
        let artifact = artifact.ok_or("artifact")?;
        let output = run("verify-bundle", &artifact, &[], &diagnostics)?;
        assert!(output.status.success());
        let record: serde_json::Value =
            serde_json::from_slice(&std::fs::read(diagnostics.join("latest.json"))?)?;
        assert_eq!(record["inputs"][0]["role"], "artifact");
        assert_eq!(record["validationStages"][2]["status"], "passed");

        // JSON syntax failure must point into the setup file, not the STEP file.
        std::fs::write(&setup, "{\n\"bad\":\n}")?;
        let output = run("prepare", &source, &[&setup], &diagnostics)?;
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let failed: serde_json::Value = serde_json::from_slice(&output.stderr)?;
        assert_eq!(failed["error"]["code"], "JSON");
        assert_eq!(failed["diagnostics"]["status"], "saved");
        let record: serde_json::Value =
            serde_json::from_slice(&std::fs::read(diagnostics.join("latest-error.json"))?)?;
        assert_eq!(record["sourceExcerpt"]["role"], "setup");
        assert_eq!(record["sourceExcerpt"]["requestedLine"], 3);
        assert_eq!(record["validationStages"][3]["status"], "failed");
        assert_eq!(record["validationStages"][4]["status"], "not_reached");
        assert!(
            std::fs::read_to_string(diagnostics.join("latest-error.txt"))?
                .contains("jsonByteColumn")
        );
        let error_record = std::fs::read(diagnostics.join("latest-error.json"))?;
        let output = run("inspect", &source, &[], &diagnostics)?;
        assert!(output.status.success());
        assert_eq!(
            std::fs::read(diagnostics.join("latest-error.json"))?,
            error_record
        );

        std::fs::write(&setup, fixture["plan"].to_string())?;
        std::fs::write(&table, "Tbad P1\n")?;
        let output = run(
            "preflight",
            &source,
            &[&setup, flag_table, &table],
            &diagnostics,
        )?;
        assert!(!output.status.success());
        let record: serde_json::Value =
            serde_json::from_slice(&std::fs::read(diagnostics.join("latest-error.json"))?)?;
        assert_eq!(record["sourceExcerpt"]["role"], "tool-table");
        assert_eq!(record["sourceExcerpt"]["requestedLine"], 1);
        std::fs::write(&target, "{\nBAD}")?;
        let output = run(
            "prepare",
            &source,
            &[&setup, flag_target, &target],
            &diagnostics,
        )?;
        assert!(!output.status.success());
        let record: serde_json::Value =
            serde_json::from_slice(&std::fs::read(diagnostics.join("latest-error.json"))?)?;
        assert_eq!(record["sourceExcerpt"]["role"], "target");

        // Archive errors never replace the job diagnostic or turn failure into success.
        let blocked = root.join("archive-is-a-file");
        std::fs::write(&blocked, "preserve")?;
        let output = run(
            "prepare",
            &source,
            &[&setup, flag_target, &target],
            &blocked,
        )?;
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let failed: serde_json::Value = serde_json::from_slice(&output.stderr)?;
        assert_eq!(failed["error"]["code"], "JSON");
        assert_eq!(failed["diagnostics"]["status"], "unavailable");
        assert_eq!(std::fs::read_to_string(&blocked)?, "preserve");
        let output = run("inspect", &source, &[], &blocked)?;
        assert!(output.status.success());
        let outcome: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(outcome["diagnostics"]["status"], "unavailable");
        assert_eq!(outcome["executable"], false);
        let collision = diagnostics.join("latest.json");
        std::fs::write(&collision, "preserve even on argument error")?;
        let output = run("inspect", &collision, &[&setup], &diagnostics)?;
        assert!(!output.status.success());
        let failed: serde_json::Value = serde_json::from_slice(&output.stderr)?;
        assert_eq!(failed["error"]["code"], "USAGE");
        assert_eq!(failed["diagnostics"]["error"]["code"], "INPUT_COLLISION");
        assert_eq!(
            std::fs::read_to_string(collision)?,
            "preserve even on argument error"
        );
        Ok(())
    })();
    let path = std::fs::canonicalize(&root)?;
    let temp = std::fs::canonicalize(std::env::temp_dir())?;
    if path.parent() == Some(temp.as_path()) {
        std::fs::remove_dir_all(path)?;
    } else {
        return Err("unexpected diagnostics test root".into());
    }
    result
}

#[cfg(unix)]
#[test]
fn native_input_rejects_fifo_without_waiting_for_a_writer() -> Result<(), Box<dyn std::error::Error>>
{
    use std::{
        process::Stdio,
        time::{Duration, Instant},
    };
    let root = std::env::temp_dir().join(format!(
        "nextnc-fifo-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    std::fs::create_dir(&root)?;
    let outcome = (|| -> Result<(), Box<dyn std::error::Error>> {
        let fifo = root.join("untrusted-input");
        let status = Command::new("mkfifo").arg(&fifo).status()?;
        assert!(status.success());
        let source = std::fs::canonicalize("tests-rust/fixtures/legacy/mill-mm.stpnc")?;
        for command in ["inspect", "verify-bundle", "prepare", "publish"] {
            let mut c = Command::new(env!("CARGO_BIN_EXE_nextnc-native"));
            c.env_clear()
                .arg(command)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            if ["prepare", "publish"].contains(&command) {
                c.arg(&source).arg(&fifo);
            } else {
                c.arg(&fifo);
            }
            if command == "publish" {
                c.arg("--store").arg(root.join("store"));
            }
            let mut child = c.spawn()?;
            let start = Instant::now();
            loop {
                if child.try_wait()?.is_some() {
                    break;
                }
                if start.elapsed() > Duration::from_secs(3) {
                    child.kill()?;
                    child.wait()?;
                    return Err(format!("{command} blocked on a FIFO with no writer").into());
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            let output = child.wait_with_output()?;
            assert!(!output.status.success());
            assert!(output.stdout.is_empty());
            let error: serde_json::Value = serde_json::from_slice(&output.stderr)?;
            assert!(
                matches!(
                    error["error"]["code"].as_str(),
                    Some("INPUT_TYPE" | "FILE_TYPE")
                ),
                "{error}"
            );
        }
        Ok(())
    })();
    let path = std::fs::canonicalize(&root)?;
    let temp = std::fs::canonicalize(std::env::temp_dir())?;
    if path.parent() == Some(temp.as_path()) {
        std::fs::remove_dir_all(path)?;
    } else {
        return Err("unexpected fifo test root".into());
    }
    outcome
}
