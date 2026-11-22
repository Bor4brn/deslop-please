//! A SCIP index of a workspace, turned into the plain model.

mod folders;
mod targets;

use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};

use crate::Error;
use crate::model::{
    File, FileId, Item, ItemId, ItemKind, Model, NodeId, NodeKind, Reference, Tree, Visibility,
};
use crate::scip::{Document, Index, Occurrence, Range, Symbol};
use crate::workspace::{Package, Workspace};

pub fn build(index: &Index, workspace: &Workspace) -> Result<Model, Error> {
    let mut builder = Builder {
        model: Model::new(Tree::new()),
        packages: HashMap::new(),
        symbols: HashMap::new(),
        impl_blocks: HashSet::new(),
    };
    builder.add_packages(workspace);
    let mut units = Vec::new();
    for document in &index.documents {
        if let Some(unit) = builder.add_document(document)? {
            units.push(unit);
        }
    }
    builder.read_symbol_information(index);
    builder.mark_test_code(&units, workspace);
    builder.count_lines(&units);
    folders::mark(&mut builder.model);
    Ok(builder.model)
}

fn is_test_module(name: &str) -> bool {
    name == "tests" || name.ends_with("_tests") || name.starts_with("test_")
}

struct Builder<'a> {
    model: Model,
    packages: HashMap<(&'a str, &'a str), NodeId>,
    symbols: HashMap<&'a str, Option<ItemId>>,
    impl_blocks: HashSet<ItemId>,
}

struct Unit<'a> {
    file: FileId,
    path: &'a str,
    inline_modules: Vec<(Range, NodeId)>,
    named_modules: Vec<NodeId>,
}

impl<'a> Builder<'a> {
    fn add_packages(&mut self, workspace: &'a Workspace) {
        let mut packages: Vec<&Package> = workspace.packages.iter().collect();
        packages.sort_by_key(|package| package.directory.split('/').count());
        let root = self.model.tree().root();
        let mut placed: Vec<(&str, NodeId)> = Vec::new();
        for package in packages {
            let node = if package.directory.is_empty() {
                let node = self.model.tree_mut().node_mut(root);
                node.kind = NodeKind::Package;
                node.name = package.crate_name.clone();
                root
            } else {
                let parent = placed
                    .iter()
                    .filter(|(directory, _)| is_under(&package.directory, directory))
                    .max_by_key(|(directory, _)| directory.len())
                    .map_or(root, |&(_, node)| node);
                self.model
                    .tree_mut()
                    .child(parent, NodeKind::Package, &package.crate_name)
            };
            placed.push((&package.directory, node));
            self.packages
                .insert((&package.name, &package.version), node);
        }
    }

    fn add_document(&mut self, document: &'a Document<'a>) -> Result<Option<Unit<'a>>, Error> {
        let not_understood = || {
            Error::new(format!(
                "{}: the index does not start with the file's module",
                document.path
            ))
        };
        let first = document.occurrences.first().ok_or_else(not_understood)?;
        let symbol = parse(first.symbol)?.ok_or_else(not_understood)?;
        let (Some(path), true) = (symbol.module(), first.definition) else {
            return Err(not_understood());
        };
        let Some(module) = self.module(&symbol, &path) else {
            return Ok(None);
        };
        let file = self.model.add_file(File {
            path: document.path.to_owned(),
            lines: lines(first.range),
            module,
            test: false,
        });
        let mut unit = Unit {
            file,
            path: document.path,
            inline_modules: Vec::new(),
            named_modules: Vec::new(),
        };
        for occurrence in &document.occurrences[1..] {
            let Some(item) = self.item(occurrence.symbol)? else {
                continue;
            };
            let item_module = self.model.item(item).module;
            if occurrence.definition {
                if let (Some(module), Some(block)) = (item_module, occurrence.enclosing) {
                    unit.inline_modules.push((block, module));
                }
            } else {
                unit.named_modules.extend(item_module);
            }
        }
        // A macro that writes modules gives each its own call as the block, so
        // the block does not say where any one of them is.
        let blocks: Vec<Range> = unit
            .inline_modules
            .iter()
            .map(|(block, _)| *block)
            .collect();
        unit.inline_modules
            .retain(|(block, _)| blocks.iter().filter(|other| *other == block).count() == 1);
        for occurrence in &document.occurrences {
            self.add_definition_or_reference(occurrence, &unit, module)?;
        }
        Ok(Some(unit))
    }

    fn add_definition_or_reference(
        &mut self,
        occurrence: &Occurrence<'a>,
        unit: &Unit,
        module: NodeId,
    ) -> Result<(), Error> {
        let Some(item) = self.item(occurrence.symbol)? else {
            return Ok(());
        };
        if occurrence.definition {
            let item = self.model.item_mut(item);
            item.file.get_or_insert(unit.file);
            return Ok(());
        }
        let from = innermost(&unit.inline_modules, occurrence.range).unwrap_or(module);
        let line = occurrence.range.start.line + 1;
        self.model.add_reference(Reference {
            file: unit.file,
            line,
            from,
            item,
            test: false,
        });
        Ok(())
    }

    fn item(&mut self, text: &'a str) -> Result<Option<ItemId>, Error> {
        if let Some(&known) = self.symbols.get(text) {
            return Ok(known);
        }
        let item = match parse(text)? {
            Some(symbol) => self.new_item(&symbol),
            None => None,
        };
        self.symbols.insert(text, item);
        Ok(item)
    }

    fn new_item(&mut self, symbol: &Symbol) -> Option<ItemId> {
        if let Some(path) = symbol.module() {
            let module = self.module(symbol, &path)?;
            let tree = self.model.tree();
            let owner = tree.parent(module).unwrap_or(module);
            let name = tree.node(module).name.clone();
            return Some(self.model.add_item(Item {
                name,
                owner,
                module: Some(module),
                file: None,
                kind: ItemKind::Module,
                visibility: Visibility::Unknown,
                signature: String::new(),
                parent_type: None,
                is_trait_impl_item: false,
            }));
        }
        let owner = self.module(symbol, &symbol.namespaces())?;
        let item = self.model.add_item(Item {
            name: symbol.item_name(),
            owner,
            module: None,
            file: None,
            kind: ItemKind::Other,
            visibility: Visibility::Unknown,
            signature: String::new(),
            parent_type: symbol.parent_type(),
            is_trait_impl_item: symbol.is_trait_impl_item(),
        });
        if symbol.is_impl_block() {
            self.impl_blocks.insert(item);
        }
        Some(item)
    }

    fn read_symbol_information(&mut self, index: &Index) {
        for information in index.documents.iter().flat_map(|d| &d.symbols) {
            let Some(&Some(id)) = self.symbols.get(information.symbol) else {
                continue;
            };
            if self.impl_blocks.contains(&id) {
                continue;
            }
            let visibility = parse_visibility(information.signature);
            match self.model.item(id).module {
                Some(module) if !information.signature.is_empty() => {
                    self.model.tree_mut().node_mut(module).public =
                        visibility == Visibility::Public;
                    let item = self.model.item_mut(id);
                    item.visibility = visibility;
                    item.signature = information.signature.to_owned();
                }
                Some(_) => {}
                None => {
                    let item = self.model.item_mut(id);
                    item.kind = item_kind(information.kind);
                    item.visibility = visibility;
                    item.signature = information.signature.to_owned();
                }
            }
        }
    }

    fn module(&mut self, symbol: &Symbol, path: &[&str]) -> Option<NodeId> {
        let mut node = *self
            .packages
            .get(&(symbol.package.as_ref(), symbol.version.as_ref()))?;
        for name in path {
            node = self.model.tree_mut().child(node, NodeKind::Module, name);
        }
        Some(node)
    }

    fn mark_test_code(&mut self, units: &[Unit], workspace: &Workspace) {
        let tree = self.model.tree_mut();
        for id in tree.ids() {
            let node = tree.node_mut(id);
            node.test = node.kind == NodeKind::Module && is_test_module(&node.name);
        }
        let target_units: Vec<targets::Unit> = units
            .iter()
            .map(|unit| {
                let module = self.model.file(unit.file).module;
                let mut defines = vec![module];
                defines.extend(unit.inline_modules.iter().map(|(_, module)| *module));
                targets::Unit {
                    path: unit.path,
                    module,
                    defines,
                    names: unit.named_modules.clone(),
                }
            })
            .collect();
        let roots: Vec<_> = workspace
            .packages
            .iter()
            .flat_map(|package| &package.targets)
            .map(|target| (target.kind, target.root.as_str()))
            .collect();
        let kinds = targets::assign(&target_units, &roots, self.model.tree());
        for (unit, kind) in units.iter().zip(kinds) {
            self.model.file_mut(unit.file).test = kind.is_some_and(|kind| kind.is_test());
        }
        let model = &mut self.model;
        let tests: Vec<bool> = model
            .references()
            .iter()
            .map(|reference| {
                model.file(reference.file).test || model.tree().is_test(reference.from)
            })
            .collect();
        for (reference, test) in model.references_mut().iter_mut().zip(tests) {
            reference.test = test;
        }
    }

    fn count_lines(&mut self, units: &[Unit]) {
        let mut own: HashMap<NodeId, i64> = HashMap::new();
        for unit in units {
            let file = self.model.file(unit.file);
            if file.test {
                continue;
            }
            *own.entry(file.module).or_default() += i64::from(file.lines);
            for &(block, module) in &unit.inline_modules {
                let span = i64::from(lines(block));
                *own.entry(module).or_default() += span;
                let outer = innermost(&unit.inline_modules, block).unwrap_or(file.module);
                *own.entry(outer).or_default() -= span;
            }
        }
        for (node, lines) in own {
            self.model.tree_mut().node_mut(node).own_lines =
                u32::try_from(lines.max(0)).unwrap_or(u32::MAX);
        }
    }
}

fn item_kind(kind: u64) -> ItemKind {
    const ASSOCIATED_TYPE: u64 = 3;
    const CONSTANT: u64 = 8;
    const ENUM: u64 = 11;
    const ENUM_MEMBER: u64 = 12;
    const FIELD: u64 = 15;
    const FUNCTION: u64 = 17;
    const METHOD: u64 = 26;
    const STRUCT: u64 = 49;
    const TRAIT: u64 = 53;
    const TYPE_ALIAS: u64 = 55;
    const UNION: u64 = 59;
    const TRAIT_METHOD: u64 = 70;
    const STATIC_METHOD: u64 = 80;
    const STATIC_VARIABLE: u64 = 82;
    match kind {
        ASSOCIATED_TYPE | ENUM | STRUCT | TRAIT | TYPE_ALIAS | UNION => ItemKind::Type,
        FUNCTION | METHOD | TRAIT_METHOD | STATIC_METHOD => ItemKind::Function,
        FIELD => ItemKind::Field,
        ENUM_MEMBER => ItemKind::Variant,
        CONSTANT | STATIC_VARIABLE => ItemKind::Constant,
        _ => ItemKind::Other,
    }
}

pub fn parse_visibility(signature: &str) -> Visibility {
    let trimmed = signature.trim_start();
    if trimmed.is_empty() {
        return Visibility::Unknown;
    }
    let Some(rest) = trimmed.strip_prefix("pub") else {
        return Visibility::Private;
    };
    if rest.starts_with(char::is_whitespace) {
        Visibility::Public
    } else if let Some(inner) = rest.strip_prefix('(') {
        let scope = inner.split_once(')').map_or("", |(scope, _)| scope.trim());
        if scope == "crate" {
            Visibility::Crate
        } else {
            Visibility::Restricted
        }
    } else {
        Visibility::Private
    }
}

fn parse(text: &str) -> Result<Option<Symbol<'_>>, Error> {
    Symbol::parse(text).map_err(|e| Error::new(e.to_string()))
}

fn innermost(inline_modules: &[(Range, NodeId)], range: Range) -> Option<NodeId> {
    inline_modules
        .iter()
        .filter(|(block, _)| {
            *block != range && block.start <= range.start && range.end <= block.end
        })
        .min_by_key(|(block, _)| (lines(*block), Reverse(block.start)))
        .map(|(_, module)| *module)
}

fn is_under(path: &str, directory: &str) -> bool {
    directory.is_empty()
        || path
            .strip_prefix(directory)
            .is_some_and(|rest| rest.starts_with('/'))
}

fn lines(range: Range) -> u32 {
    let last = if range.end.column == 0 {
        range.end.line
    } else {
        range.end.line + 1
    };
    last.saturating_sub(range.start.line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::Package;

    #[test]
    fn a_package_is_inside_the_package_whose_directory_holds_it_and_a_plain_directory_is_no_level()
    {
        let package = |name: &str, directory: &str| Package {
            name: name.into(),
            version: "0.1.0".into(),
            crate_name: name.into(),
            directory: directory.into(),
            targets: Vec::new(),
        };
        let workspace = Workspace {
            root: "/w".into(),
            target_directory: "/w/target".into(),
            packages: vec![
                package("helper", "app/tests/programs/helper"),
                package("lib", "crates/lib"),
                package("app", "app"),
            ],
        };
        let mut builder = Builder {
            model: Model::new(Tree::new()),
            packages: HashMap::new(),
            symbols: HashMap::new(),
            impl_blocks: HashSet::new(),
        };
        builder.add_packages(&workspace);
        let tree = builder.model.tree();
        let node = |name| builder.packages[&(name, "0.1.0")];
        assert_eq!(tree.parent(node("lib")), Some(tree.root()));
        assert_eq!(tree.parent(node("app")), Some(tree.root()));
        assert_eq!(tree.parent(node("helper")), Some(node("app")));
    }

    #[test]
    fn parses_rust_visibilities() {
        assert_eq!(parse_visibility("pub mod log"), Visibility::Public);
        assert_eq!(parse_visibility("pub fn serve()"), Visibility::Public);
        assert_eq!(parse_visibility("pub(crate) fn serve()"), Visibility::Crate);
        assert_eq!(
            parse_visibility("pub(super) struct Helper"),
            Visibility::Restricted
        );
        assert_eq!(
            parse_visibility("pub(in crate::cpu) fn tick()"),
            Visibility::Restricted
        );
        assert_eq!(parse_visibility("fn private()"), Visibility::Private);
        assert_eq!(parse_visibility("publish: u32"), Visibility::Private);
        assert_eq!(parse_visibility(""), Visibility::Unknown);
    }

    #[test]
    fn test_modules_by_name() {
        assert!(is_test_module("tests"));
        assert!(is_test_module("shuttle_tests"));
        assert!(is_test_module("test_machine"));
        assert!(!is_test_module("contests"));
        assert!(!is_test_module("testing"));
    }
}
