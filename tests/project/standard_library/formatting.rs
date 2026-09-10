use semaprax::project::{self, ProjectExecutionOptions, ProjectExecutionOutcome};

#[test]
fn format_writer_executes_on_all_three_backends() {
    super::run_examples_and_conformance(
        super::packages()
            .into_iter()
            .filter(|p| p.module == "std.format")
            .collect(),
    );
}

#[test]
fn format_writer_preflight_rejects_short_and_forged_output() {
    let scratch = super::temporary("format-contracts");
    for (index, body) in [
        "let text = \"é\"; let output = Writer { data: bytes_zeroed(1usize), position: 0usize }; let written = append_str(string_as_str(text), output); written.position",
        "let output = Writer { data: bytes_zeroed(19usize), position: 0usize }; let written = append_i64(-9223372036854775807 - 1, output); written.position",
        "let output = Writer { data: bytes_zeroed(2usize), position: 0usize }; let written = append_usize(100usize, output); written.position",
        "let output = Writer { data: bytes_zeroed(4usize), position: 0usize }; let written = append_bool(false, output); written.position",
        "let output = Writer { data: bytes_zeroed(1usize), position: 2usize }; let written = append_bool(true, output); written.position",
        // Padded output preflights the whole field, not just the content.
        "let text = \"abc\"; let output = Writer { data: bytes_zeroed(3usize), position: 0usize }; let written = append_str_left(string_as_str(text), 4usize, 32u8, output); written.position",
        "let output = Writer { data: bytes_zeroed(2usize), position: 0usize }; let written = append_usize_right(7usize, 3usize, 48u8, output); written.position",
        "let output = Writer { data: bytes_zeroed(4usize), position: 2usize }; let written = append_fill(32u8, 3usize, output); written.position",
        "let output = Writer { data: bytes_zeroed(3usize), position: 4usize }; let written = append_fill(32u8, 0usize, output); written.position",
    ].into_iter().enumerate() {
        let directory = scratch.join(index.to_string());
        std::fs::create_dir_all(directory.join("src")).unwrap();
        let manifest = "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"format-consumer\"\nversion = \"0.1.0\"\nprofile = \"useful-data.v2\"\n\n[modules]\nentry = \"consumer.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\ntests = [\"consumer.tests\"]\n\n[exports]\nweb = []\n\n[dependencies]\nstd.format = \"=0.1.0\"\n";
        std::fs::write(directory.join("semaprax.toml"), manifest).unwrap();
        std::fs::write(directory.join("src/app.spx"), "module consumer.app;\n\n@id(\"consumer.main\")\nfn main() -> i64\n{\n    0\n}\n").unwrap();
        let source = format!("module consumer.tests;\nuse type @id(\"std.io.writer\") from std.io as Writer;\nuse function @id(\"std.format.append-str\") from std.format as append_str;\nuse function @id(\"std.format.append-i64\") from std.format as append_i64;\nuse function @id(\"std.format.append-usize\") from std.format as append_usize;\nuse function @id(\"std.format.append-bool\") from std.format as append_bool;\nuse function @id(\"std.format.append-str-left\") from std.format as append_str_left;\nuse function @id(\"std.format.append-usize-right\") from std.format as append_usize_right;\nuse function @id(\"std.format.append-fill\") from std.format as append_fill;\n@id(\"consumer.invalid\") fn invalid() -> usize {{ {body} }}\n@id(\"consumer.tests.main\") fn main() -> i64 {{ if invalid() == 0usize {{ 0 }} else {{ 1 }} }}\n");
        let parsed = semaprax::parse(&source, "format-contract.spx").unwrap();
        std::fs::write(directory.join("src/tests.spx"), semaprax::format::canonical(&parsed)).unwrap();
        project::with_authenticated_project(&directory.join("semaprax.toml"), |snapshot| {
            for _ in 0..2 {
                let report = snapshot.execute_test(&ProjectExecutionOptions::default())?;
                let ProjectExecutionOutcome::LanguageFailure(status) = report.outcome() else { panic!("case {index}: {report:?}"); };
                assert_eq!(status.class(), semaprax::conformance::StatusClass::Contract);
                assert_eq!(status.domain_id(), semaprax::conformance::CONTRACT_STATUS_DOMAIN_V1);
                assert_eq!(status.code(), semaprax::conformance::CONTRACT_REQUIRES_FALSE_CODE);
            }
            Ok(())
        }).unwrap();
    }
    std::fs::remove_dir_all(scratch).unwrap();
}

pub(super) fn conformance_manifests(
    scratch: &std::path::Path,
    manifest: &std::path::Path,
) -> Vec<std::path::PathBuf> {
    let package = manifest.parent().unwrap();
    let source = std::fs::read_to_string(package.join("src/tests.spx")).unwrap();
    let parsed = semaprax::parse(&source, "format-cases.spx").unwrap();
    let mut manifests = Vec::new();
    for case in parsed
        .functions
        .iter()
        .filter(|f| f.name.starts_with("test_"))
    {
        assert!(case.params.is_empty());
        assert_eq!(case.return_type, semaprax::ast::Type::I64);
        let directory = scratch.join(&case.name);
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
            .find(|f| f.name == "main")
            .unwrap();
        let entry = semaprax::parse(
            &format!(
                "module fixture; @id(\"fixture.main\") fn main() -> i64 {{ {}() }}",
                case.name
            ),
            "format-entry.spx",
        )
        .unwrap();
        main.body = entry.functions[0].body.clone();
        std::fs::write(
            directory.join("src/tests.spx"),
            semaprax::format::canonical(&selected),
        )
        .unwrap();
        manifests.push(directory.join("semaprax.toml"));
    }
    assert_eq!(manifests.len(), 13, "format conformance case disappeared");
    manifests
}

pub(super) fn uses_byte_arena(manifest: &std::path::Path) -> bool {
    manifest
        .parent()
        .and_then(std::path::Path::file_name)
        .is_none_or(|name| name != "test_helpers")
}
