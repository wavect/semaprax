//! Executable evidence for Reference Interpreter scalar-widened admission.
//!
//! Proves that the widened admission profile — monomorphic effect-free
//! scalar functions whose parameters/results are direct `i64`, `i32`, `u8`,
//! `char`, `f32`, `f64`, or `bool`, including mixed signatures — evaluates
//! EXACTLY like the native C11 O0/O2 backends and, for the whole-program
//! i64/bool web-profile subset, like Node/Wasm. Every widened transcript row
//! is byte-identical across producers. Float results are rendered as their
//! exact big-endian IEEE-754 bit patterns so `-0.0`, infinities, and NaN
//! payloads are observable without trusting any platform's decimal
//! formatting. The interpreter itself never invokes a toolchain; only the
//! parity corpus does.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::interpreter::{self, ArgumentValue, InterpreterOptions, DEFAULT_MAX_STEPS};
use semaprax::{codegen, parse, verify, wasm};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const REQUIRE_ENV: &str = "SEMAPRAX_REQUIRE_INTERPRETER_BACKEND_PARITY";

/// Full-surface widened corpus module: every function is explicit-ID,
/// monomorphic, effect-free, and binds only by-value direct scalars across
/// the widened boundary while exercising checked-arithmetic statuses over
/// `i32`/`u8`, total IEEE-754 float arithmetic including `-0.0` and
/// infinities, NaN ordering semantics, char ordering, mixed signatures, and
/// recursion over widened parameter types.
const WIDEN_FIXTURE: &str = r#"
module test.interpreter_scalar_widen;

@id("app.main")
fn main() -> i64 { 0 }

@id("widen.char.echo")
fn char_echo(value: char) -> char { value }

@id("widen.char.less")
fn char_less(left: char, right: char) -> bool { left < right }

@id("widen.char.newline.eq")
fn char_newline_eq(value: char) -> bool { value == '\n' }

@id("widen.f32.arith")
fn f32_arith(value: f32) -> f32 { value / 4.0f32 - 0.25f32 }

@id("widen.f32.rounding")
fn f32_rounding() -> f32 { 0.1f32 + 0.2f32 }

@id("widen.f64.arith")
fn f64_arith(left: f64, right: f64) -> f64 { left * right + 0.5 }

@id("widen.f64.infinity")
fn f64_infinity() -> f64 { 1.0 / 0.0 }

@id("widen.f64.nan.ordering")
fn f64_nan_ordering() -> i64 {
    let quiet = 0.0 / 0.0;
    if quiet < 1.0 || quiet > 1.0 || quiet == quiet { 0 } else { 1 }
}

@id("widen.f64.neg.zero")
fn f64_neg_zero() -> f64 { -0.0 }

@id("widen.i32.add.overflow")
fn i32_add_overflow() -> i32 { 2147483647i32 + 1i32 }

@id("widen.i32.div.overflow")
fn i32_div_overflow() -> i32 { (-2147483647i32 - 1i32) / -1i32 }

@id("widen.i32.mul.overflow")
fn i32_mul_overflow() -> i32 { 100000i32 * 100000i32 }

@id("widen.i32.neg.overflow")
fn i32_neg_overflow() -> i32 { -(-2147483647i32 - 1i32) }

@id("widen.mixed.blend")
fn blend(narrow: f32, flag: bool) -> i64 {
    if flag { if narrow > 1.0f32 { 1 } else { 2 } } else { 3 }
}

@id("widen.mixed.select")
fn mixed_select(a: i32, b: u8) -> f64 {
    if a < 0i32 && b < 10u8 { -2.5 } else { 2.5 }
}

@id("widen.recursion.fib")
fn fib(n: i32) -> i32 {
    if n < 2i32 { n } else { fib(n - 1i32) + fib(n - 2i32) }
}

@id("widen.recursion.sum.to")
fn sum_to(n: u8) -> u8 {
    if n == 0u8 { 0u8 } else { sum_to(n - 1u8) + n }
}

@id("widen.u8.add.boundary")
fn u8_add_boundary() -> u8 { 250u8 + 10u8 }

@id("widen.u8.div.zero")
fn u8_div_zero() -> u8 { 7u8 / 0u8 }

@id("widen.u8.echo")
fn u8_echo(value: u8) -> u8 { value }

@id("widen.u8.mul.boundary")
fn u8_mul_boundary() -> u8 { 16u8 * 16u8 }

@id("widen.u8.sub.boundary")
fn u8_sub_boundary() -> u8 { 0u8 - 1u8 }

@id("widen.web.bool.gate")
fn bool_gate(flag: bool) -> bool { if flag && !flag { false } else { true } }

@id("widen.web.countdown")
fn countdown(total: i64) -> i64 {
    if total <= 0 { 0 } else { countdown(total - 2) }
}
"#;

/// Web-admissible subset: the Public Scalar Export Profile v1 lane requires
/// the whole program — every binding, not just exported signatures — to use
/// only direct `i64`/`bool` scalars, so the Node/Wasm leg runs over this
/// module while native and interpreter legs cover the full widened surface
/// above.
const WEB_FIXTURE: &str = r#"
module test.interpreter_scalar_widen_web;

@id("app.main")
fn main() -> i64 { 0 }

@id("widen.web.bool.gate")
fn bool_gate(flag: bool) -> bool { if flag && !flag { false } else { true } }

@id("widen.web.countdown")
fn countdown(total: i64) -> i64 {
    if total <= 0 { 0 } else { countdown(total - 2) }
}
"#;

/// One transcript observation: which function to call, with which arguments.
struct WidenCase {
    /// Transcript identifier (unique per line).
    id: &'static str,
    /// Selected stable id.
    symbol_id: &'static str,
    shape: CaseShape,
    /// Canonical `--arg` texts for the interpreter leg, in declaration order.
    args: &'static [&'static str],
    /// Matching C argument expressions for the native probe (without the
    /// leading context argument).
    c_args: &'static str,
}

#[derive(Clone, Copy, PartialEq)]
enum CaseShape {
    /// `fn() -> T` for each widened result kind.
    RetI64,
    RetI32,
    RetU8,
    RetF32,
    RetF64,
    /// `fn(i64) -> i64`.
    I64In,
    /// `fn(bool) -> bool`.
    BoolIn,
    /// `fn(u8) -> u8`.
    U8In,
    /// `fn(i32) -> i32`.
    I32In,
    /// `fn(f32) -> f32`.
    F32In,
    /// `fn(char) -> char`.
    CharIn,
    /// `fn(char) -> bool`.
    CharPred,
    /// `fn(char, char) -> bool`.
    CharPair,
    /// `fn(f64, f64) -> f64`.
    F64Pair,
    /// `fn(i32, u8) -> f64`.
    MixedI32U8,
    /// `fn(f32, bool) -> i64`.
    BlendF32Bool,
}

impl CaseShape {
    fn emitter(self) -> &'static str {
        match self {
            Self::RetI64 => "spx_emit_ret_i64",
            Self::RetI32 => "spx_emit_ret_i32",
            Self::RetU8 => "spx_emit_ret_u8",
            Self::RetF32 => "spx_emit_ret_f32",
            Self::RetF64 => "spx_emit_ret_f64",
            Self::I64In => "spx_emit_i64_in",
            Self::BoolIn => "spx_emit_bool_in",
            Self::U8In => "spx_emit_u8_in",
            Self::I32In => "spx_emit_i32_in",
            Self::F32In => "spx_emit_f32_in",
            Self::CharIn => "spx_emit_char_in",
            Self::CharPred => "spx_emit_char_pred",
            Self::CharPair => "spx_emit_char_pair",
            Self::F64Pair => "spx_emit_f64_pair",
            Self::MixedI32U8 => "spx_emit_mixed",
            Self::BlendF32Bool => "spx_emit_blend",
        }
    }
}

/// Alphabetical transcript order shared by every producer (native probe, Node
/// observer, and interpreter transcript).
const WIDEN_CASES: &[WidenCase] = &[
    WidenCase {
        id: "widen.char.echo",
        symbol_id: "widen.char.echo",
        shape: CaseShape::CharIn,
        args: &[r"'\u{2603}'"],
        c_args: "(uint32_t)9731",
    },
    WidenCase {
        id: "widen.char.less",
        symbol_id: "widen.char.less",
        shape: CaseShape::CharPair,
        args: &["'a'", "'b'"],
        c_args: "(uint32_t)97, (uint32_t)98",
    },
    WidenCase {
        id: "widen.char.newline.eq",
        symbol_id: "widen.char.newline.eq",
        shape: CaseShape::CharPred,
        args: &[r"'\n'"],
        c_args: "(uint32_t)10",
    },
    WidenCase {
        id: "widen.f32.arith",
        symbol_id: "widen.f32.arith",
        shape: CaseShape::F32In,
        args: &["9.0f32"],
        c_args: "9.0f",
    },
    WidenCase {
        id: "widen.f32.rounding",
        symbol_id: "widen.f32.rounding",
        shape: CaseShape::RetF32,
        args: &[],
        c_args: "",
    },
    WidenCase {
        id: "widen.f64.arith",
        symbol_id: "widen.f64.arith",
        shape: CaseShape::F64Pair,
        args: &["150.25", "2.0"],
        c_args: "150.25, 2.0",
    },
    WidenCase {
        id: "widen.f64.infinity",
        symbol_id: "widen.f64.infinity",
        shape: CaseShape::RetF64,
        args: &[],
        c_args: "",
    },
    WidenCase {
        id: "widen.f64.nan.ordering",
        symbol_id: "widen.f64.nan.ordering",
        shape: CaseShape::RetI64,
        args: &[],
        c_args: "",
    },
    WidenCase {
        id: "widen.f64.neg.zero",
        symbol_id: "widen.f64.neg.zero",
        shape: CaseShape::RetF64,
        args: &[],
        c_args: "",
    },
    WidenCase {
        id: "widen.i32.add.overflow",
        symbol_id: "widen.i32.add.overflow",
        shape: CaseShape::RetI32,
        args: &[],
        c_args: "",
    },
    WidenCase {
        id: "widen.i32.div.overflow",
        symbol_id: "widen.i32.div.overflow",
        shape: CaseShape::RetI32,
        args: &[],
        c_args: "",
    },
    WidenCase {
        id: "widen.i32.mul.overflow",
        symbol_id: "widen.i32.mul.overflow",
        shape: CaseShape::RetI32,
        args: &[],
        c_args: "",
    },
    WidenCase {
        id: "widen.i32.neg.overflow",
        symbol_id: "widen.i32.neg.overflow",
        shape: CaseShape::RetI32,
        args: &[],
        c_args: "",
    },
    WidenCase {
        id: "widen.mixed.blend",
        symbol_id: "widen.mixed.blend",
        shape: CaseShape::BlendF32Bool,
        args: &["2.0f32", "true"],
        c_args: "2.0f, true",
    },
    WidenCase {
        id: "widen.mixed.select",
        symbol_id: "widen.mixed.select",
        shape: CaseShape::MixedI32U8,
        args: &["-7i32", "3u8"],
        c_args: "(int32_t)-7, (uint8_t)3",
    },
    WidenCase {
        id: "widen.recursion.fib",
        symbol_id: "widen.recursion.fib",
        shape: CaseShape::I32In,
        args: &["15i32"],
        c_args: "(int32_t)15",
    },
    WidenCase {
        id: "widen.recursion.sum.to",
        symbol_id: "widen.recursion.sum.to",
        shape: CaseShape::U8In,
        args: &["22u8"],
        c_args: "(uint8_t)22",
    },
    WidenCase {
        id: "widen.u8.add.boundary",
        symbol_id: "widen.u8.add.boundary",
        shape: CaseShape::RetU8,
        args: &[],
        c_args: "",
    },
    WidenCase {
        id: "widen.u8.div.zero",
        symbol_id: "widen.u8.div.zero",
        shape: CaseShape::RetU8,
        args: &[],
        c_args: "",
    },
    WidenCase {
        id: "widen.u8.echo",
        symbol_id: "widen.u8.echo",
        shape: CaseShape::U8In,
        args: &["255u8"],
        c_args: "(uint8_t)255",
    },
    WidenCase {
        id: "widen.u8.mul.boundary",
        symbol_id: "widen.u8.mul.boundary",
        shape: CaseShape::RetU8,
        args: &[],
        c_args: "",
    },
    WidenCase {
        id: "widen.u8.sub.boundary",
        symbol_id: "widen.u8.sub.boundary",
        shape: CaseShape::RetU8,
        args: &[],
        c_args: "",
    },
    WidenCase {
        id: "widen.web.bool.gate",
        symbol_id: "widen.web.bool.gate",
        shape: CaseShape::BoolIn,
        args: &["false"],
        c_args: "false",
    },
    WidenCase {
        id: "widen.web.countdown",
        symbol_id: "widen.web.countdown",
        shape: CaseShape::I64In,
        args: &["9"],
        c_args: "(int64_t)9",
    },
];

/// The exact cross-backend transcript: sticky left-to-right failure
/// selection, compiler-owned status codes, widened success values (quoted
/// decimals for integers and char scalar values, bare booleans, and lowercase
/// IEEE-754 bit-pattern hex for floats), identical everywhere.
const EXPECTED_WIDEN_TRANSCRIPT: &str = concat!(
    "{\"id\":\"widen.char.echo\",\"ok\":true,\"value\":\"9731\"}\n",
    "{\"id\":\"widen.char.less\",\"ok\":true,\"value\":true}\n",
    "{\"id\":\"widen.char.newline.eq\",\"ok\":true,\"value\":true}\n",
    "{\"id\":\"widen.f32.arith\",\"ok\":true,\"value\":\"40000000\"}\n",
    "{\"id\":\"widen.f32.rounding\",\"ok\":true,\"value\":\"3e99999a\"}\n",
    "{\"id\":\"widen.f64.arith\",\"ok\":true,\"value\":\"4072d00000000000\"}\n",
    "{\"id\":\"widen.f64.infinity\",\"ok\":true,\"value\":\"7ff0000000000000\"}\n",
    "{\"id\":\"widen.f64.nan.ordering\",\"ok\":true,\"value\":\"1\"}\n",
    "{\"id\":\"widen.f64.neg.zero\",\"ok\":true,\"value\":\"8000000000000000\"}\n",
    "{\"id\":\"widen.i32.add.overflow\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.arithmetic.v1\",\"code\":1}}\n",
    "{\"id\":\"widen.i32.div.overflow\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.arithmetic.v1\",\"code\":5}}\n",
    "{\"id\":\"widen.i32.mul.overflow\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.arithmetic.v1\",\"code\":3}}\n",
    "{\"id\":\"widen.i32.neg.overflow\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.arithmetic.v1\",\"code\":8}}\n",
    "{\"id\":\"widen.mixed.blend\",\"ok\":true,\"value\":\"1\"}\n",
    "{\"id\":\"widen.mixed.select\",\"ok\":true,\"value\":\"c004000000000000\"}\n",
    "{\"id\":\"widen.recursion.fib\",\"ok\":true,\"value\":\"610\"}\n",
    "{\"id\":\"widen.recursion.sum.to\",\"ok\":true,\"value\":\"253\"}\n",
    "{\"id\":\"widen.u8.add.boundary\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.arithmetic.v1\",\"code\":1}}\n",
    "{\"id\":\"widen.u8.div.zero\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.arithmetic.v1\",\"code\":4}}\n",
    "{\"id\":\"widen.u8.echo\",\"ok\":true,\"value\":\"255\"}\n",
    "{\"id\":\"widen.u8.mul.boundary\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.arithmetic.v1\",\"code\":3}}\n",
    "{\"id\":\"widen.u8.sub.boundary\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.arithmetic.v1\",\"code\":2}}\n",
    "{\"id\":\"widen.web.bool.gate\",\"ok\":true,\"value\":true}\n",
    "{\"id\":\"widen.web.countdown\",\"ok\":true,\"value\":\"0\"}\n",
);

/// Rows whose selected function lives in the all-i64/bool web module.
const WEB_IDS: &[&str] = &["widen.web.bool.gate", "widen.web.countdown"];

fn owned_args(arguments: &[&str]) -> Vec<String> {
    arguments
        .iter()
        .map(|argument| (*argument).to_owned())
        .collect()
}

fn write_temp(source: &str) -> PathBuf {
    let ordinal = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-interpreter-widen-{}-{ordinal}.spx",
        std::process::id()
    ));
    std::fs::write(&path, source).unwrap();
    path
}

fn cleanup(path: &Path) {
    let _ = std::fs::remove_file(path);
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

/// Unicode scalar value of one canonical `char` literal rendering.
fn char_scalar(canonical: &str) -> u32 {
    let inner = canonical
        .strip_prefix('\'')
        .and_then(|text| text.strip_suffix('\''))
        .expect("canonical char literal");
    if let Some(escape) = inner.strip_prefix('\\') {
        match escape {
            "n" => 0x0A,
            "r" => 0x0D,
            "t" => 0x09,
            "0" => 0x00,
            "'" => 0x27,
            "\\" => 0x5C,
            _ => {
                let hex = escape
                    .strip_prefix("u{")
                    .and_then(|digits| digits.strip_suffix('}'))
                    .expect("canonical unicode escape");
                u32::from_str_radix(hex, 16).expect("hexadecimal scalar")
            }
        }
    } else {
        inner.chars().next().expect("one scalar") as u32
    }
}

/// One transcript value token per declared outcome type, byte-compatible with
/// both backend producers: quoted decimals for integers and char scalar
/// values, bare booleans, and quoted lowercase bit-pattern hex for floats.
fn transcript_value(type_text: &str, value_text: &str) -> String {
    match type_text {
        "i64" | "f32" | "f64" => format!("\"{value_text}\""),
        "bool" => value_text.to_owned(),
        "i32" => match interpreter::parse_argument(value_text) {
            Ok(ArgumentValue::Int32(value)) => format!("\"{value}\""),
            other => panic!("i32 transcript value `{value_text}` parsed as {other:?}"),
        },
        "u8" => match interpreter::parse_argument(value_text) {
            Ok(ArgumentValue::Uint8(value)) => format!("\"{value}\""),
            other => panic!("u8 transcript value `{value_text}` parsed as {other:?}"),
        },
        "char" => format!("\"{}\"", char_scalar(value_text)),
        other => panic!("unexpected transcript type {other}"),
    }
}

fn interpreter_transcript_line(envelope: &str, id: &str) -> String {
    let value: serde_json::Value = serde_json::from_str(envelope).expect("envelope JSON");
    let outcome = &value["payload"]["outcome"];
    match outcome["kind"].as_str().expect("outcome kind") {
        "returned" => format!(
            "{{\"id\":\"{id}\",\"ok\":true,\"value\":{}}}",
            transcript_value(
                outcome["type"].as_str().expect("outcome type"),
                outcome["value"].as_str().expect("outcome value"),
            )
        ),
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
        other => panic!("widened corpus cannot exhaust capacity, found {other}"),
    }
}

fn interpreter_transcript(source_path: &Path) -> String {
    WIDEN_CASES
        .iter()
        .map(|widen_case| {
            let envelope = interpret_case(source_path, widen_case.symbol_id, widen_case.args)
                .unwrap_or_else(|errors| {
                    panic!("interpret `{}` failed: {errors:?}", widen_case.symbol_id)
                });
            interpreter_transcript_line(&envelope, widen_case.id)
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
/// shared order. Integer widths, chars, and floats move through their exact C
/// scalar types; floats print as raw IEEE-754 bit patterns so `-O0` and
/// `-O2` cannot differ in formatting, only in bits.
fn native_probe() -> String {
    let mut source = r#"
typedef spx_status_token (*spx_ret_i64)(struct spx_context *, int64_t *);
typedef spx_status_token (*spx_ret_i32)(struct spx_context *, int32_t *);
typedef spx_status_token (*spx_ret_u8)(struct spx_context *, uint8_t *);
typedef spx_status_token (*spx_ret_f32)(struct spx_context *, float *);
typedef spx_status_token (*spx_ret_f64)(struct spx_context *, double *);
typedef spx_status_token (*spx_i64_in)(struct spx_context *, int64_t, int64_t *);
typedef spx_status_token (*spx_bool_in)(struct spx_context *, bool, bool *);
typedef spx_status_token (*spx_u8_in)(struct spx_context *, uint8_t, uint8_t *);
typedef spx_status_token (*spx_i32_in)(struct spx_context *, int32_t, int32_t *);
typedef spx_status_token (*spx_f32_in)(struct spx_context *, float, float *);
typedef spx_status_token (*spx_char_in)(struct spx_context *, uint32_t, uint32_t *);
typedef spx_status_token (*spx_char_pred)(struct spx_context *, uint32_t, bool *);
typedef spx_status_token (*spx_char_pair)(struct spx_context *, uint32_t, uint32_t, bool *);
typedef spx_status_token (*spx_f64_pair)(struct spx_context *, double, double, double *);
typedef spx_status_token (*spx_mixed_sig)(struct spx_context *, int32_t, uint8_t, double *);
typedef spx_status_token (*spx_blend_sig)(struct spx_context *, float, bool, int64_t *);

union spx_f32_overlay {
    float value;
    uint32_t bits;
};

union spx_f64_overlay {
    double value;
    unsigned long long bits;
};

static int spx_print_failure(
    const char *id, const struct spx_context *context, spx_status_token token
) {
    const struct spx_normalized_status *status = spx_status_resolve(context, token);
    if (status == NULL || context->status_arena.length != UINT32_C(1)) return 11;
    printf(
        "{\"id\":\"%s\",\"ok\":false,\"status\":{\"schema\":\"%s\",\"domain_id\":\"%s\",\"code\":%u}}\n",
        id, status->schema, status->domain_id, (unsigned int)status->code
    );
    return 0;
}

static int spx_emit_ret_i64(const char *id, spx_ret_i64 test_case) {
    struct spx_status_entry records[UINT32_C(8)];
    struct spx_context context = {0};
    if (!spx_context_init(&context, UINT64_C(501), records, UINT32_C(8), NULL, NULL, NULL)) return 10;
    int64_t value = INT64_C(-7777);
    spx_status_token token = test_case(&context, &value);
    if (token == SPX_STATUS_SUCCESS) {
        printf("{\"id\":\"%s\",\"ok\":true,\"value\":\"%lld\"}\n", id, (long long)value);
        return 0;
    }
    return spx_print_failure(id, &context, token);
}

static int spx_emit_ret_i32(const char *id, spx_ret_i32 test_case) {
    struct spx_status_entry records[UINT32_C(8)];
    struct spx_context context = {0};
    if (!spx_context_init(&context, UINT64_C(502), records, UINT32_C(8), NULL, NULL, NULL)) return 10;
    int32_t value = INT32_C(-7777);
    spx_status_token token = test_case(&context, &value);
    if (token == SPX_STATUS_SUCCESS) {
        printf("{\"id\":\"%s\",\"ok\":true,\"value\":\"%d\"}\n", id, (int)value);
        return 0;
    }
    return spx_print_failure(id, &context, token);
}

static int spx_emit_ret_u8(const char *id, spx_ret_u8 test_case) {
    struct spx_status_entry records[UINT32_C(8)];
    struct spx_context context = {0};
    if (!spx_context_init(&context, UINT64_C(503), records, UINT32_C(8), NULL, NULL, NULL)) return 10;
    uint8_t value = UINT8_C(171);
    spx_status_token token = test_case(&context, &value);
    if (token == SPX_STATUS_SUCCESS) {
        printf("{\"id\":\"%s\",\"ok\":true,\"value\":\"%u\"}\n", id, (unsigned int)value);
        return 0;
    }
    return spx_print_failure(id, &context, token);
}

static int spx_emit_ret_f32(const char *id, spx_ret_f32 test_case) {
    struct spx_status_entry records[UINT32_C(8)];
    struct spx_context context = {0};
    if (!spx_context_init(&context, UINT64_C(504), records, UINT32_C(8), NULL, NULL, NULL)) return 10;
    float value = -1.0f;
    spx_status_token token = test_case(&context, &value);
    if (token == SPX_STATUS_SUCCESS) {
        union spx_f32_overlay overlay = {0};
        overlay.value = value;
        printf("{\"id\":\"%s\",\"ok\":true,\"value\":\"%08x\"}\n", id, overlay.bits);
        return 0;
    }
    return spx_print_failure(id, &context, token);
}

static int spx_emit_ret_f64(const char *id, spx_ret_f64 test_case) {
    struct spx_status_entry records[UINT32_C(8)];
    struct spx_context context = {0};
    if (!spx_context_init(&context, UINT64_C(505), records, UINT32_C(8), NULL, NULL, NULL)) return 10;
    double value = -1.0;
    spx_status_token token = test_case(&context, &value);
    if (token == SPX_STATUS_SUCCESS) {
        union spx_f64_overlay overlay = {0};
        overlay.value = value;
        printf("{\"id\":\"%s\",\"ok\":true,\"value\":\"%016llx\"}\n", id, overlay.bits);
        return 0;
    }
    return spx_print_failure(id, &context, token);
}

static int spx_emit_i64_in(const char *id, spx_i64_in test_case, int64_t argument) {
    struct spx_status_entry records[UINT32_C(8)];
    struct spx_context context = {0};
    if (!spx_context_init(&context, UINT64_C(506), records, UINT32_C(8), NULL, NULL, NULL)) return 10;
    int64_t value = INT64_C(-7777);
    spx_status_token token = test_case(&context, argument, &value);
    if (token == SPX_STATUS_SUCCESS) {
        printf("{\"id\":\"%s\",\"ok\":true,\"value\":\"%lld\"}\n", id, (long long)value);
        return 0;
    }
    return spx_print_failure(id, &context, token);
}

static int spx_emit_bool_in(const char *id, spx_bool_in test_case, bool argument) {
    struct spx_status_entry records[UINT32_C(8)];
    struct spx_context context = {0};
    if (!spx_context_init(&context, UINT64_C(507), records, UINT32_C(8), NULL, NULL, NULL)) return 10;
    bool value = false;
    spx_status_token token = test_case(&context, argument, &value);
    if (token == SPX_STATUS_SUCCESS) {
        printf("{\"id\":\"%s\",\"ok\":true,\"value\":%s}\n", id, value ? "true" : "false");
        return 0;
    }
    return spx_print_failure(id, &context, token);
}

static int spx_emit_u8_in(const char *id, spx_u8_in test_case, uint8_t argument) {
    struct spx_status_entry records[UINT32_C(8)];
    struct spx_context context = {0};
    if (!spx_context_init(&context, UINT64_C(508), records, UINT32_C(8), NULL, NULL, NULL)) return 10;
    uint8_t value = UINT8_C(171);
    spx_status_token token = test_case(&context, argument, &value);
    if (token == SPX_STATUS_SUCCESS) {
        printf("{\"id\":\"%s\",\"ok\":true,\"value\":\"%u\"}\n", id, (unsigned int)value);
        return 0;
    }
    return spx_print_failure(id, &context, token);
}

static int spx_emit_i32_in(const char *id, spx_i32_in test_case, int32_t argument) {
    struct spx_status_entry records[UINT32_C(8)];
    struct spx_context context = {0};
    if (!spx_context_init(&context, UINT64_C(509), records, UINT32_C(8), NULL, NULL, NULL)) return 10;
    int32_t value = INT32_C(-7777);
    spx_status_token token = test_case(&context, argument, &value);
    if (token == SPX_STATUS_SUCCESS) {
        printf("{\"id\":\"%s\",\"ok\":true,\"value\":\"%d\"}\n", id, (int)value);
        return 0;
    }
    return spx_print_failure(id, &context, token);
}

static int spx_emit_f32_in(const char *id, spx_f32_in test_case, float argument) {
    struct spx_status_entry records[UINT32_C(8)];
    struct spx_context context = {0};
    if (!spx_context_init(&context, UINT64_C(510), records, UINT32_C(8), NULL, NULL, NULL)) return 10;
    float value = -1.0f;
    spx_status_token token = test_case(&context, argument, &value);
    if (token == SPX_STATUS_SUCCESS) {
        union spx_f32_overlay overlay = {0};
        overlay.value = value;
        printf("{\"id\":\"%s\",\"ok\":true,\"value\":\"%08x\"}\n", id, overlay.bits);
        return 0;
    }
    return spx_print_failure(id, &context, token);
}

static int spx_emit_char_in(const char *id, spx_char_in test_case, uint32_t argument) {
    struct spx_status_entry records[UINT32_C(8)];
    struct spx_context context = {0};
    if (!spx_context_init(&context, UINT64_C(511), records, UINT32_C(8), NULL, NULL, NULL)) return 10;
    uint32_t value = UINT32_C(0);
    spx_status_token token = test_case(&context, argument, &value);
    if (token == SPX_STATUS_SUCCESS) {
        printf("{\"id\":\"%s\",\"ok\":true,\"value\":\"%u\"}\n", id, (unsigned int)value);
        return 0;
    }
    return spx_print_failure(id, &context, token);
}

static int spx_emit_char_pred(const char *id, spx_char_pred test_case, uint32_t argument) {
    struct spx_status_entry records[UINT32_C(8)];
    struct spx_context context = {0};
    if (!spx_context_init(&context, UINT64_C(512), records, UINT32_C(8), NULL, NULL, NULL)) return 10;
    bool value = false;
    spx_status_token token = test_case(&context, argument, &value);
    if (token == SPX_STATUS_SUCCESS) {
        printf("{\"id\":\"%s\",\"ok\":true,\"value\":%s}\n", id, value ? "true" : "false");
        return 0;
    }
    return spx_print_failure(id, &context, token);
}

static int spx_emit_char_pair(
    const char *id, spx_char_pair test_case, uint32_t left, uint32_t right
) {
    struct spx_status_entry records[UINT32_C(8)];
    struct spx_context context = {0};
    if (!spx_context_init(&context, UINT64_C(513), records, UINT32_C(8), NULL, NULL, NULL)) return 10;
    bool value = false;
    spx_status_token token = test_case(&context, left, right, &value);
    if (token == SPX_STATUS_SUCCESS) {
        printf("{\"id\":\"%s\",\"ok\":true,\"value\":%s}\n", id, value ? "true" : "false");
        return 0;
    }
    return spx_print_failure(id, &context, token);
}

static int spx_emit_f64_pair(
    const char *id, spx_f64_pair test_case, double left, double right
) {
    struct spx_status_entry records[UINT32_C(8)];
    struct spx_context context = {0};
    if (!spx_context_init(&context, UINT64_C(514), records, UINT32_C(8), NULL, NULL, NULL)) return 10;
    double value = -1.0;
    spx_status_token token = test_case(&context, left, right, &value);
    if (token == SPX_STATUS_SUCCESS) {
        union spx_f64_overlay overlay = {0};
        overlay.value = value;
        printf("{\"id\":\"%s\",\"ok\":true,\"value\":\"%016llx\"}\n", id, overlay.bits);
        return 0;
    }
    return spx_print_failure(id, &context, token);
}

static int spx_emit_mixed(
    const char *id, spx_mixed_sig test_case, int32_t first, uint8_t second
) {
    struct spx_status_entry records[UINT32_C(8)];
    struct spx_context context = {0};
    if (!spx_context_init(&context, UINT64_C(515), records, UINT32_C(8), NULL, NULL, NULL)) return 10;
    double value = -1.0;
    spx_status_token token = test_case(&context, first, second, &value);
    if (token == SPX_STATUS_SUCCESS) {
        union spx_f64_overlay overlay = {0};
        overlay.value = value;
        printf("{\"id\":\"%s\",\"ok\":true,\"value\":\"%016llx\"}\n", id, overlay.bits);
        return 0;
    }
    return spx_print_failure(id, &context, token);
}

static int spx_emit_blend(
    const char *id, spx_blend_sig test_case, float narrow, bool flag
) {
    struct spx_status_entry records[UINT32_C(8)];
    struct spx_context context = {0};
    if (!spx_context_init(&context, UINT64_C(516), records, UINT32_C(8), NULL, NULL, NULL)) return 10;
    int64_t value = INT64_C(-7777);
    spx_status_token token = test_case(&context, narrow, flag, &value);
    if (token == SPX_STATUS_SUCCESS) {
        printf("{\"id\":\"%s\",\"ok\":true,\"value\":\"%lld\"}\n", id, (long long)value);
        return 0;
    }
    return spx_print_failure(id, &context, token);
}

int main(void) {
"#
    .to_owned();
    for (index, widen_case) in WIDEN_CASES.iter().enumerate() {
        let symbol = c_symbol(widen_case.symbol_id);
        let call_args = if widen_case.c_args.is_empty() {
            symbol.clone()
        } else {
            format!("{symbol}, {}", widen_case.c_args)
        };
        source.push_str(&format!(
            "    if ({}(\"{}\", {}) != 0) return {};\n",
            widen_case.shape.emitter(),
            widen_case.id,
            call_args,
            100 + index
        ));
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
    // Transcript rows may share declarations across shapes; exports must be
    // unique while preserving first-seen order.
    let mut selected: Vec<String> = Vec::new();
    for widen_case in WIDEN_CASES.iter().filter(|entry| ids.contains(&entry.id)) {
        let symbol_id = widen_case.symbol_id.to_owned();
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
  let values = [];
  if (!(argument === undefined || argument === "")) {
    values = [
      argument === "true" ? true
      : argument === "false" ? false
      : BigInt(argument)
    ];
  }
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
    let case_arguments: Vec<String> = WIDEN_CASES
        .iter()
        .filter(|widen_case| ids.contains(&widen_case.id))
        .map(|widen_case| {
            let argument = match widen_case.args {
                [first, ..] => (*first).to_owned(),
                _ => String::new(),
            };
            format!("{}|{}|{}", widen_case.id, widen_case.symbol_id, argument)
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
// Core evidence: exact backend parity over the widened surface.
// ---------------------------------------------------------------------------

#[test]
fn native_o0_o2_and_core_wasm_match_the_interpreter_over_widened_scalars() {
    if !require_tools_or_skip() {
        return;
    }
    let program = parse(WIDEN_FIXTURE, Path::new("interpreter-widen.spx")).unwrap();
    let diagnostics = verify::verify(&program);
    assert!(
        diagnostics.is_empty(),
        "fixture verification failed: {diagnostics:?}"
    );
    let generated = codegen::emit_c(&program).unwrap();

    let ordinal = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "semaprax-interpreter-widen-parity-{}-{ordinal}",
        std::process::id()
    ));
    std::fs::create_dir(&root).unwrap();

    let native_all_o0 = run_native(&generated, &root, "-O0");
    let native_all_o2 = run_native(&generated, &root, "-O2");

    // The Core-Wasm scalar-export lane requires the whole program to bind
    // only by-value direct i64/bool scalars; that leg runs over WEB_FIXTURE
    // and must match the same transcript rows from every other producer.
    let web_program = parse(WEB_FIXTURE, Path::new("interpreter-widen-web.spx")).unwrap();
    assert!(verify::verify(&web_program).is_empty());
    let core_wasm = run_core_wasm(&web_program, &root, WEB_IDS);

    let _ = std::fs::remove_dir_all(root);

    assert_eq!(
        native_all_o0, native_all_o2,
        "native optimization changed results"
    );
    assert_eq!(
        String::from_utf8(native_all_o0.clone()).unwrap(),
        EXPECTED_WIDEN_TRANSCRIPT,
        "pinned widened transcript drifted"
    );

    // Interpreter over the full widened surface equals native exactly.
    let fixture_path = write_temp(WIDEN_FIXTURE);
    let interpreted_full = interpreter_transcript(&fixture_path);
    cleanup(&fixture_path);
    assert_eq!(
        interpreted_full.into_bytes(),
        native_all_o0,
        "interpreter transcript diverges from both backends"
    );

    // The web subset agrees byte-for-byte with the same lines from every
    // producer: Node/Wasm == native O0 == native O2 == interpreter.
    assert_eq!(core_wasm, transcript_subset(&native_all_o0, WEB_IDS));
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
// Widened argument binding, replay, determinism, and admission guards.
// ---------------------------------------------------------------------------

#[test]
fn widened_arguments_bind_exactly_and_fail_closed_on_mismatch() {
    let path = write_temp(WIDEN_FIXTURE);

    // Bare decimals canonically denote i64: they do not bind to narrower
    // integer parameters even when they fit.
    let errors = interpreter::interpret(
        &path,
        "widen.u8.echo",
        &owned_args(&["255"]),
        &InterpreterOptions::default(),
    )
    .expect_err("bare decimal must not bind to u8");
    assert!(
        errors.iter().any(|item| item.code == "SPX-F103"),
        "{errors:?}"
    );

    let errors = interpreter::interpret(
        &path,
        "widen.recursion.fib",
        &owned_args(&["15"]),
        &InterpreterOptions::default(),
    )
    .expect_err("bare decimal must not bind to i32");
    assert!(
        errors.iter().any(|item| item.code == "SPX-F103"),
        "{errors:?}"
    );

    // Suffixed integers do not bind to wider or unrelated parameters.
    let errors = interpreter::interpret(
        &path,
        "widen.recursion.sum.to",
        &owned_args(&["22i32"]),
        &InterpreterOptions::default(),
    )
    .expect_err("i32 literal must not bind to u8");
    assert!(
        errors.iter().any(|item| item.code == "SPX-F103"),
        "{errors:?}"
    );

    // An unsuffixed float literal denotes f64 and must not bind to f32.
    let errors = interpreter::interpret(
        &path,
        "widen.f32.arith",
        &owned_args(&["9.0"]),
        &InterpreterOptions::default(),
    )
    .expect_err("f64 literal must not bind to f32");
    assert!(
        errors.iter().any(|item| item.code == "SPX-F103"),
        "{errors:?}"
    );

    // Malformed literals stay rejected.
    for hostile in ["1e5", "1.", "inf", "'ab'", "256u8", "7i64"] {
        let errors = interpreter::interpret(
            &path,
            "widen.u8.echo",
            &owned_args(&[hostile]),
            &InterpreterOptions::default(),
        )
        .expect_err(hostile);
        assert!(
            errors.iter().any(|item| item.code == "SPX-F103"),
            "{hostile}: {errors:?}"
        );
    }

    // Exact widened bindings succeed, including mixed signatures.
    let envelope =
        interpret_case(&path, "widen.mixed.select", &["-7i32", "3u8"]).expect("envelope");
    assert!(envelope.contains(
        "\"arguments\":[{\"index\":0,\"name\":\"a\",\"type\":\"i32\",\"value\":\"-7i32\"},\
{\"index\":1,\"name\":\"b\",\"type\":\"u8\",\"value\":\"3u8\"}]"
    ));
    assert!(envelope.contains(
        "\"outcome\":{\"kind\":\"returned\",\"type\":\"f64\",\"value\":\"c004000000000000\"}"
    ));
    cleanup(&path);
}

#[test]
fn widened_envelopes_replay_and_are_deterministic() {
    let path = write_temp(WIDEN_FIXTURE);

    let options = InterpreterOptions::new(65536, DEFAULT_MAX_STEPS).unwrap();
    let arguments = owned_args(&["150.25", "2.0"]);
    let first =
        interpreter::interpret(&path, "widen.f64.arith", &arguments, &options).expect("envelope");
    let second =
        interpreter::interpret(&path, "widen.f64.arith", &arguments, &options).expect("envelope");
    assert_eq!(
        first.envelope, second.envelope,
        "generation is deterministic"
    );
    interpreter::verify_envelope(&first.envelope).expect("widened envelope verifies");
    interpreter::verify_envelope_against_source(&first.envelope, &path)
        .expect("source binding holds");

    // A failed widened outcome replays through the closed status table too.
    let failed = interpret_case(&path, "widen.u8.sub.boundary", &[]).expect("envelope");
    interpreter::verify_envelope(&failed).expect("failed widened envelope verifies");
    assert!(
        failed.contains(
            "\"outcome\":{\"kind\":\"failed\",\"status\":{\"schema\":\"semaprax.status.v1\",\
\"domain_id\":\"semaprax.arithmetic.v1\",\"code\":2,"
        ),
        "{failed}"
    );

    // Canonical char rendering survives replay exactly.
    let snowman = interpret_case(&path, "widen.char.echo", &[r"'\u{2603}'"]).expect("envelope");
    let parsed: serde_json::Value = serde_json::from_str(&snowman).unwrap();
    assert_eq!(parsed["payload"]["outcome"]["type"], "char", "{snowman}");
    assert_eq!(
        parsed["payload"]["outcome"]["value"], r"'\u{2603}'",
        "{snowman}"
    );
    interpreter::verify_envelope(&snowman).expect("char envelope verifies");
    cleanup(&path);
}

#[test]
fn scope_guards_keep_strings_aggregates_and_generics_closed() {
    let guarded = r#"
module test.interpreter_widen_guard;

@id("guard.point")
record Point {
    @id("point.x") x: i64,
    @id("point.y") y: i64,
}

@id("guard.string.param")
fn string_param(value: string) -> i64 { 0 }

@id("guard.string.result")
fn string_result() -> string { "outside" }

@id("guard.record.result")
fn record_result() -> Point { Point { x: 1, y: 2 } }

@id("guard.generic")
fn generic<T>(value: T) -> i64 { 0 }

@id("app.main")
fn main() -> i64 { 0 }
"#;
    let path = write_temp(guarded);
    let expects_reason = |token: &str, reason: &str| {
        let errors = interpreter::interpret(&path, token, &[], &InterpreterOptions::default())
            .expect_err(reason);
        assert!(
            errors
                .iter()
                .any(|item| item.code == "SPX-F102" && item.message.contains(reason)),
            "{token}: {errors:?}"
        );
    };
    expects_reason("guard.string.param", "unsupported_parameter_type");
    expects_reason("guard.string.result", "unsupported_result_type");
    expects_reason("guard.record.result", "unsupported_result_type");
    expects_reason("guard.generic", "generic_function");
    cleanup(&path);
}
