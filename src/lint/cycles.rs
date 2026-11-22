//! `module-cycle` and `split-module`: circular sibling dependencies inside a
//! crate, and modules split across two mutually dependent places.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use super::{Diagnostic, Lint, Location, clickable, in_scope, relative, without_generics};
use crate::model::{FileId, ItemId, ItemKind, Model, NodeId, NodeKind, Tree};

const CHEAP_EDGE_ITEMS: usize = 3;
const HUB_SPREAD: usize = 3;
const LEAST_PAIR_ITEMS: usize = 5;

#[derive(Default)]
struct Edge {
    refs: u32,
    items: BTreeSet<ItemId>,
    first_site: Option<(FileId, u32)>,
}

type SiblingEdges = HashMap<NodeId, BTreeMap<(NodeId, NodeId), Edge>>;

pub fn check(model: &Model, scope: Option<NodeId>) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    diagnostics.extend(check_cycles(model, scope));
    diagnostics.extend(check_split_modules(model, scope));
    diagnostics
}

fn check_cycles(model: &Model, scope: Option<NodeId>) -> Vec<Diagnostic> {
    let tree = model.tree();
    let mut callers: HashMap<ItemId, Vec<NodeId>> = HashMap::new();
    for reference in model.references().iter().filter(|r| !r.test) {
        callers
            .entry(reference.item)
            .or_default()
            .push(reference.from);
    }
    let edges = sibling_edges(model);
    let mut parents: Vec<NodeId> = edges.keys().copied().collect();
    parents.sort_by_key(|&p| tree.display(p));

    let mut out = Vec::new();
    for parent in parents {
        if !in_scope(tree, scope, parent) {
            continue;
        }
        let parent_edges = &edges[&parent];
        for mut members in strongly_connected(parent_edges.keys()) {
            members.sort_by_key(|&m| tree.display(m));
            let inside: BTreeMap<(NodeId, NodeId), &Edge> = parent_edges
                .iter()
                .filter(|((a, b), _)| members.contains(a) && members.contains(b))
                .map(|(&pair, edge)| (pair, edge))
                .collect();
            let dispatch = dispatch_edges(model, &inside, &callers);
            let best_cut = inside
                .iter()
                .filter(|(pair, edge)| {
                    edge.items.len() <= CHEAP_EDGE_ITEMS && !dispatch.contains(*pair)
                })
                .filter_map(|(&(from, to), edge)| {
                    let rest = inside.keys().filter(|&&pair| pair != (from, to));
                    let still: HashSet<NodeId> =
                        strongly_connected(rest).into_iter().flatten().collect();
                    let mut freed: Vec<NodeId> = members
                        .iter()
                        .copied()
                        .filter(|m| !still.contains(m))
                        .collect();
                    freed.sort_by_key(|&m| tree.display(m));
                    (!freed.is_empty()).then_some((from, to, edge, freed))
                })
                .min_by_key(|(from, to, edge, freed)| {
                    (
                        Reverse(freed.len()),
                        edge.items.len(),
                        edge.refs,
                        tree.display(*from),
                        tree.display(*to),
                    )
                });

            let parent_name = tree.display(parent);
            let member_names: Vec<String> =
                members.iter().map(|&m| relative(tree, m, parent)).collect();
            let key = format!("{parent_name} {}", member_names.join(","));

            let sorted_edges = sorted_cycle_edges(tree, &inside);
            let edge_notes = cycle_edge_notes(model, tree, parent, &sorted_edges);
            let (message, location, notes) = if let Some((from, to, edge, freed)) = best_cut {
                let from_rel = relative(tree, from, parent);
                let to_rel = relative(tree, to, parent);
                let item_names: Vec<String> = edge
                    .items
                    .iter()
                    .map(|&id| without_generics(&model.item(id).name))
                    .collect();
                let freed_names: Vec<String> =
                    freed.iter().map(|&m| relative(tree, m, parent)).collect();
                let location =
                    edge_location(model, from, edge).or_else(|| module_location(model, from));
                let summary = if members.len() == 2 {
                    format!("cyclic dependency in `{parent_name}`: `{from_rel}` <-> `{to_rel}`")
                } else {
                    format!(
                        "cyclic dependency of {} modules in `{parent_name}` closed by `{from_rel}` -> `{to_rel}`",
                        members.len()
                    )
                };
                let mut notes = vec![format!(
                    "`{from_rel}` -> `{to_rel}` uses only {} ({}); cutting it frees {}",
                    counted(item_names.len(), "item"),
                    item_names.join(", "),
                    freed_names
                        .iter()
                        .map(|n| format!("`{n}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )];
                notes.extend(edge_notes);
                (summary, location, notes)
            } else {
                let shown: String = if member_names.len() <= 5 {
                    member_names.join(" <-> ")
                } else {
                    format!(
                        "{} (and {} more)",
                        member_names[..4].join(" <-> "),
                        member_names.len() - 4
                    )
                };
                let message = format!(
                    "cyclic dependency of {} modules in `{parent_name}`: {shown}",
                    members.len()
                );
                let location = sorted_edges
                    .first()
                    .and_then(|(from, _, edge)| edge_location(model, *from, edge))
                    .or_else(|| module_location(model, parent));
                let mut notes = vec![format!("cycle members: {}", member_names.join(", "))];
                notes.extend(edge_notes);
                (message, location, notes)
            };

            out.push(Diagnostic {
                lint: Lint::ModuleCycle,
                key,
                message,
                location,
                notes,
                help: String::new(),
            });
        }
    }
    out
}

fn check_split_modules(model: &Model, scope: Option<NodeId>) -> Vec<Diagnostic> {
    let tree = model.tree();
    let code = || {
        model
            .references()
            .iter()
            .filter(|r| !r.test && model.item(r.item).module.is_none())
    };
    #[derive(Default)]
    struct Outside {
        refs: u32,
        items: HashSet<ItemId>,
        by_side: HashMap<NodeId, u32>,
    }
    let mut outside: HashMap<NodeId, Outside> = HashMap::new();
    for reference in code() {
        let owner = model.item(reference.item).owner;
        for module in tree.ancestors(owner) {
            if tree.is_within(reference.from, module) {
                break;
            }
            if !has_interface(tree, module) {
                continue;
            }
            let s = side(tree, module, reference.from);
            if tree.is_within(module, s) || tree.package_of(s) != tree.package_of(module) {
                continue;
            }
            let uses = outside.entry(module).or_default();
            uses.refs += 1;
            uses.items.insert(reference.item);
            *uses.by_side.entry(s).or_default() += 1;
        }
    }
    struct Pair {
        module: NodeId,
        client: NodeId,
        outside_refs: u32,
        client_refs: u32,
        back_refs: u32,
        back_items: HashSet<ItemId>,
    }
    let mut clients: HashMap<NodeId, Pair> = outside
        .iter()
        .filter(|(_, uses)| uses.items.len() >= LEAST_PAIR_ITEMS)
        .filter_map(|(&module, uses)| {
            let (&client, &client_refs) = uses
                .by_side
                .iter()
                .max_by_key(|&(&s, &refs)| (refs, Reverse(s)))?;
            (client_refs * 2 >= uses.refs).then(|| {
                (
                    module,
                    Pair {
                        module,
                        client,
                        outside_refs: uses.refs,
                        client_refs,
                        back_refs: 0,
                        back_items: HashSet::new(),
                    },
                )
            })
        })
        .collect();
    for reference in code() {
        let owner = model.item(reference.item).owner;
        for module in tree.ancestors(reference.from) {
            if let Some(pair) = clients.get_mut(&module)
                && tree.is_within(owner, pair.client)
            {
                pair.back_refs += 1;
                pair.back_items.insert(reference.item);
            }
        }
    }
    let mut pairs: Vec<Pair> = clients
        .into_values()
        .filter(|p| p.back_items.len() > CHEAP_EDGE_ITEMS)
        .collect();
    pairs.sort_by_key(|p| (Reverse(p.client_refs + p.back_refs), tree.display(p.module)));
    let mut seen: HashSet<(NodeId, NodeId)> = HashSet::new();
    pairs.retain(|p| {
        let (a, b) = (p.module, p.client);
        seen.insert((a.min(b), a.max(b)))
    });

    pairs
        .into_iter()
        .filter(|p| in_scope(tree, scope, p.module) || in_scope(tree, scope, p.client))
        .map(|p| {
            let a_name = tree.display(p.module);
            let b_name = tree.display(p.client);
            let (first, second) = if a_name <= b_name {
                (&a_name, &b_name)
            } else {
                (&b_name, &a_name)
            };
            let share = p.client_refs * 100 / p.outside_refs.max(1);
            Diagnostic {
                lint: Lint::SplitModule,
                key: format!("{first} {second}"),
                message: format!("`{a_name}` and `{b_name}` are one module split in two places"),
                location: module_location(model, p.module),
                notes: vec![format!(
                    "`{b_name}` makes {share}% of `{a_name}`'s outside uses in the crate; `{a_name}` uses {} of `{b_name}` back",
                    counted(p.back_items.len(), "item")
                )],
                help: "merge them into one module, or extract the shared core below both".into(),
            }
        })
        .collect()
}

fn sibling_edges(model: &Model) -> SiblingEdges {
    let tree = model.tree();
    let mut edges: SiblingEdges = HashMap::new();
    for reference in model.references().iter().filter(|r| !r.test) {
        let item = model.item(reference.item);
        if item.module.is_some() || tree.is_test(item.owner) {
            continue;
        }
        let ancestor = tree.common_ancestor(reference.from, item.owner);
        if ancestor == reference.from || ancestor == item.owner {
            continue;
        }
        if tree.node(ancestor).kind == NodeKind::Workspace {
            continue;
        }
        let from = tree.levels_below(ancestor, reference.from)[0];
        let to = tree.levels_below(ancestor, item.owner)[0];
        let edge = edges
            .entry(ancestor)
            .or_default()
            .entry((from, to))
            .or_default();
        edge.refs += 1;
        edge.items.insert(reference.item);
        let site = (reference.file, reference.line);
        match edge.first_site {
            None => edge.first_site = Some(site),
            Some((f, l))
                if (model.file(site.0).path.as_str(), site.1)
                    < (model.file(f).path.as_str(), l) =>
            {
                edge.first_site = Some(site);
            }
            _ => {}
        }
    }
    edges
}

fn has_interface(tree: &Tree, module: NodeId) -> bool {
    let node = tree.node(module);
    node.kind == NodeKind::Module && !node.folder && !tree.is_test(module)
}

fn side(tree: &Tree, module: NodeId, from: NodeId) -> NodeId {
    let ancestor = tree.common_ancestor(module, from);
    tree.levels_below(ancestor, from)
        .into_iter()
        .find(|&level| !tree.node(level).folder)
        .unwrap_or(ancestor)
}

fn dispatch_edges(
    model: &Model,
    inside: &BTreeMap<(NodeId, NodeId), &Edge>,
    callers: &HashMap<ItemId, Vec<NodeId>>,
) -> HashSet<(NodeId, NodeId)> {
    let tree = model.tree();
    let only_it_calls = |item: ItemId, hub: NodeId| {
        model.item(item).kind == ItemKind::Function
            && callers
                .get(&item)
                .into_iter()
                .flatten()
                .all(|&from| tree.is_within(from, hub))
    };
    let named: Vec<(NodeId, NodeId)> = inside
        .iter()
        .filter(|((from, _), edge)| edge.items.iter().all(|&item| only_it_calls(item, *from)))
        .map(|(&pair, _)| pair)
        .collect();
    let mut spread: HashMap<NodeId, usize> = HashMap::new();
    for (from, _) in &named {
        *spread.entry(*from).or_default() += 1;
    }
    named
        .into_iter()
        .filter(|(from, _)| spread[from] >= HUB_SPREAD)
        .collect()
}

fn strongly_connected<'a>(
    edges: impl IntoIterator<Item = &'a (NodeId, NodeId)>,
) -> Vec<Vec<NodeId>> {
    let pairs: Vec<(NodeId, NodeId)> = edges.into_iter().copied().collect();
    let mut nodes: Vec<NodeId> = pairs.iter().flat_map(|&(a, b)| [a, b]).collect();
    nodes.sort();
    nodes.dedup();
    let index: HashMap<NodeId, usize> = nodes.iter().enumerate().map(|(i, &n)| (n, i)).collect();
    let mut out: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for (a, b) in pairs {
        out[index[&a]].push(index[&b]);
    }
    let mut tarjan = Tarjan {
        out: &out,
        index: vec![None; nodes.len()],
        low: vec![0; nodes.len()],
        on_stack: vec![false; nodes.len()],
        stack: Vec::new(),
        next: 0,
        components: Vec::new(),
    };
    for v in 0..nodes.len() {
        if tarjan.index[v].is_none() {
            tarjan.visit(v);
        }
    }
    let mut components: Vec<Vec<NodeId>> = tarjan
        .components
        .into_iter()
        .filter(|component| component.len() > 1)
        .map(|component| {
            let mut members: Vec<NodeId> = component.into_iter().map(|i| nodes[i]).collect();
            members.sort();
            members
        })
        .collect();
    components.sort();
    components
}

struct Tarjan<'a> {
    out: &'a [Vec<usize>],
    index: Vec<Option<usize>>,
    low: Vec<usize>,
    on_stack: Vec<bool>,
    stack: Vec<usize>,
    next: usize,
    components: Vec<Vec<usize>>,
}

impl Tarjan<'_> {
    fn visit(&mut self, v: usize) {
        self.index[v] = Some(self.next);
        self.low[v] = self.next;
        self.next += 1;
        self.stack.push(v);
        self.on_stack[v] = true;
        for &w in &self.out[v] {
            match self.index[w] {
                None => {
                    self.visit(w);
                    self.low[v] = self.low[v].min(self.low[w]);
                }
                Some(index) if self.on_stack[w] => self.low[v] = self.low[v].min(index),
                Some(_) => {}
            }
        }
        if Some(self.low[v]) == self.index[v] {
            let mut component = Vec::new();
            while let Some(w) = self.stack.pop() {
                self.on_stack[w] = false;
                component.push(w);
                if w == v {
                    break;
                }
            }
            self.components.push(component);
        }
    }
}

fn edge_location(model: &Model, from: NodeId, edge: &Edge) -> Option<Location> {
    edge.first_site
        .map(|(file, line)| Location {
            file: model.file(file).path.clone(),
            line,
        })
        .or_else(|| module_location(model, from))
}

fn sorted_cycle_edges<'a>(
    tree: &Tree,
    inside: &'a BTreeMap<(NodeId, NodeId), &'a Edge>,
) -> Vec<(NodeId, NodeId, &'a Edge)> {
    let mut rows: Vec<_> = inside
        .iter()
        .map(|(&(from, to), &edge)| (from, to, edge))
        .collect();
    rows.sort_by_key(|(from, to, edge)| {
        (
            edge.items.len(),
            edge.refs,
            tree.display(*from),
            tree.display(*to),
        )
    });
    rows
}

fn cycle_edge_notes(
    model: &Model,
    tree: &Tree,
    parent: NodeId,
    edges: &[(NodeId, NodeId, &Edge)],
) -> Vec<String> {
    edges
        .iter()
        .map(|&(from, to, edge)| {
            let items = counted(edge.items.len(), "item");
            let from_rel = relative(tree, from, parent);
            let to_rel = relative(tree, to, parent);
            match edge_location(model, from, edge) {
                Some(loc) => format!(
                    "{}: `{from_rel}` -> `{to_rel}` ({items})",
                    clickable(&loc.file, loc.line)
                ),
                None => format!("`{from_rel}` -> `{to_rel}` ({items})"),
            }
        })
        .collect()
}

fn module_location(model: &Model, module: NodeId) -> Option<Location> {
    // Line 1 keeps `file:1:1` clickable when we only know the module file.
    model.module_file(module).map(|file| Location {
        file: file.to_owned(),
        line: 1,
    })
}

fn counted(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{File, Item, Reference, Visibility};

    #[test]
    fn reports_cycle_with_cheapest_cut_hint() {
        let mut tree = Tree::new();
        let pkg = tree.child(tree.root(), NodeKind::Package, "a");
        let socket = tree.child(pkg, NodeKind::Module, "socket");
        let kinds = tree.child(pkg, NodeKind::Module, "kinds");
        let mut model = Model::new(tree);
        let file = model.add_file(File {
            path: "src/kinds.rs".into(),
            lines: 10,
            module: kinds,
            test: false,
        });
        for i in 0..4 {
            let item = model.add_item(Item {
                name: format!("K{i}"),
                owner: kinds,
                module: None,
                file: None,
                kind: ItemKind::Type,
                visibility: Visibility::Crate,
                signature: String::new(),
                parent_type: None,
                is_trait_impl_item: false,
            });
            model.add_reference(Reference {
                file,
                line: 1,
                from: socket,
                item,
                test: false,
            });
        }
        let back = model.add_item(Item {
            name: "Socket".into(),
            owner: socket,
            module: None,
            file: None,
            kind: ItemKind::Type,
            visibility: Visibility::Crate,
            signature: String::new(),
            parent_type: None,
            is_trait_impl_item: false,
        });
        model.add_reference(Reference {
            file,
            line: 7,
            from: kinds,
            item: back,
            test: false,
        });
        let diags = check(&model, None);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].lint, Lint::ModuleCycle);
        assert_eq!(diags[0].key, "a kinds,socket");
        assert_eq!(
            diags[0].location,
            Some(Location {
                file: "src/kinds.rs".into(),
                line: 7,
            })
        );
        assert!(diags[0].notes.iter().any(|n| n.contains("Socket")));
    }
}
