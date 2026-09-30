use nextnc_native::{
    part21::{parse, Limits, Value},
    shape,
    units::Units,
};
#[test]
fn legacy_dimensions_resolve_without_nominal_spindle_speed_conversions(
) -> Result<(), Box<dyn std::error::Error>> {
    for machine in ["mill", "lathe"] {
        for unit in ["mm", "inch"] {
            let text = std::fs::read_to_string(format!(
                "tests-rust/fixtures/legacy/{machine}-{unit}.stpnc"
            ))?;
            let doc = parse(&text, &Limits::default())?;
            shape::validate(&doc)?;
            let mut units = Units::new(&doc, 64);
            assert_eq!(units.geometry_context()?.1, unit);
            let mut found = std::collections::BTreeSet::new();
            for (r, _) in doc.all("DERIVED_UNIT") {
                found.insert(units.resolve(r.id)?);
            }
            assert!(found.contains(&format!("{unit}/minute")));
            assert!(found.contains(&format!("{unit}/revolution")));
            assert!(found.contains("revolution/minute"));
        }
    }
    Ok(())
}
#[test]
fn unit_cycle_and_wrong_conversion_are_explicit_errors() -> Result<(), Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string("tests-rust/fixtures/legacy/lathe-inch.stpnc")?;
    let mut doc = parse(&text, &Limits::default())?;
    let id = doc
        .all("LENGTH_MEASURE_WITH_UNIT")
        .next()
        .ok_or("missing conversion")?
        .0
        .id;
    let e = &mut doc.records.get_mut(&id).ok_or("record")?.parts[0];
    if let Value::Typed(t) = &mut e.args[0] {
        t.args[0] = Value::Number(25.0);
    }
    assert!(Units::new(&doc, 64).geometry_context().is_err());
    let mut doc = parse(&text, &Limits::default())?;
    let id = doc.all("DERIVED_UNIT").next().ok_or("derived")?.0.id;
    let element = doc.entity(id, "DERIVED_UNIT")?.args[0]
        .aggregate()
        .and_then(|a| a.first())
        .and_then(Value::reference)
        .ok_or("element")?;
    doc.records.get_mut(&element).ok_or("record")?.parts[0].args[0] = Value::Reference(id);
    assert!(Units::new(&doc, 64).resolve(id).is_err());
    Ok(())
}
