//! Versioned semantic single-step groups over the audited, unoptimized plan.
use motion_command::Command;
use nextnc_native::compiled::{Action, PreparedPlan};
use std::ops::Range;

pub const GROUPING_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Boundary {
    Motion,
    Dwell,
    Tool { tool: u32 },
    Fence,
    State,
    End,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    /// Zero-based prepared command indexes, never generated G-code line numbers.
    pub commands: Range<usize>,
    pub boundary: Boundary,
}

#[derive(Clone, Debug)]
pub struct Layout {
    groups: Vec<Group>,
    commands: usize,
    barriers: Vec<Range<usize>>,
    drains_before: Vec<usize>,
}

impl Layout {
    /// Every expanded polyline segment is a step; one analytic helix is one step.
    /// State preceding a source motion stays in that phase's group. Tool changes
    /// are isolated procedure boundaries, not an incidental part of a motion.
    pub fn from_prepared(plan: &PreparedPlan) -> Self {
        let mut groups = Vec::new();
        for span in plan.spans() {
            let mut start = span.commands.start;
            for index in span.commands.clone() {
                let action = plan.commands()[index].action;
                if matches!(action, Action::Event(Command::ChangeTool { .. })) && start < index {
                    groups.push(Group {
                        commands: start..index,
                        boundary: Boundary::State,
                    });
                    start = index;
                }
                let boundary = match action {
                    Action::Motion(_) | Action::Waypoint { .. } => Some(Boundary::Motion),
                    Action::Event(Command::Dwell { .. }) => Some(Boundary::Dwell),
                    Action::Event(Command::ChangeTool { tool }) => Some(Boundary::Tool { tool }),
                    Action::Event(Command::Fence) => Some(Boundary::Fence),
                    Action::Event(Command::End) => Some(Boundary::End),
                    _ => None,
                };
                if let Some(boundary) = boundary {
                    groups.push(Group {
                        commands: start..index + 1,
                        boundary,
                    });
                    start = index + 1;
                }
            }
            if start < span.commands.end {
                groups.push(Group {
                    commands: start..span.commands.end,
                    boundary: Boundary::State,
                });
            }
        }
        let barriers = groups
            .iter()
            .filter(|g| {
                matches!(
                    g.boundary,
                    Boundary::Tool { .. } | Boundary::Dwell | Boundary::Fence | Boundary::End
                )
            })
            .map(|g| g.commands.clone())
            .collect();
        Self {
            groups,
            commands: plan.commands().len(),
            barriers,
            drains_before: Vec::new(),
        }
    }

    /// Add downstream lane-change drains without altering semantic step groups
    /// or adding synthetic motion. The bound record sequence must be this plan.
    pub fn from_bound(
        plan: &PreparedPlan,
        bound: &crate::binding::BoundPlan,
    ) -> Result<Self, &'static str> {
        if plan.commands().len() != bound.records().len()
            || plan
                .commands()
                .iter()
                .zip(bound.records())
                .enumerate()
                .any(|(i, (source, b))| b.command != i || *source != b.source)
        {
            return Err("bound plan source identity differs");
        }
        let mut result = Self::from_prepared(plan);
        result.drains_before = bound
            .records()
            .iter()
            .filter(|r| r.drain_before && r.command > 0)
            .map(|r| r.command)
            .collect();
        Ok(result)
    }

    pub(crate) fn next_barrier(&self, completed: usize) -> Option<&Range<usize>> {
        self.barriers
            .get(self.barriers.partition_point(|b| b.end <= completed))
    }
    pub(crate) fn next_drain(&self, completed: usize) -> Option<usize> {
        self.drains_before
            .get(self.drains_before.partition_point(|p| *p <= completed))
            .copied()
    }

    pub fn groups(&self) -> &[Group] {
        &self.groups
    }
    pub fn commands(&self) -> usize {
        self.commands
    }
    pub fn group_for(&self, command: usize) -> Option<&Group> {
        let index = self.groups.partition_point(|g| g.commands.end <= command);
        self.groups
            .get(index)
            .filter(|g| g.commands.contains(&command))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nextnc_native::{compiled, part21::Limits};

    #[test]
    fn groups_cover_each_use_once_and_isolate_procedures() -> Result<(), Box<dyn std::error::Error>>
    {
        for name in ["mill-mm-polyline-8", "lathe-mm-arcs-8", "mill-inch-arcs-8"] {
            let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests-rust/fixtures/benchmark");
            let plan = compiled::prepare(
                &std::fs::read_to_string(base.join(format!("{name}.stpnc")))?,
                &std::fs::read_to_string(base.join(format!("{name}.plan.json")))?,
                &Limits::default(),
            )?;
            let layout = Layout::from_prepared(&plan);
            let mut end = 0;
            let mut source_motions = 0;
            for group in layout.groups() {
                assert_eq!(group.commands.start, end);
                assert!(!group.commands.is_empty());
                end = group.commands.end;
                if matches!(group.boundary, Boundary::Tool { .. }) {
                    assert_eq!(group.commands.len(), 1);
                }
                let motions = plan.commands()[group.commands.clone()]
                    .iter()
                    .filter(|r| matches!(r.action, Action::Motion(_)))
                    .count();
                assert!(motions <= 1);
                source_motions += motions;
                for i in group.commands.clone() {
                    assert_eq!(layout.group_for(i), Some(group));
                }
            }
            assert_eq!(end, layout.commands());
            assert_eq!(source_motions, 8);
            assert!(layout.group_for(end).is_none());
        }
        Ok(())
    }
}
