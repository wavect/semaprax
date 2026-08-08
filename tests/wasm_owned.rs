use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::cleanup_plan::{
    execute_for_conformance, CleanupScenario, ContractPhase, ExitContinuation, StatusCase,
    StatusProducer, StatusSourceId,
};
use semaprax::conformance::{NormalizedStatus, OperationOutcome, TraceOutcome, TraceResult};
use semaprax::hir::{DeclarationId, ResolvedFunction, ResolvedProgram};
use semaprax::semantic_trace::build_semantic_event_dictionary;
use semaprax::{hir, parse, wasm};

static NEXT_OUTPUT: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"module test.wasm_owned;

@id("token.type")
resource Token {
    @id("token.drop")
    drop trivial;
}

@id("token.discard")
fn discard(value: own Token) -> i64 { 0 }

@id("token.discard-two")
fn discard_two(first: own Token, second: own Token) -> i64 { 0 }

@id("token.requires")
fn requires_guard(value: own Token, allowed: bool) -> i64
    requires allowed
{
    0
}

@id("token.checked")
fn checked(value: own Token, number: i64) -> i64
    requires number >= 0
{
    number + 1
}

@id("token.identity")
fn identity(value: own Token) -> Token { value }

@id("token.choose-second")
fn choose_second(first: own Token, count: i64, second: own Token) -> Token
    requires count >= 0
{
    second
}

@id("token.ensures-false")
fn ensures_false(value: own Token) -> Token
    ensures false
{
    value
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

struct OutputDirectory {
    path: PathBuf,
}

impl OutputDirectory {
    fn create() -> Self {
        let ordinal = NEXT_OUTPUT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "semaprax-wasm-owned-{}-{ordinal}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        Self { path }
    }
}

impl Drop for OutputDirectory {
    fn drop(&mut self) {
        let Ok(metadata) = std::fs::symlink_metadata(&self.path) else {
            return;
        };
        if metadata.file_type().is_symlink() || metadata.is_file() {
            let _ = std::fs::remove_file(&self.path);
        } else if metadata.is_dir() {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

struct ConformanceCase {
    id: &'static str,
    function: &'static str,
    booleans: BTreeMap<semaprax::hir::ExpressionId, bool>,
    operations: BTreeMap<StatusSourceId, OperationOutcome>,
    result: Option<TraceResult>,
}

impl ConformanceCase {
    fn scenario(&self) -> CleanupScenario {
        let mut scenario = CleanupScenario::new(self.id, self.result.clone());
        scenario.booleans = self.booleans.clone();
        scenario.operations = self.operations.clone();
        scenario
    }
}

fn resolved_function<'a>(program: &'a ResolvedProgram, id: &str) -> &'a ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == id)
        .unwrap()
}

fn contract_source(function: &ResolvedFunction, phase: ContractPhase) -> StatusSourceId {
    function
        .cleanup_plan
        .status_sources
        .iter()
        .find_map(|source| {
            matches!(
                source.producer,
                StatusProducer::ContractFalse {
                    phase: candidate,
                    ..
                } if candidate == phase
            )
            .then(|| source.id.clone())
        })
        .unwrap()
}

fn arithmetic_source(function: &ResolvedFunction) -> StatusSourceId {
    function
        .cleanup_plan
        .status_sources
        .iter()
        .find_map(|source| {
            matches!(source.producer, StatusProducer::CheckedArithmetic { .. })
                .then(|| source.id.clone())
        })
        .unwrap()
}

fn conformance_cases(program: &ResolvedProgram) -> Vec<ConformanceCase> {
    let requires = resolved_function(program, "token.requires");
    let checked = resolved_function(program, "token.checked");
    let choose = resolved_function(program, "token.choose-second");
    let ensures = resolved_function(program, "token.ensures-false");
    let requires_source = contract_source(requires, ContractPhase::Requires);
    let checked_requires = contract_source(checked, ContractPhase::Requires);
    let checked_add = arithmetic_source(checked);
    let choose_requires = contract_source(choose, ContractPhase::Requires);
    let ensures_source = contract_source(ensures, ContractPhase::Ensures);
    let owned_result = || TraceResult::Owned {
        type_id: DeclarationId::new("token.type"),
    };
    let cases = vec![
        ConformanceCase {
            id: "discard-zero",
            function: "token.discard",
            booleans: BTreeMap::new(),
            operations: BTreeMap::new(),
            result: Some(TraceResult::I64(0)),
        },
        ConformanceCase {
            id: "discard-max",
            function: "token.discard",
            booleans: BTreeMap::new(),
            operations: BTreeMap::new(),
            result: Some(TraceResult::I64(0)),
        },
        ConformanceCase {
            id: "discard-two-reverse",
            function: "token.discard-two",
            booleans: BTreeMap::new(),
            operations: BTreeMap::new(),
            result: Some(TraceResult::I64(0)),
        },
        ConformanceCase {
            id: "requires-false",
            function: "token.requires",
            booleans: BTreeMap::from([(requires_source.expression.clone(), false)]),
            operations: BTreeMap::new(),
            result: None,
        },
        ConformanceCase {
            id: "requires-true",
            function: "token.requires",
            booleans: BTreeMap::from([(requires_source.expression.clone(), true)]),
            operations: BTreeMap::new(),
            result: Some(TraceResult::I64(0)),
        },
        ConformanceCase {
            id: "checked-success",
            function: "token.checked",
            booleans: BTreeMap::from([(checked_requires.expression.clone(), true)]),
            operations: BTreeMap::from([(checked_add.clone(), OperationOutcome::Success)]),
            result: Some(TraceResult::I64(42)),
        },
        ConformanceCase {
            id: "checked-add-overflow",
            function: "token.checked",
            booleans: BTreeMap::from([(checked_requires.expression.clone(), true)]),
            operations: BTreeMap::from([(
                checked_add,
                OperationOutcome::Failure(NormalizedStatus::arithmetic(StatusCase::AddOverflow)),
            )]),
            result: None,
        },
        ConformanceCase {
            id: "checked-precondition-false",
            function: "token.checked",
            booleans: BTreeMap::from([(checked_requires.expression, false)]),
            operations: BTreeMap::new(),
            result: None,
        },
        ConformanceCase {
            id: "identity-zero",
            function: "token.identity",
            booleans: BTreeMap::new(),
            operations: BTreeMap::new(),
            result: Some(owned_result()),
        },
        ConformanceCase {
            id: "identity-max",
            function: "token.identity",
            booleans: BTreeMap::new(),
            operations: BTreeMap::new(),
            result: Some(owned_result()),
        },
        ConformanceCase {
            id: "choose-second-zero-max",
            function: "token.choose-second",
            booleans: BTreeMap::from([(choose_requires.expression.clone(), true)]),
            operations: BTreeMap::new(),
            result: Some(owned_result()),
        },
        ConformanceCase {
            id: "choose-second-zero-zero",
            function: "token.choose-second",
            booleans: BTreeMap::from([(choose_requires.expression.clone(), true)]),
            operations: BTreeMap::new(),
            result: Some(owned_result()),
        },
        ConformanceCase {
            id: "choose-second-requires-false",
            function: "token.choose-second",
            booleans: BTreeMap::from([(choose_requires.expression, false)]),
            operations: BTreeMap::new(),
            result: None,
        },
        ConformanceCase {
            id: "ensures-false",
            function: "token.ensures-false",
            booleans: BTreeMap::from([(ensures_source.expression, false)]),
            operations: BTreeMap::new(),
            result: None,
        },
    ];
    assert_eq!(
        cases.iter().map(|case| case.id).collect::<Vec<_>>(),
        semaprax::semantic_trace::OWNED_RESOURCE_CORPUS_V1_SCENARIOS
    );
    cases
}

fn fingerprint_hex(bytes: [u8; 32]) -> String {
    let mut output = String::with_capacity(64);
    for byte in bytes {
        write!(output, "{byte:02x}").unwrap();
    }
    output
}

fn outcome_summary(outcome: &TraceOutcome) -> String {
    match outcome {
        TraceOutcome::Success {
            result: TraceResult::I64(value),
        } => format!("success:i64:{value}"),
        TraceOutcome::Success {
            result: TraceResult::Owned { type_id },
        } => {
            format!("success:owned:{type_id}")
        }
        TraceOutcome::Failure { status, .. } => format!(
            "failure:{}:{}:{}",
            status.domain_id(),
            status.code(),
            match status.class() {
                semaprax::conformance::StatusClass::Contract => "contract",
                semaprax::conformance::StatusClass::Arithmetic => "arithmetic",
                _ => panic!("unexpected first-corpus failure class"),
            }
        ),
        other => panic!("unexpected first-corpus outcome: {other:?}"),
    }
}

#[test]
fn direct_trivial_resource_slice_executes_in_real_node_wasm() {
    let program = parse(SOURCE, Path::new("wasm-owned.spx")).unwrap();
    let first_bytes = wasm::emit_module(&program).unwrap();
    let second_bytes = wasm::emit_module(&program).unwrap();
    assert_eq!(first_bytes, second_bytes);
    let output = OutputDirectory::create();
    wasm::build_web(&program, &output.path).unwrap();

    let manifest = std::fs::read_to_string(output.path.join("semaprax.manifest.json")).unwrap();
    assert!(manifest.contains("\"schema\":\"semaprax.web.v3\""));
    assert!(manifest.contains("\"schema\":\"semaprax.wasm-owned.v1\""));
    assert!(manifest.contains("\"resource\":\"token.type\""));
    assert!(manifest.contains("\"lifecycle\":\"token.drop\""));
    for (function, export) in [
        ("token.discard", "semaprax_owned_0"),
        ("token.discard-two", "semaprax_owned_1"),
        ("token.requires", "semaprax_owned_2"),
        ("token.checked", "semaprax_owned_3"),
        ("token.identity", "semaprax_owned_4"),
        ("token.choose-second", "semaprax_owned_5"),
        ("token.ensures-false", "semaprax_owned_6"),
    ] {
        assert!(manifest.contains(&format!("\"function\":\"{function}\"")));
        assert!(manifest.contains(&format!("\"export\":\"{export}\"")));
    }

    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let script = output.path.join("verify-owned.mjs");
    std::fs::write(&script, NODE_TEST).unwrap();
    let result = Command::new("node")
        .arg(&script)
        .arg(&output.path)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "node failed: {}\n{}",
        String::from_utf8_lossy(&result.stderr),
        String::from_utf8_lossy(&result.stdout),
    );
    assert_eq!(
        String::from_utf8_lossy(&result.stdout).trim(),
        "wasm-owned-ok"
    );

    let poisoned_script = output.path.join("verify-poisoned-allocator.mjs");
    std::fs::write(&poisoned_script, POISONED_ALLOCATOR_TEST).unwrap();
    for mode in ["invalid", "repeated"] {
        let poisoned = Command::new("node")
            .arg(&poisoned_script)
            .arg(&output.path)
            .arg(mode)
            .output()
            .unwrap();
        assert!(
            poisoned.status.success(),
            "node poisoned allocator {mode} failed: {}\n{}",
            String::from_utf8_lossy(&poisoned.stderr),
            String::from_utf8_lossy(&poisoned.stdout),
        );
        assert_eq!(
            String::from_utf8_lossy(&poisoned.stdout).trim(),
            format!("poisoned-allocator-{mode}-ok")
        );
    }
}

#[test]
fn fourteen_case_wasm_ordinals_materialize_to_the_exact_reference_trace() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let parsed = parse(SOURCE, Path::new("wasm-owned-conformance.spx")).unwrap();
    let resolved = hir::resolve(&parsed).unwrap();
    let cases = conformance_cases(&resolved);
    assert_eq!(cases.len(), 14);
    let output = OutputDirectory::create();
    wasm::build_web(&parsed, &output.path).unwrap();
    let script = output.path.join("wasm-conformance.mjs");
    std::fs::write(&script, NODE_CONFORMANCE).unwrap();
    let execution = Command::new("node")
        .arg(&script)
        .arg(&output.path)
        .output()
        .unwrap();
    assert!(
        execution.status.success(),
        "node conformance failed: {}\n{}",
        String::from_utf8_lossy(&execution.stderr),
        String::from_utf8_lossy(&execution.stdout),
    );
    let rows = String::from_utf8(execution.stdout).unwrap();
    let parsed_rows = rows
        .lines()
        .map(|line| {
            let fields = line.split('\t').collect::<Vec<_>>();
            assert_eq!(fields.len(), 5, "malformed Wasm conformance row: {line}");
            (fields[0].to_owned(), fields)
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(parsed_rows.len(), cases.len());

    for case in cases {
        let reference = execute_for_conformance(
            &resolved,
            &DeclarationId::new(case.function),
            case.scenario(),
        )
        .unwrap();
        let dictionary =
            build_semantic_event_dictionary(&resolved, &DeclarationId::new(case.function)).unwrap();
        let fields = &parsed_rows[case.id];
        assert_eq!(fields[1], case.function);
        assert_eq!(fields[2], fingerprint_hex(dictionary.fingerprint()));
        let ordinals = if fields[3].is_empty() {
            Vec::new()
        } else {
            fields[3]
                .split(',')
                .map(|ordinal| ordinal.parse::<u32>().unwrap())
                .collect::<Vec<_>>()
        };
        assert_eq!(fields[4], outcome_summary(&reference.outcome));
        let materialized = dictionary
            .materialize_trace(case.id, &ordinals, reference.outcome.clone())
            .unwrap();
        assert_eq!(
            materialized, reference,
            "Wasm/reference mismatch: {}",
            case.id
        );
        assert_eq!(materialized.to_json(), reference.to_json());
    }
}

#[test]
fn hostile_cleanup_plan_is_rejected_before_owned_admission() {
    let parsed = parse(SOURCE, Path::new("wasm-owned-hostile-plan.spx")).unwrap();
    let mut resolved = hir::resolve(&parsed).unwrap();
    let discard_two = resolved
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "token.discard-two")
        .unwrap();
    let success = discard_two
        .cleanup_plan
        .exits
        .iter_mut()
        .find(|exit| matches!(&exit.continuation, ExitContinuation::CommitResult { .. }))
        .unwrap();
    success.finalize_in_order.reverse();

    assert_eq!(
        wasm::emit_resolved_module(&resolved).unwrap_err().code,
        "SPX-H006"
    );
}

#[test]
fn excluded_resource_shapes_keep_stable_w111_gate() {
    let multiple_resources = parse(
        r#"module test.multiple_resources;
        @id("first.type")
        resource First { @id("first.drop") drop trivial; }
        @id("second.type")
        resource Second { @id("second.drop") drop trivial; }
        @id("first.discard")
        fn discard_first(value: own First) -> i64 { 0 }
        @id("second.discard")
        fn discard_second(value: own Second) -> i64 { 0 }
        @id("app.main")
        fn main() -> i64 { 0 }
        "#,
        Path::new("wasm-multiple-resource-gate.spx"),
    )
    .unwrap();
    assert_eq!(
        wasm::emit_module(&multiple_resources).unwrap_err().code,
        "SPX-W111"
    );

    let imported = parse(
        r#"module test.imported;
        @id("file.type")
        resource File {
            @id("file.drop")
            drop import "file.drop.import";
        }
        @id("file.host")
        interface Host permits { io.release } {
            @id("file.drop.import")
            import fn drop_file(value: own File) -> unit
                effects { io.release }
                failure infallible
                consumes value always;
        }
        @id("app.main")
        fn main() -> i64 { 0 }
        "#,
        Path::new("wasm-imported-gate.spx"),
    )
    .unwrap();
    assert_eq!(wasm::emit_module(&imported).unwrap_err().code, "SPX-W111");

    let borrowed = parse(
        r#"module test.borrowed;
        @id("token.type")
        resource Token { @id("token.drop") drop trivial; }
        @id("token.observe")
        fn observe(value: borrow Token) -> i64 { 0 }
        @id("app.main")
        fn main() -> i64 { 0 }
        "#,
        Path::new("wasm-borrowed-gate.spx"),
    )
    .unwrap();
    assert_eq!(wasm::emit_module(&borrowed).unwrap_err().code, "SPX-W111");
}

const NODE_CONFORMANCE: &str = r#"import { readFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";
import { join } from "node:path";

const directory = process.argv[2];
const runtime = await import(pathToFileURL(join(directory, "semaprax.js")));
const bytes = await readFile(join(directory, "app.wasm"));
const { owned } = await runtime.instantiateBytes(bytes);
const adopt = value => owned.adopt(owned.prepareTrustedAdoption(value));
const rows = [];
const run = (id, exportName, args, resultKind) => {
  const result = owned.invoke(exportName, args, resultKind);
  let outcome;
  if (!result.ok) {
    outcome = `failure:${result.status.domain_id}:${result.status.code}:${result.status.class}`;
  } else if (resultKind === "i64") {
    outcome = `success:i64:${result.value}`;
  } else {
    outcome = "success:owned:token.type";
  }
  rows.push([
    id,
    result.semantic.function,
    result.semantic.dictionary_fingerprint,
    result.semantic.ordinals.join(","),
    outcome,
  ].join("\t"));
  if (result.ok && resultKind === "resource") owned.dispose(result.value);
};

run("discard-zero", "semaprax_owned_0", [adopt(0n)], "i64");
run("discard-max", "semaprax_owned_0", [adopt(18446744073709551615n)], "i64");
run("discard-two-reverse", "semaprax_owned_1", [adopt("first"), adopt("second")], "i64");
run("requires-false", "semaprax_owned_2", [adopt("requires-false"), 0], "i64");
run("requires-true", "semaprax_owned_2", [adopt("requires-true"), 1], "i64");
run("checked-success", "semaprax_owned_3", [adopt("checked-success"), 41n], "i64");
run("checked-add-overflow", "semaprax_owned_3", [adopt("checked-overflow"), 9223372036854775807n], "i64");
run("checked-precondition-false", "semaprax_owned_3", [adopt("checked-precondition"), -1n], "i64");
run("identity-zero", "semaprax_owned_4", [adopt(0n)], "resource");
run("identity-max", "semaprax_owned_4", [adopt(18446744073709551615n)], "resource");
run("choose-second-zero-max", "semaprax_owned_5", [adopt(0n), 17n, adopt(18446744073709551615n)], "resource");
run("choose-second-zero-zero", "semaprax_owned_5", [adopt(0n), 17n, adopt(0n)], "resource");
run("choose-second-requires-false", "semaprax_owned_5", [adopt("first"), -1n, adopt("second")], "resource");
run("ensures-false", "semaprax_owned_6", [adopt("ensures")], "resource");

if (owned.liveHandleCount() !== 0) throw new Error("Wasm conformance leaked an owned handle");
console.log(rows.join("\n"));
"#;

const NODE_TEST: &str = r#"import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";
import { join } from "node:path";

const directory = process.argv[2];
const runtime = await import(pathToFileURL(join(directory, "semaprax.js")));
const bytes = await readFile(join(directory, "app.wasm"));
const tampered = new Uint8Array(bytes);
tampered[tampered.length - 1] ^= 1;
await assert.rejects(
  runtime.instantiateBytes(tampered),
  /WebAssembly artifact authentication failed/,
);
const first = await runtime.instantiateBytes(bytes);
const owned = first.owned;
const adoptWith = (ownerRuntime, value) =>
  ownerRuntime.adopt(ownerRuntime.prepareTrustedAdoption(value));
const adopt = value => adoptWith(owned, value);
const expectedStatus = (domain_id, code, statusClass) => ({
  schema: "semaprax.status.v1",
  domain_id,
  code,
  class: statusClass,
  retryable: false,
});
const contextFor = handle => ((handle & 0x7ff00000) | 0x5350) | 0;
assert.equal("imports" in owned, false);
assert.equal("linkImports" in owned, false);
assert.equal("bind" in owned, false);

let handle;
const oneShot = owned.prepareTrustedAdoption("one-shot");
handle = owned.adopt(oneShot);
assert.throws(() => owned.adopt(oneShot), /already consumed/);
assert.equal(owned.invoke("semaprax_owned_0", [handle], "i64").ok, true);

const equalPayloadFirst = Object.freeze({ value: 7 });
const equalPayloadSecond = Object.freeze({ value: 7 });
const equalFirst = owned.adopt(owned.prepareTrustedAdoption(equalPayloadFirst));
const equalSecond = owned.adopt(owned.prepareTrustedAdoption(equalPayloadSecond));
assert.notEqual(equalFirst, equalSecond);
assert.equal(owned.invoke("semaprax_owned_1", [equalFirst, equalSecond], "i64").ok, true);

handle = adopt({ label: "discard" });
let result = owned.invoke("semaprax_owned_0", [handle], "i64");
assert.deepEqual({ ok: result.ok, published: result.published, value: result.value },
  { ok: true, published: true, value: 0n });
assert.equal(result.semantic.schema, "semaprax.semantic-event-dictionary.v1");
assert.equal(result.semantic.function, "token.discard");
assert.match(result.semantic.dictionary_fingerprint, /^[0-9a-f]{64}$/);
assert.deepEqual(result.semantic.ordinals, [1, 2, 3]);
assert.equal(owned.liveHandleCount(), 0);

const reverseStart = owned.trace().length;
const firstHandle = adopt("first");
const secondHandle = adopt("second");
result = owned.invoke("semaprax_owned_1", [firstHandle, secondHandle], "i64");
assert.equal(result.ok, true);
assert.deepEqual(
  owned.trace().slice(reverseStart).filter(event => event.kind === "drop").map(event => event.handle),
  [secondHandle, firstHandle],
);

handle = adopt("requires-false");
const requiresFailureStart = owned.trace().length;
result = owned.invoke("semaprax_owned_2", [handle, 0], "i64");
assert.equal(result.ok, false);
assert.equal(result.published, false);
assert.deepEqual(result.status, expectedStatus("semaprax.contract.v1", 1, "contract"));
assert.equal(result.semantic.function, "token.requires");
assert.deepEqual(result.semantic.ordinals, [1, 2, 3]);
assert.equal(JSON.stringify(result.status),
  '{"schema":"semaprax.status.v1","domain_id":"semaprax.contract.v1","code":1,"class":"contract","retryable":false}');
assert.equal(owned.liveHandleCount(), 0);
assert.deepEqual(
  owned.trace().slice(requiresFailureStart).map(event => event.kind),
  ["commit", "status", "drop"],
);

handle = adopt("requires-true");
result = owned.invoke("semaprax_owned_2", [handle, 1], "i64");
assert.equal(result.ok, true);

handle = adopt("checked-success");
result = owned.invoke("semaprax_owned_3", [handle, 41n], "i64");
assert.equal(result.value, 42n);
handle = adopt("checked-overflow");
const checkedFailureStart = owned.trace().length;
result = owned.invoke("semaprax_owned_3", [handle, 9223372036854775807n], "i64");
assert.equal(result.ok, false);
assert.deepEqual(result.status, expectedStatus("semaprax.arithmetic.v1", 1, "arithmetic"));
assert.equal(owned.liveHandleCount(), 0);
assert.deepEqual(
  owned.trace().slice(checkedFailureStart).map(event => event.kind),
  ["commit", "status", "drop"],
);

const original = adopt("identity");
result = owned.invoke("semaprax_owned_4", [original], "resource");
assert.equal(result.ok, true);
assert.equal(result.semantic.function, "token.identity");
assert.deepEqual(result.semantic.ordinals, [1, 2, 3]);
const rotated = result.value;
assert.notEqual(rotated, original);
assert.equal(owned.liveHandleCount(), 1);
const replay = owned.invoke("semaprax_owned_0", [original], "i64");
assert.equal(replay.ok, false);
assert.deepEqual(replay.status, expectedStatus("semaprax.wasm-adapter.v1", 3, "adapter"));
assert.equal(owned.liveHandleCount(), 1);
assert.equal(owned.invoke("semaprax_owned_0", [rotated], "i64").ok, true);

const chosenFirst = adopt(Object.freeze({ same: true }));
const chosenSecond = adopt(Object.freeze({ same: true }));
const chooseTraceStart = owned.trace().length;
result = owned.invoke("semaprax_owned_5", [chosenFirst, 0n, chosenSecond], "resource");
assert.equal(result.ok, true);
assert.notEqual(result.value, chosenSecond);
assert.equal(
  owned.trace().slice(chooseTraceStart).find(event => event.kind === "publish").from,
  chosenSecond,
);
assert.equal(owned.liveHandleCount(), 1);
assert.equal(owned.invoke("semaprax_owned_0", [result.value], "i64").ok, true);

handle = adopt("ensures-false");
const ensuresFailureStart = owned.trace().length;
result = owned.invoke("semaprax_owned_6", [handle], "resource");
assert.equal(result.ok, false);
assert.deepEqual(result.status, expectedStatus("semaprax.contract.v1", 2, "contract"));
assert.equal(result.semantic.function, "token.ensures-false");
assert.deepEqual(result.semantic.ordinals, [1, 2, 3, 4, 5]);
assert.equal(owned.liveHandleCount(), 0);
assert.deepEqual(
  owned.trace().slice(ensuresFailureStart).map(event => event.kind),
  ["commit", "status", "drop"],
);

result = owned.invoke("semaprax_owned_0", [0x7fffffff], "i64");
assert.equal(result.ok, false);
assert.deepEqual(result.semantic.ordinals, []);
assert.deepEqual(result.status, expectedStatus("semaprax.wasm-adapter.v1", 3, "adapter"));
assert.equal(owned.liveHandleCount(), 0);

handle = adopt("duplicate");
result = owned.invoke("semaprax_owned_1", [handle, handle], "i64");
assert.equal(result.ok, false);
assert.equal(owned.liveHandleCount(), 1);
assert.equal(owned.invoke("semaprax_owned_0", [handle], "i64").ok, true);

const second = await runtime.instantiateBytes(bytes);
const crossInstance = adopt("cross-instance");
const localSecond = adoptWith(second.owned, "second-instance");
assert.notEqual(crossInstance, localSecond);
result = second.owned.invoke("semaprax_owned_0", [crossInstance], "i64");
assert.equal(result.ok, false);
assert.equal(second.owned.liveHandleCount(), 1);
assert.equal(second.owned.invoke("semaprax_owned_0", [localSecond], "i64").ok, true);
assert.equal(owned.invoke("semaprax_owned_0", [crossInstance], "i64").ok, true);

handle = adopt("result-kind-confusion");
assert.throws(
  () => owned.invoke("semaprax_owned_4", [handle], "i64"),
  /requires result kind resource/,
);
assert.throws(() => owned.invoke("semaprax_main", [], "i64"), /unknown SEMAPRAX owned export/);
assert.equal(owned.liveHandleCount(), 1);
result = owned.invoke("semaprax_owned_4", [handle], "resource");
assert.equal(owned.invoke("semaprax_owned_0", [result.value], "i64").ok, true);

handle = adopt("canonical-resource-argument");
for (const forged of [handle + 0x100000000, handle - 0x100000000, Number.MAX_SAFE_INTEGER, -1, 0]) {
  assert.throws(
    () => owned.invoke("semaprax_owned_0", [forged], "i64"),
    /argument 0 kind mismatch/,
  );
  assert.equal(owned.liveHandleCount(), 1);
}
assert.throws(
  () => owned.invoke("semaprax_owned_0", [handle, handle], "i64"),
  /argument count mismatch/,
);
assert.throws(
  () => owned.invoke("semaprax_owned_0", [BigInt(handle)], "i64"),
  /argument 0 kind mismatch/,
);
assert.equal(owned.invoke("semaprax_owned_0", [handle], "i64").ok, true);

const snapshottedHandle = adopt("snapshotted-argument");
const iteratorTarget = adopt("iterator-target");
const statefulArgs = [];
Object.defineProperty(statefulArgs, "0", {
  get: () => snapshottedHandle,
  enumerable: true,
});
statefulArgs.length = 1;
statefulArgs[Symbol.iterator] = function* hostileIterator() {
  yield iteratorTarget + 0x100000000;
};
assert.equal(owned.invoke("semaprax_owned_0", statefulArgs, "i64").ok, true);
assert.equal(owned.liveHandleCount(), 1);
assert.equal(owned.invoke("semaprax_owned_0", [iteratorTarget], "i64").ok, true);

handle = adopt("stateful-export-name");
let exportCoercions = 0;
const statefulExportName = {
  [Symbol.toPrimitive]() {
    exportCoercions += 1;
    return exportCoercions === 1 ? "semaprax_owned_0" : "semaprax_owned_4";
  },
};
assert.throws(
  () => owned.invoke(statefulExportName, [handle], "i64"),
  /export name must be a string/,
);
assert.equal(exportCoercions, 0);
assert.equal(owned.liveHandleCount(), 1);
assert.equal(owned.invoke("semaprax_owned_0", [handle], "i64").ok, true);

for (const invalidI64 of [9223372036854775808n, -9223372036854775809n]) {
  handle = adopt(`invalid-i64-${invalidI64}`);
  assert.throws(
    () => owned.invoke("semaprax_owned_3", [handle, invalidI64], "i64"),
    /argument 1 kind mismatch/,
  );
  assert.equal(owned.liveHandleCount(), 1);
  owned.dispose(handle);
}
handle = adopt("invalid-bool");
assert.throws(
  () => owned.invoke("semaprax_owned_2", [handle, 2], "i64"),
  /argument 1 kind mismatch/,
);
owned.dispose(handle);

for (const pointer of [1, 65536, -8]) {
  handle = adopt(`bad-out-${pointer}`);
  const view = new DataView(first.instance.exports.memory.buffer);
  view.setBigInt64(0, 0x5a5a5a5a5a5a5a5an, true);
  const before = new Uint8Array(first.instance.exports.memory.buffer).slice();
  const token = first.instance.exports.semaprax_owned_0(contextFor(handle), handle, pointer);
  assert.deepEqual(
    owned.resolveStatus(token),
    expectedStatus("semaprax.wasm-adapter.v1", 6, "adapter"),
  );
  assert.deepEqual(new Uint8Array(first.instance.exports.memory.buffer), before);
  assert.equal(view.getBigInt64(0, true), 0x5a5a5a5a5a5a5a5an);
  assert.equal(owned.liveHandleCount(), 1);
  assert.equal(owned.invoke("semaprax_owned_0", [handle], "i64").ok, true);
}

for (const pointer of [2, 65536, -4]) {
  handle = adopt(`bad-resource-out-${pointer}`);
  const view = new DataView(first.instance.exports.memory.buffer);
  view.setInt32(0, 0x5a5a5a5a, true);
  const before = new Uint8Array(first.instance.exports.memory.buffer).slice();
  const token = first.instance.exports.semaprax_owned_4(contextFor(handle), handle, pointer);
  assert.deepEqual(
    owned.resolveStatus(token),
    expectedStatus("semaprax.wasm-adapter.v1", 6, "adapter"),
  );
  assert.deepEqual(new Uint8Array(first.instance.exports.memory.buffer), before);
  assert.equal(view.getInt32(0, true), 0x5a5a5a5a);
  assert.equal(owned.liveHandleCount(), 1);
  result = owned.invoke("semaprax_owned_4", [handle], "resource");
  assert.equal(owned.invoke("semaprax_owned_0", [result.value], "i64").ok, true);
}

const slotLimited = await runtime.instantiateBytes(bytes, { maxOwnedSlots: 1 });
const occupiedTicket = slotLimited.owned.prepareTrustedAdoption("occupied");
const retryTicket = slotLimited.owned.prepareTrustedAdoption("retry-after-capacity");
const occupiedHandle = slotLimited.owned.adopt(occupiedTicket);
assert.throws(() => slotLimited.owned.adopt(retryTicket), /handle table exhausted/);
slotLimited.owned.dispose(occupiedHandle);
const retriedHandle = slotLimited.owned.adopt(retryTicket);
assert.equal(slotLimited.owned.invoke("semaprax_owned_0", [retriedHandle], "i64").ok, true);

const resultLimited = await runtime.instantiateBytes(bytes, { maxOwnedSlots: 1 });
const resultLimitedHandle = adoptWith(resultLimited.owned, "result-capacity");
result = resultLimited.owned.invoke("semaprax_owned_4", [resultLimitedHandle], "resource");
assert.deepEqual(result.status, expectedStatus("semaprax.wasm-adapter.v1", 5, "adapter"));
assert.equal(resultLimited.owned.liveHandleCount(), 1);
resultLimited.owned.dispose(resultLimitedHandle);

const finalStatus = await runtime.instantiateBytes(bytes, { maxStatusTokens: 1 });
const finalHandle = adoptWith(finalStatus.owned, "final-dynamic-status");
result = finalStatus.owned.invoke("semaprax_owned_1", [finalHandle, finalHandle], "i64");
assert.deepEqual(result.status, expectedStatus("semaprax.wasm-adapter.v1", 4, "adapter"));
assert.equal(finalStatus.owned.liveHandleCount(), 1);
result = finalStatus.owned.invoke("semaprax_owned_0", [finalHandle], "i64");
assert.deepEqual(result.status, expectedStatus("semaprax.wasm-adapter.v1", 5, "adapter"));
assert.equal(finalStatus.owned.liveHandleCount(), 1);
assert.deepEqual(
  finalStatus.owned.invoke("semaprax_owned_0", [finalHandle], "i64").status,
  expectedStatus("semaprax.wasm-adapter.v1", 5, "adapter"),
);
finalStatus.owned.dispose(finalHandle);

const successAtLimit = await runtime.instantiateBytes(bytes, { maxStatusTokens: 1 });
const successAtLimitHandle = adoptWith(successAtLimit.owned, "success-at-limit");
assert.equal(successAtLimit.owned.invoke("semaprax_owned_0", [successAtLimitHandle], "i64").ok, true);

const copiedRuntime = await import(`${pathToFileURL(join(directory, "semaprax.js")).href}?copy=1`);
const originalFresh = await runtime.instantiateBytes(bytes);
const copiedFresh = await copiedRuntime.instantiateBytes(bytes);
const originalFreshHandle = adoptWith(originalFresh.owned, "original-module");
const copiedFreshHandle = adoptWith(copiedFresh.owned, "copied-module");
assert.notEqual(originalFreshHandle, copiedFreshHandle);
result = copiedFresh.owned.invoke("semaprax_owned_0", [originalFreshHandle], "i64");
assert.deepEqual(result.status, expectedStatus("semaprax.wasm-adapter.v1", 3, "adapter"));
assert.equal(copiedFresh.owned.liveHandleCount(), 1);
assert.equal(copiedFresh.owned.invoke("semaprax_owned_0", [copiedFreshHandle], "i64").ok, true);
assert.equal(originalFresh.owned.invoke("semaprax_owned_0", [originalFreshHandle], "i64").ok, true);

const commits = owned.trace().filter(event => event.kind === "commit");
const drops = owned.trace().filter(event => event.kind === "drop");
assert.ok(commits.length > 0);
assert.ok(drops.length > 0);
console.log("wasm-owned-ok");
"#;

const POISONED_ALLOCATOR_TEST: &str = r#"import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";
import { join } from "node:path";

const directory = process.argv[2];
const mode = process.argv[3];
const key = Symbol.for("semaprax.wasm-owned.runtime-tags.v1");
Object.defineProperty(globalThis, key, {
  value: Object.freeze({ take: () => mode === "invalid" ? 0 : 1 }),
  configurable: false,
  enumerable: false,
  writable: false,
});
const runtime = await import(`${pathToFileURL(join(directory, "semaprax.js")).href}?poison=${mode}`);
const bytes = await readFile(join(directory, "app.wasm"));
if (mode === "invalid") {
  await assert.rejects(
    runtime.instantiateBytes(bytes),
    /runtime-tag allocator returned an invalid or repeated identity/,
  );
} else {
  await runtime.instantiateBytes(bytes);
  await assert.rejects(
    runtime.instantiateBytes(bytes),
    /runtime-tag allocator returned an invalid or repeated identity/,
  );
}
console.log(`poisoned-allocator-${mode}-ok`);
"#;
