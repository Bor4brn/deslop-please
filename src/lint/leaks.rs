//! `field-leak`: code that works on a field of a type it is not part of. A
//! struct guards its fields when it keeps at least one of them to itself and
//! has a method of its own with a `self` parameter, not a wrapper, that uses
//! them; then each of its fields that an outsider writes, borrows, indexes or
//! calls a method on, directly or through a getter or setter, is known
//! outside it. Reading a value is no leak, with or without a getter: a copy
//! of a field, or a borrow of plain data. Whoever reads a value depends on
//! what it is, not on how the struct keeps it. Neither is calling a method
//! that passes the call on to a field, whose callers never see the field.
//! Its insiders are its own definition and methods, wherever their `impl`
//! blocks are, and the things in its tangle. Visibility keywords play no
//! part.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use super::{
    Diagnostic, HELD_IN_MIND, Lint, Location, clickable, counted, in_scope, without_generics,
};
use crate::graph::{Graph, ThingId, Use};
use crate::model::{ItemId, ItemKind, Model, NodeId};

pub fn check(model: &Model, graph: &Graph, scope: Option<NodeId>) -> Vec<Diagnostic> {
    let mut fields: BTreeMap<ThingId, Vec<ItemId>> = BTreeMap::new();
    for (index, item) in model.items.iter().enumerate() {
        let of_struct = item
            .parent
            .is_some_and(|parent| model.items[parent.index()].kind == ItemKind::Type);
        if item.kind == ItemKind::Field
            && of_struct
            && let Some(thing) = graph.thing_of[index]
        {
            fields.entry(thing).or_default().push(ItemId::new(index));
        }
    }
    let mut outside: HashMap<ItemId, Vec<&Use>> = HashMap::new();
    let mut working: HashSet<ThingId> = HashSet::new();
    for usage in &graph.uses {
        let field = &model.items[usage.item.index()];
        let Some(owner) =
            graph.thing_of[usage.item.index()].filter(|_| field.kind == ItemKind::Field)
        else {
            continue;
        };
        let tangle = graph.tangle_of[owner.index()];
        let insider = usage.user.is_some_and(|user| {
            user == owner || (tangle.is_some() && graph.tangle_of[user.index()] == tangle)
        });
        if !insider {
            outside.entry(usage.item).or_default().push(usage);
        } else if usage.user == Some(owner) && works(model, usage) {
            working.insert(owner);
        }
    }
    let mut diagnostics = Vec::new();
    for (thing, fields) in fields {
        let kept = fields.iter().any(|field| !outside.contains_key(field));
        if !kept || !working.contains(&thing) {
            continue;
        }
        for field in fields {
            let uses: Vec<&Use> = outside
                .get(&field)
                .into_iter()
                .flatten()
                .copied()
                .filter(|usage| !usage.copies)
                .collect();
            if uses.is_empty() {
                continue;
            }
            let home = graph.things[thing.index()].home;
            let concerned = in_scope(&model.tree, scope, home)
                || uses.iter().any(|usage| {
                    in_scope(&model.tree, scope, model.references[usage.reference].from)
                });
            if concerned {
                diagnostics.push(diagnostic(model, graph, thing, field, &uses));
            }
        }
    }
    diagnostics
}

/// Whether `usage` is by a method with a `self` parameter: one that works on
/// its type's fields.
fn works(model: &Model, usage: &Use) -> bool {
    model.references[usage.reference]
        .holder
        .map(|holder| &model.items[holder.index()])
        .is_some_and(|holder| holder.kind == ItemKind::Function && holder.takes_self)
}

fn diagnostic(
    model: &Model,
    graph: &Graph,
    thing: ThingId,
    field: ItemId,
    uses: &[&Use],
) -> Diagnostic {
    let tree = &model.tree;
    let mut sites: BTreeMap<Location, String> = BTreeMap::new();
    let mut modules: BTreeSet<NodeId> = BTreeSet::new();
    for usage in uses {
        let reference = &model.references[usage.reference];
        modules.insert(reference.from);
        let user = match reference.holder {
            Some(holder) => {
                let holder = &model.items[holder.index()];
                format!(
                    "{}::{}",
                    tree.display(holder.owner),
                    without_generics(&holder.name)
                )
            }
            None => tree.display(reference.from),
        };
        let through = usage.through.map_or(String::new(), |wrapper| {
            let name = without_generics(&model.items[wrapper.index()].name);
            format!(", through `{name}`")
        });
        let at = Location {
            file: model.files[reference.file.index()].path.clone(),
            line: reference.line,
        };
        sites.entry(at).or_insert(format!("`{user}`{through}"));
    }
    let owner = &graph.things[thing.index()];
    let type_name = without_generics(&model.items[owner.item.index()].name);
    let field_name = &model.items[field.index()].name;
    let mut notes: Vec<String> = sites
        .iter()
        .take(HELD_IN_MIND)
        .map(|(at, user)| format!("{}: {user}", clickable(at)))
        .collect();
    if sites.len() > HELD_IN_MIND {
        notes.push(format!(
            "and {}",
            counted(sites.len() - HELD_IN_MIND, "more place")
        ));
    }
    Diagnostic {
        lint: Lint::FieldLeak,
        key: format!("{}::{field_name}", tree.display(owner.home)),
        message: format!(
            "`{field_name}` is known outside `{type_name}`: {} in {}",
            counted(sites.len(), "place"),
            counted(modules.len(), "module")
        ),
        location: sites.keys().next().cloned(),
        notes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::tests::Sample;

    /// `RunQueue { items, head, limit }` in `queue`, whose `full(&self)`
    /// reads every field, and a scheduler in `sched` that reads `items`.
    fn queue() -> (Sample, ItemId, ItemId) {
        let mut sample = Sample::new();
        let (queue, sched) = (sample.module("queue"), sample.module("sched"));
        let run_queue = sample.item(queue, "RunQueue", ItemKind::Type);
        let [items, head, limit] = ["items", "head", "limit"].map(|field| {
            sample.member(
                run_queue,
                queue,
                &format!("RunQueue::{field}"),
                ItemKind::Field,
            )
        });
        let full = sample.member(run_queue, queue, "RunQueue::full", ItemKind::Function);
        sample.model.items[full.index()].takes_self = true;
        for field in [items, head, limit] {
            sample.refer(full, field);
        }
        let admit = sample.item(sched, "admit", ItemKind::Function);
        sample.refer(admit, items);
        (sample, admit, head)
    }

    fn keys(sample: &Sample) -> Vec<String> {
        let graph = Graph::new(&sample.model);
        let mut keys: Vec<String> = check(&sample.model, &graph, None)
            .into_iter()
            .map(|d| d.key)
            .collect();
        keys.sort();
        keys
    }

    #[test]
    fn a_field_an_outsider_uses_leaks_from_a_struct_that_guards_its_others() {
        let (sample, _, _) = queue();
        assert_eq!(keys(&sample), ["p::queue::RunQueue::items"]);
    }

    #[test]
    fn a_getter_is_seen_through_and_free_code_in_the_same_file_is_an_outsider() {
        let (mut sample, admit, head) = queue();
        let queue = sample.model.items[head.index()].owner;
        let run_queue = sample.model.items[head.index()].parent.unwrap();
        let getter = sample.member(run_queue, queue, "RunQueue::head", ItemKind::Function);
        sample.refer(getter, head);
        let hands_out = |copies: bool| crate::model::Wrapper::Accessor {
            lent: if copies { vec![] } else { vec![head] },
            copied: if copies { vec![head] } else { vec![] },
        };
        sample.model.wrappers.insert(getter, hands_out(false));
        let drained = sample.item(queue, "drained", ItemKind::Function);
        sample.refer(drained, getter);
        sample.refer(admit, getter);
        assert_eq!(
            keys(&sample),
            ["p::queue::RunQueue::head", "p::queue::RunQueue::items"]
        );
        // `fn head(&self) -> usize { self.head }` hands out a copy.
        sample.model.wrappers.insert(getter, hands_out(true));
        assert_eq!(keys(&sample), ["p::queue::RunQueue::items"]);
    }

    #[test]
    fn taking_a_copy_of_a_field_is_no_leak_but_the_field_is_not_kept() {
        let (mut sample, admit, head) = queue();
        sample.read(admit, head);
        assert_eq!(keys(&sample), ["p::queue::RunQueue::items"]);
        let limit = ItemId::new(head.index() + 1);
        sample.read(admit, limit);
        assert_eq!(keys(&sample), Vec::<String>::new());
    }

    #[test]
    fn a_struct_that_keeps_nothing_or_does_nothing_guards_nothing() {
        let (mut sample, admit, head) = queue();
        let limit = ItemId::new(head.index() + 1);
        sample.refer(admit, head);
        sample.refer(admit, limit);
        assert_eq!(keys(&sample), Vec::<String>::new());
        let (mut sample, _, _) = queue();
        for item in &mut sample.model.items {
            item.takes_self = false;
        }
        assert_eq!(keys(&sample), Vec::<String>::new());
    }
}
