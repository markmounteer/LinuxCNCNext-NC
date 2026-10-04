use sha2::{Digest, Sha256};
use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn collect(root: &Path, directory: &str, files: &mut Vec<String>) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(root.join(directory))? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            collect(
                root,
                &format!("{directory}/{}", entry.file_name().to_string_lossy()),
                files,
            )?;
        } else {
            files.push(format!(
                "{directory}/{}",
                entry.file_name().to_string_lossy()
            ));
        }
    }
    Ok(())
}
fn field(hash: &mut Sha256, name: &str, bytes: &[u8]) {
    hash.update((name.len() as u64).to_le_bytes());
    hash.update(name);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}
fn main() -> Result<(), Box<dyn Error>> {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let mut files: Vec<String> = [
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        "build.rs",
        "crates/motion-command/Cargo.toml",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    for directory in ["src-rust", "crates/motion-command/src"] {
        println!("cargo:rerun-if-changed={directory}");
        collect(&root, directory, &mut files)?;
    }
    files.sort();
    let mut hash = Sha256::new();
    for file in files {
        println!("cargo:rerun-if-changed={file}");
        // Git treats implementation files as LF text on both supported hosts.
        // Normalize only these compiler inputs; job bytes are NEVER normalized.
        let content = fs::read_to_string(root.join(&file))?.replace("\r\n", "\n");
        field(&mut hash, &file, content.as_bytes());
    }
    let rustc = Command::new(std::env::var("RUSTC")?).arg("-vV").output()?;
    if !rustc.status.success() {
        return Err("Unable to identify the Rust compiler".into());
    }
    field(
        &mut hash,
        "rustc",
        String::from_utf8(rustc.stdout)?
            .replace("\r\n", "\n")
            .as_bytes(),
    );
    for name in ["TARGET", "PROFILE", "CARGO_ENCODED_RUSTFLAGS"] {
        println!("cargo:rerun-if-env-changed={name}");
        field(
            &mut hash,
            name,
            std::env::var(name).unwrap_or_default().as_bytes(),
        );
    }
    let mut features: Vec<_> = std::env::vars()
        .filter(|(k, _)| k.starts_with("CARGO_FEATURE_"))
        .collect();
    features.sort();
    for (k, v) in features {
        field(&mut hash, &k, v.as_bytes());
    }
    println!(
        "cargo:rustc-env=NEXTNC_COMPILER_SHA256={:x}",
        hash.finalize()
    );
    Ok(())
}
