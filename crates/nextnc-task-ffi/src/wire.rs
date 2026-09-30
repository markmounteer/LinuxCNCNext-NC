//! Fixed-width C boundary. Input integers are validated before enum conversion.
use motion_command::{Command, Coolant, EntryGate, Feed, Machine, Plane, Spindle, Termination};
use nextnc_native::compiled::Action;
use nextnc_task::{binding, lowering};
use std::collections::BTreeMap;

pub const ABI: u32 = 1;
pub const MAX_TOOLS: usize = 4096;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Snapshot {
    pub abi: u32,
    pub bytes: u32,
    pub machine: u32,
    pub axis_mask: u32,
    pub work_offset: u32,
    pub shaping: u32,
    /// 1 reverse spindle, 2 flood, 4 mist. Not enable/start permission.
    pub capabilities: u32,
    pub reserved: u32,
    pub pose: [f64; 9],
    pub work: [[f64; 9]; 9],
    pub rotation: [f64; 9],
    pub temporary: [f64; 9],
    pub tool_offset: [f64; 9],
    pub minimum: [f64; 3],
    pub maximum: [f64; 3],
    pub velocity: [f64; 3],
    pub acceleration: [f64; 3],
    pub jerk: [f64; 3],
    pub maximum_rpm: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Tool {
    pub number: u32,
    pub reserved: u32,
    pub offset: [f64; 9],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Message {
    pub abi: u32,
    pub bytes: u32,
    pub kind: u32,
    /// 1 rapid, 2 at-speed entry, 4 drain before command, 8 last piece.
    pub flags: u32,
    pub command: u64,
    pub piece: u32,
    pub pieces: u32,
    pub argument: i32,
    pub turn: i32,
    pub start: [f64; 9],
    pub end: [f64; 9],
    pub center: [f64; 3],
    pub normal: [f64; 3],
    pub velocity: f64,
    pub maximum_velocity: f64,
    pub acceleration: f64,
    pub jerk: f64,
    pub value: f64,
    pub feed_mm_s: f64,
}

impl Snapshot {
    pub fn decode(
        &self,
        tools: &[Tool],
    ) -> Result<(binding::Snapshot, lowering::Dynamics), String> {
        if self.abi != ABI || self.bytes as usize != std::mem::size_of::<Self>() {
            return Err("unsupported snapshot ABI/size".into());
        }
        if self.reserved != 0 || self.capabilities & !7 != 0 || self.shaping > 1 {
            return Err("unknown snapshot flags".into());
        }
        let machine = match (self.machine, self.axis_mask) {
            (1, 7) => Machine::MillXyz,
            (2, 5) => Machine::LatheXz,
            _ => return Err("machine/axis mask must be XYZ mill or XZ lathe".into()),
        };
        if !(1..=9).contains(&self.work_offset) {
            return Err("invalid active work offset".into());
        }
        if tools.len() > MAX_TOOLS {
            return Err("too many tool table entries".into());
        }
        let mut table = BTreeMap::new();
        for tool in tools {
            if tool.reserved != 0
                || tool.number == 0
                || table.insert(tool.number, tool.offset).is_some()
            {
                return Err("invalid or duplicate tool table entry".into());
            }
        }
        Ok((
            binding::Snapshot {
                machine,
                commanded_pose_mm: self.pose,
                active_work_offset: self.work_offset as u8,
                work_offsets: std::array::from_fn(|i| binding::WorkOffset {
                    translation_mm: self.work[i],
                    rotation_degrees: self.rotation[i],
                }),
                temporary_offset_mm: self.temporary,
                active_tool_offset_mm: self.tool_offset,
                tool_offsets_mm: table,
                limits: std::array::from_fn(|i| binding::AxisLimits {
                    minimum_mm: self.minimum[i],
                    maximum_mm: self.maximum[i],
                }),
                shaping: if self.shaping == 0 {
                    binding::Shaping::Disabled
                } else {
                    binding::Shaping::EngagedXy
                },
                reverse_spindle: self.capabilities & 1 != 0,
                maximum_rpm: self.maximum_rpm,
                flood: self.capabilities & 2 != 0,
                mist: self.capabilities & 4 != 0,
            },
            lowering::Dynamics {
                axis_mask: self.axis_mask,
                axes: std::array::from_fn(|i| lowering::AxisDynamics {
                    velocity_mm_s: self.velocity[i],
                    acceleration_mm_s2: self.acceleration[i],
                    jerk_mm_s3: self.jerk[i],
                }),
            },
        ))
    }
}

pub fn encode(piece: &lowering::Piece, pieces: usize, drain: bool) -> Result<Message, String> {
    let mut out = Message {
        abi: ABI,
        bytes: std::mem::size_of::<Message>() as u32,
        command: piece.command as u64,
        piece: piece.ordinal as u32,
        pieces: pieces as u32,
        flags: if drain { 4 } else { 0 } | if piece.ordinal + 1 == pieces { 8 } else { 0 },
        ..Message::default()
    };
    let integer =
        |n: u32| i32::try_from(n).map_err(|_| "identifier exceeds C ABI range".to_string());
    match piece.payload {
        lowering::Payload::Termination(mode) => {
            out.kind = 4;
            match mode {
                Termination::ExactStop => out.argument = 0,
                Termination::ExactPath => out.argument = 1,
                Termination::Blend { max_deviation_mm } => {
                    out.argument = 2;
                    out.value = max_deviation_mm;
                }
            }
        }
        lowering::Payload::Motion {
            motion,
            dynamics,
            turn,
        } => {
            motion_fields(&mut out, motion)?;
            out.kind = if turn.is_some() { 2 } else { 1 };
            out.turn = turn.unwrap_or(0);
            out.velocity = dynamics.velocity_mm_s;
            out.maximum_velocity = dynamics.maximum_velocity_mm_s;
            out.acceleration = dynamics.acceleration_mm_s2;
            out.jerk = dynamics.jerk_mm_s3;
        }
        lowering::Payload::Stationary(motion) => {
            motion_fields(&mut out, motion)?;
            out.kind = 3;
        }
        lowering::Payload::State(state) => match state {
            binding::BoundAction::WorkOffset { index, value } => {
                out.kind = 13;
                out.argument = i32::from(index);
                out.end = value.translation_mm;
                out.value = value.rotation_degrees;
            }
            binding::BoundAction::ToolOffset { number, value_mm } => {
                out.kind = 14;
                out.argument = integer(number)?;
                out.end = value_mm;
            }
            binding::BoundAction::State(action) => match action {
                Action::ResetModes => out.kind = 10,
                Action::ClearTemporaryOffsets => out.kind = 11,
                Action::ResetSpindleDemand => out.kind = 12,
                Action::RestoreFeedPerMinute => out.kind = 15,
                Action::Event(event) => match event {
                    Command::ChangeTool { tool } => {
                        out.kind = 16;
                        out.argument = integer(tool)?;
                    }
                    Command::Spindle(spindle) => {
                        out.kind = 17;
                        match spindle {
                            Spindle::Stop => (),
                            Spindle::Rpm { rpm, clockwise } => {
                                out.value = rpm;
                                out.argument = if clockwise { 1 } else { -1 };
                            }
                            Spindle::Css { .. } => return Err("CSS requires Stage 4".into()),
                        }
                    }
                    Command::Coolant(coolant) => {
                        out.kind = 18;
                        out.argument = match coolant {
                            Coolant::Off => 0,
                            Coolant::Flood => 1,
                            Coolant::Mist => 2,
                        };
                    }
                    Command::Dwell { seconds } => {
                        out.kind = 19;
                        out.value = seconds;
                    }
                    Command::Fence => out.kind = 20,
                    Command::End => out.kind = 21,
                    _ => return Err("unresolved source event reached ABI".into()),
                },
                _ => return Err("unresolved source action reached ABI".into()),
            },
            _ => return Err("unexpected state payload".into()),
        },
    }
    Ok(out)
}

fn motion_fields(out: &mut Message, motion: binding::Motion) -> Result<(), String> {
    out.start = motion.start_mm;
    out.end = motion.end_mm;
    match motion.feed {
        Feed::Rapid => out.flags |= 1,
        Feed::PerSecond(rate) => out.feed_mm_s = rate,
        _ => return Err("synchronized feed requires Stage 4".into()),
    }
    if motion.entry_gate == EntryGate::SpindlesAtSpeed {
        out.flags |= 2;
    }
    if let Some(circle) = motion.circular {
        out.center = circle.center_mm;
        out.normal[match circle.plane {
            Plane::Xy => 2,
            Plane::Xz => 1,
            Plane::Yz => 0,
        }] = 1.0;
    }
    Ok(())
}
