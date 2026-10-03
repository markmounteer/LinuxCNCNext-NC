//! Resolve work/radius-mm intent against a fresh task snapshot, then validate
//! the complete machine path before returning any executable candidate.
use crate::spindle::{Capability as SpindleCapability, CssDemand};
use motion_command::{v2::Geometry, Command, Feed, Machine, Plane, PointMm, Rotation, Spindle};
use nextnc_native::{
    compiled::{Action, Frame, PreparedPlan, Record},
    geometry,
};
use std::collections::BTreeMap;

pub type Pose = [f64; 9];
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorkOffset {
    pub translation_mm: Pose,
    pub rotation_degrees: f64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxisLimits {
    pub minimum_mm: f64,
    pub maximum_mm: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shaping {
    Disabled,
    EngagedXy,
}

/// Values are already canonical mm, regardless of G20/G21 or G7/G8 display.
/// G7/G8 never changes a stored tool offset's physical meaning.
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub machine: Machine,
    pub commanded_pose_mm: Pose,
    pub active_work_offset: u8,
    pub work_offsets: [WorkOffset; 9],
    pub temporary_offset_mm: Pose,
    pub active_tool_offset_mm: Pose,
    pub tool_offsets_mm: BTreeMap<u32, Pose>,
    pub limits: [AxisLimits; 3],
    pub shaping: Shaping,
    pub reverse_spindle: bool,
    pub maximum_rpm: f64,
    pub flood: bool,
    pub mist: bool,
    pub spindle: Option<SpindleCapability>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Circular {
    pub center_mm: [f64; 3],
    pub plane: Plane,
    pub rotation: Rotation,
    pub sweep_radians: f64,
    pub axial_rise_mm: f64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    pub start_mm: Pose,
    pub end_mm: Pose,
    pub circular: Option<Circular>,
    pub feed: Feed,
    pub termination: motion_command::Termination,
    pub entry_gate: motion_command::EntryGate,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BoundAction {
    Motion(Motion),
    State(Action),
    WorkOffset { index: u8, value: WorkOffset },
    ToolOffset { number: u32, value_mm: Pose },
    Css(CssDemand),
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoundRecord {
    pub command: usize,
    pub source: Record,
    pub action: BoundAction,
    /// With shaping engaged, changing XY/bypass ownership or crossing an XY
    /// rapid boundary requires a real planner/shaper drain. Reviewed traverses
    /// have no cutting-tolerance allowance for rounding through a waypoint.
    pub drain_before: bool,
    /// Reissue the active CSS demand after a receipted offset change. Keeping
    /// this separate preserves both source identity and recipient accounting.
    pub css_update: Option<CssDemand>,
}
#[derive(Debug)]
pub struct BoundPlan {
    records: Vec<BoundRecord>,
    initial: Pose,
    final_pose: Pose,
}
impl BoundPlan {
    pub fn records(&self) -> &[BoundRecord] {
        &self.records
    }
    pub fn initial_pose_mm(&self) -> Pose {
        self.initial
    }
    pub fn final_pose_mm(&self) -> Pose {
        self.final_pose
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub command: Option<usize>,
    pub reason: &'static str,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "native live binding at {:?}: {}",
            self.command, self.reason
        )
    }
}
impl std::error::Error for Error {}
type Result<T> = std::result::Result<T, Error>;
fn error(reason: &'static str) -> Error {
    Error {
        command: None,
        reason,
    }
}
fn require(condition: bool, reason: &'static str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(error(reason))
    }
}
fn xyz(p: Pose) -> PointMm {
    PointMm {
        x: p[0],
        y: p[1],
        z: p[2],
    }
}
fn direction(rotation: Rotation) -> Rotation {
    match rotation {
        Rotation::Clockwise => Rotation::Counterclockwise,
        Rotation::Counterclockwise => Rotation::Clockwise,
    }
}
fn sin_cos(degrees: f64) -> (f64, f64) {
    // Exact quadrant rotations are exact coordinate permutations, not rounded
    // approximations of a slightly tilted plane unsupported by this shim.
    match degrees.rem_euclid(360.0) {
        0.0 => (0.0, 1.0),
        90.0 => (1.0, 0.0),
        180.0 => (0.0, -1.0),
        270.0 => (-1.0, 0.0),
        value => value.to_radians().sin_cos(),
    }
}
fn rotate(p: Pose, degrees: f64) -> Pose {
    let (s, c) = sin_cos(degrees);
    let mut out = p;
    out[0] = c * p[0] - s * p[1];
    out[1] = s * p[0] + c * p[1];
    out
}

#[derive(Clone, Copy)]
struct Transform {
    work: WorkOffset,
    temporary: Pose,
    tool: Pose,
}
impl Transform {
    fn to_machine(self, p: Pose) -> Pose {
        let rotated = rotate(
            std::array::from_fn(|i| p[i] + self.temporary[i]),
            self.work.rotation_degrees,
        );
        std::array::from_fn(|i| rotated[i] + self.work.translation_mm[i] + self.tool[i])
    }
    fn to_work(self, p: Pose) -> Pose {
        let translated = std::array::from_fn(|i| p[i] - self.work.translation_mm[i] - self.tool[i]);
        let rotated = rotate(translated, -self.work.rotation_degrees);
        std::array::from_fn(|i| rotated[i] - self.temporary[i])
    }
    fn source(self, point: PointMm) -> Pose {
        self.to_machine([point.x, point.y, point.z, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0])
    }
}

impl Snapshot {
    fn validate(&self, plan: &PreparedPlan) -> Result<()> {
        require(
            (self.machine == Machine::MillXyz && plan.program().report.machine == "mill")
                || (self.machine == Machine::LatheXz && plan.program().report.machine == "lathe"),
            "machine profile differs",
        )?;
        require(
            (1..=9).contains(&self.active_work_offset),
            "invalid active work offset",
        )?;
        require(
            self.maximum_rpm.is_finite() && self.maximum_rpm > 0.0,
            "invalid spindle policy",
        )?;
        if let Some(capability) = self.spindle {
            require(
                capability.valid(),
                "invalid spindle feedback capability evidence",
            )?;
            require(
                self.machine == Machine::LatheXz,
                "synchronized spindle capability requires an XZ lathe",
            )?;
            require(
                self.shaping == Shaping::Disabled,
                "synchronized spindle capability is incompatible with input shaping",
            )?;
        }
        for pose in std::iter::once(&self.commanded_pose_mm)
            .chain(std::iter::once(&self.temporary_offset_mm))
            .chain(std::iter::once(&self.active_tool_offset_mm))
            .chain(self.work_offsets.iter().map(|w| &w.translation_mm))
            .chain(self.tool_offsets_mm.values())
        {
            require(
                pose.iter().all(|x| x.is_finite()),
                "nonfinite live pose or offset",
            )?;
            require(
                pose[3..].iter().all(|x| *x == 0.0),
                "nonzero auxiliary pose/offset is unsupported; it cannot be zeroed",
            )?;
            if self.machine == Machine::LatheXz {
                require(pose[1] == 0.0, "lathe Y pose/offset is unsupported")?;
            }
        }
        for work in self.work_offsets {
            require(work.rotation_degrees.is_finite(), "nonfinite work rotation")?;
            if self.machine == Machine::LatheXz {
                require(
                    work.rotation_degrees.rem_euclid(360.0) == 0.0,
                    "rotated XZ lathe work frame is unsupported",
                )?;
            }
        }
        for limit in self.limits {
            require(
                limit.minimum_mm.is_finite()
                    && limit.maximum_mm.is_finite()
                    && limit.minimum_mm <= limit.maximum_mm,
                "invalid live travel limits",
            )?;
        }
        Ok(())
    }
    fn motion(&self, motion: Motion, numeric_floor: f64) -> Result<()> {
        require(
            motion
                .start_mm
                .into_iter()
                .chain(motion.end_mm)
                .all(f64::is_finite),
            "coordinate transform overflow",
        )?;
        if let Feed::PerRevolution {
            mm_per_rev,
            spindle,
        } = motion.feed
        {
            require(
                self.spindle.is_some(),
                "spindle synchronization requires qualified feedback capability",
            )?;
            require(
                spindle == 0 && mm_per_rev.is_finite() && mm_per_rev > 0.0,
                "invalid spindle-zero feed per revolution",
            )?;
        }
        let geometry = if let Some(c) = motion.circular {
            Geometry::Circular {
                start: xyz(motion.start_mm),
                end: xyz(motion.end_mm),
                center: PointMm {
                    x: c.center_mm[0],
                    y: c.center_mm[1],
                    z: c.center_mm[2],
                },
                plane: c.plane,
                rotation: c.rotation,
                sweep_radians: c.sweep_radians,
                axial_rise_mm: c.axial_rise_mm,
            }
        } else {
            Geometry::Line {
                start: xyz(motion.start_mm),
                end: xyz(motion.end_mm),
            }
        };
        let metrics = geometry::validate_with_floor(geometry, numeric_floor)
            .map_err(|_| error("bound geometry is inconsistent"))?;
        for axis in 0..3 {
            require(
                metrics.minimum_mm[axis] >= self.limits[axis].minimum_mm
                    && metrics.maximum_mm[axis] <= self.limits[axis].maximum_mm,
                "continuous machine path exceeds live travel limits",
            )?;
        }
        if self.shaping == Shaping::EngagedXy {
            if let Some(c) = motion.circular {
                require(
                    c.plane == Plane::Xy && c.axial_rise_mm == 0.0,
                    "active XY shaper cannot execute this circle/helix",
                )?;
            } else {
                let xy = motion.start_mm[0] != motion.end_mm[0]
                    || motion.start_mm[1] != motion.end_mm[1];
                let z = motion.start_mm[2] != motion.end_mm[2];
                require(
                    !(xy && z),
                    "active XY shaper cannot execute mixed XY/Z motion",
                )?;
            }
        }
        Ok(())
    }
}

pub fn bind(plan: &PreparedPlan, snapshot: &Snapshot) -> Result<BoundPlan> {
    bind_from(plan, snapshot, &[])
}

/// Rebind only unexecuted commands from a freshly observed procedure result.
/// Executed records keep their original identity and geometry. This function
/// validates geometry, not permission to resume: the owner must prove the drain
/// and that `completed` is its actual procedure boundary before adopting it.
pub fn rebind(
    plan: &PreparedPlan,
    previous: &BoundPlan,
    snapshot: &Snapshot,
    completed: usize,
) -> Result<BoundPlan> {
    require(
        completed > 0
            && completed < plan.commands().len()
            && previous.records.len() == plan.commands().len()
            && previous
                .records
                .iter()
                .zip(plan.commands())
                .enumerate()
                .all(|(i, (r, source))| r.command == i && r.source == *source),
        "invalid rebind source or prefix",
    )?;
    require(
        matches!(
            plan.commands()[completed - 1].action,
            Action::Event(Command::ChangeTool { .. })
        ),
        "rebind requires a completed tool procedure",
    )?;
    let mut result = bind_from(plan, snapshot, &previous.records[..completed])?;
    result.initial = previous.initial;
    Ok(result)
}

fn bind_from(
    plan: &PreparedPlan,
    snapshot: &Snapshot,
    prefix: &[BoundRecord],
) -> Result<BoundPlan> {
    snapshot.validate(plan)?;
    let mut position = snapshot.commanded_pose_mm;
    let mut transform = Transform {
        work: snapshot.work_offsets[snapshot.active_work_offset as usize - 1],
        temporary: snapshot.temporary_offset_mm,
        tool: snapshot.active_tool_offset_mm,
    };
    let floor = if plan.program().report.units == "inch" {
        2.54e-6
    } else {
        1e-7
    };
    let mut records = Vec::with_capacity(plan.commands().len());
    records.extend_from_slice(prefix);
    let mut shaper_lane = None;
    let mut previous_shaped_rapid = false;
    let mut css = None;
    for record in prefix {
        match record.action {
            BoundAction::Css(demand) => css = Some(demand),
            BoundAction::State(Action::ResetSpindleDemand | Action::Event(Command::Spindle(_))) => {
                css = None
            }
            _ => (),
        }
    }
    for (command, source) in plan
        .commands()
        .iter()
        .copied()
        .enumerate()
        .skip(prefix.len())
    {
        let action=(|| -> Result<BoundAction> {
            match source.action {
                Action::Motion(m)=>{
                    let (start,end,circular)=match m.geometry {
                        Geometry::Line {start,end}=>(transform.source(start),transform.source(end),None),
                        Geometry::Circular {start,end,center,plane,rotation,sweep_radians,..}=>{
                            let start=transform.source(start);let end=transform.source(end);let center=transform.source(center);
                            let axis=geometry::basis(plane).2;
                            let mut normal=[0.0;9];normal[axis]=1.0;
                            let normal=rotate(normal,transform.work.rotation_degrees);
                            let n=(0..3).find(|&i|normal[i].abs()==1.0 && (0..3).filter(|j|*j!=i).all(|j|normal[j]==0.0))
                                .ok_or_else(||error("work rotation produces a non-axis-aligned circle unsupported by the shim"))?;
                            let plane=match n {0=>Plane::Yz,1=>Plane::Xz,_=>Plane::Xy};
                            let rotation=if normal[n]<0.0 {direction(rotation)} else {rotation};
                            (start,end,Some(Circular {center_mm:[center[0],center[1],center[2]],plane,rotation,sweep_radians,
                                axial_rise_mm:end[n]-start[n]}))
                        }
                    };
                    require((0..9).all(|i|(start[i]-position[i]).abs()<=floor),"actual/reviewed approach does not reach source start")?;
                    let motion=Motion {start_mm:start,end_mm:end,circular,feed:m.feed,termination:m.termination,entry_gate:m.entry_gate};
                    snapshot.motion(motion,floor)?;position=end;Ok(BoundAction::Motion(motion))
                }
                Action::Waypoint {frame,axis,value_mm}=>{
                    require(axis<3,"unsupported reviewed waypoint axis")?;
                    let mut end=position;
                    if frame==Frame::Machine {end[axis]=value_mm;} else {
                        let mut work=transform.to_work(position);work[axis]=value_mm;
                        let target=transform.to_machine(work);
                        let (s,c)=sin_cos(transform.work.rotation_degrees);
                        let column=match axis {0=>[c,s,0.0],1=>[-s,c,0.0],_=>[0.0,0.0,1.0]};
                        // Omitted coordinates are preserved bit-for-bit. An
                        // inverse/forward round trip must not create a spurious
                        // XY component on a pure-Z shaper-bypass move.
                        for i in 0..3 {if column[i]!=0.0 {end[i]=target[i];}}
                    }
                    let mut motion=Motion {start_mm:position,end_mm:end,circular:None,feed:Feed::Rapid,
                        termination:motion_command::Termination::ExactPath,entry_gate:motion_command::EntryGate::None};
                    snapshot.motion(motion,floor)?;
                    // A completed planner segment can report a few floating
                    // point rounding units away from its exact nominal target.
                    // A reviewed positioning waypoint at that same target is
                    // stationary. Keep the observed pose (no invented motion),
                    // while retaining the requested waypoint in `source`.
                    // This bound is numerical roundoff, capped by the existing
                    // coordinate validation floor, never the CAM/blend tolerance.
                    // Source cutting geometry is deliberately not coalesced.
                    let scale=position.into_iter().chain(end).map(f64::abs).fold(1.0_f64,f64::max);
                    let roundoff=(8.0*f64::EPSILON*scale).min(floor);
                    if (0..9).all(|i|(end[i]-position[i]).abs()<=roundoff) {
                        motion.end_mm=position;
                    }
                    position=motion.end_mm;Ok(BoundAction::Motion(motion))
                }
                Action::SelectWorkOffset(index)=>{
                    require((1..=9).contains(&index),"invalid selected work offset")?;
                    transform.work=snapshot.work_offsets[index as usize-1];
                    Ok(BoundAction::WorkOffset {index,value:transform.work})
                }
                Action::ClearTemporaryOffsets=>{transform.temporary=[0.0;9];Ok(BoundAction::State(source.action))}
                Action::Event(Command::ToolOffset {offset})=>{
                    transform.tool=if offset==0 {[0.0;9]} else {*snapshot.tool_offsets_mm.get(&offset).ok_or_else(||error("required H offset missing from live tool table"))?};
                    Ok(BoundAction::ToolOffset {number:offset,value_mm:transform.tool})
                }
                Action::Event(Command::ChangeTool {tool})=>{
                    require(tool==0 || snapshot.tool_offsets_mm.contains_key(&tool),"physical T tool missing from live table")?;
                    Ok(BoundAction::State(source.action))
                }
                Action::Event(Command::Spindle(spindle))=>{
                    match spindle {
                        Spindle::Css {surface_mm_per_second,maximum_rpm,clockwise}=>{
                            require(snapshot.spindle.is_some_and(|c|c.css),"CSS requires qualified spindle capability")?;
                            require(clockwise || snapshot.reverse_spindle,"reverse spindle is not supported by this machine policy")?;
                            require(maximum_rpm<=snapshot.maximum_rpm,"CSS RPM cap exceeds machine policy")?;
                            let demand=CssDemand {surface_mm_s:surface_mm_per_second,maximum_rpm,clockwise,
                                x_offset_mm:transform.source(PointMm {x:0.0,y:0.0,z:0.0})[0]};
                            require(demand.valid(),"invalid bound CSS demand")?;
                            return Ok(BoundAction::Css(demand));
                        },
                        Spindle::Rpm {rpm,clockwise}=>{
                            require(clockwise || snapshot.reverse_spindle,"reverse spindle is not supported by this machine policy")?;
                            require(rpm<=snapshot.maximum_rpm,"source spindle demand exceeds machine policy")?;
                        },Spindle::Stop=>(),
                    }
                    Ok(BoundAction::State(source.action))
                }
                Action::Event(Command::Coolant(c))=>{
                    require(match c {motion_command::Coolant::Off=>true,motion_command::Coolant::Flood=>snapshot.flood,
                        motion_command::Coolant::Mist=>snapshot.mist},"coolant function has no qualified task procedure")?;
                    Ok(BoundAction::State(source.action))
                }
                _=>Ok(BoundAction::State(source.action)),
            }
        })().map_err(|e|Error {command:Some(command),..e})?;
        let css_update = update_css(&mut css, action, transform).map_err(|e| Error {
            command: Some(command),
            ..e
        })?;
        let mut drain_before = false;
        if snapshot.shaping == Shaping::EngagedXy {
            if let BoundAction::Motion(m) = action {
                let lane = if m.circular.is_some()
                    || m.start_mm[0] != m.end_mm[0]
                    || m.start_mm[1] != m.end_mm[1]
                {
                    Some(0)
                } else if m.start_mm[2] != m.end_mm[2] {
                    Some(1)
                } else {
                    None
                };
                if let Some(lane) = lane {
                    let rapid = matches!(m.feed, Feed::Rapid);
                    drain_before = shaper_lane.is_some_and(|previous| previous != lane)
                        || (shaper_lane == Some(0)
                            && lane == 0
                            && (previous_shaped_rapid || rapid));
                    shaper_lane = Some(lane);
                    previous_shaped_rapid = rapid;
                }
            }
        }
        records.push(BoundRecord {
            command,
            source,
            action,
            drain_before,
            css_update,
        });
    }
    Ok(BoundPlan {
        records,
        initial: snapshot.commanded_pose_mm,
        final_pose: position,
    })
}

/// Update only the radius origin; an offset change does not restart a spindle.
fn update_css(
    css: &mut Option<CssDemand>,
    action: BoundAction,
    transform: Transform,
) -> Result<Option<CssDemand>> {
    match action {
        BoundAction::Css(demand) => {
            *css = Some(demand);
            Ok(None)
        }
        BoundAction::State(Action::ResetSpindleDemand | Action::Event(Command::Spindle(_))) => {
            *css = None;
            Ok(None)
        }
        BoundAction::WorkOffset { .. }
        | BoundAction::ToolOffset { .. }
        | BoundAction::State(Action::ClearTemporaryOffsets) => {
            if let Some(demand) = css {
                demand.x_offset_mm = transform.source(PointMm {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                })[0];
                require(demand.valid(), "invalid updated CSS offset")?;
            }
            Ok(*css)
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod spindle_offset_tests {
    use super::*;
    #[test]
    fn offset_changes_keep_css_rate_cap_direction_and_order(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        use crate::lowering::{self, AxisDynamics, Dynamics, Payload};
        let mut transform = Transform {
            work: WorkOffset {
                translation_mm: [0.0; 9],
                rotation_degrees: 0.0,
            },
            temporary: [0.0; 9],
            tool: [0.0; 9],
        };
        transform.temporary[0] = 2.0;
        let demand = CssDemand {
            surface_mm_s: 1000.0,
            maximum_rpm: 1800.0,
            clockwise: false,
            x_offset_mm: 2.0,
        };
        let mut css = None;
        assert_eq!(
            update_css(&mut css, BoundAction::Css(demand), transform)?,
            None
        );
        for kind in 0..3 {
            let (action, source) = match kind {
                0 => {
                    transform.work.translation_mm[0] = 7.0;
                    (
                        BoundAction::WorkOffset {
                            index: 2,
                            value: transform.work,
                        },
                        Action::SelectWorkOffset(2),
                    )
                }
                1 => {
                    transform.tool[0] = 0.5;
                    (
                        BoundAction::ToolOffset {
                            number: 2,
                            value_mm: transform.tool,
                        },
                        Action::Event(Command::ToolOffset { offset: 2 }),
                    )
                }
                _ => {
                    transform.temporary[0] = 0.0;
                    (
                        BoundAction::State(Action::ClearTemporaryOffsets),
                        Action::ClearTemporaryOffsets,
                    )
                }
            };
            let update = update_css(&mut css, action, transform)?.ok_or("missing CSS update")?;
            assert_eq!(
                update,
                CssDemand {
                    x_offset_mm: match kind {
                        0 => 9.0,
                        1 => 9.5,
                        _ => 7.5,
                    },
                    ..demand
                }
            );
            let bound = BoundPlan {
                records: vec![BoundRecord {
                    command: 0,
                    source: Record {
                        action: source,
                        site: nextnc_native::compiled::Site::Policy,
                        ordinal: None,
                    },
                    action,
                    drain_before: false,
                    css_update: Some(update),
                }],
                initial: [0.0; 9],
                final_pose: [0.0; 9],
            };
            let lowered = lowering::lower(
                &bound,
                Dynamics {
                    axis_mask: 5,
                    axes: [AxisDynamics {
                        velocity_mm_s: 1.0,
                        acceleration_mm_s2: 1.0,
                        jerk_mm_s3: 1.0,
                    }; 3],
                },
            )?;
            assert_eq!(lowered.pieces().len(), 2);
            assert_eq!(lowered.pieces()[0].payload, Payload::State(action));
            assert_eq!(lowered.pieces()[1].payload, Payload::CssUpdate(update));
            assert_eq!(lowered.pieces()[1].ordinal, 1);
        }
        transform.tool[0] = f64::MAX;
        transform.work.translation_mm[0] = f64::MAX;
        assert!(update_css(
            &mut css,
            BoundAction::State(Action::ClearTemporaryOffsets),
            transform
        )
        .is_err());
        assert_eq!(
            update_css(
                &mut css,
                BoundAction::State(Action::ResetSpindleDemand),
                transform
            )?,
            None
        );
        assert_eq!(css, None);
        Ok(())
    }
}
