//! Revision negotiation and explicit helix intent, with no controller access.
use motion_command::{v2::*, *};

fn p(x: f64, y: f64, z: f64) -> PointMm {
    PointMm { x, y, z }
}
fn caps() -> GeometryCapabilities {
    GeometryCapabilities {
        base: Capabilities::default().with(Capability::PlanarArc),
        helix: true,
        multiple_turns: true,
    }
}
fn helix() -> v2::Motion {
    v2::Motion {
        geometry: v2::Geometry::Circular {
            start: p(1.0, 0.0, 0.0),
            end: p(1.0, 0.0, -2.0),
            center: p(0.0, 0.0, 0.0),
            plane: Plane::Xy,
            rotation: Rotation::Clockwise,
            sweep_radians: 2.0 * core::f64::consts::TAU,
            axial_rise_mm: -2.0,
        },
        feed: Feed::PerSecond(2.0),
        termination: Termination::ExactPath,
        entry_gate: EntryGate::None,
        movement: Movement::RampHelix,
        tolerance: Tolerance::FusionOperationMm(0.002),
    }
}
#[test]
fn requires_revision_two_and_both_geometry_capabilities() {
    assert_eq!(
        helix().check_structure(1, Machine::MillXyz, caps()),
        Err(Error::Version)
    );
    assert_eq!(helix().check_structure(2, Machine::MillXyz, caps()), Ok(()));
    assert_eq!(
        helix().check_structure(
            2,
            Machine::MillXyz,
            GeometryCapabilities {
                helix: false,
                ..caps()
            }
        ),
        Err(Error::HelixUnsupported)
    );
    assert_eq!(
        helix().check_structure(
            2,
            Machine::MillXyz,
            GeometryCapabilities {
                multiple_turns: false,
                ..caps()
            }
        ),
        Err(Error::MultipleTurnsUnsupported)
    );
    let key = CommandKey {
        version: 2,
        binding: Binding {
            job_epoch: 1,
            state_epoch: 1,
        },
        sequence: 1,
    };
    assert_eq!(key.check_next(key.binding, 1), Err(ContractError::Version));
}
#[test]
fn explicit_rise_must_match_endpoint_and_center_stays_at_start_height() {
    let mut m = helix();
    if let v2::Geometry::Circular {
        ref mut axial_rise_mm,
        ..
    } = m.geometry
    {
        *axial_rise_mm = 2.0;
    }
    assert_eq!(
        m.check_structure(2, Machine::MillXyz, caps()),
        Err(Error::CircularStructure)
    );
    let mut m = helix();
    if let v2::Geometry::Circular { ref mut center, .. } = m.geometry {
        center.z = -2.0;
    }
    assert_eq!(
        m.check_structure(2, Machine::MillXyz, caps()),
        Err(Error::CircularStructure)
    );
}
#[test]
fn tolerance_and_movement_do_not_silently_change_feed() {
    let mut m = helix();
    m.tolerance = Tolerance::Missing;
    assert_eq!(m.check_structure(2, Machine::MillXyz, caps()), Ok(()));
    m.tolerance = Tolerance::FusionOperationMm(f64::NAN);
    assert_eq!(
        m.check_structure(2, Machine::MillXyz, caps()),
        Err(Error::Base(ContractError::NonFinite))
    );
    m.tolerance = Tolerance::Missing;
    m.movement = Movement::Rapid;
    assert_eq!(
        m.check_structure(2, Machine::MillXyz, caps()),
        Err(Error::MovementFeed)
    );
}
#[test]
fn lathe_cannot_hide_helical_y_or_wrong_plane() {
    assert_eq!(
        helix().check_structure(2, Machine::LatheXz, caps()),
        Err(Error::Base(ContractError::AxisMismatch))
    );
}
