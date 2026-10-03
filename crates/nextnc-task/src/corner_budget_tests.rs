use super::*;
use crate::lowering::{AxisDynamics, Piece};
use motion_command::{Command, PointMm};
use nextnc_native::compiled::Record;

fn machine() -> Dynamics {
    Dynamics {
        axis_mask: 7,
        interpolation_period_ns: 1_000_000,
        axes: [AxisDynamics {
            velocity_mm_s: 30.,
            acceleration_mm_s2: 100.,
            jerk_mm_s3: 1000.,
        }; 3],
    }
}

fn records() -> Vec<BoundRecord> {
    (0..6)
        .map(|i| {
            let point = |n: usize| [n as f64, 0.0005 * (n * n) as f64, 0.];
            let [x, y, z] = point(i);
            let start = PointMm { x, y, z };
            let [x, y, z] = point(i + 1);
            let end = PointMm { x, y, z };
            let source = v2::Motion {
                geometry: v2::Geometry::Line { start, end },
                feed: Feed::PerSecond(5.),
                termination: Termination::ExactPath,
                entry_gate: EntryGate::None,
                movement: v2::Movement::Cutting,
                tolerance: v2::Tolerance::FusionOperationMm(0.01),
            };
            BoundRecord {
                command: i,
                source: Record {
                    action: Action::Motion(source),
                    site: Site::Source,
                    ordinal: Some(i + 1),
                },
                action: BoundAction::Motion(Motion {
                    start_mm: [start.x, start.y, start.z, 0., 0., 0., 0., 0., 0.],
                    end_mm: [end.x, end.y, end.z, 0., 0., 0., 0., 0., 0.],
                    circular: None,
                    feed: source.feed,
                    termination: source.termination,
                    entry_gate: source.entry_gate,
                }),
                drain_before: false,
                css_update: None,
            }
        })
        .collect()
}

// Isolate post-lowering budget assignment. Public integration tests separately
// decode, bind and lower the unchanged original 512-move source fixtures.
fn plan(records: &[BoundRecord]) -> Result<Plan> {
    let mut p = Plan {
        pieces: Vec::new(),
        commands: Vec::new(),
        drains_before: Vec::new(),
        corner_budgets: Vec::new(),
    };
    for record in records {
        let payload = match record.action {
            BoundAction::Motion(m) if m.start_mm == m.end_mm && m.circular.is_none() => {
                Payload::Stationary(m)
            }
            BoundAction::Motion(m) => {
                let (dynamics, turn) = machine().resolve(m)?;
                Payload::Motion {
                    motion: m,
                    dynamics,
                    turn,
                }
            }
            state => Payload::State(state),
        };
        p.commands.push(p.pieces.len()..p.pieces.len() + 1);
        p.pieces.push(Piece {
            command: record.command,
            ordinal: 0,
            payload,
        });
        if record.drain_before {
            p.drains_before.push(record.command);
        }
    }
    Ok(p)
}

#[test]
fn semantic_boundaries_end_budgets_without_rewriting_any_source_piece() -> Result<()> {
    for variant in 0..10 {
        let mut r = records();
        let mut middle = match r[3].action {
            BoundAction::Motion(m) => m,
            _ => return Err(fail("test motion missing")),
        };
        match variant {
            0 => r[3].drain_before = true,
            1 => middle.feed = Feed::Rapid,
            2 => middle.feed = Feed::PerSecond(4.),
            3 => {
                middle.feed = Feed::PerRevolution {
                    mm_per_rev: 0.5,
                    spindle: 0,
                }
            }
            4 => middle.termination = Termination::ExactStop,
            5 => {
                middle.termination = Termination::Blend {
                    max_deviation_mm: 0.01,
                }
            }
            6 => middle.entry_gate = EntryGate::SpindlesAtSpeed,
            7 => middle.end_mm = middle.start_mm,
            8 => r[3].source.ordinal = Some(1),
            _ => r[3].source.ordinal = None,
        }
        r[3].action = BoundAction::Motion(middle);
        let mut p = plan(&r)?;
        let before = p.pieces.clone();
        let drains = p.drains_before.clone();
        apply(&r, machine(), &mut p)?;
        assert_eq!(p.corner_budgets[0].commands, 0..3, "variant {variant}");
        assert!(p
            .corner_budgets
            .iter()
            .all(|b| !(b.commands.start < 3 && b.commands.end > 3)));
        assert_eq!(p.pieces.len(), before.len());
        assert_eq!(p.drains_before, drains);
        for (a, b) in p.pieces.iter().zip(before) {
            assert_eq!((a.command, a.ordinal), (b.command, b.ordinal));
            match (a.payload, b.payload) {
                (Payload::Motion { motion: a, .. }, Payload::Motion { motion: b, .. }) => {
                    assert_eq!(a, b)
                }
                (a, b) => assert_eq!(a, b),
            }
        }
    }
    Ok(())
}

#[test]
fn events_and_source_intent_never_join_independent_corner_budgets() -> Result<()> {
    for variant in 0..6 {
        let mut r = records();
        let source = match &mut r[3].source.action {
            Action::Motion(m) => m,
            _ => return Err(fail("test source missing")),
        };
        match variant {
            0 => source.movement = v2::Movement::LeadOut,
            1 => source.tolerance = v2::Tolerance::Missing,
            2 => r[3].action = BoundAction::State(Action::Event(Command::Dwell { seconds: 0.1 })),
            3 => r[3].action = BoundAction::State(Action::Event(Command::ChangeTool { tool: 2 })),
            4 => {
                r[3].action = BoundAction::State(Action::Event(Command::Spindle(
                    motion_command::Spindle::Stop,
                )))
            }
            _ => {
                r[3].css_update = Some(crate::spindle::CssDemand {
                    surface_mm_s: 100.,
                    maximum_rpm: 2000.,
                    clockwise: true,
                    x_offset_mm: 0.,
                })
            }
        }
        let mut p = plan(&r)?;
        let original = p.pieces[3];
        apply(&r, machine(), &mut p)?;
        assert_eq!(p.corner_budgets[0].commands, 0..3, "variant {variant}");
        assert_eq!(p.pieces[3], original);
    }
    Ok(())
}

#[test]
fn budgets_are_per_source_polyline_and_do_not_slow_a_later_straight() -> Result<()> {
    let mut r = records();
    for (i, record) in r.iter_mut().enumerate().skip(3) {
        record.source.ordinal = Some(i - 2);
        if let BoundAction::Motion(ref mut m) = record.action {
            m.start_mm[1] = 0.;
            m.end_mm[1] = 0.;
        }
    }
    let mut p = plan(&r)?;
    let unchanged = p.pieces[3..].to_vec();
    apply(&r, machine(), &mut p)?;
    assert_eq!(p.corner_budgets.len(), 1);
    assert_eq!(p.corner_budgets[0].commands, 0..3);
    assert_eq!(&p.pieces[3..], unchanged);
    Ok(())
}

#[test]
fn a_noncoalescing_corner_does_not_cap_both_adjacent_straights() -> Result<()> {
    let mut r = records();
    let point = |n: usize| {
        if n <= 3 {
            [n as f64, 0., 0.]
        } else {
            [3., (n - 3) as f64, 0.]
        }
    };
    for (i, record) in r.iter_mut().enumerate() {
        if let BoundAction::Motion(ref mut motion) = record.action {
            motion.start_mm[..3].copy_from_slice(&point(i));
            motion.end_mm[..3].copy_from_slice(&point(i + 1));
        }
        if let Action::Motion(ref mut source) = record.source.action {
            let [x, y, z] = point(i);
            let start = PointMm { x, y, z };
            let [x, y, z] = point(i + 1);
            source.geometry = v2::Geometry::Line {
                start,
                end: PointMm { x, y, z },
            };
        }
    }
    let mut p = plan(&r)?;
    let original = p.pieces.clone();
    apply(&r, machine(), &mut p)?;
    assert!(
        p.corner_budgets.is_empty(),
        "the planner already retains this sharp junction"
    );
    assert_eq!(p.pieces, original);
    Ok(())
}
