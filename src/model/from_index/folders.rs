//! Folders: modules whose own file does nothing but declare child modules.
//! Such a module has no interface of its own, so walking past it is not
//! walking past a module's top.

use std::collections::{HashMap, HashSet};

use crate::model::{FileId, Model, NodeId, NodeKind};

pub fn mark(model: &mut Model) {
    for folder in find(model) {
        model.tree_mut().node_mut(folder).folder = true;
    }
}

/// A module with child modules whose own files' code names nothing but those
/// children (`mod x;`) and, in visibilities, its ancestors (`pub(crate)`), and
/// defines nothing that code uses.
fn find(model: &Model) -> Vec<NodeId> {
    let tree = model.tree();
    let mut own_files: HashMap<NodeId, Vec<FileId>> = HashMap::new();
    for (id, file) in model.files() {
        if !file.test {
            own_files.entry(file.module).or_default().push(id);
        }
    }
    let mut busy: HashSet<FileId> = HashSet::new();
    for reference in model.references().iter().filter(|r| !r.test) {
        let item = model.item(reference.item);
        match item.module {
            None => {
                busy.insert(reference.file);
                if !tree.is_test(item.owner) {
                    busy.extend(item.file);
                }
            }
            Some(named) => {
                let own = model.file(reference.file).module;
                if tree.parent(named) != Some(own) && !tree.is_within(own, named) {
                    busy.insert(reference.file);
                }
            }
        }
    }
    let mut has_children: HashSet<NodeId> = HashSet::new();
    for id in tree.ids() {
        if let Some(parent) = tree.parent(id).filter(|_| !tree.is_test(id)) {
            has_children.insert(parent);
        }
    }
    let mut folders: Vec<NodeId> = own_files
        .into_iter()
        .filter(|(module, files)| {
            tree.node(*module).kind == NodeKind::Module
                && !tree.is_test(*module)
                && has_children.contains(module)
                && files.iter().all(|file| !busy.contains(file))
        })
        .map(|(module, _)| module)
        .collect();
    folders.sort();
    folders
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{File, Item, ItemKind, Reference, Tree, Visibility};

    #[test]
    fn a_module_whose_file_only_declares_children_is_a_folder() {
        let mut tree = Tree::new();
        let package = tree.child(tree.root(), NodeKind::Package, "a");
        let kinds = tree.child(package, NodeKind::Module, "kinds");
        let epoll = tree.child(kinds, NodeKind::Module, "epoll");
        let socket = tree.child(package, NodeKind::Module, "socket");
        let stream = tree.child(socket, NodeKind::Module, "stream");
        let mut model = Model::new(tree);
        let file = |model: &mut Model, path: &str, module| {
            model.add_file(File {
                path: path.into(),
                lines: 3,
                module,
                test: false,
            })
        };
        let kinds_file = file(&mut model, "kinds/mod.rs", kinds);
        let socket_file = file(&mut model, "socket/mod.rs", socket);
        let module_item = |model: &mut Model, owner, module| {
            model.add_item(Item {
                name: "m".into(),
                owner,
                module: Some(module),
                file: None,
                kind: ItemKind::Module,
                visibility: Visibility::Unknown,
                signature: String::new(),
                parent_type: None,
                is_trait_impl_item: false,
            })
        };
        let crate_root = module_item(&mut model, package, package);
        let epoll_item = module_item(&mut model, kinds, epoll);
        let stream_item = module_item(&mut model, socket, stream);
        let ready = model.add_item(Item {
            name: "Ready".into(),
            owner: stream,
            module: None,
            file: None,
            kind: ItemKind::Type,
            visibility: Visibility::Public,
            signature: String::new(),
            parent_type: None,
            is_trait_impl_item: false,
        });
        let mut refer = |file, from, item| {
            model.add_reference(Reference {
                file,
                line: 1,
                from,
                from_function: None,
                item,
                test: false,
            })
        };
        // `pub(crate) mod epoll;`
        refer(kinds_file, kinds, crate_root);
        refer(kinds_file, kinds, epoll_item);
        // `mod stream; pub use stream::Ready;`
        refer(socket_file, socket, stream_item);
        refer(socket_file, socket, ready);
        assert_eq!(find(&model), [kinds]);
    }
}
