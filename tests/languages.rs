//! End-to-end tests of `deslop-please` on Python, TypeScript and JavaScript
//! projects, through scip-python and scip-typescript, and on a monorepo that
//! holds them and a Rust crate. The fixtures follow `end_to_end.rs`'s: a base project in
//! `tests/fixtures/<family>` and variants in `tests/variants/<family>/<variant>`
//! holding only the files they change.

use std::path::{Path, PathBuf};
use std::process::Command;

const PROGRAM: &str = env!("CARGO_BIN_EXE_deslop-please");

/// Whether the indexers these tests run are installed. Without them a test
/// is skipped, so `cargo test` runs in Rust's dev shell alone; CI sets `CI`
/// and has them, so there a missing one fails the test.
fn indexers() -> bool {
    let missing: Vec<&str> = ["scip-python", "scip-typescript", "rust-analyzer"]
        .into_iter()
        .filter(|indexer| {
            let found = Command::new(indexer).arg("--version").output();
            !found.is_ok_and(|output| output.status.success())
        })
        .collect();
    if missing.is_empty() {
        return true;
    }
    let message = format!(
        "these tests run {}, not on PATH (see README.md)",
        missing.join(" and ")
    );
    assert!(std::env::var_os("CI").is_none(), "{message}");
    eprintln!("skipped: {message}");
    false
}

/// A copy of `fixture`, with `variant`'s files over it, to run `test` in.
fn project(fixture: &str, variant: Option<&str>, test: &str) -> PathBuf {
    let copy = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("languages")
        .join(test);
    if copy.exists() {
        std::fs::remove_dir_all(&copy).unwrap();
    }
    let tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    copy_directory(&tests.join("fixtures").join(fixture), &copy);
    if let Some(variant) = variant {
        copy_directory(&tests.join("variants").join(fixture).join(variant), &copy);
    }
    copy
}

fn copy_directory(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let target = to.join(path.file_name().unwrap());
        if path.is_dir() {
            copy_directory(&path, &target);
        } else {
            std::fs::copy(&path, &target).unwrap();
        }
    }
}

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

struct Run {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

fn run(project: &Path, arguments: &[&str]) -> Run {
    run_with_path(project, arguments, None)
}

/// A run, with `PATH` in place of the test's own when one is given. The
/// indexes are cached in the test's own directory, not the user's.
fn run_with_path(project: &Path, arguments: &[&str], path: Option<&str>) -> Run {
    let cache = Path::new(env!("CARGO_TARGET_TMPDIR")).join("languages-cache");
    let mut command = Command::new(PROGRAM);
    command
        .args(arguments)
        .current_dir(project)
        .env("XDG_CACHE_HOME", cache)
        // CI sets CARGO_TERM_COLOR=always; the tests read plain text.
        .env("CARGO_TERM_COLOR", "never");
    if let Some(path) = path {
        command.env("PATH", path);
    }
    let output = command.output().unwrap();
    let run = Run {
        code: output.status.code(),
        stdout: String::from_utf8(output.stdout).unwrap(),
        stderr: String::from_utf8(output.stderr).unwrap(),
    };
    eprintln!(
        "$ deslop-please {}\n{}{}",
        arguments.join(" "),
        run.stdout,
        run.stderr
    );
    run
}

/// `lint key` for every diagnostic, as the allow file lists them, sorted.
fn keys(project: &Path, flags: &[&str]) -> Vec<String> {
    let allow = project.with_extension("keys");
    let mut arguments = vec!["--update-allow", allow.to_str().unwrap()];
    arguments.extend(flags);
    let update = run(project, &arguments);
    assert_eq!(update.code, Some(0), "{}", update.stderr);
    let mut keys: Vec<String> = std::fs::read_to_string(&allow)
        .unwrap()
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect();
    keys.sort();
    keys
}

fn variant_keys(family: &str, variant: Option<&str>) -> Vec<String> {
    let test = format!("{family}-{}", variant.unwrap_or("base"));
    keys(&project(family, variant, &test), &[])
}

const NONE: [&str; 0] = [];

/// A project of `files`, written afresh for `test`.
fn written(test: &str, files: &[(&str, &str)]) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("languages")
        .join(test);
    if root.exists() {
        std::fs::remove_dir_all(&root).unwrap();
    }
    for (path, text) in files {
        write(&root, path, text);
    }
    root
}

#[test]
fn what_rust_reads_as_variants_constants_and_plain_data_is_read_so_in_python() {
    if !indexers() {
        return;
    }
    let ring = "\
class RunQueue:
    MOST = 8

    def __init__(self) -> None:
        self.items: list[int] = []
        self.head = 0

    def full(self) -> bool:
        return len(self.items) - self.head >= RunQueue.MOST
";
    let colors = "\
from enum import Enum, auto


class Color(Enum):
    RED = auto()
    BLUE = auto()

    def cold(self) -> bool:
        return self is Color.BLUE
";
    let base = [
        ("pyproject.toml", "[project]\nname = \"app\"\n"),
        ("app/__init__.py", ""),
        ("app/ring.py", ring),
        ("app/colors.py", colors),
        ("app/report/__init__.py", ""),
    ];
    let with = |test: &str, report: &str| {
        let mut files = base.to_vec();
        files.push(("app/report/sums.py", report));
        keys(&written(test, &files), &[])
    };
    let imports = "from app.colors import Color\nfrom app.ring import RunQueue\n\n";
    // Rust's `RunQueue::MOST`, `Color::Red` and `for x in &q.items`: no leak.
    let reads = format!(
        "{imports}def total(q: RunQueue) -> int:\n    t = sum(q.items)\n    for x in q.items:\n        \
         t += x\n    return t + RunQueue.MOST\n\n\ndef red() -> Color:\n    return Color.RED\n"
    );
    assert_eq!(with("py-plain-reads", &reads), NONE);
    let leak = vec!["field-leak app::ring::RunQueue::items".to_owned()];
    // Rust's `q.items.len()`, `q.items.push(x)` and `parts_mut`: known outside.
    for (test, body) in [
        ("py-plain-len", "    return len(q.items)\n"),
        ("py-plain-append", "    q.items.append(1)\n    return 0\n"),
        (
            "py-plain-alias",
            "    items = q.items\n    items.append(1)\n    return 0\n",
        ),
    ] {
        let report = format!("{imports}def f(q: RunQueue) -> int:\n{body}");
        assert_eq!(with(test, &report), leak, "{test}");
    }
}

#[test]
fn a_callback_slot_is_a_field_and_a_destructured_field_is_taken_whole_in_typescript() {
    if !indexers() {
        return;
    }
    let base = [
        ("package.json", "{\"name\": \"app\"}\n"),
        (
            "tsconfig.json",
            "{\"compilerOptions\": {\"strict\": true}, \"include\": [\"src\"]}\n",
        ),
    ];
    // Nine callbacks that whoever renders the form passes in: data, no functions.
    let form = "\
import { helper } from \"./parts\";
export interface FormProps {
  onA: () => void; onB: () => void; onC: () => void; onD: () => void;
  onE: () => void; onF: () => void; onG: () => void; onH: () => void; onI: () => void;
}
export function render(p: FormProps): void { p.onA(); helper(); }
";
    let page = "\
import { render, FormProps } from \"../form\";
const n = (): void => {};
export function home(): void {
  const p: FormProps = { onA: n, onB: n, onC: n, onD: n, onE: n, onF: n, onG: n, onH: n, onI: n };
  render(p);
}
";
    let mut files = base.to_vec();
    files.extend([
        ("src/form/index.ts", form),
        (
            "src/form/parts.ts",
            "export function helper(): number { return 1; }\n",
        ),
        ("src/pages/home.ts", page),
    ]);
    assert_eq!(keys(&written("ts-callback-slots", &files), &[]), NONE);

    let queue = "\
export class Queue {
  items = new Map<string, number>();
  limit = 3;
  push(k: string, x: number): void { if (this.items.size < this.limit) this.items.set(k, x); }
}
";
    let admit = "\
import { Queue } from \"../queue\";
export function admit(queue: Queue, x: number): void {
  const { items } = queue;
  items.set(\"a\", x);
}
";
    let mut files = base.to_vec();
    files.extend([("src/queue.ts", queue), ("src/sched/admit.ts", admit)]);
    assert_eq!(
        keys(&written("ts-destructured", &files), &[]),
        vec!["field-leak app::queue::Queue::items".to_owned()]
    );
}

#[test]
fn python_module_cycles_and_tangles_are_found_until_the_code_moves() {
    if !indexers() {
        return;
    }
    let net = run(&project("py_module_cycle", None, "py_module_cycle"), &[]);
    assert_eq!(net.code, Some(1));
    assert!(
        net.stdout
            .contains("error[module-cycle]: 2 modules in `netlib::net` use each other in a circle")
    );
    assert!(net.stdout.contains(
        "    ┌─▶ kinds\n\
         \x20   │     ┃ kinds.py:11  Stream::open uses BACKLOG_MOST\n\
         \x20   │     ▼\n\
         \x20   │   socket\n\
         \x20   │     │ socket.py:6  listen uses Stream::{self, open}\n\
         \x20   └─────┘\n"
    ));
    assert_eq!(variant_keys("py_module_cycle", Some("moved")), NONE);

    // An import behind `TYPE_CHECKING` ties the modules as any other does.
    assert_eq!(
        variant_keys("py_tangle", None),
        [
            "module-cycle boot::kernel::tasks,boot::kernel::timers",
            "tangle boot::kernel::tasks::Tasks,boot::kernel::timers::Timers",
        ]
    );
    assert_eq!(variant_keys("py_tangle", Some("untied")), NONE);
}

#[test]
fn a_python_field_leaks_however_it_is_reached_and_not_once_the_class_does_the_work() {
    if !indexers() {
        return;
    }
    let leak = run(&project("py_field_leak", None, "py_field_leak"), &[]);
    assert_eq!(leak.code, Some(1));
    assert!(leak.stdout.contains(
        "error[field-leak]: `RunQueue::items` is known outside `RunQueue`: 5 places in 2 modules"
    ));
    // `report` only reads `head`, an `int`.
    assert!(leak.stdout.contains(
        "    runqueue::queue::ring::RunQueue\n\
         \x20     head ◀━━━ 1  runqueue::sched::policy\n\
         \x20     also known outside: items\n\
         \x20     kept: limit\n"
    ));
    assert_eq!(
        variant_keys("py_field_leak", Some("getters")),
        [
            "field-leak runqueue::queue::ring::RunQueue::_head",
            "field-leak runqueue::queue::ring::RunQueue::_items",
        ]
    );
    let getters = run(
        &project("py_field_leak", Some("getters"), "py_field_leak-g"),
        &[],
    );
    assert!(getters.stdout.contains("through `RunQueue::items`"));
    assert!(getters.stdout.contains("through `RunQueue::set_head`"));
    // The work moved into the class; and reading scalars is no leak, even of
    // a field set from an `int` parameter.
    for variant in ["real", "read"] {
        assert_eq!(
            variant_keys("py_field_leak", Some(variant)),
            NONE,
            "{variant}"
        );
    }
}

#[test]
fn a_shallow_python_package_is_found_and_a_clean_project_passes() {
    if !indexers() {
        return;
    }
    let store = run(&project("py_shallow", None, "py_shallow"), &[]);
    assert!(store.stdout.contains(
        "error[shallow-module]: `inventory::store` is shallow: it shows 9 of its 14 functions"
    ));
    assert_eq!(variant_keys("py_shallow", Some("deep")), NONE);
    // Methods that only hand the call on to the table are seen through: the
    // package still shows nine functions, the table's.
    let forwarded = run(
        &project("py_shallow", Some("forwarded"), "py_shallow-fw"),
        &[],
    );
    assert!(forwarded.stdout.contains("it shows 9 of its 9 functions"));
    assert!(
        forwarded
            .stdout
            .contains("= note: `Table` shows 9 functions")
    );

    // Dataclasses read from outside, a frozen record, a repository that
    // keeps its dict, and tests that poke at fields: nothing to report.
    let clean = run(&project("py_clean", None, "py_clean"), &[]);
    assert_eq!(clean.code, Some(0));
    assert!(clean.stdout.contains("deslop-please: ok"));
}

#[test]
fn typescript_reports_what_rust_reports_for_the_same_code() {
    if !indexers() {
        return;
    }
    let net = run(&project("ts_module_cycle", None, "ts_module_cycle"), &[]);
    assert!(
        net.stdout
            .contains("error[module-cycle]: 2 modules in `net` use each other in a circle")
    );
    assert_eq!(variant_keys("ts_module_cycle", Some("moved")), NONE);
    // `import type` ties the modules too.
    assert_eq!(
        variant_keys("ts_tangle", None),
        [
            "module-cycle boot::kernel::tasks,boot::kernel::timers",
            "tangle boot::kernel::tasks::Tasks,boot::kernel::timers::Timers",
        ]
    );

    let leak = run(&project("ts_field_leak", None, "ts_field_leak"), &[]);
    assert!(leak.stdout.contains(
        "error[field-leak]: `RunQueue::items` is known outside `RunQueue`: 4 places in 2 modules"
    ));
    // `limit` is a parameter property, `constructor(private readonly limit)`.
    assert!(leak.stdout.contains("      kept: limit\n"));
    // Through `get items()` and `set head()`, as through methods.
    assert_eq!(
        variant_keys("ts_field_leak", Some("getters")),
        [
            "field-leak runqueue::queue::ring::RunQueue::_head",
            "field-leak runqueue::queue::ring::RunQueue::_items",
        ]
    );
    assert_eq!(variant_keys("ts_field_leak", Some("real")), NONE);
    // A getter whose signature names a global that is a type and a value
    // too, `drained(): Promise<void>`, is a getter: only its body counts.
    assert_eq!(
        variant_keys("ts_field_leak", Some("promise_getter")),
        ["field-leak runqueue::queue::ring::RunQueue::settled"]
    );

    let store = run(&project("ts_shallow", None, "ts_shallow"), &[]);
    assert!(store.stdout.contains(
        "error[shallow-module]: `inventory::store` is shallow: it shows 10 of its 15 functions"
    ));
    assert_eq!(variant_keys("ts_shallow", Some("deep")), NONE);

    // A React component that reads a class's scalar, a test that pokes at a
    // private field, an HTTP client whose methods are arrow functions held
    // in properties (`get = <T>(path) => ..`), members of object types
    // written in place, and Turkish text with `·` and `₺` in it.
    let clean = run(&project("ts_clean", None, "ts_clean"), &[]);
    assert_eq!(clean.code, Some(0), "{}", clean.stdout);
}

#[test]
fn javascript_is_indexed_without_a_tsconfig_and_nothing_is_written_into_it() {
    if !indexers() {
        return;
    }
    let leaky = project("js_field_leak", None, "js_field_leak");
    assert_eq!(
        keys(&leaky, &[]),
        [
            "field-leak runqueue-js::queue::ring::RunQueue::head",
            "field-leak runqueue-js::queue::ring::RunQueue::items",
        ]
    );
    assert!(!leaky.join("tsconfig.json").exists());
    assert!(!leaky.join("index.scip").exists());
    // `#private` fields are no one else's.
    assert_eq!(variant_keys("js_field_leak", Some("real")), NONE);
    // A known limit: scip-typescript gives fields that only a constructor's
    // `this.items = []` declares no symbol of their class's, so their leaks
    // are not seen.
    assert_eq!(variant_keys("js_field_leak", Some("constructor")), NONE);

    // CommonJS: `require` ties modules as `import` does, and a project whose
    // tools' configuration is JavaScript too is checked once.
    assert_eq!(
        variant_keys("js_module_cycle", None),
        ["module-cycle net-js::lib::kinds,net-js::lib::socket"]
    );
}

#[test]
fn a_monorepo_s_backend_and_frontend_are_each_checked_and_what_no_one_wrote_is_left_out() {
    if !indexers() {
        return;
    }
    let monorepo = project("monorepo", None, "monorepo");
    // What installs, builds and virtual environments hold, each with slop of
    // its own to be reported if it were read.
    let cycle = "export class A { b = B; }\nexport class B { a = A; }\n";
    write(&monorepo, "frontend/node_modules/left-pad/index.ts", cycle);
    write(&monorepo, "frontend/.next/types/routes.ts", cycle);
    write(&monorepo, "frontend/dist/bundle.js", cycle);
    write(&monorepo, "backend/.venv/lib/site.py", "import os\n");
    write(&monorepo, "backend/venv/pyvenv.cfg", "");
    write(
        &monorepo,
        "backend/venv/lib/pkg/a.py",
        "from pkg.b import B\nclass A: pass\n",
    );
    write(
        &monorepo,
        "backend/venv/lib/pkg/b.py",
        "from pkg.a import A\nclass B: pass\n",
    );
    write(
        &monorepo,
        "backend/app/schema_pb2.py",
        "# Generated by the protocol buffer compiler.  DO NOT EDIT!\nX = 1\n",
    );

    let all = run(&monorepo, &[]);
    assert_eq!(all.code, Some(1));
    assert!(all.stderr.contains("checking backend (python)"));
    assert!(all.stderr.contains("checking engine (rust)"));
    assert!(all.stderr.contains("checking frontend (typescript)"));
    assert!(all.stdout.contains("  --> backend/app/db.py:6:1"));
    assert!(all.stdout.contains("  --> engine/src/kinds.rs:10:1"));
    assert!(all.stdout.contains("  --> frontend/src/app/page.tsx:7:1"));
    assert!(!all.stdout.contains("node_modules") && !all.stdout.contains("venv"));
    let python = "module-cycle api::app::db,api::app::models";
    let rust = "module-cycle engine::kinds,engine::socket";
    let typescript = "field-leak web::lib::cart::Cart::lines";
    assert_eq!(keys(&monorepo, &[]), [typescript, python, rust]);
    assert_eq!(keys(&monorepo, &["--language", "rust"]), [rust]);
    assert_eq!(keys(&monorepo, &["--language", "python"]), [python]);
    assert_eq!(keys(&monorepo, &["--language", "typescript"]), [typescript]);
    let javascript = run(&monorepo, &["--language", "javascript"]);
    assert_eq!(javascript.code, Some(2));
    assert!(
        javascript
            .stderr
            .contains("no javascript source files below it")
    );

    // The allow file holds both languages' debt; a scope names one package.
    let allow = monorepo.with_extension("allow");
    run(&monorepo, &["--update-allow", allow.to_str().unwrap()]);
    let allowed = run(&monorepo, &["--allow", allow.to_str().unwrap()]);
    assert_eq!(allowed.code, Some(0));
    let scoped = run(&monorepo, &["web", "--json"]);
    assert!(scoped.stdout.contains("\"field-leak\"") && !scoped.stdout.contains("module-cycle"));

    // A second run reads both indexes from the cache.
    let again = run(&monorepo, &[]);
    assert!(!again.stderr.contains("indexing"));
    assert_eq!(again.stdout, all.stdout);
}

#[test]
fn a_cargo_workspace_is_checked_as_before_when_the_other_indexers_are_missing() {
    if !indexers() {
        return;
    }
    // A Rust repository with an editor extension and release scripts beside it.
    let workspace = project("module_cycle", None, "rust-with-others-no-indexer");
    write(
        &workspace,
        "editors/code/package.json",
        "{\"name\": \"extension\"}\n",
    );
    write(
        &workspace,
        "editors/code/src/extension.ts",
        "export function activate(): number { return 1; }\n",
    );
    write(&workspace, "scripts/requirements.txt", "requests\n");
    write(&workspace, "scripts/release.py", "print(1)\n");
    let path = std::env::var("PATH").unwrap();
    let without_scip = std::env::split_paths(&path)
        .filter(|dir| !dir.join("scip-python").exists() && !dir.join("scip-typescript").exists());
    let without_scip = std::env::join_paths(without_scip).unwrap();

    let rust = run(&workspace, &["--language", "rust"]);
    let found = run_with_path(&workspace, &[], without_scip.to_str());
    assert_eq!(found.code, rust.code);
    assert_eq!(found.stdout, rust.stdout);
    assert!(
        found
            .stderr
            .contains("warning: not checked: scip-typescript is not installed")
    );
    assert!(
        found
            .stderr
            .contains("warning: not checked: scip-python is not installed")
    );

    // Asked for by name, a missing indexer is still an error.
    let asked = run_with_path(&workspace, &["--language", "python"], without_scip.to_str());
    assert_eq!(asked.code, Some(2));
}

#[test]
fn a_missing_indexer_or_a_project_with_nothing_to_read_is_explained() {
    if !indexers() {
        return;
    }
    let clean = project("py_clean", None, "py_clean-no-indexer");
    let missing = run_with_path(&clean, &[], Some("/usr/bin:/bin"));
    assert_eq!(missing.code, Some(2));
    assert!(missing.stderr.contains("scip-python is not installed"));
    assert!(
        missing
            .stderr
            .contains("npm install -g @sourcegraph/scip-python")
    );

    let empty = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("languages")
        .join("manifest-only");
    let _ = std::fs::remove_dir_all(&empty);
    write(&empty, "requirements.txt", "fastapi\n");
    write(&empty, "package.json", "{\"name\": \"x\"}\n");
    let nothing = run(&empty, &[]);
    assert_eq!(nothing.code, Some(2));
    assert!(nothing.stderr.contains("not inside a Cargo workspace"));

    // Rust's flags are Rust's.
    let flags = run(&clean, &["--language", "python", "--all-features"]);
    assert_eq!(flags.code, Some(2));
    assert!(flags.stderr.contains("applies to Rust only"));

    // A file that does not parse is indexed as far as it goes.
    let broken = project("py_clean", None, "py_clean-broken");
    write(&broken, "service/broken.py", "def half(:\n    return (\n");
    write(&broken, "package.json", "{ not json");
    let result = run(&broken, &["--language", "python"]);
    assert_eq!(result.code, Some(0), "{}", result.stderr);
}

#[test]
fn a_language_can_be_chosen_where_nothing_declares_a_project() {
    if !indexers() {
        return;
    }
    let loose = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("languages")
        .join("loose");
    let _ = std::fs::remove_dir_all(&loose);
    write(
        &loose,
        "tools/a.py",
        "from tools.b import B\n\nclass A:\n    def b(self) -> B:\n        return B()\n",
    );
    write(
        &loose,
        "tools/b.py",
        "from tools.a import A\n\nclass B:\n    def a(self) -> A:\n        return A()\n",
    );
    write(&loose, "tools/__init__.py", "");
    assert_eq!(run(&loose, &[]).code, Some(2));
    assert_eq!(
        keys(&loose, &["--language", "python"]),
        [
            "module-cycle loose::tools::a,loose::tools::b",
            "tangle loose::tools::a::A,loose::tools::b::B",
        ]
    );
}

/// Each restructuring of `end_to_end.rs`'s Rust fixtures, written in Python
/// and in TypeScript: the same keys, by the language's names. `tuple_getter`
/// differs by the languages' own rules: a number handed out in a tuple is a
/// copy that cannot be written back, as Rust's `&mut usize` is.
#[test]
fn every_rust_restructuring_reports_the_same_in_python_and_typescript() {
    if !indexers() {
        return;
    }
    let queue = |prefix: &str, fields: &[&str]| -> Vec<String> {
        let mut keys: Vec<String> = fields
            .iter()
            .map(|field| format!("field-leak runqueue::queue::ring::RunQueue::{prefix}{field}"))
            .collect();
        keys.sort();
        keys
    };
    let leaky: [(&str, Vec<String>, Vec<String>); 9] = [
        (
            "base",
            queue("", &["head", "items"]),
            queue("", &["head", "items"]),
        ),
        (
            "getters",
            queue("_", &["head", "items"]),
            queue("_", &["head", "items"]),
        ),
        (
            "let_getters",
            queue("_", &["head", "items"]),
            queue("_", &["head", "items"]),
        ),
        (
            "pasted",
            queue("", &["head", "items"]),
            queue("", &["head", "items"]),
        ),
        (
            "tuple_getter",
            queue("_", &["items"]),
            queue("", &["items"]),
        ),
        ("real", vec![], vec![]),
        ("passed_on", vec![], vec![]),
        ("read", vec![], vec![]),
        ("lent_read", vec![], vec![]),
    ];
    for (variant, python, typescript) in leaky {
        let variant = (variant != "base").then_some(variant);
        assert_eq!(
            variant_keys("py_field_leak", variant),
            python,
            "py {variant:?}"
        );
        assert_eq!(
            variant_keys("ts_field_leak", variant),
            typescript,
            "ts {variant:?}"
        );
    }

    for (family, root) in [
        ("py_module_cycle", "netlib::net"),
        ("ts_module_cycle", "net"),
    ] {
        let cycle = |members: &[&str]| {
            let members: Vec<String> = members.iter().map(|m| format!("{root}::{m}")).collect();
            vec![format!("module-cycle {}", members.join(","))]
        };
        assert_eq!(
            variant_keys(family, None),
            cycle(&["kinds", "socket"]),
            "{family}"
        );
        // Through a re-export, still the two; through a relay, three.
        assert_eq!(
            variant_keys(family, Some("reexported")),
            cycle(&["kinds", "socket"])
        );
        assert_eq!(
            variant_keys(family, Some("relayed")),
            cycle(&["kinds", "limits", "socket"])
        );
        for variant in ["moved", "for_tests"] {
            assert_eq!(
                variant_keys(family, Some(variant)),
                NONE,
                "{family} {variant}"
            );
        }
    }

    let cycle = "module-cycle boot::kernel::tasks,boot::kernel::timers";
    let tasks_and_timers = "tangle boot::kernel::tasks::Tasks,boot::kernel::timers::Timers";
    let ring = "tangle boot::ring::A,boot::ring::B,boot::ring::C,boot::ring::D,\
                boot::ring::E,boot::ring::F,boot::ring::G,boot::ring::H";
    for family in ["py_tangle", "ts_tangle"] {
        assert_eq!(
            variant_keys(family, None),
            [cycle, tasks_and_timers],
            "{family}"
        );
        assert_eq!(
            variant_keys(family, Some("lifted")),
            ["tangle boot::kernel::Tasks,boot::kernel::timers::Timers"],
            "{family}"
        );
        assert_eq!(
            variant_keys(family, Some("ring")),
            [cycle, tasks_and_timers, ring]
        );
        for variant in ["merged", "untied"] {
            assert_eq!(
                variant_keys(family, Some(variant)),
                NONE,
                "{family} {variant}"
            );
        }
    }

    for family in ["py_shallow", "ts_shallow"] {
        for variant in [None, Some("padded"), Some("free_functions")] {
            let keys = variant_keys(family, variant);
            assert_eq!(
                keys,
                ["shallow-module inventory::store"],
                "{family} {variant:?}"
            );
        }
        assert_eq!(variant_keys(family, Some("deep")), NONE, "{family}");
    }
    for family in ["py_small_types", "ts_small_types"] {
        assert_eq!(variant_keys(family, None), NONE, "{family}");
    }
}

#[test]
fn a_long_file_and_a_wide_directory_are_reported_in_every_language() {
    if !indexers() {
        return;
    }
    for (family, file, directory, extension) in [
        ("py_clean", "service/api.py", "service/many", "py"),
        ("ts_clean", "src/lib/format.ts", "src/many", "ts"),
    ] {
        let root = project(family, None, &format!("{family}-layout"));
        let original = std::fs::read_to_string(root.join(file)).unwrap();
        let padded = original.clone() + &"\n".repeat(2_000 - original.lines().count());
        std::fs::write(root.join(file), &padded).unwrap();
        // 2,000 lines, and 15 files: at the most, not past it.
        let part = if extension == "py" {
            "X = 1\n"
        } else {
            "export {};\n"
        };
        for n in 1..=15 {
            write(&root, &format!("{directory}/part{n}.{extension}"), part);
        }
        assert_eq!(keys(&root, &[]), NONE, "{family}");

        write(
            &root,
            &format!("{directory}/part16.{extension}"),
            "export {};\n",
        );
        std::fs::write(root.join(file), padded + "\n").unwrap();
        assert_eq!(
            keys(&root, &[]),
            [
                format!("long-file {file}"),
                format!("wide-directory {directory}/")
            ],
            "{family}"
        );
    }
}
