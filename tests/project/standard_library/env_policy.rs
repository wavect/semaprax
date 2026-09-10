//! Named cross-backend selector and focused regressions for the bundled
//! `std.env.policy` package. The package is pure policy: it reads no
//! environment, declares no effect, and allocates nothing.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{format, hir, parse, verify};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const LIBRARY: &str = include_str!("../../../std/env-policy/src/policy.spx");

fn source(main: &str) -> String {
    format!(
        "{}\n{main}\n",
        LIBRARY.replacen("module std.env.policy;", "module app;", 1)
    )
}

fn canonical_checked(main: &str) -> String {
    let program = parse(&source(main), "std-env-policy.spx").expect("policy fixture parses");
    let diagnostics = verify::verify(&program);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.severity.is_error()),
        "policy fixture source diagnostics: {diagnostics:?}"
    );
    let canonical = format::canonical(&program);
    let reparsed = parse(&canonical, "std-env-policy.spx").expect("canonical fixture reparses");
    assert_eq!(format::canonical(&reparsed), canonical);
    hir::resolve(&reparsed).expect("checked policy fixture resolves");
    canonical
}

fn returns(main: &str, expected: &str) {
    let canonical = canonical_checked(main);
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let path: PathBuf = std::env::temp_dir().join(format!(
        "semaprax-std-env-policy-{}-{id}.spx",
        std::process::id()
    ));
    std::fs::write(&path, &canonical).expect("writes temporary checked source");
    let result = interpreter::interpret(&path, "app.main", &[], &InterpreterOptions::default())
        .expect("policy interpreter entry is admitted");
    interpreter::verify_envelope(&result.envelope).expect("interpreter envelope is canonical");
    std::fs::remove_file(path).expect("removes temporary checked source");
    assert!(result.returned, "expected returned {expected}");
    let document: serde_json::Value =
        serde_json::from_str(&result.envelope).expect("envelope JSON");
    assert_eq!(
        document["payload"]["outcome"]["value"].as_str(),
        Some(expected)
    );
}

#[test]
fn env_policy_executes_on_all_three_backends() {
    super::run_examples_and_conformance(
        super::packages()
            .into_iter()
            .filter(|p| p.module == "std.env.policy")
            .collect(),
    );
}

#[test]
fn env_policy_claims_no_capability_and_allocates_nothing() {
    // The package exists precisely so these predicates need not declare the
    // environment-read capability the `std.env` gate requires of every
    // function there.
    let library = std::fs::read_to_string(super::root().join("std/env-policy/src/policy.spx"))
        .expect("policy source is readable");
    for forbidden in [
        "uses {",
        "permit {",
        "bytes_zeroed",
        "bytes_copy",
        "bytes_set",
    ] {
        assert!(
            !library.contains(forbidden),
            "std.env.policy must not contain `{forbidden}`"
        );
    }
    let manifest = std::fs::read_to_string(super::root().join("std/env-policy/semaprax.toml"))
        .expect("policy manifest is readable");
    assert!(manifest.contains("web = []"), "policy exports nothing");
    assert!(
        !manifest.contains("[dependencies]"),
        "policy depends on nothing"
    );
}

#[test]
fn env_policy_splits_at_the_first_separator_and_rejects_nul() {
    returns(
        r#"
@id("app.main")
fn main() -> i64
{
    let chained = [65u8, 61u8, 66u8, 61u8, 67u8];
    let chained_view = array_as_slice(chained);
    let first = assignment_separator(chained_view) == 1usize && assignment_value_start(chained_view) == 2usize;
    let admitted = assignment_is_valid(chained_view);
    let empty_value = [88u8, 61u8];
    let empty_view = array_as_slice(empty_value);
    let trailing = assignment_is_valid(empty_view) && assignment_value_start(empty_view) == 2usize;
    let empty_name = [61u8, 86u8];
    let refused = !assignment_is_valid(array_as_slice(empty_name));
    let nul_name = [65u8, 0u8, 66u8];
    let nul_value = [88u8, 61u8, 0u8];
    let nuls = !name_is_valid(array_as_slice(nul_name)) && !assignment_is_valid(array_as_slice(nul_value));
    let leading_digit = [49u8, 66u8];
    let digit = !name_is_valid(array_as_slice(leading_digit));
    let bare = [80u8, 65u8, 84u8, 72u8];
    let plain = name_is_valid(array_as_slice(bare)) && assignment_separator(array_as_slice(bare)) == 4usize;
    if first && admitted && trailing && refused && nuls && digit && plain { 0 } else { 1 }
}
"#,
        "0",
    );
}
