//! The plain model every lint reads: the hierarchy, the workspace's files,
//! the items they define, every reference from code to an item, and which
//! functions only forward to another.

mod from_index;
mod tree;

pub use from_index::build;
pub use tree::{NodeId, NodeKind, Tree};

use std::collections::HashMap;

#[derive(Debug)]
pub struct Model {
    tree: Tree,
    files: Vec<File>,
    items: Vec<Item>,
    references: Vec<Reference>,
    /// Each forwarder and the function its body calls. No chain loops.
    forwarding: HashMap<ItemId, ItemId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FileId(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ItemId(u32);

#[derive(Debug)]
pub struct File {
    pub path: String,
    pub lines: u32,
    pub module: NodeId,
    pub test: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Visibility {
    Public,
    Crate,
    Restricted,
    Private,
    Unknown,
}

#[derive(Debug)]
pub struct Item {
    pub name: String,
    pub owner: NodeId,
    pub module: Option<NodeId>,
    pub file: Option<FileId>,
    pub kind: ItemKind,
    pub visibility: Visibility,
    pub signature: String,
    pub parent_type: Option<String>,
    pub is_trait_impl_item: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    Module,
    Type,
    Function,
    Field,
    Variant,
    Constant,
    Other,
}

#[derive(Debug)]
pub struct Reference {
    pub file: FileId,
    pub line: u32,
    pub from: NodeId,
    /// The innermost function whose item holds the reference, signature or body.
    pub from_function: Option<ItemId>,
    pub item: ItemId,
    pub test: bool,
}

impl Model {
    pub fn new(tree: Tree) -> Model {
        Model {
            tree,
            files: Vec::new(),
            items: Vec::new(),
            references: Vec::new(),
            forwarding: HashMap::new(),
        }
    }

    pub fn tree(&self) -> &Tree {
        &self.tree
    }

    pub fn add_file(&mut self, file: File) -> FileId {
        self.files.push(file);
        FileId(index(self.files.len() - 1))
    }

    pub fn add_item(&mut self, item: Item) -> ItemId {
        self.items.push(item);
        ItemId(index(self.items.len() - 1))
    }

    pub fn add_reference(&mut self, reference: Reference) {
        self.references.push(reference);
    }

    /// Records that `forwarder`'s body only calls `target`, passing its own
    /// parameters on. A link that would close a loop of forwarders is left
    /// out, so every chain ends at a function that does the work.
    pub fn add_forwarding(&mut self, forwarder: ItemId, target: ItemId) {
        if !self.is_forwarder(forwarder) && self.worker(target) != forwarder {
            self.forwarding.insert(forwarder, target);
        }
    }

    pub fn is_forwarder(&self, function: ItemId) -> bool {
        self.forwarding.contains_key(&function)
    }

    /// The function that does the work behind `function`: the end of its
    /// chain of forwarders, or `function` itself.
    pub fn worker(&self, function: ItemId) -> ItemId {
        let mut at = function;
        while let Some(&next) = self.forwarding.get(&at) {
            at = next;
        }
        at
    }

    pub fn file(&self, id: FileId) -> &File {
        &self.files[id.0 as usize]
    }

    pub fn item(&self, id: ItemId) -> &Item {
        &self.items[id.0 as usize]
    }

    pub fn files(&self) -> impl Iterator<Item = (FileId, &File)> {
        self.files
            .iter()
            .enumerate()
            .map(|(i, file)| (FileId(index(i)), file))
    }

    pub fn references(&self) -> &[Reference] {
        &self.references
    }

    pub fn items(&self) -> impl Iterator<Item = (ItemId, &Item)> {
        self.items
            .iter()
            .enumerate()
            .map(|(i, item)| (ItemId(index(i)), item))
    }

    /// The primary non-test source file defining `module`, if any.
    pub fn module_file(&self, module: NodeId) -> Option<&str> {
        self.files()
            .filter(|(_, f)| !f.test && f.module == module)
            .min_by_key(|(_, f)| {
                let rank = if f.path.ends_with("/src/lib.rs") || f.path == "src/lib.rs" {
                    0
                } else if f.path.ends_with("/mod.rs") || f.path.ends_with("/src/main.rs") {
                    1
                } else if f.path.contains("/examples/") {
                    3
                } else {
                    2
                };
                (rank, f.path.as_str())
            })
            .map(|(_, f)| f.path.as_str())
    }

    /// For each struct `(owner, type_name)`: `(private_fields, non_private_fields)`.
    pub fn struct_field_counts(&self) -> HashMap<(NodeId, String), (usize, usize)> {
        let mut counts: HashMap<(NodeId, String), (usize, usize)> = HashMap::new();
        for (_, item) in self.items() {
            if item.kind != ItemKind::Field || item.name.matches("::").count() >= 2 {
                continue;
            }
            let Some(parent) = &item.parent_type else {
                continue;
            };
            let entry = counts.entry((item.owner, parent.clone())).or_default();
            match item.visibility {
                Visibility::Private => entry.0 += 1,
                Visibility::Public | Visibility::Crate | Visibility::Restricted => entry.1 += 1,
                Visibility::Unknown => {}
            }
        }
        counts
    }

    fn tree_mut(&mut self) -> &mut Tree {
        &mut self.tree
    }

    fn item_mut(&mut self, id: ItemId) -> &mut Item {
        &mut self.items[id.0 as usize]
    }

    fn file_mut(&mut self, id: FileId) -> &mut File {
        &mut self.files[id.0 as usize]
    }

    fn references_mut(&mut self) -> &mut [Reference] {
        &mut self.references
    }
}

fn index(position: usize) -> u32 {
    u32::try_from(position).expect("fewer than 2^32 of each")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chain_of_forwarders_ends_at_its_worker_and_a_loop_is_not_closed() {
        let mut tree = Tree::new();
        let package = tree.child(tree.root(), NodeKind::Package, "a");
        let mut model = Model::new(tree);
        let [first, second, worker] = ["first", "second", "worker"].map(|name| {
            model.add_item(Item {
                name: name.into(),
                owner: package,
                module: None,
                file: None,
                kind: ItemKind::Function,
                visibility: Visibility::Public,
                signature: String::new(),
                parent_type: None,
                is_trait_impl_item: false,
            })
        });
        model.add_forwarding(first, second);
        model.add_forwarding(second, worker);
        model.add_forwarding(worker, first);
        assert_eq!(model.worker(first), worker);
        assert_eq!(model.worker(second), worker);
        assert_eq!(model.worker(worker), worker);
        assert!(model.is_forwarder(first));
        assert!(!model.is_forwarder(worker));
    }
}
