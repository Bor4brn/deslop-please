//! Forwarders: functions whose body only calls one other function and passes
//! their own parameters on. The index holds no syntax, only the names written
//! in a function's item, in order. A forwarder's names are its signature's
//! parameters and types, then one call to a workspace function whose receiver
//! and arguments are its parameters or field paths of `self`, and nothing else.

use std::collections::HashMap;

use crate::model::ItemId;

/// A name written in a function's item, as far as forwarders are told by it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Name<'a> {
    /// The definition of one of the function's parameters, by its local symbol.
    Parameter {
        local: &'a str,
        is_self: bool,
    },
    /// A use of a local value: a parameter or a variable.
    Value(&'a str),
    /// What signatures and paths name: a type, trait, lifetime, or module.
    Type,
    Field,
    /// A function, if it is one of the workspace's.
    Function(Option<ItemId>),
    /// Anything else, and the definition of anything but a parameter.
    Other,
}

#[derive(Default)]
pub struct Definitions {
    /// `None` for a function defined more than once: no one body describes it.
    functions: HashMap<ItemId, Option<Function>>,
}

struct Function {
    parameters: usize,
    call: Option<Call>,
}

#[derive(Clone, Copy)]
struct Call {
    target: ItemId,
    operands: usize,
}

impl Definitions {
    /// One definition of `function`, from the names its item writes after its
    /// own. Only a function that `may_forward` can be a forwarder.
    pub fn add(&mut self, function: ItemId, may_forward: bool, names: &[Name]) {
        let read = Function::read(function, may_forward, names);
        self.functions
            .entry(function)
            .and_modify(|known| *known = None)
            .or_insert(Some(read));
    }

    /// Each forwarder, in order, with the function it calls. Its call passes
    /// as many operands as that function has parameters: a call that seems to
    /// pass fewer passes something the index does not show, such as a literal.
    pub fn forwardings(&self) -> Vec<(ItemId, ItemId)> {
        let parameters = |function| {
            self.functions
                .get(&function)
                .and_then(Option::as_ref)
                .map(|known: &Function| known.parameters)
        };
        let mut links: Vec<(ItemId, ItemId)> = self
            .functions
            .iter()
            .filter_map(|(&function, known)| {
                let call = known.as_ref()?.call?;
                (parameters(call.target) == Some(call.operands)).then_some((function, call.target))
            })
            .collect();
        links.sort();
        links
    }
}

impl Function {
    fn read(itself: ItemId, may_forward: bool, names: &[Name]) -> Function {
        let signature = names
            .iter()
            .take_while(|name| matches!(name, Name::Parameter { .. } | Name::Type))
            .count();
        let (signature, body) = names.split_at(signature);
        let parameters: Vec<(&str, bool)> = signature
            .iter()
            .filter_map(|name| match *name {
                Name::Parameter { local, is_self } => Some((local, is_self)),
                _ => None,
            })
            .collect();
        let call = if may_forward {
            only_call(itself, &parameters, body)
        } else {
            None
        };
        Function {
            parameters: parameters.len(),
            call,
        }
    }
}

/// The call `body` makes, if making it is all `body` does.
fn only_call(itself: ItemId, parameters: &[(&str, bool)], body: &[Name]) -> Option<Call> {
    let at = body
        .iter()
        .position(|name| matches!(name, Name::Function(_)))?;
    let Name::Function(Some(target)) = body[at] else {
        return None;
    };
    let receiver = operands(parameters, &body[..at])?;
    let arguments = operands(parameters, &body[at + 1..])?;
    (target != itself && receiver <= 1).then_some(Call {
        target,
        operands: receiver + arguments,
    })
}

/// How many operands `names` are, if each is a parameter or a field path of `self`.
fn operands(parameters: &[(&str, bool)], names: &[Name]) -> Option<usize> {
    let mut count = 0;
    let mut in_self = false;
    for name in names {
        match *name {
            Name::Value(local) => {
                let &(_, is_self) = parameters
                    .iter()
                    .find(|(parameter, _)| *parameter == local)?;
                count += 1;
                in_self = is_self;
            }
            Name::Field if in_self => {}
            _ => return None,
        }
    }
    Some(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TARGET: ItemId = ItemId(0);
    const SELF: Name = Name::Parameter {
        local: "local 0",
        is_self: true,
    };
    const ID: Name = Name::Parameter {
        local: "local 1",
        is_self: false,
    };
    const USE_SELF: Name = Name::Value("local 0");
    const USE_ID: Name = Name::Value("local 1");
    const CALL: Name = Name::Function(Some(TARGET));

    /// `fn target(&self, id: Id) -> Out { .. }`, with a body of its own.
    fn with_target() -> Definitions {
        let mut definitions = Definitions::default();
        let names = [SELF, ID, Name::Type, USE_SELF, Name::Field, Name::Other];
        definitions.add(TARGET, true, &names);
        definitions
    }

    #[test]
    fn a_call_on_a_field_of_self_that_passes_the_parameters_forwards() {
        let mut definitions = with_target();
        // fn forwarder(&self, id: Id) -> Out { self.inner.target(id) }
        let names = [SELF, ID, Name::Type, USE_SELF, Name::Field, CALL, USE_ID];
        definitions.add(ItemId(1), true, &names);
        assert_eq!(definitions.forwardings(), [(ItemId(1), TARGET)]);
    }

    #[test]
    fn a_call_that_passes_what_the_index_does_not_show_does_not_forward() {
        let mut definitions = with_target();
        // fn literal(&self) { self.target(0) }
        definitions.add(ItemId(1), true, &[SELF, USE_SELF, CALL]);
        // fn projected(&self, id: Id) { self.target(id.0) }
        let projected = [SELF, ID, USE_SELF, CALL, USE_ID, Name::Field];
        definitions.add(ItemId(2), true, &projected);
        assert_eq!(definitions.forwardings(), []);
    }

    #[test]
    fn a_trait_method_a_second_definition_or_a_second_call_does_not_forward() {
        let mut definitions = with_target();
        let forwarding = [SELF, ID, USE_SELF, CALL, USE_ID];
        definitions.add(ItemId(1), false, &forwarding);
        definitions.add(ItemId(2), true, &forwarding);
        definitions.add(ItemId(2), true, &forwarding);
        let calls_twice = [&forwarding[..], &[Name::Function(None)]].concat();
        definitions.add(ItemId(3), true, &calls_twice);
        assert_eq!(definitions.forwardings(), []);
    }
}
