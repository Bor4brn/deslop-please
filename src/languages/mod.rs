//! The languages this tool reads, and the projects of each found where it
//! runs: a Cargo workspace through rust-analyzer, Python projects through
//! scip-python, TypeScript and JavaScript projects through scip-typescript.
//! Each project is indexed and turned into its own model; the lints read the
//! model and know nothing of the language. Adding a language is adding its
//! variant here, how its projects are found (`detect`), how they are indexed
//! (`index`), and how its symbols name modules (`model::Syntax`).

mod detect;
mod index;

use std::fmt;
use std::path::{Path, PathBuf};

use crate::configuration::Configuration;
use crate::model::{self, Model, Syntax};
use crate::workspace::Workspace;
use crate::{Error, indexing, scip};

pub use detect::detect;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Language {
    Rust,
    Python,
    TypeScript,
    JavaScript,
}

impl Language {
    /// `None` for `auto`, which takes every language found.
    pub fn parse(name: &str) -> Result<Option<Language>, String> {
        match name {
            "auto" => Ok(None),
            "rust" => Ok(Some(Language::Rust)),
            "python" => Ok(Some(Language::Python)),
            "typescript" => Ok(Some(Language::TypeScript)),
            "javascript" => Ok(Some(Language::JavaScript)),
            other => Err(format!(
                "unknown language '{other}': expected rust, python, typescript, javascript or auto"
            )),
        }
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Language::Rust => "rust",
            Language::Python => "python",
            Language::TypeScript => "typescript",
            Language::JavaScript => "javascript",
        })
    }
}

/// A project of one language, found below where the run started.
pub struct Project {
    pub language: Language,
    /// Absolute.
    root: PathBuf,
    /// `root` relative to where the run started, `/`-separated, ending in
    /// `/`; empty for there, and for a Cargo workspace that holds it.
    prefix: String,
    /// Its source files, relative to `root`: those the model reads. Empty for
    /// Rust, whose files cargo knows.
    files: Vec<String>,
}

impl Project {
    /// `backend (python)`, or `. (rust)` where the run started.
    pub fn describe(&self) -> String {
        let place = self.prefix.trim_end_matches('/');
        let place = if place.is_empty() { "." } else { place };
        format!("{place} ({})", self.language)
    }

    /// Whether its indexer is installed; Rust's is taken to be, as cargo's
    /// rust-analyzer is checked where it runs.
    pub fn indexer_installed(&self) -> Result<(), Error> {
        if self.language == Language::Rust {
            return Ok(());
        }
        index::installed(self.language, &self.root, &self.describe())
    }

    /// Its model, read from a fresh or cached index. `configuration` is
    /// cargo's, which only a Rust project takes.
    pub fn model(&self, configuration: &Configuration) -> Result<Model, Error> {
        if self.language == Language::Rust {
            let workspace = Workspace::discover_in(&self.root)?;
            let bytes = indexing::index(&workspace, configuration)?;
            let index = scip::read(&bytes)
                .map_err(|e| Error::new(format!("rust-analyzer's index: {e}")))?;
            let mut model = model::build(&index, &workspace)?;
            for file in &mut model.files {
                file.path.insert_str(0, &self.prefix);
            }
            return Ok(model);
        }
        let name = self.name();
        let shown = self.describe();
        let bytes = index::index(self.language, &self.root, &self.files, &name, &shown)?;
        let index = scip::read(&bytes)
            .map_err(|e| Error::new(format!("{}'s index: {e}", self.language)))?;
        let read = |path: &str| {
            let on_disk = self.root.join(path);
            std::fs::read_to_string(&on_disk)
                .map_err(|e| Error::new(format!("{}: {e}", on_disk.display())))
        };
        let syntax = match self.language {
            Language::Python => Syntax::Python,
            _ => Syntax::Script,
        };
        let model = model::build_from_scip(&index, syntax, &name, &self.prefix, &self.files, read)?;
        if model.files.is_empty() && !self.files.is_empty() {
            return Err(Error::new(format!(
                "{}: the {} indexer indexed none of its {} source files; check that its \
                 tsconfig.json or pyproject.toml includes them",
                self.describe(),
                self.language,
                self.files.len()
            )));
        }
        Ok(model)
    }

    /// The name its package goes by in reports: `name` in `pyproject.toml`
    /// or `package.json`, else its directory's.
    fn name(&self) -> String {
        let declared = match self.language {
            Language::Python => pyproject_name(&self.root.join("pyproject.toml")),
            Language::TypeScript | Language::JavaScript => {
                package_json_name(&self.root.join("package.json"))
            }
            Language::Rust => None,
        };
        let name = declared.unwrap_or_else(|| directory_name(&self.root));
        // A SCIP symbol ends a field at a space.
        name.replace(char::is_whitespace, "-")
    }
}

fn directory_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || "project".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// `name` under `[project]` or `[tool.poetry]`.
fn pyproject_name(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut section = "";
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            section = line;
        } else if (section == "[project]" || section == "[tool.poetry]")
            && let Some((key, value)) = line.split_once('=')
            && key.trim() == "name"
        {
            let value = value.trim().trim_matches(|c| c == '"' || c == '\'');
            return (!value.is_empty()).then(|| value.to_owned());
        }
    }
    None
}

fn package_json_name(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    let name = json["name"].as_str()?;
    (!name.is_empty()).then(|| name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TOOL;

    #[test]
    fn languages_are_named_on_the_command_line_as_they_are_shown() {
        for language in [
            Language::Rust,
            Language::Python,
            Language::TypeScript,
            Language::JavaScript,
        ] {
            assert_eq!(Language::parse(&language.to_string()), Ok(Some(language)));
        }
        assert_eq!(Language::parse("auto"), Ok(None));
        assert!(Language::parse("go").is_err());
    }

    #[test]
    fn a_pyproject_names_its_project_under_project_or_poetry() {
        let directory =
            std::env::temp_dir().join(format!("{TOOL}-pyproject-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("pyproject.toml");
        std::fs::write(
            &path,
            "[build-system]\nname = 'x'\n[project]\nname = \"api\"\n",
        )
        .unwrap();
        assert_eq!(pyproject_name(&path).as_deref(), Some("api"));
        std::fs::write(&path, "[tool.poetry]\nname='svc'\n").unwrap();
        assert_eq!(pyproject_name(&path).as_deref(), Some("svc"));
        std::fs::write(&path, "[tool.ruff]\nline-length = 1\n").unwrap();
        assert_eq!(pyproject_name(&path), None);
        std::fs::remove_dir_all(&directory).unwrap();
    }
}
