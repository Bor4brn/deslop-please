//! A SCIP index of a workspace, turned into the plain model.

mod blocks;
mod folders;
mod forwarders;
mod targets;

use std::collections::{HashMap, HashSet};

use crate::Error;
use crate::model::{
    File, FileId, Item, ItemId, ItemKind, Model, NodeId, NodeKind, Reference, Tree, Visibility,
};
use crate::scip::{Document, Index, Kind, Occurrence, Range, Symbol};
use crate::workspace::{Package, Workspace};
use blocks::Blocks;
use forwarders::Name;

pub fn build(index: &Index, workspace: &Workspace) -> Result<Model, Error> {
    let mut builder = Builder::new();
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
    builder.add_forwardings(&units);
    folders::mark(&mut builder.model);
    Ok(builder.model)
}

fn is_test_module(name: &str) -> bool {
    name == "tests" || name.ends_with("_tests") || name.starts_with("test_")
}

struct Builder<'a> {
    model: Model,
    packages: HashMap<(&'a str, &'a str), NodeId>,
    symbols: HashMap<&'a str, Named>,
    impl_blocks: HashSet<ItemId>,
    functions: HashSet<ItemId>,
    free_or_inherent_functions: HashSet<ItemId>,
}

/// What a symbol of the index stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Named {
    /// A parameter, variable, or type parameter, which only its document names.
    Local,
    Item(ItemId),
    /// Something outside the workspace, by what its last descriptor says it is.
    Outside(Kind),
}

struct Unit<'a> {
    file: FileId,
    document: &'a Document<'a>,
    inline_modules: Blocks<NodeId>,
    functions: Blocks<ItemId>,
    named_modules: Vec<NodeId>,
}

impl<'a> Builder<'a> {
    fn new() -> Builder<'a> {
        Builder {
            model: Model::new(Tree::new()),
            packages: HashMap::new(),
            symbols: HashMap::new(),
            impl_blocks: HashSet::new(),
            functions: HashSet::new(),
            free_or_inherent_functions: HashSet::new(),
        }
    }

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
        let mut inline_modules = Vec::new();
        let mut functions = Vec::new();
        let mut named_modules = Vec::new();
        for occurrence in &document.occurrences[1..] {
            let Some(item) = self.item(occurrence.symbol)? else {
                continue;
            };
            let item_module = self.model.item(item).module;
            if !occurrence.definition {
                named_modules.extend(item_module);
                continue;
            }
            let Some(block) = occurrence.enclosing else {
                continue;
            };
            if let Some(module) = item_module {
                inline_modules.push((block, module));
            } else if self.functions.contains(&item) {
                functions.push((block, item));
            }
        }
        let unit = Unit {
            file,
            document,
            inline_modules: Blocks::new(inline_modules),
            functions: Blocks::new(functions),
            named_modules,
        };
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
        let from = unit
            .inline_modules
            .innermost(occurrence.range)
            .unwrap_or(module);
        let line = occurrence.range.start.line + 1;
        self.model.add_reference(Reference {
            file: unit.file,
            line,
            from,
            from_function: unit.functions.innermost(occurrence.range),
            item,
            test: false,
        });
        Ok(())
    }

    fn item(&mut self, text: &'a str) -> Result<Option<ItemId>, Error> {
        match self.named(text)? {
            Named::Item(item) => Ok(Some(item)),
            Named::Local | Named::Outside(_) => Ok(None),
        }
    }

    fn named(&mut self, text: &'a str) -> Result<Named, Error> {
        if let Some(&known) = self.symbols.get(text) {
            return Ok(known);
        }
        let named = match parse(text)? {
            Some(symbol) => self
                .new_item(&symbol)
                .map_or(Named::Outside(symbol.kind()), Named::Item),
            None => Named::Local,
        };
        self.symbols.insert(text, named);
        Ok(named)
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
        if symbol.is_function() {
            self.functions.insert(item);
        }
        if symbol.is_free_or_inherent_function() {
            self.free_or_inherent_functions.insert(item);
        }
        Some(item)
    }

    fn read_symbol_information(&mut self, index: &Index) {
        for information in index.documents.iter().flat_map(|d| &d.symbols) {
            let Some(&Named::Item(id)) = self.symbols.get(information.symbol) else {
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
                defines.extend(unit.inline_modules.iter().map(|(_, module)| module));
                targets::Unit {
                    path: unit.document.path,
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
            for (block, module) in unit.inline_modules.iter() {
                let span = i64::from(lines(block));
                *own.entry(module).or_default() += span;
                let outer = unit.inline_modules.innermost(block).unwrap_or(file.module);
                *own.entry(outer).or_default() -= span;
            }
        }
        for (node, lines) in own {
            self.model.tree_mut().node_mut(node).own_lines =
                u32::try_from(lines.max(0)).unwrap_or(u32::MAX);
        }
    }

    fn add_forwardings(&mut self, units: &[Unit<'a>]) {
        let mut definitions = forwarders::Definitions::default();
        for unit in units {
            let kinds: HashMap<&str, u64> = unit
                .document
                .symbols
                .iter()
                .map(|information| (information.symbol, information.kind))
                .collect();
            let mut occurrences: Vec<&Occurrence> = unit.document.occurrences.iter().collect();
            occurrences.sort_by_key(|occurrence| occurrence.range.start);
            for (block, function) in unit.functions.iter() {
                let defines_function = |occurrence: &Occurrence| {
                    occurrence.definition
                        && self.symbols.get(occurrence.symbol) == Some(&Named::Item(function))
                };
                let start = occurrences.partition_point(|o| o.range.start < block.start);
                let names: Vec<Name> = occurrences[start..]
                    .iter()
                    .copied()
                    .take_while(|occurrence| block.holds(occurrence.range))
                    .skip_while(|occurrence| !defines_function(occurrence))
                    .skip(1)
                    .map(|occurrence| self.name(occurrence, &kinds))
                    .collect();
                let may_forward = self.free_or_inherent_functions.contains(&function);
                definitions.add(function, may_forward, &names);
            }
        }
        for (forwarder, target) in definitions.forwardings() {
            self.model.add_forwarding(forwarder, target);
        }
    }

    /// What `occurrence` names; `kinds` has the kinds of its document's symbols.
    fn name(&self, occurrence: &Occurrence<'a>, kinds: &HashMap<&str, u64>) -> Name<'a> {
        let symbol = occurrence.symbol;
        let Some(&named) = self.symbols.get(symbol) else {
            return Name::Other;
        };
        match named {
            Named::Local => local_name(symbol, kinds.get(symbol).copied(), occurrence.definition),
            _ if occurrence.definition => Name::Other,
            Named::Item(item) if self.impl_blocks.contains(&item) => Name::Type,
            Named::Item(item) => match self.model.item(item).kind {
                ItemKind::Module | ItemKind::Type => Name::Type,
                ItemKind::Field => Name::Field,
                ItemKind::Function => Name::Function(Some(item)),
                ItemKind::Variant | ItemKind::Constant | ItemKind::Other => Name::Other,
            },
            Named::Outside(kind) => match kind {
                Kind::Namespace | Kind::Type | Kind::TypeParameter => Name::Type,
                Kind::Method => Name::Function(None),
                Kind::Term | Kind::Parameter | Kind::Meta | Kind::Macro => Name::Other,
            },
        }
    }
}

/// `kind`: `scip.proto`'s `SymbolInformation.Kind` of the local, if known.
fn local_name(symbol: &str, kind: Option<u64>, definition: bool) -> Name<'_> {
    const PARAMETER: u64 = 37;
    const SELF_PARAMETER: u64 = 44;
    const TYPE_PARAMETER: u64 = 58;
    match (kind, definition) {
        (Some(TYPE_PARAMETER), _) => Name::Type,
        (Some(PARAMETER), true) => Name::Parameter {
            local: symbol,
            is_self: false,
        },
        (Some(SELF_PARAMETER), true) => Name::Parameter {
            local: symbol,
            is_self: true,
        },
        (_, true) => Name::Other,
        (_, false) => Name::Value(symbol),
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
        let mut builder = Builder::new();
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
