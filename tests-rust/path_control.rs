use nextnc_native::contract::{Feed, Termination};
use nextnc_native::{bundle, command_audit, compiled, error_budget, part21::Limits, plan, profile};
use serde_json::{json, Value};
type TestResult = Result<(), Box<dyn std::error::Error>>;

fn fixture(name: &str) -> Result<(String, Value), Box<dyn std::error::Error>> {
    let root = format!("tests-rust/fixtures/path-control/{name}-path-control");
    Ok((
        std::fs::read_to_string(format!("{root}.stpnc"))?,
        serde_json::from_str(&std::fs::read_to_string(format!("{root}.plan.json"))?)?,
    ))
}

#[test]
fn reviewed_operation_controls_survive_whole_job_roundtrip_without_spending_cam_tolerance(
) -> TestResult {
    for name in [
        "mill-mm",
        "lathe-mm",
        "mill-inch",
        "lathe-inch",
        "mill-mm-blend",
        "lathe-mm-blend",
        "mill-inch-blend",
        "lathe-inch-blend",
    ] {
        let (source, setup) = fixture(name)?;
        let setup_text = setup.to_string();
        let artifact = bundle::compile(
            bundle::Inputs {
                source: &source,
                setup: &setup_text,
                tool_table: None,
                target: None,
            },
            &Limits::default(),
        )?;
        let loaded = bundle::load(artifact.bytes().to_vec(), &Limits::default())?;
        assert_eq!(artifact.prepared().commands(), loaded.prepared().commands());
        assert!(loaded
            .prepared()
            .audit()
            .required_capabilities
            .iter()
            .any(|s| s == "blend"));
        let report = error_budget::describe(loaded.prepared())?;
        assert!(report.approximation_enabled && !report.execution_authorized);
        for (i, operation) in report.operations.iter().enumerate() {
            assert!((operation.cam_tolerance_mm.ok_or("CAM tolerance")? - 0.002).abs() < 1e-12);
            assert!((operation.blend_allocated_mm - [0.02, 0.2, 0., 0., 0.01][i]).abs() < 1e-12);
            assert_eq!(
                operation.blend_used_mm,
                if [0, 1, 4].contains(&i) {
                    None
                } else {
                    Some(0.)
                }
            );
            assert_eq!(operation.pre_shaper_total_bound_mm, None);
            assert_eq!(operation.source_motions, 3);
        }
        for span in loaded.prepared().spans() {
            if let compiled::Phase::Path { section, .. } = span.phase {
                let expected = loaded.prepared().setup().path_controls()[section].termination();
                for record in &loaded.prepared().commands()[span.commands.clone()] {
                    if let compiled::Action::Motion(m) = record.action {
                        assert_eq!(
                            m.termination,
                            if m.feed == Feed::Rapid {
                                Termination::ExactPath
                            } else {
                                expected
                            }
                        );
                    }
                }
            }
        }
        let mut controls = artifact.prepared().audit().required_capabilities.clone();
        controls.retain(|s| s != "blend");
        let target=json!({"schema":"nextnc-native/target-capabilities/1","machine":setup["machine"],"motionContractVersion":2,
            "capabilities":controls,"evidenceSha256":"0".repeat(64)}).to_string();
        assert!(nextnc_native::capabilities::check_prepared(
            Some(&target),
            artifact.prepared(),
            &Limits::default()
        )
        .is_err());
    }
    Ok(())
}

#[test]
fn old_plans_stay_exact_and_new_policy_is_included_in_artifact_identity() -> TestResult {
    let (source, mut setup) = fixture("mill-mm")?;
    let blended = bundle::compile(
        bundle::Inputs {
            source: &source,
            setup: &setup.to_string(),
            tool_table: None,
            target: None,
        },
        &Limits::default(),
    )?;
    setup["schema"] = json!("linuxcnc-next-nc/execution-plan/4");
    setup
        .as_object_mut()
        .ok_or("plan object")?
        .remove("pathControl");
    let exact = bundle::compile(
        bundle::Inputs {
            source: &source,
            setup: &setup.to_string(),
            tool_table: None,
            target: None,
        },
        &Limits::default(),
    )?;
    assert_ne!(blended.identity(), exact.identity());
    assert_ne!(blended.sha256(), exact.sha256());
    for record in exact.prepared().commands() {
        if let compiled::Action::Motion(m) = record.action {
            assert_eq!(m.termination, Termination::ExactPath);
        }
    }
    assert!(!error_budget::describe(exact.prepared())?.approximation_enabled);
    Ok(())
}

#[test]
fn missing_unknown_or_invalid_additional_allowances_fail_the_complete_plan() -> TestResult {
    let (source, setup) = fixture("lathe-inch")?;
    let program = profile::decode(&source, &Limits::default())?;
    for replacement in [
        Value::Null,
        json!([]),
        json!([{"mode":"blend","additionalDeviation":0.01}]),
    ] {
        let mut bad = setup.clone();
        bad["pathControl"] = replacement;
        assert!(plan::validate(&bad, &program).is_err());
    }
    for control in [
        json!({"mode":"blend"}),
        json!({"mode":"blend","additionalDeviation":0}),
        json!({"mode":"blend","additionalDeviation":-0.01}),
        json!({"mode":"blend","additionalDeviation":"0.01"}),
        json!({"mode":"blend","additionalDeviation":1e308}),
        json!({"mode":"blend","camTolerance":0.01}),
        json!({"mode":"exactPath","additionalDeviation":0.01}),
        json!({"mode":"exactStop","additionalDeviation":0}),
        json!({"mode":"inherit"}),
    ] {
        let mut bad = setup.clone();
        bad["pathControl"][4] = control;
        let error = plan::validate(&bad, &program)
            .err()
            .ok_or("invalid allowance accepted")?;
        assert_eq!(error.context["section"], json!(5));
    }
    let mut bad = setup.clone();
    bad["schema"] = json!("linuxcnc-next-nc/execution-plan/4");
    assert_eq!(
        plan::validate(&bad, &program)
            .err()
            .ok_or("old schema accepted new policy")?
            .code,
        "PATH_CONTROL"
    );
    Ok(())
}

#[test]
fn independent_audit_rejects_policy_substitution_or_increased_blend_allowance() -> TestResult {
    let (source, setup) = fixture("mill-mm")?;
    let prepared = compiled::prepare(&source, &setup.to_string(), &Limits::default())?;
    for term in [
        Termination::ExactPath,
        Termination::ExactStop,
        Termination::Blend {
            max_deviation_mm: 0.2,
        },
    ] {
        let mut commands = prepared.commands().to_vec();
        let record = commands
            .iter_mut()
            .find(|r| matches!(r.action, compiled::Action::Motion(_)))
            .ok_or("motion")?;
        if let compiled::Action::Motion(ref mut motion) = record.action {
            motion.termination = term;
        }
        assert!(command_audit::audit(
            prepared.program(),
            prepared.setup(),
            &commands,
            prepared.spans(),
            &Limits::default()
        )
        .is_err());
    }
    Ok(())
}
