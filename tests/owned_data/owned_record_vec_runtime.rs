//! SPX-AI-020 (issue #119): the bounded owned-record collection profile, run
//! from committed source on every backend that claims to implement it.
//!
//! The element is the catalog-normalizer payload shape — two owned `Bytes`
//! leaves and one Copy scalar — inside the compiler-owned `Vec` carrier. The
//! reference interpreter executes it per element; native C11 (`SPX-B115`) and
//! Core Wasm (`SPX-W125`) do not implement the carrier yet and refuse the same
//! source up front with their own stable diagnostic. Backend agreement here is
//! therefore agreement-by-refusal for the two targets that claim nothing, and
//! real executed values for the one that does.
//!
//! Every case below is a real `.spx` program driven through `semaprax::check`
//! and `interpreter::interpret`, not a hand-built plan.

use semaprax::{codegen, hir, interpreter, wasm};

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

fn resolved(source: &str) -> hir::ResolvedProgram {
    let parsed = semaprax::parse(source, std::path::Path::new("owned-record-vec.spx")).unwrap();
    hir::resolve(&parsed).expect("the profile must resolve")
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

/// Backend agreement while conformance is partial. Native C11 and Core Wasm do
/// not implement this carrier, so they refuse the exact same source the
/// interpreter executes, each with its own stable diagnostic, rather than
/// emitting a carrier they cannot lower. When either lifts, it must agree with
/// the interpreter's values above, not merely stop refusing.
#[test]
fn native_and_wasm_refuse_the_same_source_the_interpreter_executes() {
    for (name, body) in [
        ("accumulate", ACCUMULATE),
        ("empty", EMPTY),
        ("singleton", SINGLETON_AT_CAPACITY),
    ] {
        let program = resolved(&source(body));
        let native = codegen::emit_hir_c(&program)
            .err()
            .unwrap_or_else(|| panic!("native must refuse `{name}`"));
        assert_eq!(native.code, "SPX-B115", "fixture `{name}`");
        let emitted = wasm::emit_resolved_module(&program)
            .err()
            .unwrap_or_else(|| panic!("Wasm must refuse `{name}`"));
        assert_eq!(emitted.code, "SPX-W125", "fixture `{name}`");
    }
}
