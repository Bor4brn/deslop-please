//! A Python, TypeScript or JavaScript project's SCIP index, and its files'
//! text, turned into the plain model the lints read. The project is the
//! model's one package; its modules are its packages and files, as the
//! language names them: `app.store.table` for `app/store/table.py`,
//! `store::table` for `src/store/table.ts`. A leading `src` is no level, and
//! `__init__.py` and `index.ts` are their directory's module, as `mod.rs` is.
//!
//! The items are the classes, interfaces, enums and type aliases (types);
//! functions, methods and constructors; class attributes and properties
//! (fields); and module-level constants and variables. A module-level
//! variable that holds a function, `const f = () => ..`, is a function. A
//! reference is any name the code writes for an item outside an import.
//! What a module re-exports in an `__init__.py` or `index.ts` is no use.

mod text;

use std::collections::{HashMap, HashSet};

use super::from_index::{Blocks, Forwarding, ForwardingName, Forwardings};
use crate::Error;
use crate::model::{
    File, FileId, Item, ItemId, ItemKind, Model, NodeId, NodeKind, Reference, Tree, Wrapper,
};
use crate::scip::{Document, Index, Kind, Occurrence, Place, Position, Range, Symbol};
use text::{Access, Declared, Text};

/// How a language writes what the model reads from its text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Syntax {
    Python,
    /// TypeScript and JavaScript.
    Script,
}

/// `scip.proto`'s kinds that scip-typescript writes and that the
/// descriptors do not tell apart.
const CONSTRUCTOR: u64 = 9;
const ENUM_MEMBER: u64 = 12;
const FUNCTION: u64 = 17;
const GETTER: u64 = 18;
const METHOD: u64 = 26;
const SETTER: u64 = 45;
const STATIC_METHOD: u64 = 80;

/// The model of a project called `name`, whose `files`, relative to its
/// root, are its sources: what the index says of others is left out. The
/// model's paths start with `prefix`. `read` gives a file's text.
pub fn build(
    index: &Index,
    syntax: Syntax,
    name: &str,
    prefix: &str,
    files: &[String],
    read: impl Fn(&str) -> Result<String, Error>,
) -> Result<Model, Error> {
    let sources: HashSet<&str> = files.iter().map(String::as_str).collect();
    let mut seen = HashSet::new();
    let mut documents: Vec<&Document> = index
        .documents
        .iter()
        .filter(|document| sources.contains(&*document.path) && seen.insert(&*document.path))
        .collect();
    documents.sort_by(|a, b| a.path.cmp(&b.path));
    let mut builder = Builder::new(syntax, name);
    let modules: Vec<NodeId> = documents
        .iter()
        .map(|document| builder.add_module(document))
        .collect();
    for (document, module) in documents.into_iter().zip(modules) {
        let text = Text::new(read(&document.path)?, syntax);
        builder.add_document(document, module, text, prefix);
    }
    // Every definition is known before the first reference, whichever file
    // it is in.
    builder.add_references();
    builder.read_kinds(index);
    Ok(builder.finish())
}

/// What a symbol of the index stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Named {
    /// A variable, which only its document names.
    Local,
    /// A parameter of a function of the project's.
    Parameter,
    /// A module, or what is no item: an object literal's key.
    NoItem,
    Item(ItemId),
    /// Something outside the project, by what its last descriptor says it is.
    Outside(Kind),
}

struct Unit<'a> {
    file: FileId,
    module: NodeId,
    document: &'a Document<'a>,
    text: Text,
    /// Byte ranges of its imports, in order.
    imports: Vec<(usize, usize)>,
    /// The functions and types whose definitions hold code.
    holders: Blocks<ItemId>,
}

struct Builder<'a> {
    syntax: Syntax,
    /// The package's name as code imports it: `my_app` for `my-app`.
    package: String,
    tree: Tree,
    files: Vec<File>,
    items: Vec<Item>,
    references: Vec<Reference>,
    /// By reference: what its code does with what it names.
    accesses: Vec<Access>,
    /// By reference: where the member its code reaches into next is named,
    /// `city` in `user.address.city`.
    members: Vec<Option<(FileId, Position)>>,
    /// Each reference, by where it is named.
    reference_at: HashMap<(FileId, Position), usize>,
    symbols: HashMap<&'a str, Named>,
    /// By item.
    symbol_of: Vec<&'a str>,
    /// What the index says each symbol is, where it says.
    kinds: HashMap<&'a str, u64>,
    /// The packages, by name and version, whose symbols are the project's.
    packages: HashSet<(String, String)>,
    /// The module of each source file, by its path below the package.
    modules: HashMap<Vec<String>, NodeId>,
    /// Each item's definitions, by unit and the position of its name.
    definitions: HashMap<ItemId, Vec<(usize, Position)>>,
    /// Each holder's definition, by unit, name and whole range; `None` for one
    /// defined more than once, which no one text describes.
    bodies: HashMap<ItemId, Option<(usize, Position, Range)>>,
    /// Python classes that derive from `Enum` or `Flag`: what their bodies
    /// define are members, as a Rust enum's variants.
    enums: HashSet<ItemId>,
    /// Fields that hold plain data, a list of numbers or of text: read whole,
    /// they are read, as a borrow of Rust's `Vec<u32>` is.
    data: HashSet<ItemId>,
    /// By reference: the code lends the value whole for the moment, calling
    /// no method of it, `f(q.items)`; not `q.items.append(x)`, `len(q.items)`
    /// or `items = q.items`.
    whole: Vec<bool>,
    units: Vec<Unit<'a>>,
}

impl<'a> Builder<'a> {
    fn new(syntax: Syntax, name: &str) -> Builder<'a> {
        let mut tree = Tree::new();
        let root = tree.root();
        let node = tree.node_mut(root);
        node.kind = NodeKind::Package;
        node.name = name.to_owned();
        Builder {
            syntax,
            package: name.replace('-', "_").to_lowercase(),
            tree,
            files: Vec::new(),
            items: Vec::new(),
            references: Vec::new(),
            accesses: Vec::new(),
            members: Vec::new(),
            reference_at: HashMap::new(),
            symbols: HashMap::new(),
            symbol_of: Vec::new(),
            kinds: HashMap::new(),
            packages: HashSet::new(),
            modules: HashMap::new(),
            definitions: HashMap::new(),
            bodies: HashMap::new(),
            enums: HashSet::new(),
            data: HashSet::new(),
            whole: Vec::new(),
            units: Vec::new(),
        }
    }

    /// Makes the module of `document`, and gives it.
    fn add_module(&mut self, document: &Document<'a>) -> NodeId {
        let path = self.document_module(document);
        let node = self.node(&path);
        self.modules.insert(path, node);
        node
    }

    /// The module of `document`, as the symbol of its first occurrence names
    /// it, or as its path does; and the package that symbol is in.
    fn document_module(&mut self, document: &Document<'a>) -> Vec<String> {
        let first = document
            .occurrences
            .first()
            .filter(|first| first.definition);
        if let Some(Ok(Some(symbol))) = first.map(|first| Symbol::parse(first.symbol))
            && matches!(symbol.kind(), Kind::Namespace | Kind::Meta)
            && let Some(path) = module_path(self.syntax, &symbol)
        {
            self.packages.insert(symbol.package_and_version());
            return self.below_package(path);
        }
        self.below_package(file_module_path(self.syntax, &document.path))
    }

    /// `path` below the package: a top-level package of the package's own
    /// name, `boot/` of the project `boot`, is its root, as `lib.rs` is a
    /// crate's.
    fn below_package(&self, mut path: Vec<String>) -> Vec<String> {
        if path
            .first()
            .is_some_and(|first| first.to_lowercase() == self.package)
        {
            path.remove(0);
        }
        path
    }

    fn node(&mut self, path: &[String]) -> NodeId {
        let mut node = self.tree.root();
        for name in path {
            node = self.tree.child(node, NodeKind::Module, name);
        }
        node
    }

    fn add_document(
        &mut self,
        document: &'a Document<'a>,
        module: NodeId,
        text: Text,
        prefix: &str,
    ) {
        let file = FileId::new(self.files.len());
        self.files.push(File {
            path: format!("{prefix}{}", document.path),
            module,
            test: is_test_file(self.syntax, &document.path),
            lines: text.lines(),
        });
        let at = self.units.len();
        let mut holders = Vec::new();
        for occurrence in document.occurrences.iter().filter(|o| o.definition) {
            if self.syntax == Syntax::Script
                && occurrence.symbol.contains("`<constructor>`().(")
                && text.has_modifier(occurrence.range.start)
            {
                self.add_parameter_property(occurrence.symbol);
            }
            let Some(item) = self.item(occurrence.symbol) else {
                continue;
            };
            self.items[item.index()].file.get_or_insert(file);
            let position = occurrence.range.start;
            if self.syntax == Syntax::Python {
                let defined = &self.items[item.index()];
                if defined.kind == ItemKind::Type && text.is_enum(position) {
                    self.enums.insert(item);
                } else if defined.kind == ItemKind::Field && !text.dot_before(position) {
                    let last = defined.name.rsplit("::").next().unwrap_or_default();
                    let constant = last.chars().any(|c| c.is_ascii_uppercase())
                        && !last.chars().any(|c| c.is_ascii_lowercase());
                    if defined
                        .parent
                        .is_some_and(|class| self.enums.contains(&class))
                    {
                        // `RED = auto()`; `self.mass = mass` in its `__init__` is a field.
                        self.items[item.index()].kind = ItemKind::Variant;
                    } else if constant {
                        // `MAX_SIZE = 10` in a class body: an associated constant.
                        self.items[item.index()].kind = ItemKind::Constant;
                    }
                }
            }
            self.definitions
                .entry(item)
                .or_default()
                .push((at, position));
            if self.syntax == Syntax::Script
                && self.items[item.index()].kind == ItemKind::Field
                && text.holds_function(position)
            {
                // `get = (path: string) => ..`: a method, written as a property.
                self.items[item.index()].kind = ItemKind::Function;
            }
            let function = self.items[item.index()].kind == ItemKind::Function;
            let extent = || {
                (self.syntax == Syntax::Script && function)
                    .then(|| text.extent(position))
                    .flatten()
            };
            let Some(block) = occurrence.enclosing.or_else(extent) else {
                continue;
            };
            let kind = &mut self.items[item.index()].kind;
            if *kind == ItemKind::Constant && self.syntax == Syntax::Script {
                // `const f = () => ..`: the index encloses only functions and classes.
                *kind = ItemKind::Function;
            }
            if matches!(*kind, ItemKind::Function | ItemKind::Type) {
                holders.push((block, item));
                self.bodies
                    .entry(item)
                    .and_modify(|known| *known = None)
                    .or_insert(Some((at, position, block)));
            }
        }
        let mut imports = text.imports();
        imports.sort_unstable();
        self.units.push(Unit {
            file,
            module,
            document,
            text,
            imports,
            holders: Blocks::new(holders),
        });
    }

    /// A field for TypeScript's `constructor(private limit: number)`, which
    /// declares a property that the index names as the parameter.
    fn add_parameter_property(&mut self, text: &'a str) {
        let Ok(Some(symbol)) = Symbol::parse(text) else {
            return;
        };
        let Place::In(constructor) = symbol.place() else {
            return;
        };
        let class = self
            .item(constructor)
            .and_then(|constructor| self.items[constructor.index()].parent);
        let (Some(class), Some((_, name))) = (class, text.trim_end_matches(')').rsplit_once('('))
        else {
            return;
        };
        let owner = &self.items[class.index()];
        let id = ItemId::new(self.items.len());
        self.items.push(Item {
            name: format!("{}::{name}", owner.name),
            owner: owner.owner,
            module: None,
            file: None,
            kind: ItemKind::Field,
            parent: Some(class),
            trait_impl: false,
            takes_self: false,
            holds_nothing: false,
        });
        self.symbol_of.push(text);
        self.symbols.insert(text, Named::Item(id));
    }

    fn add_references(&mut self) {
        let units = std::mem::take(&mut self.units);
        for unit in &units {
            for occurrence in &unit.document.occurrences {
                self.add_reference(occurrence, unit);
            }
        }
        self.units = units;
    }

    fn add_reference(&mut self, occurrence: &Occurrence<'a>, unit: &Unit) {
        let Some(item) = self.item(occurrence.symbol) else {
            return;
        };
        if unit.in_import(occurrence.range.start) {
            return;
        }
        let holder = unit.holders.innermost(occurrence.range);
        let kind = self.items[item.index()].kind;
        let name = &self.items[item.index()].name;
        let (getter, setter) = (name.contains("::<get>"), name.contains("::<set>"));
        let access = if occurrence.definition {
            // `self.head = 0` in a method defines `head` in Python, and writes it.
            let in_function =
                holder.is_some_and(|h| self.items[h.index()].kind == ItemKind::Function);
            if kind != ItemKind::Field || self.syntax != Syntax::Python || !in_function {
                return;
            }
            Access::Write
        } else if kind == ItemKind::Field {
            unit.text.access(occurrence.range.start)
        } else if kind == ItemKind::Function {
            let access = unit.text.call_access(occurrence.range.start);
            // TypeScript names both a property's getter and its setter where
            // code names the property; it calls one of them.
            let assigned = (getter || setter) && unit.text.assigned(occurrence.range.start);
            if (getter && assigned) || (setter && !assigned) {
                return;
            }
            access
        } else {
            Access::Read
        };
        let member = (access == Access::Use)
            .then(|| {
                unit.text
                    .member_after(occurrence.range.start, kind == ItemKind::Function)
            })
            .flatten();
        self.whole.push(
            access == Access::Use
                && member.is_none()
                && unit
                    .text
                    .lent_for_now(occurrence.range.start, kind == ItemKind::Function),
        );
        self.members.push(member.map(|member| (unit.file, member)));
        self.reference_at
            .entry((unit.file, occurrence.range.start))
            .or_insert(self.references.len());
        self.references.push(Reference {
            file: unit.file,
            line: occurrence.range.start.line + 1,
            from: unit.module,
            holder,
            item,
            copies: false,
            test: false,
        });
        self.accesses.push(access);
    }

    fn item(&mut self, text: &'a str) -> Option<ItemId> {
        match self.named(text) {
            Named::Item(item) => Some(item),
            _ => None,
        }
    }

    fn named(&mut self, text: &'a str) -> Named {
        if let Some(&known) = self.symbols.get(text) {
            return known;
        }
        let named = match Symbol::parse(text) {
            Ok(Some(symbol)) => self.new_item(text, &symbol),
            Ok(None) => Named::Local,
            Err(_) => Named::NoItem,
        };
        self.symbols.insert(text, named);
        named
    }

    fn new_item(&mut self, text: &'a str, symbol: &Symbol<'a>) -> Named {
        let kind = symbol.kind();
        if !self.packages.contains(&symbol.package_and_version()) {
            return Named::Outside(kind);
        }
        if let Some((container, member, end)) = type_literal_member(text) {
            return self.type_literal_member(text, container, member, end);
        }
        if in_type_literal(text) {
            // A member of an object type in a signature or a type argument,
            // `f(): { a: number }`: nothing the code names.
            return Named::NoItem;
        }
        match kind {
            Kind::Namespace | Kind::Meta | Kind::Macro => return Named::NoItem,
            Kind::Parameter | Kind::TypeParameter => return Named::Parameter,
            Kind::Type | Kind::Term | Kind::Method => {}
        }
        let path = module_path(self.syntax, symbol).map(|path| self.below_package(path));
        let Some(owner) = path.and_then(|path| self.modules.get(&path).copied()) else {
            return Named::Outside(kind);
        };
        let parent = match symbol.place() {
            Place::In(container) => self.item(container),
            Place::Module | Place::Impl(_) => None,
        };
        let parent_kind = parent.map(|parent| self.items[parent.index()].kind);
        let name = symbol.item_name();
        let item_kind = match kind {
            Kind::Type => ItemKind::Type,
            Kind::Method => ItemKind::Function,
            _ => match parent_kind {
                Some(ItemKind::Type) => ItemKind::Field,
                Some(_) => ItemKind::Other,
                None => ItemKind::Constant,
            },
        };
        let dunder = self.syntax == Syntax::Python
            && item_kind == ItemKind::Function
            && name
                .rsplit("::")
                .next()
                .is_some_and(|last| last.starts_with("__") && last.ends_with("__"));
        let id = ItemId::new(self.items.len());
        self.items.push(Item {
            name,
            owner,
            module: None,
            file: None,
            kind: item_kind,
            parent,
            // A protocol's method, `__init__` or `__eq__`, which Python calls
            // and code does not, as a trait's is.
            trait_impl: dunder,
            takes_self: false,
            holds_nothing: false,
        });
        self.symbol_of.push(text);
        Named::Item(id)
    }

    /// What the index says each symbol is, in any of its documents.
    fn read_kinds(&mut self, index: &'a Index<'a>) {
        for information in index
            .documents
            .iter()
            .flat_map(|document| &document.symbols)
        {
            self.kinds.insert(information.symbol, information.kind);
        }
    }

    /// A member of an object type written in place, `type Row = { id: number }`,
    /// which scip-typescript names `Row#typeLiteral0:id.`: a field of `Row`,
    /// or a function of it. What is inside such a member is the member.
    fn type_literal_member(
        &mut self,
        text: &'a str,
        container: &'a str,
        member: &str,
        end: usize,
    ) -> Named {
        if end < text.len() {
            return self.named(&text[..end]);
        }
        let Some(parent) = self.item(container) else {
            return Named::NoItem;
        };
        let owner = &self.items[parent.index()];
        let kind = if text.ends_with(").") {
            ItemKind::Function
        } else {
            ItemKind::Field
        };
        let id = ItemId::new(self.items.len());
        self.items.push(Item {
            name: format!("{}::{member}", owner.name),
            owner: owner.owner,
            module: None,
            file: None,
            kind,
            parent: Some(parent),
            trait_impl: false,
            takes_self: false,
            holds_nothing: false,
        });
        self.symbol_of.push(text);
        Named::Item(id)
    }

    /// The model, once every document is in.
    fn finish(mut self) -> Model {
        self.refine_kinds();
        self.mark_test_code();
        self.mark_methods();
        let scalars = self.scalar_fields();
        self.data = self.data_fields();
        for at in 0..self.references.len() {
            let item = self.references[at].item;
            if self.items[item.index()].kind == ItemKind::Field {
                let reads = self.reads(at, &[item], &scalars, 0);
                self.references[at].copies = reads;
            }
        }
        let wrappers = self.wrappers(&scalars);
        self.mark_lent_reads(&wrappers, &scalars);
        Model {
            tree: self.tree,
            files: self.files,
            items: self.items,
            references: self.references,
            wrappers,
        }
    }

    /// What the index's kinds tell that the descriptors do not: an enum's
    /// members, and getters and setters, which are written as properties.
    fn refine_kinds(&mut self) {
        for (id, symbol) in self.symbol_of.iter().enumerate() {
            let item = &mut self.items[id];
            match self.kinds.get(symbol) {
                Some(&ENUM_MEMBER) if item.kind == ItemKind::Field => item.kind = ItemKind::Variant,
                Some(&(GETTER | SETTER | METHOD | CONSTRUCTOR | FUNCTION | STATIC_METHOD))
                    if item.kind == ItemKind::Field =>
                {
                    item.kind = ItemKind::Function;
                }
                _ => {}
            }
        }
    }

    fn mark_test_code(&mut self) {
        let syntax = self.syntax;
        for id in self.tree.ids() {
            let node = self.tree.node_mut(id);
            node.test = node.kind == NodeKind::Module && is_test_module(syntax, &node.name);
        }
        for reference in &mut self.references {
            reference.test =
                self.files[reference.file.index()].test || self.tree.is_test(reference.from);
        }
    }

    /// Marks the methods that take an instance: in Python those with a
    /// `self` parameter, in TypeScript those of a class not written `static`.
    fn mark_methods(&mut self) {
        for id in 0..self.items.len() {
            let item = &self.items[id];
            let of_type = item
                .parent
                .is_some_and(|parent| self.items[parent.index()].kind == ItemKind::Type);
            if item.kind != ItemKind::Function || !of_type {
                continue;
            }
            // A constructor makes an instance, as Rust's `new` does, and takes none.
            let constructor =
                item.name.ends_with("::<constructor>") || item.name.ends_with("::__init__");
            let takes_self = !constructor
                && match self.syntax {
                    Syntax::Python => {
                        let parameter = format!("{}(self)", self.symbol_of[id]);
                        self.symbols.contains_key(parameter.as_str())
                    }
                    Syntax::Script => {
                        match self.definitions.get(&ItemId::new(id)).map(Vec::as_slice) {
                            Some([(unit, name), ..]) => !self.units[*unit].text.is_static(*name),
                            _ => false,
                        }
                    }
                };
            self.items[id].takes_self = takes_self;
        }
    }

    /// The fields whose definitions say they hold a scalar: a number, a
    /// string or a boolean, which code can only read or overwrite.
    fn scalar_fields(&self) -> HashSet<ItemId> {
        let mut scalars = HashSet::new();
        for (&field, definitions) in &self.definitions {
            if self.items[field.index()].kind != ItemKind::Field {
                continue;
            }
            let declared: Vec<Declared> = definitions
                .iter()
                .map(|&(unit, name)| self.declared(unit, name))
                .collect();
            let annotated = declared.iter().find_map(|d| match d {
                Declared::Annotated(scalar) => Some(*scalar),
                _ => None,
            });
            let scalar = annotated
                .unwrap_or_else(|| declared.iter().all(|d| *d == Declared::Initialized(true)));
            if scalar {
                scalars.insert(field);
            }
        }
        scalars
    }

    fn data_fields(&self) -> HashSet<ItemId> {
        self.definitions
            .iter()
            .filter(|(field, _)| self.items[field.index()].kind == ItemKind::Field)
            .filter(|(_, definitions)| {
                definitions
                    .iter()
                    .any(|&(unit, name)| self.units[unit].text.holds_data(name))
            })
            .map(|(&field, _)| field)
            .collect()
    }

    /// What the definition at `name` in `unit` says of its type, a name it
    /// is given followed to the annotation of the parameter it names.
    fn declared(&self, unit: usize, name: Position) -> Declared {
        let unit = &self.units[unit];
        let Declared::Assigned(value) = unit.text.declared(name) else {
            return unit.text.declared(name);
        };
        let occurrences = &unit.document.occurrences;
        let parameter = occurrences
            .iter()
            .find(|o| o.range.start == value && !o.definition)
            .filter(|o| self.symbols.get(o.symbol) == Some(&Named::Parameter));
        let definition = parameter.and_then(|parameter| {
            occurrences
                .iter()
                .find(|o| o.definition && o.symbol == parameter.symbol)
        });
        match definition.map(|definition| unit.text.declared(definition.range.start)) {
            Some(Declared::Annotated(scalar)) => Declared::Initialized(scalar),
            _ => Declared::Initialized(false),
        }
    }

    /// Every forwarder and accessor.
    fn wrappers(&self, scalars: &HashSet<ItemId>) -> HashMap<ItemId, Wrapper> {
        let mut wrappers = HashMap::new();
        for Forwarding {
            forwarder,
            target,
            fields,
        } in self.forwardings().forwardings()
        {
            if !reaches(&wrappers, target, forwarder) {
                wrappers.insert(forwarder, Wrapper::Forwarder { target, fields });
            }
        }
        for (&function, &body) in &self.bodies {
            let Some((unit, name, range)) = body else {
                continue;
            };
            let item = &self.items[function.index()];
            let accessor_shaped =
                item.kind == ItemKind::Function && item.takes_self && !item.trait_impl;
            if !accessor_shaped || wrappers.contains_key(&function) {
                continue;
            }
            if let Some(accessor) = self.accessor(&self.units[unit], function, name, range, scalars)
            {
                wrappers.insert(function, accessor);
            }
        }
        wrappers
    }

    /// What `function`, a method of a class, is as an accessor, if it is one:
    /// as Rust's are, a body that names fields of its own class and nothing
    /// else of the project's but types, and computes nothing; what its
    /// signature names is no part of it. It lends what
    /// is no scalar and copies what is; and lends to read, unless it writes a
    /// field or hands one to a callback.
    fn accessor(
        &self,
        unit: &Unit,
        function: ItemId,
        name: Position,
        range: Range,
        scalars: &HashSet<ItemId>,
    ) -> Option<Wrapper> {
        let (start, calls) = unit.text.calls_only_names(name, range)?;
        let named = |position: Position| {
            let occurrence = unit
                .document
                .occurrences
                .iter()
                .find(|o| o.range.start == position)?;
            self.symbols.get(occurrence.symbol).copied()
        };
        if calls
            .iter()
            .any(|&call| named(call) != Some(Named::Parameter))
        {
            return None;
        }
        let own = self.items[function.index()].parent;
        let (mut fields, mut written) = (Vec::new(), Vec::new());
        let body = unit.document.occurrences.iter().filter(|o| {
            range.holds(o.range) && o.range.start >= start && !unit.in_import(o.range.start)
        });
        for occurrence in body {
            match self.symbols.get(occurrence.symbol).copied() {
                Some(Named::Item(item)) => {
                    let named = &self.items[item.index()];
                    match named.kind {
                        ItemKind::Field if named.parent == own => {
                            fields.push(item);
                            if unit.text.access(occurrence.range.start) == Access::Write {
                                written.push(item);
                            }
                        }
                        ItemKind::Type | ItemKind::Module | ItemKind::Impl => {}
                        _ => return None,
                    }
                }
                Some(Named::Outside(Kind::Method | Kind::Term | Kind::Macro | Kind::Meta)) => {
                    return None;
                }
                _ => {}
            }
        }
        fields.sort();
        fields.dedup();
        if fields.is_empty() {
            return None;
        }
        // What it overwrites it lends, as Rust's accessors do, scalar or not.
        let (copied, lent) = fields
            .into_iter()
            .partition(|field| scalars.contains(field) && !written.contains(field));
        Some(Wrapper::Accessor {
            lent,
            copied,
            to_read: written.is_empty() && calls.is_empty(),
        })
    }

    /// What each function's body names, as forwarders are told by it.
    fn forwardings(&self) -> Forwardings {
        let mut forwardings = Forwardings::default();
        for (unit_at, unit) in self.units.iter().enumerate() {
            let mut occurrences: Vec<&Occurrence> = unit.document.occurrences.iter().collect();
            occurrences.sort_by_key(|occurrence| occurrence.range.start);
            for (block, function) in unit.holders.iter() {
                let Some(Some((at, name, range))) = self.bodies.get(&function).copied() else {
                    continue;
                };
                let item = &self.items[function.index()];
                if at != unit_at || item.kind != ItemKind::Function {
                    continue;
                }
                let method = item.takes_self && self.syntax == Syntax::Script;
                let mut names: Vec<ForwardingName> = Vec::new();
                if method {
                    names.push(ForwardingName::Parameter {
                        local: "this",
                        is_self: true,
                    });
                }
                let start = occurrences.partition_point(|o| o.range.start < block.start);
                let inside = occurrences[start..]
                    .iter()
                    .take_while(|occurrence| block.holds(occurrence.range))
                    .skip_while(|occurrence| occurrence.range.start != name)
                    .skip(1);
                for occurrence in inside {
                    if unit.in_import(occurrence.range.start) {
                        continue;
                    }
                    let named = self.forwarding_name(occurrence);
                    let through_this = matches!(
                        named,
                        ForwardingName::Field(_) | ForwardingName::Function(_)
                    ) && method
                        && unit.text.this_before(occurrence.range.start);
                    if through_this {
                        names.push(ForwardingName::Value("this"));
                    }
                    names.push(named);
                }
                let may_forward = !unit.text.computes(name, range);
                forwardings.add(function, may_forward, &names);
            }
        }
        forwardings
    }

    fn forwarding_name(&self, occurrence: &Occurrence<'a>) -> ForwardingName<'a> {
        let symbol = occurrence.symbol;
        let named = self.symbols.get(symbol).copied().unwrap_or(Named::NoItem);
        match (named, occurrence.definition) {
            (Named::Parameter, true) => ForwardingName::Parameter {
                local: symbol,
                is_self: symbol.ends_with("(self)"),
            },
            (Named::Parameter | Named::Local, false) => ForwardingName::Value(symbol),
            (_, true) => ForwardingName::Other,
            (Named::Item(item), false) => match self.items[item.index()].kind {
                ItemKind::Type | ItemKind::Module | ItemKind::Impl => ForwardingName::Type,
                ItemKind::Field => ForwardingName::Field(item),
                ItemKind::Function => ForwardingName::Function(Some(item)),
                ItemKind::Variant | ItemKind::Constant | ItemKind::Other => ForwardingName::Other,
            },
            (Named::Outside(kind), false) => match kind {
                Kind::Namespace | Kind::Type | Kind::TypeParameter => ForwardingName::Type,
                Kind::Method => ForwardingName::Function(None),
                _ => ForwardingName::Other,
            },
            (Named::NoItem, false) => ForwardingName::Type,
        }
    }

    /// Whether the code of the reference at `at`, to `fields` or to what hands
    /// them out, only reads their values: it reads it, or reaches into it for
    /// a scalar, or for a field whose code only reads, as Rust reads
    /// `queue.limit().0`. `depth` is how far down a path it has gone.
    fn reads(&self, at: usize, fields: &[ItemId], scalars: &HashSet<ItemId>, depth: usize) -> bool {
        match self.accesses[at] {
            Access::Read => true,
            Access::Write => false,
            Access::Use => {
                let next = self.members[at].and_then(|member| self.reference_at.get(&member));
                fields.iter().all(|field| scalars.contains(field))
                    // Plain data given whole, `sum(q.items)`, `for x in q.items`;
                    // a method called on it, `q.items.append(x)`, `len(q.items)`, is no read.
                    || (self.whole[at]
                        && fields
                            .iter()
                            .all(|field| scalars.contains(field) || self.data.contains(field)))
                    || next.is_some_and(|&next| {
                        let member = self.references[next].item;
                        depth < 8
                            && self.items[member.index()].kind == ItemKind::Field
                            && self.reads(next, &[member], scalars, depth + 1)
                    })
            }
        }
    }

    /// Marks the calls of getters, directly or through forwarders, that only
    /// read what they hand out.
    fn mark_lent_reads(&mut self, wrappers: &HashMap<ItemId, Wrapper>, scalars: &HashSet<ItemId>) {
        for at in 0..self.references.len() {
            let mut called = self.references[at].item;
            while let Some(Wrapper::Forwarder { target, .. }) = wrappers.get(&called) {
                called = *target;
            }
            if let Some(Wrapper::Accessor {
                lent,
                to_read: true,
                ..
            }) = wrappers.get(&called)
                && !lent.is_empty()
            {
                let reads = self.reads(at, lent, scalars, 0);
                self.references[at].copies = reads;
            }
        }
    }
}

impl Unit<'_> {
    fn in_import(&self, position: Position) -> bool {
        let Some(offset) = self.text.offset(position) else {
            return false;
        };
        let before = self.imports.partition_point(|&(start, _)| start <= offset);
        before > 0 && offset < self.imports[before - 1].1
    }
}

/// For a symbol of a member of an object type written in place,
/// `..Row#typeLiteral0:id.`: the symbol of the type it is written in, the
/// member's name, and where the member's own descriptor ends.
fn type_literal_member(text: &str) -> Option<(&str, &str, usize)> {
    let at = text.find("#typeLiteral")? + 1;
    let after = &text[at + "typeLiteral".len()..];
    let digits = after.find(|c: char| !c.is_ascii_digit())?;
    let name = after[digits..].strip_prefix(':')?;
    let length = name.find(['.', '(']).filter(|&length| length > 0)?;
    let start = text.len() - name.len();
    let descriptor = if name[length..].starts_with('(') {
        name[length..].find(").")? + 2
    } else {
        1
    };
    Some((&text[..at], &name[..length], start + length + descriptor))
}

/// Whether `text` is the symbol of something inside an object type written
/// in place, as scip-typescript writes `typeLiteral0:` for one.
fn in_type_literal(text: &str) -> bool {
    text.match_indices("typeLiteral").any(|(at, word)| {
        let digits = text[at + word.len()..].trim_start_matches(|c: char| c.is_ascii_digit());
        digits.len() < text.len() - at - word.len() && digits.starts_with(':')
    })
}

/// Whether the chain of forwarders from `from` passes `to`.
fn reaches(wrappers: &HashMap<ItemId, Wrapper>, from: ItemId, to: ItemId) -> bool {
    let mut at = from;
    loop {
        if at == to {
            return true;
        }
        match wrappers.get(&at) {
            Some(Wrapper::Forwarder { target, .. }) => at = *target,
            _ => return false,
        }
    }
}

/// The module a symbol is in, or is, below its package: the dotted name of
/// Python's first descriptor, `app.store.table`; the directories and file of
/// TypeScript's, `src/store/`table.ts``.
fn module_path(syntax: Syntax, symbol: &Symbol) -> Option<Vec<String>> {
    let namespaces = symbol.namespaces();
    let path: Vec<String> = match syntax {
        Syntax::Python => namespaces.first()?.split('.').map(str::to_owned).collect(),
        Syntax::Script => namespaces.iter().map(|&name| name.to_owned()).collect(),
    };
    Some(normal_path(syntax, path))
}

/// The module of the file at `path`, for a document whose index does not say.
fn file_module_path(syntax: Syntax, path: &str) -> Vec<String> {
    let path = match syntax {
        Syntax::Python => path.trim_end_matches(".py"),
        Syntax::Script => path,
    };
    normal_path(syntax, path.split('/').map(str::to_owned).collect())
}

/// A module's path below its package, without a leading `src`, a file's
/// extension, or a last `__init__` or `index`.
fn normal_path(syntax: Syntax, mut path: Vec<String>) -> Vec<String> {
    if syntax == Syntax::Script
        && let Some(last) = path.last_mut()
        && let Some((stem, _)) = last.rsplit_once('.')
    {
        *last = stem.to_owned();
    }
    if path
        .last()
        .is_some_and(|last| last == "__init__" || last == "index")
    {
        path.pop();
    }
    if path.first().is_some_and(|first| first == "src") {
        path.remove(0);
    }
    path
}

/// Python's `test_x.py`, `x_test.py`, `conftest.py` and what `tests/` holds;
/// TypeScript's `x.test.ts`, `x.spec.ts`, `x.stories.tsx`, and what
/// `__tests__/`, `tests/` and the like hold.
fn is_test_file(syntax: Syntax, path: &str) -> bool {
    let mut parts: Vec<&str> = path.split('/').collect();
    let file = parts.pop().unwrap_or_default();
    let directories = [
        "tests",
        "test",
        "__tests__",
        "__mocks__",
        "e2e",
        "cypress",
        "testing",
    ];
    if parts.iter().any(|part| directories.contains(part)) {
        return true;
    }
    match syntax {
        Syntax::Python => {
            let stem = file.trim_end_matches(".py");
            stem.starts_with("test_") || stem.ends_with("_test") || stem == "conftest"
        }
        Syntax::Script => [".test.", ".spec.", ".stories.", ".e2e."]
            .iter()
            .any(|mark| file.contains(mark)),
    }
}

/// Modules of tests by their names, as files of tests are by theirs.
fn is_test_module(syntax: Syntax, name: &str) -> bool {
    let both = name == "tests" || name == "test" || name == "__tests__" || name == "__mocks__";
    both || match syntax {
        Syntax::Python => {
            name.starts_with("test_")
                || name.ends_with("_test")
                || name.ends_with("_tests")
                || name == "conftest"
        }
        Syntax::Script => [".test", ".spec", ".stories", ".e2e"]
            .iter()
            .any(|end| name.ends_with(end)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modules_are_named_as_the_language_names_them() {
        let path = |syntax, file: &str| file_module_path(syntax, file).join("::");
        assert_eq!(
            path(Syntax::Python, "app/store/table.py"),
            "app::store::table"
        );
        assert_eq!(path(Syntax::Python, "app/store/__init__.py"), "app::store");
        assert_eq!(path(Syntax::Python, "src/pkg/core.py"), "pkg::core");
        assert_eq!(path(Syntax::Script, "src/store/table.ts"), "store::table");
        assert_eq!(path(Syntax::Script, "src/store/index.tsx"), "store");
        assert_eq!(path(Syntax::Script, "lib/queue.test.js"), "lib::queue.test");
        let symbol = |text: &'static str| Symbol::parse(text).unwrap().unwrap();
        let python = symbol("scip-python python api 0 `app.models`/Row#name.");
        assert_eq!(
            module_path(Syntax::Python, &python).unwrap(),
            ["app", "models"]
        );
        let script = symbol("scip-typescript npm web 1.0.0 src/store/`table.ts`/Table#push().");
        assert_eq!(
            module_path(Syntax::Script, &script).unwrap(),
            ["store", "table"]
        );
    }

    #[test]
    fn a_member_of_an_object_type_written_in_place_is_its_type_s() {
        let text = "scip-typescript npm w 1 src/`a.ts`/Row#typeLiteral9:id.";
        let (container, member, end) = type_literal_member(text).unwrap();
        assert_eq!(container, "scip-typescript npm w 1 src/`a.ts`/Row#");
        assert_eq!((member, end), ("id", text.len()));
        let nested = "x src/`a.ts`/Row#typeLiteral1:opts.typeLiteral2:deep.";
        let (_, member, end) = type_literal_member(nested).unwrap();
        assert_eq!(
            (member, &nested[..end]),
            ("opts", "x src/`a.ts`/Row#typeLiteral1:opts.")
        );
        let method = "x src/`a.ts`/Api#typeLiteral0:get().";
        assert_eq!(type_literal_member(method).unwrap().2, method.len());
        assert_eq!(type_literal_member("x src/`a.ts`/Row#id."), None);
        assert!(in_type_literal(
            "x src/`g.ts`/marks().Map:typeLiteral35:odev."
        ));
        assert!(in_type_literal(
            "x src/`a.ts`/load().(extra)typeLiteral27:yon."
        ));
        assert!(!in_type_literal("x src/`a.ts`/typeLiterals."));
    }

    #[test]
    fn tests_by_their_files_and_modules() {
        assert!(is_test_file(Syntax::Python, "tests/test_models.py"));
        assert!(is_test_file(Syntax::Python, "app/models_test.py"));
        assert!(!is_test_file(Syntax::Python, "app/testimony.py"));
        assert!(is_test_file(Syntax::Script, "src/table.test.ts"));
        assert!(is_test_file(Syntax::Script, "src/__tests__/table.ts"));
        assert!(!is_test_file(Syntax::Script, "src/contest.ts"));
        assert!(is_test_module(Syntax::Script, "table.spec"));
        assert!(is_test_module(Syntax::Python, "test_models"));
        assert!(!is_test_module(Syntax::Python, "testing_tools"));
    }
}
