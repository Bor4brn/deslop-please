//! rust-analyzer's SCIP index of the workspace, cached per workspace under
//! `target/deslop-please/` and rebuilt only when a source file changes. Runs with
//! build scripts disabled so indexing stays fast and compiles nothing.

use std::hash::{DefaultHasher, Hasher};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::workspace::Workspace;
use crate::{Error, TOOL};

const RA_CONFIG: &str = r#"{"cargo":{"buildScripts":{"enable":false}}}"#;

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
        let config = directory.join("ra-config.json");
        std::fs::write(&config, RA_CONFIG).map_err(|e| io_error(&config, e))?;
        run_rust_analyzer(&workspace.root, &config, &index)?;
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
    stamp += RA_CONFIG;
    stamp.push('\n');
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

fn run_rust_analyzer(root: &Path, config: &Path, index: &Path) -> Result<(), Error> {
    let output = Command::new("rust-analyzer")
        .arg("scip")
        .arg(root)
        .arg("--config-path")
        .arg(config)
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
