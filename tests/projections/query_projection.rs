//! Executable evidence for Declaration Query v1 (`semaprax query`).
//!
//! Proves the filters select by kind, name, identity prefix, effect, and call
//! relation over the documentation model, that the CLI prints the library's
//! bytes, that the JSON result names the graph revision, and that unknown
//! kinds or identities fail closed instead of matching nothing.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use semaprax::query::{self, QueryFilters};
use semaprax::{graph, verify};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn cli(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg("query")
        .args(arguments)
        .current_dir(root())
        .output()
        .unwrap()
}

fn checked(path: &Path) -> (semaprax::ast::Program, semaprax::lexer::Comments) {
    let source = std::fs::read_to_string(path).unwrap();
    let (program, comments) = semaprax::parse_with_comments(&source, path).unwrap();
    assert!(!verify::verify(&program)
        .iter()
        .any(|item| item.severity.is_error()));
    (program, comments)
}

fn ids(result: &query::QueryResult) -> Vec<&str> {
    result
        .matches
        .iter()
        .map(|found| found.entry.id.as_str())
        .collect()
}

#[test]
fn filters_select_declarations_and_call_relations() {
    let path = root().join("examples/effects.spx");
    let (program, comments) = checked(&path);
    let all = query::run(&program, &comments, &QueryFilters::default()).unwrap();
    assert_eq!(ids(&all), ["clock.logical_tick", "app.main"]);
    assert_eq!(all.revision, graph::revision(&program));
    assert_eq!(all.module, "examples.effects");
    let main = all
        .matches
        .iter()
        .find(|found| found.entry.id == "app.main")
        .unwrap();
    assert_eq!(main.calls, ["clock.logical_tick"]);
    assert!(main.called_by.is_empty());
    let tick = &all.matches[0];
    assert!(tick.calls.is_empty());
    assert_eq!(tick.called_by, ["app.main"]);

    let by_effect = query::run(
        &program,
        &comments,
        &QueryFilters {
            effect: Some("clock.read".to_owned()),
            ..QueryFilters::default()
        },
    )
    .unwrap();
    assert_eq!(ids(&by_effect), ["clock.logical_tick", "app.main"]);
    let by_name = query::run(
        &program,
        &comments,
        &QueryFilters {
            kinds: vec!["function".to_owned()],
            name: Some("tick".to_owned()),
            ..QueryFilters::default()
        },
    )
    .unwrap();
    assert_eq!(ids(&by_name), ["clock.logical_tick"]);
    let callers = query::run(
        &program,
        &comments,
        &QueryFilters {
            calls: Some("clock.logical_tick".to_owned()),
            ..QueryFilters::default()
        },
    )
    .unwrap();
    assert_eq!(ids(&callers), ["app.main"]);
    let callees = query::run(
        &program,
        &comments,
        &QueryFilters {
            called_by: Some("app.main".to_owned()),
            ..QueryFilters::default()
        },
    )
    .unwrap();
    assert_eq!(ids(&callees), ["clock.logical_tick"]);
    let none = query::run(
        &program,
        &comments,
        &QueryFilters {
            id_prefix: Some("nothing.".to_owned()),
            ..QueryFilters::default()
        },
    )
    .unwrap();
    assert!(none.matches.is_empty());
    assert_eq!(query::text(&none), "");
}

#[test]
fn methods_records_and_fields_are_queryable_by_kind() {
    let path = root().join("examples/classes.spx");
    let (program, comments) = checked(&path);
    let methods = query::run(
        &program,
        &comments,
        &QueryFilters {
            kinds: vec!["method".to_owned()],
            ..QueryFilters::default()
        },
    )
    .unwrap();
    assert_eq!(
        ids(&methods),
        ["example.counter.get", "example.counter.bumped"]
    );
    for found in &methods.matches {
        assert!(found
            .entry
            .facts
            .iter()
            .any(|fact| fact.label == "Owner" && fact.values == ["example.counter"]));
    }
    let classes = query::run(
        &program,
        &comments,
        &QueryFilters {
            kinds: vec!["class".to_owned(), "record".to_owned()],
            ..QueryFilters::default()
        },
    )
    .unwrap();
    assert_eq!(ids(&classes), ["example.counter"]);
}

#[test]
fn cli_prints_the_library_text_and_json() {
    let path = root().join("examples/effects.spx");
    let (program, comments) = checked(&path);
    let result = query::run(
        &program,
        &comments,
        &QueryFilters {
            effect: Some("clock.read".to_owned()),
            ..QueryFilters::default()
        },
    )
    .unwrap();
    let text = cli(&["examples/effects.spx", "--effect", "clock.read"]);
    assert!(text.status.success());
    assert!(text.stderr.is_empty());
    assert_eq!(
        String::from_utf8(text.stdout).unwrap(),
        query::text(&result)
    );
    assert_eq!(
        query::text(&result),
        "function\tclock.logical_tick\tfn logical_tick(value: i64) -> i64\nfunction\tapp.main\tfn main() -> i64\n"
    );

    let json = cli(&["--json", "examples/effects.spx", "--effect", "clock.read"]);
    assert!(json.status.success());
    assert!(json.stderr.is_empty());
    let json = String::from_utf8(json.stdout).unwrap();
    assert_eq!(json, query::json(&result));
    assert_eq!(json.matches('\n').count(), 1);
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["schema"], query::SCHEMA_V1);
    assert_eq!(value["revision"], graph::revision(&program));
    assert_eq!(value["filters"]["effect"], "clock.read");
    assert_eq!(value["filters"]["name"], serde_json::Value::Null);
    assert_eq!(value["matches"][1]["calls"][0], "clock.logical_tick");
    assert_eq!(value["matches"][0]["called_by"][0], "app.main");
}

#[test]
fn unknown_kinds_and_identities_fail_closed() {
    for (arguments, code) in [
        (
            &["examples/effects.spx", "--kind", "module"][..],
            "SPX-V211",
        ),
        (
            &["examples/effects.spx", "--kind", "function,typo"][..],
            "SPX-V211",
        ),
        (
            &["examples/effects.spx", "--calls", "app.missing"][..],
            "SPX-V212",
        ),
        (
            &["examples/effects.spx", "--called-by", "app.missing"][..],
            "SPX-V212",
        ),
    ] {
        let output = cli(arguments);
        assert_eq!(output.status.code(), Some(1), "{arguments:?}");
        assert!(output.stdout.is_empty(), "{arguments:?}");
        assert!(
            String::from_utf8(output.stderr).unwrap().contains(code),
            "{arguments:?}"
        );
    }
    for arguments in [
        &[][..],
        &["examples/effects.spx", "extra"][..],
        &["examples/effects.spx", "--name"][..],
        &["examples/effects.spx", "--unknown", "x"][..],
    ] {
        let output = cli(arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(output.stdout.is_empty(), "{arguments:?}");
    }
    let missing = cli(&["examples/does-not-exist.spx"]);
    assert_eq!(missing.status.code(), Some(1));
    assert!(String::from_utf8(missing.stderr)
        .unwrap()
        .contains("SPX-I001"));
}
