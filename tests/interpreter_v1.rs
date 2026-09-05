//! Executable evidence for Reference Interpreter v1 (`semaprax.interpret.v1`).
//!
//! Proves that direct HIR evaluation EXACTLY matches the native C11 O0/O2
//! and Node/Wasm backend results and statuses for the same inputs, pins
//! golden envelope digests, exercises fuel exhaustion fail-closed behavior,
//! determinism, per-field tamper rejection through independent replay,
//! admission/argument diagnostics, source-drift binding, and CLI exit codes.
//! The interpreter itself never invokes a toolchain; only the parity corpus
//! does.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::interpreter::{
    self, InterpreterOptions, DEFAULT_MAX_STEPS, MAX_CALL_DEPTH, MAX_STEPS_LIMIT, SCHEMA,
};
use semaprax::{codegen, parse, verify, wasm};
use sha2::{Digest as _, Sha256};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const PAYLOAD_DIGEST_DOMAIN: &[u8] = b"semaprax.interpret.payload.v1\0";
const SOURCE_DIGEST_DOMAIN: &[u8] = b"semaprax.interpret.source.v1\0";
const REQUIRE_ENV: &str = "SEMAPRAX_REQUIRE_INTERPRETER_BACKEND_PARITY";

const MEANING_PATH: &str = "examples/meaning.spx";

#[path = "interpreter_v1/verification_and_cli.rs"]
mod verification_and_cli;

/// Backend-parity corpus: every function is explicit-ID, monomorphic,
/// effect-free, and takes only by-value direct `i64`/`bool` parameters with a
/// direct `i64`/`bool` result, while bodies exercise the admitted scalar
/// surface including checked-arithmetic statuses, contracts, Explicit
/// Mutation v1 forms, lazy operators, and evaluation-order probes.
const PARITY_FIXTURE: &str = r#"
module test.interpreter_parity;

@id("case.success.i64")
fn success_i64() -> i64 { 42 }

@id("case.success.bool")
fn success_bool() -> bool { true }

@id("case.requires")
fn requires_false() -> i64 requires false { 11 }

@id("case.ensures")
fn ensures_false() -> i64 ensures result == 12 { 7 }

@id("case.add")
fn add_overflow() -> i64 { 9223372036854775807 + 1 }

@id("case.sub")
fn sub_overflow() -> i64 { -9223372036854775807 - 2 }

@id("case.mul")
fn mul_overflow() -> i64 { 9223372036854775807 * 2 }

@id("case.div.zero")
fn division_by_zero() -> i64 { 7 / 0 }

@id("case.div.overflow")
fn division_overflow() -> i64 { (-9223372036854775807 - 1) / -1 }

@id("case.rem.zero")
fn remainder_by_zero() -> i64 { 7 % 0 }

@id("case.rem.overflow")
fn remainder_overflow() -> i64 { (-9223372036854775807 - 1) % -1 }

@id("case.neg")
fn negation_overflow() -> i64 { -(-9223372036854775807 - 1) }

@id("case.i32.mul.overflow")
fn i32_mul_overflow() -> i64 {
    let wide = 100000i32 * 100000i32;
    if wide > 0i32 { 1 } else { 2 }
}

@id("case.u8.add.overflow")
fn u8_add_overflow() -> i64 {
    let wrapped = 250u8 + 10u8;
    if wrapped > 0u8 { 1 } else { 2 }
}

@id("case.char.order")
fn char_order() -> i64 {
    let low = 'S';
    let high = 'z';
    if low < high { 1 } else { 0 }
}

@id("case.floats.compare")
fn float_compare() -> i64 {
    let wide = 150.0 + 0.25;
    let narrow = 0.5f32 * 3.0f32;
    let inverted = -(wide);
    if wide == 150.25 && narrow == 1.5f32 && inverted < 0.0 { 1 } else { 0 }
}

@id("case.mutate.chain")
fn mutate_chain() -> i64 {
    let mut total = 5;
    total = total * 3;
    let mut small = 4;
    small = small + 1;
    total = total - small;
    total
}

@id("case.mutate.branches")
fn mutate_branches(flag: bool) -> i64 {
    let mut acc = 0;
    let branch = if flag {
        acc = acc + 10;
        1
    } else {
        acc = acc + 20;
        2
    };
    acc = acc + branch + branch;
    acc
}

@id("case.lazy.and")
fn lazy_and() -> i64 {
    if !(false && ((7 / 0) == 0)) { 1 } else { 2 }
}

@id("case.lazy.or")
fn lazy_or() -> i64 {
    if true || ((7 / 0) == 0) { 1 } else { 2 }
}

@id("case.eager.left.fails")
fn eager_left_fails() -> i64 {
    if ((7 / 0) == 0) || true { 1 } else { 2 }
}

@id("case.fail.requires")
fn fail_requires() -> i64 requires false { 0 }

@id("case.fail.div")
fn fail_division() -> i64 { 1 / 0 }

@id("case.pair")
fn pair(left: i64, right: i64) -> i64 { left + right }

@id("case.arg.order")
fn arg_order() -> i64 { pair(fail_requires(), fail_division()) }

@id("case.first")
fn first() -> i64 requires false { 1 }

@id("case.second")
fn second() -> i64 ensures false { 2 }

@id("case.sum")
fn sum(left: i64, right: i64) -> i64 { left + right }

@id("case.nested")
fn nested() -> i64 { sum(first(), second()) }

@id("app.main")
fn main() -> i64 { success_i64() }
"#;

/// One transcript observation: which function to call and how.
struct ParityCase {
    /// Transcript identifier (unique per line).
    id: &'static str,
    /// Selected stable id (differs from `id` only for the argument variants).
    symbol_id: &'static str,
    shape: CaseShape,
}

#[derive(Clone, Copy, PartialEq)]
enum CaseShape {
    I64,
    Bool,
    /// `fn(bool) -> i64` plus the canonical bool argument to pass.
    BoolIn(bool),
}

/// Web-admissible subset: the Public Scalar Export Profile v1 lane requires
/// the whole program to bind only by-value direct `i64`/`bool` scalars, so the
/// Node/Wasm leg runs over this module while native and interpreter legs cover
/// the full scalar surface above.
const WEB_FIXTURE: &str = r#"

module test.interpreter_parity;

@id("case.success.i64")
fn success_i64() -> i64 { 42 }

@id("case.success.bool")
fn success_bool() -> bool { true }

@id("case.requires")
fn requires_false() -> i64 requires false { 11 }

@id("case.ensures")
fn ensures_false() -> i64 ensures result == 12 { 7 }

@id("case.add")
fn add_overflow() -> i64 { 9223372036854775807 + 1 }

@id("case.sub")
fn sub_overflow() -> i64 { -9223372036854775807 - 2 }

@id("case.mul")
fn mul_overflow() -> i64 { 9223372036854775807 * 2 }

@id("case.div.zero")
fn division_by_zero() -> i64 { 7 / 0 }

@id("case.div.overflow")
fn division_overflow() -> i64 { (-9223372036854775807 - 1) / -1 }

@id("case.rem.zero")
fn remainder_by_zero() -> i64 { 7 % 0 }

@id("case.rem.overflow")
fn remainder_overflow() -> i64 { (-9223372036854775807 - 1) % -1 }

@id("case.neg")
fn negation_overflow() -> i64 { -(-9223372036854775807 - 1) }

@id("case.mutate.chain")
fn mutate_chain() -> i64 {
    let mut total = 5;
    total = total * 3;
    let mut small = 4;
    small = small + 1;
    total = total - small;
    total
}

@id("case.mutate.branches")
fn mutate_branches(flag: bool) -> i64 {
    let mut acc = 0;
    let branch = if flag {
        acc = acc + 10;
        1
    } else {
        acc = acc + 20;
        2
    };
    acc = acc + branch + branch;
    acc
}

@id("case.lazy.and")
fn lazy_and() -> i64 {
    if !(false && ((7 / 0) == 0)) { 1 } else { 2 }
}

@id("case.lazy.or")
fn lazy_or() -> i64 {
    if true || ((7 / 0) == 0) { 1 } else { 2 }
}

@id("case.eager.left.fails")
fn eager_left_fails() -> i64 {
    if ((7 / 0) == 0) || true { 1 } else { 2 }
}

@id("case.fail.requires")
fn fail_requires() -> i64 requires false { 0 }

@id("case.fail.div")
fn fail_division() -> i64 { 1 / 0 }

@id("case.pair")
fn pair(left: i64, right: i64) -> i64 { left + right }

@id("case.arg.order")
fn arg_order() -> i64 { pair(fail_requires(), fail_division()) }

@id("case.first")
fn first() -> i64 requires false { 1 }

@id("case.second")
fn second() -> i64 ensures false { 2 }

@id("case.sum")
fn sum(left: i64, right: i64) -> i64 { left + right }

@id("case.nested")
fn nested() -> i64 { sum(first(), second()) }

@id("app.main")
fn main() -> i64 { success_i64() }
"#;

/// Alphabetical transcript order shared by every producer (native probe, Node
/// observer, and interpreter transcript).
const PARITY_CASES: &[ParityCase] = &[
    ParityCase {
        id: "case.add",
        symbol_id: "case.add",
        shape: CaseShape::I64,
    },
    ParityCase {
        id: "case.arg.order",
        symbol_id: "case.arg.order",
        shape: CaseShape::I64,
    },
    ParityCase {
        id: "case.char.order",
        symbol_id: "case.char.order",
        shape: CaseShape::I64,
    },
    ParityCase {
        id: "case.div.overflow",
        symbol_id: "case.div.overflow",
        shape: CaseShape::I64,
    },
    ParityCase {
        id: "case.div.zero",
        symbol_id: "case.div.zero",
        shape: CaseShape::I64,
    },
    ParityCase {
        id: "case.eager.left.fails",
        symbol_id: "case.eager.left.fails",
        shape: CaseShape::I64,
    },
    ParityCase {
        id: "case.ensures",
        symbol_id: "case.ensures",
        shape: CaseShape::I64,
    },
    ParityCase {
        id: "case.fail.div",
        symbol_id: "case.fail.div",
        shape: CaseShape::I64,
    },
    ParityCase {
        id: "case.fail.requires",
        symbol_id: "case.fail.requires",
        shape: CaseShape::I64,
    },
    ParityCase {
        id: "case.first",
        symbol_id: "case.first",
        shape: CaseShape::I64,
    },
    ParityCase {
        id: "case.floats.compare",
        symbol_id: "case.floats.compare",
        shape: CaseShape::I64,
    },
    ParityCase {
        id: "case.i32.mul.overflow",
        symbol_id: "case.i32.mul.overflow",
        shape: CaseShape::I64,
    },
    ParityCase {
        id: "case.lazy.and",
        symbol_id: "case.lazy.and",
        shape: CaseShape::I64,
    },
    ParityCase {
        id: "case.lazy.or",
        symbol_id: "case.lazy.or",
        shape: CaseShape::I64,
    },
    ParityCase {
        id: "case.mul",
        symbol_id: "case.mul",
        shape: CaseShape::I64,
    },
    ParityCase {
        id: "case.mutate.branches.false",
        symbol_id: "case.mutate.branches",
        shape: CaseShape::BoolIn(false),
    },
    ParityCase {
        id: "case.mutate.branches.true",
        symbol_id: "case.mutate.branches",
        shape: CaseShape::BoolIn(true),
    },
    ParityCase {
        id: "case.mutate.chain",
        symbol_id: "case.mutate.chain",
        shape: CaseShape::I64,
    },
    ParityCase {
        id: "case.neg",
        symbol_id: "case.neg",
        shape: CaseShape::I64,
    },
    ParityCase {
        id: "case.nested",
        symbol_id: "case.nested",
        shape: CaseShape::I64,
    },
    ParityCase {
        id: "case.rem.overflow",
        symbol_id: "case.rem.overflow",
        shape: CaseShape::I64,
    },
    ParityCase {
        id: "case.rem.zero",
        symbol_id: "case.rem.zero",
        shape: CaseShape::I64,
    },
    ParityCase {
        id: "case.requires",
        symbol_id: "case.requires",
        shape: CaseShape::I64,
    },
    ParityCase {
        id: "case.second",
        symbol_id: "case.second",
        shape: CaseShape::I64,
    },
    ParityCase {
        id: "case.sub",
        symbol_id: "case.sub",
        shape: CaseShape::I64,
    },
    ParityCase {
        id: "case.success.bool",
        symbol_id: "case.success.bool",
        shape: CaseShape::Bool,
    },
    ParityCase {
        id: "case.success.i64",
        symbol_id: "case.success.i64",
        shape: CaseShape::I64,
    },
    ParityCase {
        id: "case.u8.add.overflow",
        symbol_id: "case.u8.add.overflow",
        shape: CaseShape::I64,
    },
];

/// The exact cross-backend transcript: sticky left-to-right failure
/// selection, compiler-owned status codes, and identical results everywhere.
const EXPECTED_PARITY_TRANSCRIPT: &str = concat!(
    "{\"id\":\"case.add\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.arithmetic.v1\",\"code\":1}}\n",
    "{\"id\":\"case.arg.order\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.contract.v1\",\"code\":1}}\n",
    "{\"id\":\"case.char.order\",\"ok\":true,\"value\":\"1\"}\n",
    "{\"id\":\"case.div.overflow\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.arithmetic.v1\",\"code\":5}}\n",
    "{\"id\":\"case.div.zero\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.arithmetic.v1\",\"code\":4}}\n",
    "{\"id\":\"case.eager.left.fails\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.arithmetic.v1\",\"code\":4}}\n",
    "{\"id\":\"case.ensures\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.contract.v1\",\"code\":2}}\n",
    "{\"id\":\"case.fail.div\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.arithmetic.v1\",\"code\":4}}\n",
    "{\"id\":\"case.fail.requires\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.contract.v1\",\"code\":1}}\n",
    "{\"id\":\"case.first\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.contract.v1\",\"code\":1}}\n",
    "{\"id\":\"case.floats.compare\",\"ok\":true,\"value\":\"1\"}\n",
    "{\"id\":\"case.i32.mul.overflow\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.arithmetic.v1\",\"code\":3}}\n",
    "{\"id\":\"case.lazy.and\",\"ok\":true,\"value\":\"1\"}\n",
    "{\"id\":\"case.lazy.or\",\"ok\":true,\"value\":\"1\"}\n",
    "{\"id\":\"case.mul\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.arithmetic.v1\",\"code\":3}}\n",
    "{\"id\":\"case.mutate.branches.false\",\"ok\":true,\"value\":\"24\"}\n",
    "{\"id\":\"case.mutate.branches.true\",\"ok\":true,\"value\":\"12\"}\n",
    "{\"id\":\"case.mutate.chain\",\"ok\":true,\"value\":\"10\"}\n",
    "{\"id\":\"case.neg\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.arithmetic.v1\",\"code\":8}}\n",
    "{\"id\":\"case.nested\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.contract.v1\",\"code\":1}}\n",
    "{\"id\":\"case.rem.overflow\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.arithmetic.v1\",\"code\":7}}\n",
    "{\"id\":\"case.rem.zero\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.arithmetic.v1\",\"code\":6}}\n",
    "{\"id\":\"case.requires\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.contract.v1\",\"code\":1}}\n",
    "{\"id\":\"case.second\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.contract.v1\",\"code\":2}}\n",
    "{\"id\":\"case.sub\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.arithmetic.v1\",\"code\":2}}\n",
    "{\"id\":\"case.success.bool\",\"ok\":true,\"value\":true}\n",
    "{\"id\":\"case.success.i64\",\"ok\":true,\"value\":\"42\"}\n",
    "{\"id\":\"case.u8.add.overflow\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.arithmetic.v1\",\"code\":1}}\n",
);

fn owned_args(arguments: &[&str]) -> Vec<String> {
    arguments
        .iter()
        .map(|argument| (*argument).to_owned())
        .collect()
}

fn write_temp(source: &str) -> PathBuf {
    let ordinal = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-interpreter-v1-{}-{ordinal}.spx",
        std::process::id()
    ));
    std::fs::write(&path, source).unwrap();
    path
}

fn cleanup(path: &Path) {
    let _ = std::fs::remove_file(path);
}

fn cli(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args(args)
        .output()
        .expect("semaprax binary");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    )
}

fn payload_digest(payload: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(PAYLOAD_DIGEST_DOMAIN);
    hasher.update((payload.len() as u64).to_le_bytes());
    hasher.update(payload.as_bytes());
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    )
}

fn source_digest_hex(source: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(SOURCE_DIGEST_DOMAIN);
    hasher.update((source.len() as u64).to_le_bytes());
    hasher.update(source.as_bytes());
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    )
}

/// Re-mints the outer digest around `tampered_envelope`'s exact payload bytes
/// so replay must rely on its derivation rules rather than the digest alone.
fn remint_digest(tampered_envelope: &str) -> String {
    let payload_key = "\"payload\":";
    let payload_offset = tampered_envelope
        .find(payload_key)
        .expect("tampered envelope keeps its payload member")
        + payload_key.len();
    let payload = &tampered_envelope[payload_offset..tampered_envelope.len() - 1];
    let (prefix, _) = tampered_envelope
        .split_once("\"digest\":")
        .expect("digest member");
    format!(
        "{prefix}\"digest\":{},\"bytes\":{},\"payload\":{}}}",
        semaprax::diagnostic::quote_json(&payload_digest(payload)),
        payload.len(),
        payload
    )
}

fn interpret_case(
    source_path: &Path,
    token: &str,
    arguments: &[&str],
) -> Result<String, Vec<semaprax::diagnostic::Diagnostic>> {
    let owned: Vec<String> = arguments
        .iter()
        .map(|argument| (*argument).to_owned())
        .collect();
    interpreter::interpret(source_path, token, &owned, &InterpreterOptions::default())
        .map(|interpretation| interpretation.envelope)
}

/// One transcript line per interpretation, byte-compatible with both backend
/// producers: quoted decimal `i64` values, bare booleans, and schema/domain/
/// code triples for normalized failure statuses.
fn interpreter_transcript_line(envelope: &str, id: &str) -> String {
    let value: serde_json::Value = serde_json::from_str(envelope).expect("envelope JSON");
    interpreter_transcript_line_for_value(&value, id)
}

fn interpreter_transcript_line_for_value(value: &serde_json::Value, id: &str) -> String {
    let outcome = &value["payload"]["outcome"];
    match outcome["kind"].as_str().expect("outcome kind") {
        "returned" => {
            let rendered = match outcome["type"].as_str().expect("outcome type") {
                "i64" => format!("\"{}\"", outcome["value"].as_str().unwrap()),
                "bool" => outcome["value"].as_str().unwrap().to_owned(),
                other => panic!("unexpected boundary type {other}"),
            };
            format!("{{\"id\":\"{id}\",\"ok\":true,\"value\":{rendered}}}")
        }
        "failed" => {
            let status = &outcome["status"];
            format!(
                "{{\"id\":\"{id}\",\"ok\":false,\"status\":{{\"schema\":\"{}\",\
\"domain_id\":\"{}\",\"code\":{}}}}}",
                status["schema"].as_str().unwrap(),
                status["domain_id"].as_str().unwrap(),
                status["code"].as_u64().unwrap(),
            )
        }
        other => panic!("parity corpus cannot exhaust capacity, found {other}"),
    }
}

fn interpreter_transcript(source_path: &Path) -> String {
    PARITY_CASES
        .iter()
        .map(|parity_case| {
            let arguments: Vec<&str> = match parity_case.shape {
                CaseShape::BoolIn(false) => vec!["false"],
                CaseShape::BoolIn(true) => vec!["true"],
                _ => Vec::new(),
            };
            let envelope = interpret_case(source_path, parity_case.symbol_id, &arguments)
                .unwrap_or_else(|errors| {
                    panic!("interpret `{}` failed: {errors:?}", parity_case.symbol_id)
                });
            interpreter_transcript_line(&envelope, parity_case.id)
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn tool_available(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn required() -> bool {
    std::env::var_os(REQUIRE_ENV).is_some()
}

fn require_tools_or_skip() -> bool {
    let missing = ["clang", "node"]
        .into_iter()
        .filter(|tool| !tool_available(tool))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return true;
    }
    assert!(
        !required(),
        "{REQUIRE_ENV} requires clang and Node; missing {}",
        missing.join(", ")
    );
    false
}

fn c_symbol(stable_id: &str) -> String {
    let mut symbol = String::from("spx_decl_");
    for byte in stable_id.bytes() {
        symbol.push_str(&format!("{byte:02x}"));
    }
    symbol
}

/// Generates the native probe that prints one transcript line per case in
/// shared order, handling both success and failure outcomes generically.
fn native_probe() -> String {
    let mut source = r#"
typedef spx_status_token (*spx_i64_case)(struct spx_context *, int64_t *);
typedef spx_status_token (*spx_bool_case)(struct spx_context *, bool *);
typedef spx_status_token (*spx_bool_in_i64_case)(struct spx_context *, bool, int64_t *);

static int spx_emit_i64(const char *id, spx_i64_case test_case) {
    struct spx_status_entry records[UINT32_C(8)];
    struct spx_context context = {0};
    if (!spx_context_init(&context, UINT64_C(401), records, UINT32_C(8), NULL, NULL, NULL)) return 10;
    int64_t value = INT64_C(-7777);
    spx_status_token token = test_case(&context, &value);
    if (token == SPX_STATUS_SUCCESS) {
        printf("{\"id\":\"%s\",\"ok\":true,\"value\":\"%lld\"}\n", id, (long long)value);
        return 0;
    }
    const struct spx_normalized_status *status = spx_status_resolve(&context, token);
    if (status == NULL || context.status_arena.length != UINT32_C(1)) return 11;
    printf(
        "{\"id\":\"%s\",\"ok\":false,\"status\":{\"schema\":\"%s\",\"domain_id\":\"%s\",\"code\":%u}}\n",
        id, status->schema, status->domain_id, (unsigned int)status->code
    );
    return 0;
}

static int spx_emit_bool(const char *id, spx_bool_case test_case) {
    struct spx_status_entry records[UINT32_C(8)];
    struct spx_context context = {0};
    if (!spx_context_init(&context, UINT64_C(402), records, UINT32_C(8), NULL, NULL, NULL)) return 20;
    bool value = false;
    spx_status_token token = test_case(&context, &value);
    if (token == SPX_STATUS_SUCCESS) {
        printf("{\"id\":\"%s\",\"ok\":true,\"value\":%s}\n", id, value ? "true" : "false");
        return 0;
    }
    const struct spx_normalized_status *status = spx_status_resolve(&context, token);
    if (status == NULL || context.status_arena.length != UINT32_C(1)) return 21;
    printf(
        "{\"id\":\"%s\",\"ok\":false,\"status\":{\"schema\":\"%s\",\"domain_id\":\"%s\",\"code\":%u}}\n",
        id, status->schema, status->domain_id, (unsigned int)status->code
    );
    return 0;
}

static int spx_emit_bool_in_i64(const char *id, spx_bool_in_i64_case test_case, bool argument) {
    struct spx_status_entry records[UINT32_C(8)];
    struct spx_context context = {0};
    if (!spx_context_init(&context, UINT64_C(403), records, UINT32_C(8), NULL, NULL, NULL)) return 30;
    int64_t value = INT64_C(-7777);
    spx_status_token token = test_case(&context, argument, &value);
    if (token == SPX_STATUS_SUCCESS) {
        printf("{\"id\":\"%s\",\"ok\":true,\"value\":\"%lld\"}\n", id, (long long)value);
        return 0;
    }
    const struct spx_normalized_status *status = spx_status_resolve(&context, token);
    if (status == NULL || context.status_arena.length != UINT32_C(1)) return 31;
    printf(
        "{\"id\":\"%s\",\"ok\":false,\"status\":{\"schema\":\"%s\",\"domain_id\":\"%s\",\"code\":%u}}\n",
        id, status->schema, status->domain_id, (unsigned int)status->code
    );
    return 0;
}

int main(void) {
"#
    .to_owned();
    for (index, parity_case) in PARITY_CASES.iter().enumerate() {
        let symbol = c_symbol(parity_case.symbol_id);
        match parity_case.shape {
            CaseShape::I64 => {
                source.push_str(&format!(
                    "    if (spx_emit_i64(\"{}\", {}) != 0) return {};\n",
                    parity_case.id,
                    symbol,
                    100 + index
                ));
            }
            CaseShape::Bool => {
                source.push_str(&format!(
                    "    if (spx_emit_bool(\"{}\", {}) != 0) return {};\n",
                    parity_case.id,
                    symbol,
                    100 + index
                ));
            }
            CaseShape::BoolIn(argument) => {
                source.push_str(&format!(
                    "    if (spx_emit_bool_in_i64(\"{}\", {}, {}) != 0) return {};\n",
                    parity_case.id,
                    symbol,
                    argument,
                    100 + index
                ));
            }
        }
    }
    source.push_str("    return 0;\n}\n");
    source
}

fn normalized_stdout(output: std::process::Output, label: &str) -> Vec<u8> {
    assert!(
        output.status.success(),
        "{label} failed with {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .replace("\r\n", "\n")
        .into_bytes()
}

fn run_native(generated: &str, root: &Path, optimization: &str) -> Vec<u8> {
    let source = root.join(format!("native-{optimization}.c"));
    let executable = root.join(format!(
        "native-{optimization}{}",
        std::env::consts::EXE_SUFFIX
    ));
    std::fs::write(&source, format!("{generated}\n{}", native_probe())).unwrap();
    let compiled = Command::new("clang")
        .args([
            "-std=c11",
            optimization,
            "-Wall",
            "-Wextra",
            "-Werror",
            "-DSPX_NO_ENTRY_WRAPPER",
        ])
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "native {optimization} compilation failed: {}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    normalized_stdout(
        Command::new(&executable).output().unwrap(),
        &format!("native {optimization}"),
    )
}

fn run_core_wasm(program: &semaprax::ast::Program, root: &Path, ids: &[&str]) -> Vec<u8> {
    let package = root.join("web");
    // Transcript rows may share one declaration (the mutate.branches pair);
    // exports must be unique while preserving first-seen order.
    let mut selected: Vec<String> = Vec::new();
    for parity_case in PARITY_CASES.iter().filter(|entry| ids.contains(&entry.id)) {
        let symbol_id = parity_case.symbol_id.to_owned();
        if !selected.contains(&symbol_id) {
            selected.push(symbol_id);
        }
    }
    wasm::build_web_with_scalar_exports(program, &package, &selected).unwrap();
    let script = root.join("observe-core-wasm.mjs");
    std::fs::write(
        &script,
        r#"import { readFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";
import { resolve } from "node:path";

const packageDirectory = resolve(process.argv[2]);
const bindings = await import(pathToFileURL(resolve(packageDirectory, "semaprax.bindings.js")));
const runtime = await bindings.instantiateBytes(await readFile(resolve(packageDirectory, "app.wasm")));
const cases = process.argv.slice(3).map((entry) => {
  const [id, symbol, argument] = entry.split("|");
  return { id, symbol, argument };
});
for (const { id, symbol, argument } of cases) {
  const values = argument === undefined || argument === "" ? [] : [argument === "true"];
  const outcome = runtime.call(symbol, ...values);
  const observation = outcome.ok
    ? { id, ok: true, value: typeof outcome.value === "bigint" ? outcome.value.toString() : outcome.value }
    : { id, ok: false, status: {
        schema: outcome.status.schema,
        domain_id: outcome.status.domain_id,
        code: outcome.status.code,
      } };
  process.stdout.write(`${JSON.stringify(observation)}\n`);
}
"#,
    )
    .unwrap();
    let case_arguments: Vec<String> = PARITY_CASES
        .iter()
        .filter(|parity_case| ids.contains(&parity_case.id))
        .map(|parity_case| {
            format!(
                "{}|{}|{}",
                parity_case.id,
                parity_case.symbol_id,
                match parity_case.shape {
                    CaseShape::BoolIn(false) => "false".to_owned(),
                    CaseShape::BoolIn(true) => "true".to_owned(),
                    _ => String::new(),
                }
            )
        })
        .collect();
    normalized_stdout(
        Command::new("node")
            .arg(&script)
            .arg(&package)
            .args(&case_arguments)
            .output()
            .unwrap(),
        "Core-Wasm Node observer",
    )
}

// ---------------------------------------------------------------------------
// Core evidence: exact backend parity.
// ---------------------------------------------------------------------------

#[test]
fn native_o0_o2_and_core_wasm_match_the_interpreter_exactly() {
    if !require_tools_or_skip() {
        return;
    }
    let program = parse(PARITY_FIXTURE, Path::new("interpreter-parity.spx")).unwrap();
    let diagnostics = verify::verify(&program);
    assert!(
        diagnostics.is_empty(),
        "fixture verification failed: {diagnostics:?}"
    );
    let generated = codegen::emit_c(&program).unwrap();

    let ordinal = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "semaprax-interpreter-parity-{}-{ordinal}",
        std::process::id()
    ));
    std::fs::create_dir(&root).unwrap();

    let native_all_o0 = run_native(&generated, &root, "-O0");
    let native_all_o2 = run_native(&generated, &root, "-O2");

    // The Core-Wasm scalar-export lane requires the whole program to bind
    // only by-value direct i64/bool scalars; that leg runs over WEB_FIXTURE
    // and must match the same transcript rows from every other producer.
    const WEB_EXCLUDED: &[&str] = &[
        "case.i32.mul.overflow",
        "case.u8.add.overflow",
        "case.char.order",
        "case.floats.compare",
    ];
    let web_ids: Vec<&str> = PARITY_CASES
        .iter()
        .map(|parity_case| parity_case.id)
        .filter(|id| !WEB_EXCLUDED.contains(id))
        .collect();
    let web_program = parse(WEB_FIXTURE, Path::new("interpreter-parity-web.spx")).unwrap();
    assert!(verify::verify(&web_program).is_empty());
    let core_wasm = run_core_wasm(&web_program, &root, &web_ids);

    let _ = std::fs::remove_dir_all(root);

    assert_eq!(
        native_all_o0, native_all_o2,
        "native optimization changed results"
    );
    assert_eq!(native_all_o0, EXPECTED_PARITY_TRANSCRIPT.as_bytes());

    // Interpreter over the full surface equals native exactly.
    let fixture_path = write_temp(PARITY_FIXTURE);
    let interpreted_full = interpreter_transcript(&fixture_path);
    cleanup(&fixture_path);
    assert_eq!(
        interpreted_full.into_bytes(),
        native_all_o0,
        "interpreter transcript diverges from both backends"
    );

    // The web subset agrees byte-for-byte with the same lines from every
    // producer: Node/Wasm == native O0 == native O2 == interpreter.
    assert_eq!(core_wasm, transcript_subset(&native_all_o0, &web_ids));
}

/// Keeps the transcript lines whose `id` appears in `ids` (shared order).
fn transcript_subset(transcript: &[u8], ids: &[&str]) -> Vec<u8> {
    String::from_utf8(transcript.to_vec())
        .unwrap()
        .lines()
        .filter(|line| {
            let start = line.find("\"id\":\"").expect("id member") + "\"id\":\"".len();
            let end = start + line[start..].find('"').unwrap();
            ids.contains(&&line[start..end])
        })
        .map(|line| format!("{line}\n"))
        .collect::<String>()
        .into_bytes()
}

// ---------------------------------------------------------------------------
// Golden KATs and determinism.
// ---------------------------------------------------------------------------

const GOLDEN_ENVELOPE_DIGEST: &str =
    "sha256:3822764122096b52498af17e81ca4f7c117ff8ac506604c90fa768ddf30fa460";
const FUEL_EXHAUSTED_ENVELOPE_DIGEST: &str =
    "sha256:df024bfd3a963e5917224195d4e13bbec30b76b443bab5fec74ddf4d651ac5e7";

#[test]
fn golden_envelopes_are_pinned_and_deterministic() {
    let path = Path::new(MEANING_PATH);
    let options = InterpreterOptions::default();
    let first = interpreter::interpret(path, "math.add", &owned_args(&["19", "23"]), &options)
        .expect("golden envelope");
    let second = interpreter::interpret(path, "math.add", &owned_args(&["19", "23"]), &options)
        .expect("golden envelope");
    assert_eq!(
        first.envelope, second.envelope,
        "generation is deterministic"
    );
    assert!(first.returned);
    assert_eq!(
        sha256_hex(first.envelope.as_bytes()),
        GOLDEN_ENVELOPE_DIGEST
    );
    assert!(first
        .envelope
        .contains("\"outcome\":{\"kind\":\"returned\",\"type\":\"i64\",\"value\":\"42\"}"));
    assert!(first.envelope.contains(
        "\"arguments\":[{\"index\":0,\"name\":\"left\",\"type\":\"i64\",\"value\":\"19\"},\
{\"index\":1,\"name\":\"right\",\"type\":\"i64\",\"value\":\"23\"}]"
    ));

    // Fuel exhaustion is deterministic and pinned too.
    let tiny = InterpreterOptions::new(options.max_bytes, 16).unwrap();
    let exhausted = interpreter::interpret(path, "math.add", &owned_args(&["19", "23"]), &tiny)
        .expect("envelope");
    assert!(!exhausted.returned);
    assert!(exhausted
        .envelope
        .contains("\"fuel\":{\"steps_used\":16,\"budget\":16,\"exhausted\":true}"));
    assert!(exhausted
        .envelope
        .contains("\"outcome\":{\"kind\":\"fuel_exhausted\"}"));
    assert_eq!(
        sha256_hex(exhausted.envelope.as_bytes()),
        FUEL_EXHAUSTED_ENVELOPE_DIGEST
    );

    interpreter::verify_envelope(&first.envelope).expect("golden envelope verifies");
    interpreter::verify_envelope_against_source(&first.envelope, path)
        .expect("source binding holds");
    interpreter::verify_envelope(&exhausted.envelope).expect("exhausted envelope verifies");
}

#[test]
fn cli_double_run_is_byte_identical() {
    let args = [
        "interpret",
        MEANING_PATH,
        "--function",
        "math.add",
        "--arg",
        "19",
        "--arg",
        "23",
    ];
    let (first_code, first_out, _) = cli(&args);
    let (second_code, second_out, _) = cli(&args);
    assert_eq!(first_code, 0);
    assert_eq!(second_code, 0);
    assert_eq!(first_out, second_out);
    assert!(first_out.contains("\"kind\":\"returned\""));
}

#[test]
fn single_file_run_uses_the_interpreter_with_capacity_and_json_options() {
    let source = r#"
module test.single_file_run;
@id("app.main")
fn main() -> i64 { 40 + 2 }
"#;
    let path = write_temp(source);
    let path_text = path.to_str().unwrap();

    let (code, stdout, stderr) = cli(&["run", path_text]);
    assert_eq!(code, 0);
    assert_eq!(stdout, "42\n");
    assert_eq!(stderr, "");

    let (code, stdout, stderr) = cli(&[
        "run",
        path_text,
        "--json",
        "--max-steps",
        "100",
        "--max-bytes",
        "65536",
    ]);
    assert_eq!(code, 0);
    assert_eq!(stderr, "");
    let envelope: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(envelope["schema"], "semaprax.interpret.v1");
    assert_eq!(envelope["payload"]["outcome"]["value"], "42");

    let (code, stdout, stderr) = cli(&["run", path_text, "--max-steps", "1"]);
    assert_eq!(code, 1);
    assert_eq!(stdout, "");
    assert_eq!(stderr, "single-file execution exhausted its step budget\n");
    cleanup(&path);
}

#[test]
fn single_file_run_executes_the_bounded_stdout_interpreter_profile() {
    let source = r#"
module test.single_file_stdout;
permit { process.stdout.write }
@id("app.main")
fn main() -> i64 uses { process.stdout.write } {
    let data = [65u8, 0u8, 66u8];
    let view = array_as_slice(data);
    let written = stdout_write(view);
    if written == 3usize { 7 } else { 0 }
}
"#;
    let path = write_temp(source);
    let path_text = path.to_str().unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args(["run", path_text])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, [b'A', 0, b'B', b'7', b'\n']);
    assert!(output.stderr.is_empty());

    let (code, stdout, stderr) = cli(&["run", path_text, "--json"]);
    assert_eq!(code, 0);
    assert_eq!(stderr, "");
    let envelope: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(envelope["schema"], "semaprax.single-file-run.v1");
    assert_eq!(envelope["outcome"]["value"], "7");
    assert_eq!(envelope["stdout"], serde_json::json!([65, 0, 66]));
    cleanup(&path);
}

// ---------------------------------------------------------------------------
// Fuel accounting and capacity limits.
// ---------------------------------------------------------------------------

#[test]
fn fuel_exhaustion_is_fail_closed_with_exact_accounting() {
    let path = write_temp(PARITY_FIXTURE);

    // Tiny budgets exhaust deterministically: steps_used pins to the budget
    // and the outcome is a capacity kind, never a language status.
    for budget in [1usize, 4] {
        let options = InterpreterOptions::new(65536, budget).unwrap();
        let interpretation =
            interpreter::interpret(&path, "case.mutate.chain", &[], &options).expect("envelope");
        assert!(!interpretation.returned, "budget {budget}");
        assert!(
            interpretation.envelope.contains(&format!(
                "\"fuel\":{{\"steps_used\":{budget},\"budget\":{budget},\"exhausted\":true}}"
            )),
            "{}",
            interpretation.envelope
        );
        assert!(interpretation
            .envelope
            .contains("\"outcome\":{\"kind\":\"fuel_exhausted\"}"));
        interpreter::verify_envelope(&interpretation.envelope)
            .unwrap_or_else(|error| panic!("budget {budget}: {error}"));
    }

    // A generous budget succeeds and never reports exhaustion.
    let options = InterpreterOptions::new(65536, DEFAULT_MAX_STEPS).unwrap();
    let interpretation =
        interpreter::interpret(&path, "case.mutate.chain", &[], &options).expect("envelope");
    assert!(interpretation.returned);
    assert!(!interpretation.envelope.contains("\"exhausted\":true"));

    // Step bounds reject out-of-range library options.
    assert!(InterpreterOptions::new(65536, 0).is_err());
    assert!(InterpreterOptions::new(65536, MAX_STEPS_LIMIT + 1).is_err());
    assert!(InterpreterOptions::new(65536, MAX_STEPS_LIMIT).is_ok());
    cleanup(&path);
}

#[test]
fn call_depth_ceiling_is_reported_not_crashed() {
    let recursive = r#"
module test.interpreter_depth;

@id("depth.down")
fn down(value: i64) -> i64 {
    if value <= 0 { 0 } else { down(value - 1) }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;
    let path = write_temp(recursive);
    let deep = format!("{}", MAX_CALL_DEPTH * 4);
    let interpretation = interpreter::interpret(
        &path,
        "depth.down",
        std::slice::from_ref(&deep),
        &InterpreterOptions::default(),
    )
    .expect("envelope");
    assert!(!interpretation.returned);
    assert!(interpretation
        .envelope
        .contains("\"outcome\":{\"kind\":\"call_depth_exceeded\"}"));
    assert!(!interpretation.envelope.contains("\"exhausted\":true"));
    interpreter::verify_envelope(&interpretation.envelope).expect("verifies");

    // Shallow recursion within the ceiling returns normally.
    let shallow = interpreter::interpret(
        &path,
        "depth.down",
        &owned_args(&["3"]),
        &InterpreterOptions::default(),
    )
    .expect("envelope");
    assert!(shallow.returned);
    cleanup(&path);
}

// ---------------------------------------------------------------------------
// Admission, selection, and argument diagnostics.
// ---------------------------------------------------------------------------

#[test]
fn admission_and_rejections_use_closed_results() {
    // Automatic-identity functions are outside the profile.
    let automatic = r#"
module test.interpreter_automatic;

fn helper(value: i64) -> i64 { value }

@id("app.main")
fn main() -> i64 { helper(1) }
"#;
    let path = write_temp(automatic);
    let errors = interpreter::interpret(
        &path,
        "helper",
        &owned_args(&["1"]),
        &InterpreterOptions::default(),
    )
    .expect_err("automatic identities are outside the profile");
    assert!(
        errors
            .iter()
            .any(|item| item.code == "SPX-F102" && item.message.contains("automatic_identity")),
        "{errors:?}"
    );
    cleanup(&path);

    // Copy-record construction and projection are part of the admitted
    // interpreter profile and execute before the remaining rejection cases.
    let aggregate_body = r#"
module test.interpreter_aggregate;

record Point {
    @id("point.x") x: i64,
    @id("point.y") y: i64,
}

@id("agg.make-y")
fn make_y() -> i64 {
    let point = Point { x: 1, y: 2 };
    point.y
}

@id("app.main")
fn main() -> i64 { make_y() }
"#;
    let path = write_temp(aggregate_body);
    let interpretation =
        interpreter::interpret(&path, "make_y", &[], &InterpreterOptions::default())
            .expect("copy-record construction and projection are admitted");
    assert!(interpretation.returned);
    assert!(interpretation.envelope.contains("\"value\":\"2\""));
    cleanup(&path);

    // Callees outside the profile poison an otherwise admitted entry.
    let mixed_callees = r#"
module test.interpreter_callees;

@id("mixed.admitted-callee")
fn admitted_callee(value: i64) -> i64 { value + 1 }

@id("mixed.entry")
fn entry() -> i64 { admitted_callee(1) }

fn unannotated() -> i64 { 0 }

@id("mixed.calls-automatic")
fn calls_automatic() -> i64 { unannotated() }

@id("app.main")
fn main() -> i64 { 0 }
"#;
    let path = write_temp(mixed_callees);
    let envelope = interpret_case(&path, "mixed.entry", &[]).expect("admitted callee chain");
    assert!(envelope.contains("\"value\":\"2\""));
    let errors = interpreter::interpret(
        &path,
        "calls_automatic",
        &[],
        &InterpreterOptions::default(),
    )
    .expect_err("callee with automatic identity is rejected");
    assert!(
        errors
            .iter()
            .any(|item| item.code == "SPX-F102" && item.message.contains("unsupported_callee")),
        "{errors:?}"
    );
    cleanup(&path);
}

#[test]
fn argument_diagnostics_are_exact() {
    let meaning = Path::new(MEANING_PATH);

    let errors = interpreter::interpret(
        meaning,
        "math.add",
        &owned_args(&["19"]),
        &InterpreterOptions::default(),
    )
    .expect_err("too few arguments");
    assert!(
        errors.iter().any(|item| item.code == "SPX-F103"),
        "{errors:?}"
    );

    let errors = interpreter::interpret(
        meaning,
        "math.add",
        &owned_args(&["19", "23", "5"]),
        &InterpreterOptions::default(),
    )
    .expect_err("too many arguments");
    assert!(
        errors.iter().any(|item| item.code == "SPX-F103"),
        "{errors:?}"
    );

    let errors = interpreter::interpret(
        meaning,
        "math.add",
        &owned_args(&["true", "23"]),
        &InterpreterOptions::default(),
    )
    .expect_err("bool literal for i64 parameter");
    assert!(
        errors.iter().any(|item| item.code == "SPX-F103"),
        "{errors:?}"
    );

    let errors = interpreter::interpret(
        meaning,
        "math.add",
        &owned_args(&["0x19", "23"]),
        &InterpreterOptions::default(),
    )
    .expect_err("non-canonical literal");
    assert!(
        errors.iter().any(|item| item.code == "SPX-F103"),
        "{errors:?}"
    );

    let errors = interpreter::interpret(
        meaning,
        "math.add",
        &owned_args(&["007", "23"]),
        &InterpreterOptions::default(),
    )
    .expect_err("leading-zero literal");
    assert!(
        errors.iter().any(|item| item.code == "SPX-F103"),
        "{errors:?}"
    );

    // Canonical literals are accepted, including negatives.
    let fixture = write_temp(PARITY_FIXTURE);
    let envelope = interpret_case(&fixture, "case.sum", &["-19", "-3"]).expect("envelope");
    assert!(envelope.contains("\"value\":\"-22\""), "{envelope}");
    cleanup(&fixture);
}

// ---------------------------------------------------------------------------
// Envelope replay, tamper rejection, and drift binding.
// ---------------------------------------------------------------------------

#[test]
fn verify_envelope_accepts_only_genuine_envelopes() {
    let path = write_temp(PARITY_FIXTURE);
    let envelope = interpret_case(&path, "case.mutate.chain", &[]).expect("envelope");
    interpreter::verify_envelope(&envelope).expect("genuine envelope verifies");
    interpreter::verify_envelope_against_source(&envelope, &path).expect("binding holds");

    interpreter::verify_envelope("not json").unwrap_err();
    interpreter::verify_envelope("[]").unwrap_err();
    interpreter::verify_envelope(&format!(
        "{}{}",
        &envelope[..envelope.len() - 1],
        ",\"injected\":true}"
    ))
    .unwrap_err();
    let foreign = envelope.replace(SCHEMA, "semaprax.foreign.v1");
    assert_ne!(foreign, envelope);
    interpreter::verify_envelope(&foreign).unwrap_err();
    cleanup(&path);
}

/// Replaces the exact value of one `"key":"string"` member (first match).
fn replace_string_field(envelope: &str, key: &str, replacement: &str) -> String {
    let needle = format!("\"{key}\":\"");
    let start = envelope
        .find(&needle)
        .unwrap_or_else(|| panic!("field {key} present"))
        + needle.len();
    let end = start + envelope[start..].find('"').unwrap();
    let mut tampered = envelope.to_owned();
    tampered.replace_range(start..end, replacement);
    tampered
}

/// Replaces the exact digits of one unsigned `"key":N` member (first match).
fn replace_number_field(envelope: &str, key: &str, replacement: u64) -> String {
    let needle = format!("\"{key}\":");
    let start = envelope
        .find(&needle)
        .unwrap_or_else(|| panic!("field {key} present"))
        + needle.len();
    let end = start
        + envelope[start..]
            .find(|character: char| !character.is_ascii_digit())
            .unwrap();
    let mut tampered = envelope.to_owned();
    tampered.replace_range(start..end, &replacement.to_string());
    tampered
}

#[test]
fn every_payload_field_is_tamper_evident() {
    let path = write_temp(PARITY_FIXTURE);
    let envelope = interpret_case(&path, "case.mutate.chain", &[]).expect("envelope");
    interpreter::verify_envelope(&envelope).expect("genuine envelope verifies");

    // Every mutation below changes exactly one field; every one must be
    // rejected by independent replay.
    let mutations: Vec<(&str, String)> = vec![
        (
            "outer schema",
            envelope.replacen(SCHEMA, "semaprax.foreign.v1", 1),
        ),
        (
            "payload schema",
            envelope.replace(
                &format!("\"payload\":{{\"schema\":\"{SCHEMA}\""),
                "\"payload\":{\"schema\":\"semaprax.foreign.v1\"",
            ),
        ),
        (
            "digest",
            replace_string_field(&envelope, "digest", "sha256:00"),
        ),
        ("bytes", replace_number_field(&envelope, "bytes", 0)),
        (
            "source.path",
            replace_string_field(&envelope, "path", "drifted.spx"),
        ),
        (
            "source.revision",
            replace_string_field(&envelope, "revision", "sha256:11"),
        ),
        (
            "source.sha256",
            replace_string_field(&envelope, "sha256", "sha256:22"),
        ),
        (
            "function.stable_id",
            replace_string_field(&envelope, "stable_id", "case.other"),
        ),
        (
            "function.name",
            replace_string_field(&envelope, "name", "other"),
        ),
        (
            "limits.max_bytes",
            replace_number_field(&envelope, "max_bytes", 1024),
        ),
        (
            "limits.max_steps",
            replace_number_field(&envelope, "max_steps", 1),
        ),
        (
            "fuel.steps_used",
            replace_number_field(&envelope, "steps_used", 0),
        ),
        ("fuel.budget", {
            // `budget` appears after `steps_used`; target the fuel member.
            let fuel_needle = "\"budget\":";
            let limits_end = envelope.find("\"fuel\"").unwrap();
            let offset =
                envelope[limits_end..].find(fuel_needle).unwrap() + limits_end + fuel_needle.len();
            let end = offset
                + envelope[offset..]
                    .find(|character: char| !character.is_ascii_digit())
                    .unwrap();
            let mut tampered = envelope.clone();
            tampered.replace_range(offset..end, "2");
            tampered
        }),
        (
            "fuel.exhausted",
            envelope.replace("\"exhausted\":false", "\"exhausted\":true"),
        ),
        (
            "outcome.kind",
            envelope.replace("\"kind\":\"returned\"", "\"kind\":\"failed\""),
        ),
        (
            "outcome.type",
            envelope.replace("\"type\":\"i64\"", "\"type\":\"bool\""),
        ),
        (
            "outcome.value",
            envelope.replace("\"value\":\"10\"", "\"value\":\"11\""),
        ),
        (
            "nonclaims",
            envelope.replace(
                "read_only_evaluation_only",
                "read_only_evaluation_only_extra",
            ),
        ),
        (
            "injected member",
            format!("{}{}", &envelope[..envelope.len() - 1], ",\"extra\":1}"),
        ),
    ];

    for (label, tampered) in &mutations {
        assert_ne!(*tampered, envelope, "tamper ({label}) changed nothing");
        assert!(
            interpreter::verify_envelope(tampered).is_err(),
            "tamper ({label}) must be rejected"
        );
    }

    // Forged-but-re-signed payloads still fail closed where closed derivations
    // exist inside the payload itself. Echo-only fields (path, names, step
    // counts) are authenticated by the payload digest but deliberately not
    // independently re-derivable during replay.
    let contradictory_fuel =
        remint_digest(&envelope.replace("\"exhausted\":false", "\"exhausted\":true"));
    let error = interpreter::verify_envelope(&contradictory_fuel)
        .expect_err("exhausted fuel without steps_used == budget fails replay");
    assert_eq!(error.code, "SPX-F106");

    cleanup(&path);
}
