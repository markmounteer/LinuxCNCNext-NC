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
