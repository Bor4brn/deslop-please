//! SCIP's symbol strings: `<scheme> <manager> <package> <version> <descriptors>`,
//! for example `rust-analyzer cargo my-crate 0.1.0 kinds/epoll/Ready#`.

use std::borrow::Cow;
use std::fmt;

#[derive(Debug, PartialEq)]
pub struct Symbol<'a> {
    pub package: Cow<'a, str>,
    pub version: Cow<'a, str>,
    pub descriptors: Vec<Descriptor<'a>>,
}

#[derive(Debug, PartialEq)]
pub struct Descriptor<'a> {
    pub name: Cow<'a, str>,
    pub kind: Kind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Namespace,
    Type,
    Term,
    Method,
    TypeParameter,
    Parameter,
    Meta,
    Macro,
}

#[derive(Debug)]
pub struct BadSymbol(pub String);

impl fmt::Display for BadSymbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "a SCIP symbol this tool cannot read: {:?}", self.0)
    }
}

/// rust-analyzer's name for a crate's root module.
const ROOT_MODULE: &str = "crate";

impl<'a> Symbol<'a> {
    /// `None` for a local symbol: one that only its own document can name.
    pub fn parse(text: &'a str) -> Result<Option<Symbol<'a>>, BadSymbol> {
        if text.starts_with("local ") {
            return Ok(None);
        }
        let bad = || BadSymbol(text.to_owned());
        let (_scheme, rest) = space_separated(text).ok_or_else(bad)?;
        let (_manager, rest) = space_separated(rest).ok_or_else(bad)?;
        let (package, rest) = space_separated(rest).ok_or_else(bad)?;
        let (version, rest) = space_separated(rest).ok_or_else(bad)?;
        let descriptors = descriptors(rest).ok_or_else(bad)?;
        Ok(Some(Symbol {
            package,
            version,
            descriptors,
        }))
    }

    /// The module this symbol names, as the path below its crate's root, if it names one.
    pub fn module(&self) -> Option<Vec<&str>> {
        let is_module = self.descriptors.iter().all(|d| d.kind == Kind::Namespace);
        is_module.then(|| self.namespaces())
    }

    /// The module the symbol is in, or is: its leading namespaces.
    pub fn namespaces(&self) -> Vec<&str> {
        let path: Vec<&str> = self
            .descriptors
            .iter()
            .take_while(|d| d.kind == Kind::Namespace)
            .map(|d| d.name.as_ref())
            .collect();
        if path == [ROOT_MODULE] {
            Vec::new()
        } else {
            path
        }
    }

    /// What its last descriptor says the symbol is; a module's is a namespace.
    pub fn kind(&self) -> Kind {
        self.descriptors.last().map_or(Kind::Namespace, |d| d.kind)
    }

    /// A function or method: `serve().`, `impl#[Ready]new().`.
    pub fn is_function(&self) -> bool {
        self.kind() == Kind::Method
    }

    /// A function that no trait stands between a caller and: a free function,
    /// or a function of an `impl` with no trait. `serve().`, `impl#[Ready]new().`.
    pub fn is_free_or_inherent_function(&self) -> bool {
        let rest: Vec<&Descriptor> = self
            .descriptors
            .iter()
            .skip_while(|d| d.kind == Kind::Namespace)
            .collect();
        match rest[..] {
            [function] => function.kind == Kind::Method,
            [block, ty, function] => {
                block.kind == Kind::Type
                    && block.name == "impl"
                    && ty.kind == Kind::TypeParameter
                    && function.kind == Kind::Method
            }
            _ => false,
        }
    }

    /// An `impl` block itself, not an item inside one: `kinds/impl#[Ready]`.
    pub fn is_impl_block(&self) -> bool {
        let last_named = self
            .descriptors
            .iter()
            .rev()
            .find(|d| d.kind != Kind::TypeParameter);
        last_named.is_some_and(|d| d.kind == Kind::Type && d.name == "impl")
            && self
                .descriptors
                .last()
                .is_some_and(|d| d.kind == Kind::TypeParameter)
    }

    /// Whether this item lives inside a trait `impl` block (`impl Trait for Type`).
    pub fn is_trait_impl_item(&self) -> bool {
        let mut rest = self
            .descriptors
            .iter()
            .skip_while(|d| d.kind == Kind::Namespace)
            .peekable();
        while let Some(descriptor) = rest.next() {
            if descriptor.kind == Kind::Type && descriptor.name == "impl" {
                let mut params = 0;
                while rest.next_if(|d| d.kind == Kind::TypeParameter).is_some() {
                    params += 1;
                }
                if params >= 2 && rest.peek().is_some() {
                    return true;
                }
            }
        }
        false
    }

    /// Owning type name for a field, variant, or inherent/trait method (generics stripped).
    pub fn parent_type(&self) -> Option<String> {
        let mut rest = self
            .descriptors
            .iter()
            .skip_while(|d| d.kind == Kind::Namespace)
            .peekable();
        let first = rest.next()?;
        if first.kind == Kind::Type && first.name == "impl" {
            let ty = rest.next_if(|d| d.kind == Kind::TypeParameter)?;
            while rest.next_if(|d| d.kind == Kind::TypeParameter).is_some() {}
            rest.peek()?;
            return Some(strip_generics(&ty.name));
        }
        if first.kind == Kind::Type && rest.peek().is_some() {
            return Some(strip_generics(&first.name));
        }
        None
    }

    /// How a reader would write the symbol inside its module: `Ready`, `Ready::new`, `<Ready as Display>::fmt`.
    pub fn item_name(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        let mut rest = self
            .descriptors
            .iter()
            .skip_while(|d| d.kind == Kind::Namespace)
            .peekable();
        while let Some(descriptor) = rest.next() {
            if descriptor.kind == Kind::Type && descriptor.name == "impl" {
                let mut types = Vec::new();
                while let Some(parameter) = rest.next_if(|d| d.kind == Kind::TypeParameter) {
                    types.push(parameter.name.as_ref());
                }
                parts.push(match types.as_slice() {
                    [ty, interface] => format!("<{ty} as {interface}>"),
                    _ => types.join(""),
                });
            } else if descriptor.kind == Kind::Macro {
                parts.push(format!("{}!", descriptor.name));
            } else {
                parts.push(descriptor.name.to_string());
            }
        }
        parts.join("::")
    }
}

fn strip_generics(name: &str) -> String {
    match name.find('<') {
        Some(index) => name[..index].to_owned(),
        None => name.to_owned(),
    }
}

/// A space ends a field; two spaces are one space inside it.
fn space_separated(text: &str) -> Option<(Cow<'_, str>, &str)> {
    let bytes = text.as_bytes();
    let mut i = 0;
    let mut escaped = false;
    while i < bytes.len() {
        if bytes[i] == b' ' {
            if bytes.get(i + 1) == Some(&b' ') {
                escaped = true;
                i += 2;
                continue;
            }
            let field = &text[..i];
            let field = if escaped {
                Cow::Owned(field.replace("  ", " "))
            } else {
                Cow::Borrowed(field)
            };
            return Some((field, &text[i + 1..]));
        }
        i += 1;
    }
    None
}

fn descriptors(mut text: &str) -> Option<Vec<Descriptor<'_>>> {
    let mut parsed = Vec::new();
    while !text.is_empty() {
        let (descriptor, rest) = descriptor(text)?;
        parsed.push(descriptor);
        text = rest;
    }
    (!parsed.is_empty()).then_some(parsed)
}

fn descriptor(text: &str) -> Option<(Descriptor<'_>, &str)> {
    let enclosed = |open: &str, close: char, kind: Kind| {
        let (name, rest) = name(text.strip_prefix(open)?)?;
        Some((Descriptor { name, kind }, rest.strip_prefix(close)?))
    };
    if text.starts_with('[') {
        return enclosed("[", ']', Kind::TypeParameter);
    }
    if text.starts_with('(') {
        return enclosed("(", ')', Kind::Parameter);
    }
    let (name, rest) = name(text)?;
    let mut chars = rest.chars();
    let kind = match chars.next()? {
        '/' => Kind::Namespace,
        '#' => Kind::Type,
        '.' => Kind::Term,
        ':' => Kind::Meta,
        '!' => Kind::Macro,
        '(' => {
            let close = rest.find(").")?;
            return Some((
                Descriptor {
                    name,
                    kind: Kind::Method,
                },
                &rest[close + 2..],
            ));
        }
        _ => return None,
    };
    Some((Descriptor { name, kind }, chars.as_str()))
}

fn name(text: &str) -> Option<(Cow<'_, str>, &str)> {
    if let Some(quoted) = text.strip_prefix('`') {
        let mut end = 0;
        loop {
            end += quoted[end..].find('`')?;
            if quoted[end + 1..].starts_with('`') {
                end += 2;
            } else {
                break;
            }
        }
        let raw = &quoted[..end];
        let name = if raw.contains("``") {
            Cow::Owned(raw.replace("``", "`"))
        } else {
            Cow::Borrowed(raw)
        };
        return Some((name, &quoted[end + 1..]));
    }
    let end = text
        .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '+' | '-' | '$')))
        .unwrap_or(text.len());
    (end > 0).then(|| (Cow::Borrowed(&text[..end]), &text[end..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Symbol<'_> {
        Symbol::parse(text).unwrap().unwrap()
    }

    #[test]
    fn a_module_symbol_gives_its_package_and_path() {
        let symbol = parse("rust-analyzer cargo my-crate 0.1.0 kinds/epoll/");
        assert_eq!(symbol.package, "my-crate");
        assert_eq!(symbol.version, "0.1.0");
        assert_eq!(symbol.module(), Some(vec!["kinds", "epoll"]));
    }

    #[test]
    fn the_crate_root_is_the_empty_path() {
        assert_eq!(
            parse("rust-analyzer cargo a 0.1.0 crate/").module(),
            Some(vec![])
        );
        assert_eq!(
            parse("rust-analyzer cargo a 0.1.0 Files#").namespaces(),
            Vec::<&str>::new()
        );
    }

    #[test]
    fn items_are_not_modules_but_say_which_module_they_are_in() {
        let symbol = parse("rust-analyzer cargo a 0.1.0 kinds/epoll/impl#[Ready]new().");
        assert_eq!(symbol.module(), None);
        assert_eq!(symbol.namespaces(), ["kinds", "epoll"]);
        assert_eq!(symbol.item_name(), "Ready::new");
    }

    #[test]
    fn an_impl_block_is_told_from_its_items() {
        assert!(parse("rust-analyzer cargo a 0.1.0 kinds/impl#[Ready]").is_impl_block());
        assert!(parse("rust-analyzer cargo a 0.1.0 impl#[Ready][Display]").is_impl_block());
        assert!(!parse("rust-analyzer cargo a 0.1.0 kinds/impl#[Ready]new().").is_impl_block());
        assert!(!parse("rust-analyzer cargo a 0.1.0 kinds/Ready#").is_impl_block());
    }

    #[test]
    fn free_and_inherent_functions_are_told_from_trait_ones() {
        let free_or_inherent = |descriptors: &str| {
            parse(&format!("rust-analyzer cargo a 0.1.0 {descriptors}"))
                .is_free_or_inherent_function()
        };
        assert!(free_or_inherent("kinds/serve()."));
        assert!(free_or_inherent("kinds/impl#[`Ready<T>`]new()."));
        assert!(!free_or_inherent("kinds/impl#[Ready][Display]fmt()."));
        assert!(!free_or_inherent("kinds/Serve#serve()."));
        assert!(!free_or_inherent("kinds/Ready#ready."));
        assert!(parse("rust-analyzer cargo a 0.1.0 kinds/Serve#serve().").is_function());
        assert!(!parse("rust-analyzer cargo a 0.1.0 kinds/").is_function());
    }

    #[test]
    fn escaped_names_and_trait_impls() {
        let symbol = parse(
            "rust-analyzer cargo std https://x/std os/impl#[`fs::Metadata`][MetadataExt]mode().",
        );
        assert_eq!(symbol.version, "https://x/std");
        assert_eq!(symbol.item_name(), "<fs::Metadata as MetadataExt>::mode");
        let quoted = parse("rust-analyzer cargo a 0.1.0 `a``b`#");
        assert_eq!(quoted.descriptors[0].name, "a`b");
    }

    #[test]
    fn spaces_in_a_package_name_are_doubled() {
        assert_eq!(
            parse("rust-analyzer cargo my  crate 1 m/").package,
            "my crate"
        );
    }

    #[test]
    fn locals_are_not_symbols_and_garbage_is_an_error() {
        assert!(Symbol::parse("local 12").unwrap().is_none());
        assert!(Symbol::parse("rust-analyzer cargo a 0.1.0 ").is_err());
        assert!(Symbol::parse("rust-analyzer cargo a 0.1.0 x%").is_err());
    }
}
