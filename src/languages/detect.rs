//! Which projects, of which languages, are below the directory a run starts
//! in. One walk of the tree finds them by their manifests: `Cargo.toml`;
//! `pyproject.toml`, `setup.py`, `setup.cfg`, `requirements.txt` or `Pipfile`;
//! `package.json`, or `tsconfig.json` outside any `package.json` project. A
//! project of each family holds the source files below it, less those of a
//! project of the same family nested in it, so each package of a monorepo is
//! a project of its own. Dependencies, virtual environments, build output,
//! hidden directories, test data and generated files are no one's sources. A project
//! with no sources of its own, such as one whose only JavaScript is its tools'
//! configuration, is left out.
//!
//! A Cargo workspace around the start, as `cargo metadata` finds it, is the
//! Rust project, as it has always been; below the start, each outermost
//! `Cargo.toml` is one.

use std::path::{Path, PathBuf};

use super::{Language, Project};
use crate::Error;

/// Directories that hold no one's sources: dependencies, build output,
/// caches, and the test data that tests write projects into.
const SKIPPED: [&str; 16] = [
    "node_modules",
    "bower_components",
    "jspm_packages",
    "site-packages",
    "__pycache__",
    "__generated__",
    "target",
    "dist",
    "build",
    "out",
    "coverage",
    "htmlcov",
    "fixtures",
    "__fixtures__",
    "testdata",
    "__snapshots__",
];

const PYTHON_MANIFESTS: [&str; 5] = [
    "pyproject.toml",
    "setup.py",
    "setup.cfg",
    "requirements.txt",
    "Pipfile",
];

/// The projects below `start`, of `language` or of every language.
pub fn detect(start: &Path, language: Option<Language>) -> Result<Vec<Project>, Error> {
    let mut found = Found::default();
    let rust_around = cargo_around(start);
    if rust_around {
        found.projects.push(Project {
            language: Language::Rust,
            root: start.to_owned(),
            prefix: String::new(),
            files: Vec::new(),
        });
    }
    let owners = Owners {
        rust: rust_around,
        python: None,
        script: None,
    };
    found.walk(start, start, owners, true)?;
    let mut projects: Vec<Project> = found
        .projects
        .into_iter()
        .filter(|project| project.language == Language::Rust || has_sources(project))
        .collect();
    if let Some(language) = language {
        projects.retain(|project| project.language == language);
        if projects.is_empty() && language != Language::Rust {
            // Asked for by name: the start is the project, manifest or not.
            projects.extend(loose_project(start, language)?);
        }
    }
    projects.sort_by(|a, b| (&a.prefix, a.language).cmp(&(&b.prefix, b.language)));
    if projects.is_empty() {
        return Err(nothing_found(start, language));
    }
    Ok(projects)
}

/// Whether a Cargo workspace holds `start` among its sources: a `Cargo.toml`
/// in it or above it, with no directory between that holds no one's sources,
/// such as the workspace's own `target`.
fn cargo_around(start: &Path) -> bool {
    for directory in start.ancestors() {
        if directory.join("Cargo.toml").is_file() {
            return true;
        }
        let name = directory.file_name().map(|name| name.to_string_lossy());
        if name.is_some_and(|name| skipped_directory(&name, directory)) {
            return false;
        }
    }
    false
}

fn nothing_found(start: &Path, language: Option<Language>) -> Error {
    let manifests = match language {
        Some(Language::Rust) => "not inside a Cargo workspace".to_owned(),
        Some(language) => format!("no {language} source files below it"),
        None => "not inside a Cargo workspace, and no pyproject.toml, setup.py, setup.cfg, \
                 requirements.txt, Pipfile, package.json or tsconfig.json with sources below it"
            .to_owned(),
    };
    Error::new(format!("{}: {manifests}", start.display()))
}

#[derive(Default)]
struct Found {
    /// By the index each was found at; a project's files are added as the
    /// walk passes them.
    projects: Vec<Project>,
}

/// The project of each family that the files of a directory belong to, by
/// its place in `Found::projects`.
#[derive(Clone, Copy)]
struct Owners {
    /// A Cargo workspace holds this directory: cargo finds its own members.
    rust: bool,
    python: Option<usize>,
    script: Option<usize>,
}

impl Found {
    /// Adds the projects below `directory`, when `manifests` makes them, and
    /// the source files of each.
    fn walk(
        &mut self,
        start: &Path,
        directory: &Path,
        mut owners: Owners,
        manifests: bool,
    ) -> Result<(), Error> {
        let has = |name: &str| manifests && directory.join(name).is_file();
        if !owners.rust && has("Cargo.toml") {
            self.add(start, directory, Language::Rust);
            owners.rust = true;
        }
        if PYTHON_MANIFESTS.iter().any(|name| has(name)) {
            owners.python = Some(self.add(start, directory, Language::Python));
        }
        if has("package.json") || (owners.script.is_none() && has("tsconfig.json")) {
            owners.script = Some(self.add(start, directory, Language::JavaScript));
        }
        let unreadable = |e| Error::new(format!("cannot read {}: {e}", directory.display()));
        let mut entries: Vec<_> = std::fs::read_dir(directory)
            .map_err(unreadable)?
            .collect::<Result<_, _>>()
            .map_err(unreadable)?;
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let name = entry.file_name().to_string_lossy().into_owned();
            // Not through symbolic links, which can loop.
            let file_type = entry.file_type().map_err(unreadable)?;
            let path = entry.path();
            if file_type.is_dir() {
                if !skipped_directory(&name, &path) {
                    self.walk(start, &path, owners, manifests)?;
                }
            } else if file_type.is_file() {
                self.add_file(&path, &name, owners);
            }
        }
        Ok(())
    }

    fn add(&mut self, start: &Path, root: &Path, language: Language) -> usize {
        self.projects.push(Project {
            language,
            root: root.to_owned(),
            prefix: prefix(start, root),
            files: Vec::new(),
        });
        self.projects.len() - 1
    }

    fn add_file(&mut self, path: &Path, name: &str, owners: Owners) {
        let Some(family) = family(name) else {
            return;
        };
        let owner = match family {
            Family::Python => owners.python,
            Family::TypeScript | Family::JavaScript => owners.script,
        };
        let Some(project) = owner.map(|at| &mut self.projects[at]) else {
            return;
        };
        if generated(path, name) {
            return;
        }
        let Ok(relative) = path.strip_prefix(&project.root) else {
            return;
        };
        let parts: Vec<_> = relative
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect();
        project.files.push(parts.join("/"));
        if family == Family::TypeScript {
            project.language = Language::TypeScript;
        }
    }
}

/// A project of `language` at `start` itself, with every source file below it.
fn loose_project(start: &Path, language: Language) -> Result<Option<Project>, Error> {
    let mut found = Found::default();
    // JavaScript until a TypeScript file is found, as a walk finds it.
    let family = match language {
        Language::TypeScript => Language::JavaScript,
        other => other,
    };
    let at = found.add(start, start, family);
    let owners = match language {
        Language::Python => Owners {
            rust: true,
            python: Some(at),
            script: None,
        },
        _ => Owners {
            rust: true,
            python: None,
            script: Some(at),
        },
    };
    found.walk(start, start, owners, false)?;
    let project = found.projects.swap_remove(at);
    Ok((project.language == language && has_sources(&project)).then_some(project))
}

/// Whether `project` has sources of its own: files other than its tools'
/// configuration, `vite.config.ts` or `eslint.config.js`.
fn has_sources(project: &Project) -> bool {
    project.files.iter().any(|file| {
        let name = file.rsplit('/').next().unwrap_or(file);
        !name.contains(".config.") && name != "setup.py" && name != "conftest.py"
    })
}

fn skipped_directory(name: &str, path: &Path) -> bool {
    name.starts_with('.')
        || SKIPPED.contains(&name)
        // A virtual environment, whatever it is called.
        || path.join("pyvenv.cfg").is_file()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Family {
    Python,
    TypeScript,
    JavaScript,
}

/// The family a file's name says it is source of, if any.
fn family(name: &str) -> Option<Family> {
    let (_, extension) = name.rsplit_once('.')?;
    match extension {
        "py" => Some(Family::Python),
        "ts" | "tsx" | "mts" | "cts" if !is_declaration(name) => Some(Family::TypeScript),
        "js" | "jsx" | "mjs" | "cjs" => Some(Family::JavaScript),
        _ => None,
    }
}

/// `x.d.ts`, `x.d.mts`: types only, mostly written by tools.
fn is_declaration(name: &str) -> bool {
    [".d.ts", ".d.mts", ".d.cts"]
        .iter()
        .any(|end| name.ends_with(end))
}

/// Whether a tool wrote the file: by its name (`x.min.js`, `x_pb2.py`,
/// `x.generated.ts`), or by a marker in its first lines (`@generated`,
/// `DO NOT EDIT`, `auto-generated`).
fn generated(path: &Path, name: &str) -> bool {
    let by_name = name.contains(".min.")
        || name.contains(".generated.")
        || name.ends_with("_pb2.py")
        || name.ends_with("_pb2_grpc.py")
        || name.contains("_pb.")
        || name.contains(".pb.");
    by_name || head_says_generated(path)
}

fn head_says_generated(path: &Path) -> bool {
    use std::io::Read;
    let mut head = [0u8; 512];
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let read = file.read(&mut head).unwrap_or(0);
    let head = String::from_utf8_lossy(&head[..read]).to_lowercase();
    head.lines().take(5).any(|line| {
        line.contains("@generated")
            || line.contains("do not edit")
            || line.contains("auto-generated")
            || line.contains("autogenerated")
            || line.contains("code generated by")
    })
}

/// `root` relative to `start`, `/`-separated and ending in `/`; empty for `start`.
fn prefix(start: &Path, root: &Path) -> String {
    let relative: PathBuf = root
        .strip_prefix(start)
        .map(Path::to_owned)
        .unwrap_or_default();
    let parts: Vec<_> = relative
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect();
    if parts.is_empty() {
        String::new()
    } else {
        format!("{}/", parts.join("/"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "deslop-please-detect-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        for (path, text) in files {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        root
    }

    fn found(root: &Path, language: Option<Language>) -> Vec<(String, Language, Vec<String>)> {
        detect(root, language)
            .unwrap()
            .into_iter()
            .map(|p| (p.prefix, p.language, p.files))
            .collect()
    }

    #[test]
    fn a_monorepo_holds_a_project_per_package_and_skips_what_no_one_wrote() {
        let root = tree(
            "monorepo",
            &[
                ("backend/pyproject.toml", "[project]\nname='api'\n"),
                ("backend/app/main.py", "x = 1\n"),
                ("backend/app/schema_pb2.py", "y = 1\n"),
                ("backend/.venv/lib/site.py", ""),
                ("backend/venv/pyvenv.cfg", ""),
                ("backend/venv/lib/x.py", ""),
                ("frontend/package.json", "{}"),
                ("frontend/tsconfig.json", "{}"),
                ("frontend/next.config.js", ""),
                ("frontend/src/app/page.tsx", "export {}\n"),
                ("frontend/src/types.d.ts", ""),
                (
                    "frontend/src/gen.ts",
                    "// @generated by a tool\nexport {}\n",
                ),
                ("frontend/node_modules/react/index.js", ""),
                ("frontend/dist/out.js", ""),
                ("tools/package.json", "{}"),
                ("tools/eslint.config.js", ""),
                ("docs/figures.py", ""),
            ],
        );
        assert_eq!(
            found(&root, None),
            [
                (
                    "backend/".into(),
                    Language::Python,
                    vec!["app/main.py".into()]
                ),
                (
                    "frontend/".into(),
                    Language::TypeScript,
                    vec!["next.config.js".into(), "src/app/page.tsx".into()]
                ),
            ]
        );
        assert_eq!(found(&root, Some(Language::Python)).len(), 1);
        assert!(detect(&root, Some(Language::JavaScript)).is_err());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_nested_package_is_a_project_of_its_own_and_not_its_parents() {
        let root = tree(
            "nested",
            &[
                ("package.json", "{}"),
                ("index.js", ""),
                ("packages/ui/package.json", "{}"),
                ("packages/ui/button.js", ""),
                ("packages/ui/src/tsconfig.json", "{}"),
            ],
        );
        assert_eq!(
            found(&root, None),
            [
                ("".into(), Language::JavaScript, vec!["index.js".into()]),
                (
                    "packages/ui/".into(),
                    Language::JavaScript,
                    vec!["button.js".into()]
                ),
            ]
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_language_asked_for_by_name_needs_no_manifest() {
        let root = tree(
            "loose",
            &[
                ("scripts/run.py", ""),
                ("scripts/lib/util.py", ""),
                ("scripts/menu.js", ""),
            ],
        );
        assert!(detect(&root, None).is_err());
        assert!(detect(&root, Some(Language::TypeScript)).is_err());
        assert_eq!(
            found(&root, Some(Language::Python)),
            [(
                "".into(),
                Language::Python,
                vec!["scripts/lib/util.py".into(), "scripts/run.py".into()]
            )]
        );
        assert_eq!(found(&root, Some(Language::JavaScript)).len(), 1);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
