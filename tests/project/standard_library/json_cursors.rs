//! Independently invoked JSON cursor cases preserve the legacy one-owner
//! decoder gate while requiring exactly two live owners for borrowed input
//! and a caller-provided output buffer.
use std::path::{Path, PathBuf};

pub(super) fn is_cursor_case(manifest: &Path) -> bool {
    manifest
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|name| name.to_string_lossy().starts_with("json-cursor-"))
}

pub(super) fn conformance_manifests(scratch: &Path, manifest: &Path, module: &str) -> Vec<PathBuf> {
    let package = manifest.parent().unwrap();
    let source = std::fs::read_to_string(
        super::root()
            .join("tests/project/standard_library/fixtures")
            .join(format!(
                "{}-cursors.spx",
                package.file_name().unwrap().to_string_lossy()
            )),
    )
    .unwrap();
    let parsed = semaprax::parse(&source, "json-cursor-cases.spx").unwrap();
    let prefix = format!("{module}.tests.cursor-");
    let mut manifests = vec![manifest.to_owned()];
    for case in parsed
        .functions
        .iter()
        .filter(|function| function.stable_id.starts_with(&prefix))
    {
        assert!(case.params.is_empty());
        assert_eq!(case.return_type, semaprax::ast::Type::Bool);
        let directory = scratch.join(format!("json-cursor-{}-{}", module, case.name));
        std::fs::create_dir_all(directory.join("src")).unwrap();
        std::fs::copy(manifest, directory.join("semaprax.toml")).unwrap();
        for file in std::fs::read_dir(package.join("src")).unwrap() {
            let file = file.unwrap();
            std::fs::copy(file.path(), directory.join("src").join(file.file_name())).unwrap();
        }
        let mut selected = parsed.clone();
        let main = selected
            .functions
            .iter_mut()
            .find(|f| f.stable_id == format!("{module}.tests.main"))
            .unwrap();
        let body = semaprax::parse(&format!("module fixture; @id(\"fixture.main\") fn main() -> i64 {{ if {}() {{ 0 }} else {{ 1 }} }}", case.name), "cursor-entry.spx").unwrap();
        main.body = body.functions[0].body.clone();
        std::fs::write(
            directory.join("src/tests.spx"),
            semaprax::format::canonical(&selected),
        )
        .unwrap();
        manifests.push(directory.join("semaprax.toml"));
    }
    assert!(manifests.len() >= 4, "JSON cursor conformance disappeared");
    manifests
}

#[test]
fn json_cursors_decode_execute_on_all_three_backends() {
    run_cursor_package("std.data.json.dec");
}

#[test]
fn json_cursors_write_execute_on_all_three_backends() {
    run_cursor_package("std.data.json.write");
}

fn run_cursor_package(module: &str) {
    super::run_examples_and_conformance(
        super::packages()
            .into_iter()
            .filter(|p| p.module == module)
            .collect(),
    );
}

#[test]
fn json_cursor_contracts_reject_malformed_input_and_short_or_forged_output() {
    use semaprax::project::{self, ProjectExecutionOptions, ProjectExecutionOutcome};
    let scratch = super::temporary("json-cursor-contracts");
    for (index, directory, operation, body) in [
        (0, "data-json-dec", "decode-into", "let raw = [34u8, 97u8, 34u8]; let input = Reader { data: bytes_copy(array_as_slice(raw)), position: 0usize }; let output = Writer { data: bytes_zeroed(0usize), position: 0usize }; let written = operation(input, output); written.position"),
        (1, "data-json-dec", "decode-into", "let raw = [34u8, 92u8, 113u8, 34u8]; let input = Reader { data: bytes_copy(array_as_slice(raw)), position: 0usize }; let output = Writer { data: bytes_zeroed(8usize), position: 0usize }; let written = operation(input, output); written.position"),
        (2, "data-json-dec", "decode-into", "let input = Reader { data: bytes_zeroed(1usize), position: 2usize }; let output = Writer { data: bytes_zeroed(8usize), position: 0usize }; let written = operation(input, output); written.position"),
        (3, "data-json-write", "quoted-into", "let input = Reader { data: bytes_zeroed(0usize), position: 0usize }; let output = Writer { data: bytes_zeroed(1usize), position: 0usize }; let written = operation(input, output); written.position"),
        (4, "data-json-write", "count-into", "let output = Writer { data: bytes_zeroed(2usize), position: 0usize }; let written = operation(100usize, output); written.position"),
        (5, "data-json-write", "count-into", "let output = Writer { data: bytes_zeroed(1usize), position: 2usize }; let written = operation(0usize, output); written.position"),
    ] {
        let package = super::root().join("std").join(directory);
        let dest = scratch.join(index.to_string());
        std::fs::create_dir_all(dest.join("src")).unwrap();
        for file in std::fs::read_dir(package.join("src")).unwrap() {
            let file = file.unwrap();
            std::fs::copy(file.path(), dest.join("src").join(file.file_name())).unwrap();
        }
        let module = if directory.ends_with("dec") { "std.data.json.dec" } else { "std.data.json.write" };
        let manifest = std::fs::read_to_string(package.join("semaprax.toml")).unwrap();
        // This fixture checks only the private invocation's contract behavior;
        // the unmodified package/export is covered by conformance separately.
        let manifest = manifest.lines().map(|line| if line.starts_with("web =") { "web = []" } else { line }).collect::<Vec<_>>().join("\n") + "\n";
        std::fs::write(dest.join("semaprax.toml"), manifest).unwrap();
        let source = format!("module {module}.tests;\nuse type @id(\"std.io.reader\") from std.io as Reader;\nuse type @id(\"std.io.writer\") from std.io as Writer;\nuse function @id(\"{module}.{operation}\") from {module} as operation;\n@id(\"fixture.invalid\") fn invalid() -> usize {{ {body} }}\n@id(\"fixture.main\") fn main() -> i64 {{ if invalid() == 0usize {{ 0 }} else {{ 1 }} }}\n");
        let parsed = semaprax::parse(&source, "json-cursor-contract.spx").unwrap();
        std::fs::write(dest.join("src/tests.spx"), semaprax::format::canonical(&parsed)).unwrap();
        project::with_authenticated_project(&dest.join("semaprax.toml"), |snapshot| {
            for _ in 0..2 {
                let result = snapshot.execute_test(&ProjectExecutionOptions::default())?;
                let ProjectExecutionOutcome::LanguageFailure(status) = result.outcome() else { panic!("case {index}: {result:?}"); };
                assert_eq!(status.class(), semaprax::conformance::StatusClass::Contract);
                assert_eq!(status.domain_id(), semaprax::conformance::CONTRACT_STATUS_DOMAIN_V1);
                assert_eq!(status.code(), semaprax::conformance::CONTRACT_REQUIRES_FALSE_CODE);
            }
            Ok(())
        }).unwrap();
    }
    std::fs::remove_dir_all(scratch).unwrap();
}

use super::{compile_and_run_c, temporary};
#[path = "json_roundtrip.rs"]
mod json_roundtrip;
