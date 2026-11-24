//! rust-analyzer's SCIP index of the workspace, cached per workspace under
//! `target/deslop-please/` and rebuilt only when a source file changes.
//!
//! Indexing builds what the code needs at compile time, as `cargo check
//! --compile-time-deps` does: `rust-analyzer scip` compiles and runs the build
//! scripts, and compiles the proc macros and what they and the build scripts
//! depend on, into the workspace's target directory. The rest it only reads.
//! `scip` always does this (`cargo.buildScripts.enable` is the language
//! server's), and the index needs it to read the code as the compiler does:
//! which `cfg`s a build script sets, what it writes to `OUT_DIR`, what a proc
//! macro expands to. The lints see such generated items where the workspace's
//! own code uses them. The first index pays for the build; later ones, and the
//! next `cargo check`, find it done. Generated code is as fresh as the last
//! index: a change to a build script's other inputs alone does not index again.

use std::hash::{DefaultHasher, Hasher};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::workspace::Workspace;
use crate::{Error, TOOL};

pub fn index(workspace: &Workspace) -> Result<Vec<u8>, Error> {
    let directory = cache_dir(workspace);
    let index = directory.join("index.scip");
    let stamp_path = directory.join("index.stamp");
    let stamp = stamp(workspace)?;
    let last_stamp = std::fs::read_to_string(&stamp_path).ok();
    if last_stamp.as_deref() != Some(stamp.as_str()) || !index.exists() {
        eprintln!("{TOOL}: indexing workspace...");
        std::fs::create_dir_all(&directory).map_err(|e| io_error(&directory, e))?;
        remove_if_present(&stamp_path)?;
        run_rust_analyzer(&workspace.root, &index)?;
        std::fs::write(&stamp_path, &stamp).map_err(|e| io_error(&stamp_path, e))?;
    }
    std::fs::read(&index).map_err(|e| io_error(&index, e))
}

fn cache_dir(workspace: &Workspace) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    hasher.write(workspace.root.to_string_lossy().as_bytes());
    workspace
        .target_directory
        .join(TOOL)
        .join(format!("{:016x}", hasher.finish()))
}

fn stamp(workspace: &Workspace) -> Result<String, Error> {
    let version = Command::new("rust-analyzer")
        .arg("--version")
        .output()
        .map_err(missing_rust_analyzer)?;
    let mut stamp = String::from_utf8_lossy(&version.stdout).into_owned();
    for file in workspace.source_files()? {
        let hash = match std::fs::read(workspace.root.join(&file)) {
            Ok(bytes) => {
                let mut hasher = DefaultHasher::new();
                hasher.write(&bytes);
                format!("{:016x}", hasher.finish())
            }
            Err(e) if e.kind() == ErrorKind::NotFound => "missing".to_owned(),
            Err(e) => return Err(io_error(&workspace.root.join(&file), e)),
        };
        stamp += &format!("{hash} {file}\n");
    }
    Ok(stamp)
}

fn run_rust_analyzer(root: &Path, index: &Path) -> Result<(), Error> {
    let output = Command::new("rust-analyzer")
        .arg("scip")
        .arg(root)
        .arg("--output")
        .arg(index)
        .output()
        .map_err(missing_rust_analyzer)?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let last_lines: Vec<&str> = stderr.lines().rev().take(20).collect();
    let last_lines: Vec<&str> = last_lines.into_iter().rev().collect();
    Err(Error::new(format!(
        "rust-analyzer scip failed ({}):\n{}",
        output.status,
        last_lines.join("\n")
    )))
}

fn missing_rust_analyzer(error: std::io::Error) -> Error {
    if error.kind() == ErrorKind::NotFound {
        Error::new(format!(
            "rust-analyzer is not on PATH: {TOOL} reads the index that `rust-analyzer scip` writes"
        ))
    } else {
        Error::new(format!("cannot run rust-analyzer: {error}"))
    }
}

fn remove_if_present(path: &Path) -> Result<(), Error> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != ErrorKind::NotFound => Err(io_error(path, e)),
        _ => Ok(()),
    }
}

fn io_error(path: &Path, error: std::io::Error) -> Error {
    Error::new(format!("{}: {error}", path.display()))
}
