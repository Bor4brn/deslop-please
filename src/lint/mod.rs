//! The lints. `module-cycle` reads which modules use which, by where code is
//! written; `tangle`, `field-leak` and `shallow-module` read the graph of
//! which things know which, wherever they are written.

mod cycles;
mod leaks;
mod shallow;
mod tangles;

use std::collections::BTreeSet;

use crate::graph::{Graph, ThingId};
use crate::model::{ItemId, Model, NodeId, NodeKind, Reference, Tree};

/// What a reader can hold in mind at once: seven (Miller, "The Magical Number
/// Seven, Plus or Minus Two", 1956). The members of a tangle cannot be learned
/// one at a time, so a tangle of more than seven is too much anywhere; an
/// interface of up to seven things, each with up to seven functions, is small
/// enough to need no justification. A note lists no more than this either.
const HELD_IN_MIND: usize = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Lint {
    ModuleCycle,
    Tangle,
    FieldLeak,
    ShallowModule,
}

impl Lint {
    pub fn name(self) -> &'static str {
        match self {
            Lint::ModuleCycle => "module-cycle",
            Lint::Tangle => "tangle",
            Lint::FieldLeak => "field-leak",
            Lint::ShallowModule => "shallow-module",
        }
    }

    pub fn parse(name: &str) -> Option<Lint> {
        match name {
            "module-cycle" => Some(Lint::ModuleCycle),
            "tangle" => Some(Lint::Tangle),
            "field-leak" => Some(Lint::FieldLeak),
            "shallow-module" => Some(Lint::ShallowModule),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Location {
    pub file: String,
    pub line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub lint: Lint,
    /// Stable identifier used in the allow file.
    pub key: String,
    pub message: String,
    pub location: Option<Location>,
    pub notes: Vec<String>,
}

/// Every lint over `model`, optionally only where `scope`, a crate or module, is concerned.
pub fn check(model: &Model, scope: Option<NodeId>) -> Vec<Diagnostic> {
    let graph = Graph::new(model);
    let mut diagnostics = cycles::check(model, scope);
    diagnostics.extend(tangles::check(model, &graph, scope));
    diagnostics.extend(leaks::check(model, &graph, scope));
    diagnostics.extend(shallow::check(model, &graph, scope));
    diagnostics.sort_by(|a, b| a.lint.cmp(&b.lint).then_with(|| a.key.cmp(&b.key)));
    diagnostics
}

pub fn resolve_node(model: &Model, query: &str) -> Option<NodeId> {
    let tree = &model.tree;
    let normalized = query.replace('-', "_");
    tree.ids().find(|&id| {
        tree.node(id).kind != NodeKind::Workspace
            && (tree.display(id) == query || tree.display(id) == normalized)
    })
}

/// The references by which one member of a circle knows another.
#[derive(Default)]
struct Tie {
    items: BTreeSet<ItemId>,
    sites: BTreeSet<(String, u32)>,
}

impl Tie {
    /// Adds `reference`, which is to `item` or to a wrapper of it.
    fn add(&mut self, model: &Model, item: ItemId, reference: &Reference) {
        self.items.insert(item);
        let path = &model.files[reference.file.index()].path;
        self.sites.insert((path.clone(), reference.line));
    }
}

/// A note for each tie of a circle, by the names of the two it ties, and
/// where the first is. The ties of fewest items come first: they are the
/// cheapest to cut.
fn tie_notes(
    model: &Model,
    verb: &str,
    mut ties: Vec<((String, String), &Tie)>,
) -> (Option<Location>, Vec<String>) {
    ties.sort_by(|(a, tie_a), (b, tie_b)| (tie_a.items.len(), a).cmp(&(tie_b.items.len(), b)));
    let site = |tie: &Tie| {
        let (file, line) = tie.sites.first().expect("a tie has a site");
        Location {
            file: file.clone(),
            line: *line,
        }
    };
    let mut notes: Vec<String> = ties
        .iter()
        .take(HELD_IN_MIND)
        .map(|((user, used), tie)| {
            let items: Vec<String> = tie
                .items
                .iter()
                .map(|item| without_generics(&model.items[item.index()].name))
                .collect();
            format!(
                "{}: `{user}` {verb} `{used}` by {}: {}",
                clickable(&site(tie)),
                counted(tie.items.len(), "item"),
                listed(&items)
            )
        })
        .collect();
    if ties.len() > HELD_IN_MIND {
        notes.push(format!(
            "and {}",
            counted(ties.len() - HELD_IN_MIND, "more tie")
        ));
    }
    (ties.first().map(|(_, tie)| site(tie)), notes)
}

/// `file:line:1`, the shape rustc writes, so terminals and editors link it.
fn clickable(location: &Location) -> String {
    format!("{}:{}:1", location.file, location.line)
}

fn in_scope(tree: &Tree, scope: Option<NodeId>, node: NodeId) -> bool {
    scope.is_none_or(|scope| tree.is_within(node, scope))
}

/// `my_crate::kinds::Ready`: where a thing is defined, and its name there.
fn full_name(model: &Model, graph: &Graph, thing: ThingId) -> String {
    let thing = &graph.things[thing.index()];
    let name = without_generics(&model.items[thing.item.index()].name);
    format!("{}::{name}", model.tree.display(thing.home))
}

/// `a, b, c, +2 more`: at most [`HELD_IN_MIND`] of `names`.
fn listed(names: &[String]) -> String {
    let shown = names
        .iter()
        .take(HELD_IN_MIND)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    match names.len().checked_sub(HELD_IN_MIND) {
        Some(more) if more > 0 => format!("{shown}, +{more} more"),
        _ => shown,
    }
}

/// `1 module`, `2 modules`.
fn counted(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

/// `<Ready as Display>`: a name with generic arguments left out.
fn without_generics(name: &str) -> String {
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

/// The first line of `module`'s primary non-test file, if it has one.
fn module_location(model: &Model, module: NodeId) -> Option<Location> {
    model
        .files
        .iter()
        .filter(|file| !file.test && file.module == module)
        .min_by_key(|file| {
            let path = file.path.as_str();
            let rank = if path.ends_with("/src/lib.rs") || path == "src/lib.rs" {
                0
            } else if path.ends_with("/mod.rs") || path.ends_with("src/main.rs") {
                1
            } else if path.contains("/examples/") {
                3
            } else {
                2
            };
            (rank, path)
        })
        .map(|file| Location {
            file: file.path.clone(),
            line: 1,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_without_its_generic_arguments() {
        assert_eq!(
            without_generics("<Vec<T> as From<(A, B)>>::from"),
            "<Vec as From>::from"
        );
        assert_eq!(without_generics("Ready::new"), "Ready::new");
    }
}
