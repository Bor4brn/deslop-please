//! `tangle`: things that know each other in a circle, written in more than one
//! module, or more of them than a reader holds in mind. Where a tangle is
//! written is its members' homes and the modules of the references that tie
//! it, so moving an `impl` block or lifting a member into the parent module's
//! file changes nothing.

use std::collections::{BTreeMap, BTreeSet};

use super::{Diagnostic, HELD_IN_MIND, Lint, Tie, counted, full_name, in_scope, listed, tie_notes};
use crate::graph::{Graph, ThingId};
use crate::model::{Model, NodeId};

#[derive(Default)]
struct Written {
    modules: BTreeSet<NodeId>,
    ties: BTreeMap<(ThingId, ThingId), Tie>,
}

pub fn check(model: &Model, graph: &Graph, scope: Option<NodeId>) -> Vec<Diagnostic> {
    let mut written: Vec<Written> = graph
        .tangles
        .iter()
        .map(|members| Written {
            modules: members
                .iter()
                .map(|&member| graph.things[member.index()].home)
                .collect(),
            ties: BTreeMap::new(),
        })
        .collect();
    let tangle_of = |thing: ThingId| graph.tangle_of[thing.index()];
    for usage in &graph.uses {
        let Some(user) = usage.user else { continue };
        let Some(used) = graph.thing_of[usage.item.index()] else {
            continue;
        };
        let Some(tangle) = tangle_of(user).filter(|&t| user != used && tangle_of(used) == Some(t))
        else {
            continue;
        };
        let reference = &model.references[usage.reference];
        let written = &mut written[tangle.index()];
        written.modules.insert(reference.from);
        let tie = written.ties.entry((user, used)).or_default();
        tie.add(model, usage.item, reference);
    }
    let mut diagnostics = Vec::new();
    for (index, written) in written.into_iter().enumerate() {
        let members = &graph.tangles[index];
        let concerned = members
            .iter()
            .any(|&member| in_scope(&model.tree, scope, graph.things[member.index()].home));
        let modules = written.modules.len();
        if concerned && (modules > 1 || members.len() > HELD_IN_MIND) {
            diagnostics.push(diagnostic(model, graph, members, written));
        }
    }
    diagnostics
}

fn diagnostic(model: &Model, graph: &Graph, members: &[ThingId], written: Written) -> Diagnostic {
    let mut names: Vec<String> = members
        .iter()
        .map(|&member| full_name(model, graph, member))
        .collect();
    names.sort();
    let ties = written
        .ties
        .iter()
        .map(|(&(user, used), tie)| {
            let names = (full_name(model, graph, user), full_name(model, graph, used));
            (names, tie)
        })
        .collect();
    let (location, ties) = tie_notes(model, "knows", ties);
    let mut notes = vec![format!("members: {}", listed(&names))];
    notes.extend(ties);
    let count = names.len();
    let message = match written.modules.iter().collect::<Vec<_>>()[..] {
        [only] => format!(
            "{count} things know each other in a circle, all in `{}`",
            model.tree.display(*only)
        ),
        _ => format!(
            "{count} things know each other in a circle, across {}",
            counted(written.modules.len(), "module")
        ),
    };
    Diagnostic {
        lint: Lint::Tangle,
        key: names.join(","),
        message,
        location,
        notes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::tests::Sample;
    use crate::model::{ItemId, ItemKind};

    fn keys(sample: &Sample) -> Vec<String> {
        let graph = Graph::new(&sample.model);
        check(&sample.model, &graph, None)
            .into_iter()
            .map(|d| d.key)
            .collect()
    }

    #[test]
    fn two_things_in_a_circle_are_reported_when_their_code_is_in_two_modules() {
        let mut sample = Sample::new();
        let (kernel, timers) = (sample.module("kernel"), sample.module("kernel::timers"));
        let tasks = sample.item(kernel, "Tasks", ItemKind::Type);
        let timer = sample.item(timers, "Timers", ItemKind::Type);
        let spawn = sample.member(tasks, kernel, "Tasks::spawn", ItemKind::Function);
        let fire = sample.member(timer, timers, "Timers::fire", ItemKind::Function);
        sample.refer(spawn, timer);
        sample.refer(fire, tasks);
        assert_eq!(
            keys(&sample),
            ["p::kernel::Tasks,p::kernel::timers::Timers"]
        );
    }

    #[test]
    fn a_circle_in_one_module_is_reported_only_when_it_is_too_big_to_hold_in_mind() {
        let mut sample = Sample::new();
        let module = sample.module("ring");
        let ring = |sample: &mut Sample, size: usize| {
            let things: Vec<ItemId> = (0..size)
                .map(|i| sample.item(module, &format!("T{size}x{i}"), ItemKind::Type))
                .collect();
            for i in 0..size {
                sample.refer(things[i], things[(i + 1) % size]);
            }
        };
        ring(&mut sample, HELD_IN_MIND);
        assert_eq!(keys(&sample), Vec::<String>::new());
        ring(&mut sample, HELD_IN_MIND + 1);
        assert_eq!(keys(&sample).len(), 1);
    }

    #[test]
    fn an_impl_block_written_elsewhere_ties_its_type_from_there() {
        let mut sample = Sample::new();
        let (scheduling, elsewhere) = (sample.module("scheduling"), sample.module("elsewhere"));
        let tasks = sample.item(scheduling, "Tasks", ItemKind::Type);
        let timers = sample.item(scheduling, "Timers", ItemKind::Type);
        let spawn = sample.member(tasks, scheduling, "Tasks::spawn", ItemKind::Function);
        let fire = sample.member(timers, elsewhere, "Timers::fire", ItemKind::Function);
        sample.refer(spawn, timers);
        sample.refer(fire, tasks);
        let graph = Graph::new(&sample.model);
        let diagnostics = check(&sample.model, &graph, None);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].message,
            "2 things know each other in a circle, across 2 modules"
        );
    }
}
