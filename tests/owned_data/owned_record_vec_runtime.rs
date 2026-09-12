//! SPX-AI-020 (issue #119): the bounded owned-record collection profile, run
//! from committed source on every backend that claims to implement it.
//!
//! The element is the catalog-normalizer payload shape — two owned `Bytes`
//! leaves and one Copy scalar — inside the compiler-owned `Vec` carrier. The
//! reference interpreter, the native C11 backend and Core Wasm all execute it
//! per element, and all three must publish the same value or select the same
//! status. Backend agreement here is real executed values on every target, not
//! agreement by refusal: no target refuses the profile any more.
//!
//! Every case below is a real `.spx` program driven through `semaprax::check`
//! and `interpreter::interpret`, through `codegen::emit_c`, `clang -O0`/`-O2`
//! and an executed binary, and through `wasm::emit_module` and an instantiated
//! module under Node — not a hand-built plan.

use semaprax::{codegen, hir, interpreter};

#[path = "owned_record_vec_runtime/native.rs"]
mod native;
#[path = "owned_record_vec_runtime/wasm.rs"]
mod wasm;

/// The admitted element declaration, shared by every fixture.
const DECLARATION: &str = r#"module app.catalog;
@id("app.catalog.line") record Line {
 @id("app.catalog.line.id") id: Bytes,
 @id("app.catalog.line.label") label: Bytes,
 @id("app.catalog.line.quantity") quantity: i64,
}
"#;

/// A small application fragment: accumulate three catalog lines into a bounded
/// carrier, observe its length and capacity, drop every element with `clear`
/// while retaining capacity, and reuse the same carrier afterwards.
const ACCUMULATE: &str = r#"@id("app.main") fn main()->i64 {
 let lines=vec_with_capacity<Line>(3usize);
 let one=vec_push<Line>(lines,Line{id:bytes_zeroed(4usize),label:bytes_zeroed(8usize),quantity:11});
 let two=vec_push<Line>(one,Line{id:bytes_zeroed(4usize),label:bytes_zeroed(8usize),quantity:22});
 let three=vec_push<Line>(two,Line{id:bytes_zeroed(4usize),label:bytes_zeroed(8usize),quantity:33});
 let filled=vec_len<Line>(three)==3usize && vec_capacity<Line>(three)==3usize;
 let cleared=vec_clear<Line>(three);
 let emptied=vec_len<Line>(cleared)==0usize && vec_capacity<Line>(cleared)==3usize;
 let reused=vec_push<Line>(cleared,Line{id:bytes_zeroed(4usize),label:bytes_zeroed(8usize),quantity:44});
 if filled && emptied && vec_len<Line>(reused)==1usize && vec_capacity<Line>(reused)==3usize {29}else{0}
}
"#;

/// The empty boundary: a zero-capacity carrier owns nothing, reports nothing,
/// and still settles.
const EMPTY: &str = r#"@id("app.main") fn main()->i64 {
 let lines=vec_with_capacity<Line>(0usize);
 let cleared=vec_clear<Line>(lines);
 if vec_len<Line>(cleared)==0usize && vec_capacity<Line>(cleared)==0usize {29}else{0}
}
"#;

/// A single element at exactly full capacity.
const SINGLETON_AT_CAPACITY: &str = r#"@id("app.main") fn main()->i64 {
 let lines=vec_with_capacity<Line>(1usize);
 let one=vec_push<Line>(lines,Line{id:bytes_zeroed(1usize),label:bytes_zeroed(2usize),quantity:7});
 if vec_len<Line>(one)==1usize && vec_capacity<Line>(one)==1usize {29}else{0}
}
"#;

/// Pushing past capacity selects sticky `semaprax.vec.v1` code 1. The vector
/// and the staged record both settle, and no value is published.
const PUSH_AT_FULL_CAPACITY: &str = r#"@id("app.main") fn main()->i64 {
 let lines=vec_with_capacity<Line>(1usize);
 let one=vec_push<Line>(lines,Line{id:bytes_zeroed(1usize),label:bytes_zeroed(2usize),quantity:7});
 let two=vec_push<Line>(one,Line{id:bytes_zeroed(3usize),label:bytes_zeroed(4usize),quantity:9});
 if vec_len<Line>(two)==2usize {29}else{0}
}
"#;

/// A dynamic capacity inside the shared 8192-element bound but past this
/// profile's own owned-payload bound (two `Bytes` leaves per element) selects
/// the construction-allocation failure, code 3, rather than over-committing.
const OVERSIZED_OWNED_PAYLOAD: &str = r#"@id("app.main") fn main()->i64 {
 let requested=8192usize;
 let lines=vec_with_capacity<Line>(requested);
 if vec_capacity<Line>(lines)==requested {29}else{0}
}
"#;

fn source(body: &str) -> String {
    format!("{DECLARATION}{body}")
}

fn fixture_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "owned-record-vec-{}-{}.spx",
        std::process::id(),
        name
    ))
}

/// One interpreter run of one fixture, returning either the published value or
/// the selected status domain and code.
fn interpret_once(name: &str, source: &str) -> Result<i64, (String, u64)> {
    let path = fixture_path(name);
    semaprax::check(source, &path).expect("the source verifier must admit the profile");
    std::fs::write(&path, source).unwrap();
    let outcome = interpreter::interpret(
        &path,
        "app.main",
        &[],
        &interpreter::InterpreterOptions::default(),
    )
    .expect("the interpreter must admit the profile");
    interpreter::verify_envelope(&outcome.envelope).unwrap();
    let envelope: serde_json::Value = serde_json::from_str(&outcome.envelope).unwrap();
    let _ = std::fs::remove_file(&path);
    if outcome.returned {
        Ok(envelope["payload"]["outcome"]["value"]
            .as_str()
            .expect("a returned i64 is published as text")
            .parse()
            .unwrap())
    } else {
        let status = &envelope["payload"]["outcome"]["status"];
        Err((
            status["domain_id"].as_str().unwrap().to_owned(),
            status["code"].as_u64().unwrap(),
        ))
    }
}

/// The interpreter executes the whole corpus, deterministically: every fixture
/// is run four times and must publish the same outcome each time.
#[test]
fn the_interpreter_executes_the_owned_record_collection_corpus() {
    let cases: [(&str, &str, Result<i64, (&str, u64)>); 7] = [
        ("accumulate", ACCUMULATE, Ok(29)),
        ("empty", EMPTY, Ok(29)),
        ("singleton", SINGLETON_AT_CAPACITY, Ok(29)),
        ("full", PUSH_AT_FULL_CAPACITY, Err(("semaprax.vec.v1", 1))),
        (
            "oversized",
            OVERSIZED_OWNED_PAYLOAD,
            Err(("semaprax.vec.v1", 3)),
        ),
        (
            "precondition",
            &ACCUMULATE.replace("fn main()->i64 {", "fn main()->i64 requires false {"),
            Err(("semaprax.contract.v1", 1)),
        ),
        (
            "postcondition",
            &ACCUMULATE.replace("fn main()->i64 {", "fn main()->i64 ensures false {"),
            Err(("semaprax.contract.v1", 2)),
        ),
    ];
    for (name, body, expected) in cases {
        let program = source(body);
        for _ in 0..4 {
            let observed = interpret_once(name, &program);
            let expected = expected.map_err(|(domain, code)| (domain.to_owned(), code));
            assert_eq!(observed, expected, "fixture `{name}` diverged");
        }
    }
}

/// Sticky failure selection, observed rather than argued: the profile's own
/// dynamic failure is published, and the per-element and carrier cleanup that
/// follows it never replaces the selected status with a value.
#[test]
fn a_selected_vec_failure_survives_the_cleanup_that_follows_it() {
    let program = source(PUSH_AT_FULL_CAPACITY);
    assert_eq!(
        interpret_once("sticky", &program),
        Err(("semaprax.vec.v1".to_owned(), 1)),
        "cleanup cannot replace the selected status"
    );
    // The same program with room for the second push publishes a value, so the
    // failure above is the push itself and not an unrelated refusal.
    let widened = program.replace(
        "vec_with_capacity<Line>(1usize)",
        "vec_with_capacity<Line>(2usize)",
    );
    assert_eq!(interpret_once("sticky-widened", &widened), Ok(29));
}

/// Core Wasm executes the whole corpus and must publish exactly what the
/// reference interpreter and the native C11 lane publish above.
///
/// The Wasm carrier is not in linear memory: it is one `i64` host handle behind
/// the owned-payload import boundary, so the settlement probe is host-side.
/// Every fixture is invoked four times; after each invocation the host must
/// hold zero live vector handles and zero live `Bytes` handles, and must have
/// dropped exactly as many payloads as it allocated — the host errors on a
/// double drop, so each element's two leaves are proven dropped exactly once.
/// The ordered element scalars the host was handed are checked too, which is
/// what distinguishes real per-element record storage from a carrier that
/// stored nothing.
#[test]
fn core_wasm_executes_the_owned_record_collection_corpus() {
    let full = [11, 22, 33, 44];
    let cases: [(&str, u32, i64, u32, &[i64]); 7] = [
        (ACCUMULATE, 0, 29, 8, &full),
        (EMPTY, 0, 29, 0, &[]),
        (SINGLETON_AT_CAPACITY, 0, 29, 2, &[7]),
        (PUSH_AT_FULL_CAPACITY, WASM_STATUS_VEC_PUSH_FULL, 0, 4, &[7]),
        (
            OVERSIZED_OWNED_PAYLOAD,
            WASM_STATUS_VEC_ALLOCATION_FAILURE,
            0,
            0,
            &[],
        ),
        (
            &ACCUMULATE.replace("fn main()->i64 {", "fn main()->i64 requires false {"),
            WASM_STATUS_REQUIRES_FALSE,
            0,
            0,
            &[],
        ),
        (
            &ACCUMULATE.replace("fn main()->i64 {", "fn main()->i64 ensures false {"),
            WASM_STATUS_ENSURES_FALSE,
            0,
            8,
            &full,
        ),
    ];
    for (body, status, value, copies, scalars) in cases {
        wasm::run_wasm(&source(body), status, value, copies, scalars, "none");
    }
}

/// The same injected carrier-allocation failure the native lane settles, at the
/// Wasm host boundary: the record `with_capacity` refuses, the profile's own
/// allocation status is selected, no value is published, and nothing stays live.
#[test]
fn core_wasm_settles_injected_carrier_allocation_failure() {
    wasm::run_wasm(
        &source(ACCUMULATE),
        WASM_STATUS_VEC_ALLOCATION_FAILURE,
        0,
        0,
        &[],
        "allocation",
    );
}

/// The Wasm status integers the module selects, mapped to the status domains
/// and codes the reference interpreter publishes for the same fixtures:
/// `semaprax.vec.v1` 1 and 3, and `semaprax.contract.v1` 1 and 2.
const WASM_STATUS_REQUIRES_FALSE: u32 = 9;
const WASM_STATUS_ENSURES_FALSE: u32 = 10;
const WASM_STATUS_VEC_PUSH_FULL: u32 = 13;
const WASM_STATUS_VEC_ALLOCATION_FAILURE: u32 = 15;

/// The native C11 lane executes the whole corpus, at `-O0` and `-O2`, and must
/// publish exactly what the reference interpreter publishes above. Each case
/// additionally proves zero live heap allocations and zero live carrier
/// authority entries after four consecutive invocations, on success and after
/// injected failure alike.
#[test]
fn native_c11_executes_the_owned_record_collection_corpus() {
    let cases: [(&str, Result<i64, (&str, u32)>); 7] = [
        (ACCUMULATE, Ok(29)),
        (EMPTY, Ok(29)),
        (SINGLETON_AT_CAPACITY, Ok(29)),
        (PUSH_AT_FULL_CAPACITY, Err(("semaprax.vec.v1", 1))),
        (OVERSIZED_OWNED_PAYLOAD, Err(("semaprax.vec.v1", 3))),
        (
            &ACCUMULATE.replace("fn main()->i64 {", "fn main()->i64 requires false {"),
            Err(("semaprax.contract.v1", 1)),
        ),
        (
            &ACCUMULATE.replace("fn main()->i64 {", "fn main()->i64 ensures false {"),
            Err(("semaprax.contract.v1", 2)),
        ),
    ];
    for (body, expected) in cases {
        let program = source(body);
        match expected {
            Ok(value) => native::run_native(&program, "", 0, value, "none"),
            Err((domain, code)) => native::run_native(&program, domain, code, 0, "none"),
        }
    }
}

/// Negative control for the governing invariant: ownership errors are
/// compile-time diagnostics, never backend accidents. An element one field
/// away from the admitted shape reaches neither the record lowering nor the
/// scalar lowering; the front end refuses it with a stable diagnostic, and
/// neither backend emitter is ever asked to place a carrier it has no layout
/// for. Both emitters additionally re-derive admission from declaration facts
/// at their own emission boundary, so a forged or widened HIR that got past the
/// front end is a diagnostic there too, never a panic.
#[test]
fn an_element_outside_the_admitted_shape_is_refused_with_a_stable_diagnostic() {
    let near_miss = r#"module app.catalog;
@id("app.catalog.line") record Line {
 @id("app.catalog.line.id") id: Bytes,
 @id("app.catalog.line.label") label: Bytes,
 @id("app.catalog.line.note") note: Bytes,
}
@id("app.main") fn main()->i64 {
 let lines=vec_with_capacity<Line>(1usize);
 if vec_capacity<Line>(lines)==1usize {29}else{0}
}
"#;
    let path = fixture_path("near-miss");
    let diagnostics = semaprax::check(near_miss, &path).expect_err("the near miss must be refused");
    assert!(
        diagnostics.iter().all(|diagnostic| {
            !diagnostic.code.is_empty() && diagnostic.code.starts_with("SPX-")
        }),
        "every refusal must carry a stable code: {diagnostics:?}"
    );
    let parsed = semaprax::parse(near_miss, std::path::Path::new("owned-record-vec.spx")).unwrap();
    match hir::resolve(&parsed) {
        Err(diagnostics) => assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code.starts_with("SPX-"))),
        Ok(program) => {
            let native = codegen::emit_hir_c(&program)
                .expect_err("native must refuse an inadmissible element");
            assert!(native.code.starts_with("SPX-"), "{native:?}");
            let emitted = semaprax::wasm::emit_resolved_module(&program)
                .expect_err("Wasm must refuse an inadmissible element");
            assert!(emitted.code.starts_with("SPX-"), "{emitted:?}");
        }
    }
}

/// Allocation failure injected at the one site this profile allocates a
/// carrier: the element array. It selects the profile's own allocation
/// status, publishes no value, and leaves nothing live.
///
/// A refused owned-`Bytes` leaf allocation is deliberately not a case here:
/// `byte_ops` keeps physical allocation failure invariant fail-stop rather
/// than a selected status, so injecting it would assert an abort, not a
/// settlement. The staged-element settlement it would probe is covered by
/// `PUSH_AT_FULL_CAPACITY` above, whose refused push has a fully constructed
/// two-leaf element live at the moment of failure.
#[test]
fn native_c11_settles_injected_carrier_allocation_failure() {
    native::run_native(&source(ACCUMULATE), "semaprax.vec.v1", 3, 0, "carrier");
}
