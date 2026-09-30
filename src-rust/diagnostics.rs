//! Persistent preparation evidence, never admission or restart permission.
//! Reports contain exact observed input identities, bounded excerpts and actual
//! stage outcomes. A later file read cannot replace the identity used by a job.
use crate::{bundle, Diagnostic, Result};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub const REPORT_LIMIT: usize = 16 * 1024 * 1024;
const EXCERPT_LIMIT: usize = 8192;
static NEXT: AtomicU64 = AtomicU64::new(1);
#[derive(Clone, Debug, Serialize)]
pub struct InputEvidence {
    pub role: String,
    pub path: PathBuf,
    pub bytes: Option<usize>,
    pub sha256: Option<String>,
    pub observed: bool,
    #[serde(skip)]
    limit: usize,
}
#[derive(Clone, Debug, Serialize)]
pub struct Stage {
    pub name: String,
    pub status: &'static str,
    pub elapsed_microseconds: Option<u128>,
}
#[derive(Debug)]
pub struct Trace {
    command: String,
    inputs: Vec<InputEvidence>,
    protected_paths: Vec<PathBuf>,
    stages: Vec<Stage>,
    started: Instant,
}
impl Trace {
    pub fn new(command: &str) -> Self {
        let names: &[&str] = match command {
            "check-syntax" => &["arguments", "source-read", "syntax", "shape"],
            "inspect" => &["arguments", "source-read", "syntax", "source-profile"],
            "preflight" => &[
                "arguments",
                "source-read",
                "syntax",
                "source-profile",
                "setup-read",
                "setup-plan",
                "option-inputs",
                "tool-table",
                "target-capabilities",
            ],
            "prepare" => &[
                "arguments",
                "source-read",
                "setup-read",
                "native-compile",
                "option-inputs",
                "tool-table",
                "target-capabilities",
            ],
            "publish" => &[
                "arguments",
                "open-store",
                "input-snapshot",
                "native-bundle",
                "publication",
            ],
            "verify-bundle" => &["arguments", "artifact-read", "bundle-validation"],
            _ => &["arguments"],
        };
        Self {
            command: command.into(),
            inputs: Vec::new(),
            protected_paths: Vec::new(),
            stages: names
                .iter()
                .map(|s| Stage {
                    name: (*s).into(),
                    status: "not_reached",
                    elapsed_microseconds: None,
                })
                .collect(),
            started: Instant::now(),
        }
    }
    pub fn stage<T>(&mut self, name: &str, f: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        let start = Instant::now();
        let result = f(self);
        self.mark(
            name,
            if result.is_ok() { "passed" } else { "failed" },
            Some(start.elapsed().as_micros()),
        );
        result
    }
    pub fn checked_report(
        &mut self,
        name: &str,
        f: impl FnOnce() -> Result<Value>,
    ) -> Result<Value> {
        let result = self.stage(name, |_| f())?;
        if result["status"] == "not_checked" {
            self.mark(name, "not_checked", None);
        }
        Ok(result)
    }
    pub fn mark(&mut self, name: &str, status: &'static str, elapsed: Option<u128>) {
        if let Some(s) = self.stages.iter_mut().find(|s| s.name == name) {
            s.status = status;
            if elapsed.is_some() {
                s.elapsed_microseconds = elapsed;
            }
        }
    }
    pub fn note_input(&mut self, role: &str, path: &Path, limit: usize) -> Result<()> {
        let path = if path.is_absolute() {
            path.to_owned()
        } else {
            std::env::current_dir()
                .map_err(|e| io_error("resolve input path", e))?
                .join(path)
        };
        if !self.protected_paths.contains(&path) {
            self.protected_paths.push(path.clone());
        }
        if let Some(input) = self.inputs.iter_mut().find(|v| v.role == role) {
            input.path = path;
            input.limit = limit;
            input.bytes = None;
            input.sha256 = None;
            input.observed = false;
        } else {
            self.inputs.push(InputEvidence {
                role: role.into(),
                path,
                bytes: None,
                sha256: None,
                observed: false,
                limit,
            });
        }
        Ok(())
    }
    pub fn observed(&mut self, role: &str, bytes: &[u8]) {
        if let Some(v) = self.inputs.iter_mut().find(|v| v.role == role) {
            v.bytes = Some(bytes.len());
            v.sha256 = Some(bundle::digest(bytes));
            v.observed = true;
        }
    }
    pub fn read_bytes(&mut self, role: &str, path: &Path, limit: usize) -> Result<Vec<u8>> {
        self.note_input(role, path, limit)?;
        let bytes = bounded_read(path, limit).map_err(|e| e.with("inputRole", role))?;
        self.observed(role, &bytes);
        Ok(bytes)
    }
    pub fn read_text(&mut self, role: &str, path: &Path, limit: usize) -> Result<String> {
        String::from_utf8(self.read_bytes(role, path, limit)?).map_err(|e| {
            Diagnostic::new("read", "UTF8", e.to_string())
                .with("file", path.display().to_string())
                .with("inputRole", role)
        })
    }
    fn excerpt(&self, error: &Diagnostic) -> Value {
        let role = if let Some(role) = error.context.get("inputRole").and_then(Value::as_str) {
            role
        } else if error.stage == "execution-plan" {
            "setup"
        } else if error.stage == "tool-table" {
            "tool-table"
        } else if error.stage == "target-capabilities" {
            "target"
        } else {
            "source"
        };
        let Some(input) = self.inputs.iter().find(|i| i.role == role && i.observed) else {
            return json!({"status":"unavailable","reason":if self.command=="verify-bundle"{"Source location may refer to embedded bundle input; no outer-file excerpt is inferred"}else{"No exact observed input identity for this error stage"}});
        };
        let line = error
            .source
            .as_ref()
            .map(|s| s.line)
            .or_else(|| {
                error
                    .context
                    .get(if role == "tool-table" {
                        "toolTableLine"
                    } else {
                        "line"
                    })
                    .and_then(Value::as_u64)
                    .and_then(|n| n.try_into().ok())
            })
            .or_else(|| {
                // JSON line numbers describe a complete JSON input only when
                // the caller identified that input; embedded property JSON is
                // located by the outer STEP record instead.
                if matches!(role, "setup" | "target") {
                    error
                        .context
                        .get("jsonLine")
                        .and_then(Value::as_u64)
                        .and_then(|n| n.try_into().ok())
                } else {
                    None
                }
            })
            .or_else(|| {
                error
                    .context
                    .get("provenance")
                    .and_then(|p| p["toolpath"]["sourceLine"].as_u64())
                    .and_then(|n| n.try_into().ok())
            });
        let Some(line) = line.filter(|n| *n > 0) else {
            return json!({"status":"unavailable","reason":"Error has no applicable source line; use its structured context or plan pointers"});
        };
        let bytes = match bounded_read(&input.path, input.limit) {
            Ok(v) => v,
            Err(e) => {
                return json!({"status":"unavailable","reason":"Input could not be reread","detail":e.message})
            }
        };
        if Some(bundle::digest(&bytes)).as_ref() != input.sha256.as_ref() {
            return json!({"status":"unavailable","reason":"Input changed after it was read; changed bytes are not presented as the original source"});
        }
        let text = match std::str::from_utf8(&bytes) {
            Ok(s) => s,
            Err(_) => return json!({"status":"unavailable","reason":"Observed input is not UTF-8"}),
        };
        let first = line.saturating_sub(3).max(1);
        let last = line.saturating_add(3);
        let mut used = 0;
        let mut lines = Vec::new();
        let mut truncated = false;
        for (index, text) in text
            .lines()
            .enumerate()
            .skip(first - 1)
            .take(last - first + 1)
        {
            let remaining = EXCERPT_LIMIT.saturating_sub(used);
            if remaining == 0 {
                truncated = true;
                break;
            }
            let mut end = text.len().min(remaining);
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            truncated |= end < text.len();
            used += end;
            lines.push(json!({"line":index+1,"text":&text[..end]}));
        }
        if lines.is_empty() {
            return json!({"status":"unavailable","reason":"Reported line lies outside the observed input"});
        }
        json!({"status":"verified","role":role,"path":input.path,"sha256":input.sha256,"requestedLine":line,"lines":lines,"truncated":truncated})
    }
    fn record(
        &self,
        outcome: &Value,
        error: Option<&Diagnostic>,
        stamp: Duration,
        id: &str,
    ) -> Value {
        let summary = if let Some(fields) = outcome.as_object() {
            Value::Object(
                fields
                    .iter()
                    .map(|(key, value)| {
                        (
                            key.clone(),
                            if key == "program" {
                                value["report"].clone()
                            } else {
                                value.clone()
                            },
                        )
                    })
                    .collect(),
            )
        } else {
            outcome.clone()
        };
        json!({"schema":"linuxcnc-next-nc/native-diagnostic/1","reportId":id,"timeUTC":utc(stamp),"timeUnixNanoseconds":stamp.as_nanos().to_string(),
            "compilerVersion":env!("CARGO_PKG_VERSION"),"compilerSHA256":bundle::COMPILER_SHA256,"schemaSHA256":bundle::schema_sha256(),"policySHA256":bundle::policy_sha256(),
            "command":self.command,"status":if error.is_some(){"failed"}else{"passed"},"elapsedMicroseconds":self.started.elapsed().as_micros(),
            "inputs":self.inputs,"validationStages":self.stages,"error":error,"correction":error.map(correction),"sourceExcerpt":error.map(|e|self.excerpt(e)),
            "result":summary,"reportScope":"Preparation evidence; source model and full command arrays are not duplicated in this archive",
            "executionAuthorized":false,"liveControllerState":"not_checked"})
    }
}
fn bounded_read(path: &Path, limit: usize) -> Result<Vec<u8>> {
    crate::fileio::read_bytes(path, limit, true)
}
fn io_error(action: &str, e: std::io::Error) -> Diagnostic {
    Diagnostic::new("diagnostic-archive", "IO", e.to_string()).with("action", action)
}
fn problem(code: &str, message: &str) -> Diagnostic {
    Diagnostic::new("diagnostic-archive", code, message)
}

/// The existing translator's directory convention, with no global-plan fallback.
pub fn directory() -> Result<PathBuf> {
    directory_from(std::env::vars_os().collect(), cfg!(windows))
}
fn directory_from(env: BTreeMap<OsString, OsString>, windows: bool) -> Result<PathBuf> {
    let get = |name: &str| {
        env.get(&OsString::from(name))
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    if let Some(path) = get("NEXTNC_DIAGNOSTICS") {
        return if path.is_absolute() {
            Ok(path)
        } else {
            Ok(std::env::current_dir()
                .map_err(|e| io_error("resolve diagnostics override", e))?
                .join(path))
        };
    }
    let root = if windows {
        get("LOCALAPPDATA").or_else(|| get("USERPROFILE").map(|p| p.join("AppData").join("Local")))
    } else {
        get("XDG_STATE_HOME").or_else(|| get("HOME").map(|p| p.join(".local").join("state")))
    };
    let root = root.ok_or_else(|| {
        problem(
            "DIRECTORY_UNAVAILABLE",
            "No platform state directory is available; set NEXTNC_DIAGNOSTICS explicitly",
        )
    })?;
    if !root.is_absolute() {
        return Err(problem("DIRECTORY_RELATIVE","Platform state/home paths must be absolute; use NEXTNC_DIAGNOSTICS for an explicit relative override"));
    }
    Ok(root.join("LinuxCNCNext-NC").join("diagnostics"))
}
pub fn correction(error: &Diagnostic) -> &'static str {
    if error.code == "UNSUPPORTED_CAPABILITIES" {
        "Use a target with qualified support for every listed capability, or regenerate the affected CAM operation. Do not replace synchronized feed with a nominal-RPM conversion."
    } else if error.code.starts_with("TOOL_TABLE") || error.stage == "tool-table" {
        "Check the named T/H records in the selected LinuxCNC tool-table snapshot and the reviewed plan mapping. File validation does not verify a physically fitted tool."
    } else if error.stage == "execution-plan" {
        "Correct the listed reviewed-plan fields or JSON pointers, retaining intended tools, work offsets and ordered approach/retract paths. Re-run preparation."
    } else if error.stage == "geometry"
        || error.code.contains("GEOMETRY")
        || error.code == "PATH_CONTINUITY"
    {
        "Review the named source operation and geometry, regenerate its Fusion toolpath and post again. Changing setup mappings cannot repair source geometry."
    } else if error.code == "SOURCE_CHANGED" {
        "Prepare a fresh snapshot of the intended source and reviewed setup. Do not select a result from the obsolete request."
    } else if error.code == "CORRUPT_BUNDLE" || error.code == "IDENTITY" {
        "Re-prepare from the intended source/setup using this compiler and policy. A stored checksum or old selection is not permission to run."
    } else if error.code.contains("LIMIT") || error.code.contains("SIZE") {
        "Inspect the recorded resource limit and job expansion. Do not bypass it or execute a validated prefix; the complete job must fit supported preparation bounds."
    } else if error.stage == "native-command-audit" {
        "Preserve this report and exact input identities when reporting the compiler inconsistency. Do not alter machining geometry merely to bypass the audit."
    } else if error.code == "USAGE" {
        "Use the command syntax in the error message. Source and reviewed setup are explicit inputs; there is no global-plan fallback."
    } else {
        "Use the error's source record, byte location and structured context to correct the named input, then re-run preparation. This report does not authorize machine execution."
    }
}
fn utc(stamp: Duration) -> Option<String> {
    let seconds = stamp.as_secs();
    if seconds > 253_402_300_799 {
        return None;
    }
    let mut days = seconds / 86400;
    let mut year = 1970u64;
    let leap = |y: u64| y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400));
    loop {
        let length = 365 + u64::from(leap(year));
        if days < length {
            break;
        }
        days -= length;
        year += 1;
    }
    let mut month = 1;
    for length in [
        31,
        if leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ] {
        if days < length {
            break;
        }
        days -= length;
        month += 1;
    }
    Some(format!(
        "{year:04}-{month:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        days + 1,
        (seconds / 3600) % 24,
        (seconds / 60) % 60,
        seconds % 60,
        stamp.subsec_millis()
    ))
}
fn visible(s: &str) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if (c.is_control() && c != '\n' && c != '\t')
            || matches!(c,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}'|'\u{200e}'|'\u{200f}')
        {
            let _ = write!(out, "\\u{{{:04x}}}", c as u32);
        } else {
            out.push(c);
        }
    }
    out
}
fn human(record: &Value) -> Result<String> {
    let mut out=format!("Next-NC native preparation report\nReport: {}\nUTC: {}\nStatus: {}\nCommand: {}\nCompiler: {} / {}\n\n",record["reportId"],record["timeUTC"],record["status"],record["command"],record["compilerVersion"],record["compilerSHA256"]);
    if let Some(inputs) = record["inputs"].as_array() {
        for i in inputs {
            out.push_str(&format!(
                "Input {}: {}\n  Exact observed SHA-256: {}\n  Observed bytes: {}\n",
                i["role"], i["path"], i["sha256"], i["bytes"]
            ));
        }
    }
    if !record["error"].is_null() {
        out.push_str(&format!(
            "\nERROR\n{}\n\nCorrection\n{}\n\nSource excerpt\n{}\n",
            serde_json::to_string_pretty(&record["error"])
                .map_err(|e| problem("SERIALIZATION", &e.to_string()))?,
            record["correction"],
            serde_json::to_string_pretty(&record["sourceExcerpt"])
                .map_err(|e| problem("SERIALIZATION", &e.to_string()))?
        ));
    }
    out.push_str(&format!("\nValidation stages (not_reached is not a pass)\n{}\n\nResult summary\n{}\n\nNo execution permission or live controller binding is recorded.\n",record["validationStages"],serde_json::to_string_pretty(&record["result"]).map_err(|e|problem("SERIALIZATION",&e.to_string()))?));
    let out = visible(&out);
    if out.len() > REPORT_LIMIT {
        return Err(problem(
            "REPORT_SIZE",
            "Readable diagnostic exceeds report limit",
        ));
    }
    Ok(out)
}
struct Bounded {
    bytes: Vec<u8>,
}
impl Write for Bounded {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > REPORT_LIMIT.saturating_sub(self.bytes.len()) {
            return Err(std::io::Error::other(
                "Diagnostic report exceeds byte limit",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn exclusive(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut f = options
        .open(path)
        .map_err(|e| io_error("create diagnostic stage", e))?;
    f.write_all(bytes)
        .and_then(|_| f.sync_all())
        .map_err(|e| io_error("write/sync diagnostic stage", e))
}
fn replace(dir: &Path, name: &str, id: &str, bytes: &[u8]) -> Result<PathBuf> {
    let path = dir.join(name);
    if fs::symlink_metadata(&path).is_ok_and(|m| !m.is_file() || m.file_type().is_symlink()) {
        return Err(problem(
            "INDEX_TYPE",
            "Diagnostic index is not an ordinary file",
        ));
    }
    let stage = dir.join(format!(".{name}.{id}.tmp"));
    exclusive(&stage, bytes)?;
    fs::rename(stage, &path).map_err(|e| io_error("publish diagnostic index", e))?;
    Ok(path)
}
fn overlaps(a: &Path, b: &Path) -> bool {
    let a = fs::canonicalize(a).unwrap_or_else(|_| a.to_owned());
    let b = fs::canonicalize(b).unwrap_or_else(|_| b.to_owned());
    #[cfg(windows)]
    {
        a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
    }
    #[cfg(not(windows))]
    {
        a == b
    }
}
/// Archiving is best effort and is explicitly reported separately from the job
/// result. It must never suppress the original diagnostic or turn failure into success.
pub fn archive(trace: &Trace, outcome: &Value, error: Option<&Diagnostic>) -> Value {
    match directory() {
        Ok(dir) => save(&dir, trace, outcome, error),
        Err(e) => json!({"status":"unavailable","error":e}),
    }
}
pub fn save(dir: &Path, trace: &Trace, outcome: &Value, error: Option<&Diagnostic>) -> Value {
    let mut published_json = None;
    let mut published_text = None;
    let result = (|| -> Result<Value> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| problem("CLOCK", "System clock precedes Unix epoch"))?;
        let id = format!(
            "native-{:032x}-{:08x}-{:016x}",
            stamp.as_nanos(),
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let record = trace.record(outcome, error, stamp, &id);
        let mut buffer = Bounded { bytes: Vec::new() };
        serde_json::to_writer_pretty(&mut buffer, &record)
            .map_err(|e| problem("REPORT_SIZE", &e.to_string()))?;
        let text = human(&record)?;
        fs::create_dir_all(dir).map_err(|e| io_error("create diagnostics directory", e))?;
        let metadata =
            fs::symlink_metadata(dir).map_err(|e| io_error("inspect diagnostics directory", e))?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(problem(
                "DIRECTORY_TYPE",
                "Diagnostics directory must be an ordinary directory",
            ));
        }
        let dir =
            fs::canonicalize(dir).map_err(|e| io_error("resolve diagnostics directory", e))?;
        let json_path = dir.join(format!("{id}.json"));
        let text_path = dir.join(format!("{id}.txt"));
        let json_stage = dir.join(format!(".{id}.json.tmp"));
        exclusive(&json_stage, &buffer.bytes)?;
        // Never replace an immutable record, including the unlikely ID collision.
        fs::hard_link(&json_stage, &json_path)
            .map_err(|e| io_error("publish exclusive diagnostic record", e))?;
        published_json = Some(json_path.clone());
        fs::remove_file(json_stage).map_err(|e| io_error("remove diagnostic stage", e))?;
        let text_stage = dir.join(format!(".{id}.txt.tmp"));
        exclusive(&text_stage, text.as_bytes())?;
        fs::hard_link(&text_stage, &text_path)
            .map_err(|e| io_error("publish exclusive readable record", e))?;
        published_text = Some(text_path.clone());
        fs::remove_file(text_stage).map_err(|e| io_error("remove readable stage", e))?;
        let lock_path = dir.join("native-latest.lock");
        if trace
            .protected_paths
            .iter()
            .any(|p| overlaps(p, &lock_path))
        {
            return Err(problem(
                "INPUT_COLLISION",
                "Diagnostics lock would overlap an input",
            ));
        }
        if fs::symlink_metadata(&lock_path)
            .is_ok_and(|m| !m.is_file() || m.file_type().is_symlink())
        {
            return Err(problem(
                "INDEX_TYPE",
                "Diagnostics lock is not an ordinary file",
            ));
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)
            .map_err(|e| io_error("open diagnostic index lock", e))?;
        let mut locked = false;
        for _ in 0..50 {
            match lock.try_lock() {
                Ok(()) => {
                    locked = true;
                    break;
                }
                Err(std::fs::TryLockError::WouldBlock) => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                Err(e) => return Err(problem("INDEX_LOCK", &e.to_string())),
            }
        }
        if !locked {
            return Ok(
                json!({"status":"saved","reportId":id,"json":json_path,"text":text_path,"latestUpdated":false,"reason":"Latest-index writer is busy; immutable report is complete"}),
            );
        }
        let names = if error.is_some() {
            vec![
                "latest.json",
                "latest.txt",
                "latest-error.json",
                "latest-error.txt",
            ]
        } else {
            vec!["latest.json", "latest.txt"]
        };
        for name in &names {
            let path = dir.join(name);
            if trace.protected_paths.iter().any(|p| overlaps(p, &path)) {
                return Err(problem(
                    "INPUT_COLLISION",
                    "A latest diagnostic index would overwrite an input",
                ));
            }
        }
        for name in names {
            replace(
                &dir,
                name,
                &id,
                if name.ends_with("json") {
                    &buffer.bytes
                } else {
                    text.as_bytes()
                },
            )?;
        }
        #[cfg(unix)]
        {
            fs::File::open(&dir)
                .and_then(|f| f.sync_all())
                .map_err(|e| io_error("sync diagnostic directory", e))?;
        }
        Ok(
            json!({"status":"saved","reportId":id,"json":json_path,"text":text_path,"latestUpdated":true,"latest":dir.join("latest.json"),"latestError":if error.is_some(){Some(dir.join("latest-error.txt"))}else{None}}),
        )
    })();
    match result {
        Ok(v) => v,
        Err(e) => {
            json!({"status":if published_json.is_some(){"partial"}else{"unavailable"},"json":published_json,"text":published_text,"latestUpdated":false,"error":e})
        }
    }
}

#[cfg(test)]
#[path = "../tests-rust/diagnostics.rs"]
mod tests;
