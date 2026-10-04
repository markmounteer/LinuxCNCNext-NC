use nextnc_native::{json, part21::Limits, plan, profile, tool_table};
use serde_json::{json, Value};
fn fixture(name: &str) -> Result<(Value, profile::Program), Box<dyn std::error::Error>> {
    let f = json::parse(
        &std::fs::read_to_string(format!("tests-rust/fixtures/legacy/{name}.json"))?,
        &Limits::default(),
    )?;
    let p = profile::decode(
        f["text"].as_str().ok_or("missing source")?,
        &Limits::default(),
    )?;
    Ok((f, p))
}
#[test]
fn legacy_plan_schemas_transitions_and_tool_table_snapshots_match(
) -> Result<(), Box<dyn std::error::Error>> {
    for machine in ["lathe", "mill"] {
        for units in ["mm", "inch"] {
            let (f, p) = fixture(&format!("{machine}-{units}"))?;
            let plan = plan::validate(&f["plan"], &p)?;
            assert_eq!(plan.transitions().len(), 4);
            assert!(matches!(plan.transitions()[1], plan::Transition::Continue));
            assert!(matches!(
                plan.transitions()[2],
                plan::Transition::Link { .. }
            ));
            let report = tool_table::check(f["toolTable"].as_str(), &p, &plan)?;
            for key in ["sha256", "records", "mappings"] {
                assert_eq!(
                    report[key], f["translation"]["report"]["toolTable"][key],
                    "{machine} {units} {key}"
                );
            }
        }
    }
    Ok(())
}
#[test]
fn all_plan_failures_are_collected_and_dependent_boundaries_marked_unchecked(
) -> Result<(), Box<dyn std::error::Error>> {
    let (f, p) = fixture("lathe-mm")?;
    let mut bad = f["plan"].clone();
    bad["tools"]["1:3"]["tool"] = json!(0);
    bad["workOffsets"]["1"] = json!("G60");
    bad["sections"][0]["retract"] = json!([]);
    bad["sections"][0]["approach"] = json!([{"z":2},{"x":5}]);
    bad["end"] = json!([{"x":0}]);
    let e = plan::validate(&bad, &p).err().ok_or("bad plan accepted")?;
    let issues = e.context["issues"].as_array().ok_or("no issues")?;
    assert_eq!(issues.len(), 9); // 3 tool + 3 WCS + retract/approach/end
    assert_eq!(e.context["notChecked"].as_array().map_or(0, Vec::len), 2);
    for code in [
        "TOOL_MAPPING",
        "WCS_MAPPING",
        "TRANSITION",
        "ENTRY_MISMATCH",
    ] {
        assert!(issues.iter().any(|i| i["code"] == code));
    }
    Ok(())
}
#[test]
fn mismatched_units_identity_machine_unknown_fields_and_unsafe_boundaries_fail(
) -> Result<(), Box<dyn std::error::Error>> {
    let (f, p) = fixture("lathe-mm")?;
    for (pointer, value, code) in [
        ("/programFingerprint", json!("other"), "PLAN_MISMATCH"),
        ("/units", json!("inch"), "PLAN_UNITS"),
        ("/schema", json!("unknown"), "PLAN_SCHEMA"),
        ("/sections/0/mode", json!("continue"), "PLAN"),
        ("/sections/2/moves/1/x", json!(7), "LINK_ENDPOINT"),
        ("/sections/0/retract/0/x", json!(1e9), "TRANSITION"),
    ] {
        let mut bad = f["plan"].clone();
        *bad.pointer_mut(pointer).ok_or("missing mutation")? = value;
        let e = plan::validate(&bad, &p)
            .err()
            .ok_or("invalid plan accepted")?;
        assert_eq!(e.code, code, "{pointer}");
    }
    let mut bad = f["plan"].clone();
    bad["machine"] = json!("lathe");
    assert_eq!(
        plan::validate(&bad, &p)
            .err()
            .ok_or("extra machine accepted")?
            .code,
        "PLAN_MACHINE"
    );
    bad = f["plan"].clone();
    bad["sections"][0] = json!({"mode":"continue"});
    assert_eq!(
        plan::validate(&bad, &p)
            .err()
            .ok_or("first continue accepted")?
            .code,
        "CONTINUATION"
    );
    bad = f["plan"].clone();
    bad["sections"][2] = json!({"mode":"continue"});
    assert_eq!(
        plan::validate(&bad, &p)
            .err()
            .ok_or("disconnected continue accepted")?
            .code,
        "CONTINUATION"
    );
    Ok(())
}
#[test]
fn tool_table_rejects_bad_unused_records_and_reports_all_missing_uses(
) -> Result<(), Box<dyn std::error::Error>> {
    let (f, p) = fixture("lathe-mm")?;
    let plan = plan::validate(&f["plan"], &p)?;
    assert_eq!(tool_table::check(None, &p, &plan)?["status"], "not_checked");
    for line in [
        "T4 P4 Q10",
        "T4 P4 Q1.0",
        "T4 P4 XNaN",
        "T4 P4 X1e999",
        "T4 P4 X1 X2",
        "T4.0 P4",
        "T4 P4e0",
        "T4 P-0",
        "T4 P4 J1_0",
        "T4",
        "T1 P4",
        "T4 P4 N2",
        "T4 P4 X.4.2",
    ] {
        let text = format!(
            "{}\n{line}",
            f["toolTable"].as_str().ok_or("missing table")?
        );
        assert!(tool_table::check(Some(&text), &p, &plan).is_err(), "{line}");
    }
    let e = tool_table::check(Some("T1 P1"), &p, &plan)
        .err()
        .ok_or("missing records accepted")?;
    assert_eq!(e.code, "TOOL_TABLE_MISSING");
    assert_eq!(e.context["operations"].as_array().map_or(0, Vec::len), 4);
    let table = "\u{feff}t1 p1 X+.5e-2 ; comments\nT2 P2 Q+9\nT3 P3 X1.\n";
    assert_eq!(
        tool_table::check(Some(table), &p, &plan)?["status"],
        "passed"
    );
    Ok(())
}
