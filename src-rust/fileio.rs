//! Bounded regular-file reads. Never wait for a FIFO writer while deciding
//! whether an input is a file. No unsafe code or external process is needed.
use crate::{Diagnostic, Result};
use std::{fs, fs::OpenOptions, io::Read, path::Path};

pub(crate) fn read_bytes(path: &Path, limit: usize, follow_symlinks: bool) -> Result<Vec<u8>> {
    let error = |code: &str, message: String| {
        Diagnostic::new("read", code, message).with("file", path.display().to_string())
    };
    let io = |e: std::io::Error| error("IO", e.to_string());
    let path_type = fs::symlink_metadata(path).map_err(io)?;
    if !follow_symlinks && path_type.file_type().is_symlink() {
        return Err(error(
            "INPUT_TYPE",
            "Input must not be a symbolic link".into(),
        ));
    }
    if !fs::metadata(path).map_err(io)?.is_file() {
        return Err(error("INPUT_TYPE", "Expected a regular file".into()));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // O_NONBLOCK closes the check/open FIFO race; descriptor metadata below
        // remains authoritative. Publication also refuses symlink substitution.
        options.custom_flags(libc::O_NONBLOCK | if follow_symlinks { 0 } else { libc::O_NOFOLLOW });
    }
    let file = options.open(path).map_err(io)?;
    let before = file.metadata().map_err(io)?;
    if !before.is_file() {
        return Err(error(
            "INPUT_TYPE",
            "Opened input is not a regular file".into(),
        ));
    }
    let size_error =
        || error("INPUT_SIZE", "Input exceeds byte limit".into()).with("limitBytes", limit);
    if before.len() > limit as u64 {
        return Err(size_error());
    }
    let mut bytes = Vec::new();
    (&file)
        .take((limit as u64).saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(io)?;
    if bytes.len() > limit {
        return Err(size_error());
    }
    let after = file.metadata().map_err(io)?;
    if before.len() != after.len()
        || after.len() != bytes.len() as u64
        || before.modified().ok() != after.modified().ok()
    {
        return Err(error(
            "SOURCE_CHANGED",
            "Input changed during its bounded read; prepare a fresh snapshot".into(),
        ));
    }
    Ok(bytes)
}
