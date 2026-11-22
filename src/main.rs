//! Architectural linter for Rust workspaces: enforces deep modules and narrow
//! interfaces, working like `cargo check` / `cargo clippy`.

mod allow;
mod format;
mod indexing;
mod lint;
mod model;
mod scip;
mod workspace;

use std::fmt;
use std::path::PathBuf;
use std::process::ExitCode;

use allow::AllowList;
use model::Model;
use workspace::Workspace;

pub const TOOL: &str = env!("CARGO_BIN_NAME");

#[derive(Debug)]
pub struct Error(String);

impl Error {
    pub fn new(message: impl Into<String>) -> Error {
        Error(message.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

struct Options {
    scope: Option<String>,
    allow: Option<PathBuf>,
    update_allow: Option<PathBuf>,
}

fn usage() -> String {
    format!(
        "usage: {TOOL} [crate_or_mod] [--allow <path>] [--update-allow <path>]\n\
         \n\
         Runs architectural lints (module-cycle, split-module, facade-bypass,\n\
         field-leak, chatty-seam, shallow-module) on the Cargo workspace.\n\
         \n\
         options:\n\
           <crate_or_mod>          check only one crate or module (e.g. my-crate)\n\
           --allow <path>          suppress listed existing debt; new errors still fail\n\
           --update-allow <path>   write current diagnostics to <path>\n"
    )
}

fn parse_arguments(arguments: &[String]) -> Option<Options> {
    let words: Vec<&str> = arguments.iter().map(String::as_str).collect();
    let mut scope = None;
    let mut allow = None;
    let mut update_allow = None;
    let mut i = 0;
    while i < words.len() {
        match words[i] {
            "--allow" => {
                allow = Some(PathBuf::from(words.get(i + 1)?));
                i += 2;
            }
            "--update-allow" => {
                update_allow = Some(PathBuf::from(words.get(i + 1)?));
                i += 2;
            }
            arg if !arg.starts_with('-') && scope.is_none() => {
                scope = Some(arg.to_owned());
                i += 1;
            }
            _ => return None,
        }
    }
    if allow.is_some() && update_allow.is_some() {
        return None;
    }
    Some(Options {
        scope,
        allow,
        update_allow,
    })
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if matches!(
        arguments.as_slice(),
        [help] if help == "-h" || help == "--help" || help == "help"
    ) {
        print!("{}", usage());
        return ExitCode::SUCCESS;
    }
    let Some(options) = parse_arguments(&arguments) else {
        eprint!("{}", usage());
        return ExitCode::from(2);
    };
    match run(options) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("{TOOL}: {error}");
            ExitCode::from(2)
        }
    }
}

fn run(options: Options) -> Result<bool, Error> {
    let workspace = Workspace::discover()?;
    let model = load_model(&workspace)?;
    let scope = match &options.scope {
        Some(query) => Some(
            lint::resolve_node(&model, query)
                .ok_or_else(|| Error::new(format!("{query}: no such crate or module")))?,
        ),
        None => None,
    };
    let all = lint::check(&model, scope);

    if let Some(path) = options.update_allow {
        let allow = AllowList::from_diagnostics(&all);
        allow.write(&path)?;
        eprintln!("{TOOL}: wrote {} entries to {}", all.len(), path.display());
        return Ok(true);
    }

    let (reported, fixed_allowed, allow_shown) = match &options.allow {
        Some(path) => {
            let allow = AllowList::read(path)?;
            let fixed = allow.fixed_count(&all);
            let active: Vec<_> = all.into_iter().filter(|d| !allow.is_allowed(d)).collect();
            (active, fixed, Some(path.display().to_string()))
        }
        None => (all, 0, None),
    };

    print!(
        "{}",
        format::render(&reported, fixed_allowed, allow_shown.as_deref())
    );
    Ok(reported.is_empty())
}

fn load_model(workspace: &Workspace) -> Result<Model, Error> {
    let bytes = indexing::index(workspace)?;
    let index =
        scip::read(&bytes).map_err(|e| Error::new(format!("rust-analyzer's index: {e}")))?;
    model::build(&index, workspace)
}
