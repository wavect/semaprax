//! Executable evidence for the Copy-scalar widening of the Public Scalar
//! Export Profile v1.
//!
//! The profile admits `i64`, `i32`, `u8`, `char`, `f32`, `f64`, and `bool` —
//! the same surface the reference interpreter, the interop projections, and the
//! schema projections already admit, minus `usize`. This file proves that the
//! exported adapter reuses the monomorphic callee's interned Core-Wasm type
//! rather than a converted one, that the export edge traps on every host value
//! its SEMAPRAX parameter type cannot contain, that the generated
//! JavaScript/TypeScript facade describes and enforces the same surface, and
//! that widening is additive: an `i64`/`bool` program renders the frozen v1
//! facade with no widened material anywhere in it.
//!
//! Node executes the real boundary through
//! `scripts/verify-wasm-scalar-widening.mjs`; it is skipped when no `node` is
//! on the path, exactly as the profile's own package evidence skips it.

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use semaprax::{hir, parse, verify, wasm};
use wasmparser::{ExternalKind, Parser, Payload, ValType};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

const CALCULATOR: &str = include_str!("../../examples/calculator.spx");

/// One function per admitted scalar plus one mixed signature, so a single
/// module proves the whole widened surface and their coexistence.
const WIDENED_SOURCE: &str = r#"
module test.wasmscalarwiden;

@id("widen.bool")
fn pick_bool(value: bool) -> bool { value }

@id("widen.char")
fn pick_char(value: char) -> char { value }

@id("widen.f32")
fn pick_f32(value: f32) -> f32 { value }

@id("widen.f64")
fn pick_f64(value: f64) -> f64 { value }

@id("widen.i32")
fn pick_i32(value: i32) -> i32 { value }

@id("widen.i64")
fn pick_i64(value: i64) -> i64 { value }

@id("widen.mixed")
fn mixed(flag: bool, count: i64, small: u8, medium: i32, code: char, ratio: f32) -> f64 { 2.5 }

@id("widen.u8")
fn pick_u8(value: u8) -> u8 { value }

@id("app.main")
fn main() -> i64 { 0 }
"#;

const WIDENED_IDS: &[&str] = &[
    "widen.bool",
    "widen.char",
    "widen.f32",
    "widen.f64",
    "widen.i32",
    "widen.i64",
    "widen.mixed",
    "widen.u8",
];

const CALCULATOR_IDS: &[&str] = &[
    "calculator.add",
    "calculator.divide",
    "calculator.is-negative",
    "calculator.multiply",
    "calculator.not",
    "calculator.subtract",
];

/// The frozen v1 `argument` and `result` guards. Widening inserts new guards
/// between the `i64` guard and the `bool` fall-through, so these two blocks
/// must still appear verbatim in every generated facade.
const FROZEN_ARGUMENT_GUARD: &str = "function argument(value, type, index) {\n  if (type === \"i64\") {\n    if (typeof value !== \"bigint\" || value < SPX_MIN || value > SPX_MAX) throw new TypeError(`argument ${index} must be a signed 64-bit bigint`);\n    return value;\n  }\n";
const FROZEN_BOOL_ARGUMENT_GUARD: &str = "  if (typeof value !== \"boolean\") throw new TypeError(`argument ${index} must be boolean`);\n  return value ? 1 : 0;\n}\n";
const FROZEN_BOOL_RESULT_GUARD: &str = "  if (value !== 0 && value !== 1) throw new TypeError(\"SEMAPRAX adapter returned non-canonical bool\");\n  return value === 1;\n}\n";

const WIDENED_GUARD_MARKERS: &[&str] = &[
    "if (type === \"i32\")",
    "if (type === \"u8\")",
    "if (type === \"char\")",
    "if (type === \"f32\")",
    "if (type === \"f64\")",
];

fn selection(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|id| (*id).to_owned()).collect()
}

fn resolved(source: &str) -> hir::ResolvedProgram {
    let path = std::path::Path::new("wasm-scalar-export-widen.spx");
    let program = parse(source, path).expect("widening fixture parses");
    let diagnostics = verify::verify(&program);
    assert!(
        diagnostics.iter().all(|item| !item.severity.is_error()),
        "widening fixture must verify: {diagnostics:?}"
    );
    hir::resolve(&program).expect("widening fixture resolves")
}

fn temporary(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "semaprax-wasm-scalar-widen-{label}-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ))
}

struct Module {
    types: Vec<(Vec<ValType>, Vec<ValType>)>,
    function_types: Vec<u32>,
    function_exports: Vec<(String, u32)>,
    imports: usize,
}

fn inspect(bytes: &[u8]) -> Module {
    let mut module = Module {
        types: Vec::new(),
        function_types: Vec::new(),
        function_exports: Vec::new(),
        imports: 0,
    };
    for payload in Parser::new(0).parse_all(bytes) {
        match payload.expect("emitted module parses") {
            Payload::TypeSection(section) => {
                module
                    .types
                    .extend(section.into_iter_err_on_gc_types().map(|ty| {
                        let ty = ty.unwrap();
                        (ty.params().to_vec(), ty.results().to_vec())
                    }));
            }
            Payload::ImportSection(section) => module.imports = section.count() as usize,
            Payload::FunctionSection(section) => {
                module
                    .function_types
                    .extend(section.into_iter().map(Result::unwrap));
            }
            Payload::ExportSection(section) => {
                for export in section {
                    let export = export.unwrap();
                    if export.kind == ExternalKind::Func {
                        module
                            .function_exports
                            .push((export.name.to_owned(), export.index));
                    }
                }
            }
            _ => {}
        }
    }
    module
}

fn raw_symbol(stable_id: &str) -> String {
    let mut symbol = String::from("spx_scalar_");
    for byte in stable_id.bytes() {
        symbol.push_str(&format!("{byte:02x}"));
    }
    symbol
}

/// The exported adapter must reuse the monomorphic callee's interned type, not
/// a converted one. Type interning is by structural signature, so an equal
/// type index is exact proof that the public export signature is the same
/// Core-Wasm value-type lowering the verified body already uses.
#[test]
fn every_admitted_scalar_exports_the_callee_value_type_exactly() {
    let program = resolved(WIDENED_SOURCE);
    let bytes =
        wasm::emit_resolved_module_with_scalar_exports(&program, &selection(WIDENED_IDS)).unwrap();
    let module = inspect(&bytes);

    let executable = program.functions.len();
    assert_eq!(module.function_types.len(), executable + WIDENED_IDS.len());
    assert_eq!(
        module
            .function_exports
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        WIDENED_IDS
            .iter()
            .map(|id| raw_symbol(id))
            .collect::<Vec<_>>()
    );

    for (ordinal, stable_id) in WIDENED_IDS.iter().enumerate() {
        let callee = program
            .functions
            .iter()
            .position(|function| function.id.as_str() == *stable_id)
            .expect("selected identity is a monomorphic function");
        let adapter = executable + ordinal;
        assert_eq!(
            module.function_types[adapter], module.function_types[callee],
            "adapter for {stable_id} does not reuse the callee's interned type"
        );
        let (_, index) = &module.function_exports[ordinal];
        assert_eq!(
            *index as usize,
            module.imports + adapter,
            "adapter export for {stable_id} names the wrong function"
        );
    }

    let expected: &[(&str, &[ValType], ValType)] = &[
        ("widen.bool", &[ValType::I32], ValType::I32),
        ("widen.char", &[ValType::I32], ValType::I32),
        ("widen.f32", &[ValType::F32], ValType::F32),
        ("widen.f64", &[ValType::F64], ValType::F64),
        ("widen.i32", &[ValType::I32], ValType::I32),
        ("widen.i64", &[ValType::I64], ValType::I64),
        (
            "widen.mixed",
            &[
                ValType::I32,
                ValType::I64,
                ValType::I32,
                ValType::I32,
                ValType::I32,
                ValType::F32,
            ],
            ValType::F64,
        ),
        ("widen.u8", &[ValType::I32], ValType::I32),
    ];
    for (ordinal, (stable_id, params, result)) in expected.iter().enumerate() {
        let signature = &module.types[module.function_types[executable + ordinal] as usize];
        assert_eq!(signature.0, *params, "parameter lowering for {stable_id}");
        assert_eq!(
            signature.1,
            vec![*result],
            "result lowering for {stable_id}"
        );
    }
}

/// Widening is additive. An `i64`/`bool` program is still admitted, still
/// renders the frozen v1 facade, and carries no widened guard, ABI spelling, or
/// TypeScript type anywhere in its generated package.
#[test]
fn i64_and_bool_packages_render_the_frozen_v1_facade() {
    let program = parse(CALCULATOR, std::path::Path::new("calculator.spx")).unwrap();
    let output = temporary("frozen");
    wasm::build_web_with_scalar_exports(&program, &output, &selection(CALCULATOR_IDS)).unwrap();

    let bindings = std::fs::read_to_string(output.join("semaprax.bindings.js")).unwrap();
    for frozen in [
        FROZEN_ARGUMENT_GUARD,
        FROZEN_BOOL_ARGUMENT_GUARD,
        FROZEN_BOOL_RESULT_GUARD,
    ] {
        assert!(
            bindings.contains(frozen),
            "the frozen v1 guard is no longer rendered verbatim"
        );
    }
    for marker in WIDENED_GUARD_MARKERS {
        assert!(
            !bindings.contains(marker),
            "an i64/bool package must not carry the {marker} guard"
        );
    }

    let declarations = std::fs::read_to_string(output.join("semaprax.bindings.d.ts")).unwrap();
    assert!(!declarations.contains(": number"));
    let manifest = std::fs::read_to_string(output.join("semaprax.scalar-exports.json")).unwrap();
    for widened in ["\"i32\"", "\"u8\"", "\"char\"", "\"f32\"", "\"f64\""] {
        assert!(
            !manifest.contains(widened),
            "an i64/bool manifest must not name {widened}"
        );
    }

    let _ = std::fs::remove_dir_all(output);
}

/// The manifest ABI, the TypeScript call signatures, and the JavaScript guards
/// describe one surface. `i64` alone stays a `bigint` because it is the only
/// admitted scalar whose range exceeds an exact JavaScript number.
#[test]
fn generated_package_projects_the_widened_surface_consistently() {
    let program = parse(WIDENED_SOURCE, std::path::Path::new("widen.spx")).unwrap();
    let output = temporary("package");
    wasm::build_web_with_scalar_exports(&program, &output, &selection(WIDENED_IDS)).unwrap();

    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(output.join("semaprax.scalar-exports.json")).unwrap(),
    )
    .unwrap();
    let functions = manifest["scalar_abi"]["functions"].as_array().unwrap();
    assert_eq!(functions.len(), WIDENED_IDS.len());
    assert_eq!(functions[6]["stable_id"], "widen.mixed");
    assert_eq!(
        functions[6]["parameters"],
        serde_json::json!(["bool", "i64", "u8", "i32", "char", "f32"])
    );
    assert_eq!(functions[6]["result"], "f64");
    assert_eq!(functions[1]["parameters"], serde_json::json!(["char"]));
    assert_eq!(functions[1]["result"], "char");

    let declarations = std::fs::read_to_string(output.join("semaprax.bindings.d.ts")).unwrap();
    assert!(declarations.contains(
        "readonly \"widen.mixed\": (arg0: boolean, arg1: bigint, arg2: number, arg3: number, arg4: number, arg5: number) => ScalarResult<number>;"
    ));
    assert!(
        declarations.contains("readonly \"widen.char\": (arg0: number) => ScalarResult<number>;")
    );
    assert!(
        declarations.contains("readonly \"widen.i64\": (arg0: bigint) => ScalarResult<bigint>;")
    );

    let bindings = std::fs::read_to_string(output.join("semaprax.bindings.js")).unwrap();
    for marker in WIDENED_GUARD_MARKERS {
        assert!(bindings.contains(marker), "missing the {marker} guard");
    }
    // The frozen guards keep their exact position around the widened ones.
    assert!(bindings.contains(FROZEN_ARGUMENT_GUARD));
    assert!(bindings.contains(FROZEN_BOOL_ARGUMENT_GUARD));
    assert!(bindings.contains(FROZEN_BOOL_RESULT_GUARD));

    let _ = std::fs::remove_dir_all(output);
}

/// A package renders only the guards for the scalars it actually projects, so
/// the widening adds nothing to a facade that cannot use it.
#[test]
fn guards_are_rendered_only_for_projected_scalars() {
    let program = parse(WIDENED_SOURCE, std::path::Path::new("widen.spx")).unwrap();
    let output = temporary("subset");
    wasm::build_web_with_scalar_exports(&program, &output, &selection(&["widen.f64", "widen.i64"]))
        .unwrap();

    let bindings = std::fs::read_to_string(output.join("semaprax.bindings.js")).unwrap();
    assert!(bindings.contains("if (type === \"f64\")"));
    for absent in [
        "if (type === \"i32\")",
        "if (type === \"u8\")",
        "if (type === \"char\")",
        "if (type === \"f32\")",
    ] {
        assert!(!bindings.contains(absent), "unused guard {absent} rendered");
    }

    let _ = std::fs::remove_dir_all(output);
}

/// Widened emission stays deterministic.
#[test]
fn widened_emission_is_byte_deterministic() {
    let program = resolved(WIDENED_SOURCE);
    let first =
        wasm::emit_resolved_module_with_scalar_exports(&program, &selection(WIDENED_IDS)).unwrap();
    let second =
        wasm::emit_resolved_module_with_scalar_exports(&program, &selection(WIDENED_IDS)).unwrap();
    assert_eq!(first, second);
}

/// The closed exclusion vocabulary still holds beside the widened admissions.
/// `usize` in particular stays out: its width is a host fact, not a public fact
/// of this profile.
#[test]
fn excluded_shapes_still_fail_closed() {
    let excluded = [
        (
            "usize",
            "@id(\"x.f\") fn f(value: usize) -> usize { value }",
        ),
        ("string", "@id(\"x.f\") fn f(value: string) -> i64 { 0 }"),
        (
            "borrowed str",
            "@id(\"x.f\") fn f(value: borrow str) -> i64 { 0 }",
        ),
        (
            "borrowed byte slice",
            "@id(\"x.f\") fn f(value: borrow Slice<u8>) -> i64 { 0 }",
        ),
        (
            "owned bytes",
            "@id(\"x.f\") fn f(value: own Bytes) -> i64 { 0 }",
        ),
    ];
    for (label, declaration) in excluded {
        let source = format!(
            "module test.excluded;\n{declaration}\n@id(\"x.main\") fn main() -> i64 {{ 0 }}\n"
        );
        // Each case must reach the profile gate: a shape rejected earlier by
        // the parser or the resolver would prove nothing about admission.
        let program = parse(&source, std::path::Path::new("excluded.spx"))
            .unwrap_or_else(|error| panic!("{label} must parse: {error:?}"));
        let program = hir::resolve(&program)
            .unwrap_or_else(|error| panic!("{label} must resolve: {error:?}"));
        let error = wasm::emit_resolved_module_with_scalar_exports(&program, &selection(&["x.f"]))
            .expect_err(&format!("{label} must stay outside the profile"));
        assert!(
            matches!(error.code, "SPX-W115" | "SPX-W116"),
            "{label} rejected with {}",
            error.code
        );
    }
}

/// Node executes the generated facade and the raw adapters. This is the only
/// evidence here that the emitted traps actually fire.
#[test]
fn node_executes_the_widened_boundary() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let program = parse(WIDENED_SOURCE, std::path::Path::new("widen.spx")).unwrap();
    let output = temporary("node");
    wasm::build_web_with_scalar_exports(&program, &output, &selection(WIDENED_IDS)).unwrap();

    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scripts/verify-wasm-scalar-widening.mjs");
    let result = Command::new("node")
        .arg(script)
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "node failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&result.stdout).trim(),
        "scalar-widening-v1-ok"
    );

    let _ = std::fs::remove_dir_all(output);
}
