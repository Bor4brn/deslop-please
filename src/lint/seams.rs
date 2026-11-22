//! `chatty-seam` and `shallow-module`:
//! - `chatty-seam`: one boundary calling too many distinct functions/methods
//!   of another boundary (logic split across the wrong side of the seam).
//! - `shallow-module`: a boundary that exposes many functions/methods while
//!   hiding very little behavior internally.
//!
//! Both count through forwarders, as through a `pub use`: a forwarder is not
//! a function of its own, a call to it is a call to the function at the end
//! of its chain, and the call it makes is not counted.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::{Diagnostic, Lint, Location, boundaries, home_boundary, in_scope, without_generics};
use crate::model::{ItemId, ItemKind, Model, NodeId, Reference};

/// Calling this many distinct functions/methods on a single other boundary
/// signals a chatty seam where logic should be pushed behind a narrower API.
const CHATTY_SEAM_FUNCTIONS: usize = 25;

/// A boundary with at least this many externally used functions/methods that
/// hides less than `SHALLOW_MAX_HIDDEN_PERCENT`% of its defined functions is shallow.
const SHALLOW_MIN_EXTERNAL_FUNCTIONS: usize = 8;
const SHALLOW_MAX_HIDDEN_PERCENT: usize = 35;

/// How many names a note shows before it says how many more there are.
const NOTE_NAMES: usize = 6;

pub fn check(model: &Model, scope: Option<NodeId>) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    diagnostics.extend(check_chatty_seams(model, scope));
    diagnostics.extend(check_shallow_modules(model, scope));
    diagnostics
}

struct Call<'m> {
    reference: &'m Reference,
    /// The function that does the work behind the one the reference names.
    worker: ItemId,
}

impl Call<'_> {
    fn forwarder(&self) -> Option<ItemId> {
        (self.worker != self.reference.item).then_some(self.reference.item)
    }
}

/// The references of non-test code, seen through forwarders.
fn calls(model: &Model) -> impl Iterator<Item = Call<'_>> {
    model
        .references()
        .iter()
        .filter(|reference| {
            !reference.test
                && !reference
                    .from_function
                    .is_some_and(|function| model.is_forwarder(function))
        })
        .map(|reference| Call {
            reference,
            worker: model.worker(reference.item),
        })
}

/// `a, b, c, +2 more`: the distinct names of `items`, sorted.
fn names(model: &Model, items: &BTreeSet<ItemId>) -> String {
    let mut names: Vec<String> = items
        .iter()
        .map(|&id| without_generics(&model.item(id).name))
        .collect();
    names.sort();
    names.dedup();
    let shown = names
        .iter()
        .take(NOTE_NAMES)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if names.len() > NOTE_NAMES {
        format!("{shown}, +{} more", names.len() - NOTE_NAMES)
    } else {
        shown
    }
}

fn forwarders_note(model: &Model, forwarders: &BTreeSet<ItemId>) -> Option<String> {
    (!forwarders.is_empty()).then(|| {
        format!(
            "seen through {} forwarders: {}",
            forwarders.len(),
            names(model, forwarders)
        )
    })
}

fn check_chatty_seams(model: &Model, scope: Option<NodeId>) -> Vec<Diagnostic> {
    let tree = model.tree();
    let bounds = boundaries(tree);
    #[derive(Default)]
    struct Seam {
        fns: BTreeSet<ItemId>,
        forwarders: BTreeSet<ItemId>,
        refs: u32,
        first: Option<(String, u32)>,
    }
    let mut seams: BTreeMap<(NodeId, NodeId), Seam> = BTreeMap::new();
    for call in calls(model) {
        let item = model.item(call.worker);
        if item.kind != ItemKind::Function || item.is_trait_impl_item || tree.is_test(item.owner) {
            continue;
        }
        let reference = call.reference;
        let from = home_boundary(tree, &bounds, reference.from);
        let to = home_boundary(tree, &bounds, item.owner);
        if from == to || tree.is_within(from, to) || tree.is_within(to, from) {
            continue;
        }
        let path = model.file(reference.file).path.as_str();
        let entry = seams.entry((from, to)).or_default();
        entry.fns.insert(call.worker);
        entry.forwarders.extend(call.forwarder());
        entry.refs += 1;
        let site = (path.to_owned(), reference.line);
        if entry
            .first
            .as_ref()
            .is_none_or(|prev| (&site.0, site.1) < (&prev.0, prev.1))
        {
            entry.first = Some(site);
        }
    }

    seams
        .into_iter()
        .filter(|&((from, to), ref seam)| {
            seam.fns.len() >= CHATTY_SEAM_FUNCTIONS
                && (in_scope(tree, scope, from) || in_scope(tree, scope, to))
        })
        .map(|((from, to), seam)| {
            let from_str = tree.display(from);
            let to_str = tree.display(to);
            let location = seam
                .first
                .map(|(file, line)| Location { file, line })
                .or_else(|| {
                    model.module_file(from).map(|f| Location {
                        file: f.to_owned(),
                        line: 1,
                    })
                });
            let mut notes = vec![format!("calls: {}", names(model, &seam.fns))];
            notes.extend(forwarders_note(model, &seam.forwarders));
            Diagnostic {
                lint: Lint::ChattySeam,
                key: format!("{from_str} -> {to_str}"),
                message: format!(
                    "wide seam `{from_str}` -> `{to_str}`: calls {} distinct functions/methods ({} references)",
                    seam.fns.len(),
                    seam.refs
                ),
                location,
                notes,
                help: format!(
                    "push coordination logic down into `{to_str}` behind fewer, higher-level operations"
                ),
            }
        })
        .collect()
}

fn check_shallow_modules(model: &Model, scope: Option<NodeId>) -> Vec<Diagnostic> {
    let tree = model.tree();
    let bounds = boundaries(tree);
    let is_defined_fn = |id: ItemId| {
        let item = model.item(id);
        item.kind == ItemKind::Function
            && !item.is_trait_impl_item
            && !tree.is_test(item.owner)
            && !item.file.is_some_and(|f| model.file(f).test)
    };

    let mut defined: HashMap<NodeId, usize> = HashMap::new();
    let mut forwarders: HashMap<NodeId, BTreeSet<ItemId>> = HashMap::new();
    for (id, item) in model.items() {
        if !is_defined_fn(id) {
            continue;
        }
        for ancestor in tree.ancestors(item.owner) {
            if !bounds.contains(&ancestor) {
                continue;
            }
            if model.is_forwarder(id) {
                forwarders.entry(ancestor).or_default().insert(id);
            } else {
                *defined.entry(ancestor).or_default() += 1;
            }
        }
    }

    let mut used_external: HashMap<NodeId, BTreeSet<ItemId>> = HashMap::new();
    for call in calls(model) {
        if !is_defined_fn(call.worker) {
            continue;
        }
        let owner = model.item(call.worker).owner;
        let ancestor = tree.common_ancestor(owner, call.reference.from);
        for node in tree.ancestors(owner).take_while(|&n| n != ancestor) {
            if bounds.contains(&node) {
                used_external.entry(node).or_default().insert(call.worker);
                forwarders.entry(node).or_default().extend(call.forwarder());
            }
        }
    }

    let mut out = Vec::new();
    for &boundary in &bounds {
        if !in_scope(tree, scope, boundary) {
            continue;
        }
        let total = defined.get(&boundary).copied().unwrap_or(0);
        let external = used_external.get(&boundary).map_or(0, BTreeSet::len);
        if external < SHALLOW_MIN_EXTERNAL_FUNCTIONS || total == 0 {
            continue;
        }
        let hidden = total.saturating_sub(external);
        let hidden_pct = hidden * 100 / total;
        if hidden_pct >= SHALLOW_MAX_HIDDEN_PERCENT {
            continue;
        }
        let name = tree.display(boundary);
        let notes = forwarders
            .get(&boundary)
            .and_then(|forwarders| forwarders_note(model, forwarders))
            .into_iter()
            .collect();
        out.push(Diagnostic {
            lint: Lint::ShallowModule,
            key: name.clone(),
            message: format!(
                "`{name}` is shallow: {external}/{total} functions/methods are called from outside (only {hidden_pct}% hidden)"
            ),
            location: model.module_file(boundary).map(|f| Location {
                file: f.to_owned(),
                line: 1,
            }),
            notes,
            help: format!(
                "narrow `{name}`'s public API so more logic lives behind it, or dissolve it into its parent/caller"
            ),
        });
    }
    out
}
