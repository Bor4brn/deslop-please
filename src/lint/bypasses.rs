//! `facade-bypass`: code outside a module reaching past its facade into an
//! internal submodule.

use std::collections::BTreeMap;

use super::{Diagnostic, Lint, Location, clickable, in_scope};
use crate::model::{Model, NodeId, Tree};

pub fn check(model: &Model, scope: Option<NodeId>) -> Vec<Diagnostic> {
    let tree = model.tree();
    let mut grouped: BTreeMap<(String, NodeId, NodeId, NodeId), Vec<u32>> = BTreeMap::new();
    for reference in model.references().iter().filter(|r| !r.test) {
        let Some(named) = model.item(reference.item).module else {
            continue;
        };
        let Some((past, from_side)) = judge(tree, reference.from, named) else {
            continue;
        };
        if !in_scope(tree, scope, reference.from) && !in_scope(tree, scope, past) {
            continue;
        }
        let file = model.file(reference.file).path.clone();
        grouped
            .entry((file, named, past, from_side))
            .or_default()
            .push(reference.line);
    }

    grouped
        .into_iter()
        .map(|((file, named, past, from_side), mut lines)| {
            lines.sort();
            lines.dedup();
            let first = lines[0];
            let named_str = tree.display(named);
            let past_str = tree.display(past);
            let from_str = tree.display(from_side);
            let mut notes = Vec::new();
            if lines.len() > 1 {
                let more: Vec<String> = lines[1..]
                    .iter()
                    .take(5)
                    .map(|&line| clickable(&file, line))
                    .collect();
                let suffix = if lines.len() > 6 { ", ..." } else { "" };
                notes.push(format!("also: {}{suffix}", more.join(", ")));
            }
            Diagnostic {
                lint: Lint::FacadeBypass,
                key: format!("{file} {named_str}"),
                message: format!("`{from_str}` reaches past `{past_str}` into `{named_str}`"),
                location: Some(Location { file, line: first }),
                notes,
                help: format!(
                    "re-export what callers need from `{past_str}` and keep `{named_str}` private"
                ),
            }
        })
        .collect()
}

fn judge(tree: &Tree, from: NodeId, named: NodeId) -> Option<(NodeId, NodeId)> {
    let ancestor = tree.common_ancestor(from, named);
    let named_levels = levels(tree, ancestor, named);
    let [past, _, ..] = named_levels[..] else {
        return None;
    };
    if is_interface_of_another_crate(tree, from, named) {
        return None;
    }
    let from_side = levels(tree, ancestor, from)
        .first()
        .copied()
        .unwrap_or(ancestor);
    Some((past, from_side))
}

fn levels(tree: &Tree, ancestor: NodeId, id: NodeId) -> Vec<NodeId> {
    let mut levels = tree.levels_below(ancestor, id);
    levels.retain(|&level| !tree.node(level).folder);
    levels
}

fn is_interface_of_another_crate(tree: &Tree, from: NodeId, named: NodeId) -> bool {
    let Some(package) = tree.package_of(named) else {
        return false;
    };
    !tree.is_within(from, package)
        && tree
            .levels_below(package, named)
            .iter()
            .all(|&module| tree.node(module).public)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{File, Item, ItemKind, NodeKind, Reference, Visibility};

    #[test]
    fn detects_bypass_and_ignores_folders_and_pub_crate_api() {
        let mut tree = Tree::new();
        let root = tree.root();
        let a = tree.child(root, NodeKind::Package, "a");
        let kinds = tree.child(a, NodeKind::Module, "kinds");
        let epoll = tree.child(kinds, NodeKind::Module, "epoll");
        let socket = tree.child(a, NodeKind::Module, "socket");
        let mut model = Model::new(tree);
        let file = model.add_file(File {
            path: "src/socket.rs".into(),
            lines: 10,
            module: socket,
            test: false,
        });
        let epoll_item = model.add_item(Item {
            name: "epoll".into(),
            owner: kinds,
            module: Some(epoll),
            file: None,
            kind: ItemKind::Module,
            visibility: Visibility::Crate,
            signature: String::new(),
            parent_type: None,
            is_trait_impl_item: false,
        });
        model.add_reference(Reference {
            file,
            line: 3,
            from: socket,
            item: epoll_item,
            test: false,
        });
        let diags = check(&model, None);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].lint, Lint::FacadeBypass);
        assert_eq!(diags[0].key, "src/socket.rs a::kinds::epoll");
    }
}
