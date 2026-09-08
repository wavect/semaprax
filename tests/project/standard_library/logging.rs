use semaprax::project::{self, ProjectExecutionOptions, ProjectExecutionOutcome};
use std::path::{Path, PathBuf};

#[test]
fn log_writer_executes_on_all_three_backends() {
    super::run_examples_and_conformance(
        super::packages()
            .into_iter()
            .filter(|p| p.module == "std.log")
            .collect(),
    );
}

#[test]
fn utf8_ascii_scan_preserves_package_conformance() {
    super::run_examples_and_conformance(
        super::packages()
            .into_iter()
            .filter(|p| p.module == "std.data.json.utf8")
            .collect(),
    );
}

#[test]
fn log_writer_preflight_rejects_invalid_events() {
    let scratch = super::temporary("log-contracts");
    // Invalid levels, output state, and each invalid UTF-8 family fail before writing.
    for (index, (name, message, level, capacity, position)) in [
        ("97u8", "98u8", 6, 64, 0),
        ("97u8", "98u8", 2, 54, 0),
        ("97u8", "98u8", 2, 64, 65),
        ("255u8", "98u8", 2, 64, 0),
        ("97u8", "128u8", 2, 64, 0),
        ("192u8, 128u8", "98u8", 2, 64, 0),
        ("97u8", "237u8, 160u8, 128u8", 2, 64, 0),
        ("244u8, 144u8, 128u8, 128u8", "98u8", 2, 64, 0),
        ("97u8", "226u8, 130u8", 2, 64, 0),
    ]
    .into_iter()
    .enumerate()
    {
        let directory = scratch.join(index.to_string());
        std::fs::create_dir_all(directory.join("src")).unwrap();
        std::fs::write(
            directory.join("semaprax.toml"),
            r#"schema = "semaprax.manifest.v1"

[package]
name = "log-consumer"
version = "0.1.0"
profile = "useful-data.v2"

[modules]
entry = "consumer.app"
sources = ["src/app.spx", "src/tests.spx"]
tests = ["consumer.tests"]

[exports]
web = []

[dependencies]
std.log = "=0.1.0"
"#,
        )
        .unwrap();
        std::fs::write(
            directory.join("src/app.spx"),
            "module consumer.app;\n\n@id(\"consumer.main\")\nfn main() -> i64\n{\n    0\n}\n",
        )
        .unwrap();
        let source = format!(
            r#"module consumer.tests;
use type @id("std.log.event") from std.log as Event;
use type @id("std.io.writer") from std.io as Writer;
use function @id("std.log.append-event") from std.log as append_event;
@id("consumer.invalid") fn invalid() -> usize {{
    let name = [{name}]; let message = [{message}];
    let event = Event {{ level: {level}u8, sequence: 7usize, name: bytes_copy(array_as_slice(name)), message: bytes_copy(array_as_slice(message)) }};
    let output = Writer {{ data: bytes_zeroed({capacity}usize), position: {position}usize }};
    let written = append_event(event, output);
    written.position
}}
@id("consumer.tests.main") fn main() -> i64 {{ if invalid() == 0usize {{ 0 }} else {{ 1 }} }}
"#
        );
        let parsed = semaprax::parse(&source, "log-contract.spx").unwrap();
        std::fs::write(
            directory.join("src/tests.spx"),
            semaprax::format::canonical(&parsed),
        )
        .unwrap();
        project::with_authenticated_project(&directory.join("semaprax.toml"), |snapshot| {
            for _ in 0..2 {
                let report = snapshot.execute_test(&ProjectExecutionOptions::default())?;
                let ProjectExecutionOutcome::LanguageFailure(status) = report.outcome() else {
                    panic!("case {index}: {report:?}");
                };
                assert_eq!(status.class(), semaprax::conformance::StatusClass::Contract);
                assert_eq!(
                    status.domain_id(),
                    semaprax::conformance::CONTRACT_STATUS_DOMAIN_V1
                );
                assert_eq!(
                    status.code(),
                    semaprax::conformance::CONTRACT_REQUIRES_FALSE_CODE
                );
            }
            Ok(())
        })
        .unwrap();
    }
    std::fs::remove_dir_all(scratch).unwrap();
}

pub(super) fn conformance_manifests(scratch: &Path, manifest: &Path, module: &str) -> Vec<PathBuf> {
    match module {
        "std.format" => super::formatting::conformance_manifests(scratch, manifest),
        "std.log" => log_conformance_manifests(scratch, manifest),
        "std.path.value" => super::typed_paths::conformance_manifests(scratch, manifest),
        "std.data.json.dec" | "std.data.json.write" => {
            super::json_cursors::conformance_manifests(scratch, manifest, module)
        }
        _ => vec![manifest.to_path_buf()],
    }
}

pub(super) fn uses_byte_writes(manifest: &Path) -> bool {
    !matches!(case_name(manifest), "test_helpers" | "test_event_len")
}

pub(super) fn live_byte_bound(manifest: &Path) -> usize {
    match case_name(manifest) {
        "test_helpers" => 0,
        "test_event_len" | "test_quote_utf8" => 2,
        name if name.starts_with("test_level_") || name == "test_fixed" => 1,
        _ => 3,
    }
}

fn case_name(manifest: &Path) -> &str {
    manifest
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .unwrap()
}

/// Keep each expanded case intact but admit only its local call closure in the
/// scratch workspace. The shipped representative package is also run unchanged.
fn log_conformance_manifests(scratch: &Path, manifest: &Path) -> Vec<PathBuf> {
    use semaprax::ast::{ModuleUseKind, Type};
    use std::collections::BTreeSet;
    const SOURCE: &str = include_str!("log_cases.spx");
    const CASES: &[&str] = &[
        "test_helpers",
        "test_level_trace",
        "test_level_debug",
        "test_level_info",
        "test_level_warn",
        "test_level_error",
        "test_level_fatal",
        "test_fixed",
        "test_quote_utf8",
        "test_event_len",
        "test_line",
        "test_prefix_suffix",
        "test_unicode_exact_capacity",
        "test_unicode_wide_exact_capacity",
        "test_long_message",
    ];
    let parsed = semaprax::parse(SOURCE, "log-cases.spx").unwrap();
    assert_eq!(semaprax::format::canonical(&parsed), SOURCE);
    let actual: Vec<_> = parsed
        .functions
        .iter()
        .filter(|f| f.name.starts_with("test_"))
        .map(|f| f.name.as_str())
        .collect();
    assert_eq!(actual, CASES, "logger case inventory changed");
    let package = manifest.parent().unwrap();
    let mut manifests = vec![manifest.to_path_buf()];
    let mut covered = BTreeSet::new();
    for case in CASES {
        let mut selected = parsed.clone();
        let main = selected
            .functions
            .iter_mut()
            .find(|f| f.name == "main")
            .unwrap();
        main.body = semaprax::parse(
            &format!("module fixture; @id(\"fixture.main\") fn main() -> i64 {{ {case}() }}"),
            "log-entry.spx",
        )
        .unwrap()
        .functions[0]
            .body
            .clone();
        let mut calls = BTreeSet::from(["main".to_owned()]);
        loop {
            let previous = calls.len();
            for function in &selected.functions {
                if calls.contains(&function.name) {
                    assert!(function.type_parameters.is_empty());
                    for expression in function
                        .requires
                        .iter()
                        .chain(&function.ensures)
                        .chain(std::iter::once(&function.body))
                    {
                        expression.visit_calls(&mut |name, _| {
                            calls.insert(name.to_owned());
                        });
                    }
                }
            }
            if previous == calls.len() {
                break;
            }
        }
        selected.functions.retain(|f| calls.contains(&f.name));
        let entry = selected.functions.iter().find(|f| f.name == *case).unwrap();
        assert!(entry.params.is_empty());
        assert_eq!(entry.return_type, Type::I64);
        selected
            .module_uses
            .retain(|u| u.kind != ModuleUseKind::Function || calls.contains(&u.alias));
        for import in &selected.module_uses {
            if import.kind == ModuleUseKind::Function && import.target_module == "std.log" {
                covered.insert(import.persistent_id.clone());
            }
        }
        let directory = scratch.join(case);
        std::fs::create_dir_all(directory.join("src")).unwrap();
        std::fs::copy(manifest, directory.join("semaprax.toml")).unwrap();
        for file in std::fs::read_dir(package.join("src")).unwrap() {
            let file = file.unwrap();
            std::fs::copy(file.path(), directory.join("src").join(file.file_name())).unwrap();
        }
        std::fs::write(
            directory.join("src/tests.spx"),
            semaprax::format::canonical(&selected),
        )
        .unwrap();
        manifests.push(directory.join("semaprax.toml"));
    }
    let library = std::fs::read_to_string(package.join("src/log.spx")).unwrap();
    let library = semaprax::parse(&library, "log.spx").unwrap();
    let public: BTreeSet<_> = library
        .functions
        .iter()
        .map(|f| f.stable_id.clone())
        .collect();
    assert_eq!(
        covered, public,
        "every logger function needs direct executed coverage"
    );
    manifests
}
