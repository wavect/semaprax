//! Focused source, graph, and cross-backend regressions for the bundled
//! `std.io.lines` package: line meaning over the existing `std.io` cursors.
//! No host I/O authority is involved in these transitions.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{format, graph, hir, parse, verify};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const CURSORS: &str = include_str!("../../../std/io/src/io.spx");
const LINES: &str = include_str!("../../../std/io-lines/src/lines.spx");

/// One checked module holds both libraries: the cursor shapes the line
/// operations compose, and the line operations themselves. The imports the
/// package route resolves across the dependency become local declarations with
/// the same names, so the fixture exercises the identical bodies.
fn source(main: &str) -> String {
    let cursors = CURSORS.replacen("module std.io;", "module app;", 1);
    let lines: String = LINES
        .lines()
        .filter(|line| !line.starts_with("module ") && !line.starts_with("use "))
        .collect::<Vec<_>>()
        .join("\n");
    format!("{cursors}\n{lines}\n{main}\n")
}

fn canonical_checked(main: &str) -> String {
    let program = parse(&source(main), "std-io-lines.spx").expect("std.io.lines fixture parses");
    let diagnostics = verify::verify(&program);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.severity.is_error()),
        "std.io.lines fixture source diagnostics: {diagnostics:?}"
    );
    let canonical = format::canonical(&program);
    let reparsed = parse(&canonical, "std-io-lines.spx").expect("canonical fixture reparses");
    assert_eq!(format::canonical(&reparsed), canonical);
    hir::resolve(&reparsed).expect("checked std.io.lines fixture resolves");
    canonical
}

fn source_file(source: &str) -> PathBuf {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-std-io-lines-{}-{id}.spx",
        std::process::id()
    ));
    std::fs::write(&path, source).expect("writes temporary checked source");
    path
}

fn interpretation(main: &str) -> interpreter::Interpretation {
    let canonical = canonical_checked(main);
    let path = source_file(&canonical);
    let result = interpreter::interpret(&path, "app.main", &[], &InterpreterOptions::default())
        .expect("std.io.lines interpreter entry is admitted");
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
    assert!(!result.returned, "invalid transition returned");
    let document: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
    let outcome = &document["payload"]["outcome"];
    assert_eq!(outcome["kind"], "failed", "{document}");
    assert_eq!(outcome["status"]["class"], "contract", "{document}");
}

#[test]
fn io_lines_execute_on_all_three_backends() {
    super::run_examples_and_conformance(
        super::packages()
            .into_iter()
            .filter(|p| p.module == "std.io.lines")
            .collect(),
    );
}

#[test]
fn io_line_contracts_fail_before_insufficient_capacity_or_forged_cursor() {
    for main in [
        // The copy preflights the whole line against the live writer capacity.
        r#"
@id("app.main")
fn main() -> i64
{
    let source = [97u8, 98u8, 10u8];
    let input = bytes_copy(array_as_slice(source));
    let reader = reader_from_bytes(input);
    let output = writer_from_bytes(bytes_zeroed(1usize));
    let written = reader_line_into(reader, output);
    if writer_position(written) == 2usize { 0 } else { 1 }
}
"#,
        // A writer whose cursor leaves less capacity than the line needs.
        r#"
@id("app.main")
fn main() -> i64
{
    let source = [97u8, 98u8, 10u8];
    let input = bytes_copy(array_as_slice(source));
    let reader = reader_from_bytes(input);
    let output = Writer { data: bytes_zeroed(2usize), position: 1usize };
    let written = reader_line_into(reader, output);
    if writer_position(written) == 3usize { 0 } else { 1 }
}
"#,
        // Forged cursors are rejected by every line observer and transition.
        r#"
@id("app.main")
fn main() -> i64
{
    let reader = Reader { data: bytes_zeroed(1usize), position: 2usize };
    if reader_line_len(reader) == 0usize { 0 } else { 1 }
}
"#,
        r#"
@id("app.main")
fn main() -> i64
{
    let reader = Reader { data: bytes_zeroed(1usize), position: 2usize };
    if reader_line_complete(reader) { 0 } else { 1 }
}
"#,
        r#"
@id("app.main")
fn main() -> i64
{
    let reader = Reader { data: bytes_zeroed(1usize), position: 2usize };
    let advanced = reader_next_line(reader);
    if reader_position(advanced) == 1usize { 0 } else { 1 }
}
"#,
        r#"
@id("app.main")
fn main() -> i64
{
    let reader = Reader { data: bytes_zeroed(1usize), position: 2usize };
    let output = writer_from_bytes(bytes_zeroed(1usize));
    let written = reader_line_into(reader, output);
    if writer_position(written) == 0usize { 0 } else { 1 }
}
"#,
        // A view offset past the end is rejected before any scan.
        r#"
@id("app.main")
fn main() -> i64
{
    let data = bytes_zeroed(1usize);
    let view = bytes_as_slice(data);
    if line_end(view, 2usize) == 1usize { 0 } else { 1 }
}
"#,
        r#"
@id("app.main")
fn main() -> i64
{
    let data = bytes_zeroed(1usize);
    let view = bytes_as_slice(data);
    if line_content_len(view, 2usize) == 0usize { 0 } else { 1 }
}
"#,
        r#"
@id("app.main")
fn main() -> i64
{
    let data = bytes_zeroed(1usize);
    let view = bytes_as_slice(data);
    if line_terminated(view, 2usize) { 0 } else { 1 }
}
"#,
    ] {
        fails(main);
    }
}

#[test]
fn io_line_copy_preserves_the_borrowed_reader_and_its_bytes() {
    returns(
        r#"
@id("app.main")
fn main() -> i64
{
    let source = [97u8, 13u8, 10u8, 98u8];
    let input = bytes_copy(array_as_slice(source));
    let reader = reader_from_bytes(input);
    let output = writer_from_bytes(bytes_zeroed(1usize));
    let written = reader_line_into(reader, output);
    let unchanged = reader_position(reader) == 0usize && reader_remaining(reader) == 4usize;
    let advanced = reader_next_line(reader);
    let stepped = reader_position(advanced) == 3usize && reader_line_len(advanced) == 1usize;
    let retained = reader_finish(advanced);
    let copied = writer_position(written) == 1usize;
    let bytes = writer_finish(written);
    let head = match byte_get(bytes_as_slice(bytes), 0usize) { Option::Some { value } => value, Option::None {} => 0u8, };
    if unchanged && stepped && copied && head == 97u8 && byte_len(bytes_as_slice(retained)) == 4usize { 0 } else { 1 }
}
"#,
        "0",
    );
}

#[test]
fn io_line_graph_retains_ownership_cleanup_schema_and_rejects_forged_or_drifted_projections() {
    let main = r#"
@id("app.main")
fn main() -> i64
{
    let source = [97u8, 13u8, 10u8, 98u8];
    let input = bytes_copy(array_as_slice(source));
    let reader = reader_from_bytes(input);
    let output = writer_from_bytes(bytes_zeroed(1usize));
    let written = reader_line_into(reader, output);
    let stepped = reader_next_line(reader);
    let retained = reader_finish(stepped);
    let bytes = writer_finish(written);
    if byte_len(bytes_as_slice(bytes)) == 1usize && byte_len(bytes_as_slice(retained)) == 4usize { 0 } else { 1 }
}
"#;
    let canonical = canonical_checked(main);
    let program = parse(&canonical, "std-io-lines-graph.spx").unwrap();
    let json = graph::to_json(&program).expect("line graph serializes");
    graph::verify_json(&program, &json).expect("derived line graph independently replays");
    for id in [
        "std.io.lines.line-end",
        "std.io.lines.line-terminated",
        "std.io.lines.line-content-len",
        "std.io.lines.reader.line-len",
        "std.io.lines.reader.line-complete",
        "std.io.lines.reader.line-into",
        "std.io.lines.reader.next-line",
    ] {
        assert!(json.contains(id), "missing {id}");
    }
    let document: serde_json::Value = serde_json::from_str(&json).unwrap();
    let node = |id: &str| {
        document["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["id"] == id)
            .unwrap_or_else(|| panic!("missing node {id}"))
            .clone()
    };
    // The copy borrows its input, transfers exactly the caller's Writer, and
    // carries that owner as its cleanup root under the existing v5 schema.
    let copy = node("std.io.lines.reader.line-into");
    assert_eq!(copy["params"][0]["ownership_mode"], "borrow");
    assert_eq!(copy["params"][1]["ownership_mode"], "own");
    assert_eq!(copy["result"]["ownership_mode"], "own");
    assert_eq!(copy["cleanup"]["schema"], "semaprax.cleanup-plan.v5");
    assert_eq!(
        copy["cleanup"]["entry_state"]["live_owned_parameters"]
            .as_array()
            .unwrap()
            .len(),
        1,
        "the borrowed Reader must not enter the cleanup inventory"
    );
    assert_eq!(
        copy["cleanup"]["slots"][0]["field_liveness_shape"]["fields"][0]["shape"]["lifecycle"],
        "core.bytes.drop"
    );
    // The line transition consumes and returns the one owner.
    let step = node("std.io.lines.reader.next-line");
    assert_eq!(step["params"][0]["ownership_mode"], "own");
    assert_eq!(step["result"]["ownership_mode"], "own");
    assert_eq!(step["cleanup"]["schema"], "semaprax.cleanup-plan.v5");
    // Observers borrow and return Copy scalars, so they own no cleanup.
    // A record observer selects the same contract-carrying v5 schema with an
    // empty owned inventory; a pure view helper selects v2. The selection is a
    // semantic fact of the shape, not of a backend layout.
    for id in [
        "std.io.lines.reader.line-len",
        "std.io.lines.reader.line-complete",
    ] {
        let observer = node(id);
        assert_eq!(observer["params"][0]["ownership_mode"], "borrow", "{id}");
        assert_eq!(observer["result"]["ownership_mode"], "value", "{id}");
        assert_eq!(
            observer["cleanup"]["schema"], "semaprax.cleanup-plan.v5",
            "{id}"
        );
        assert!(
            observer["cleanup"]["entry_state"]["live_owned_parameters"]
                .as_array()
                .unwrap()
                .is_empty(),
            "{id} borrowed its Reader into the cleanup inventory"
        );
        assert!(
            observer["cleanup"]["slots"].as_array().unwrap().is_empty(),
            "{id} acquired a cleanup slot"
        );
    }
    for id in [
        "std.io.lines.line-end",
        "std.io.lines.line-terminated",
        "std.io.lines.line-content-len",
    ] {
        let helper = node(id);
        assert_eq!(helper["params"][0]["ownership_mode"], "borrow", "{id}");
        assert_eq!(helper["params"][1]["ownership_mode"], "value", "{id}");
        assert_eq!(helper["result"]["ownership_mode"], "value", "{id}");
        assert_eq!(
            helper["cleanup"]["schema"], "semaprax.cleanup-plan.v2",
            "{id}"
        );
        assert!(
            helper["cleanup"]["slots"].as_array().unwrap().is_empty(),
            "{id} acquired a cleanup slot"
        );
    }
    // A reminted field identity and drifted source both fail replay.
    let forged = json.replace("std.io.writer.data", "std.io.writer.forged-data");
    assert_ne!(forged, json);
    assert!(graph::verify_json(&program, &forged).is_err());
    let drifted = parse(
        &canonical.replace("value == 10u8", "value == 13u8"),
        "drifted-lines.spx",
    )
    .unwrap();
    assert_ne!(format::canonical(&drifted), canonical);
    assert!(graph::verify_json(&drifted, &json).is_err());
}

/// Keep every line case intact but admit only its local call closure in the
/// scratch workspace.  The shipped representative package also runs unchanged.
pub(super) fn conformance_manifests(scratch: &Path, manifest: &Path) -> Vec<PathBuf> {
    use semaprax::ast::{ModuleUseKind, Type};
    use std::collections::BTreeSet;
    const SOURCE: &str = include_str!("io_lines_cases.spx");
    const CASES: &[&str] = &[
        "test_crlf_line",
        "test_lf_line",
        "test_empty_line",
        "test_crlf_only",
        "test_bare_cr",
        "test_unterminated_tail",
        "test_two_line_walk",
        "test_offset_write",
    ];
    let parsed = parse(SOURCE, "io-lines-cases.spx").unwrap();
    assert_eq!(format::canonical(&parsed), SOURCE);
    let actual: Vec<_> = parsed
        .functions
        .iter()
        .filter(|f| f.name.starts_with("test_"))
        .map(|f| f.name.as_str())
        .collect();
    assert_eq!(actual, CASES, "line case inventory changed");
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
        main.body = parse(
            &format!("module fixture; @id(\"fixture.main\") fn main() -> i64 {{ {case}() }}"),
            "io-lines-entry.spx",
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
            if import.kind == ModuleUseKind::Function && import.target_module == "std.io.lines" {
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
            format::canonical(&selected),
        )
        .unwrap();
        manifests.push(directory.join("semaprax.toml"));
    }
    let library = std::fs::read_to_string(package.join("src/lines.spx")).unwrap();
    let library = parse(&library, "lines.spx").unwrap();
    let public: BTreeSet<_> = library
        .functions
        .iter()
        .map(|f| f.stable_id.clone())
        .collect();
    assert_eq!(
        covered, public,
        "every line function needs direct executed coverage"
    );
    manifests
}

/// Each case balances its declared live `Bytes` bound: a line copy holds the
/// borrowed input beside the caller's output buffer.
pub(super) fn live_byte_bound(manifest: &Path) -> usize {
    let _ = manifest;
    // Every shipped and generated case holds the borrowed input beside the
    // caller's output buffer, and nothing else.
    2
}
