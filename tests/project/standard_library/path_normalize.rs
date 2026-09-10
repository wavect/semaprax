//! Focused source, graph, and cross-backend regressions for the bundled
//! `std.path.normalize` package: lexical POSIX path normalization over the
//! existing `std.path.value` typed Path. No host filesystem authority is
//! involved in these transitions.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{format, graph, hir, parse, verify};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const VALUE: &str = include_str!("../../../std/path-value/src/path.spx");
const NORMALIZE: &str = include_str!("../../../std/path-normalize/src/normalize.spx");

/// One checked module holds both libraries: the typed Path record and value
/// helpers the normalization composes, and the normalization operations
/// themselves. The imports the package route resolves across the dependency
/// become local declarations with the same names, so the fixture exercises
/// the identical bodies.
fn source(main: &str) -> String {
    let value = VALUE.replacen("module std.path.value;", "module app;", 1);
    let normalize: String = NORMALIZE
        .lines()
        .filter(|line| !line.starts_with("module ") && !line.starts_with("use "))
        .collect::<Vec<_>>()
        .join("\n");
    format!("{value}\n{normalize}\n{main}\n")
}

fn canonical_checked(main: &str) -> String {
    let program =
        parse(&source(main), "std-path-normalize.spx").expect("std.path.normalize fixture parses");
    let diagnostics = verify::verify(&program);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.severity.is_error()),
        "std.path.normalize fixture source diagnostics: {diagnostics:?}"
    );
    let canonical = format::canonical(&program);
    let reparsed = parse(&canonical, "std-path-normalize.spx").expect("canonical fixture reparses");
    assert_eq!(format::canonical(&reparsed), canonical);
    hir::resolve(&reparsed).expect("checked std.path.normalize fixture resolves");
    canonical
}

fn source_file(source: &str) -> PathBuf {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-std-path-normalize-{}-{id}.spx",
        std::process::id()
    ));
    std::fs::write(&path, source).expect("writes temporary checked source");
    path
}

fn interpretation(main: &str) -> interpreter::Interpretation {
    let canonical = canonical_checked(main);
    let path = source_file(&canonical);
    let result = interpreter::interpret(&path, "app.main", &[], &InterpreterOptions::default())
        .expect("std.path.normalize interpreter entry is admitted");
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
fn path_normalization_executes_on_all_three_backends() {
    super::run_examples_and_conformance(
        super::packages()
            .into_iter()
            .filter(|p| p.module == "std.path.normalize")
            .collect(),
    );
}

#[test]
fn path_normalize_contracts_fail_before_insufficient_capacity_or_forged_path() {
    for main in [
        // A buffer one byte short of the normalized form's exact length.
        r#"
@id("app.main")
fn main() -> i64
{
    let source = [47u8, 97u8, 47u8, 98u8, 47u8, 46u8, 46u8, 47u8, 99u8];
    let path = path_from_bytes(bytes_copy(array_as_slice(source)));
    let normalized = path_normalize(path, bytes_zeroed(3usize));
    if path_length(normalized) == 4usize { 0 } else { 1 }
}
"#,
        // A forged Path whose length exceeds its buffer capacity is rejected
        // by the length observer before any scan.
        r#"
@id("app.main")
fn main() -> i64
{
    let path = Path { data: bytes_zeroed(2usize), length: 4usize };
    if normalized_path_len(path) == 0usize { 0 } else { 1 }
}
"#,
        // The same forged Path is rejected by the normalizing transition too.
        r#"
@id("app.main")
fn main() -> i64
{
    let path = Path { data: bytes_zeroed(2usize), length: 4usize };
    let normalized = path_normalize(path, bytes_zeroed(4usize));
    if path_length(normalized) == 0usize { 0 } else { 1 }
}
"#,
        // A pull index at the normalized length names a byte past the form.
        r#"
@id("app.main")
fn main() -> i64
{
    let source = [97u8];
    let data = bytes_copy(array_as_slice(source));
    let view = bytes_as_slice(data);
    let total = normalized_len(view, 1usize);
    if normalized_byte(view, 1usize, total) == 0u8 { 0 } else { 1 }
}
"#,
        // seg_end rejects a start past the logical length before any scan.
        r#"
@id("app.main")
fn main() -> i64
{
    let source = [97u8, 98u8];
    let data = bytes_copy(array_as_slice(source));
    let view = bytes_as_slice(data);
    if seg_end(view, 1usize, 2usize) == 1usize { 0 } else { 1 }
}
"#,
        // seg_start rejects a from past the logical length before any scan.
        r#"
@id("app.main")
fn main() -> i64
{
    let source = [97u8, 98u8];
    let data = bytes_copy(array_as_slice(source));
    let view = bytes_as_slice(data);
    if seg_start(view, 1usize, 2usize) == 1usize { 0 } else { 1 }
}
"#,
        // skip_from rejects a from past the logical length before any scan.
        r#"
@id("app.main")
fn main() -> i64
{
    let source = [97u8, 98u8];
    let data = bytes_copy(array_as_slice(source));
    let view = bytes_as_slice(data);
    if skip_from(view, 1usize, 2usize) == 0usize { 0 } else { 1 }
}
"#,
        // normalized_len rejects a logical length past the borrowed view.
        r#"
@id("app.main")
fn main() -> i64
{
    let source = [97u8];
    let data = bytes_copy(array_as_slice(source));
    let view = bytes_as_slice(data);
    if normalized_len(view, 2usize) == 0usize { 0 } else { 1 }
}
"#,
    ] {
        fails(main);
    }
}

#[test]
fn path_normalize_copy_preserves_the_borrowed_input_and_its_bytes() {
    returns(
        r#"
@id("app.main")
fn main() -> i64
{
    let source = [47u8, 97u8, 47u8, 98u8, 47u8, 46u8, 46u8, 47u8, 99u8];
    let path = path_from_bytes(bytes_copy(array_as_slice(source)));
    let normalized = path_normalize(path, bytes_zeroed(4usize));
    let unchanged = path_length(path) == 9usize;
    let sized = path_length(normalized) == 4usize;
    let bytes = path_finish(normalized);
    let head = match byte_get(bytes_as_slice(bytes), 0usize) { Option::Some { value } => value, Option::None {} => 0u8, };
    let tail = match byte_get(bytes_as_slice(bytes), 3usize) { Option::Some { value } => value, Option::None {} => 0u8, };
    let retained = path_finish(path);
    if unchanged && sized && head == 47u8 && tail == 99u8 && byte_len(bytes_as_slice(retained)) == 9usize { 0 } else { 1 }
}
"#,
        "0",
    );
}

#[test]
fn path_normalize_graph_retains_ownership_cleanup_schema_and_rejects_forged_or_drifted_projections()
{
    let main = r#"
@id("app.main")
fn main() -> i64
{
    let source = [47u8, 97u8, 47u8, 98u8, 47u8, 46u8, 46u8, 47u8, 99u8];
    let path = path_from_bytes(bytes_copy(array_as_slice(source)));
    let normalized = path_normalize(path, bytes_zeroed(4usize));
    let retained = path_finish(path);
    let bytes = path_finish(normalized);
    if byte_len(bytes_as_slice(bytes)) == 4usize && byte_len(bytes_as_slice(retained)) == 9usize { 0 } else { 1 }
}
"#;
    let canonical = canonical_checked(main);
    let program = parse(&canonical, "std-path-normalize-graph.spx").unwrap();
    let json = graph::to_json(&program).expect("path normalize graph serializes");
    graph::verify_json(&program, &json)
        .expect("derived path normalize graph independently replays");
    for id in [
        "std.path.normalize.seg-end",
        "std.path.normalize.seg-start",
        "std.path.normalize.seg-is-dot",
        "std.path.normalize.seg-is-dotdot",
        "std.path.normalize.skip-from",
        "std.path.normalize.seg-retained",
        "std.path.normalize.is-absolute",
        "std.path.normalize.leading-parents",
        "std.path.normalize.kept-bytes",
        "std.path.normalize.kept-count",
        "std.path.normalize.normalized-len",
        "std.path.normalize.parent-region",
        "std.path.normalize.emitted-offset",
        "std.path.normalize.body-owner",
        "std.path.normalize.normalized-byte",
        "std.path.normalize.path-length",
        "std.path.normalize.into",
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
    // The normalizing transition borrows its input Path, transfers exactly
    // the caller's output buffer, and carries that owner as its cleanup root
    // under the existing v5 schema.
    let into = node("std.path.normalize.into");
    assert_eq!(into["params"][0]["ownership_mode"], "borrow");
    assert_eq!(into["params"][1]["ownership_mode"], "own");
    assert_eq!(into["result"]["ownership_mode"], "own");
    assert_eq!(into["cleanup"]["schema"], "semaprax.cleanup-plan.v5");
    assert_eq!(
        into["cleanup"]["entry_state"]["live_owned_parameters"]
            .as_array()
            .unwrap()
            .len(),
        1,
        "the borrowed input Path must not enter the cleanup inventory"
    );
    assert_eq!(
        into["cleanup"]["slots"][0]["field_liveness_shape"]["lifecycle"],
        "core.bytes.drop"
    );
    // The length observer takes the same borrowed Path record and returns a
    // Copy scalar: a record observer selects the same contract-carrying v5
    // schema with an empty owned inventory, exactly as `std.io.lines`'s
    // Reader observers do. The selection is a semantic fact of the shape, not
    // of a backend layout.
    let length = node("std.path.normalize.path-length");
    assert_eq!(length["params"][0]["ownership_mode"], "borrow");
    assert_eq!(length["result"]["ownership_mode"], "value");
    assert_eq!(length["cleanup"]["schema"], "semaprax.cleanup-plan.v5");
    assert!(
        length["cleanup"]["entry_state"]["live_owned_parameters"]
            .as_array()
            .unwrap()
            .is_empty(),
        "the length observer borrowed its Path into the cleanup inventory"
    );
    assert!(
        length["cleanup"]["slots"].as_array().unwrap().is_empty(),
        "the length observer acquired a cleanup slot"
    );
    // The pure lexical view helpers borrow their Slice view and return Copy
    // scalars, so they own no cleanup and select v2 uniformly.
    for id in [
        "std.path.normalize.seg-end",
        "std.path.normalize.seg-start",
        "std.path.normalize.seg-is-dot",
        "std.path.normalize.seg-is-dotdot",
        "std.path.normalize.skip-from",
        "std.path.normalize.seg-retained",
        "std.path.normalize.is-absolute",
        "std.path.normalize.leading-parents",
        "std.path.normalize.kept-bytes",
        "std.path.normalize.kept-count",
        "std.path.normalize.normalized-len",
        "std.path.normalize.emitted-offset",
        "std.path.normalize.body-owner",
        "std.path.normalize.normalized-byte",
    ] {
        let helper = node(id);
        assert_eq!(helper["params"][0]["ownership_mode"], "borrow", "{id}");
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
    // parent_region takes no Slice view at all: its sole parameter is a Copy
    // scalar, but it still selects the same pure v2 schema.
    let parent_region = node("std.path.normalize.parent-region");
    assert_eq!(parent_region["params"][0]["ownership_mode"], "value");
    assert_eq!(parent_region["result"]["ownership_mode"], "value");
    assert_eq!(
        parent_region["cleanup"]["schema"],
        "semaprax.cleanup-plan.v2"
    );
    assert!(parent_region["cleanup"]["slots"]
        .as_array()
        .unwrap()
        .is_empty());
    // A reminted field identity and drifted source both fail replay.
    let forged = json.replace(
        "std.path.value.path.data",
        "std.path.value.path.forged-data",
    );
    assert_ne!(forged, json);
    assert!(graph::verify_json(&program, &forged).is_err());
    let drifted = parse(
        &canonical.replace(
            "byte_len(bytes_as_slice(retained)) == 9usize",
            "byte_len(bytes_as_slice(retained)) == 8usize",
        ),
        "drifted-path-normalize.spx",
    )
    .unwrap();
    assert_ne!(format::canonical(&drifted), canonical);
    assert!(graph::verify_json(&drifted, &json).is_err());
}

/// Keep every normalization case intact but admit only its local call closure
/// in the scratch workspace. The shipped representative package also runs
/// unchanged.
pub(super) fn conformance_manifests(scratch: &Path, manifest: &Path) -> Vec<PathBuf> {
    use semaprax::ast::{ModuleUseKind, Type};
    use std::collections::BTreeSet;
    const SOURCE: &str = include_str!("path_normalize_cases.spx");
    const CASES: &[&str] = &[
        "test_dot_segment",
        "test_parent_segment",
        "test_trailing_parent",
        "test_relative_parents",
        "test_root_parent",
        "test_separator_run",
        "test_leading_parent",
        "test_cancelled_chain",
        "test_empty_relative",
        "test_absolute_chain",
    ];
    let parsed = parse(SOURCE, "path-normalize-cases.spx").unwrap();
    assert_eq!(format::canonical(&parsed), SOURCE);
    let actual: Vec<_> = parsed
        .functions
        .iter()
        .filter(|f| f.name.starts_with("test_"))
        .map(|f| f.name.as_str())
        .collect();
    assert_eq!(actual, CASES, "path normalize case inventory changed");
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
            "path-normalize-entry.spx",
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
            if import.kind == ModuleUseKind::Function
                && import.target_module == "std.path.normalize"
            {
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
    let library = std::fs::read_to_string(package.join("src/normalize.spx")).unwrap();
    let library = parse(&library, "normalize.spx").unwrap();
    let public: BTreeSet<_> = library
        .functions
        .iter()
        .map(|f| f.stable_id.clone())
        .collect();
    assert_eq!(
        covered, public,
        "every path normalize function needs direct executed coverage"
    );
    manifests
}

/// Each case balances its declared live `Bytes` bound: the input Path buffer
/// holds the borrowed source bytes beside the caller's output buffer, and
/// nothing else.
pub(super) fn live_byte_bound(manifest: &Path) -> usize {
    let _ = manifest;
    2
}
