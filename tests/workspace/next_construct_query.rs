//! Focused evidence for the additive `next_constructs` Universal Semantic
//! Query v1 operation (issue #198). See
//! `docs/UNIVERSAL-SEMANTIC-QUERY-V1.md#next_constructs` for the exact scope
//! and `src/project/next_construct_query.rs` for the implementation and its
//! documented nonclaims.
//!
//! The fixture below uses the `useful-data-command.v1` project profile
//! (schema `semaprax.project.v4`) because Project v1's default `ScalarV1`
//! profile admits only Copy-scalar function signatures end to end, and no
//! project profile admits an arbitrary record type as a parameter at all;
//! `useful-data-command.v1` is the narrowest profile that admits both `own
//! Bytes` parameters and a real, checked non-empty effect
//! (`process.stdout.write`, the one effect `project_effects_admitted` grants
//! this profile) together, which is enough surface to exercise both the
//! own-mode exclusion and the effect-gated call exclusion this operation
//! performs.
//!
//! The following focused command passed locally:
//!
//! ```sh
//! CARGO_TARGET_DIR=target/private \
//!   cargo test --locked -p semaprax --test workspace \
//!   next_construct_query --no-fail-fast
//! ```

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use semaprax::diagnostic::Diagnostic;
use semaprax::hir::{ResolvedExprKind, ResolvedFunction, ResolvedStatement};
use semaprax::project::{
    with_authenticated_project, ProjectRevision, SemanticQuery, SemanticWorkspaceService,
    SEMANTIC_QUERY_NEXT_CONSTRUCTS_SCHEMA, SEMANTIC_QUERY_RESULT_SCHEMA,
};
use serde_json::Value;

static SERIAL: AtomicU64 = AtomicU64::new(0);

const MANIFEST: &str = "schema = \"semaprax.project.v4\"\nname = \"next-construct\"\nversion = \"0.1.0\"\nprofile = \"useful-data-command.v1\"\nentry = \"next_construct.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"next_construct.command\"]\ncommand = \"next_construct.command\"\ncapabilities = [\"process.stdout.write\"]\ntests = [\"next_construct.tests\"]\n";

// `useful-data-command.v1`'s module-level `permit` admission
// (`permits_admitted` in `retained_validation.rs`) only exempts the exact
// singleton `["process.stdout.write"]` permit set when it is declared by the
// manifest's own `entry` module, so every declaration below lives in
// `next_construct.app` rather than a separate `core` module.
const APP: &str = r#"module next_construct.app;

permit { process.stdout.write }

@id("next_construct.identity_bytes")
fn identity_bytes(value: own Bytes) -> Bytes
{
    value
}

@id("next_construct.zero_bytes")
fn zero_bytes() -> Bytes
{
    let seed = [0u8];
    bytes_copy(array_as_slice(seed))
}

@id("next_construct.write_bytes")
fn write_bytes() -> Bytes
    uses { process.stdout.write }
{
    let seed = [0u8];
    bytes_copy(array_as_slice(seed))
}

@id("next_construct.write_count")
fn write_count() -> i64
    uses { process.stdout.write }
{
    1
}

@id("next_construct.target")
fn target(payload: own Bytes, count: i64) -> i64
{
    let forwarded = identity_bytes(payload);
    let size = byte_len(bytes_as_slice(forwarded));
    if size == 0usize { count } else { count }
}

@id("next_construct.command")
fn command(input: borrow Slice<u8>) -> bool
{
    byte_len(input) == byte_len(input)
}

@id("next_construct.app.main")
fn main() -> i64
{
    0
}
"#;

const TESTS: &str = r#"module next_construct.tests;

@id("next_construct.tests.main")
fn main() -> i64
{
    0
}
"#;

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-next-construct-query-v1-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("semaprax.toml"), MANIFEST).unwrap();
        write_canonical(&root.join("src/app.spx"), "src/app.spx", APP);
        write_canonical(&root.join("src/tests.spx"), "src/tests.spx", TESTS);
        Self(root.canonicalize().unwrap())
    }

    fn revision(&self) -> Arc<ProjectRevision> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn write_canonical(path: &Path, module_path: &str, source: &str) {
    let program = semaprax::parse(source, module_path).unwrap();
    let canonical = semaprax::format::canonical(&program);
    std::fs::write(path, canonical).unwrap();
}

fn target_function(revision: &ProjectRevision) -> &ResolvedFunction {
    revision
        .entry_program()
        .functions
        .iter()
        .find(|function| function.id.as_str() == "next_construct.target")
        .expect("next_construct.target is retained")
}

/// The `payload` argument expression inside `identity_bytes(payload)`: an
/// `own Bytes` position (`ownership_mode == "own"`).
fn own_position_expression_id(function: &ResolvedFunction) -> String {
    let ResolvedExprKind::Block { statements, .. } = &function.body.kind else {
        panic!("expected the function body to be a block");
    };
    let ResolvedStatement::Let { value, .. } = &statements[0] else {
        panic!("expected the first statement to be a let");
    };
    let ResolvedExprKind::Call { args, .. } = &value.kind else {
        panic!("expected identity_bytes to be a direct call");
    };
    args[0].id.as_str().to_owned()
}

/// The tail `count` read inside the `if`'s `then` branch: an `i64` `value`
/// position.
fn value_position_expression_id(function: &ResolvedFunction) -> String {
    let ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
        panic!("expected the function body to be a block");
    };
    let ResolvedExprKind::If { then_branch, .. } = &tail.kind else {
        panic!("expected the tail to be an if expression");
    };
    let ResolvedExprKind::Block { tail: inner, .. } = &then_branch.kind else {
        panic!("expected the then branch to be a block");
    };
    inner.id.as_str().to_owned()
}

fn assert_code<T>(result: Result<T, Vec<Diagnostic>>, code: &str) {
    let errors = result.err().unwrap_or_else(|| panic!("expected {code}"));
    assert!(errors.iter().any(|error| error.code == code), "{errors:?}");
}

fn value(source: &str) -> Value {
    serde_json::from_str(source).unwrap()
}

fn payload(result: &semaprax::project::SemanticQueryResult) -> Value {
    let parsed = value(result.to_json());
    assert_eq!(parsed["schema"], SEMANTIC_QUERY_RESULT_SCHEMA);
    assert_eq!(parsed["operation"], "next_constructs");
    assert_eq!(parsed["authority"], false);
    parsed["payload"].clone()
}

fn names(entries: &Value, key: &str) -> Vec<String> {
    entries
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry[key].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn own_position_admits_no_identifier_and_excludes_the_owned_parameter_with_a_reason() {
    let fixture = Fixture::new();
    let service = SemanticWorkspaceService::open(fixture.revision()).unwrap();
    let generation = service.active_generation();
    let revision = generation.workspace_revision();
    let function = target_function(generation.revision());
    let expression_id = own_position_expression_id(function);

    let query =
        SemanticQuery::next_constructs(revision, "next_construct.target", &expression_id).unwrap();
    assert_eq!(
        SemanticQuery::from_json(query.to_json().as_bytes()).unwrap(),
        query
    );
    let result = service.query(query.to_json().as_bytes()).unwrap();
    let body = payload(&result);

    assert_eq!(body["schema"], SEMANTIC_QUERY_NEXT_CONSTRUCTS_SCHEMA);
    assert_eq!(body["position_ownership_mode"], "own");
    assert_eq!(body["expected_type_identity"], "bytes");

    // No literal: Bytes is not one of Explicit Mutation v1's Copy scalars.
    assert!(!names(&body["admitted"], "construct").contains(&"literal".to_owned()));
    // The matching-type `payload` parameter is `own`, so it is excluded, not
    // silently admitted and not silently omitted.
    let excluded_params: Vec<Value> = body["excluded"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["construct"] == "parameter_reference")
        .cloned()
        .collect();
    assert_eq!(excluded_params.len(), 1);
    assert_eq!(excluded_params[0]["name"], "payload");
    assert_eq!(
        excluded_params[0]["reason"],
        "flow_sensitive_availability_not_computed"
    );
    assert!(!names(&body["admitted"], "construct")
        .iter()
        .any(|construct| construct == "parameter_reference"));

    // Calls returning `Bytes`: `identity_bytes` and `zero_bytes` need no
    // effect `target` lacks, so both are admitted; `write_bytes` needs
    // `process.stdout.write`, which `target` does not declare, so it is
    // excluded with the exact missing effect.
    let admitted_calls = names(&body["admitted"], "stable_id");
    assert!(admitted_calls.contains(&"next_construct.identity_bytes".to_owned()));
    assert!(admitted_calls.contains(&"next_construct.zero_bytes".to_owned()));
    let excluded_calls: Vec<Value> = body["excluded"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["construct"] == "call")
        .cloned()
        .collect();
    assert_eq!(excluded_calls.len(), 1);
    assert_eq!(excluded_calls[0]["stable_id"], "next_construct.write_bytes");
    assert_eq!(excluded_calls[0]["reason"], "effect_not_available");
    assert_eq!(
        excluded_calls[0]["missing_effects"],
        serde_json::json!(["process.stdout.write"])
    );

    // Determinism: repeat execution against the same unchanged revision
    // yields byte-identical results.
    let repeat = service.query(query.to_json().as_bytes()).unwrap();
    assert_eq!(repeat.to_json(), result.to_json());
    assert_eq!(repeat.result_digest(), result.result_digest());

    // Replay succeeds against a retained snapshot.
    let snapshot = service.snapshot(revision).unwrap();
    let replayed = SemanticQuery::replay(
        &snapshot,
        query.to_json().as_bytes(),
        result.result_digest(),
        result.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(replayed.to_json(), result.to_json());
}

#[test]
fn value_position_admits_a_literal_and_the_matching_scalar_parameter() {
    let fixture = Fixture::new();
    let service = SemanticWorkspaceService::open(fixture.revision()).unwrap();
    let generation = service.active_generation();
    let revision = generation.workspace_revision();
    let function = target_function(generation.revision());
    let expression_id = value_position_expression_id(function);

    let query =
        SemanticQuery::next_constructs(revision, "next_construct.target", &expression_id).unwrap();
    let body = payload(&service.query(query.to_json().as_bytes()).unwrap());

    assert_eq!(body["position_ownership_mode"], "value");
    assert_eq!(body["expected_type_identity"], "i64");
    assert!(names(&body["admitted"], "construct").contains(&"literal".to_owned()));
    let admitted_params: Vec<Value> = body["admitted"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["construct"] == "parameter_reference")
        .cloned()
        .collect();
    assert_eq!(admitted_params.len(), 1);
    assert_eq!(admitted_params[0]["name"], "count");
    assert_eq!(admitted_params[0]["insertion_template"], "count");
    // `payload` is `Bytes`, not `i64`, so it is neither admitted nor excluded
    // here: a type mismatch is silently omitted, never asserted either way.
    assert!(!body["admitted"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["name"] == "payload"));
    assert!(!body["excluded"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["name"] == "payload"));

    // `write_count` returns `i64` but needs `process.stdout.write`, which
    // `target` lacks.
    let excluded_calls: Vec<Value> = body["excluded"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["construct"] == "call")
        .cloned()
        .collect();
    assert_eq!(excluded_calls.len(), 1);
    assert_eq!(excluded_calls[0]["stable_id"], "next_construct.write_count");
    assert_eq!(excluded_calls[0]["reason"], "effect_not_available");
}

#[test]
fn stale_revision_unknown_declaration_and_unknown_expression_fail_closed() {
    let fixture = Fixture::new();
    let service = SemanticWorkspaceService::open(fixture.revision()).unwrap();
    let generation = service.active_generation();
    let revision = generation.workspace_revision();
    let function = target_function(generation.revision());
    let expression_id = own_position_expression_id(function);

    let unknown_declaration =
        SemanticQuery::next_constructs(revision, "next_construct.missing", &expression_id).unwrap();
    assert_code(
        service.query(unknown_declaration.to_json().as_bytes()),
        "SPX-G531",
    );

    let unknown_expression =
        SemanticQuery::next_constructs(revision, "next_construct.target", "foreign.expr").unwrap();
    assert_code(
        service.query(unknown_expression.to_json().as_bytes()),
        "SPX-G531",
    );

    let stale = SemanticQuery::next_constructs(
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        "next_construct.target",
        &expression_id,
    )
    .unwrap();
    assert_code(service.query(stale.to_json().as_bytes()), "SPX-G533");
}
