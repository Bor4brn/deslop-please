//! Which configuration of the workspace rust-analyzer indexes: a target and
//! the features, chosen by the flags `cargo check` takes. It indexes one per
//! run, the host's target with default features unless the flags say
//! otherwise, and reads nothing behind a `cfg` that configuration leaves off.

use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

use crate::Error;

/// `--target`, `--features`, `--all-features` and `--no-default-features`, as
/// `cargo check --workspace` reads them: a feature is turned on in every
/// member that has it, and `member/feature` in that member only.
#[derive(Debug, Default, PartialEq, Eq, Hash)]
pub struct Configuration {
    target: Option<String>,
    features: BTreeSet<String>,
    all_features: bool,
    no_default_features: bool,
}

impl Configuration {
    /// `--target <triple>`. Cargo takes several targets; rust-analyzer
    /// indexes one.
    pub fn target(&mut self, triple: &str) -> Result<(), String> {
        if triple.is_empty() {
            return Err("'--target' needs a target triple".to_owned());
        }
        match &self.target {
            Some(other) if other != triple => Err(format!(
                "'--target {other}' and '--target {triple}': rust-analyzer indexes one target \
                 per run, so run once for each"
            )),
            _ => {
                self.target = Some(triple.to_owned());
                Ok(())
            }
        }
    }

    /// `--features <features>`: separated by commas or spaces, as cargo
    /// takes them.
    pub fn features(&mut self, list: &str) {
        let names = list
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|name| !name.is_empty());
        self.features.extend(names.map(str::to_owned));
    }

    pub fn all_features(&mut self) {
        self.all_features = true;
    }

    pub fn no_default_features(&mut self) {
        self.no_default_features = true;
    }

    /// The host's target with default features: what no flag changes.
    pub fn is_default(&self) -> bool {
        *self == Configuration::default()
    }

    /// The settings of rust-analyzer that choose it; the default has none.
    pub fn settings(&self) -> Value {
        let mut cargo = serde_json::Map::new();
        if let Some(target) = &self.target {
            cargo.insert("target".to_owned(), json!(target));
        }
        if self.all_features {
            cargo.insert("features".to_owned(), json!("all"));
        } else if !self.features.is_empty() {
            cargo.insert("features".to_owned(), json!(self.features));
        }
        if self.no_default_features {
            cargo.insert("noDefaultFeatures".to_owned(), json!(true));
        }
        if cargo.is_empty() {
            json!({})
        } else {
            json!({ "cargo": cargo })
        }
    }

    /// Whether cargo takes it for the workspace at `root`, by asking
    /// `cargo metadata`, which writes `Cargo.lock` when `cargo check` would.
    /// rust-analyzer takes any target and features: when cargo rejects them,
    /// it indexes the workspace with no features at all and says nothing.
    pub fn check(&self, root: &Path) -> Result<(), Error> {
        if self.is_default() {
            return Ok(());
        }
        let output = Command::new("cargo")
            .args(["metadata", "--format-version", "1"])
            .args(self.flags("--filter-platform"))
            .current_dir(root)
            .stdout(Stdio::null())
            .output()
            .map_err(|e| Error::new(format!("cannot run cargo: {e}")))?;
        if output.status.success() {
            return Ok(());
        }
        Err(Error::new(format!(
            "cannot index with `{self}`: cargo metadata says: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }

    /// The flags that choose it, with `target` for `--target`.
    fn flags(&self, target: &str) -> Vec<String> {
        let mut flags = Vec::new();
        if let Some(triple) = &self.target {
            flags.extend([target.to_owned(), triple.clone()]);
        }
        if !self.features.is_empty() {
            let features: Vec<&str> = self.features.iter().map(String::as_str).collect();
            flags.extend(["--features".to_owned(), features.join(",")]);
        }
        if self.all_features {
            flags.push("--all-features".to_owned());
        }
        if self.no_default_features {
            flags.push("--no-default-features".to_owned());
        }
        flags
    }
}

/// `--target x86_64-pc-windows-msvc --features a,b`: the flags that choose it.
impl fmt::Display for Configuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.flags("--target").join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn features_are_separated_by_commas_or_spaces_and_given_any_number_of_times() {
        let mut configuration = Configuration::default();
        configuration.features("b, a  member/c,,");
        configuration.features("d");
        configuration.features("");
        assert_eq!(
            configuration.settings()["cargo"]["features"],
            json!(["a", "b", "d", "member/c"])
        );
        assert_eq!(configuration.to_string(), "--features a,b,d,member/c");
    }

    #[test]
    fn the_default_adds_no_setting() {
        let configuration = Configuration::default();
        assert!(configuration.is_default());
        assert_eq!(configuration.settings(), json!({}));
        assert_eq!(configuration.to_string(), "");
    }

    #[test]
    fn each_flag_is_a_setting_of_rust_analyzer() {
        let mut configuration = Configuration::default();
        configuration.target("x86_64-pc-windows-msvc").unwrap();
        configuration.features("a");
        configuration.all_features();
        configuration.no_default_features();
        assert_eq!(
            configuration.settings(),
            json!({"cargo": {
                "target": "x86_64-pc-windows-msvc",
                "features": "all",
                "noDefaultFeatures": true,
            }})
        );
        assert_eq!(
            configuration.to_string(),
            "--target x86_64-pc-windows-msvc --features a --all-features --no-default-features"
        );
    }

    #[test]
    fn one_target_and_not_two() {
        let mut configuration = Configuration::default();
        configuration.target("aarch64-apple-darwin").unwrap();
        configuration.target("aarch64-apple-darwin").unwrap();
        let two = configuration.target("x86_64-pc-windows-msvc").unwrap_err();
        assert!(two.contains("one target per run"), "{two}");
        assert!(configuration.target("").is_err());
    }
}
