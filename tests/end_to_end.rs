//! End-to-end tests of `deslop-please` on small workspaces through `rust-analyzer scip`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const TOOL: &str = "deslop-please";
const PROGRAM: &str = env!("CARGO_BIN_EXE_deslop-please");

fn workspace(fixture: &str, test: &str) -> PathBuf {
    let rust_analyzer = Command::new("rust-analyzer").arg("--version").output();
    assert!(
        rust_analyzer.is_ok_and(|output| output.status.success()),
        "these tests run rust-analyzer, which is not on PATH (the dev shell has it)"
    );
    let copy = Path::new(env!("CARGO_TARGET_TMPDIR")).join(test);
    if copy.exists() {
        std::fs::remove_dir_all(&copy).unwrap();
    }
    copy_directory(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(fixture),
        &copy,
    );
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

struct Run {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

fn run(workspace: &Path, arguments: &[&str]) -> Run {
    let Output {
        status,
        stdout,
        stderr,
    } = Command::new(PROGRAM)
        .args(arguments)
        .current_dir(workspace)
        .output()
        .unwrap();
    let run = Run {
        code: status.code(),
        stdout: String::from_utf8(stdout).unwrap(),
        stderr: String::from_utf8(stderr).unwrap(),
    };
    eprintln!(
        "$ {TOOL} {}\n{}{}",
        arguments.join(" "),
        run.stdout,
        run.stderr
    );
    run
}

#[test]
fn clean_workspace_exits_zero() {
    let workspace = workspace("clean", "clean");
    let result = run(&workspace, &[]);
    assert_eq!(result.code, Some(0));
    assert!(result.stdout.contains(&format!("{TOOL}: ok")));
}

#[test]
fn facade_bypass_is_reported_and_allowlist_suppresses_only_existing_entries() {
    let workspace = workspace("deep_reach", "bypass");
    let first = run(&workspace, &[]);
    assert_eq!(first.code, Some(1));
    assert!(first.stdout.contains("error[facade-bypass]"));
    assert!(first.stdout.contains("--> src/socket.rs:1"));

    let allow = workspace.join(".deslop-please.allow");
    let allow_str = allow.to_str().unwrap();
    assert_eq!(
        run(&workspace, &["--update-allow", allow_str]).code,
        Some(0)
    );
    let allowed = run(&workspace, &["--allow", allow_str]);
    assert_eq!(allowed.code, Some(0));
    assert!(allowed.stdout.contains(&format!("{TOOL}: ok")));

    // Fixing an allowed bypass does NOT fail CI; it only prints a note.
    let clean_socket =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reexport/src/socket.rs");
    std::fs::copy(clean_socket, workspace.join("src/socket.rs")).unwrap();
    let fixed = run(&workspace, &["--allow", allow_str]);
    assert_eq!(fixed.code, Some(0));
    assert!(fixed.stdout.contains("now fixed"));
}

#[test]
fn module_cycle_reports_cheapest_cut_and_passes_when_broken() {
    let workspace = workspace("problems", "cycle");
    let bad = run(&workspace, &[]);
    assert_eq!(bad.code, Some(1));
    assert!(bad.stdout.contains("error[module-cycle]"));
    assert!(bad.stdout.contains("Socket"));
    assert!(bad.stdout.contains(&format!("{TOOL}: 1 error, well done!")));

    let clean_stream = "pub struct Stream;\n\nimpl Stream {\n    pub fn new<T>(_: T) -> Stream {\n        Stream\n    }\n}\n";
    std::fs::write(workspace.join("src/kinds/stream.rs"), clean_stream).unwrap();
    let good = run(&workspace, &[]);
    assert_eq!(good.code, Some(0));
    assert!(good.stdout.contains(&format!("{TOOL}: ok")));
}

#[test]
fn field_leak_catches_encapsulated_struct_and_allows_plain_data() {
    let workspace = workspace("two_crates", "field_leak");
    let allow = workspace.join(".deslop-please.allow");
    let allow_str = allow.to_str().unwrap();
    assert_eq!(
        run(&workspace, &["--update-allow", allow_str]).code,
        Some(0)
    );

    let lib_rs = workspace.join("crates/lib/src/lib.rs");
    let added = std::fs::read_to_string(&lib_rs).unwrap()
        + "\npub struct Plain { pub x: u32, pub y: u32 }\n\
           pub struct Vault { secret: u32, pub leaked: u32 }\n\
           impl Vault { pub fn new() -> Self { Self { secret: 1, leaked: 2 } } pub fn get(&self) -> u32 { self.secret } }\n";
    std::fs::write(&lib_rs, added).unwrap();

    let main_rs = workspace.join("app/src/main.rs");
    let original_main = std::fs::read_to_string(&main_rs).unwrap();
    std::fs::write(
        &main_rs,
        format!("{original_main}\nfn read_plain(p: lib::Plain) -> u32 {{ p.x }}\n"),
    )
    .unwrap();
    assert_eq!(run(&workspace, &["--allow", allow_str]).code, Some(0));

    std::fs::write(
        &main_rs,
        format!("{original_main}\nfn read_vault(v: lib::Vault) -> u32 {{ v.leaked }}\n"),
    )
    .unwrap();
    let fail = run(&workspace, &["--allow", allow_str]);
    assert_eq!(fail.code, Some(1));
    assert!(fail.stdout.contains("error[field-leak]"));
    assert!(fail.stdout.contains("Vault::leaked"));
}

#[test]
fn a_facade_that_only_forwards_hides_nothing_and_is_no_bypass() {
    let workspace = workspace("forwarders", "forwarders");
    let shallow = run(&workspace, &[]);
    assert_eq!(shallow.code, Some(1));
    assert!(shallow.stdout.contains(
        "error[shallow-module]: `forwarders::store` is shallow: 9/10 functions/methods are called from outside (only 10% hidden)"
    ));
    assert!(shallow.stdout.contains(
        "= note: seen through 9 forwarders: Store::a, Store::b, Store::c, Store::d, Store::e, Store::f, +3 more"
    ));
    assert!(!shallow.stdout.contains("facade-bypass"));

    let store = workspace.join("src/store/mod.rs");
    let working = std::fs::read_to_string(&store)
        .unwrap()
        .replace("()\n", "().max(1)\n");
    std::fs::write(&store, working).unwrap();
    let deep = run(&workspace, &[]);
    assert_eq!(deep.code, Some(0));
    assert!(deep.stdout.contains(&format!("{TOOL}: ok")));
}

#[test]
fn a_call_through_a_forwarder_is_on_the_seam_to_the_module_that_does_the_work() {
    let workspace = workspace("seams", "seams");
    let result = run(&workspace, &[]);
    assert_eq!(result.code, Some(1));
    assert!(result.stdout.contains(
        "error[chatty-seam]: wide seam `seams::app` -> `seams::back`: calls 25 distinct functions/methods (25 references)"
    ));
    assert!(result.stdout.contains(
        "= note: seen through 25 forwarders: Front::f01, Front::f02, Front::f03, Front::f04, Front::f05, Front::f06, +19 more"
    ));
    assert!(!result.stdout.contains("`seams::app` -> `seams::front`"));
    assert!(!result.stdout.contains("`seams::front` -> `seams::back`"));
}

#[test]
fn outside_a_cargo_workspace_reports_error() {
    let empty = std::env::temp_dir().join(format!("{TOOL}-not-a-workspace-{}", std::process::id()));
    std::fs::create_dir_all(&empty).unwrap();
    let run = run(&empty, &[]);
    std::fs::remove_dir_all(&empty).unwrap();
    assert_eq!(run.code, Some(2));
    assert!(run.stderr.contains("not inside a Cargo workspace"));
}
