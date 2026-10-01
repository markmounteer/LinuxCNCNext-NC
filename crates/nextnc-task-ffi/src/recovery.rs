//! Immutable expected motion modes, projected on the preparation worker.
//! This is acceptance validation, not proof of physical execution.
use crate::{wire::Message, Result};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Modes {
    pub motion: u32,
    /// 0 inherits the launch plane; 1 XY, 2 XZ, 3 YZ.
    pub plane: u32,
    /// None inherits the launch feed. Native G94 explicitly sets Some(0).
    pub feed_mm_s: Option<f64>,
}

impl Modes {
    pub fn apply(&mut self, message: &Message) -> Result<Option<Self>> {
        let rapid = message.flags & 1 != 0;
        match message.kind {
            15 => self.feed_mm_s = Some(0.0),
            1..=3 => {
                if !rapid {
                    self.feed_mm_s = Some(message.feed_mm_s);
                }
                if message.kind == 2 {
                    self.motion = if message.turn < 0 { 3 } else { 4 };
                    self.plane = match message.normal {
                        [0.0, 0.0, 1.0] => 1,
                        [0.0, 1.0, 0.0] => 2,
                        [1.0, 0.0, 0.0] => 3,
                        _ => return Err("unsupported recovery motion plane".into()),
                    };
                } else {
                    self.motion = if rapid { 1 } else { 2 };
                }
                if message.kind != 3 {
                    return Ok(Some(*self));
                }
            }
            _ => {}
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inheritance_and_ordered_state_are_independent_of_receipt_values() -> Result<()> {
        let mut modes = Modes::default();
        let mut message = Message {
            kind: 1,
            flags: 1,
            ..Message::default()
        };
        assert_eq!(
            modes.apply(&message)?,
            Some(Modes {
                motion: 1,
                ..Modes::default()
            })
        );
        message.kind = 15;
        assert_eq!(modes.apply(&message)?, None);
        message.kind = 2;
        message.flags = 0;
        message.turn = -1;
        message.normal = [0.0, 1.0, 0.0];
        message.feed_mm_s = 2.0;
        let arc = Modes {
            motion: 3,
            plane: 2,
            feed_mm_s: Some(2.0),
        };
        assert_eq!(modes.apply(&message)?, Some(arc));
        message.kind = 1;
        message.flags = 1;
        message.feed_mm_s = 999.0;
        assert_eq!(modes.apply(&message)?, Some(Modes { motion: 1, ..arc }));
        message.kind = 3;
        message.flags = 0;
        message.feed_mm_s = 4.0;
        assert_eq!(modes.apply(&message)?, None);
        message.kind = 1;
        message.flags = 1;
        assert_eq!(
            modes.apply(&message)?,
            Some(Modes {
                motion: 1,
                plane: 2,
                feed_mm_s: Some(4.0)
            })
        );
        message.kind = 15;
        assert_eq!(modes.apply(&message)?, None);
        assert_eq!(modes.feed_mm_s, Some(0.0));
        message.kind = 2;
        message.flags = 0;
        assert_eq!(
            modes.apply(&message)?.ok_or("missing arc receipt")?.motion,
            3
        );
        message.kind = 3;
        message.flags = 1;
        assert_eq!(modes.apply(&message)?, None);
        assert_eq!(
            modes.motion, 1,
            "stationary rapid selects G0 without a motion receipt"
        );
        assert_eq!(modes.feed_mm_s, Some(4.0), "rapid preserves the last feed");
        assert_eq!(modes.plane, 2, "rapid preserves the last plane");
        Ok(())
    }
}
