# deslop-please

An opinionated linter to remove sloppy code. 

It follows *A Philosophy of Software Design*: 
**deep** modules, **narrow** interfaces. 


```bash
cargo install deslop-please
```

```bash
> deslop-please [code-I-never-read] 

==================================================

error[module-cycle]: cyclic dependency in `problems`: `kinds` <-> `socket`
  --> src/kinds/stream.rs:1:1
  = note: `kinds` -> `socket` uses only 1 item (Socket); cutting it frees `kinds`, `socket`
  = note: src/kinds/stream.rs:1:1: `kinds` -> `socket` (1 item)
  = note: src/socket.rs:1:1: `socket` -> `kinds` (2 items)

deslop-please: 65535 errors, well done!
```

### It's not my fault 

Sure, let's just mute them:

```bash
deslop-please --update-allow .deslop-please.allow  
deslop-please --allow .deslop-please.allow 
```

### Add to your `AGENTS.md`

```markdown
-  Run [`deslop-please`](https://github.com/deterministic-machines/deslop-please) to check for sloppy code structure.
-  Add `deslop-please` to the CI.
```

## Why they are sloppy 

They are really just one single idea: **deep** modules, **narrow** interfaces.

Deep modules hide the attention for you and your agents, so you only need to care about the narrow interface.

### module-cycle

Sibling modules that use each other in a circle.

![module-cycle: before, five sibling modules use each other in cycles; after, they form layers and every arrow points down](docs/module-cycle.svg)

### split-module

Two modules in different places that lean on each other so much they are
really one module.

![split-module: before, vm::memory and arch::paging depend on each other across the tree; after, paging is a child of memory](docs/split-module.svg)

### facade-bypass

Code that reaches past a module into one of its submodules.

![facade-bypass: before, socket and files reach into kinds submodules; after, they use kinds and its submodules are private](docs/facade-bypass.svg)


### field-leak

Code that reads or writes a field of a struct owned by another module, when
that struct has private fields.

![field-leak: before, three modules access RunQueue fields directly; after, they call push and pop, which own the fields](docs/field-leak.svg)

### chatty-seam

One module calling many different functions of another.

![chatty-seam: before, app functions call many memory functions; after, they call two memory operations](docs/chatty-seam.svg)

### shallow-module

A module where most functions are called from outside.

![shallow-module: before, callers use 8 of 10 functions; after, they use 2 entry points over a tree of hidden functions](docs/shallow-module.svg)


## License

MIT. See [LICENSE](LICENSE).
