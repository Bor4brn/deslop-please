//! The plain model every lint reads: the hierarchy, the workspace's files,
//! the items they define, and every reference from code to an item.

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
