use nextnc_native::part21::{parse, Limits};
use nextnc_native::shape;
fn file(data: &str) -> String {
    format!("ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('test'),'2;1');\nFILE_NAME('test','2026',(''),(''),'','','');\nFILE_SCHEMA(('INTEGRATED_CNC_SCHEMA'));\nENDSEC;\nDATA;\n{data}\nENDSEC;\nEND-ISO-10303-21;\n")
}
#[test]
fn reads_complex_entities_unicode_and_exact_input_identity(
) -> Result<(), Box<dyn std::error::Error>> {
    let text=file("#1=CARTESIAN_POINT('O''Brien \\X2\\D83DDE00\\X0\\',(1.,-2.E-3,0.));\n#2=(LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI.,.METRE.));");
    let d = parse(&text, &Limits::default())?;
    assert_eq!(d.records.len(), 2);
    assert_eq!(
        d.entity(1, "CARTESIAN_POINT")?.args[0].string(),
        Some("O'Brien 😀")
    );
    let crlf = text.replace('\n', "\r\n");
    let w = parse(&crlf, &Limits::default())?;
    assert_ne!(d.input_sha256, w.input_sha256);
    assert_eq!(d.records[&1].parts, w.records[&1].parts);
    let spaced = parse(&text.replace('\n', " \t "), &Limits::default())?;
    assert_eq!(d.records[&1].parts, spaced.records[&1].parts);
    assert_ne!(d.input_sha256, spaced.input_sha256);
    Ok(())
}
#[test]
fn rejects_dangling_duplicate_nonfinite_escape_and_incomplete_inputs() {
    for data in [
        "#1=POLYLINE('',(#2,#2));",
        "#1=X();#1=X();",
        "#1=X(1.E999);",
        "#1=X('\\X2\\D800\\X0\\');",
        "#1=(X() X());",
        "#0=X();",
    ] {
        assert!(parse(&file(data), &Limits::default()).is_err(), "{data}");
    }
    assert!(parse(
        &file("#1=X();").replace("END-ISO-10303-21;", ""),
        &Limits::default()
    )
    .is_err());
}
#[test]
fn resource_bounds_fail_with_diagnostics_before_unbounded_expansion() {
    let text = file("#1=X((1.,2.),#1,#1,'abc');#2=X();");
    for limits in [
        Limits {
            input_bytes: 2,
            ..Limits::default()
        },
        Limits {
            entities: 1,
            ..Limits::default()
        },
        Limits {
            references: 1,
            ..Limits::default()
        },
        Limits {
            values: 1,
            ..Limits::default()
        },
        Limits {
            nesting: 1,
            ..Limits::default()
        },
        Limits {
            aggregate_items: 1,
            ..Limits::default()
        },
        Limits {
            string_bytes: 2,
            ..Limits::default()
        },
    ] {
        assert!(parse(&text, &limits).is_err());
    }
}

#[test]
fn legacy_corpus_shape_counts_and_source_hashes_match_pinned_reader(
) -> Result<(), Box<dyn std::error::Error>> {
    for machine in ["mill", "lathe"] {
        for units in ["mm", "inch"] {
            let base = format!("tests-rust/fixtures/legacy/{machine}-{units}");
            let text = std::fs::read_to_string(format!("{base}.stpnc"))?;
            let expected: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(format!("{base}.json"))?)?;
            let doc = parse(&text, &Limits::default())?;
            let actual = shape::validate(&doc)?;
            assert_eq!(
                actual.records as u64,
                expected["shape"]["records"].as_u64().ok_or("records")?
            );
            assert_eq!(
                actual.components as u64,
                expected["shape"]["components"]
                    .as_u64()
                    .ok_or("components")?
            );
            assert_eq!(
                actual.attributes as u64,
                expected["shape"]["attributes"]
                    .as_u64()
                    .ok_or("attributes")?
            );
            assert_eq!(
                doc.input_sha256,
                expected["program"]["provenance"]["inputSHA256"]
                    .as_str()
                    .ok_or("hash")?
            );
        }
    }
    Ok(())
}

#[test]
fn every_legacy_entity_arity_and_attribute_rejects_an_invalid_value(
) -> Result<(), Box<dyn std::error::Error>> {
    use nextnc_native::part21::Value;
    use std::collections::BTreeSet;
    let mut entities = BTreeSet::new();
    let mut attributes = BTreeSet::new();
    for machine in ["mill", "lathe"] {
        for units in ["mm", "inch"] {
            let text = std::fs::read_to_string(format!(
                "tests-rust/fixtures/legacy/{machine}-{units}.stpnc"
            ))?;
            let mut doc = parse(&text, &Limits::default())?;
            shape::validate(&doc)?;
            let ids: Vec<_> = doc.records.keys().copied().collect();
            for id in ids {
                let parts = doc.records[&id].parts.clone();
                for (component, part) in parts.iter().enumerate() {
                    if entities.insert(part.name.clone()) {
                        doc.records.get_mut(&id).ok_or("record")?.parts[component]
                            .args
                            .push(Value::Symbol(".NOT_A_PROFILE_VALUE.".into()));
                        let error = shape::validate(&doc)
                            .err()
                            .ok_or("accepted extra attribute")?;
                        assert_eq!(error.code, "ARITY", "{}", part.name);
                        assert_eq!(error.source.as_ref().and_then(|s| s.record), Some(id));
                        doc.records.get_mut(&id).ok_or("record")?.parts[component].args =
                            part.args.clone();
                    }
                    for (index, value) in part.args.iter().enumerate() {
                        if !attributes.insert((part.name.clone(), index)) {
                            continue;
                        }
                        doc.records.get_mut(&id).ok_or("record")?.parts[component].args[index] =
                            Value::Symbol(".NOT_A_PROFILE_VALUE.".into());
                        let error = shape::validate(&doc)
                            .err()
                            .ok_or("accepted invalid attribute")?;
                        assert_eq!(
                            error.code,
                            "ATTRIBUTE",
                            "{} parameter {}",
                            part.name,
                            index + 1
                        );
                        assert_eq!(error.source.as_ref().and_then(|s| s.record), Some(id));
                        doc.records.get_mut(&id).ok_or("record")?.parts[component].args[index] =
                            value.clone();
                    }
                }
            }
            shape::validate(&doc)?;
        }
    }
    assert_eq!(
        entities.len(),
        61,
        "Complete reviewed legacy grammar arities"
    );
    assert_eq!(
        attributes.len(),
        187,
        "Complete reviewed legacy grammar attributes"
    );
    Ok(())
}
