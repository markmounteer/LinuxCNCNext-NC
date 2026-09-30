//! Deterministic, bounded native artifacts. No live state or permission is cached.
//! Decode is independent of encode and re-audits serialized commands against the
//! exact embedded source/setup; a checksum alone never qualifies a command plan.
use crate::{
    capabilities, command_audit,
    compiled::{self, Action, Frame, Phase, PreparedPlan, Record, Site, Span},
    part21::Limits,
    plan, profile, tool_table, Diagnostic, Result,
};
use motion_command::{
    v2::{Geometry, Motion, Movement, Tolerance},
    Command, Coolant, EntryGate, Feed, Plane, PointMm, Rotation, Spindle, Termination,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

const MAGIC: &[u8; 8] = b"NEXTNC\0\x01";
const SCHEMA: &str = include_str!("bundle-schema.txt");
pub const COMPILER_SHA256: &str = env!("NEXTNC_COMPILER_SHA256");
const POLICY_DETAIL:&str="exact-path;fit-mm=0;blend-mm=0;no-reductions;first-nonrapid-after-spindle-demand-at-speed;task-events-drain;live-binding-required";
pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn schema_sha256() -> String {
    digest(format!("{SCHEMA}\n{}", include_str!("profile-shape.json")).as_bytes())
}
pub fn policy_sha256() -> String {
    digest(format!("{}\n{POLICY_DETAIL}", compiled::POLICY).as_bytes())
}

/// Borrowed exact snapshots. Paths/timestamps are not identities or instructions.
#[derive(Clone, Copy, Debug)]
pub struct Inputs<'a> {
    pub source: &'a str,
    pub setup: &'a str,
    pub tool_table: Option<&'a str>,
    pub target: Option<&'a str>,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Identity {
    pub source_sha256: String,
    pub setup_sha256: String,
    pub tool_table_sha256: Option<String>,
    pub target_sha256: Option<String>,
    pub compiler_sha256: String,
    pub schema_sha256: String,
    pub policy_sha256: String,
}
impl Identity {
    pub fn of(inputs: Inputs<'_>) -> Self {
        Self {
            source_sha256: digest(inputs.source.as_bytes()),
            setup_sha256: digest(inputs.setup.as_bytes()),
            tool_table_sha256: inputs.tool_table.map(|s| digest(s.as_bytes())),
            target_sha256: inputs.target.map(|s| digest(s.as_bytes())),
            compiler_sha256: COMPILER_SHA256.into(),
            schema_sha256: schema_sha256(),
            policy_sha256: policy_sha256(),
        }
    }
}
#[derive(Debug)]
pub struct Artifact {
    bytes: Vec<u8>,
    sha256: String,
    identity: Identity,
    prepared: PreparedPlan,
}
impl Artifact {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn sha256(&self) -> &str {
        &self.sha256
    }
    pub fn identity(&self) -> &Identity {
        &self.identity
    }
    pub fn prepared(&self) -> &PreparedPlan {
        &self.prepared
    }
}
fn fail(code: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("native-bundle", code, message)
}

/// First audit preparation, then independently reload the serialization before
/// returning immutable artifact bytes. No caller can bless an edited command Vec.
pub fn compile(inputs: Inputs<'_>, limits: &Limits) -> Result<Artifact> {
    let bytes = {
        let prepared = compiled::prepare(inputs.source, inputs.setup, limits)?;
        tool_table::check(inputs.tool_table, prepared.program(), prepared.setup())?;
        capabilities::check_prepared(inputs.target, &prepared, limits)?;
        encode(&prepared, inputs, limits)?
    };
    // The original compiler plan has been dropped; this result is reconstructed
    // from the actual bytes with a separate reader and semantic audit.
    load(bytes, limits)
}
pub fn load(bytes: Vec<u8>, limits: &Limits) -> Result<Artifact> {
    let (prepared, identity) = decode(&bytes, limits)?;
    Ok(Artifact {
        sha256: digest(&bytes),
        bytes,
        identity,
        prepared,
    })
}

struct Writer {
    bytes: Vec<u8>,
    limit: usize,
}
impl Writer {
    fn put(&mut self, bytes: &[u8]) -> Result<()> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            return Err(fail("BUNDLE_SIZE", "Native bundle exceeds byte limit"));
        }
        self.bytes
            .try_reserve(bytes.len())
            .map_err(|_| fail("RESOURCE", "Cannot allocate bundle storage"))?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    fn tag(&mut self, n: u8) -> Result<()> {
        self.put(&[n])
    }
    fn u64(&mut self, n: u64) -> Result<()> {
        self.put(&n.to_le_bytes())
    }
    fn usize(&mut self, n: usize) -> Result<()> {
        self.u64(
            n.try_into()
                .map_err(|_| fail("INDEX", "Index exceeds wire range"))?,
        )
    }
    fn f64(&mut self, n: f64) -> Result<()> {
        if !n.is_finite() {
            return Err(fail("NONFINITE", "Cannot encode nonfinite value"));
        }
        self.put(&n.to_le_bytes())
    }
    fn text(&mut self, s: Option<&str>) -> Result<()> {
        if let Some(s) = s {
            self.usize(s.len())?;
            self.put(s.as_bytes())
        } else {
            self.u64(u64::MAX)
        }
    }
    fn point(&mut self, p: PointMm) -> Result<()> {
        self.f64(p.x)?;
        self.f64(p.y)?;
        self.f64(p.z)
    }
    fn motion(&mut self, m: Motion) -> Result<()> {
        match m.geometry {
            Geometry::Line { start, end } => {
                self.tag(0)?;
                self.point(start)?;
                self.point(end)?;
            }
            Geometry::Circular {
                start,
                end,
                center,
                plane,
                rotation,
                sweep_radians,
                axial_rise_mm,
            } => {
                self.tag(1)?;
                self.point(start)?;
                self.point(end)?;
                self.point(center)?;
                self.tag(match plane {
                    Plane::Xy => 0,
                    Plane::Xz => 1,
                    Plane::Yz => 2,
                })?;
                self.tag(if rotation == Rotation::Clockwise {
                    0
                } else {
                    1
                })?;
                self.f64(sweep_radians)?;
                self.f64(axial_rise_mm)?;
            }
        }
        match m.feed {
            Feed::Rapid => self.tag(0)?,
            Feed::PerSecond(v) => {
                self.tag(1)?;
                self.f64(v)?;
            }
            Feed::PerRevolution {
                mm_per_rev,
                spindle,
            } => {
                self.tag(2)?;
                self.f64(mm_per_rev)?;
                self.tag(spindle)?;
            }
        }
        match m.termination {
            Termination::ExactPath => self.tag(0)?,
            Termination::ExactStop => self.tag(1)?,
            Termination::Blend { max_deviation_mm } => {
                self.tag(2)?;
                self.f64(max_deviation_mm)?;
            }
        }
        self.tag(if m.entry_gate == EntryGate::None {
            0
        } else {
            1
        })?;
        let movement = match m.movement {
            Movement::Unspecified => 0,
            Movement::Rapid => 1,
            Movement::Cutting => 2,
            Movement::FinishCutting => 3,
            Movement::LeadIn => 4,
            Movement::LeadOut => 5,
            Movement::LinkTransition => 6,
            Movement::LinkDirect => 7,
            Movement::RampHelix => 8,
            Movement::RampProfile => 9,
            Movement::RampZigZag => 10,
            Movement::Ramp => 11,
            Movement::Plunge => 12,
            Movement::Predrill => 13,
            Movement::Extended => 14,
            Movement::Reduced => 15,
            Movement::HighFeed => 16,
        };
        self.tag(movement)?;
        match m.tolerance {
            Tolerance::Missing => self.tag(0),
            Tolerance::FusionOperationMm(v) => {
                self.tag(1)?;
                self.f64(v)
            }
            Tolerance::SourceDeclaredMm(v) => {
                self.tag(2)?;
                self.f64(v)
            }
        }
    }
    fn event(&mut self, c: Command) -> Result<()> {
        match c {
            Command::Spindle(Spindle::Stop) => self.tag(0),
            Command::Spindle(Spindle::Rpm { rpm, clockwise }) => {
                self.tag(1)?;
                self.f64(rpm)?;
                self.tag(u8::from(clockwise))
            }
            Command::Spindle(Spindle::Css {
                surface_mm_per_second,
                maximum_rpm,
                clockwise,
            }) => {
                self.tag(2)?;
                self.f64(surface_mm_per_second)?;
                self.f64(maximum_rpm)?;
                self.tag(u8::from(clockwise))
            }
            Command::Coolant(c) => {
                self.tag(3)?;
                self.tag(match c {
                    Coolant::Off => 0,
                    Coolant::Flood => 1,
                    Coolant::Mist => 2,
                })
            }
            Command::ChangeTool { tool } => {
                self.tag(4)?;
                self.put(&tool.to_le_bytes())
            }
            Command::ToolOffset { offset } => {
                self.tag(5)?;
                self.put(&offset.to_le_bytes())
            }
            Command::Dwell { seconds } => {
                self.tag(6)?;
                self.f64(seconds)
            }
            Command::Fence => self.tag(7),
            Command::End => self.tag(8),
            Command::Motion(_) => Err(fail(
                "CONTRACT_REVISION",
                "Revision-1 geometry is forbidden in a native bundle",
            )),
        }
    }
    fn record(&mut self, r: Record) -> Result<()> {
        self.tag(match r.site {
            Site::Policy => 0,
            Site::Retract => 1,
            Site::Approach => 2,
            Site::Link => 3,
            Site::End => 4,
            Site::Source => 5,
        })?;
        self.u64(match r.ordinal {
            Some(n) => u64::try_from(n).map_err(|_| fail("INDEX", "Use index overflow"))?,
            None => u64::MAX,
        })?;
        match r.action {
            Action::Motion(m) => {
                self.tag(0)?;
                self.motion(m)
            }
            Action::Event(c) => {
                self.tag(1)?;
                self.event(c)
            }
            Action::ResetModes => self.tag(2),
            Action::ClearTemporaryOffsets => self.tag(3),
            Action::ResetSpindleDemand => self.tag(4),
            Action::SelectWorkOffset(w) => {
                self.tag(5)?;
                self.tag(w)
            }
            Action::RestoreFeedPerMinute => self.tag(6),
            Action::Waypoint {
                frame,
                axis,
                value_mm,
            } => {
                self.tag(7)?;
                self.tag(if frame == Frame::Machine { 0 } else { 1 })?;
                self.tag(axis.try_into().map_err(|_| fail("AXIS", "Axis overflow"))?)?;
                self.f64(value_mm)
            }
        }
    }
}
fn encode(p: &PreparedPlan, inputs: Inputs<'_>, limits: &Limits) -> Result<Vec<u8>> {
    let mut w = Writer {
        bytes: Vec::new(),
        limit: limits.bundle_bytes,
    };
    w.put(MAGIC)?;
    w.put(COMPILER_SHA256.as_bytes())?;
    w.put(schema_sha256().as_bytes())?;
    w.put(policy_sha256().as_bytes())?;
    for text in [
        Some(inputs.source),
        Some(inputs.setup),
        inputs.tool_table,
        inputs.target,
    ] {
        w.text(text)?;
    }
    w.usize(p.commands().len())?;
    for record in p.commands() {
        w.record(*record)?;
    }
    w.usize(p.spans().len())?;
    for span in p.spans() {
        match span.phase {
            Phase::Header => w.tag(0)?,
            Phase::Transition(s) => {
                w.tag(1)?;
                w.usize(s)?;
            }
            Phase::Entry(s) => {
                w.tag(2)?;
                w.usize(s)?;
            }
            Phase::Path { section, path } => {
                w.tag(3)?;
                w.usize(section)?;
                w.usize(path)?;
            }
            Phase::End => w.tag(4)?,
        }
        w.usize(span.commands.start)?;
        w.usize(span.commands.end)?;
    }
    Ok(w.bytes)
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self
            .at
            .checked_add(n)
            .filter(|n| *n <= self.bytes.len())
            .ok_or_else(|| {
                fail("TRUNCATED", "Incomplete native bundle").with("byteOffset", self.at)
            })?;
        let b = &self.bytes[self.at..end];
        self.at = end;
        Ok(b)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        self.take(N)?
            .try_into()
            .map_err(|_| fail("TRUNCATED", "Incomplete wire scalar"))
    }
    fn tag(&mut self) -> Result<u8> {
        Ok(self.array::<1>()?[0])
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    fn usize(&mut self) -> Result<usize> {
        self.u64()?
            .try_into()
            .map_err(|_| fail("INDEX", "Index exceeds this host's range"))
    }
    fn f64(&mut self) -> Result<f64> {
        let n = f64::from_le_bytes(self.array()?);
        if !n.is_finite() {
            return Err(fail("NONFINITE", "Nonfinite wire scalar"));
        }
        Ok(n)
    }
    fn boolean(&mut self) -> Result<bool> {
        match self.tag()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(fail("TAG", "Invalid boolean tag")),
        }
    }
    fn text(&mut self, optional: bool, limit: usize) -> Result<Option<&'a str>> {
        let n = self.u64()?;
        if optional && n == u64::MAX {
            return Ok(None);
        }
        let n = usize::try_from(n).map_err(|_| fail("INPUT_SIZE", "Input length overflow"))?;
        if n > limit {
            return Err(fail("INPUT_SIZE", "Embedded input exceeds its byte limit"));
        }
        let text = std::str::from_utf8(self.take(n)?)
            .map_err(|_| fail("UTF8", "Embedded input is not UTF-8"))?;
        Ok(Some(text))
    }
    fn point(&mut self) -> Result<PointMm> {
        Ok(PointMm {
            x: self.f64()?,
            y: self.f64()?,
            z: self.f64()?,
        })
    }
    fn motion(&mut self) -> Result<Motion> {
        let geometry = match self.tag()? {
            0 => Geometry::Line {
                start: self.point()?,
                end: self.point()?,
            },
            1 => {
                let start = self.point()?;
                let end = self.point()?;
                let center = self.point()?;
                let plane = match self.tag()? {
                    0 => Plane::Xy,
                    1 => Plane::Xz,
                    2 => Plane::Yz,
                    _ => return Err(fail("TAG", "Invalid circular plane")),
                };
                let rotation = match self.tag()? {
                    0 => Rotation::Clockwise,
                    1 => Rotation::Counterclockwise,
                    _ => return Err(fail("TAG", "Invalid rotation")),
                };
                Geometry::Circular {
                    start,
                    end,
                    center,
                    plane,
                    rotation,
                    sweep_radians: self.f64()?,
                    axial_rise_mm: self.f64()?,
                }
            }
            _ => return Err(fail("TAG", "Invalid geometry tag")),
        };
        let feed = match self.tag()? {
            0 => Feed::Rapid,
            1 => Feed::PerSecond(self.f64()?),
            2 => Feed::PerRevolution {
                mm_per_rev: self.f64()?,
                spindle: self.tag()?,
            },
            _ => return Err(fail("TAG", "Invalid feed tag")),
        };
        let termination = match self.tag()? {
            0 => Termination::ExactPath,
            1 => Termination::ExactStop,
            2 => Termination::Blend {
                max_deviation_mm: self.f64()?,
            },
            _ => return Err(fail("TAG", "Invalid termination tag")),
        };
        let entry_gate = match self.tag()? {
            0 => EntryGate::None,
            1 => EntryGate::SpindlesAtSpeed,
            _ => return Err(fail("TAG", "Invalid entry gate")),
        };
        let movement = match self.tag()? {
            0 => Movement::Unspecified,
            1 => Movement::Rapid,
            2 => Movement::Cutting,
            3 => Movement::FinishCutting,
            4 => Movement::LeadIn,
            5 => Movement::LeadOut,
            6 => Movement::LinkTransition,
            7 => Movement::LinkDirect,
            8 => Movement::RampHelix,
            9 => Movement::RampProfile,
            10 => Movement::RampZigZag,
            11 => Movement::Ramp,
            12 => Movement::Plunge,
            13 => Movement::Predrill,
            14 => Movement::Extended,
            15 => Movement::Reduced,
            16 => Movement::HighFeed,
            _ => return Err(fail("TAG", "Invalid movement tag")),
        };
        let tolerance = match self.tag()? {
            0 => Tolerance::Missing,
            1 => Tolerance::FusionOperationMm(self.f64()?),
            2 => Tolerance::SourceDeclaredMm(self.f64()?),
            _ => return Err(fail("TAG", "Invalid tolerance tag")),
        };
        Ok(Motion {
            geometry,
            feed,
            termination,
            entry_gate,
            movement,
            tolerance,
        })
    }
    fn event(&mut self) -> Result<Command> {
        Ok(match self.tag()? {
            0 => Command::Spindle(Spindle::Stop),
            1 => Command::Spindle(Spindle::Rpm {
                rpm: self.f64()?,
                clockwise: self.boolean()?,
            }),
            2 => Command::Spindle(Spindle::Css {
                surface_mm_per_second: self.f64()?,
                maximum_rpm: self.f64()?,
                clockwise: self.boolean()?,
            }),
            3 => Command::Coolant(match self.tag()? {
                0 => Coolant::Off,
                1 => Coolant::Flood,
                2 => Coolant::Mist,
                _ => return Err(fail("TAG", "Invalid coolant tag")),
            }),
            4 => Command::ChangeTool {
                tool: u32::from_le_bytes(self.array()?),
            },
            5 => Command::ToolOffset {
                offset: u32::from_le_bytes(self.array()?),
            },
            6 => Command::Dwell {
                seconds: self.f64()?,
            },
            7 => Command::Fence,
            8 => Command::End,
            _ => return Err(fail("TAG", "Invalid event tag")),
        })
    }
    fn record(&mut self) -> Result<Record> {
        let site = match self.tag()? {
            0 => Site::Policy,
            1 => Site::Retract,
            2 => Site::Approach,
            3 => Site::Link,
            4 => Site::End,
            5 => Site::Source,
            _ => return Err(fail("TAG", "Invalid source-site tag")),
        };
        let ordinal = match self.u64()? {
            u64::MAX => None,
            n => Some(
                n.try_into()
                    .map_err(|_| fail("INDEX", "Source use exceeds host range"))?,
            ),
        };
        let action = match self.tag()? {
            0 => Action::Motion(self.motion()?),
            1 => Action::Event(self.event()?),
            2 => Action::ResetModes,
            3 => Action::ClearTemporaryOffsets,
            4 => Action::ResetSpindleDemand,
            5 => Action::SelectWorkOffset(self.tag()?),
            6 => Action::RestoreFeedPerMinute,
            7 => {
                let frame = match self.tag()? {
                    0 => Frame::Machine,
                    1 => Frame::Work,
                    _ => return Err(fail("TAG", "Invalid coordinate frame")),
                };
                Action::Waypoint {
                    frame,
                    axis: self.tag()? as usize,
                    value_mm: self.f64()?,
                }
            }
            _ => return Err(fail("TAG", "Invalid action tag")),
        };
        Ok(Record {
            action,
            site,
            ordinal,
        })
    }
}
fn decode(bytes: &[u8], limits: &Limits) -> Result<(PreparedPlan, Identity)> {
    limits.check()?;
    if bytes.len() > limits.bundle_bytes {
        return Err(fail("BUNDLE_SIZE", "Bundle exceeds byte limit"));
    }
    let mut r = Reader { bytes, at: 0 };
    if r.take(8)? != MAGIC {
        return Err(fail("SCHEMA", "Unknown native bundle schema"));
    }
    for (name, expected) in [
        ("compiler", COMPILER_SHA256.to_string()),
        ("schema", schema_sha256()),
        ("policy", policy_sha256()),
    ] {
        if r.take(64)? != expected.as_bytes() {
            return Err(fail(
                "IDENTITY",
                format!("Bundle {name} differs from this compiler; prepare again"),
            ));
        }
    }
    let source = r
        .text(false, limits.input_bytes)?
        .ok_or_else(|| fail("SOURCE", "Missing source"))?;
    let setup = r
        .text(false, limits.input_bytes)?
        .ok_or_else(|| fail("SETUP", "Missing reviewed setup"))?;
    let tool_table = r.text(true, 1024 * 1024)?;
    let target = r.text(true, 1024 * 1024)?;
    let inputs = Inputs {
        source,
        setup,
        tool_table,
        target,
    };
    let program = profile::decode(source, limits)?;
    let setup = plan::parse(setup, &program, limits)?;
    let count = r.usize()?;
    // A record has at least site+ordinal+action (10 bytes). Check BEFORE reserve.
    if count > limits.output_commands || count > (bytes.len() - r.at) / 10 {
        return Err(fail(
            "OUTPUT_COMMAND_LIMIT",
            "Invalid or excessive command count",
        ));
    }
    let mut commands = Vec::new();
    commands
        .try_reserve_exact(count)
        .map_err(|_| fail("RESOURCE", "Cannot allocate decoded commands"))?;
    for index in 0..count {
        commands.push(
            r.record()
                .map_err(|e| e.with("command", index + 1).with("byteOffset", r.at))?,
        );
    }
    let count = r.usize()?;
    let expected = program
        .report
        .sections
        .checked_mul(2)
        .and_then(|n| n.checked_add(program.report.paths))
        .and_then(|n| n.checked_add(2))
        .ok_or_else(|| fail("SPAN_COUNT", "Source span count overflow"))?;
    if count != expected || count > (bytes.len() - r.at) / 17 {
        return Err(fail("SPAN_COUNT", "Spans do not cover this source job"));
    }
    let mut spans = Vec::new();
    spans
        .try_reserve_exact(count)
        .map_err(|_| fail("RESOURCE", "Cannot allocate decoded spans"))?;
    for _ in 0..count {
        let phase = match r.tag()? {
            0 => Phase::Header,
            1 => Phase::Transition(r.usize()?),
            2 => Phase::Entry(r.usize()?),
            3 => Phase::Path {
                section: r.usize()?,
                path: r.usize()?,
            },
            4 => Phase::End,
            _ => return Err(fail("TAG", "Invalid phase tag")),
        };
        spans.push(Span {
            phase,
            commands: r.usize()?..r.usize()?,
        });
    }
    if r.at != bytes.len() {
        return Err(fail(
            "TRAILING_DATA",
            "Native bundle contains trailing data",
        ));
    }
    let audit = command_audit::audit(&program, &setup, &commands, &spans, limits)?;
    let prepared = PreparedPlan {
        program,
        setup,
        commands,
        spans,
        audit,
    };
    tool_table::check(tool_table, prepared.program(), prepared.setup())?;
    capabilities::check_prepared(target, &prepared, limits)?;
    Ok((prepared, Identity::of(inputs)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;
    fn legacy(name: &str) -> std::result::Result<Value, Box<dyn std::error::Error>> {
        Ok(crate::json::parse(
            &std::fs::read_to_string(format!("tests-rust/fixtures/legacy/{name}.json"))?,
            &Limits::default(),
        )?)
    }
    fn inputs<'a>(
        f: &'a Value,
        setup: &'a str,
    ) -> std::result::Result<Inputs<'a>, Box<dyn std::error::Error>> {
        Ok(Inputs {
            source: f["text"].as_str().ok_or("source")?,
            setup,
            tool_table: f["toolTable"].as_str(),
            target: None,
        })
    }
    fn offsets(bytes: &[u8]) -> Result<(usize, Vec<usize>, usize)> {
        let mut r = Reader { bytes, at: 200 };
        for _ in 0..4 {
            r.text(true, usize::MAX)?;
        }
        let count_at = r.at;
        let count = r.usize()?;
        let mut records = Vec::new();
        for _ in 0..count {
            records.push(r.at);
            r.record()?;
        }
        Ok((count_at, records, r.at))
    }
    #[test]
    fn complete_legacy_bundles_are_deterministic_and_independently_reaudited() -> TestResult {
        for name in ["mill-mm", "mill-inch", "lathe-mm", "lathe-inch"] {
            let f = legacy(name)?;
            let setup = f["plan"].to_string();
            let input = inputs(&f, &setup)?;
            let first = compile(input, &Limits::default())?;
            let second = compile(input, &Limits::default())?;
            assert_eq!(first.bytes(), second.bytes());
            assert_eq!(first.sha256(), digest(first.bytes()));
            assert_eq!(first.identity(), &Identity::of(input));
            let direct = compiled::prepare(input.source, input.setup, &Limits::default())?;
            assert_eq!(first.prepared().commands(), direct.commands());
            assert_eq!(first.prepared().spans(), direct.spans());
            assert_eq!(
                first.prepared().program().provenance,
                direct.program().provenance
            );
            let reloaded = load(first.bytes().to_vec(), &Limits::default())?;
            assert_eq!(reloaded.prepared().commands(), direct.commands());
            assert!(!reloaded.prepared().audit().execution_authorized);
            let (_, record, _) = offsets(first.bytes())?;
            // Independent literal encoding check: policy site, absent ordinal,
            // ResetModes action. No Rust enum layout/discriminant is serialized.
            assert_eq!(
                &first.bytes()[record[0]..record[0] + 10],
                &[0, 255, 255, 255, 255, 255, 255, 255, 255, 2]
            );
            for (index, r) in first.prepared().commands().iter().enumerate() {
                if let Action::Motion(Motion {
                    geometry:
                        Geometry::Circular {
                            plane, rotation, ..
                        },
                    ..
                }) = r.action
                {
                    assert_eq!(
                        first.bytes()[record[index] + 83],
                        match plane {
                            Plane::Xy => 0,
                            Plane::Xz => 1,
                            Plane::Yz => 2,
                        }
                    );
                    assert_eq!(
                        first.bytes()[record[index] + 84],
                        if rotation == Rotation::Clockwise {
                            0
                        } else {
                            1
                        }
                    );
                }
            }
        }
        Ok(())
    }
    #[test]
    fn identities_bind_exact_inputs_and_optional_snapshot_presence() -> TestResult {
        let f = legacy("lathe-mm")?;
        let setup = f["plan"].to_string();
        let input = inputs(&f, &setup)?;
        let a = compile(input, &Limits::default())?;
        let changed_source = format!("{}\n", input.source);
        let b = compile(
            Inputs {
                source: &changed_source,
                ..input
            },
            &Limits::default(),
        )?;
        assert_ne!(a.identity().source_sha256, b.identity().source_sha256);
        assert_ne!(a.sha256(), b.sha256());
        assert_eq!(a.prepared().commands(), b.prepared().commands());
        let changed_setup = format!("{}\n", input.setup);
        let c = compile(
            Inputs {
                setup: &changed_setup,
                ..input
            },
            &Limits::default(),
        )?;
        assert_ne!(a.identity().setup_sha256, c.identity().setup_sha256);
        let without = compile(
            Inputs {
                tool_table: None,
                ..input
            },
            &Limits::default(),
        )?;
        assert_ne!(a.identity(), without.identity());
        assert_ne!(a.sha256(), without.sha256());
        assert!(compile(
            Inputs {
                tool_table: Some("T9 P9"),
                ..input
            },
            &Limits::default()
        )
        .is_err());
        let target=json!({"schema":"nextnc-native/target-capabilities/1","machine":"lathe","motionContractVersion":2,
            "capabilities":a.prepared().audit().required_capabilities,"evidenceSha256":"0".repeat(64)}).to_string();
        let with_target = compile(
            Inputs {
                target: Some(&target),
                ..input
            },
            &Limits::default(),
        )?;
        assert_eq!(
            with_target.identity().target_sha256,
            Some(digest(target.as_bytes()))
        );
        assert_ne!(a.sha256(), with_target.sha256());
        Ok(())
    }
    #[test]
    fn malformed_truncated_stale_and_resource_exhausting_bundles_never_load() -> TestResult {
        let f = legacy("mill-mm")?;
        let setup = f["plan"].to_string();
        let a = compile(inputs(&f, &setup)?, &Limits::default())?;
        let (count_at, record, spans_at) = offsets(a.bytes())?;
        let mut cuts = vec![
            0,
            7,
            8,
            71,
            72,
            135,
            136,
            199,
            200,
            count_at,
            count_at + 7,
            spans_at,
            spans_at + 7,
            a.bytes().len() - 1,
        ];
        for start in &record {
            cuts.extend([*start, *start + 9]);
        }
        for cut in cuts {
            assert!(
                load(a.bytes()[..cut].to_vec(), &Limits::default()).is_err(),
                "accepted truncated {cut}"
            );
        }
        for at in [0, 8, 72, 136] {
            let mut bad = a.bytes().to_vec();
            bad[at] ^= 1;
            assert!(load(bad, &Limits::default()).is_err(), "header {at}");
        }
        for at in [count_at, spans_at] {
            let mut bad = a.bytes().to_vec();
            bad[at..at + 8].copy_from_slice(&u64::MAX.to_le_bytes());
            assert!(load(bad, &Limits::default()).is_err());
        }
        for at in record {
            for offset in [0, 9] {
                let mut bad = a.bytes().to_vec();
                bad[at + offset] = 255;
                assert!(load(bad, &Limits::default()).is_err(), "tag {at} {offset}");
            }
        }
        let mut trailing = a.bytes().to_vec();
        trailing.push(0);
        assert_eq!(
            load(trailing, &Limits::default())
                .err()
                .ok_or("trailing accepted")?
                .code,
            "TRAILING_DATA"
        );
        let small = Limits {
            bundle_bytes: a.bytes().len() - 1,
            ..Limits::default()
        };
        assert_eq!(
            load(a.bytes().to_vec(), &small)
                .err()
                .ok_or("load bound")?
                .code,
            "BUNDLE_SIZE"
        );
        assert_eq!(
            compile(inputs(&f, &setup)?, &small)
                .err()
                .ok_or("write bound")?
                .code,
            "BUNDLE_SIZE"
        );
        let exact = Limits {
            bundle_bytes: a.bytes().len(),
            ..Limits::default()
        };
        compile(inputs(&f, &setup)?, &exact)?;
        let few = Limits {
            output_commands: 1,
            ..Limits::default()
        };
        assert!(load(a.bytes().to_vec(), &few).is_err());
        Ok(())
    }
    #[test]
    fn serialization_faults_are_rejected_by_semantics_not_just_checksums() -> TestResult {
        let f = legacy("lathe-mm")?;
        let setup = f["plan"].to_string();
        let input = inputs(&f, &setup)?;
        let original = compiled::prepare(input.source, input.setup, &Limits::default())?;
        for index in 0..original.commands.len() {
            let mut corrupted = original.clone();
            corrupted.commands[index].action =
                if corrupted.commands[index].action == Action::ResetModes {
                    Action::ClearTemporaryOffsets
                } else {
                    Action::ResetModes
                };
            // A fully well-formed encoding with a new, valid content digest is
            // still rejected by the independent source/setup/policy checker.
            let bytes = encode(&corrupted, input, &Limits::default())?;
            assert!(
                load(bytes, &Limits::default()).is_err(),
                "corrupt command {index}"
            );
        }
        Ok(())
    }
}
