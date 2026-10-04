use super::*;
use crate::{diagnostic::Location, part21::Limits};
type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;
struct Temp(PathBuf);
impl Temp {
    fn new() -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let p = std::env::temp_dir().join(format!(
            "nextnc-diagnostic-{}-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p)?;
        Ok(Self(fs::canonicalize(p)?))
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        if let (Ok(root), Ok(path)) = (
            fs::canonicalize(std::env::temp_dir()),
            fs::canonicalize(&self.0),
        ) {
            if path.parent() == Some(root.as_path())
                && path
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("nextnc-diagnostic-"))
            {
                let _ = fs::remove_dir_all(path);
            }
        }
    }
}
fn read_json(path: &Path) -> std::result::Result<Value, Box<dyn std::error::Error>> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}
fn located() -> Diagnostic {
    Diagnostic::new("parse", "BAD_SOURCE", "Malformed source").at(Location {
        byte_offset: 2,
        line: 2,
        byte_column: 1,
        record: Some(42),
    })
}

#[test]
fn directory_conventions_are_explicit_and_do_not_fall_back_to_working_directory() -> TestResult {
    let temp = Temp::new()?;
    for windows in [true, false] {
        let mut env = BTreeMap::new();
        assert_eq!(
            directory_from(env.clone(), windows)
                .err()
                .ok_or("missing failure")?
                .code,
            "DIRECTORY_UNAVAILABLE"
        );
        let home = temp.0.join("home");
        let state = temp.0.join("state");
        env.insert(
            OsString::from(if windows { "USERPROFILE" } else { "HOME" }),
            home.clone().into_os_string(),
        );
        let suffix = if windows {
            PathBuf::from("AppData").join("Local")
        } else {
            PathBuf::from(".local").join("state")
        };
        assert_eq!(
            directory_from(env.clone(), windows)?,
            home.join(suffix).join("LinuxCNCNext-NC/diagnostics")
        );
        let key = OsString::from(if windows {
            "LOCALAPPDATA"
        } else {
            "XDG_STATE_HOME"
        });
        env.insert(key.clone(), state.clone().into_os_string());
        assert_eq!(
            directory_from(env.clone(), windows)?,
            state.join("LinuxCNCNext-NC/diagnostics")
        );
        env.insert(key, "relative-state".into());
        assert_eq!(
            directory_from(env.clone(), windows)
                .err()
                .ok_or("relative failure")?
                .code,
            "DIRECTORY_RELATIVE"
        );
        env.insert("NEXTNC_DIAGNOSTICS".into(), temp.0.clone().into_os_string());
        assert_eq!(directory_from(env.clone(), windows)?, temp.0);
        env.insert("NEXTNC_DIAGNOSTICS".into(), "explicit-relative".into());
        assert_eq!(
            directory_from(env, windows)?,
            std::env::current_dir()?.join("explicit-relative")
        );
    }
    Ok(())
}

#[test]
fn excerpts_require_exact_observed_bytes_and_the_correct_input_role() -> TestResult {
    let temp = Temp::new()?;
    let source = temp.0.join("source.stpnc");
    let text = "first\nsecond Ω\nthird\n";
    fs::write(&source, text)?;
    let mut trace = Trace::new("inspect");
    trace.note_input("source", &source, 100)?;
    assert_eq!(trace.excerpt(&located())["status"], "unavailable");
    assert_eq!(trace.read_text("source", &source, 100)?, text);
    let excerpt = trace.excerpt(&located());
    assert_eq!(excerpt["sha256"], bundle::digest(text.as_bytes()));
    assert_eq!(excerpt["lines"][1]["text"], "second Ω");
    assert_eq!(
        trace.excerpt(&located().with("inputRole", "setup"))["status"],
        "unavailable"
    );
    fs::write(&source, "replaced\nsource\n")?;
    let changed = trace.excerpt(&located());
    assert_eq!(changed["status"], "unavailable");
    assert!(changed["reason"]
        .as_str()
        .is_some_and(|s| s.contains("changed")));
    assert_eq!(
        trace.inputs[0].sha256.as_deref(),
        Some(bundle::digest(text.as_bytes()).as_str())
    );

    let setup = temp.0.join("plan.json");
    fs::write(&setup, "{\n\"bad\":\n}")?;
    let value = trace.read_text("setup", &setup, 100)?;
    let error = crate::json::parse(&value, &Limits::default())
        .err()
        .ok_or("invalid JSON")?
        .with("inputRole", "setup");
    assert_eq!(error.context["jsonLine"], 3);
    assert_eq!(trace.excerpt(&error)["role"], "setup");
    assert_eq!(trace.excerpt(&error)["requestedLine"], 3);
    let table = temp.0.join("tool.tbl");
    fs::write(&table, "T1 P1\nBAD\n")?;
    trace.read_text("tool-table", &table, 100)?;
    let error =
        Diagnostic::new("tool-table", "TOOL_TABLE_SYNTAX", "Bad record").with("toolTableLine", 2);
    assert_eq!(trace.excerpt(&error)["lines"][1]["text"], "BAD");
    assert_eq!(
        Trace::new("verify-bundle").excerpt(&located())["status"],
        "unavailable"
    );
    Ok(())
}

#[test]
fn read_bounds_invalid_utf8_and_excerpt_bounds_preserve_honest_identity() -> TestResult {
    let temp = Temp::new()?;
    let source = temp.0.join("source");
    fs::write(&source, [0xff, 0xfe])?;
    let mut trace = Trace::new("inspect");
    let error = trace.read_text("source", &source, 2).err().ok_or("utf8")?;
    assert_eq!(error.code, "UTF8");
    assert!(trace.inputs[0].observed);
    assert_eq!(trace.inputs[0].sha256, Some(bundle::digest(&[0xff, 0xfe])));
    assert_eq!(trace.excerpt(&located())["status"], "unavailable");
    let mut limited = Trace::new("inspect");
    assert_eq!(
        limited
            .read_bytes("source", &source, 1)
            .err()
            .ok_or("bound")?
            .code,
        "INPUT_SIZE"
    );
    assert!(!limited.inputs[0].observed);
    assert_eq!(
        limited
            .read_bytes("directory", &temp.0, 100)
            .err()
            .ok_or("directory")?
            .stage,
        "read"
    );
    let text = format!("first\n{}\nlast", "Ω".repeat(EXCERPT_LIMIT));
    fs::write(&source, &text)?;
    trace.read_text("source", &source, text.len())?;
    // The same role is a single observed snapshot; its read limit must follow
    // the actual read, including when a caller deliberately rereads it.
    let excerpt = trace.excerpt(&located());
    assert_eq!(excerpt["status"], "verified");
    assert_eq!(excerpt["truncated"], true);
    let used: usize = excerpt["lines"]
        .as_array()
        .ok_or("lines")?
        .iter()
        .filter_map(|l| l["text"].as_str())
        .map(str::len)
        .sum();
    assert!(used <= EXCERPT_LIMIT);
    Ok(())
}

#[test]
fn stages_and_archive_summaries_do_not_claim_unreached_or_missing_checks_passed() -> TestResult {
    let mut trace = Trace::new("preflight");
    trace.stage("arguments", |_| Ok(()))?;
    trace.checked_report("tool-table", || Ok(json!({"status":"not_checked"})))?;
    let error = trace
        .stage::<()>("setup-plan", |_| {
            Err(Diagnostic::new("execution-plan", "PLAN", "Invalid mapping")
                .with("issues", json!([{"field":"tools"},{"field":"end"}])))
        })
        .err()
        .ok_or("failure")?;
    let record = trace.record(&json!({"program":{"report":{"sections":7},"model":{"large":"excluded"},"provenance":[1,2,3]}}), Some(&error), Duration::ZERO, "test");
    assert_eq!(record["executionAuthorized"], false);
    assert_eq!(record["result"]["program"], json!({"sections":7}));
    assert_eq!(
        record["error"]["context"]["issues"]
            .as_array()
            .ok_or("issues")?
            .len(),
        2
    );
    let state = |name: &str| {
        trace
            .stages
            .iter()
            .find(|s| s.name == name)
            .map(|s| s.status)
    };
    assert_eq!(state("arguments"), Some("passed"));
    assert_eq!(state("setup-plan"), Some("failed"));
    assert_eq!(state("tool-table"), Some("not_checked"));
    assert_eq!(state("target-capabilities"), Some("not_reached"));
    let mut bound = Bounded {
        bytes: vec![0; REPORT_LIMIT - 1],
    };
    bound.write_all(&[1])?;
    assert!(bound.write_all(&[2]).is_err());
    assert_eq!(bound.bytes.len(), REPORT_LIMIT);
    assert!(human(&json!({"error":{"message":"x".repeat(REPORT_LIMIT)}})).is_err());
    Ok(())
}

#[test]
fn utc_and_readable_control_escaping_cover_calendar_boundaries() {
    for (stamp, expected) in [
        (0, "1970-01-01T00:00:00.123Z"),
        (951_782_400, "2000-02-29T00:00:00.123Z"),
        (2_147_483_648, "2038-01-19T03:14:08.123Z"),
        (4_107_542_400, "2100-03-01T00:00:00.123Z"),
        (253_402_300_799, "9999-12-31T23:59:59.123Z"),
    ] {
        assert_eq!(
            utc(Duration::new(stamp, 123_000_000)).as_deref(),
            Some(expected)
        );
    }
    assert_eq!(utc(Duration::from_secs(253_402_300_800)), None);
    assert_eq!(
        visible("A\u{1b}[31m\u{202e}\nB"),
        "A\\u{001b}[31m\\u{202e}\nB"
    );
}

#[test]
fn success_preserves_latest_error_and_concurrent_writers_keep_complete_records() -> TestResult {
    let temp = Temp::new()?;
    let error = located();
    let first = save(
        &temp.0,
        &Trace::new("inspect"),
        &json!({"status":"failed"}),
        Some(&error),
    );
    assert_eq!(first["status"], "saved", "{first}");
    let error_bytes = fs::read(temp.0.join("latest-error.json"))?;
    let mut workers = Vec::new();
    for n in 0..8 {
        let path = temp.0.clone();
        workers.push(std::thread::spawn(move || {
            save(
                &path,
                &Trace::new("prepare"),
                &json!({"worker":n,"executable":false}),
                None,
            )
        }));
    }
    let mut ids = std::collections::BTreeSet::new();
    for worker in workers {
        let result = worker.join().map_err(|_| "diagnostic worker panic")?;
        assert_eq!(result["status"], "saved", "{result}");
        let json_path = Path::new(result["json"].as_str().ok_or("json path")?);
        let record = read_json(json_path)?;
        assert_eq!(record["reportId"], result["reportId"]);
        assert!(
            fs::read_to_string(result["text"].as_str().ok_or("text path")?)?
                .contains(result["reportId"].as_str().ok_or("id")?)
        );
        assert!(ids.insert(result["reportId"].as_str().ok_or("id")?.to_owned()));
    }
    assert_eq!(fs::read(temp.0.join("latest-error.json"))?, error_bytes);
    let latest = read_json(&temp.0.join("latest.json"))?;
    assert!(ids.contains(latest["reportId"].as_str().ok_or("latest id")?));
    assert!(fs::read_to_string(temp.0.join("latest.txt"))?
        .contains(latest["reportId"].as_str().ok_or("id")?));
    assert_eq!(
        fs::read_dir(&temp.0)?
            .filter_map(std::result::Result::ok)
            .filter(|e| e.path().extension().is_some_and(|e| e == "tmp"))
            .count(),
        0
    );
    Ok(())
}

#[test]
fn busy_index_input_collision_and_storage_failure_never_erase_original_evidence() -> TestResult {
    let temp = Temp::new()?;
    let trace = Trace::new("inspect");
    let error = located();
    let path = temp.0.join("native-latest.lock");
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(path)?;
    lock.try_lock()?;
    let busy = save(&temp.0, &trace, &json!({"status":"failed"}), Some(&error));
    assert_eq!(busy["status"], "saved", "{busy}");
    assert_eq!(busy["latestUpdated"], false);
    assert_eq!(
        read_json(Path::new(busy["json"].as_str().ok_or("path")?))?["error"]["code"],
        "BAD_SOURCE"
    );
    drop(lock);
    let input = temp.0.join("latest.json");
    fs::write(&input, "original machining input")?;
    let mut trace = Trace::new("inspect");
    trace.read_text("source", &input, 100)?;
    let collision = save(&temp.0, &trace, &json!({}), Some(&error));
    assert_eq!(collision["status"], "partial");
    assert_eq!(collision["error"]["code"], "INPUT_COLLISION");
    assert_eq!(fs::read_to_string(&input)?, "original machining input");
    // An existing directory at an index path simulates a publication I/O
    // failure after the immutable reports are already complete.
    fs::create_dir(temp.0.join("latest.txt"))?;
    let failed = save(&temp.0, &Trace::new("inspect"), &json!({}), Some(&error));
    assert_eq!(failed["status"], "partial");
    assert_eq!(failed["error"]["code"], "INDEX_TYPE");
    assert_eq!(
        read_json(Path::new(failed["json"].as_str().ok_or("json")?))?["error"]["code"],
        "BAD_SOURCE"
    );
    let blocked = temp.0.join("file-not-directory");
    fs::write(&blocked, "keep")?;
    let unavailable = save(&blocked, &Trace::new("inspect"), &json!({}), Some(&error));
    assert_eq!(unavailable["status"], "unavailable");
    assert_eq!(fs::read_to_string(blocked)?, "keep");
    Ok(())
}
