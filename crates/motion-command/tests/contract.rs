//! Structural and identity contract checks; no controller is loaded or commanded.
use motion_command::*;

fn point(x: f64, y: f64, z: f64) -> PointMm {
    PointMm { x, y, z }
}

fn geometry_capabilities() -> Capabilities {
    Capabilities::default()
        .with(Capability::Linear)
        .with(Capability::PlanarArc)
        .with(Capability::Blend)
        .with(Capability::AtSpeed)
}

fn line() -> Motion {
    Motion {
        geometry: Geometry::Line {
            start: point(10.0, 0.0, 0.0),
            end: point(10.0, 0.0, -5.0),
        },
        feed: Feed::PerSecond(2.0),
        termination: Termination::ExactPath,
        entry_gate: EntryGate::None,
    }
}

fn check(motion: Motion, machine: Machine) -> Result<(), ContractError> {
    Command::Motion(motion).check_structure(machine, geometry_capabilities())
}

fn arc(plane: Plane, extent: ArcExtent) -> Motion {
    let (start, end) = match plane {
        Plane::Xy => (point(10.0, 0.0, 0.0), point(0.0, 10.0, 0.0)),
        Plane::Xz => (point(10.0, 0.0, 0.0), point(0.0, 0.0, 10.0)),
        Plane::Yz => (point(0.0, 10.0, 0.0), point(0.0, 0.0, 10.0)),
    };
    Motion {
        geometry: Geometry::Arc {
            start,
            end: if extent == ArcExtent::FullCircle {
                start
            } else {
                end
            },
            center: point(0.0, 0.0, 0.0),
            plane,
            rotation: Rotation::Counterclockwise,
            extent,
        },
        ..line()
    }
}

#[test]
fn both_machine_profiles_accept_cartesian_lines() {
    for machine in [Machine::MillXyz, Machine::LatheXz] {
        assert_eq!(check(line(), machine), Ok(()));
    }
}

#[test]
fn no_capability_is_assumed_from_file_or_machine_type() {
    assert_eq!(
        Command::Motion(line()).check_structure(Machine::MillXyz, Capabilities::default()),
        Err(ContractError::Unsupported(Capability::Linear))
    );
}

#[test]
fn lathe_y_is_rejected_rather_than_discarded() {
    let motion = Motion {
        geometry: Geometry::Line {
            start: point(10.0, 1.0, 0.0),
            end: point(10.0, 1.0, -5.0),
        },
        ..line()
    };
    assert_eq!(
        check(motion, Machine::LatheXz),
        Err(ContractError::AxisMismatch)
    );
    assert_eq!(check(motion, Machine::MillXyz), Ok(()));
}

#[test]
fn rejects_nonfinite_coordinates_at_either_endpoint() {
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for (start, end) in [
            (point(invalid, 0.0, 0.0), point(0.0, 0.0, 0.0)),
            (point(0.0, 0.0, 0.0), point(0.0, invalid, 0.0)),
        ] {
            assert_eq!(
                check(
                    Motion {
                        geometry: Geometry::Line { start, end },
                        ..line()
                    },
                    Machine::MillXyz
                ),
                Err(ContractError::NonFinite)
            );
        }
    }
}

#[test]
fn zero_length_source_segments_are_not_silently_optimized_away() {
    assert_eq!(
        check(
            Motion {
                geometry: Geometry::Line {
                    start: point(10.0, 0.0, 0.0),
                    end: point(10.0, 0.0, 0.0),
                },
                ..line()
            },
            Machine::LatheXz
        ),
        Ok(())
    );
    // Runtime must preserve semantic boundaries even if it elides geometric work.
}

#[test]
fn feed_values_must_be_finite_and_positive() {
    for invalid in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(check(
            Motion {
                feed: Feed::PerSecond(invalid),
                ..line()
            },
            Machine::MillXyz
        )
        .is_err());
    }
}

#[test]
fn feed_per_revolution_needs_its_own_capability() {
    let motion = Motion {
        feed: Feed::PerRevolution {
            mm_per_rev: 0.1,
            spindle: 0,
        },
        ..line()
    };
    assert_eq!(
        check(motion, Machine::LatheXz),
        Err(ContractError::Unsupported(Capability::FeedPerRevolution))
    );
    assert_eq!(
        Command::Motion(motion).check_structure(
            Machine::LatheXz,
            geometry_capabilities().with(Capability::FeedPerRevolution)
        ),
        Ok(())
    );
    let foreign = Motion {
        feed: Feed::PerRevolution {
            mm_per_rev: 0.1,
            spindle: 1,
        },
        ..motion
    };
    assert_eq!(
        check(foreign, Machine::LatheXz),
        Err(ContractError::SpindleIndex)
    );
}

#[test]
fn atspeed_is_distinct_from_spindle_synchronization() {
    let motion = Motion {
        entry_gate: EntryGate::SpindlesAtSpeed,
        ..line()
    };
    assert_eq!(
        Command::Motion(motion).check_structure(
            Machine::LatheXz,
            Capabilities::default().with(Capability::Linear)
        ),
        Err(ContractError::Unsupported(Capability::AtSpeed))
    );
    assert_eq!(check(motion, Machine::LatheXz), Ok(()));
}

#[test]
fn blend_budget_is_explicit_and_positive() {
    for invalid in [0.0, -0.1, f64::NAN, f64::INFINITY] {
        assert!(check(
            Motion {
                termination: Termination::Blend {
                    max_deviation_mm: invalid
                },
                ..line()
            },
            Machine::MillXyz
        )
        .is_err());
    }
    assert_eq!(
        check(
            Motion {
                termination: Termination::Blend {
                    max_deviation_mm: 0.01
                },
                ..line()
            },
            Machine::MillXyz
        ),
        Ok(())
    );
    assert_ne!(Termination::ExactPath, Termination::ExactStop);
}

#[test]
fn all_three_mill_planes_and_explicit_full_circles_are_representable() {
    for plane in [Plane::Xy, Plane::Xz, Plane::Yz] {
        for extent in [ArcExtent::Partial, ArcExtent::FullCircle] {
            assert_eq!(check(arc(plane, extent), Machine::MillXyz), Ok(()));
        }
    }
    assert_eq!(
        check(arc(Plane::Xz, ArcExtent::FullCircle), Machine::LatheXz),
        Ok(())
    );
    assert_eq!(
        check(arc(Plane::Xy, ArcExtent::Partial), Machine::LatheXz),
        Err(ContractError::AxisMismatch)
    );
}

#[test]
fn rapid_arcs_and_implicit_full_circles_are_rejected() {
    let full = arc(Plane::Xz, ArcExtent::FullCircle);
    assert_eq!(
        check(
            Motion {
                feed: Feed::Rapid,
                ..full
            },
            Machine::LatheXz
        ),
        Err(ContractError::RapidArc)
    );
    if let Geometry::Arc {
        start,
        end,
        center,
        plane,
        rotation,
        ..
    } = full.geometry
    {
        let ambiguous = Motion {
            geometry: Geometry::Arc {
                start,
                end,
                center,
                plane,
                rotation,
                extent: ArcExtent::Partial,
            },
            ..full
        };
        assert_eq!(
            check(ambiguous, Machine::LatheXz),
            Err(ContractError::ArcStructure)
        );
    } else {
        panic!("arc fixture must be an arc");
    }
}

#[test]
fn helical_and_zero_radius_arcs_are_outside_v1() {
    let basic = arc(Plane::Xy, ArcExtent::Partial);
    if let Geometry::Arc {
        start,
        end,
        center,
        plane,
        rotation,
        extent,
    } = basic.geometry
    {
        for (bad_end, bad_center) in [(point(end.x, end.y, 1.0), center), (end, start)] {
            assert_eq!(
                check(
                    Motion {
                        geometry: Geometry::Arc {
                            start,
                            end: bad_end,
                            center: bad_center,
                            plane,
                            rotation,
                            extent
                        },
                        ..basic
                    },
                    Machine::MillXyz
                ),
                Err(ContractError::ArcStructure)
            );
        }
    } else {
        panic!("arc fixture must be an arc");
    }
}

#[test]
fn css_is_lathe_specific_and_needs_a_separate_capability() {
    let css = Command::Spindle(Spindle::Css {
        surface_mm_per_second: 1000.0,
        maximum_rpm: 1500.0,
        clockwise: true,
    });
    let caps = Capabilities::default()
        .with(Capability::Spindle)
        .with(Capability::Completion);
    assert_eq!(
        css.check_structure(Machine::LatheXz, caps),
        Err(ContractError::Unsupported(Capability::Css))
    );
    assert_eq!(
        css.check_structure(Machine::LatheXz, caps.with(Capability::Css)),
        Ok(())
    );
    assert_eq!(
        css.check_structure(Machine::MillXyz, caps.with(Capability::Css)),
        Err(ContractError::AxisMismatch)
    );
}

#[test]
fn task_events_are_not_inferred_from_geometry_support() {
    let cases = [
        (Command::Coolant(Coolant::Flood), Capability::Coolant),
        (Command::ChangeTool { tool: 2 }, Capability::ToolChange),
        (Command::ToolOffset { offset: 7 }, Capability::ToolOffset),
        (Command::Dwell { seconds: 0.5 }, Capability::Dwell),
        (Command::Fence, Capability::Completion),
        (Command::End, Capability::Completion),
    ];
    for (command, capability) in cases {
        assert_eq!(
            command.check_structure(Machine::MillXyz, geometry_capabilities()),
            Err(ContractError::Unsupported(capability))
        );
        assert_eq!(
            command.check_structure(
                Machine::MillXyz,
                geometry_capabilities()
                    .with(capability)
                    .with(Capability::Completion)
            ),
            Ok(())
        );
    }
}

#[test]
fn dwell_does_not_accept_negative_or_nonfinite_time() {
    let caps = Capabilities::default()
        .with(Capability::Dwell)
        .with(Capability::Completion);
    for invalid in [-1.0, f64::NAN, f64::INFINITY] {
        assert!(Command::Dwell { seconds: invalid }
            .check_structure(Machine::MillXyz, caps)
            .is_err());
    }
    assert_eq!(
        Command::Dwell { seconds: 0.0 }.check_structure(Machine::MillXyz, caps),
        Ok(())
    );
}

fn key() -> CommandKey {
    CommandKey {
        version: CONTRACT_VERSION,
        binding: Binding {
            job_epoch: 4,
            state_epoch: 8,
        },
        sequence: 1,
    }
}

#[test]
fn stale_job_and_coordinate_state_are_rejected() {
    let key = key();
    for active in [
        Binding {
            job_epoch: 5,
            ..key.binding
        },
        Binding {
            state_epoch: 9,
            ..key.binding
        },
    ] {
        assert_eq!(key.check_next(active, 1), Err(ContractError::StaleBinding));
    }
    assert_eq!(key.check_next(key.binding, 1), Ok(()));
}

#[test]
fn incompatible_revision_gaps_and_wraparound_are_rejected() {
    let original = key();
    assert_eq!(
        CommandKey {
            version: CONTRACT_VERSION + 1,
            ..original
        }
        .check_next(original.binding, 1),
        Err(ContractError::Version)
    );
    assert_eq!(
        original.check_next(original.binding, 2),
        Err(ContractError::Sequence)
    );
    for sequence in [0, u64::MAX] {
        assert_eq!(
            CommandKey {
                sequence,
                ..original
            }
            .check_next(original.binding, sequence),
            Err(ContractError::InvalidIdentity)
        );
    }
    assert_eq!(
        CommandKey {
            binding: Binding {
                job_epoch: 0,
                state_epoch: 8
            },
            ..original
        }
        .check_next(original.binding, 1),
        Err(ContractError::InvalidIdentity)
    );
}

#[test]
fn structural_checks_do_not_claim_to_validate_arc_radii() {
    // Deliberately unequal radii: proving representation is not permission to run.
    let candidate = Motion {
        geometry: Geometry::Arc {
            start: point(10.0, 0.0, 0.0),
            end: point(0.0, 20.0, 0.0),
            center: point(0.0, 0.0, 0.0),
            plane: Plane::Xy,
            rotation: Rotation::Clockwise,
            extent: ArcExtent::Partial,
        },
        ..line()
    };
    assert_eq!(check(candidate, Machine::MillXyz), Ok(()));
    // The geometry oracle/admission adapter MUST reject this; neither exists here.
}

#[test]
fn machine_events_require_completion_in_addition_to_the_event_capability() {
    assert_eq!(
        Command::ChangeTool { tool: 2 }.check_structure(
            Machine::MillXyz,
            Capabilities::default().with(Capability::ToolChange)
        ),
        Err(ContractError::Unsupported(Capability::Completion))
    );
    assert_eq!(
        Command::Spindle(Spindle::Stop).check_structure(
            Machine::LatheXz,
            Capabilities::default().with(Capability::Spindle)
        ),
        Err(ContractError::Unsupported(Capability::Completion))
    );
}
