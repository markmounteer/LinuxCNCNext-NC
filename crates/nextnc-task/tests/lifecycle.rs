use nextnc_native::{compiled, part21::Limits};
use nextnc_task::{
    lifecycle::*,
    steps::{Boundary, Layout},
};
type TestResult = Result<(), Box<dyn std::error::Error>>;
type Result<T, E = Box<dyn std::error::Error>> = std::result::Result<T, E>;

fn layout() -> Result<Layout> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests-rust/fixtures/benchmark/mill-mm-polyline-8");
    let source = std::fs::read_to_string(root.with_extension("stpnc"))?;
    let setup = std::fs::read_to_string(root.with_extension("plan.json"))?;
    Ok(Layout::from_prepared(&compiled::prepare(
        &source,
        &setup,
        &Limits::default(),
    )?))
}
fn ready() -> Readiness {
    Readiness {
        automatic: true,
        enabled: true,
        homed: true,
        fault_free: true,
        quiescent: true,
        source_binding_current: true,
        downstream_compatible: true,
    }
}
fn drain() -> Drain {
    Drain {
        task_empty: true,
        io_done: true,
        motion_done: true,
        in_position: true,
        shaper_done: true,
        fault_free: true,
        observed_after_admission: true,
    }
}
fn armed(capacity: usize) -> Result<(Owner, Binding)> {
    let mut owner = Owner::new(51, capacity)?;
    let generation = owner.begin_select()?;
    owner.selected(generation, [7; 32], layout()?)?;
    let binding = Binding {
        generation,
        state_epoch: 1,
        state_sha256: [3; 32],
        artifact_sha256: [7; 32],
    };
    owner.arm(binding, ready())?;
    Ok((owner, binding))
}

#[test]
fn failed_replacement_and_stale_workers_never_restore_old_authority() -> TestResult {
    let (mut owner, old) = armed(16)?;
    let new = owner.begin_select()?;
    assert!(!owner.allows_mdi());
    assert_eq!(
        owner.selected(old.generation, [7; 32], layout()?),
        Err(Error::Stale)
    );
    assert_eq!(owner.selection_failed(old.generation), Err(Error::Stale));
    assert_eq!(owner.phase(), Phase::Loading);
    owner.selection_failed(new)?;
    assert!(owner.allows_mdi());
    assert_eq!(
        owner.start(old, ready(), Start::Continuous, 0),
        Err(Error::Stale)
    );
    let mut restarted = Owner::new(52, 16)?;
    assert_eq!(
        restarted.start(old, ready(), Start::Continuous, 0),
        Err(Error::Stale)
    );
    assert_eq!(restarted.prefixes(), Prefixes::default());
    Ok(())
}

#[test]
fn every_start_route_needs_the_same_fresh_binding_and_readiness() -> TestResult {
    for field in 0..7 {
        let (mut owner, binding) = armed(16)?;
        let mut r = ready();
        match field {
            0 => r.automatic = false,
            1 => r.enabled = false,
            2 => r.homed = false,
            3 => r.fault_free = false,
            4 => r.quiescent = false,
            5 => r.source_binding_current = false,
            _ => r.downstream_compatible = false,
        }
        assert_eq!(
            owner.start(binding, r, Start::Continuous, 0),
            Err(Error::NotReady)
        );
        assert_eq!(owner.offer(16)?, None);
        assert_eq!(owner.prefixes(), Prefixes::default());
    }
    let (mut owner, binding) = armed(16)?;
    assert_eq!(
        owner.start(binding, ready(), Start::Step, 1),
        Err(Error::Restart)
    );
    assert_eq!(
        owner.start(
            Binding {
                state_epoch: 2,
                ..binding
            },
            ready(),
            Start::Continuous,
            0
        ),
        Err(Error::Stale)
    );
    owner.start(binding, ready(), Start::Continuous, 0)?;
    assert_eq!(
        owner.start(binding, ready(), Start::Continuous, 0),
        Err(Error::State)
    );
    assert_eq!(owner.begin_select(), Err(Error::Busy));
    assert!(!owner.allows_mdi());
    Ok(())
}

#[test]
fn partial_acceptance_retries_and_capacity_preserve_exactly_one_ordered_prefix() -> TestResult {
    for capacity in 1..=16 {
        let (mut owner, binding) = armed(capacity)?;
        owner.start(binding, ready(), Start::Continuous, 0)?;
        let total = layout()?.commands();
        let mut seen = Vec::new();
        for tick in 0..total * 8 {
            if owner.phase() != Phase::Running {
                break;
            }
            if let Some(offer) = owner.offer(9)? {
                let first = owner.prefixes().accepted;
                let end = (first + 1 + (tick % 3)).min(offer.commands.end);
                assert_eq!(first, offer.commands.start);
                owner.accept(&offer, end)?;
                owner.accept(&offer, end)?;
                seen.extend(first..end);
                assert!(owner.prefixes().accepted - owner.prefixes().admitted <= capacity);
                let fake = Offer {
                    serial: offer.serial + 100,
                    ..offer.clone()
                };
                assert_eq!(owner.accept(&fake, end), Err(Error::Sequence));
                owner.admitted(binding, end)?;
                if owner.phase() == Phase::Running && owner.offer(9)?.is_none() {
                    owner.drained(binding, drain())?;
                }
            } else {
                owner.drained(binding, drain())?;
            }
        }
        assert_eq!(seen, (0..total).collect::<Vec<_>>());
        assert_eq!(owner.phase(), Phase::Draining);
        assert!(owner.prefixes().completed < total);
        owner.drained(binding, drain())?;
        assert_eq!(owner.phase(), Phase::Reconciling);
        assert!(!owner.allows_mdi());
        owner.reconciled(drain(), true)?;
        assert_eq!(owner.phase(), Phase::Complete);
        assert!(owner.allows_mdi());
        assert_eq!(
            owner.start(binding, ready(), Start::Continuous, 0),
            Err(Error::Stale)
        );
    }
    Ok(())
}

#[test]
fn motion_done_alone_never_completes_a_step_or_releases_mdi() -> TestResult {
    let (mut owner, binding) = armed(16)?;
    owner.start(binding, ready(), Start::Step, 0)?;
    let offer = owner.offer(16)?.ok_or("missing initial step")?;
    owner.accept(&offer, offer.commands.end)?;
    owner.admitted(binding, offer.commands.end)?;
    assert_eq!(owner.phase(), Phase::StepDrain);
    for field in 0..7 {
        let mut d = drain();
        match field {
            0 => d.task_empty = false,
            1 => d.io_done = false,
            2 => d.motion_done = false,
            3 => d.in_position = false,
            4 => d.shaper_done = false,
            5 => d.fault_free = false,
            _ => d.observed_after_admission = false,
        }
        assert_eq!(owner.drained(binding, d), Err(Error::NotDrained));
        assert_eq!(owner.prefixes().completed, 0);
        assert!(owner.offer(16)?.is_none());
    }
    owner.drained(binding, drain())?;
    assert_eq!(owner.phase(), Phase::Held);
    assert!(!owner.allows_mdi());
    Ok(())
}

#[test]
fn stepping_observes_all_group_boundaries_including_states_tools_and_dwell() -> TestResult {
    let expected = layout()?;
    let (mut owner, binding) = armed(2)?;
    owner.start(binding, ready(), Start::Step, 0)?;
    for group in expected.groups() {
        if owner.phase() == Phase::Held {
            let proposed = owner.propose_step(binding)?;
            assert_eq!(proposed, group.commands.end);
            assert_eq!(
                owner.resume(binding, ready(), Some(proposed + 1)),
                Err(Error::Sequence)
            );
            owner.resume(binding, ready(), Some(proposed))?;
        }
        while owner.phase() == Phase::Running {
            let offer = owner
                .offer(100)?
                .ok_or("step did not offer remaining prefix")?;
            assert!(offer.commands.end <= group.commands.end);
            owner.accept(&offer, offer.commands.end)?;
            owner.admitted(binding, offer.commands.end)?;
        }
        assert_eq!(owner.prefixes().accepted, group.commands.end);
        owner.drained(binding, drain())?;
        assert_eq!(owner.prefixes().completed, group.commands.end);
    }
    assert_eq!(owner.phase(), Phase::Reconciling);
    assert_eq!(owner.reconciled(drain(), false), Err(Error::NotDrained));
    owner.reconciled(drain(), true)?;
    Ok(())
}

#[test]
fn hold_at_capacity_revokes_unaccepted_offers_and_acknowledges_effective_step_boundary(
) -> TestResult {
    let (mut owner, binding) = armed(2)?;
    owner.start(binding, ready(), Start::Continuous, 0)?;
    let offered = owner.offer(100)?.ok_or("missing offer")?;
    owner.accept(&offered, offered.commands.end)?;
    assert!(owner.offer(100)?.is_none());
    owner.hold()?;
    assert!(owner.offer(100)?.is_none());
    assert_eq!(owner.held(binding, false), Err(Error::NotReady));
    owner.held(binding, true)?;
    owner.admitted(binding, offered.commands.end)?;
    assert_eq!(owner.phase(), Phase::Held);
    let boundary = owner.propose_step(binding)?;
    assert!(boundary >= offered.commands.end);
    let mut disabled = ready();
    disabled.enabled = false;
    assert_eq!(
        owner.resume(binding, disabled, Some(boundary)),
        Err(Error::NotReady)
    );
    owner.resume(binding, ready(), Some(boundary))?;
    while owner.phase() == Phase::Running {
        let offer = owner.offer(100)?.ok_or("step suffix")?;
        assert!(offer.commands.end <= boundary);
        owner.accept(&offer, offer.commands.end)?;
        owner.admitted(binding, offer.commands.end)?;
    }
    owner.drained(binding, drain())?;
    assert_eq!(owner.phase(), Phase::Held);
    assert_eq!(owner.prefixes().completed, boundary);
    Ok(())
}

#[test]
fn tool_boundary_drains_before_and_after_and_rebinding_invalidates_old_receipts() -> TestResult {
    let l = layout()?;
    let tool = l
        .groups()
        .iter()
        .find(|g| matches!(g.boundary, Boundary::Tool { .. }))
        .ok_or("tool group")?;
    let (mut owner, binding) = armed(4096)?;
    owner.start(binding, ready(), Start::Continuous, 0)?;
    let before = owner.offer(4096)?.ok_or("initial range")?;
    assert_eq!(before.commands.end, tool.commands.start);
    owner.accept(&before, before.commands.end)?;
    owner.admitted(binding, before.commands.end)?;
    assert!(owner.offer(4096)?.is_none());
    owner.drained(binding, drain())?;
    let change = owner.offer(4096)?.ok_or("tool offer")?;
    assert_eq!(change.commands, tool.commands);
    owner.accept(&change, change.commands.end)?;
    owner.admitted(binding, change.commands.end)?;
    assert!(owner.offer(4096)?.is_none());
    let next = Binding {
        state_epoch: 2,
        state_sha256: [4; 32],
        ..binding
    };
    assert_eq!(owner.rebind(binding, next, drain()), Err(Error::NotDrained));
    owner.drained(binding, drain())?;
    owner.rebind(binding, next, drain())?;
    assert_eq!(
        owner.accept(&change, change.commands.end),
        Err(Error::Stale)
    );
    assert_eq!(
        owner.admitted(binding, change.commands.end),
        Err(Error::Stale)
    );
    assert!(owner.offer(4096)?.is_some());
    Ok(())
}

#[test]
fn abort_fault_and_disconnect_close_admission_before_cleanup_and_never_resume() -> TestResult {
    for action in 0..3 {
        let (mut owner, binding) = armed(1)?;
        owner.start(binding, ready(), Start::Continuous, 0)?;
        let offer = owner.offer(1)?.ok_or("offer")?;
        owner.accept(&offer, offer.commands.end)?;
        match action {
            0 => owner.abort(),
            1 => owner.fault(),
            _ => owner.disconnected(),
        }
        assert!(owner.offer(1)?.is_none());
        assert_eq!(owner.accept(&offer, offer.commands.end), Err(Error::Stale));
        assert_eq!(
            owner.admitted(binding, offer.commands.end),
            Err(Error::Stale)
        );
        assert_eq!(owner.begin_select(), Err(Error::Busy));
        assert!(!owner.allows_mdi());
        assert_eq!(owner.reconciled(drain(), false), Err(Error::NotDrained));
        owner.reconciled(drain(), true)?;
        assert!(owner.allows_mdi());
        assert_eq!(owner.prefixes(), Prefixes::default());
        assert_eq!(owner.resume(binding, ready(), None), Err(Error::Stale));
    }
    Ok(())
}
