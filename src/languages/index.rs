//! The SCIP index of a Python, TypeScript or JavaScript project, written by
//! scip-python or scip-typescript and cached outside the project, under the
//! user's cache directory, keyed by the project's place; it is rebuilt only
//! when the indexer, a manifest, or a source file changes. Nothing is written
//! into the project: a JavaScript project without a `tsconfig.json` is indexed
//! through one written to the cache, which names the project's files by
//! their absolute paths.

use std::hash::{DefaultHasher, Hasher};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use super::Language;
use crate::{Error, TOOL};

/// Files beside the sources whose change can change the index.
const MANIFESTS: [&str; 6] = [
    "pyproject.toml",
    "setup.cfg",
    "pyrightconfig.json",
    "package.json",
    "tsconfig.json",
    "jsconfig.json",
];

/// The index of the `language` project at `root`, whose sources are `files`
/// and whose package is called `name`; `shown` is how messages name it.
pub fn index(
    language: Language,
    root: &Path,
    files: &[String],
    name: &str,
    shown: &str,
) -> Result<Vec<u8>, Error> {
    let program = match language {
        Language::Python => "scip-python",
        _ => "scip-typescript",
    };
    let indexer = find(root, program);
    let not_runnable = |error: std::io::Error| {
        if error.kind() == ErrorKind::NotFound {
            missing(program, language, shown)
        } else {
            Error::new(format!("cannot run {program}: {error}"))
        }
    };
    let version = run(Command::new(&indexer).arg("--version")).map_err(not_runnable)?;
    let directory = cache_dir(root, language);
    let index = directory.join("index.scip");
    let stamp_path = directory.join("index.stamp");
    let stamp = stamp(root, files, name, &String::from_utf8_lossy(&version.stdout))?;
    if std::fs::read_to_string(&stamp_path).ok().as_deref() == Some(stamp.as_str())
        && index.exists()
    {
        return std::fs::read(&index).map_err(|e| io_error(&index, e));
    }
    eprintln!("{TOOL}: indexing {shown}...");
    std::fs::create_dir_all(&directory).map_err(|e| io_error(&directory, e))?;
    let _ = std::fs::remove_file(&stamp_path);
    let mut command = Command::new(&indexer);
    command.current_dir(root);
    match language {
        Language::Python => {
            command
                .args([
                    "index",
                    "--quiet",
                    "--project-name",
                    name,
                    "--project-version",
                    "0",
                ])
                .arg("--cwd")
                .arg(root)
                .arg("--output")
                .arg(&index);
        }
        _ => {
            let tsconfig = tsconfig(root, files, &directory)?;
            command
                .args(["index", "--no-progress-bar"])
                .arg("--cwd")
                .arg(root)
                .arg("--output")
                .arg(&index)
                .arg(tsconfig);
        }
    }
    let _ = std::fs::remove_file(&index);
    let output = run(&mut command).map_err(not_runnable)?;
    if !output.status.success() || !index.exists() {
        return Err(failed(program, shown, &output));
    }
    std::fs::write(&stamp_path, &stamp).map_err(|e| io_error(&stamp_path, e))?;
    std::fs::read(&index).map_err(|e| io_error(&index, e))
}

/// The project's own copy of `program`, a devDependency, or the one on `PATH`.
fn find(root: &Path, program: &str) -> PathBuf {
    let local = root.join("node_modules").join(".bin").join(program);
    if local.is_file() {
        local
    } else {
        PathBuf::from(program)
    }
}

fn run(command: &mut Command) -> std::io::Result<Output> {
    let output = command.output()?;
    // `env: node: No such file or directory`: the indexer is a Node.js script.
    if output.status.code() == Some(127) {
        return Err(std::io::Error::new(
            ErrorKind::NotFound,
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ));
    }
    Ok(output)
}

/// The error for an indexer that cannot be found, and how to install it.
fn missing(program: &str, language: Language, shown: &str) -> Error {
    let install = match language {
        Language::Python => "npm install -g @sourcegraph/scip-python   (needs Node.js 18 or later)",
        _ => {
            "npm install -g @sourcegraph/scip-typescript   (needs Node.js 18 or later; \
             a devDependency of the project works too)"
        }
    };
    Error::new(format!(
        "{program} is not installed: {TOOL} reads the index it writes for the {language} \
         project {shown}\n  install it with: {install}\n  or leave this language out with \
         `--language <other>`"
    ))
}

fn failed(program: &str, shown: &str, output: &Output) -> Error {
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let lines: Vec<&str> = text.lines().collect();
    let last = &lines[lines.len().saturating_sub(20)..];
    Error::new(format!(
        "{program} failed on {shown} ({}):\n{}",
        output.status,
        last.join("\n")
    ))
}

/// The `tsconfig.json` to index the project at `root` by: its own, or, when
/// it has none, one written to `directory` that takes its `files`, as
/// `--infer-tsconfig` would but without writing into the project.
fn tsconfig(root: &Path, files: &[String], directory: &Path) -> Result<PathBuf, Error> {
    let own = root.join("tsconfig.json");
    if own.is_file() {
        return Ok(own);
    }
    let root = root.to_string_lossy().replace('\\', "/");
    let config = serde_json::json!({
        "compilerOptions": {"allowJs": true, "checkJs": false, "noEmit": true, "jsx": "preserve"},
        "files": files.iter().map(|file| format!("{root}/{file}")).collect::<Vec<_>>(),
    });
    let path = directory.join("tsconfig.json");
    std::fs::write(&path, config.to_string()).map_err(|e| io_error(&path, e))?;
    Ok(path)
}

/// Where the index of the `language` project at `root` is kept: each
/// project's apart.
fn cache_dir(root: &Path, language: Language) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    hasher.write(root.to_string_lossy().as_bytes());
    hasher.write(language.to_string().as_bytes());
    cache_root()
        .join(TOOL)
        .join(format!("{:016x}", hasher.finish()))
}

/// `$XDG_CACHE_HOME`, else `~/.cache`, else the system's temporary directory.
fn cache_root() -> PathBuf {
    let from = |variable: &str| {
        std::env::var_os(variable)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    };
    from("XDG_CACHE_HOME")
        .or_else(|| from("HOME").map(|home| home.join(".cache")))
        .or_else(|| from("LOCALAPPDATA"))
        .unwrap_or_else(std::env::temp_dir)
}

/// What the index is built from: the indexer, the package's name, the
/// manifests, and the sources.
fn stamp(root: &Path, files: &[String], name: &str, version: &str) -> Result<String, Error> {
    let mut stamp = format!("{version}\n{name}\n");
    let files = MANIFESTS
        .iter()
        .map(|&file| file.to_owned())
        .chain(files.iter().cloned());
    for file in files {
        let path = root.join(&file);
        let hash = match std::fs::read(&path) {
            Ok(bytes) => {
                let mut hasher = DefaultHasher::new();
                hasher.write(&bytes);
                format!("{:016x}", hasher.finish())
            }
            Err(e) if e.kind() == ErrorKind::NotFound => "missing".to_owned(),
            Err(e) => return Err(io_error(&path, e)),
        };
        stamp += &format!("{hash} {file}\n");
    }
    Ok(stamp)
}

fn io_error(path: &Path, error: std::io::Error) -> Error {
    Error::new(format!("{}: {error}", path.display()))
}
