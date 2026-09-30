use nextnc_native::{json, part21::Limits, profile};
use serde_json::Value;

#[test]
fn native_extensions_are_closed_and_capabilities_are_checked_for_the_whole_job(
) -> Result<(), Box<dyn std::error::Error>> {
    use nextnc_native::capabilities;
    let text = include_str!("fixtures/native/synthetic-14.stpnc");
    let program = profile::decode(text, &Limits::default())?;
    let mut manifest = serde_json::json!({"schema":"nextnc-native/target-capabilities/1","machine":"mill","motionContractVersion":2,"capabilities":program.report.required_capabilities,"evidenceSha256":"0".repeat(64)});
    assert_eq!(
        capabilities::check(Some(&manifest.to_string()), &program, &Limits::default())?["status"],
        "passed"
    );
    assert_eq!(
        capabilities::check(None, &program, &Limits::default())?["status"],
        "not_checked"
    );
    manifest["capabilities"] = serde_json::json!([]);
    assert_eq!(
        capabilities::check(Some(&manifest.to_string()), &program, &Limits::default())
            .err()
            .ok_or("missing support accepted")?
            .code,
        "UNSUPPORTED_CAPABILITIES"
    );
    for caps in [
        serde_json::json!(["helix"]),
        serde_json::json!(["linear", "linear"]),
        serde_json::json!(["phase-threading"]),
    ] {
        manifest["capabilities"] = caps;
        assert!(capabilities::Manifest::parse(&manifest.to_string(), &Limits::default()).is_err());
    }
    for (from, to) in [
        ("milling-toolpath/0.2", "milling-toolpath/0.1"),
        (
            "'next-nc movement','ramp-helix'",
            "'next-nc movement','probe'",
        ),
        ("\"axialRise\":-3", "\"axialRise\":3"),
        ("\"clockwise\":false", "\"clockwise\":0"),
        ("\"plane\":\"XY\"", "\"plane\":\"AB\""),
        (
            "\"provenance\":\"source-declared\"",
            "\"provenance\":\"guess\"",
        ),
        ("\"value\":0.002", "\"value\":0.002,\"value\":0.003"),
    ] {
        assert!(text.contains(from), "missing mutation {from}");
        assert!(
            profile::decode(&text.replace(from, to), &Limits::default()).is_err(),
            "accepted {to}"
        );
    }
    Ok(())
}

#[test]
fn captured_legacy_negative_corpus_still_fails_closed() -> Result<(), Box<dyn std::error::Error>> {
    let mut count = 0;
    for entry in std::fs::read_dir("tests-rust/fixtures/negative")? {
        let path = entry?.path();
        if path.extension().and_then(|x| x.to_str()) != Some("json") {
            continue;
        }
        let case = json::parse(&std::fs::read_to_string(&path)?, &Limits::default())?;
        let text = case["text"].as_str().ok_or("missing negative source")?;
        let error = match case["stage"].as_str() {
            Some("profile") => profile::decode(text, &Limits::default()).err(),
            Some("plan") => {
                let program = profile::decode(text, &Limits::default())?;
                nextnc_native::plan::validate(&case["plan"], &program).err()
            }
            Some("tool-table") => {
                let program = profile::decode(text, &Limits::default())?;
                let plan = nextnc_native::plan::validate(&case["plan"], &program)?;
                nextnc_native::tool_table::check(case["toolTable"].as_str(), &program, &plan).err()
            }
            _ => return Err("unrecognized negative oracle stage".into()),
        };
        assert!(
            error.is_some(),
            "Legacy failure was accepted: {}: {}",
            path.display(),
            case["expected"]
        );
        if case["stage"] != "profile" {
            assert_eq!(
                error.as_ref().map(|e| e.code.as_str()),
                case["expected"]["code"].as_str(),
                "{}",
                path.display()
            );
        }
        count += 1;
    }
    assert!(
        count >= 100,
        "Negative capture unexpectedly incomplete: {count}"
    );
    Ok(())
}

#[test]
fn all_native_models_match_the_independent_producer_reader(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = "tests-rust/fixtures/native";
    let manifest = json::parse(
        &std::fs::read_to_string(format!("{root}/manifest.json"))?,
        &Limits::default(),
    )?;
    let cases = manifest["cases"].as_array().ok_or("missing cases")?;
    assert_eq!(cases.len(), 130);
    for case in cases {
        let name = case["name"].as_str().ok_or("missing name")?;
        let text = std::fs::read_to_string(format!("{root}/{name}.stpnc"))?;
        let expected = json::parse(
            &std::fs::read_to_string(format!("{root}/{name}.json"))?,
            &Limits::default(),
        )?;
        let decoded =
            profile::decode(&text, &Limits::default()).map_err(|e| format!("{name}: {e}"))?;
        assert_eq!(
            json::stringify(&decoded.model)?,
            json::stringify(&expected["model"])?,
            "{name}"
        );
        assert_eq!(
            Some(decoded.report.fingerprint.value.as_str()),
            case["fingerprint"].as_str(),
            "{name}"
        );
        assert_eq!(
            decoded.provenance["inputSHA256"], case["inputSHA256"],
            "{name}"
        );
    }
    Ok(())
}

#[test]
fn complete_legacy_models_fingerprints_bounds_and_ordered_uses_match(
) -> Result<(), Box<dyn std::error::Error>> {
    for machine in ["mill", "lathe"] {
        for units in ["mm", "inch"] {
            let name = format!("tests-rust/fixtures/legacy/{machine}-{units}");
            let text = std::fs::read_to_string(format!("{name}.stpnc"))?;
            let baseline = json::parse(
                &std::fs::read_to_string(format!("{name}.json"))?,
                &Limits::default(),
            )?;
            let program = profile::decode(&text, &Limits::default())?;
            let expected = &baseline["program"];
            assert_eq!(
                json::stringify(&program.model)?,
                json::stringify(&expected["model"])?,
                "{name}"
            );
            assert_eq!(
                Some(program.report.fingerprint.value.as_str()),
                expected["report"]["programFingerprint"]["value"].as_str(),
                "{name}"
            );
            assert_eq!(
                json::stringify(&program.report.bounds)?,
                json::stringify(&expected["report"]["bounds"])?,
                "{name}"
            );
            assert_eq!(
                program.provenance["inputSHA256"],
                expected["provenance"]["inputSHA256"]
            );
            let sections = program.model["sections"]
                .as_array()
                .ok_or("missing sections")?;
            let sources = program.provenance["sections"]
                .as_array()
                .ok_or("missing source sections")?;
            assert_eq!(sections.len(), sources.len());
            for (section, source) in sections.iter().zip(sources) {
                let paths = section["paths"].as_array().ok_or("missing paths")?;
                let path_sources = source["paths"].as_array().ok_or("missing path sources")?;
                assert_eq!(paths.len(), path_sources.len());
                for (path, provenance) in paths.iter().zip(path_sources) {
                    if let Some(points) = path["points"].as_array() {
                        assert_eq!(
                            points.len(),
                            provenance["vertices"].as_array().map_or(0, Vec::len)
                        );
                    }
                    assert!(provenance["sequenceRelationship"]["record"].is_string());
                }
            }
        }
    }
    Ok(())
}

#[test]
fn expanded_geometry_budget_and_late_semantic_failures_are_enforced(
) -> Result<(), Box<dyn std::error::Error>> {
    let text = include_str!("fixtures/legacy/lathe-mm.stpnc");
    let error = profile::decode(
        text,
        &Limits {
            expanded_items: 2,
            ..Limits::default()
        },
    )
    .err()
    .ok_or("limit was ignored")?;
    assert_eq!(error.code, "EXPANSION_LIMIT");
    for (before, after) in [
        ("NUMERIC_MEASURE(60.)", "NUMERIC_MEASURE(-60.)"),
        (
            "CARTESIAN_POINT('',(7.,0.,0.))",
            "CARTESIAN_POINT('',(7.,1.,0.))",
        ),
        ("'next-nc work offset','2'", "'next-nc work offset','-2'"),
    ] {
        assert!(text.contains(before));
        let error = profile::decode(&text.replace(before, after), &Limits::default())
            .err()
            .ok_or("late invalid semantics accepted")?;
        assert_eq!(error.context.get("section"), Some(&Value::from(4)));
    }
    Ok(())
}
