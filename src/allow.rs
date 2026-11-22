//! Plain-text allowlist for existing architectural debt.
//! One `<lint> <key>` per line, sorted. Fixing an allowed entry never fails the check.

use std::collections::BTreeSet;
use std::path::Path;

use crate::lint::{Diagnostic, Lint};
use crate::{Error, TOOL};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct AllowList {
    entries: BTreeSet<(Lint, String)>,
}

impl AllowList {
    pub fn from_diagnostics(diagnostics: &[Diagnostic]) -> AllowList {
        AllowList {
            entries: diagnostics
                .iter()
                .map(|d| (d.lint, d.key.clone()))
                .collect(),
        }
    }

    pub fn read(path: &Path) -> Result<AllowList, Error> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| Error::new(format!("{}: {e}", path.display())))?;
        AllowList::parse(&text).map_err(|line| {
            Error::new(format!(
                "{}:{line}: invalid allowlist entry",
                path.display()
            ))
        })
    }

    pub fn write(&self, path: &Path) -> Result<(), Error> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Error::new(format!("{}: {e}", parent.display())))?;
        }
        std::fs::write(path, self.to_text())
            .map_err(|e| Error::new(format!("{}: {e}", path.display())))
    }

    pub fn parse(text: &str) -> Result<AllowList, usize> {
        let mut entries = BTreeSet::new();
        for (index, line) in text.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let Some((lint_name, key)) = trimmed.split_once(' ') else {
                return Err(index + 1);
            };
            let Some(lint) = Lint::parse(lint_name) else {
                return Err(index + 1);
            };
            if key.trim().is_empty() {
                return Err(index + 1);
            }
            entries.insert((lint, key.trim().to_owned()));
        }
        Ok(AllowList { entries })
    }

    pub fn to_text(&self) -> String {
        let mut out = format!(
            "# Existing architectural debt suppressed by `{TOOL} --allow`.\n\
             # Fixing an entry here never fails CI; new issues not listed here fail.\n",
        );
        for (lint, key) in &self.entries {
            out += &format!("{} {key}\n", lint.name());
        }
        out
    }

    pub fn is_allowed(&self, diagnostic: &Diagnostic) -> bool {
        if self
            .entries
            .contains(&(diagnostic.lint, diagnostic.key.clone()))
        {
            return true;
        }
        // Shrinking an allowed cycle is an improvement, never a new error.
        if diagnostic.lint == Lint::ModuleCycle
            && let Some((parent, members)) = parse_cycle_key(&diagnostic.key)
        {
            return self.entries.iter().any(|(lint, key)| {
                *lint == Lint::ModuleCycle
                    && parse_cycle_key(key).is_some_and(|(allowed_parent, allowed_members)| {
                        allowed_parent == parent
                            && members.iter().all(|m| allowed_members.contains(m))
                    })
            });
        }
        false
    }

    /// Allowed entries that are no longer present in `diagnostics`.
    pub fn fixed_count(&self, diagnostics: &[Diagnostic]) -> usize {
        self.entries
            .iter()
            .filter(|(lint, key)| {
                !diagnostics.iter().any(|d| {
                    if d.lint != *lint {
                        return false;
                    }
                    if d.key == *key {
                        return true;
                    }
                    *lint == Lint::ModuleCycle
                        && parse_cycle_key(key)
                            .zip(parse_cycle_key(&d.key))
                            .is_some_and(|((ap, am), (dp, dm))| {
                                ap == dp && dm.iter().all(|m| am.contains(m))
                            })
                })
            })
            .count()
    }
}

fn parse_cycle_key(key: &str) -> Option<(&str, Vec<&str>)> {
    let (parent, members) = key.split_once(' ')?;
    Some((parent, members.split(',').collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shrinking_allowed_cycle_stays_allowed() {
        let allow = AllowList::parse("module-cycle a x,y,z\n").unwrap();
        let shrunk = Diagnostic {
            lint: Lint::ModuleCycle,
            key: "a x,y".into(),
            message: String::new(),
            location: None,
            notes: vec![],
            help: String::new(),
        };
        let expanded = Diagnostic {
            lint: Lint::ModuleCycle,
            key: "a w,x,y,z".into(),
            message: String::new(),
            location: None,
            notes: vec![],
            help: String::new(),
        };
        assert!(allow.is_allowed(&shrunk));
        assert!(!allow.is_allowed(&expanded));
    }
}
