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
