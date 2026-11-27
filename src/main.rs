//! Architectural linter: enforces deep modules and narrow interfaces in Rust,
//! Python, TypeScript and JavaScript projects, working like `cargo check` /
//! `cargo clippy`.

mod allow;
mod configuration;
mod format;
mod graph;
mod indexing;
mod languages;
mod lint;
mod model;
mod scip;
mod workspace;

use std::fmt;
use std::path::PathBuf;
use std::process::ExitCode;

use allow::AllowList;
use configuration::Configuration;
use languages::Language;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputFormat {
    Text,
    Json,
    Sarif,
}

struct Options {
    scope: Option<String>,
    allow: Option<PathBuf>,
    update_allow: Option<PathBuf>,
    format: OutputFormat,
    /// `None` for every language found.
    language: Option<Language>,
    configuration: Configuration,
}

fn usage() -> String {
    format!(
        "\
usage: {TOOL} [crate_or_mod] [--language <language>] [--allow <path>]
       [--update-allow <path>] [--target <triple>] [-F, --features <features>]
       [--all-features] [--no-default-features] [--json | --sarif]

Runs architectural lints (module-cycle, tangle, field-leak, shallow-module,
wide-directory, long-file) on the projects found here: the Cargo workspace
around this directory, and the Rust, Python, TypeScript and JavaScript
projects below it, each package of a monorepo on its own.

options:
  <crate_or_mod>               check only one crate, package or module
                               (e.g. my-crate, my_crate::store, api::app.models)
  --language <language>        rust, python, typescript, javascript, or auto
                               (the default): which projects to check
  --allow <path>               suppress listed existing debt; new errors still fail
  --update-allow <path>        write current diagnostics to <path>
  --json                       write diagnostics as JSON to stdout
  --sarif                      write diagnostics as SARIF 2.1.0 to stdout
  -V, --version                print the version and exit
  -h, --help                   print this help and exit

configuration, as `cargo check --workspace` takes it:
  --target <triple>            check for the target triple, not the host's
  -F, --features <features>    space or comma separated list of features to
                               activate; may be given more than once
  --all-features               activate all features of every member
  --no-default-features        do not activate the `default` feature

A run checks one configuration: the host's target with default features,
unless these flags or cargo's own configuration (`build.target`, `RUSTFLAGS`)
choose another. Code behind a `cfg` that it leaves off, such as a
`#[cfg(windows)]` module on Linux, is not checked: run once for each
configuration that matters.

These flags apply to the Rust project only.

Rust is indexed by rust-analyzer, Python by scip-python, TypeScript and
JavaScript by scip-typescript; each must be installed for its projects.

A flag's value is the next word, or follows `=`: `--target=<triple>`.
"
    )
}

/// The flags, and whether each takes a value. `-F` is `--features`.
const FLAGS: [(&str, bool); 9] = [
    ("--language", true),
    ("--allow", true),
    ("--update-allow", true),
    ("--target", true),
    ("--features", true),
    ("--all-features", false),
    ("--no-default-features", false),
    ("--json", false),
    ("--sarif", false),
];

/// Reads the command line as cargo reads its own: a flag's value is the next
/// word, or follows `=` in the flag's own (`--target=<triple>`, `-F=a,b`) or
/// `-F` (`-Fa,b`); only `--target` and `--features` may be given more than
/// once.
fn parse_arguments(arguments: &[String]) -> Result<Options, String> {
    let mut options = Options {
        scope: None,
        allow: None,
        update_allow: None,
        format: OutputFormat::Text,
        language: None,
        configuration: Configuration::default(),
    };
    let mut given = Vec::new();
    let mut words = arguments.iter().map(String::as_str).peekable();
    while let Some(word) = words.next() {
        if !word.starts_with('-') {
            if options.scope.replace(word.to_owned()).is_some() {
                return Err(format!("unexpected argument '{word}'"));
            }
            continue;
        }
        let (flag, attached) = split_flag(word);
        let Some(&(flag, takes_value)) = FLAGS.iter().find(|(name, _)| *name == flag) else {
            return Err(format!("unexpected argument '{word}'"));
        };
        if given.contains(&flag) && !matches!(flag, "--target" | "--features") {
            return Err(format!("'{flag}' cannot be given more than once"));
        }
        given.push(flag);
        let value = match (takes_value, attached) {
            (true, Some(value)) => value,
            (true, None) => words
                .next_if(|next| !next.starts_with('-'))
                .ok_or_else(|| format!("'{flag}' needs a value"))?,
            (false, Some(value)) => return Err(format!("'{flag}' takes no value: '{value}'")),
            (false, None) => "",
        };
        let configuration = &mut options.configuration;
        match flag {
            "--language" => options.language = Language::parse(value)?,
            "--allow" => {
                if value.is_empty() {
                    return Err("'--allow' needs a path".to_owned());
                }
                options.allow = Some(PathBuf::from(value));
            }
            "--update-allow" => {
                if value.is_empty() {
                    return Err("'--update-allow' needs a path".to_owned());
                }
                options.update_allow = Some(PathBuf::from(value));
            }
            "--target" => configuration.target(value)?,
            "--features" => configuration.features(value),
            "--all-features" => configuration.all_features(),
            "--no-default-features" => configuration.no_default_features(),
            "--json" => {
                if options.format == OutputFormat::Sarif {
                    return Err("'--json' and '--sarif' cannot be given together".to_owned());
                }
                options.format = OutputFormat::Json;
            }
            "--sarif" => {
                if options.format == OutputFormat::Json {
                    return Err("'--json' and '--sarif' cannot be given together".to_owned());
                }
                options.format = OutputFormat::Sarif;
            }
            _ => unreachable!("{flag} has no meaning"),
        }
    }
    let rust = options
        .language
        .is_none_or(|language| language == Language::Rust);
    if !rust && !options.configuration.is_default() {
        return Err(format!(
            "'{}' applies to Rust only, not to --language {}",
            options.configuration,
            options
                .language
                .map_or_else(String::new, |language| language.to_string())
        ));
    }
    if options.allow.is_some() && options.update_allow.is_some() {
        return Err("'--allow' and '--update-allow' cannot be given together".to_owned());
    }
    if options.update_allow.is_some() {
        match options.format {
            OutputFormat::Text => {}
            OutputFormat::Json => {
                return Err("'--json' and '--update-allow' cannot be given together".to_owned());
            }
            OutputFormat::Sarif => {
                return Err("'--sarif' and '--update-allow' cannot be given together".to_owned());
            }
        }
    }
    Ok(options)
}

/// The flag `word` names, and the value written in it: `--target=<triple>`,
/// and `-F<features>` or `-F=<features>`.
fn split_flag(word: &str) -> (&str, Option<&str>) {
    if let Some(value) = word.strip_prefix("-F") {
        let value = (!value.is_empty()).then(|| value.strip_prefix('=').unwrap_or(value));
        return ("--features", value);
    }
    match word.split_once('=') {
        Some((flag, value)) => (flag, Some(value)),
        None => (word, None),
    }
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
    if matches!(
        arguments.as_slice(),
        [version] if version == "-V" || version == "--version" || version == "version"
    ) {
        println!("{TOOL} {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    let options = match parse_arguments(&arguments) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{TOOL}: {message}");
            eprint!("{}", usage());
            return ExitCode::from(2);
        }
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
    let start = std::env::current_dir()
        .map_err(|e| Error::new(format!("cannot read current directory: {e}")))?;
    let mut projects = languages::detect(&start, options.language)?;
    // Found, not asked for: a project whose indexer is missing does not keep
    // the others, such as the Cargo workspace around it, from being checked.
    if options.language.is_none() && projects.len() > 1 {
        let (ready, unready): (Vec<_>, Vec<_>) = projects
            .into_iter()
            .map(|project| (project.indexer_installed(), project))
            .partition(|(installed, _)| installed.is_ok());
        if ready.is_empty() {
            projects = unready.into_iter().map(|(_, project)| project).collect();
        } else {
            for (installed, _) in unready {
                if let Err(error) = installed {
                    eprintln!("{TOOL}: warning: not checked: {error}");
                }
            }
            projects = ready.into_iter().map(|(_, project)| project).collect();
        }
    }
    let mut all = Vec::new();
    let mut scoped = false;
    for project in &projects {
        if projects.len() > 1 {
            eprintln!("{TOOL}: checking {}", project.describe());
        }
        let model = project.model(&options.configuration)?;
        let scope = match &options.scope {
            Some(query) => match lint::resolve_node(&model, query) {
                Some(node) => Some(node),
                None => continue,
            },
            None => None,
        };
        scoped = true;
        all.extend(lint::check(&model, scope));
    }
    if let (Some(query), false) = (&options.scope, scoped) {
        return Err(Error::new(format!("{query}: no such crate or module")));
    }
    all.sort_by(|a, b| a.lint.cmp(&b.lint).then_with(|| a.key.cmp(&b.key)));

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

    match options.format {
        OutputFormat::Text => print!(
            "{}",
            format::render(&reported, fixed_allowed, allow_shown.as_deref())
        ),
        OutputFormat::Json => print!(
            "{}",
            format::render_json(&reported, fixed_allowed, allow_shown.as_deref())
        ),
        OutputFormat::Sarif => print!("{}", format::render_sarif(&reported)),
    }
    Ok(reported.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(words: &[&str]) -> Result<Options, String> {
        let arguments: Vec<String> = words.iter().map(|&word| word.to_owned()).collect();
        parse_arguments(&arguments)
    }

    fn configuration(words: &[&str]) -> Configuration {
        parsed(words).unwrap().configuration
    }

    #[test]
    fn no_flags_choose_the_default_configuration() {
        let options = parsed(&["my-crate"]).unwrap();
        assert_eq!(options.scope.as_deref(), Some("my-crate"));
        assert_eq!((options.allow, options.update_allow), (None, None));
        assert_eq!(options.format, OutputFormat::Text);
        assert!(options.configuration.is_default());
    }

    #[test]
    fn a_value_follows_its_flag_or_an_equals_sign_in_it() {
        let spaced = [
            "my-crate",
            "--allow",
            "a.allow",
            "--target",
            "aarch64-apple-darwin",
        ];
        let joined = [
            "--allow=a.allow",
            "--target=aarch64-apple-darwin",
            "my-crate",
        ];
        for words in [&spaced[..], &joined[..]] {
            let options = parsed(words).unwrap();
            assert_eq!(options.scope.as_deref(), Some("my-crate"));
            assert_eq!(options.allow, Some(PathBuf::from("a.allow")));
            assert_eq!(
                options.configuration.to_string(),
                "--target aarch64-apple-darwin"
            );
        }
        for words in [
            &["--update-allow", "b.allow"][..],
            &["--update-allow=b.allow"],
        ] {
            let options = parsed(words).unwrap();
            assert_eq!(options.update_allow, Some(PathBuf::from("b.allow")));
        }
    }

    #[test]
    fn features_are_written_as_cargo_takes_them() {
        let expected = configuration(&["--features", "a,b"]);
        for words in [
            &["--features", "a b"][..],
            &["--features= a , b "],
            &["--features", "a", "--features", "b"],
            &["-F", "a", "-Fb"],
            &["-F=a,b"],
        ] {
            assert_eq!(configuration(words), expected, "{words:?}");
        }
        assert!(configuration(&["--features="]).is_default());
        assert_eq!(
            configuration(&["--no-default-features", "--all-features"]).to_string(),
            "--all-features --no-default-features"
        );
    }

    #[test]
    fn a_language_is_chosen_by_name_or_found() {
        assert_eq!(parsed(&[]).unwrap().language, None);
        assert_eq!(parsed(&["--language", "auto"]).unwrap().language, None);
        assert_eq!(
            parsed(&["--language=python", "api"]).unwrap().language,
            Some(Language::Python)
        );
        let rust = parsed(&["--language", "rust", "--target", "aarch64-apple-darwin"]).unwrap();
        assert_eq!(rust.language, Some(Language::Rust));
        assert!(parsed(&["--all-features"]).is_ok());
    }

    #[test]
    fn json_and_sarif_flags_are_parsed() {
        let options = parsed(&["--json", "my-crate"]).unwrap();
        assert_eq!(options.format, OutputFormat::Json);
        assert_eq!(options.scope.as_deref(), Some("my-crate"));
        assert!(parsed(&["--json", "--json"]).is_err());

        let options = parsed(&["--sarif", "my-crate"]).unwrap();
        assert_eq!(options.format, OutputFormat::Sarif);
        assert_eq!(options.scope.as_deref(), Some("my-crate"));
        assert!(parsed(&["--sarif", "--sarif"]).is_err());
        assert!(parsed(&["--json", "--sarif"]).is_err());
        assert!(parsed(&["--sarif", "--json"]).is_err());
    }

    #[test]
    fn two_targets_are_an_error_since_rust_analyzer_indexes_one() {
        let two = [
            "--target",
            "aarch64-apple-darwin",
            "--target=x86_64-pc-windows-msvc",
        ];
        let error = parsed(&two).err().unwrap();
        assert!(error.contains("one target per run"), "{error}");
        let same = [
            "--target",
            "aarch64-apple-darwin",
            "--target=aarch64-apple-darwin",
        ];
        assert_eq!(
            configuration(&same).to_string(),
            "--target aarch64-apple-darwin"
        );
    }

    #[test]
    fn what_cargo_would_reject_is_an_error() {
        for words in [
            &["--target"][..],
            &["--target="],
            &["--features"],
            &["--target", "--features", "a"],
            &["--all-features", "--all-features"],
            &["--no-default-features=true"],
            &["--allow", "a", "--allow", "b"],
            &["--allow", "a", "--update-allow", "b"],
            &["--allow="],
            &["--update-allow="],
            &["--json", "--update-allow", "a"],
            &["--sarif", "--update-allow", "a"],
            &["--sarif=true"],
            &["--release"],
            &["-p", "my-crate"],
            &["my-crate", "other-crate"],
            &["--language", "go"],
            &["--language"],
            &["--language", "rust", "--language", "python"],
            &["--language", "python", "--target", "aarch64-apple-darwin"],
            &["--language=typescript", "--all-features"],
        ] {
            assert!(parsed(words).is_err(), "{words:?}");
        }
    }
}
