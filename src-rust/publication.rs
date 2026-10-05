//! Single-owner, headless preparation/selection lifecycle. Selection is inspection
//! state, never controller start authority. An owner restart restores no selection.
//! Worker tickets are process-local capabilities tied to that owner's lifetime.
use crate::{
    bundle::{self, Artifact, Identity, Inputs},
    part21::Limits,
    Diagnostic, Result,
};
use serde::Serialize;
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};

static UNIQUE: AtomicU64 = AtomicU64::new(1);
#[derive(Clone, Debug)]
pub struct Paths {
    pub source: PathBuf,
    pub setup: PathBuf,
    pub tool_table: Option<PathBuf>,
    pub target: Option<PathBuf>,
}
#[derive(Debug)]
pub struct Snapshot {
    source: String,
    setup: String,
    tool_table: Option<String>,
    target: Option<String>,
}
impl Snapshot {
    pub fn inputs(&self) -> Inputs<'_> {
        Inputs {
            source: &self.source,
            setup: &self.setup,
            tool_table: self.tool_table.as_deref(),
            target: self.target.as_deref(),
        }
    }
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Generation {
    pub session: String,
    pub request: u64,
}
#[derive(Clone, Debug)]
enum InputSource {
    Files(Paths),
    Bundle(String),
}
#[derive(Clone, Debug)]
pub struct Ticket {
    owner: Arc<()>,
    generation: Generation,
    input: InputSource,
    identity: Identity,
}
impl Ticket {
    pub fn generation(&self) -> &Generation {
        &self.generation
    }
    pub fn identity(&self) -> &Identity {
        &self.identity
    }
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Selection {
    pub generation: Generation,
    pub artifact_sha256: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Point {
    StagePartial,
    AfterStageSync,
    BeforeObjectRename,
    AfterObjectRename,
    StatePartial,
    AfterStateSync,
}
pub struct Store {
    root: PathBuf,
    _lock: File,
    owner: Arc<()>,
    session: String,
    request: u64,
    limits: Limits,
    pending: Option<Ticket>,
    selected: Option<(Ticket, Selection)>,
    #[cfg(test)]
    fault: Option<Point>,
    #[cfg(test)]
    crash: bool,
}
impl Drop for Store {
    fn drop(&mut self) {
        // Another thread may fork while this store is open. Closing our File
        // alone then leaves its lock held by the child's inherited descriptor
        // until exec, spuriously denying a replacement owner in this process.
        // Ownership ends with Store, not with every copy of that descriptor.
        // On an OS unlock error File still closes; a retained lock fails closed.
        let _ = self._lock.unlock();
    }
}
fn fail(code: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("native-publication", code, message)
}
fn io_error(action: &str, e: std::io::Error) -> Diagnostic {
    fail("IO", e.to_string()).with("action", action)
}
fn directory(path: &Path) -> Result<()> {
    fs::create_dir_all(path).map_err(|e| io_error("create private artifact directory", e))?;
    let m = fs::symlink_metadata(path).map_err(|e| io_error("inspect artifact directory", e))?;
    if !m.is_dir() || m.file_type().is_symlink() {
        return Err(fail(
            "DIRECTORY",
            "Artifact directories must be ordinary directories",
        ));
    }
    Ok(())
}
fn ordinary(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.is_file() && !m.file_type().is_symlink() => Ok(()),
        Ok(_) => Err(fail("FILE_TYPE", "Artifact path is not an ordinary file")
            .with("file", path.display().to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(io_error("inspect artifact file", e)),
    }
}
fn new_file(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).read(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(path)
        .map_err(|e| io_error("create exclusive staged file", e))
}
fn within(path: &Path, root: &Path) -> bool {
    #[cfg(windows)]
    {
        let p = path.to_string_lossy().to_lowercase();
        let r = root.to_string_lossy().to_lowercase();
        p == r || p.starts_with(&(r + "\\"))
    }
    #[cfg(not(windows))]
    {
        path.starts_with(root)
    }
}
fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        File::open(path)
            .and_then(|f| f.sync_all())
            .map_err(|e| io_error("sync artifact directory", e))?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    // On Windows file contents are explicitly flushed. We do not claim a POSIX
    // directory fsync. Restart disarms and every candidate is reverified anyway.
    Ok(())
}
fn read_bytes(path: &Path, limit: usize) -> Result<Vec<u8>> {
    ordinary(path)?;
    crate::fileio::read_bytes(path, limit, false).map_err(|mut e| {
        e.stage = "native-publication".into();
        e
    })
}
fn snapshot(paths: &Paths, limits: &Limits) -> Result<Snapshot> {
    fn stable(path: &Path, limit: usize) -> Result<String> {
        let bytes = read_bytes(path, limit)?;
        if bytes != read_bytes(path, limit)? {
            return Err(fail(
                "SOURCE_CHANGED",
                "Input changed while taking the preparation snapshot",
            )
            .with("file", path.display().to_string()));
        }
        String::from_utf8(bytes).map_err(|_| {
            fail("UTF8", "Preparation inputs must be UTF-8")
                .with("file", path.display().to_string())
        })
    }
    Ok(Snapshot {
        source: stable(&paths.source, limits.input_bytes)?,
        setup: stable(&paths.setup, limits.input_bytes)?,
        tool_table: paths
            .tool_table
            .as_deref()
            .map(|p| stable(p, 1024 * 1024))
            .transpose()?,
        target: paths
            .target
            .as_deref()
            .map(|p| stable(p, 1024 * 1024))
            .transpose()?,
    })
}
impl Store {
    /// Exclusively owns this store until dropped. OS locks release on process
    /// death; a leftover lock file is not evidence that its owner is still alive.
    pub fn open(root: &Path, limits: Limits) -> Result<Self> {
        limits.check()?;
        directory(root)?;
        let root = fs::canonicalize(root).map_err(|e| io_error("resolve artifact root", e))?;
        let format_path = root.join("store-format");
        let format = b"nextnc-native/artifact-store/1\n";
        if !format_path
            .try_exists()
            .map_err(|e| io_error("inspect store format", e))?
        {
            for entry in fs::read_dir(&root).map_err(|e| io_error("inspect new store", e))? {
                let entry = entry.map_err(|e| io_error("inspect new store entry", e))?;
                if entry.file_name() != "owner.lock" {
                    return Err(fail("STORE_FORMAT","Use an empty directory or an existing native artifact store; unrelated files are not overwritten"));
                }
            }
        }
        let lock_path = root.join("owner.lock");
        ordinary(&lock_path)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|e| io_error("open owner lock", e))?;
        lock.try_lock().map_err(|e| {
            fail(
                "OWNER_BUSY",
                format!("Artifact store cannot obtain exclusive ownership: {e}"),
            )
        })?;
        if format_path
            .try_exists()
            .map_err(|e| io_error("inspect store format", e))?
        {
            if read_bytes(&format_path, 128)? != format {
                return Err(fail(
                    "STORE_FORMAT",
                    "Unknown or incomplete native artifact store format",
                ));
            }
        } else {
            let mut file = new_file(&format_path)?;
            file.write_all(format)
                .and_then(|_| file.sync_all())
                .map_err(|e| io_error("initialize store format", e))?;
            sync_directory(&root)?;
        }
        for sub in ["objects", "staging", "sessions"] {
            directory(&root.join(sub))?;
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| fail("CLOCK", "System clock precedes epoch"))?
            .as_nanos();
        let session = loop {
            let serial = UNIQUE.fetch_add(1, Ordering::Relaxed);
            let name = format!("{now:032x}-{:08x}-{serial:016x}", std::process::id());
            let path = root.join("sessions").join(&name);
            match OpenOptions::new().write(true).create_new(true).open(path) {
                Ok(f) => {
                    f.sync_all()
                        .map_err(|e| io_error("record owner session", e))?;
                    break name;
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(io_error("record owner session", e)),
            }
        };
        sync_directory(&root.join("sessions"))?;
        Ok(Self {
            root,
            _lock: lock,
            owner: Arc::new(()),
            session,
            request: 0,
            limits,
            pending: None,
            selected: None,
            #[cfg(test)]
            fault: None,
            #[cfg(test)]
            crash: false,
        })
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    /// Disarm FIRST, even if the disk is full or the new source cannot be read.
    /// All workers return their ticket to this owner; they never select directly.
    pub fn begin(&mut self, paths: Paths) -> Result<(Ticket, Snapshot)> {
        let generation = self.next_generation()?;
        let cwd = std::env::current_dir().map_err(|e| io_error("resolve input paths", e))?;
        let absolute = |p: PathBuf| if p.is_absolute() { p } else { cwd.join(p) };
        let paths = Paths {
            source: absolute(paths.source),
            setup: absolute(paths.setup),
            tool_table: paths.tool_table.map(absolute),
            target: paths.target.map(absolute),
        };
        for path in [
            Some(&paths.source),
            Some(&paths.setup),
            paths.tool_table.as_ref(),
            paths.target.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            ordinary(path)?;
            let resolved =
                fs::canonicalize(path).map_err(|e| io_error("resolve preparation input", e))?;
            if within(&resolved, &self.root) {
                return Err(fail(
                    "INPUT_STORE_OVERLAP",
                    "Preparation inputs must be outside the artifact store",
                ));
            }
        }
        self.state("preparing", &generation, None, None)?;
        let captured = snapshot(&paths, &self.limits)?;
        let ticket = Ticket {
            owner: Arc::clone(&self.owner),
            generation,
            input: InputSource::Files(paths),
            identity: Identity::of(captured.inputs()),
        };
        self.pending = Some(ticket.clone());
        Ok((ticket, captured))
    }
    fn next_generation(&mut self) -> Result<Generation> {
        self.pending = None;
        self.selected = None;
        self.request = self
            .request
            .checked_add(1)
            .ok_or_else(|| fail("GENERATION", "Selection generation exhausted"))?;
        Ok(Generation {
            session: self.session.clone(),
            request: self.request,
        })
    }
    /// Explicit selection of a published immutable job, independent of its old
    /// project paths/compiler process. Embedded inputs are revalidated; live
    /// tool data and controller state are still NOT bound or authorized here.
    pub fn begin_bundle(&mut self, sha256: &str) -> Result<(Ticket, Artifact)> {
        let generation = self.next_generation()?;
        self.state("preparing", &generation, None, None)?;
        let artifact = self.read_artifact(&self.object_path(sha256)?, sha256)?;
        let ticket = Ticket {
            owner: Arc::clone(&self.owner),
            generation,
            input: InputSource::Bundle(sha256.into()),
            identity: artifact.identity().clone(),
        };
        self.pending = Some(ticket.clone());
        Ok((ticket, artifact))
    }
    fn current(&self, ticket: &Ticket) -> bool {
        Arc::ptr_eq(&ticket.owner, &self.owner)
            && self
                .pending
                .as_ref()
                .is_some_and(|p| p.generation == ticket.generation && p.identity == ticket.identity)
    }
    fn check_snapshot(&self, ticket: &Ticket) -> Result<()> {
        let identity = match &ticket.input {
            InputSource::Files(paths) => Identity::of(snapshot(paths, &self.limits)?.inputs()),
            InputSource::Bundle(sha) => self
                .read_artifact(&self.object_path(sha)?, sha)?
                .identity()
                .clone(),
        };
        if identity != ticket.identity {
            return Err(fail(
                "SOURCE_CHANGED",
                "Source, setup or optional snapshot changed after preparation began",
            ));
        }
        Ok(())
    }
    pub fn cancel(&mut self, ticket: &Ticket) -> Result<()> {
        let selected = Arc::ptr_eq(&ticket.owner, &self.owner)
            && self
                .selected
                .as_ref()
                .is_some_and(|(t, _)| t.generation == ticket.generation);
        if !self.current(ticket) && !selected {
            return Err(fail(
                "STALE_WORKER",
                "Cancellation belongs to an obsolete owner/request",
            ));
        }
        self.pending = None;
        self.selected = None;
        self.state("cancelled", &ticket.generation, None, None)
    }
    pub fn reject(&mut self, ticket: &Ticket, error: &Diagnostic) -> Result<()> {
        if !self.current(ticket) {
            return Err(fail(
                "STALE_WORKER",
                "Failure belongs to an obsolete owner/request",
            ));
        }
        self.pending = None;
        self.selected = None;
        self.state("failed", &ticket.generation, None, Some(&error.code))
    }
    pub fn commit(&mut self, ticket: &Ticket, artifact: &Artifact) -> Result<Selection> {
        if !self.current(ticket) {
            return Err(fail(
                "STALE_WORKER",
                "Obsolete worker cannot select or disarm a newer request",
            ));
        }
        let result = self.commit_current(ticket, artifact);
        if let Err(ref error) = result {
            self.pending = None;
            self.selected = None;
            // Original failure wins. If storage itself failed, memory remains
            // disarmed regardless of whether the inspection journal can update.
            let _ = self.state("failed", &ticket.generation, None, Some(&error.code));
        }
        result
    }
    fn commit_current(&mut self, ticket: &Ticket, artifact: &Artifact) -> Result<Selection> {
        if artifact.bytes().len() > self.limits.bundle_bytes
            || artifact.prepared().commands().len() > self.limits.output_commands
        {
            return Err(fail(
                "RESOURCE_LIMIT",
                "Candidate exceeds this store's byte/command limits",
            ));
        }
        if artifact.identity() != ticket.identity() {
            return Err(fail(
                "IDENTITY",
                "Worker result does not match this request's exact input/compiler/policy snapshot",
            ));
        }
        if let InputSource::Bundle(expected) = &ticket.input {
            if artifact.sha256() != expected {
                return Err(fail(
                    "IDENTITY",
                    "Worker replaced the explicitly chosen immutable bundle",
                ));
            }
        }
        self.check_snapshot(ticket)?;
        let stage = self.stage_path("bundle");
        let mut file = new_file(&stage)?;
        // Test injection uses a real partial file write and I/O failure. It is
        // unreachable in production builds and cannot be enabled by a job file.
        #[cfg(test)]
        if self.fault == Some(Point::StagePartial) {
            file.write_all(&artifact.bytes()[..artifact.bytes().len().min(7)])
                .map_err(|e| io_error("write injected partial stage", e))?;
        }
        self.checkpoint(Point::StagePartial)?;
        file.write_all(artifact.bytes())
            .and_then(|_| file.sync_all())
            .map_err(|e| io_error("write/sync candidate bundle", e))?;
        drop(file);
        self.checkpoint(Point::AfterStageSync)?;
        let checked = self.read_artifact(&stage, artifact.sha256())?;
        if checked.identity() != ticket.identity() {
            return Err(fail(
                "IDENTITY",
                "Staged bundle identity differs from request",
            ));
        }
        drop(checked);
        self.check_snapshot(ticket)?;
        let object = self.object_path(artifact.sha256())?;
        self.checkpoint(Point::BeforeObjectRename)?;
        if object
            .try_exists()
            .map_err(|e| io_error("inspect existing immutable object", e))?
        {
            self.read_artifact(&object, artifact.sha256())?;
            fs::remove_file(&stage).map_err(|e| io_error("remove redundant private stage", e))?;
        } else {
            fs::rename(&stage, &object)
                .map_err(|e| io_error("publish complete immutable object", e))?;
        }
        sync_directory(&self.root.join("objects"))?;
        self.checkpoint(Point::AfterObjectRename)?;
        // Recheck source after staging and again before selection publication.
        self.check_snapshot(ticket)?;
        self.state(
            "selected",
            &ticket.generation,
            Some(artifact.sha256()),
            None,
        )?;
        let selection = Selection {
            generation: ticket.generation.clone(),
            artifact_sha256: artifact.sha256().into(),
        };
        self.selected = Some((ticket.clone(), selection.clone()));
        self.pending = None;
        Ok(selection)
    }
    /// Explicitly reselect a verified cached candidate under a NEW begin ticket.
    /// Nothing on disk restores an earlier owner, generation or run permission.
    pub fn select_existing(&mut self, ticket: &Ticket, sha256: &str) -> Result<Selection> {
        if !self.current(ticket) {
            return Err(fail("STALE_WORKER", "Cached selection request is stale"));
        }
        let artifact = self
            .object_path(sha256)
            .and_then(|p| self.read_artifact(&p, sha256));
        match artifact {
            Ok(a) => self.commit(ticket, &a),
            Err(e) => {
                let _ = self.reject(ticket, &e);
                Err(e)
            }
        }
    }
    /// Fresh verification for inspection/admission preparation, never a start API.
    /// A changed source, corrupt object or storage error disarms this selection.
    pub fn selected(&mut self) -> Result<Option<(Selection, Artifact)>> {
        let Some((ticket, selection)) = self.selected.clone() else {
            return Ok(None);
        };
        let result = (|| {
            self.check_snapshot(&ticket)?;
            let a = self.read_artifact(
                &self.object_path(&selection.artifact_sha256)?,
                &selection.artifact_sha256,
            )?;
            if a.identity() != ticket.identity() {
                return Err(fail("IDENTITY", "Selected artifact changed identity"));
            }
            Ok(a)
        })();
        match result {
            Ok(a) => Ok(Some((selection, a))),
            Err(e) => {
                self.selected = None;
                let _ = self.state("invalidated", &ticket.generation, None, Some(&e.code));
                Err(e)
            }
        }
    }
    fn object_path(&self, sha: &str) -> Result<PathBuf> {
        if sha.len() != 64
            || !sha
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(fail("HASH", "Object identity must be lowercase SHA-256"));
        }
        Ok(self.root.join("objects").join(format!("{sha}.nncb")))
    }
    fn read_artifact(&self, path: &Path, sha: &str) -> Result<Artifact> {
        let bytes = read_bytes(path, self.limits.bundle_bytes)?;
        if bundle::digest(&bytes) != sha {
            return Err(fail(
                "CORRUPT_BUNDLE",
                "Stored bytes differ from their content identity",
            ));
        }
        bundle::load(bytes, &self.limits)
    }
    fn stage_path(&self, label: &str) -> PathBuf {
        self.root.join("staging").join(format!(
            "{}-{}-{label}-{}.tmp",
            self.session,
            self.request,
            UNIQUE.fetch_add(1, Ordering::Relaxed)
        ))
    }
    fn state(
        &mut self,
        status: &str,
        generation: &Generation,
        artifact: Option<&str>,
        error: Option<&str>,
    ) -> Result<()> {
        let value = serde_json::json!({"schema":"nextnc-native/selection-journal/1","status":status,"generation":generation,
            "artifactSHA256":artifact,"error":error,"executionAuthorized":false,"restartRestoresSelection":false});
        let path = self.stage_path("journal");
        let mut file = new_file(&path)?;
        #[cfg(test)]
        if self.fault == Some(Point::StatePartial) {
            file.write_all(b"{")
                .map_err(|e| io_error("write injected partial journal", e))?;
        }
        self.checkpoint(Point::StatePartial)?;
        file.write_all(value.to_string().as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|e| io_error("write/sync selection journal", e))?;
        drop(file);
        self.checkpoint(Point::AfterStateSync)?;
        let dest = self.root.join("selection.json");
        ordinary(&dest)?;
        fs::rename(path, dest).map_err(|e| io_error("publish selection journal", e))?;
        sync_directory(&self.root)
    }
    fn checkpoint(&mut self, point: Point) -> Result<()> {
        #[cfg(test)]
        if self.fault == Some(point) {
            self.fault = None;
            if self.crash {
                std::process::exit(86);
            }
            return Err(fail(
                "IO",
                "Injected full-storage/interrupted-publication failure",
            )
            .with("point", format!("{point:?}")));
        }
        let _ = point;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> std::result::Result<Self, Box<dyn std::error::Error>> {
            let p = std::env::temp_dir().join(format!(
                "nextnc-publication-{}-{}-{}",
                std::process::id(),
                SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos(),
                UNIQUE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&p)?;
            Ok(Self(fs::canonicalize(p)?))
        }
        fn paths(&self) -> std::result::Result<Paths, Box<dyn std::error::Error>> {
            let f = crate::json::parse(
                &fs::read_to_string("tests-rust/fixtures/legacy/lathe-mm.json")?,
                &Limits::default(),
            )?;
            let paths = Paths {
                source: self.0.join("source.stpnc"),
                setup: self.0.join("setup.json"),
                tool_table: Some(self.0.join("tool.tbl")),
                target: None,
            };
            fs::write(&paths.source, f["text"].as_str().ok_or("fixture")?)?;
            fs::write(&paths.setup, f["plan"].to_string())?;
            fs::write(
                paths.tool_table.as_ref().ok_or("table")?,
                f["toolTable"].as_str().ok_or("table fixture")?,
            )?;
            Ok(paths)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            // Delete only this test's verified, direct child of the OS temp root.
            if let (Ok(root), Ok(path)) = (
                fs::canonicalize(std::env::temp_dir()),
                fs::canonicalize(&self.0),
            ) {
                if path.parent() == Some(root.as_path())
                    && path
                        .file_name()
                        .is_some_and(|n| n.to_string_lossy().starts_with("nextnc-publication-"))
                {
                    let _ = fs::remove_dir_all(path);
                }
            }
        }
    }
    fn prepared(
        store: &mut Store,
        paths: Paths,
    ) -> std::result::Result<(Ticket, Artifact), Box<dyn std::error::Error>> {
        let (t, s) = store.begin(paths)?;
        let a = bundle::compile(s.inputs(), &store.limits)?;
        Ok((t, a))
    }
    #[test]
    fn single_owner_epochs_and_stale_workers_cannot_restore_old_selection() -> TestResult {
        let temp = Temp::new()?;
        let paths = temp.paths()?;
        let root = temp.0.join("store");
        let mut store = Store::open(&root, Limits::default())?;
        assert!(
            Store::open(&root, Limits::default()).is_err(),
            "two owners accepted"
        );
        let (old, a) = prepared(&mut store, paths.clone())?;
        store.commit(&old, &a)?;
        assert!(store.selected()?.is_some());
        let (new, b) = prepared(&mut store, paths.clone())?;
        assert!(store.selected()?.is_none());
        assert_eq!(
            store.commit(&old, &a).err().ok_or("stale accepted")?.code,
            "STALE_WORKER"
        );
        store.commit(&new, &b)?;
        assert!(store.selected()?.is_some());
        assert!(store.cancel(&old).is_err());
        assert!(
            store.selected()?.is_some(),
            "obsolete cancellation disarmed current job"
        );
        let generation = new.generation.clone();
        drop(store);
        let mut restarted = Store::open(&root, Limits::default())?;
        assert!(
            restarted.selected()?.is_none(),
            "restart restored selection"
        );
        assert!(restarted.commit(&new, &b).is_err());
        let (fresh, _) = restarted.begin(paths)?;
        assert_ne!(fresh.generation.session, generation.session);
        restarted.select_existing(&fresh, a.sha256())?;
        assert!(restarted.selected()?.is_some());
        restarted.cancel(&fresh)?;
        assert!(restarted.selected()?.is_none());
        Ok(())
    }
    #[test]
    fn changed_source_setup_table_and_corrupt_cache_disarm_before_use() -> TestResult {
        for which in 0..3 {
            let temp = Temp::new()?;
            let paths = temp.paths()?;
            let mut store = Store::open(&temp.0.join("store"), Limits::default())?;
            let (ticket, a) = prepared(&mut store, paths.clone())?;
            let changed = match which {
                0 => &paths.source,
                1 => &paths.setup,
                _ => paths.tool_table.as_ref().ok_or("table")?,
            };
            let mut bytes = fs::read(changed)?;
            bytes.push(b'\n');
            fs::write(changed, bytes)?;
            assert_eq!(
                store
                    .commit(&ticket, &a)
                    .err()
                    .ok_or("source change accepted")?
                    .code,
                "SOURCE_CHANGED"
            );
            assert!(store.selected()?.is_none());
            let (ticket, a) = prepared(&mut store, paths.clone())?;
            let selected = store.commit(&ticket, &a)?;
            if which == 0 {
                let mut bytes = fs::read(&paths.source)?;
                bytes.push(b'\n');
                fs::write(&paths.source, bytes)?;
                assert_eq!(
                    store
                        .selected()
                        .err()
                        .ok_or("selected source mutation accepted")?
                        .code,
                    "SOURCE_CHANGED"
                );
            } else {
                let object = store.object_path(&selected.artifact_sha256)?;
                fs::write(object, b"truncated/corrupt")?;
                assert_eq!(
                    store
                        .selected()
                        .err()
                        .ok_or("corrupt bundle accepted")?
                        .code,
                    "CORRUPT_BUNDLE"
                );
            }
            assert!(store.selected()?.is_none());
        }
        Ok(())
    }
    #[test]
    fn cancellation_failed_replacement_and_storage_exhaustion_never_fall_back() -> TestResult {
        let temp = Temp::new()?;
        let paths = temp.paths()?;
        let mut store = Store::open(&temp.0.join("store"), Limits::default())?;
        let (old, a) = prepared(&mut store, paths.clone())?;
        store.commit(&old, &a)?;
        store.fault = Some(Point::StatePartial);
        assert!(store.begin(paths.clone()).is_err());
        assert!(
            store.selected()?.is_none(),
            "full storage resurrected old selection"
        );
        let (cancelled, a) = prepared(&mut store, paths.clone())?;
        store.cancel(&cancelled)?;
        assert!(store.commit(&cancelled, &a).is_err());
        assert!(store.selected()?.is_none());
        let mut missing = paths.clone();
        missing.source = temp.0.join("missing.stpnc");
        assert!(store.begin(missing).is_err());
        assert!(store.selected()?.is_none());
        let (failed, _) = store.begin(paths.clone())?;
        store.reject(&failed, &fail("TEST", "worker failed"))?;
        assert!(store.commit(&failed, &a).is_err());
        for point in [
            Point::StagePartial,
            Point::AfterStageSync,
            Point::BeforeObjectRename,
            Point::AfterObjectRename,
            Point::StatePartial,
            Point::AfterStateSync,
        ] {
            let (ticket, a) = prepared(&mut store, paths.clone())?;
            store.fault = Some(point);
            assert!(
                store.commit(&ticket, &a).is_err(),
                "injected failure {point:?} accepted"
            );
            assert!(store.selected()?.is_none());
            assert!(store.commit(&ticket, &a).is_err(), "failed ticket reused");
        }
        let (ticket, a) = prepared(&mut store, paths)?;
        store.commit(&ticket, &a)?;
        assert!(store.selected()?.is_some());
        Ok(())
    }
    #[test]
    fn wrong_worker_identity_and_path_injection_fail_closed() -> TestResult {
        let temp = Temp::new()?;
        let paths = temp.paths()?;
        let mut store = Store::open(&temp.0.join("store"), Limits::default())?;
        let (ticket, s) = store.begin(paths.clone())?;
        let source = format!("{}\n", s.inputs().source);
        let wrong = bundle::compile(
            Inputs {
                source: &source,
                ..s.inputs()
            },
            &Limits::default(),
        )?;
        assert_eq!(
            store
                .commit(&ticket, &wrong)
                .err()
                .ok_or("wrong snapshot accepted")?
                .code,
            "IDENTITY"
        );
        assert!(store.selected()?.is_none());
        for hash in ["../source.stpnc", "", &"A".repeat(64)] {
            let (ticket, _) = store.begin(paths.clone())?;
            assert!(store.select_existing(&ticket, hash).is_err());
            assert!(store.selected()?.is_none());
        }
        Ok(())
    }
    #[test]
    fn store_never_overwrites_unrelated_files_or_its_own_input() -> TestResult {
        let temp = Temp::new()?;
        let paths = temp.paths()?;
        let source = fs::read(&paths.source)?;
        assert!(Store::open(&temp.0, Limits::default()).is_err());
        assert_eq!(fs::read_dir(&temp.0)?.count(), 3);
        let mut store = Store::open(&temp.0.join("store"), Limits::default())?;
        let protected = store.root.join("selection.json");
        fs::write(&protected, &source)?;
        let mut overlapping = paths;
        overlapping.source = protected.clone();
        assert_eq!(
            store
                .begin(overlapping)
                .err()
                .ok_or("overlap accepted")?
                .code,
            "INPUT_STORE_OVERLAP"
        );
        assert_eq!(fs::read(protected)?, source);
        assert!(store.selected()?.is_none());
        Ok(())
    }
    #[test]
    fn dropping_store_releases_ownership_even_while_an_inherited_descriptor_remains() -> TestResult
    {
        let temp = Temp::new()?;
        let root = temp.0.join("store");
        let store = Store::open(&root, Limits::default())?;
        // A fork in another thread retains the same open file description until
        // its exec. try_clone deterministically supplies that same lifetime
        // condition, without racing a subprocess or adding unsafe test code.
        let inherited = store._lock.try_clone()?;
        drop(store);
        let replacement = Store::open(&root, Limits::default())?;
        // Closing an old descriptor must not unlock the replacement owner.
        drop(inherited);
        assert_eq!(
            Store::open(&root, Limits::default())
                .err()
                .ok_or("replacement ownership was lost")?
                .code,
            "OWNER_BUSY"
        );
        drop(replacement);
        let _next = Store::open(&root, Limits::default())?;
        Ok(())
    }
    #[test]
    fn published_job_is_reusable_without_old_project_files_but_never_auto_selected() -> TestResult {
        let temp = Temp::new()?;
        let paths = temp.paths()?;
        let root = temp.0.join("store");
        let (hash, old_generation) = {
            let mut store = Store::open(&root, Limits::default())?;
            let (ticket, a) = prepared(&mut store, paths.clone())?;
            let selection = store.commit(&ticket, &a)?;
            (selection.artifact_sha256, selection.generation)
        };
        fs::remove_file(&paths.source)?;
        fs::remove_file(&paths.setup)?;
        fs::remove_file(paths.tool_table.as_ref().ok_or("table")?)?;
        let mut store = Store::open(&root, Limits::default())?;
        assert!(store.selected()?.is_none());
        let (ticket, a) = store.begin_bundle(&hash)?;
        assert!(store.selected()?.is_none());
        assert_ne!(ticket.generation.session, old_generation.session);
        assert!(!a.prepared().audit().execution_authorized);
        store.commit(&ticket, &a)?;
        assert_eq!(
            store.selected()?.ok_or("selection")?.0.artifact_sha256,
            hash
        );
        let original = fs::read(store.object_path(&hash)?)?;
        fs::write(
            store.object_path(&hash)?,
            b"corruption after explicit selection",
        )?;
        assert!(store.selected().is_err());
        assert!(store.selected()?.is_none());
        fs::write(store.object_path(&hash)?, original)?;
        let (cancelled, a) = store.begin_bundle(&hash)?;
        store.cancel(&cancelled)?;
        assert!(store.commit(&cancelled, &a).is_err());
        assert!(store.begin_bundle("../staging/partial").is_err());
        assert!(store.selected()?.is_none());
        Ok(())
    }
    #[test]
    fn crash_child() -> TestResult {
        let Ok(root) = std::env::var("NEXTNC_TEST_CRASH_ROOT") else {
            return Ok(());
        };
        let index: usize = std::env::var("NEXTNC_TEST_CRASH_POINT")?.parse()?;
        let root = PathBuf::from(root);
        let paths = Paths {
            source: root.join("source.stpnc"),
            setup: root.join("setup.json"),
            tool_table: Some(root.join("tool.tbl")),
            target: None,
        };
        let mut store = Store::open(&root.join("store"), Limits::default())?;
        let (ticket, a) = prepared(&mut store, paths)?;
        store.fault = Some(
            [
                Point::StagePartial,
                Point::AfterStageSync,
                Point::BeforeObjectRename,
                Point::AfterObjectRename,
                Point::StatePartial,
                Point::AfterStateSync,
            ][index],
        );
        store.crash = true;
        store.commit(&ticket, &a)?;
        Err("child failed to exit at checkpoint".into())
    }
    #[test]
    fn real_process_exit_releases_owner_and_never_restores_an_old_job() -> TestResult {
        for index in 0..6 {
            let temp = Temp::new()?;
            let paths = temp.paths()?;
            let root = temp.0.join("store");
            {
                let mut old = Store::open(&root, Limits::default())?;
                let (ticket, a) = prepared(&mut old, paths.clone())?;
                old.commit(&ticket, &a)?;
            }
            let output = std::process::Command::new(std::env::current_exe()?)
                .args(["--exact", "publication::tests::crash_child", "--nocapture"])
                .env("NEXTNC_TEST_CRASH_ROOT", &temp.0)
                .env("NEXTNC_TEST_CRASH_POINT", index.to_string())
                .output()?;
            assert_eq!(
                output.status.code(),
                Some(86),
                "child {index}: {} {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let mut recovered = Store::open(&root, Limits::default())?;
            assert!(
                recovered.selected()?.is_none(),
                "crash checkpoint {index} restored selection"
            );
            let (ticket, a) = prepared(&mut recovered, paths)?;
            recovered.commit(&ticket, &a)?;
            assert!(recovered.selected()?.is_some());
        }
        Ok(())
    }
}
