//! Architectural lint rules enforcing deep modules and narrow interfaces.

mod bypasses;
mod cycles;
mod leaks;
mod seams;

use std::collections::HashSet;

use crate::model::{Model, NodeId, NodeKind, Tree};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Lint {
    ModuleCycle,
    SplitModule,
    FacadeBypass,
    FieldLeak,
    ChattySeam,
    ShallowModule,
}

impl Lint {
    pub fn name(self) -> &'static str {
        match self {
            Lint::ModuleCycle => "module-cycle",
            Lint::SplitModule => "split-module",
            Lint::FacadeBypass => "facade-bypass",
            Lint::FieldLeak => "field-leak",
            Lint::ChattySeam => "chatty-seam",
            Lint::ShallowModule => "shallow-module",
        }
    }

    pub fn parse(name: &str) -> Option<Lint> {
        match name {
            "module-cycle" => Some(Lint::ModuleCycle),
            "split-module" => Some(Lint::SplitModule),
            "facade-bypass" => Some(Lint::FacadeBypass),
            "field-leak" => Some(Lint::FieldLeak),
            "chatty-seam" => Some(Lint::ChattySeam),
            "shallow-module" => Some(Lint::ShallowModule),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub file: String,
    pub line: u32,
}

/// `file:line:1` — rustc-shaped so terminals/IDEs make it clickable.
pub(super) fn clickable(file: &str, line: u32) -> String {
    format!("{file}:{line}:1")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub lint: Lint,
    /// Stable identifier used in the allow file.
    pub key: String,
    pub message: String,
    pub location: Option<Location>,
    pub notes: Vec<String>,
    pub help: String,
}

/// Run every architectural lint over `model`, optionally scoped to a single crate or module.
pub fn check(model: &Model, scope: Option<NodeId>) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    diagnostics.extend(cycles::check(model, scope));
    diagnostics.extend(bypasses::check(model, scope));
    diagnostics.extend(leaks::check(model, scope));
    diagnostics.extend(seams::check(model, scope));
    diagnostics.sort_by(|a, b| a.lint.cmp(&b.lint).then_with(|| a.key.cmp(&b.key)));
    diagnostics
}

pub fn resolve_node(model: &Model, query: &str) -> Option<NodeId> {
    let tree = model.tree();
    let normalized = query.replace('-', "_");
    tree.ids().find(|&id| {
        tree.node(id).kind != NodeKind::Workspace
            && (tree.display(id) == query || tree.display(id) == normalized)
    })
}

/// Non-test packages and non-folder directory modules (`mod.rs` with child modules).
pub(super) fn boundaries(tree: &Tree) -> HashSet<NodeId> {
    let mut has_children: HashSet<NodeId> = HashSet::new();
    for id in tree.ids() {
        if let Some(parent) = tree.parent(id).filter(|_| !tree.is_test(id)) {
            has_children.insert(parent);
        }
    }
    tree.ids()
        .filter(|&id| {
            let node = tree.node(id);
            !tree.is_test(id)
                && match node.kind {
                    NodeKind::Package => true,
                    NodeKind::Module => !node.folder && has_children.contains(&id),
                    NodeKind::Workspace => false,
                }
        })
        .collect()
}

pub(super) fn home_boundary(tree: &Tree, bounds: &HashSet<NodeId>, node: NodeId) -> NodeId {
    tree.ancestors(node)
        .find(|id| bounds.contains(id))
        .unwrap_or(node)
}

pub(super) fn in_scope(tree: &Tree, scope: Option<NodeId>, node: NodeId) -> bool {
    scope.is_none_or(|s| tree.is_within(node, s))
}

pub(super) fn without_generics(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut depth = 0;
    for c in name.chars() {
        match c {
            '<' if depth > 0 || out.ends_with(|c: char| c.is_alphanumeric() || c == '_') => {
                depth += 1
            }
            '>' if depth > 0 => depth -= 1,
            _ if depth > 0 => {}
            _ => out.push(c),
        }
    }
    out
}

pub(super) fn relative(tree: &Tree, node: NodeId, ancestor: NodeId) -> String {
    tree.levels_below(ancestor, node)
        .iter()
        .map(|&level| tree.node(level).name.clone())
        .collect::<Vec<_>>()
        .join("::")
}
