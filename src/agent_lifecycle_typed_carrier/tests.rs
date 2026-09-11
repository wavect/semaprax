use std::path::{Path, PathBuf};

use crate::agent_interaction_schema::{compile_agent_interaction_schema, CompiledInteractionSchema};
use crate::agent_runtime::AgentCancellation;
use crate::diagnostic::quote_json;
use crate::hir::{self, ResolvedProgram};
use crate::interpreter::retained_call::{prepare_retained_call, RetainedCallOutcome, RetainedValue};

use super::binding::{LifecycleStageRole, StageBinding};
use super::checkpoint;
use super::ownership::{stage_and_evaluate, OwnershipLedger};
use super::projection::{to_retained, InteractionTypeGraph};
use super::registry::{call_typed_operation, TypedCarrierHandler, TypedCarrierOperation, TypedCarrierRegistry};

const MAX_STEPS: usize = 10_000;

const OUTER_FIXTURE: &str = r#"
module test.agent_lifecycle_typed_carrier;

@id("inner.type")
record Inner {
    @id("inner.x")
    x: i64,
    @id("inner.blob")
    blob: Bytes,
}

@id("outer.type")
record Outer {
    @id("outer.flag")
    flag: bool,
    @id("outer.tag")
    tag: u8,
    @id("outer.count")
    count: usize,
    @id("outer.marker")
    marker: i64,
    @id("outer.inner")
    inner: Inner,
}

@id("outer.combined")
fn combined(value: own Outer) -> i64
{
    value.marker + value.inner.x
}

@id("outer.blob_len")
fn blob_len(value: own Outer) -> usize
{
    let view = bytes_as_slice(value.inner.blob);
    byte_len(view)
}

@id("outer.strict")
fn strict(value: own Outer) -> i64
    ensures result == 0
{
    value.marker
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

const RENAMED_FIXTURE: &str = r#"
module test.agent_lifecycle_typed_carrier;

@id("inner.type")
record InnerRenamed {
    @id("inner.x")
    xx: i64,
    @id("inner.blob")
    bb: Bytes,
}

@id("outer.type")
record OuterRenamed {
    @id("outer.flag")
    ff: bool,
    @id("outer.tag")
    tt: u8,
    @id("outer.count")
    cc: usize,
    @id("outer.marker")
    mm: i64,
    @id("outer.inner")
    ii: InnerRenamed,
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

const STRUCTURAL_FIXTURE: &str = r#"
module test.agent_lifecycle_typed_carrier;

@id("inner.type")
record Inner {
    @id("inner.x")
    x: i64,
    @id("inner.blob")
    blob: Bytes,
}

@id("outer.type")
record Outer {
    @id("outer.flag")
    flag: bool,
    @id("outer.tag")
    tag: u8,
    @id("outer.count")
    count: usize,
    @id("outer.marker")
    marker: i64,
    @id("outer.extra")
    extra: i64,
    @id("outer.inner")
    inner: Inner,
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

const CHOICE_FIXTURE: &str = r#"
module test.agent_lifecycle_typed_carrier_choice;

@id("choice.type")
variant Choice {
    @id("choice.a")
    A {
        @id("choice.a.n")
        n: i64,
    },
    @id("choice.b")
    B,
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

const TEXT_FIXTURE: &str = r#"
module test.agent_lifecycle_typed_carrier_text;

@id("text.type")
record WithText {
    @id("text.value")
    value: string,
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn write_temp(source: &str, label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-agent-lifecycle-typed-carrier-{label}-{}-{}.spx",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, source).unwrap();
    path
}

fn resolved(source: &str, path: &Path) -> ResolvedProgram {
    let program = crate::check(source, path).expect("fixture checks");
    let resolved = hir::resolve(&program).expect("fixture resolves");
    hir::validate(&resolved).expect("fixture validates");
    resolved
}

fn graph(program: &ResolvedProgram, root_type_id: &str) -> InteractionTypeGraph {
    InteractionTypeGraph::derive(program, root_type_id).expect("fixture derives a type graph")
}

fn compiled_schema(path: &Path, root_type_id: &str) -> CompiledInteractionSchema {
    compile_agent_interaction_schema(path, root_type_id).expect("fixture compiles an interaction schema")
}

/// Builds one canonical `outer.type` interaction value document by hand,
/// in exact declared field order, so it is admitted by
/// `agent_interaction_schema`'s byte-exact canonical-replay decoder.
/// Returns `(document, value_json)` so callers can independently
/// reconstruct an expected checkpoint document without going through
/// [`checkpoint::encode`].
fn outer_document(
    schema_digest: &str,
    flag: bool,
    tag: u8,
    count: u64,
    marker: i64,
    x: i64,
    blob: &[u8],
) -> (String, String) {
    let blob_json = format!(
        "[{}]",
        blob.iter().map(u8::to_string).collect::<Vec<_>>().join(",")
    );
    let inner = format!("{{\"fields\":{{\"inner.x\":\"{x}\",\"inner.blob\":{blob_json}}}}}");
    let value = format!(
        "{{\"fields\":{{\"outer.flag\":{flag},\"outer.tag\":\"{tag}\",\"outer.count\":\"{count}\",\"outer.marker\":\"{marker}\",\"outer.inner\":{inner}}}}}"
    );
    let digest = quote_json(schema_digest);
    let document = format!(
        "{{\"schema\":\"semaprax.agent-interaction-value.v1\",\"root_type_id\":\"outer.type\",\"schema_digest\":{digest},\"value\":{value}}}\n"
    );
    (document, value)
}

fn inner_document(schema_digest: &str, x: i64, blob: &[u8]) -> String {
    let blob_json = format!(
        "[{}]",
        blob.iter().map(u8::to_string).collect::<Vec<_>>().join(",")
    );
    let digest = quote_json(schema_digest);
    format!(
        "{{\"schema\":\"semaprax.agent-interaction-value.v1\",\"root_type_id\":\"inner.type\",\"schema_digest\":{digest},\"value\":{{\"fields\":{{\"inner.x\":\"{x}\",\"inner.blob\":{blob_json}}}}}}}\n"
    )
}

fn choice_document(schema_digest: &str, case: &str, fields_json: &str) -> String {
    let digest = quote_json(schema_digest);
    format!(
        "{{\"schema\":\"semaprax.agent-interaction-value.v1\",\"root_type_id\":\"choice.type\",\"schema_digest\":{digest},\"value\":{{\"case\":\"{case}\",\"fields\":{fields_json}}}}}\n"
    )
}

struct EchoHandler {
    response: Vec<u8>,
}
impl TypedCarrierHandler for EchoHandler {
    fn execute(&mut self, _operation_id: &str, _argument: &RetainedValue) -> Vec<u8> {
        self.response.clone()
    }
}

struct PanicHandler;
impl TypedCarrierHandler for PanicHandler {
    fn execute(&mut self, _operation_id: &str, _argument: &RetainedValue) -> Vec<u8> {
        panic!("a refused operation must never reach the handler");
    }
}

// ---------------------------------------------------------------------
// Interpreter execution: the rich boundary through the real retained-call
// evaluator, and ownership settlement on success and on failure.
// ---------------------------------------------------------------------

struct OuterFixture {
    program: ResolvedProgram,
    graph: InteractionTypeGraph,
    schema: CompiledInteractionSchema,
    path: PathBuf,
}

fn outer_fixture(source: &str, label: &str) -> OuterFixture {
    let path = write_temp(source, label);
    let program = resolved(source, &path);
    let graph = graph(&program, "outer.type");
    let schema = compiled_schema(&path, "outer.type");
    OuterFixture {
        program,
        graph,
        schema,
        path,
    }
}

impl Drop for OuterFixture {
    fn drop(&mut self) {
        std::fs::remove_file(&self.path).ok();
    }
}

#[test]
fn admits_a_valid_value_executes_it_through_the_real_interpreter_and_settles_ownership() {
    let fixture = outer_fixture(OUTER_FIXTURE, "combined");
    let prepared = prepare_retained_call(&fixture.program, "outer.combined").expect("prepares");
    let (document, _) = outer_document(fixture.schema.schema().digest(), true, 7, 3, 10, 5, &[1, 2, 3]);
    let decoded = fixture.schema.decode(document.as_bytes()).expect("decodes");

    let ledger = OwnershipLedger::new();
    let cancellation = AgentCancellation::new();
    let binding = StageBinding::new(LifecycleStageRole::Reduce, &fixture.schema);
    let evaluation = stage_and_evaluate(
        &ledger,
        &cancellation,
        &fixture.program,
        &prepared,
        &fixture.graph,
        &binding,
        decoded,
        MAX_STEPS,
    )
    .expect("evaluation succeeds");

    match evaluation.outcome {
        RetainedCallOutcome::Returned(RetainedValue::I64(value)) => assert_eq!(value, 10 + 5),
        other => panic!("unexpected outcome: {other:?}"),
    }
    assert_eq!(ledger.live(), 0, "ownership must settle on success");
}

#[test]
fn owned_bytes_leaf_identity_survives_the_boundary_not_just_a_scalar() {
    let fixture = outer_fixture(OUTER_FIXTURE, "blob-len");
    let prepared = prepare_retained_call(&fixture.program, "outer.blob_len").expect("prepares");
    let blob = [9u8, 8, 7, 6, 5];
    let (document, _) = outer_document(fixture.schema.schema().digest(), false, 1, 0, 0, 0, &blob);
    let decoded = fixture.schema.decode(document.as_bytes()).expect("decodes");

    let ledger = OwnershipLedger::new();
    let cancellation = AgentCancellation::new();
    let binding = StageBinding::new(LifecycleStageRole::Reduce, &fixture.schema);
    let evaluation = stage_and_evaluate(
        &ledger,
        &cancellation,
        &fixture.program,
        &prepared,
        &fixture.graph,
        &binding,
        decoded,
        MAX_STEPS,
    )
    .expect("evaluation succeeds");

    match evaluation.outcome {
        RetainedCallOutcome::Returned(RetainedValue::Usize(length)) => {
            assert_eq!(length, blob.len() as u64);
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
    assert_eq!(ledger.live(), 0);
}

#[test]
fn contract_failure_still_settles_ownership_and_does_not_replace_the_failure_status() {
    let fixture = outer_fixture(OUTER_FIXTURE, "strict");
    let prepared = prepare_retained_call(&fixture.program, "outer.strict").expect("prepares");
    // marker != 0 violates `ensures result == 0`.
    let (document, _) = outer_document(fixture.schema.schema().digest(), true, 0, 0, 42, 0, &[]);
    let decoded = fixture.schema.decode(document.as_bytes()).expect("decodes");

    let ledger = OwnershipLedger::new();
    let cancellation = AgentCancellation::new();
    let binding = StageBinding::new(LifecycleStageRole::Effect, &fixture.schema);
    let evaluation = stage_and_evaluate(
        &ledger,
        &cancellation,
        &fixture.program,
        &prepared,
        &fixture.graph,
        &binding,
        decoded,
        MAX_STEPS,
    )
    .expect("the call itself is not refused; the postcondition fails inside it");

    match evaluation.outcome {
        RetainedCallOutcome::LanguageFailure(_) => {}
        other => panic!("expected a language-level contract failure, got {other:?}"),
    }
    assert_eq!(
        ledger.live(),
        0,
        "an owned temporary must settle on a contract failure too"
    );
}

#[test]
fn cancellation_refuses_before_any_owned_temporary_is_created() {
    let fixture = outer_fixture(OUTER_FIXTURE, "cancel");
    let prepared = prepare_retained_call(&fixture.program, "outer.combined").expect("prepares");
    let (document, _) = outer_document(fixture.schema.schema().digest(), true, 1, 1, 1, 1, &[]);
    let decoded = fixture.schema.decode(document.as_bytes()).expect("decodes");

    let ledger = OwnershipLedger::new();
    let cancellation = AgentCancellation::new();
    cancellation.cancel();
    let binding = StageBinding::new(LifecycleStageRole::Reduce, &fixture.schema);
    let error = stage_and_evaluate(
        &ledger,
        &cancellation,
        &fixture.program,
        &prepared,
        &fixture.graph,
        &binding,
        decoded,
        MAX_STEPS,
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-Z213");
    assert_eq!(ledger.live(), 0);
}

// ---------------------------------------------------------------------
// Admission: wrong nominal type, wrong variant, stale structural schema,
// display-rename invariance, and unsupported (`string`) projection.
// ---------------------------------------------------------------------

#[test]
fn wrong_nominal_type_is_refused_before_admission() {
    let path = write_temp(OUTER_FIXTURE, "wrong-nominal");
    let outer_schema = compiled_schema(&path, "outer.type");
    let inner_schema = compiled_schema(&path, "inner.type");
    std::fs::remove_file(&path).ok();

    let binding = StageBinding::new(LifecycleStageRole::Observe, &outer_schema);
    let mismatched = inner_document(inner_schema.schema().digest(), 1, &[1]);
    let decoded = inner_schema.decode(mismatched.as_bytes()).expect("decodes under its own schema");

    let error = binding.admit(decoded).unwrap_err();
    assert_eq!(error.code, "SPX-Z210");
}

#[test]
fn wrong_variant_is_refused_before_unauthorized_dispatch() {
    let path = write_temp(CHOICE_FIXTURE, "wrong-variant");
    let schema = compiled_schema(&path, "choice.type");
    std::fs::remove_file(&path).ok();

    let binding = StageBinding::new(LifecycleStageRole::Authorize, &schema).expect_case("choice.a");
    let document = choice_document(schema.schema().digest(), "choice.b", "{}");
    let decoded = schema.decode(document.as_bytes()).expect("decodes");

    let error = binding.admit(decoded).unwrap_err();
    assert_eq!(error.code, "SPX-Z210");

    // The matching case is admitted.
    let matching = choice_document(schema.schema().digest(), "choice.a", "{\"choice.a.n\":\"5\"}");
    let decoded = schema.decode(matching.as_bytes()).expect("decodes");
    assert!(binding.admit(decoded).is_ok());
}

#[test]
fn stale_structural_schema_is_refused_but_display_rename_is_not() {
    let outer_path = write_temp(OUTER_FIXTURE, "stale-base");
    let renamed_path = write_temp(RENAMED_FIXTURE, "stale-renamed");
    let structural_path = write_temp(STRUCTURAL_FIXTURE, "stale-structural");

    let base_schema = compiled_schema(&outer_path, "outer.type");
    let renamed_schema = compiled_schema(&renamed_path, "outer.type");
    let structural_schema = compiled_schema(&structural_path, "outer.type");

    std::fs::remove_file(&outer_path).ok();
    std::fs::remove_file(&renamed_path).ok();
    std::fs::remove_file(&structural_path).ok();

    // A display-only rename leaves the schema revision unchanged...
    assert_eq!(base_schema.schema().digest(), renamed_schema.schema().digest());
    // ...but a genuine structural change (an added field) does not.
    assert_ne!(base_schema.schema().digest(), structural_schema.schema().digest());

    let binding = StageBinding::new(LifecycleStageRole::Initialize, &base_schema);

    // A value decoded under the renamed (structurally identical) schema is
    // still admitted: field identity persisted through the display rename.
    let (renamed_document, _) = outer_document(renamed_schema.schema().digest(), true, 2, 4, 6, 8, &[]);
    let decoded_renamed = renamed_schema.decode(renamed_document.as_bytes()).expect("decodes");
    assert!(binding.admit(decoded_renamed).is_ok());

    // A value decoded under the structurally different schema is refused:
    // stale structural schema use fails.
    let (structural_document, _) =
        outer_document(structural_schema.schema().digest(), true, 2, 4, 6, 8, &[]);
    // The structural schema has an extra `outer.extra` field the base
    // decoder cannot even parse against, so decode it against its own
    // (structural) schema first, then try to admit it under the base
    // binding built from the un-extended schema.
    let structural_document = structural_document.replacen(
        "\"outer.marker\":\"6\",",
        "\"outer.marker\":\"6\",\"outer.extra\":\"0\",",
        1,
    );
    let decoded_structural = structural_schema
        .decode(structural_document.as_bytes())
        .expect("decodes under its own (structural) schema");
    let error = binding.admit(decoded_structural).unwrap_err();
    assert_eq!(error.code, "SPX-Z210");
}

#[test]
fn field_order_mutation_is_refused_upstream_before_admission_ever_runs() {
    let path = write_temp(OUTER_FIXTURE, "field-order");
    let schema = compiled_schema(&path, "outer.type");
    std::fs::remove_file(&path).ok();

    let (canonical, _) = outer_document(schema.schema().digest(), true, 1, 2, 3, 4, &[5]);
    assert!(schema.decode(canonical.as_bytes()).is_ok());

    // Swap two fields' order inside the canonical `fields` object. The
    // document still parses as JSON and names the same fields and values,
    // but its canonical replay (always in declared order) no longer
    // matches these bytes, so `agent_interaction_schema::decode` itself
    // refuses it — this carrier never gets a `DecodedInteractionValue` to
    // admit in the first place.
    let mutated = canonical.replacen(
        "\"outer.flag\":true,\"outer.tag\":\"1\"",
        "\"outer.tag\":\"1\",\"outer.flag\":true",
        1,
    );
    assert_ne!(mutated, canonical);
    assert!(schema.decode(mutated.as_bytes()).is_err());
}

#[test]
fn projection_refuses_a_string_leaf_explicitly_never_silently_degrades_it() {
    let path = write_temp(TEXT_FIXTURE, "string-refused");
    let schema = compiled_schema(&path, "text.type");
    let program = resolved(TEXT_FIXTURE, &path);
    let graph = graph(&program, "text.type");
    std::fs::remove_file(&path).ok();

    let digest = quote_json(schema.schema().digest());
    let document =
        format!("{{\"schema\":\"semaprax.agent-interaction-value.v1\",\"root_type_id\":\"text.type\",\"schema_digest\":{digest},\"value\":{{\"fields\":{{\"text.value\":\"hello\"}}}}}}\n");
    let decoded = schema.decode(document.as_bytes()).expect("decodes");

    let error = to_retained(&graph, &decoded).unwrap_err();
    assert_eq!(error.code, "SPX-Z210");
}

// ---------------------------------------------------------------------
// The rich effect operation registry: selector/deployed-operation binding
// and malformed-result settlement.
// ---------------------------------------------------------------------

#[test]
fn incorrect_deployed_operation_is_refused_before_the_handler_is_ever_called() {
    let fixture = outer_fixture(OUTER_FIXTURE, "op-mismatch");
    let operation = TypedCarrierOperation {
        deployed_operation_id: "outer.op".to_owned(),
        argument: StageBinding::new(LifecycleStageRole::Effect, &fixture.schema),
        result: StageBinding::new(LifecycleStageRole::Effect, &fixture.schema),
    };
    let registry = TypedCarrierRegistry::new(vec![operation]);
    let ledger = OwnershipLedger::new();
    let cancellation = AgentCancellation::new();
    let (document, _) = outer_document(fixture.schema.schema().digest(), true, 1, 1, 1, 1, &[]);
    let decoded = fixture.schema.decode(document.as_bytes()).expect("decodes");
    let mut handler = PanicHandler;

    let error = call_typed_operation(
        &ledger,
        &cancellation,
        &registry,
        0,
        "a-different-deployed-operation",
        &fixture.graph,
        decoded,
        &fixture.schema,
        &mut handler,
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-Z211");
    assert_eq!(ledger.live(), 0);
}

#[test]
fn cancellation_refuses_a_typed_operation_before_dispatch_too() {
    let fixture = outer_fixture(OUTER_FIXTURE, "op-cancel");
    let operation = TypedCarrierOperation {
        deployed_operation_id: "outer.op".to_owned(),
        argument: StageBinding::new(LifecycleStageRole::Effect, &fixture.schema),
        result: StageBinding::new(LifecycleStageRole::Effect, &fixture.schema),
    };
    let registry = TypedCarrierRegistry::new(vec![operation]);
    let ledger = OwnershipLedger::new();
    let cancellation = AgentCancellation::new();
    cancellation.cancel();
    let (document, _) = outer_document(fixture.schema.schema().digest(), true, 1, 1, 1, 1, &[]);
    let decoded = fixture.schema.decode(document.as_bytes()).expect("decodes");
    let mut handler = PanicHandler;

    let error = call_typed_operation(
        &ledger,
        &cancellation,
        &registry,
        0,
        "outer.op",
        &fixture.graph,
        decoded,
        &fixture.schema,
        &mut handler,
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-Z213");
    assert_eq!(ledger.live(), 0);
}

#[test]
fn malformed_result_is_refused_and_the_argument_still_settles() {
    let fixture = outer_fixture(OUTER_FIXTURE, "op-malformed");
    let operation = TypedCarrierOperation {
        deployed_operation_id: "outer.op".to_owned(),
        argument: StageBinding::new(LifecycleStageRole::Effect, &fixture.schema),
        result: StageBinding::new(LifecycleStageRole::Effect, &fixture.schema),
    };
    let registry = TypedCarrierRegistry::new(vec![operation]);
    let ledger = OwnershipLedger::new();
    let cancellation = AgentCancellation::new();
    let (document, _) = outer_document(fixture.schema.schema().digest(), true, 1, 1, 1, 1, &[1, 2]);
    let decoded = fixture.schema.decode(document.as_bytes()).expect("decodes");
    let mut handler = EchoHandler {
        response: b"not a valid interaction value document".to_vec(),
    };

    let error = call_typed_operation(
        &ledger,
        &cancellation,
        &registry,
        0,
        "outer.op",
        &fixture.graph,
        decoded,
        &fixture.schema,
        &mut handler,
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-Z212");
    assert_eq!(
        ledger.live(),
        0,
        "the argument's ownership settles before the malformed result is even inspected"
    );
}

#[test]
fn a_well_formed_result_is_admitted_after_the_handler_call() {
    let fixture = outer_fixture(OUTER_FIXTURE, "op-success");
    let operation = TypedCarrierOperation {
        deployed_operation_id: "outer.op".to_owned(),
        argument: StageBinding::new(LifecycleStageRole::Effect, &fixture.schema),
        result: StageBinding::new(LifecycleStageRole::Effect, &fixture.schema),
    };
    let registry = TypedCarrierRegistry::new(vec![operation]);
    let ledger = OwnershipLedger::new();
    let cancellation = AgentCancellation::new();
    let (argument_document, _) =
        outer_document(fixture.schema.schema().digest(), true, 1, 1, 1, 1, &[1, 2]);
    let decoded_argument = fixture.schema.decode(argument_document.as_bytes()).expect("decodes");
    let (result_document, _) = outer_document(fixture.schema.schema().digest(), false, 9, 9, 9, 9, &[]);
    let mut handler = EchoHandler {
        response: result_document.clone().into_bytes(),
    };

    let admitted_result = call_typed_operation(
        &ledger,
        &cancellation,
        &registry,
        0,
        "outer.op",
        &fixture.graph,
        decoded_argument,
        &fixture.schema,
        &mut handler,
    )
    .expect("well-formed result is admitted");
    assert_eq!(admitted_result.canonical_json(), result_document.as_str());
    assert_eq!(ledger.live(), 0);
}

// ---------------------------------------------------------------------
// The rich value checkpoint codec: golden determinism, round trip, and
// every required refusal.
// ---------------------------------------------------------------------

#[test]
fn checkpoint_encoding_is_deterministic_and_independently_reconstructible() {
    let path = write_temp(OUTER_FIXTURE, "checkpoint-golden");
    let schema = compiled_schema(&path, "outer.type");
    std::fs::remove_file(&path).ok();

    let (document, value_json) = outer_document(schema.schema().digest(), true, 7, 3, 10, 5, &[1, 2, 3]);
    let decoded_first = schema.decode(document.as_bytes()).expect("decodes");
    let decoded_second = schema.decode(document.as_bytes()).expect("decodes again");

    let encoded_first = checkpoint::encode(&decoded_first).expect("encodes");
    let encoded_second = checkpoint::encode(&decoded_second).expect("encodes again");
    assert_eq!(
        encoded_first, encoded_second,
        "the same admitted value must always encode to the same bytes"
    );

    // Independently reconstruct the expected checkpoint document, without
    // calling `checkpoint::encode`'s own formula.
    let digest = quote_json(schema.schema().digest());
    let expected = format!(
        "{{\"schema\":\"semaprax.agent-lifecycle-typed-checkpoint.v1\",\"type_version\":1,\"root_type_id\":\"outer.type\",\"schema_digest\":{digest},\"value\":{value_json}}}\n"
    );
    assert_eq!(encoded_first, expected);
}

#[test]
fn checkpoint_round_trip_recovers_the_same_admitted_value() {
    let path = write_temp(OUTER_FIXTURE, "checkpoint-roundtrip");
    let schema = compiled_schema(&path, "outer.type");
    std::fs::remove_file(&path).ok();

    let (document, _) = outer_document(schema.schema().digest(), false, 2, 4, 6, 8, &[9, 9]);
    let decoded = schema.decode(document.as_bytes()).expect("decodes");
    let encoded = checkpoint::encode(&decoded).expect("encodes");
    let recovered = checkpoint::decode(&schema, schema.schema().digest(), encoded.as_bytes())
        .expect("round-trips");
    assert_eq!(decoded, recovered);
}

#[test]
fn checkpoint_rejects_an_unknown_type_version() {
    let path = write_temp(OUTER_FIXTURE, "checkpoint-version");
    let schema = compiled_schema(&path, "outer.type");
    std::fs::remove_file(&path).ok();

    let (document, _) = outer_document(schema.schema().digest(), true, 1, 1, 1, 1, &[]);
    let decoded = schema.decode(document.as_bytes()).expect("decodes");
    let encoded = checkpoint::encode(&decoded).expect("encodes");
    let mutated = encoded.replacen("\"type_version\":1,", "\"type_version\":2,", 1);

    let error = checkpoint::decode(&schema, schema.schema().digest(), mutated.as_bytes()).unwrap_err();
    assert_eq!(error.code, "SPX-Z212");
}

#[test]
fn checkpoint_rejects_a_stale_schema_binding() {
    let path = write_temp(OUTER_FIXTURE, "checkpoint-stale");
    let schema = compiled_schema(&path, "outer.type");
    std::fs::remove_file(&path).ok();

    let (document, _) = outer_document(schema.schema().digest(), true, 1, 1, 1, 1, &[]);
    let decoded = schema.decode(document.as_bytes()).expect("decodes");
    let encoded = checkpoint::encode(&decoded).expect("encodes");

    let error = checkpoint::decode(&schema, "sha256:0000000000000000000000000000000000000000000000000000000000000000", encoded.as_bytes())
        .unwrap_err();
    assert_eq!(error.code, "SPX-Z212");
}

#[test]
fn checkpoint_rejects_an_oversized_payload_before_any_parsing() {
    let path = write_temp(OUTER_FIXTURE, "checkpoint-oversized");
    let schema = compiled_schema(&path, "outer.type");
    std::fs::remove_file(&path).ok();

    let oversized = vec![b'a'; checkpoint::MAX_CHECKPOINT_BYTES + 1];
    let error = checkpoint::decode(&schema, schema.schema().digest(), &oversized).unwrap_err();
    assert_eq!(error.code, "SPX-Z212");
}

#[test]
fn checkpoint_rejects_a_malformed_value_payload() {
    let path = write_temp(OUTER_FIXTURE, "checkpoint-malformed");
    let schema = compiled_schema(&path, "outer.type");
    std::fs::remove_file(&path).ok();

    let (document, _) = outer_document(schema.schema().digest(), true, 1, 1, 1, 1, &[]);
    let decoded = schema.decode(document.as_bytes()).expect("decodes");
    let encoded = checkpoint::encode(&decoded).expect("encodes");
    // Corrupt the value payload while keeping the envelope well formed.
    let mutated = encoded.replacen("\"outer.flag\":true", "\"outer.flag\":\"not-a-bool\"", 1);

    let error = checkpoint::decode(&schema, schema.schema().digest(), mutated.as_bytes()).unwrap_err();
    assert_eq!(error.code, "SPX-Z212");
}

// ---------------------------------------------------------------------
// The legacy flat scalar typed-effects boundary is untouched: this module
// adds no file under `agent_lifecycle/` and reads, but never edits,
// `agent_interaction_schema` or `live_invocation`. Its own regression
// suites (`cargo test --lib agent_lifecycle`, `--lib
// agent_interaction_schema`, `--lib live_invocation`) are unaffected by
// this module's existence, which this crate's own build (one binary, one
// dependency graph) makes structurally true rather than merely asserted
// here.
// ---------------------------------------------------------------------
