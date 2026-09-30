use nextnc_native::{json, part21::Limits};
use sha2::{Digest, Sha256};
#[test]
fn strict_json_rejects_duplicates_at_every_depth_and_resource_limit() {
    for text in [r#"{"a":1,"a":2}"#, r#"{"x":{"a":1,"a":1}}"#, r#"[1e999]"#] {
        assert!(json::parse(text, &Limits::default()).is_err());
    }
    assert!(json::parse(
        "[[1]]",
        &Limits {
            nesting: 1,
            ..Limits::default()
        }
    )
    .is_err());
    assert!(json::parse(
        "[1,2]",
        &Limits {
            values: 2,
            ..Limits::default()
        }
    )
    .is_err());
}
#[test]
fn native_canonical_numbers_match_legacy_model_fingerprints(
) -> Result<(), Box<dyn std::error::Error>> {
    for machine in ["mill", "lathe"] {
        for units in ["mm", "inch"] {
            let text = std::fs::read_to_string(format!(
                "tests-rust/fixtures/legacy/{machine}-{units}.json"
            ))?;
            let data = json::parse(&text, &Limits::default())?;
            let model = &data["program"]["model"];
            let value = serde_json::json!({"schema":"next-nc/decoded-program/1","model":model});
            let hash = format!("{:x}", Sha256::digest(json::stringify(&value)?.as_bytes()));
            assert_eq!(
                Some(hash.as_str()),
                data["program"]["report"]["programFingerprint"]["value"].as_str()
            );
        }
    }
    Ok(())
}
#[test]
fn zero_and_exponent_thresholds_follow_ecmascript() -> Result<(), Box<dyn std::error::Error>> {
    let data = json::parse("[-0.0,1.0,1e-7,1e-6,1e20,1e21]", &Limits::default())?;
    assert_eq!(
        json::stringify(&data)?,
        "[0,1,1e-7,0.000001,100000000000000000000,1e+21]"
    );
    Ok(())
}

#[test]
fn streamed_fingerprint_matches_materialized_oracle_for_all_profile_models_and_edge_values(
) -> Result<(), Box<dyn std::error::Error>> {
    fn check(schema: &str, model: &serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
        let envelope = serde_json::json!({"schema":schema,"model":model});
        let expected = format!(
            "{:x}",
            Sha256::digest(json::stringify(&envelope)?.as_bytes())
        );
        assert_eq!(json::fingerprint(schema, model)?, expected);
        Ok(())
    }
    for dir in ["legacy", "native"] {
        for entry in std::fs::read_dir(format!("tests-rust/fixtures/{dir}"))? {
            let path = entry?.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json")
                || path.file_name().and_then(|s| s.to_str()) == Some("manifest.json")
            {
                continue;
            }
            let data = json::parse(&std::fs::read_to_string(path)?, &Limits::default())?;
            let (schema, model) = if dir == "legacy" {
                ("next-nc/decoded-program/1", &data["program"]["model"])
            } else {
                ("next-nc/decoded-program/2", &data["model"])
            };
            check(schema, model)?;
        }
    }
    let edge = json::parse(
        r#"{"z":[null,true,false,-0.0,1e-323,1e-7,1e-6,1e20,1e21,1e308],"0":"\u0000\r\n\t\\\"Ω𝄞","a":{"3":1,"1":2}}"#,
        &Limits::default(),
    )?;
    check("schema\"\\\nΩ", &edge)?;
    // More than one hashing buffer; insertion order and long escaped strings matter.
    check(
        "large",
        &serde_json::json!({"text":"\u{0}Ω\\\"".repeat(20000),"items":[edge.clone(),edge]}),
    )?;
    assert_ne!(
        json::fingerprint("a", &serde_json::json!({"x":1,"y":2}))?,
        json::fingerprint("a", &serde_json::json!({"y":2,"x":1}))?
    );
    Ok(())
}
