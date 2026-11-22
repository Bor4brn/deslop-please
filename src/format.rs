//! `rustc`/`clippy`-style diagnostic formatting.

use std::fmt::Write;

use crate::TOOL;
use crate::lint::Diagnostic;

pub fn render(
    diagnostics: &[Diagnostic],
    fixed_allowed: usize,
    allow_path: Option<&str>,
) -> String {
    let mut out = String::new();
    for diag in diagnostics {
        let _ = writeln!(out, "error[{}]: {}", diag.lint.name(), diag.message);
        if let Some(loc) = &diag.location {
            // rustc shape (`file:line:column`) so IDE terminals link-navigate.
            let _ = writeln!(out, "  --> {}:{}:1", loc.file, loc.line);
        }
        for note in &diag.notes {
            let _ = writeln!(out, "  = note: {note}");
        }
        if !diag.help.is_empty() {
            let _ = writeln!(out, "  = help: {}", diag.help);
        }
        out.push('\n');
    }

    if fixed_allowed > 0
        && let Some(path) = allow_path
    {
        let noun = if fixed_allowed == 1 {
            "entry"
        } else {
            "entries"
        };
        let _ = writeln!(
            out,
            "note: {fixed_allowed} allowed {noun} in {path} are now fixed (run `{TOOL} --update-allow {path}` to tighten it)"
        );
    }

    if diagnostics.is_empty() {
        let _ = writeln!(out, "{TOOL}: ok");
    } else {
        let noun = if diagnostics.len() == 1 {
            "error"
        } else {
            "errors"
        };
        let _ = writeln!(out, "{TOOL}: {} {noun}, well done!", diagnostics.len());
    }
    out
}
