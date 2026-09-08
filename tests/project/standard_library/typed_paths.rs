//! Focused source and interpreter regressions for the private `std.path.value`
//! package. Paths keep caller-owned Bytes and a checked logical prefix; they do
//! not acquire filesystem authority.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{format, graph, hir, parse, verify};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const LIBRARY: &str = include_str!("../../../std/path-value/src/path.spx");

fn source(main: &str) -> String {
    format!(
        "{}\n{main}\n",
        LIBRARY.replacen("module std.path.value;", "module app;", 1)
    )
}

fn canonical_checked(main: &str) -> String {
    let program =
        parse(&source(main), "std-path-value.spx").expect("std.path.value fixture parses");
    let diagnostics = verify::verify(&program);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.severity.is_error()),
        "std.path.value fixture source diagnostics: {diagnostics:?}"
    );
    let canonical = format::canonical(&program);
    let reparsed =
        parse(&canonical, "std-path-value.spx").expect("canonical std.path.value reparses");
    assert_eq!(format::canonical(&reparsed), canonical);
    hir::resolve(&reparsed).expect("checked std.path.value fixture resolves");
    canonical
}

fn source_file(source: &str) -> PathBuf {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-std-path-value-{}-{id}.spx",
        std::process::id()
    ));
    std::fs::write(&path, source).expect("writes temporary checked source");
    path
}

fn interpretation(main: &str) -> interpreter::Interpretation {
    let canonical = canonical_checked(main);
    let path = source_file(&canonical);
    let result = interpreter::interpret(&path, "app.main", &[], &InterpreterOptions::default())
        .expect("std.path.value interpreter entry is admitted");
    interpreter::verify_envelope(&result.envelope).expect("interpreter envelope is canonical");
    std::fs::remove_file(path).expect("removes temporary checked source");
    result
}

fn returns(main: &str, expected: &str) {
    let result = interpretation(main);
    assert!(
        result.returned,
        "expected returned {expected}: {:?}",
        result.envelope
    );
    let document: serde_json::Value =
        serde_json::from_str(&result.envelope).expect("envelope JSON");
    assert_eq!(
        document["payload"]["outcome"]["value"].as_str(),
        Some(expected)
    );
}

fn fails(main: &str) {
    let result = interpretation(main);
    assert!(!result.returned, "invalid Path transition returned");
    let document: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
    let outcome = &document["payload"]["outcome"];
    assert_eq!(outcome["kind"], "failed", "{document}");
    assert_eq!(outcome["status"]["class"], "contract", "{document}");
}

#[test]
fn typed_paths_join_parent_and_logical_prefix_observers_execute() {
    returns(
        r#"
@id("app.main")
fn main() -> i64
{
    let base_source = [47u8, 117u8, 115u8, 114u8];
    let child_source = [98u8, 105u8, 110u8];
    let base = path_from_bytes(bytes_copy(array_as_slice(base_source)));
    let child = path_from_bytes(bytes_copy(array_as_slice(child_source)));
    let joined = path_join(base, child, bytes_zeroed(12usize));
    let shape = path_valid(joined) && path_is_absolute(joined) && path_length(joined) == 8usize && path_capacity(joined) == 12usize && path_segment_count(joined) == 2usize && path_file_name_start(joined) == 5usize && path_parent_end(joined) == 4usize && path_extension_start(joined) == 8usize;
    let bytes = path_finish(joined);
    let suffix_is_retained = byte_len(bytes_as_slice(bytes)) == 12usize;
    let parent_source = [47u8, 117u8, 115u8, 114u8, 47u8, 98u8, 105u8, 110u8];
    let parent = path_parent(path_from_bytes(bytes_copy(array_as_slice(parent_source))));
    let parent_shape = path_length(parent) == 4usize && path_byte_at(parent, 0usize) == 47u8 && path_byte_at(parent, 3usize) == 114u8;
    let root_source = [47u8];
    let root_parent = path_parent(path_from_bytes(bytes_copy(array_as_slice(root_source))));
    if shape && suffix_is_retained && parent_shape && path_length(root_parent) == 1usize && path_byte_at(root_parent, 0usize) == 47u8 { 0 } else { 1 }
}
"#,
        "0",
    );
}

#[test]
fn typed_path_contracts_reject_forged_prefixes_and_short_join_buffer() {
    for main in [
        r#"
@id("app.main")
fn main() -> i64 {
    let path = path_from_bytes(bytes_zeroed(1usize));
    if path_length(path) == 1usize { 0 } else { 1 }
}
"#,
        r#"
@id("app.main")
fn main() -> i64 {
    let source = [97u8];
    let path = path_from_bytes(bytes_copy(array_as_slice(source)));
    if path_byte_at(path, 1usize) == 0u8 { 0 } else { 1 }
}
"#,
        r#"
@id("app.main")
fn main() -> i64
{
    let source = [97u8];
    let forged = Path { data: bytes_copy(array_as_slice(source)), length: 2usize };
    if path_length(forged) == 0usize { 0 } else { 1 }
}
"#,
        r#"
@id("app.main")
fn main() -> i64
{
    let nul = Path { data: bytes_zeroed(1usize), length: 1usize };
    if path_length(nul) == 0usize { 0 } else { 1 }
}
"#,
        r#"
@id("app.main")
fn main() -> i64
{
    let base_source = [117u8, 115u8, 114u8];
    let child_source = [98u8, 105u8, 110u8];
    let base = path_from_bytes(bytes_copy(array_as_slice(base_source)));
    let child = path_from_bytes(bytes_copy(array_as_slice(child_source)));
    let joined = path_join(base, child, bytes_zeroed(6usize));
    if path_length(joined) == 0usize { 0 } else { 1 }
}
"#,
    ] {
        fails(main);
    }
}

#[test]
fn typed_paths_canonical_hir_and_graph_retain_nominal_contract_and_cleanup_facts() {
    let main = r#"
@id("app.main")
fn main() -> i64
{
    let source = [115u8, 114u8, 99u8, 47u8, 109u8, 97u8, 105u8, 110u8, 46u8, 115u8, 112u8, 120u8];
    let path = path_from_bytes(bytes_copy(array_as_slice(source)));
    let parent = path_parent(path);
    let bytes = path_finish(parent);
    if byte_len(bytes_as_slice(bytes)) == 12usize { 0 } else { 1 }
}
"#;
    let canonical = canonical_checked(main);
    let program = parse(&canonical, "std-path-value-graph.spx").unwrap();
    let json = graph::to_json(&program).expect("typed Path graph serializes");
    graph::verify_json(&program, &json).expect("derived Path graph independently replays");
    let forged = json.replace(
        "std.path.value.path.data",
        "std.path.value.path.forged-data",
    );
    assert_ne!(forged, json);
    assert!(graph::verify_json(&program, &forged).is_err());
    let drifted = parse(
        &canonical.replace("length: parent", "length: 0usize"),
        "drifted-path.spx",
    )
    .unwrap();
    assert_ne!(format::canonical(&drifted), canonical);
    assert!(graph::verify_json(&drifted, &json).is_err());
    for id in [
        "std.path.value.path",
        "std.path.value.path.data",
        "std.path.value.path.length",
        "std.path.value.valid",
        "std.path.value.parent",
        "std.path.value.finish",
        "std.path.value.join",
    ] {
        assert!(json.contains(id), "missing {id}");
    }
    let document: serde_json::Value = serde_json::from_str(&json).unwrap();
    let constructor = document["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == "std.path.value.from-bytes")
        .unwrap();
    assert_eq!(constructor["params"][0]["ownership_mode"], "own");
    assert_eq!(constructor["result"]["ownership_mode"], "own");
    assert_eq!(
        constructor["requires_graph"][0]["callee"],
        "std.path.value.prefix-valid"
    );
    assert!(
        json.contains("\"kind\":\"match\",\"ownership_mode\":\"own\""),
        "Path consuming cleanup transitions disappeared"
    );
}

#[test]
fn borrowed_path_cannot_be_consumed_to_escape_its_bytes() {
    let source = source(
        r#"
@id("app.leak")
fn leak(path: borrow Path) -> Bytes
{
    match own path { Path { data, length: _ } => data, }
}

@id("app.main")
fn main() -> i64
{
    let source = [115u8, 97u8, 102u8, 101u8];
    let path = path_from_bytes(bytes_copy(array_as_slice(source)));
    let bytes = leak(path);
    if byte_len(bytes_as_slice(bytes)) == 4usize { 0 } else { 1 }
}
"#,
    );
    let program =
        parse(&source, "std-path-value-borrow-escape.spx").expect("hostile source parses");
    let diagnostics = verify::verify(&program);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-O117"),
        "borrowed Path consumption must retain its exact ownership rejection: {diagnostics:?}"
    );
}

#[test]
fn typed_paths_library_has_no_public_descriptor_or_export_authority() {
    let manifest = super::root().join("std/path-value/semaprax.toml");
    semaprax::project::with_authenticated_project(&manifest, |snapshot| {
        snapshot.check()?;
        assert!(snapshot.manifest().web_exports().is_empty());
        let diagnostics = snapshot
            .public_api_descriptor()
            .expect_err("library acquired a public descriptor");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "SPX-J105"),
            "{diagnostics:?}"
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn typed_paths_bundled_dependency_executes_without_vendored_library_or_exports() {
    let scratch = super::temporary("path-value-bundled-dependency");
    std::fs::create_dir(scratch.join("src")).unwrap();
    std::fs::write(
        scratch.join("semaprax.toml"),
        r#"schema = "semaprax.manifest.v1"

[package]
name = "path-value-consumer"
version = "0.1.0"
profile = "owned-data-api.v1"

[modules]
entry = "consumer.app"
sources = ["src/app.spx", "src/tests.spx"]
tests = ["consumer.tests"]

[exports]
web = []

[dependencies]
std.path.value = "^0.1.0"
"#,
    )
    .unwrap();
    for (original, destination, from, to) in [
        (
            "examples.spx",
            "app.spx",
            "std.path.value.examples",
            "consumer.app",
        ),
        (
            "tests.spx",
            "tests.spx",
            "std.path.value.tests",
            "consumer.tests",
        ),
    ] {
        let text = std::fs::read_to_string(super::root().join("std/path-value/src").join(original))
            .unwrap();
        std::fs::write(
            scratch.join("src").join(destination),
            text.replace(from, to),
        )
        .unwrap();
    }
    semaprax::project::with_authenticated_project(&scratch.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        let options = semaprax::project::ProjectExecutionOptions::default();
        assert_eq!(
            snapshot.execute_entry(&options)?.outcome(),
            &semaprax::project::ProjectExecutionOutcome::Returned(0)
        );
        assert_eq!(
            snapshot.execute_test(&options)?.outcome(),
            &semaprax::project::ProjectExecutionOutcome::Returned(0)
        );
        assert!(snapshot.public_api_descriptor().is_err());
        Ok(())
    })
    .unwrap();
    std::fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn typed_path_join_cannot_consume_a_borrowed_input_as_output_storage() {
    let text = source(
        r#"
@id("app.main")
fn main() -> i64 {
    let raw = [97u8];
    let path = path_from_bytes(bytes_copy(array_as_slice(raw)));
    let joined = path_join(path, path, path_finish(path));
    if path_length(joined) == 1usize { 0 } else { 1 }
}
"#,
    );
    let program = parse(&text, "path-join-alias.spx").unwrap();
    let diagnostics = hir::resolve(&program).expect_err("join consumed its still-borrowed input");
    assert!(
        diagnostics.iter().any(|d| d.code == "SPX-H006"),
        "{diagnostics:?}"
    );
}

// Invoke every authored zero-argument conformance case independently. The
// fixed per-invocation allocation budget remains 16; aggregating all cases in
// a single source main would exceed it even though each case settles its own
// owners. Every selected package snapshot still goes through normal checking.
pub(super) fn conformance_manifests(
    scratch: &std::path::Path,
    manifest: &std::path::Path,
) -> Vec<PathBuf> {
    let package = manifest.parent().unwrap();
    let tests = std::fs::read_to_string(package.join("src/tests.spx")).unwrap();
    let parsed = parse(&tests, "typed-path-cases.spx").unwrap();
    let split = tests.rfind("@id(\"std.path.value.tests.main\")").unwrap();
    let mut manifests = vec![manifest.to_owned()];
    for case in parsed
        .functions
        .iter()
        .filter(|f| f.params.is_empty() && f.return_type == semaprax::ast::Type::Bool)
    {
        let directory = scratch.join(format!("path-case-{}", case.name));
        std::fs::create_dir_all(directory.join("src")).unwrap();
        for file in ["semaprax.toml", "src/path.spx", "src/examples.spx"] {
            std::fs::copy(package.join(file), directory.join(file)).unwrap();
        }
        let source = format!("{}@id(\"std.path.value.tests.main\")\nfn main() -> i64 {{ if {}() {{ 0 }} else {{ 1 }} }}\n", &tests[..split], case.name);
        let (parsed, comments) =
            semaprax::parse_with_comments(&source, "typed-path-case.spx").unwrap();
        let canonical = semaprax::format::comments::canonical_with_comments(&parsed, &comments);
        std::fs::write(directory.join("src/tests.spx"), canonical).unwrap();
        manifests.push(directory.join("semaprax.toml"));
    }
    assert!(
        manifests.len() >= 9,
        "typed Path conformance cases disappeared"
    );
    manifests
}
