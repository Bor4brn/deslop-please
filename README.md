# deslop-please

[![slop](https://img.shields.io/endpoint?url=https%3A%2F%2Fraw.githubusercontent.com%2Fdeterministic-machines%2Fdeslop-please%2Fbadges%2Fslop.json)](https://github.com/deterministic-machines/deslop-please/actions/workflows/ci.yml?query=branch%3Amain)
[![test](https://github.com/deterministic-machines/deslop-please/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/deterministic-machines/deslop-please/actions/workflows/ci.yml?query=branch%3Amain)
[![crates.io](https://img.shields.io/crates/v/deslop-please.svg)](https://crates.io/crates/deslop-please)
[![license](https://img.shields.io/crates/l/deslop-please.svg)](LICENSE)

An opinionated linter to remove sloppy code. 

It follows *A Philosophy of Software Design*: 
**deep** modules, **narrow** interfaces. 


```bash
cargo install deslop-please
```

```bash
> deslop-please [code-I-never-read] 

==================================================

error[field-leak]: `RunQueue::items` is known outside `RunQueue`: 4 places in 2 modules
  --> src/sched/policy.rs:7:1
  = note: src/sched/policy.rs:7:1: `runqueue::sched::policy::admit`, through `RunQueue::items_mut`
  = note: src/sched/policy.rs:12:1: `runqueue::sched::policy::next`, through `RunQueue::items`
  = note: src/sched/report.rs:4:1: `runqueue::sched::report::backlog`, through `RunQueue::items`
  = note: src/sched/report.rs:8:1: `runqueue::sched::report::drained`, through `RunQueue::items`

deslop-please: 65535 errors, well done!
```

### It's not my fault 

Sure, let's just mute them:

```bash
deslop-please --update-allow .deslop-please.allow  
deslop-please --allow .deslop-please.allow 
```

A module cycle or tangle that loses members stays allowed.

### Add to your `AGENTS.md`

```markdown
-  Run [`deslop-please`](https://github.com/deterministic-machines/deslop-please) to check for sloppy code structure.
-  Add `deslop-please` to the CI.
```

## Why they are sloppy 

They are really just one single idea: **deep** modules, **narrow** interfaces.

Deep modules hide the attention for you and your agents, so you only need to care about the narrow interface.

### module-cycle

Sibling modules that use each other in a circle, so there is no order to read
them in. A module uses a sibling when code written in it names something
written in the sibling, outside `use` declarations and test code. Moving code
to the side that uses it puts them in order.

![module-cycle: before, five sibling modules use each other in cycles; after, they form layers and every arrow points down](docs/module-cycle.svg)

The other lints read one graph: which **things** know which, wherever they
are written. A thing is a type, with its fields, variants and methods wherever
their `impl` blocks are; a free function; or a free constant or static. A
thing knows another when its code names it or a part of it, outside `use`
declarations and test code.

### tangle

Things that know each other in a circle, written in more than one module, or
more of them than you can hold in mind. None of them can be learned alone.
Moving an `impl` block or lifting a type into its parent's file puts the
modules in order and leaves the tangle.

![tangle: before, Tasks and Timers in two modules use each other; after, tick above them uses both and only Tasks uses Timers](docs/tangle.svg)

### field-leak

Code that works on a field of a struct it is not part of: writes it, borrows
it, indexes it, or calls a method on it. The struct must guard its fields: it
keeps one of them to itself and works on them in a method of its own. Its own
methods and the things in its tangle are part of it. Visibility plays no part,
so a `pub` field, a getter that lends the field, a setter, or a function
pasted into its file leaks all the same.

Reading a value is no leak, with or without a getter: taking a copy of a
field, or borrowing plain data such as text and bytes to read it. Whoever
reads a value depends on what it is, not on how the struct keeps it. A method
that passes a call on to a field is no leak either, since its callers never
see the field; it is still not counted as a function, so it makes no module
deeper.

![field-leak: before, three modules access RunQueue fields directly; after, they call push and pop, which own the fields](docs/field-leak.svg)

### shallow-module

A crate, or a module with child modules, that shows more than seven functions,
at least as many as it hides, and a list too long to hold in mind: more than
seven things, or more than seven functions of one thing. You learn an
interface one list at a time, its things and then the functions of one thing,
so a few small types pass where one wide type does not. A thing's functions
are its methods and the module's free functions that use it. It counts its
working functions, neither wrappers nor methods of a trait's `impl`, a method
where its type is defined, and shows those that code outside it uses. It shows
a thing when it shows one of its functions.

![shallow-module: before, callers use 8 of 10 functions; after, they use 2 entry points over a tree of hidden functions](docs/shallow-module.svg)

## Two numbers

There is nothing to configure.

- **7**, what you can hold in mind: Miller, "The Magical Number Seven, Plus
  or Minus Two" (1956). A larger tangle is too much anywhere, and an interface
  of up to seven things, each with up to seven functions, needs no excuse.
- **1**, the break-even of a module: what it shows is the cost of using it and
  what it hides is the benefit. *A Philosophy of Software Design* calls a
  module shallow when "the benefit of not having to learn about how they work
  internally is negated by the cost of learning and using their interfaces";
  they are even when it shows as much as it hides.

## Wrappers

`tangle`, `field-leak` and `shallow-module` see through wrappers, as through
a `pub use`. A use of a wrapper is a use of what it wraps, its own body is no
use, and it is never counted as a function.

- A **forwarder** is a free function or inherent method that only calls one
  other function of the workspace and passes its own parameters on, directly
  or on a field of `self`, like
  `fn main(&self) -> &Tables { self.space.main() }`. A use of it is a use of
  that function. The index shows only the names in a body, so `self.now().0`
  and `Offset(self.size())` also count. A method that passes a field of
  `self` as an argument, like
  `fn kick(&self) { self.signal(&self.doorbell) }`, decides something its
  callers do not, and is no forwarder.
- An **accessor** is an inherent method with `self` that only hands out or
  overwrites fields of `self`, like
  `fn items(&self) -> &[Task] { &self.items }`. Its body names nothing of the
  workspace but those fields and types, has no operator or macro, and calls
  only what passes a value on, such as `clone`, `borrow`, `lock`, `load`,
  `set`, `as_ref` and `unwrap`, or a closure it was given; its signature
  writes the type of each field. A use of it is a use of its fields: a read
  if all it hands out is a copy, as `fn head(&self) -> usize { self.head }`
  does, and work on them if it lends or overwrites them. How the body is laid
  out plays no part: a `let` or a line break hides nothing.

## License

MIT. See [LICENSE](LICENSE).
