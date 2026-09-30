use nextnc_native::{
    part21::{parse, Limits, Value},
    profile_graph::Graph,
};
#[test]
fn every_ordered_use_and_shared_state_must_be_consumed() -> Result<(), Box<dyn std::error::Error>> {
    for machine in ["mill", "lathe"] {
        for units in ["mm", "inch"] {
            let text = std::fs::read_to_string(format!(
                "tests-rust/fixtures/legacy/{machine}-{units}.stpnc"
            ))?;
            let doc = parse(&text, &Limits::default())?;
            let mut g = Graph::new(&doc)?;
            assert!(g.finish().is_err());
            assert_eq!(g.milling, machine == "mill");
            assert_eq!(g.revision, 1);
            let steps = g.sequence("MACHINING_PROCESS_SEQUENCE_RELATIONSHIP", g.workplan)?;
            assert_eq!(steps.len(), 4);
            for (ws, _) in steps {
                g.visit(ws, "MACHINING_WORKINGSTEP")?;
                let op = g.target("MACHINING_OPERATION_RELATIONSHIP", ws)?;
                g.visit(
                    op,
                    if g.milling {
                        "MILLING_TYPE_OPERATION"
                    } else {
                        "TURNING_TYPE_OPERATION"
                    },
                )?;
                g.tool(op)?;
                let mut owners = vec![op];
                for (path, _) in g.sequence("MACHINING_TOOLPATH_SEQUENCE_RELATIONSHIP", op)? {
                    g.visit(path, "MACHINING_TOOLPATH")?;
                    owners.push(path);
                }
                for owner in owners {
                    let tech = g.target("MACHINING_TECHNOLOGY_RELATIONSHIP", owner)?;
                    g.use_definition(tech, "MACHINING_TECHNOLOGY")?;
                    let functions = g.target("MACHINING_FUNCTIONS_RELATIONSHIP", owner)?;
                    g.use_definition(functions, "MACHINING_FUNCTIONS")?;
                }
            }
            g.finish()?;
        }
    }
    Ok(())
}
#[test]
fn unknown_properties_and_bad_sequence_cannot_hide_in_graph(
) -> Result<(), Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string("tests-rust/fixtures/legacy/mill-mm.stpnc")?;
    let bad = text.replace("next-nc coordinates", "next-nc unimplemented instruction");
    assert!(Graph::new(&parse(&bad, &Limits::default())?).is_err());
    let mut doc = parse(&text, &Limits::default())?;
    let id = doc
        .all("MACHINING_PROCESS_SEQUENCE_RELATIONSHIP")
        .next()
        .ok_or("sequence")?
        .0
        .id;
    doc.records.get_mut(&id).ok_or("record")?.parts[0].args[4] = Value::Number(2.0);
    let mut graph = Graph::new(&doc)?;
    assert!(graph
        .sequence("MACHINING_PROCESS_SEQUENCE_RELATIONSHIP", graph.workplan)
        .is_err());
    Ok(())
}
