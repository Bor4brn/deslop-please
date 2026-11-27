//! What the index leaves out of a Python, TypeScript or JavaScript file, read
//! from its tokens: which names are in `import` statements, what code does
//! with a field it names, whether a field's declared type or first value is a
//! scalar, and what a method's body is when it only hands out or overwrites a
//! field. Comments are left out; a string, f-string or template literal is
//! one token, so a name inside one is only read.

use super::Syntax;
use crate::scip::{Position, Range};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Ident,
    /// A number, string, template literal or regular expression.
    Literal,
    Punct,
    Open,
    Close,
    /// The end of a Python statement: a line break outside brackets.
    Newline,
}

#[derive(Debug, Clone, Copy)]
struct Token {
    kind: Kind,
    start: usize,
    end: usize,
}

/// What code does with a field it names, or with what a call returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// Only reads it: a name inside a string, a keyword argument, a key of an
    /// object literal, a value given whole.
    Read,
    /// Writes, deletes, increments or indexes it: `q.head = 0`, `q.items[0]`.
    Write,
    /// Calls a method on it, reaches into it, or passes it on: `q.items.append(x)`,
    /// `len(q.items)`. Only a scalar is read so.
    Use,
}

/// What a field's definition says of its type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Declared {
    /// An annotation, scalar or not: `size: int`, `rows: Row[]`.
    Annotated(bool),
    /// No annotation, and a first value, a scalar literal or not: `self.head = 0`.
    Initialized(bool),
    /// No annotation, and a first value that is one name, at this position:
    /// `self.limit = limit`, which has the type of what it names.
    Assigned(Position),
    Unknown,
}

pub struct Text {
    text: String,
    syntax: Syntax,
    line_starts: Vec<usize>,
    tokens: Vec<Token>,
}

const SCRIPT_PUNCTUATION: [&str; 33] = [
    ">>>=", "===", "!==", "**=", "...", "<<=", ">>=", ">>>", "&&=", "||=", "??=", "=>", "==", "!=",
    "<=", ">=", "&&", "||", "??", "?.", "++", "--", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=",
    "**", "<<", ">>",
];

const PYTHON_PUNCTUATION: [&str; 25] = [
    "**=", "//=", ">>=", "<<=", "...", "->", ":=", "==", "!=", "<=", ">=", "+=", "-=", "*=", "/=",
    "%=", "&=", "|=", "^=", "@=", "**", "//", "<<", ">>", "<>",
];

const PYTHON_SCALARS: [&str; 15] = [
    "int", "float", "complex", "bool", "str", "bytes", "None", "Optional", "Final", "ClassVar",
    "Literal", "Union", "typing", "Decimal", "datetime",
];

const SCRIPT_SCALARS: [&str; 11] = [
    "number",
    "string",
    "boolean",
    "bigint",
    "symbol",
    "null",
    "undefined",
    "Number",
    "String",
    "Boolean",
    "Date",
];

const SCALAR_VALUES: [&str; 7] = [
    "True",
    "False",
    "None",
    "true",
    "false",
    "null",
    "undefined",
];

/// Words after which a `/` starts a regular expression, not a division.
const BEFORE_REGEX: [&str; 13] = [
    "return", "typeof", "case", "do", "else", "in", "of", "new", "delete", "void", "throw",
    "yield", "await",
];

/// Words that make a body compute something, so it forwards nothing.
const COMPUTING: [&str; 41] = [
    "if",
    "else",
    "elif",
    "for",
    "while",
    "not",
    "and",
    "or",
    "in",
    "is",
    "lambda",
    "yield",
    "new",
    "typeof",
    "delete",
    "void",
    "throw",
    "try",
    "except",
    "finally",
    "with",
    "def",
    "class",
    "const",
    "let",
    "var",
    "function",
    "switch",
    "case",
    "break",
    "continue",
    "raise",
    "pass",
    "assert",
    "import",
    "from",
    "global",
    "nonlocal",
    "instanceof",
    "do",
    "of",
];

impl Text {
    pub fn new(text: String, syntax: Syntax) -> Text {
        let line_starts = std::iter::once(0)
            .chain(text.match_indices('\n').map(|(at, _)| at + 1))
            .collect();
        let tokens = read(&text, syntax);
        Text {
            text,
            syntax,
            line_starts,
            tokens,
        }
    }

    pub fn lines(&self) -> usize {
        self.text.lines().count()
    }

    /// The byte offset of `position`, whose column counts UTF-16 code units
    /// as the language servers behind both indexers do.
    pub fn offset(&self, position: Position) -> Option<usize> {
        let start = *self.line_starts.get(position.line as usize)?;
        let mut units = 0;
        for (at, c) in self.text[start..].char_indices() {
            if units >= position.column as usize || c == '\n' {
                return Some(start + at);
            }
            units += c.len_utf16();
        }
        Some(self.text.len())
    }

    /// Where each import is, by byte offsets: Python's `import` and `from`
    /// statements; `import ... from "m"`, `export ... from "m"`, `export { .. }`
    /// and `const x = require("m")`.
    pub fn imports(&self) -> Vec<(usize, usize)> {
        let tokens = &self.tokens;
        let mut imports = Vec::new();
        let mut at = 0;
        while at < tokens.len() {
            let found = match self.syntax {
                Syntax::Python => self.python_import(at),
                Syntax::Script => self.script_import(at),
            };
            match found {
                Some((start, end)) => {
                    imports.push((tokens[start].start, tokens[end].end));
                    at = end + 1;
                }
                None => at += 1,
            }
        }
        imports
    }

    /// The first and last token of the import statement starting at `at`.
    fn python_import(&self, at: usize) -> Option<(usize, usize)> {
        let statement_start = at == 0 || self.tokens[at - 1].kind == Kind::Newline;
        if !statement_start || !(self.is(at, "import") || self.is(at, "from")) {
            return None;
        }
        let end = (at..self.tokens.len())
            .find(|&i| self.tokens[i].kind == Kind::Newline)
            .unwrap_or(self.tokens.len());
        Some((at, end - 1))
    }

    /// The first and last token of the import that starts at, or for a
    /// `require` reaches back from, `at`.
    fn script_import(&self, at: usize) -> Option<(usize, usize)> {
        let tokens = &self.tokens;
        let after = |i: usize| (i + 1 < tokens.len()).then_some(i + 1);
        if self.is(at, "require") && after(at).is_some_and(|i| self.is(i, "(")) {
            // `const { a } = require("m")`: from the declaration to the `)`.
            let close = self.matching(at + 1)?;
            let declaration = (at.saturating_sub(48)..at)
                .rev()
                .take_while(|&i| !self.is(i, ";"))
                .find(|&i| self.is(i, "const") || self.is(i, "let") || self.is(i, "var"))?;
            return Some((declaration, close));
        }
        let statement_start = at == 0
            || self.is(at - 1, ";")
            || self.is(at - 1, "}")
            || self.line(tokens[at - 1].start) < self.line(tokens[at].start);
        if !statement_start {
            return None;
        }
        let next = after(at)?;
        if self.is(at, "import") && !self.is(next, "(") && !self.is(next, ".") {
            // Up to the module's name, the first string after `import`.
            let name = (next..tokens.len()).find(|&i| tokens[i].kind == Kind::Literal)?;
            return Some((at, name));
        }
        if self.is(at, "export") {
            let mut open = next;
            if self.is(open, "type") {
                open += 1;
            }
            if self.is(open, "*") {
                let name = (open..tokens.len()).find(|&i| tokens[i].kind == Kind::Literal)?;
                return Some((at, name));
            }
            if self.is(open, "{") {
                let close = self.matching(open)?;
                let from = after(close).filter(|&i| self.is(i, "from"));
                return Some((at, from.and_then(after).unwrap_or(close)));
            }
        }
        None
    }

    /// What the code that names a field at `name` does with it.
    pub fn access(&self, name: Position) -> Access {
        let Some(at) = self.offset(name).and_then(|offset| self.token_at(offset)) else {
            return Access::Read;
        };
        if self.tokens[at].kind == Kind::Literal || at == 0 {
            return Access::Read;
        }
        if !self.is_dot(at - 1) {
            // `const { items } = queue`: takes the value whole, as `queue.items` alone does.
            return if self.destructured(at) {
                Access::Use
            } else {
                Access::Read
            };
        }
        // Back over `a.b.c` to the start of the path.
        let mut first = at;
        while first >= 2 && self.is_dot(first - 1) && self.tokens[first - 2].kind == Kind::Ident {
            first -= 2;
        }
        if first > 0
            && ["del", "delete", "++", "--"]
                .iter()
                .any(|word| self.is(first - 1, word))
        {
            return Access::Write;
        }
        self.after(at + 1)
    }

    /// Whether the path that ends at `name`, or the call that `name` makes
    /// when `call`, is lent for the moment only: an argument of a call,
    /// `sum(q.items)`, or what `for x in`, `x in`, `for (const x of` or a
    /// spread `...` goes through. Kept in a name, `items = q.items`, it can be
    /// changed later; Python's `len`, `iter`, `next` and `reversed` call a
    /// method of it, as Rust's `q.items.len()` does.
    pub fn lent_for_now(&self, name: Position, call: bool) -> bool {
        let Some(mut first) = self.offset(name).and_then(|offset| self.token_at(offset)) else {
            return false;
        };
        let mut last = first;
        if call && last + 1 < self.tokens.len() && self.is(last + 1, "(") {
            match self.matching(last + 1) {
                Some(close) => last = close,
                None => return false,
            }
        }
        while first >= 2 && self.is_dot(first - 1) && self.tokens[first - 2].kind == Kind::Ident {
            first -= 2;
        }
        if first == 0 || last + 1 >= self.tokens.len() {
            return false;
        }
        let before = self.token_text(self.tokens[first - 1]);
        if matches!(before, "in" | "of" | "...") {
            return true;
        }
        let argument =
            matches!(before, "(" | ",") && (self.is(last + 1, ")") || self.is(last + 1, ","));
        if !argument {
            return false;
        }
        // Back to the call's `(`: a call's, not a tuple's or a group's.
        let mut open = first - 1;
        let mut depth = 0usize;
        while self.token_text(self.tokens[open]) != "(" || depth > 0 {
            match self.token_text(self.tokens[open]) {
                ")" | "]" | "}" => depth += 1,
                "(" | "[" | "{" => match depth.checked_sub(1) {
                    Some(less) => depth = less,
                    None => return false,
                },
                _ => {}
            }
            if open == 0 {
                return false;
            }
            open -= 1;
        }
        let protocol = self.syntax == Syntax::Python
            && open >= 1
            && ["len", "iter", "next", "reversed"]
                .iter()
                .any(|word| self.is(open - 1, word));
        open >= 1 && self.tokens[open - 1].kind == Kind::Ident && !protocol
    }

    /// Whether token `at` is a key of an object pattern that a value is
    /// assigned to, `const { items, head: h } = queue`.
    fn destructured(&self, at: usize) -> bool {
        if self.syntax != Syntax::Script || !(self.is(at - 1, "{") || self.is(at - 1, ",")) {
            return false;
        }
        let mut depth = 0usize;
        for i in at + 1..self.tokens.len() {
            match self.token_text(self.tokens[i]) {
                "{" | "(" | "[" => depth += 1,
                ")" | "]" => match depth.checked_sub(1) {
                    Some(less) => depth = less,
                    None => return false,
                },
                ";" => return false,
                "}" if depth > 0 => depth -= 1,
                "}" => return i + 1 < self.tokens.len() && self.is(i + 1, "="),
                _ => {}
            }
        }
        false
    }

    /// Whether the code that names a property at `name` assigns to it, which
    /// calls its setter: `q.head = 1`, `q.head += 1`, `delete q.head`.
    pub fn assigned(&self, name: Position) -> bool {
        let indexed = self
            .offset(name)
            .and_then(|offset| self.token_at(offset))
            .is_some_and(|at| at + 1 < self.tokens.len() && self.is(at + 1, "["));
        !indexed && self.access(name) == Access::Write
    }

    /// What the code that names a function at `name` does with what it returns.
    pub fn call_access(&self, name: Position) -> Access {
        let Some(at) = self.offset(name).and_then(|offset| self.token_at(offset)) else {
            return Access::Read;
        };
        if self.tokens[at].kind == Kind::Literal {
            return Access::Read;
        }
        if at + 1 < self.tokens.len() && self.is(at + 1, "(") {
            return match self.matching(at + 1) {
                Some(close) => self.after(close + 1),
                None => Access::Use,
            };
        }
        self.access(name)
    }

    /// What the tokens from `next` on do with the value before them.
    fn after(&self, mut next: usize) -> Access {
        // `q.items!.push(x)`: TypeScript's non-null assertion changes nothing.
        if next < self.tokens.len() && self.is(next, "!") && !self.is_followed_by_equals(next) {
            next += 1;
        }
        let Some(token) = self.tokens.get(next) else {
            return Access::Use;
        };
        let text = self.token_text(*token);
        let assigns = text == "="
            || (text.len() > 1
                && text.ends_with('=')
                && !matches!(text, "==" | "===" | "!=" | "!==" | "<=" | ">=" | "=>"));
        if assigns || text == "++" || text == "--" || text == "[" {
            Access::Write
        } else {
            Access::Use
        }
    }

    fn is_followed_by_equals(&self, at: usize) -> bool {
        self.tokens
            .get(at + 1)
            .is_some_and(|t| self.token_text(*t) == "=")
            && self.tokens[at].end == self.tokens[at + 1].start
    }

    /// What the definition of a field at `name` says of its type.
    pub fn declared(&self, name: Position) -> Declared {
        let Some(at) = self.offset(name).and_then(|offset| self.token_at(offset)) else {
            return Declared::Unknown;
        };
        let line = self.line(self.tokens[at].start);
        let mut next = at + 1;
        while next < self.tokens.len() && (self.is(next, "?") || self.is(next, "!")) {
            next += 1;
        }
        let ends = |i: usize, depth: usize| {
            let Some(token) = self.tokens.get(i) else {
                return true;
            };
            token.kind == Kind::Newline
                || (depth == 0
                    && (matches!(self.token_text(*token), "=" | ";" | "," | ")" | "}")
                        || (self.syntax == Syntax::Script && self.line(token.start) > line)))
        };
        if next < self.tokens.len() && self.is(next, ":") {
            let scalars: &[&str] = match self.syntax {
                Syntax::Python => &PYTHON_SCALARS,
                Syntax::Script => &SCRIPT_SCALARS,
            };
            let (mut depth, mut i, mut any) = (0usize, next + 1, false);
            let mut scalar = true;
            while !ends(i, depth) {
                let token = self.tokens[i];
                let text = self.token_text(token);
                match token.kind {
                    // `number[]`: an array of scalars is no scalar.
                    Kind::Open
                        if text == "["
                            && self
                                .tokens
                                .get(i + 1)
                                .is_some_and(|t| self.token_text(*t) == "]") =>
                    {
                        scalar = false;
                        depth += 1;
                    }
                    Kind::Open => depth += 1,
                    Kind::Close => depth = depth.saturating_sub(1),
                    Kind::Ident => scalar &= scalars.contains(&text),
                    Kind::Literal => {}
                    Kind::Punct => scalar &= matches!(text, "|" | "." | "," | "?"),
                    Kind::Newline => {}
                }
                any = true;
                i += 1;
            }
            return Declared::Annotated(any && scalar);
        }
        if next < self.tokens.len() && self.is(next, "=") {
            let mut value = next + 1;
            if value < self.tokens.len() && self.is(value, "-") {
                value += 1;
            }
            let Some(&token) = self.tokens.get(value) else {
                return Declared::Initialized(false);
            };
            let literal = token.kind == Kind::Literal
                && !self.token_text(token).starts_with('/')
                && !self.token_text(token).contains("${");
            let word = token.kind == Kind::Ident;
            if word && !SCALAR_VALUES.contains(&self.token_text(token)) && ends(value + 1, 0) {
                return Declared::Assigned(self.position(token.start));
            }
            let scalar = literal || (word && SCALAR_VALUES.contains(&self.token_text(token)));
            return Declared::Initialized(scalar && ends(value + 1, 0));
        }
        Declared::Unknown
    }

    /// Whether the definition at `name` says it holds plain data, as Rust's
    /// `Vec<u32>` or `Option<String>` does: a list, tuple or array of scalars,
    /// by its annotation (`list[int]`, `tuple[str, ...]`, `number[]`,
    /// `readonly string[]`, `Array<number>`, `[number, string]`) or, without
    /// one, by a first value of scalar literals (`= ("a", "b")`). An empty
    /// `[]` tells nothing of what it will hold.
    pub fn holds_data(&self, name: Position) -> bool {
        let Some(at) = self.offset(name).and_then(|offset| self.token_at(offset)) else {
            return false;
        };
        let line = self.line(self.tokens[at].start);
        let mut next = at + 1;
        while next < self.tokens.len() && (self.is(next, "?") || self.is(next, "!")) {
            next += 1;
        }
        let Some(&after) = self.tokens.get(next) else {
            return false;
        };
        let (scalars, holders): (&[&str], &[&str]) = match self.syntax {
            Syntax::Python => (
                &PYTHON_SCALARS,
                &["list", "List", "tuple", "Tuple", "Sequence"],
            ),
            Syntax::Script => (&SCRIPT_SCALARS, &["Array", "ReadonlyArray", "readonly"]),
        };
        match self.token_text(after) {
            ":" => {
                let (mut depth, mut i, mut held) = (0usize, next + 1, false);
                while let Some(&token) = self.tokens.get(i) {
                    let text = self.token_text(token);
                    let ends = token.kind == Kind::Newline
                        || (depth == 0
                            && (matches!(text, "=" | ";" | "," | ")" | "}")
                                || (self.syntax == Syntax::Script
                                    && self.line(token.start) > line)));
                    if ends {
                        break;
                    }
                    match token.kind {
                        Kind::Open => {
                            held |= text == "[";
                            depth += 1;
                        }
                        Kind::Close => depth = depth.saturating_sub(1),
                        Kind::Ident if holders.contains(&text) => held = true,
                        Kind::Ident if scalars.contains(&text) => {}
                        Kind::Ident => return false,
                        Kind::Punct
                            if !matches!(text, "|" | "." | "," | "?" | "<" | ">" | "...") =>
                        {
                            return false;
                        }
                        _ => {}
                    }
                    i += 1;
                }
                held
            }
            "=" => {
                let open = next + 1;
                if open >= self.tokens.len() || !(self.is(open, "[") || self.is(open, "(")) {
                    return false;
                }
                let Some(close) = self.matching(open) else {
                    return false;
                };
                close > open + 1
                    && (open + 1..close).all(|i| {
                        let token = self.tokens[i];
                        let text = self.token_text(token);
                        (token.kind == Kind::Literal
                            && !text.starts_with('/')
                            && !text.contains("${"))
                            || SCALAR_VALUES.contains(&text)
                            || matches!(text, "," | "-")
                            || token.kind == Kind::Newline
                    })
            }
            _ => false,
        }
    }

    /// Whether the definition at `name` is written `static`, TypeScript's
    /// way to say a member takes no instance.
    pub fn is_static(&self, name: Position) -> bool {
        let Some(at) = self.offset(name).and_then(|offset| self.token_at(offset)) else {
            return false;
        };
        let line = self.line(self.tokens[at].start);
        (0..at)
            .rev()
            .take_while(|&i| self.line(self.tokens[i].start) == line)
            .any(|i| self.is(i, "static"))
    }

    /// The whole definition of the function whose name is at `name`, to the
    /// end of its body, for one the index gives no range: a getter or setter.
    pub fn extent(&self, name: Position) -> Option<Range> {
        let at = self.token_at(self.offset(name)?)?;
        let open = (at..self.tokens.len().min(at + 3)).find(|&i| self.is(i, "("))?;
        let close = self.matching(open)?;
        let brace = (close + 1..self.tokens.len())
            .take_while(|&i| !self.is(i, ";"))
            .find(|&i| self.is(i, "{"))?;
        let end = self.matching(brace)?;
        Some(Range {
            start: name,
            end: self.position(self.tokens[end].end),
        })
    }

    /// Whether the property defined at `name` holds a function its class
    /// writes: its value is an arrow function or a function expression,
    /// `get = <T>(p: string) => ..`. A property that only has a function's
    /// type, `onChange: (value: string) => void`, is a slot that others fill,
    /// a field, as a Rust struct's field that holds a closure is.
    pub fn holds_function(&self, name: Position) -> bool {
        let Some(at) = self.offset(name).and_then(|offset| self.token_at(offset)) else {
            return false;
        };
        let mut next = at + 1;
        while next < self.tokens.len() && (self.is(next, "?") || self.is(next, "!")) {
            next += 1;
        }
        if next < self.tokens.len() && self.is(next, ":") {
            // Past the type, `p: (x: T) => U = (x) => ..`, to the value.
            let mut depth = 0usize;
            next += 1;
            while next < self.tokens.len() {
                match self.token_text(self.tokens[next]) {
                    "(" | "[" | "{" | "<" => depth += 1,
                    ")" | "]" | ">" => depth = depth.saturating_sub(1),
                    "}" if depth == 0 => return false,
                    "}" => depth -= 1,
                    ";" | "," if depth == 0 => return false,
                    "=" if depth == 0 => break,
                    _ => {}
                }
                next += 1;
            }
        }
        if next >= self.tokens.len() || !self.is(next, "=") {
            return false;
        }
        let mut value = next + 1;
        if value < self.tokens.len() && self.is(value, "async") {
            value += 1;
        }
        value < self.tokens.len() && (self.is(value, "function") || self.is_function_at(value))
    }

    /// Whether an arrow function, or a function's type, starts at token `at`:
    /// `(a) =>`, `<T>(a: T) =>`, `a =>`.
    fn is_function_at(&self, mut at: usize) -> bool {
        if at < self.tokens.len() && self.is(at, "<") {
            let mut depth = 0usize;
            while at < self.tokens.len() {
                match self.token_text(self.tokens[at]) {
                    "<" => depth += 1,
                    ">" => depth -= 1,
                    _ => {}
                }
                at += 1;
                if depth == 0 {
                    break;
                }
            }
        }
        let Some(&token) = self.tokens.get(at) else {
            return false;
        };
        let after = if self.token_text(token) == "(" {
            match self.matching(at) {
                Some(close) => close + 1,
                None => return false,
            }
        } else if token.kind == Kind::Ident {
            at + 1
        } else {
            return false;
        };
        if after >= self.tokens.len() {
            return false;
        }
        // `(a: T): R => ..`: past a return type, to the arrow.
        self.is(after, "=>")
            || (self.is(after, ":")
                && (after..self.tokens.len().min(after + 16))
                    .take_while(|&i| !self.is(i, ";") && !self.is(i, "{") && !self.is(i, "="))
                    .any(|i| self.is(i, "=>")))
    }

    /// Whether the parameter defined at `name` is written with an access
    /// modifier, `private limit: number`, which makes it a property.
    pub fn has_modifier(&self, name: Position) -> bool {
        let Some(at) = self.offset(name).and_then(|offset| self.token_at(offset)) else {
            return false;
        };
        (0..at)
            .rev()
            .take_while(|&i| !self.is(i, "(") && !self.is(i, ","))
            .any(|i| {
                ["private", "public", "protected", "readonly", "override"]
                    .iter()
                    .any(|word| self.is(i, word))
            })
    }

    /// The tokens of the body of the function defined at `name`, within
    /// `item`, the whole definition: past the signature, and a docstring.
    fn body(&self, name: Position, item: Range) -> Option<(usize, usize)> {
        let at = self.token_at(self.offset(name)?)?;
        let end_offset = self.offset(item.end)?;
        let end = self
            .tokens
            .partition_point(|token| token.start < end_offset);
        let open = (at..end).find(|&i| self.is(i, "(") || self.is(i, "=>"))?;
        let after_parameters = if self.is(open, "(") {
            self.matching(open)? + 1
        } else {
            open
        };
        match self.syntax {
            Syntax::Python => {
                let colon = (after_parameters..end).find(|&i| self.is(i, ":"))?;
                let mut start = colon + 1;
                while start < end && self.tokens[start].kind == Kind::Newline {
                    start += 1;
                }
                let docstring = start + 1 < end
                    && self.tokens[start].kind == Kind::Literal
                    && self.tokens[start + 1].kind == Kind::Newline;
                if docstring {
                    start += 2;
                }
                Some((start, end))
            }
            Syntax::Script => {
                let arrow = (after_parameters..end).find(|&i| self.is(i, "=>"));
                let brace = (after_parameters..end).find(|&i| self.is(i, "{"));
                match (arrow, brace) {
                    (Some(arrow), brace) if brace.is_none_or(|brace| arrow < brace) => {
                        let start = arrow + 1;
                        if start < end && self.is(start, "{") {
                            Some((start + 1, self.matching(start)?))
                        } else {
                            Some((start, end))
                        }
                    }
                    (_, Some(brace)) => Some((brace + 1, self.matching(brace)?)),
                    _ => None,
                }
            }
        }
    }

    /// Whether the body of the function at `name` does more than make one
    /// call: an operator, a literal, a keyword that branches or binds.
    pub fn computes(&self, name: Position, item: Range) -> bool {
        let Some((start, end)) = self.body(name, item) else {
            return true;
        };
        self.tokens[start..end].iter().any(|&token| {
            let text = self.token_text(token);
            match token.kind {
                Kind::Literal => true,
                Kind::Ident => COMPUTING.contains(&text),
                Kind::Punct => !matches!(text, "." | "?." | "," | ";"),
                Kind::Open | Kind::Close => !matches!(text, "(" | ")"),
                Kind::Newline => false,
            }
        })
    }

    /// When the body of the function at `name` computes nothing, as an
    /// accessor's does: no literal, no operator but `=`, no keyword that
    /// branches, no index, nothing built with braces, and no call of anything
    /// but a name. Then where the body starts, past the signature, and the
    /// positions of the names it calls, which the caller checks are
    /// parameters: `each(this.items)` lends a field to a callback. A local,
    /// `items = self._items`, and a tuple, `return (a, b)`, hide nothing.
    pub fn calls_only_names(
        &self,
        name: Position,
        item: Range,
    ) -> Option<(Position, Vec<Position>)> {
        let (start, end) = self.body(name, item)?;
        let keyword = |at: usize| {
            ["return", "await", "yield"]
                .iter()
                .any(|word| self.is(at, word))
        };
        let mut calls = Vec::new();
        for at in start..end {
            let token = self.tokens[at];
            let text = self.token_text(token);
            let after_name =
                at > start && self.tokens[at - 1].kind == Kind::Ident && !keyword(at - 1);
            match token.kind {
                Kind::Literal => return None,
                Kind::Ident
                    if COMPUTING.contains(&text) && !matches!(text, "const" | "let" | "var") =>
                {
                    return None;
                }
                Kind::Punct if !matches!(text, "." | "?." | "," | ";" | "=" | ":") => return None,
                Kind::Open if text == "{" => return None,
                // `[]`: a new value is made, as Rust's `Vec::new()` makes one.
                Kind::Open
                    if text == "["
                        && self
                            .tokens
                            .get(at + 1)
                            .is_some_and(|t| self.token_text(*t) == "]") =>
                {
                    return None;
                }
                Kind::Open
                    if text == "[" && (after_name || self.tokens[at - 1].kind == Kind::Close) =>
                {
                    return None;
                }
                Kind::Open if text == "(" && after_name => {
                    calls.push(self.position(self.tokens[at - 1].start));
                }
                _ => {}
            }
        }
        let body = self
            .tokens
            .get(start)
            .map_or(item.end, |token| self.position(token.start));
        Some((body, calls))
    }

    /// Where the member that code reaches into right after the name at
    /// `name` is named: `city` in `user.address.city`, and, for a function,
    /// `value` in `queue.limit().value`.
    pub fn member_after(&self, name: Position, call: bool) -> Option<Position> {
        let at = self.token_at(self.offset(name)?)?;
        let mut next = at + 1;
        if call && next < self.tokens.len() && self.is(next, "(") {
            next = self.matching(next)? + 1;
        }
        if next < self.tokens.len() && self.is(next, "!") {
            next += 1;
        }
        let member = self.tokens.get(next + 1)?;
        (self.is_dot(next) && member.kind == Kind::Ident).then(|| self.position(member.start))
    }

    /// Whether the class whose name is at `name` is a Python enum: one of its
    /// bases is `Enum`, `IntEnum`, `StrEnum`, `Flag` or `IntFlag`, written
    /// alone or as `enum.Enum`.
    pub fn is_enum(&self, name: Position) -> bool {
        let Some(at) = self.offset(name).and_then(|offset| self.token_at(offset)) else {
            return false;
        };
        let open = at + 1;
        if open >= self.tokens.len() || !self.is(open, "(") {
            return false;
        }
        let Some(close) = self.matching(open) else {
            return false;
        };
        (open + 1..close).any(|i| {
            let word = self.token_text(self.tokens[i]);
            ["Enum", "IntEnum", "StrEnum", "Flag", "IntFlag"].contains(&word)
        })
    }

    /// Whether a `.` is written right before the name at `name`: `self.mass`,
    /// not `RED` in a class body.
    pub fn dot_before(&self, name: Position) -> bool {
        let Some(at) = self.offset(name).and_then(|offset| self.token_at(offset)) else {
            return false;
        };
        at >= 1 && self.is_dot(at - 1)
    }

    /// Whether `this.` is written right before the name at `name`.
    pub fn this_before(&self, name: Position) -> bool {
        let Some(at) = self.offset(name).and_then(|offset| self.token_at(offset)) else {
            return false;
        };
        at >= 2 && self.is_dot(at - 1) && self.is(at - 2, "this")
    }

    /// The position of the byte at `offset`.
    pub fn position(&self, offset: usize) -> Position {
        let line = self.line(offset);
        let start = self.line_starts[line];
        let column: usize = self.text[start..offset].chars().map(char::len_utf16).sum();
        Position {
            line: u32::try_from(line).unwrap_or(u32::MAX),
            column: u32::try_from(column).unwrap_or(u32::MAX),
        }
    }

    fn line(&self, offset: usize) -> usize {
        self.line_starts.partition_point(|&start| start <= offset) - 1
    }

    /// The token that holds the byte at `offset`.
    fn token_at(&self, offset: usize) -> Option<usize> {
        let at = self.tokens.partition_point(|token| token.end <= offset);
        self.tokens
            .get(at)
            .filter(|token| token.start <= offset)
            .map(|_| at)
    }

    fn token_text(&self, token: Token) -> &str {
        &self.text[token.start..token.end]
    }

    fn is(&self, at: usize, text: &str) -> bool {
        self.token_text(self.tokens[at]) == text
    }

    fn is_dot(&self, at: usize) -> bool {
        self.is(at, ".") || self.is(at, "?.")
    }

    /// The bracket that closes the one at `open`.
    fn matching(&self, open: usize) -> Option<usize> {
        let mut depth = 0usize;
        for (i, token) in self.tokens.iter().enumerate().skip(open) {
            match token.kind {
                Kind::Open => depth += 1,
                Kind::Close => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                _ => {}
            }
        }
        None
    }
}

fn read(text: &str, syntax: Syntax) -> Vec<Token> {
    let bytes = text.as_bytes();
    let mut tokens: Vec<Token> = Vec::new();
    let mut depth = 0usize;
    let mut at = 0;
    let python = syntax == Syntax::Python;
    while at < bytes.len() {
        let rest = &text[at..];
        let b = bytes[at];
        let first = rest.chars().next().unwrap_or_default();
        let (kind, length) = match b {
            b'\n' if python && depth == 0 => {
                let after_statement = tokens.last().is_some_and(|t| t.kind != Kind::Newline);
                (after_statement.then_some(Kind::Newline), 1)
            }
            b'\\' if python && rest[1..].starts_with('\n') => (None, 2),
            // `\u{a0}` and the like are whitespace too, of more than one byte.
            _ if first.is_whitespace() => (None, first.len_utf8()),
            b'#' if python => (None, rest.find('\n').unwrap_or(rest.len())),
            _ if !python && rest.starts_with("//") => (None, rest.find('\n').unwrap_or(rest.len())),
            _ if !python && rest.starts_with("/*") => {
                (None, rest.find("*/").map_or(rest.len(), |end| end + 2))
            }
            b'"' | b'\'' => (Some(Kind::Literal), quoted(rest, 0, python)),
            b'`' if !python => (Some(Kind::Literal), template(rest)),
            b'/' if !python && regex_allowed(text, &tokens) => match regex(rest) {
                Some(length) => (Some(Kind::Literal), length),
                None => (Some(Kind::Punct), 1),
            },
            b'0'..=b'9' => (Some(Kind::Literal), number(rest)),
            b'(' | b'[' | b'{' => {
                depth += 1;
                (Some(Kind::Open), 1)
            }
            b')' | b']' | b'}' => {
                depth = depth.saturating_sub(1);
                (Some(Kind::Close), 1)
            }
            _ if b == b'_' || b == b'$' || first.is_alphanumeric() => {
                let length = rest
                    .find(|c: char| !(c == '_' || c == '$' || c.is_alphanumeric()))
                    .unwrap_or(rest.len());
                let word = &rest[..length];
                let prefixes_string = python
                    && length <= 2
                    && word.chars().all(|c| "rRbBuUfF".contains(c))
                    && matches!(bytes.get(at + length), Some(b'"' | b'\''));
                if prefixes_string {
                    (Some(Kind::Literal), quoted(rest, length, python))
                } else {
                    (Some(Kind::Ident), length)
                }
            }
            _ => {
                let punctuation: &[&str] = if python {
                    &PYTHON_PUNCTUATION
                } else {
                    &SCRIPT_PUNCTUATION
                };
                let length = punctuation
                    .iter()
                    .find(|p| rest.starts_with(*p))
                    .map_or_else(
                        || rest.chars().next().map_or(1, char::len_utf8),
                        |p| p.len(),
                    );
                (Some(Kind::Punct), length)
            }
        };
        if let Some(kind) = kind {
            tokens.push(Token {
                kind,
                start: at,
                end: at + length,
            });
        }
        // Never inside a character: a token of nothing still steps over one.
        at += length.max(first.len_utf8()).max(1);
    }
    tokens
}

/// The length of the string at `rest`, its quote at `quote`: triple-quoted
/// in Python, or ending at its line's end if it is never closed.
fn quoted(rest: &str, quote: usize, python: bool) -> usize {
    let bytes = rest.as_bytes();
    let mark = bytes[quote];
    let triple =
        python && bytes.get(quote + 1) == Some(&mark) && bytes.get(quote + 2) == Some(&mark);
    let mut at = quote + if triple { 3 } else { 1 };
    while at < bytes.len() {
        match bytes[at] {
            b'\\' => at += 2,
            b'\n' if !triple => return at,
            c if c == mark => {
                if !triple {
                    return at + 1;
                }
                if bytes.get(at + 1) == Some(&mark) && bytes.get(at + 2) == Some(&mark) {
                    return at + 3;
                }
                at += 1;
            }
            _ => at += 1,
        }
    }
    bytes.len()
}

/// The length of the template literal at `rest`, `${..}` parts and all.
fn template(rest: &str) -> usize {
    let bytes = rest.as_bytes();
    let (mut at, mut depth) = (1, 0usize);
    while at < bytes.len() {
        match bytes[at] {
            b'\\' => at += 1,
            b'$' if bytes.get(at + 1) == Some(&b'{') => {
                depth += 1;
                at += 1;
            }
            b'}' if depth > 0 => depth -= 1,
            b'`' if depth == 0 => return at + 1,
            _ => {}
        }
        at += 1;
    }
    bytes.len()
}

/// Whether a `/` here starts a regular expression: after an operator, an
/// opening bracket, or a word like `return`, not after a value.
fn regex_allowed(text: &str, tokens: &[Token]) -> bool {
    tokens.last().is_none_or(|last| match last.kind {
        Kind::Literal | Kind::Close => false,
        Kind::Ident => BEFORE_REGEX.contains(&&text[last.start..last.end]),
        _ => true,
    })
}

/// The length of the regular expression at `rest`, flags and all, if it
/// closes on its line.
fn regex(rest: &str) -> Option<usize> {
    let bytes = rest.as_bytes();
    let (mut at, mut class) = (1, false);
    while at < bytes.len() {
        match bytes[at] {
            b'\\' => at += 1,
            b'\n' => return None,
            b'[' => class = true,
            b']' => class = false,
            b'/' if !class => {
                let flags = rest[at + 1..]
                    .find(|c: char| !c.is_ascii_alphabetic())
                    .unwrap_or(rest.len() - at - 1);
                return Some(at + 1 + flags);
            }
            _ => {}
        }
        at += 1;
    }
    None
}

fn number(rest: &str) -> usize {
    rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '.'))
        .unwrap_or(rest.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn access(text: &str, syntax: Syntax, needle: &str) -> Access {
        let source = Text::new(text.to_owned(), syntax);
        source.access(source.position(text.rfind(needle).unwrap()))
    }

    #[test]
    fn any_text_is_read_in_whole_characters() {
        let texts = [
            "const a = \"x\" · b; // ölçü — ok\n",
            "x\u{a0}=\u{a0}1; y = `ş${ı}ğ`; z = 'ü\\é';",
            "<p>Öğrenci’nin ödevi: 🎒 {count}</p>",
            "a = r'\\·' # yorum ·\nb = 1\u{2028}c = 2\n",
            "// only a comment ✓",
            "x = /[·]+/u.test(y) ? \"·\" : '·'",
        ];
        for text in texts {
            for syntax in [Syntax::Python, Syntax::Script] {
                let read = Text::new(text.to_owned(), syntax);
                for token in &read.tokens {
                    assert!(
                        text.is_char_boundary(token.start) && text.is_char_boundary(token.end),
                        "{text:?}: {token:?}"
                    );
                }
            }
        }
        let words = Text::new("öğrenci.ödevler.push(x)".to_owned(), Syntax::Script);
        assert_eq!(words.token_text(words.tokens[0]), "öğrenci");
        assert_eq!(words.token_text(words.tokens[2]), "ödevler");
    }

    #[test]
    fn what_code_does_with_a_field() {
        let py = Syntax::Python;
        assert_eq!(access("q.head = 1\n", py, "head"), Access::Write);
        assert_eq!(access("q.head += 1\n", py, "head"), Access::Write);
        assert_eq!(access("q.items[0]\n", py, "items"), Access::Write);
        assert_eq!(access("del q.items\n", py, "items"), Access::Write);
        assert_eq!(access("q.items.append(1)\n", py, "items"), Access::Use);
        assert_eq!(access("len(q.items)\n", py, "items"), Access::Use);
        assert_eq!(access("Row(name='a')\n", py, "name"), Access::Read);
        assert_eq!(access("f\"{q.items}\"\n", py, "items"), Access::Read);
        assert_eq!(access("x = q.head == 1\n", py, "head"), Access::Use);
        let ts = Syntax::Script;
        assert_eq!(access("q.head++;", ts, "head"), Access::Write);
        assert_eq!(access("++q.head;", ts, "head"), Access::Write);
        assert_eq!(access("q.items!.push(1);", ts, "items"), Access::Use);
        assert_eq!(access("q?.head ??= 2;", ts, "head"), Access::Write);
        assert_eq!(access("f({ name: 1 });", ts, "name"), Access::Read);
        assert_eq!(access("`${q.items}`;", ts, "items"), Access::Read);
    }

    #[test]
    fn the_member_reached_into_next() {
        let text = "a = user.address.city\nb = queue.limit().value\nc = q.items\n";
        let source = Text::new(text.to_owned(), Syntax::Python);
        let at = |needle: &str| source.position(text.find(needle).unwrap());
        assert_eq!(source.member_after(at("address"), false), Some(at("city")));
        assert_eq!(source.member_after(at("limit"), true), Some(at("value")));
        assert_eq!(source.member_after(at("items"), false), None);
    }

    #[test]
    fn what_a_definition_declares() {
        let declared = |text: &str, syntax, needle: &str| {
            let source = Text::new(text.to_owned(), syntax);
            source.declared(source.position(text.find(needle).unwrap()))
        };
        let py = Syntax::Python;
        assert_eq!(
            declared("    size: int = 0\n", py, "size"),
            Declared::Annotated(true)
        );
        assert_eq!(
            declared("    rows: list[Row]\n", py, "rows"),
            Declared::Annotated(false)
        );
        assert_eq!(
            declared("    name: Optional[str]\n", py, "name"),
            Declared::Annotated(true)
        );
        assert_eq!(
            declared("        self.head = 0\n", py, "head"),
            Declared::Initialized(true)
        );
        assert_eq!(
            declared("        self.rows = []\n", py, "rows"),
            Declared::Initialized(false)
        );
        assert_eq!(
            declared("        self.n = a + 1\n", py, "n"),
            Declared::Initialized(false)
        );
        assert!(matches!(
            declared("        self.limit = limit\n", py, "limit"),
            Declared::Assigned(_)
        ));
        assert_eq!(
            declared("        self.x = a.b\n", py, "x"),
            Declared::Initialized(false)
        );
        let ts = Syntax::Script;
        assert_eq!(
            declared("  private name: string;\n", ts, "name"),
            Declared::Annotated(true)
        );
        assert_eq!(
            declared("  size?: number | null\n  x = 1", ts, "size"),
            Declared::Annotated(true)
        );
        assert_eq!(
            declared("  rows: Row[] = [];\n", ts, "rows"),
            Declared::Annotated(false)
        );
        assert_eq!(
            declared("  items: number[] = [];\n", ts, "items"),
            Declared::Annotated(false)
        );
        assert_eq!(
            declared("  head = 0;\n", ts, "head"),
            Declared::Initialized(true)
        );
    }

    #[test]
    fn properties_that_hold_functions_are_methods() {
        let holds = |text: &str| {
            let source = Text::new(text.to_owned(), Syntax::Script);
            source.holds_function(source.position(text.find("p").unwrap()))
        };
        assert!(holds("  p = <T>(yol: string) => this.ham<T>({ yol });"));
        assert!(holds("  p = async (x) => { await x; };"));
        assert!(holds("  p = x => x + 1;"));
        assert!(holds("  p = function (x) { return x; };"));
        assert!(holds("  p = (a: number): number => a;"));
        assert!(holds("  p: (a: number) => number = (a) => a;"));
        assert!(holds("  p: Handler<T> = async (a) => { await a; };"));
        // A slot that whoever builds the object fills: a field.
        assert!(!holds("  p?: (value: string) => void;"));
        assert!(!holds("  p: (value: string) => void, q: number;"));
        assert!(!holds("  p: () => void }"));
        assert!(!holds("  p = [];"));
        assert!(!holds("  p: number[] = [];"));
        assert!(!holds("  p = (a + b) * 2;"));
        assert!(!holds("  p = (x) * f((y) => y);"));
        assert!(!holds("  p: Map<string, number>;"));
    }

    #[test]
    fn imports_are_found_and_code_is_not() {
        let python = "import a.b as c\nfrom x import (\n    y,\n    z,\n)\nv = 1\n";
        let text = Text::new(python.to_owned(), Syntax::Python);
        let imports: Vec<&str> = text.imports().iter().map(|&(s, e)| &python[s..e]).collect();
        assert_eq!(
            imports,
            ["import a.b as c", "from x import (\n    y,\n    z,\n)"]
        );
        let script = "import { a } from \"./a\";\nimport \"./side\";\nexport { b } from './b';\n\
                      export * from './c';\nconst { d } = require('./d');\nconst e = import('./e');\n\
                      export const f = 1;\n";
        let text = Text::new(script.to_owned(), Syntax::Script);
        let imports: Vec<&str> = text.imports().iter().map(|&(s, e)| &script[s..e]).collect();
        assert_eq!(
            imports,
            [
                "import { a } from \"./a\"",
                "import \"./side\"",
                "export { b } from './b'",
                "export * from './c'",
                "const { d } = require('./d')",
            ]
        );
    }

    #[test]
    fn getters_setters_and_bodies_that_compute() {
        // What the body of the one method in `method`, called `name`, calls,
        // if it computes nothing; and whether it computes, as forwarders see.
        let body = |method: &str, name: &str| {
            let text = Text::new(method.to_owned(), Syntax::Python);
            let whole = Range {
                start: Position { line: 0, column: 0 },
                end: text.position(method.len()),
            };
            let name = text.position(method.find(name).unwrap());
            (
                text.calls_only_names(name, whole)
                    .map(|(_, calls)| calls.len()),
                text.computes(name, whole),
            )
        };
        let getter = "def items(self):\n    \"\"\"The items.\"\"\"\n    return self._items\n";
        assert_eq!(body(getter, "items").0, Some(0));
        let local = "def items(self):\n    items = self._items\n    return items\n";
        assert_eq!(body(local, "items").0, Some(0));
        let tuple = "def parts(self):\n    return (self._items, self._head)\n";
        assert_eq!(body(tuple, "parts").0, Some(0));
        let lends = "def with_items(self, each):\n    return each(self._items)\n";
        assert_eq!(body(lends, "with_items").0, Some(1));
        let setter = "def set_head(self, head):\n    self.head = head\n";
        assert_eq!(body(setter, "set_head").0, Some(0));
        let computes = "def size(self):\n    return len(self._items) + 1\n";
        assert_eq!(body(computes, "size"), (None, true));
        let indexes = "def first(self):\n    return self._items[0]\n";
        assert_eq!(body(indexes, "first").0, None);
        let resets = "def reset(self):\n    self._items = []\n";
        assert_eq!(body(resets, "reset").0, None);
        let pair = "def parts(self):\n    return [self._items, self._head]\n";
        assert_eq!(body(pair, "parts").0, Some(0));
        let forwards = "def push(self, x):\n    return self._inner.push(x)\n";
        assert_eq!(body(forwards, "push"), (Some(1), false));
        let script = "class Q {\n  push(x: number): void { this.inner.push(x); }\n  static of() { return new Q(); }\n}\n";
        let text = Text::new(script.to_owned(), Syntax::Script);
        let line = |line: u32, end: u32| Range {
            start: Position { line, column: 2 },
            end: Position { line, column: end },
        };
        let push = text.position(script.find("push").unwrap());
        assert!(!text.computes(push, line(1, 48)));
        let of = text.position(script.find("of()").unwrap());
        assert!(text.computes(of, line(2, 32)));
        assert!(text.this_before(text.position(script.find("inner").unwrap())));
        assert!(text.is_static(of));
        let constructor = "constructor(private readonly limit: number, name: string) {}";
        let text = Text::new(constructor.to_owned(), Syntax::Script);
        let at = |needle: &str| text.position(constructor.find(needle).unwrap());
        assert!(text.has_modifier(at("limit")));
        assert!(!text.has_modifier(at("name")));
    }
}
