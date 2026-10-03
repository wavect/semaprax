//! Retained affine source captures use an owned carrier, not lexical substitution.
use super::*;

const SOURCE: &str = r#"
module test.affine_capture;
@id("once.consume") fn consume(payload: own Bytes) -> i64 { 42 }
@id("once.make") fn make() -> FnOnce() -> i64 ensures true {
    let payload = bytes_zeroed(4usize);
    once fn() -> i64 { consume(payload) }
}
@id("once.forward") fn forward(callback: own FnOnce() -> i64) -> FnOnce() -> i64 {
    callback
}
@id("once.run") fn run(callback: own FnOnce() -> i64) -> i64 { callback() }
@id("app.main") fn main() -> i64 {
    let callback = make();
    let moved = forward(callback);
    run(moved)
}
"#;

#[test]
fn affine_capture_retained_owner_executes_and_settles_on_all_backends() {
    assert!(command_available("clang"), "physical C compiler required");
    assert!(command_available("node"), "physical Wasm runtime required");
    for (label, source, expected) in [
        ("affine-called", SOURCE.to_owned(), 42),
        (
            "affine-unused",
            SOURCE.replace("    run(moved)", "    99"),
            99,
        ),
    ] {
        let program = checked_program(&source, label);
        let canonical = semaprax::format::canonical(&program);
        let roundtrip = checked_program(&canonical, label);
        assert_eq!(canonical, semaprax::format::canonical(&roundtrip));
        let graph = semaprax::graph::to_json(&program).unwrap();
        semaprax::graph::verify_json(&program, &graph).unwrap();
        assert!(graph.contains("semaprax.graph.v62"));
        assert!(graph.contains("affine_function"));
        assert!(graph.contains("core.fn_once.construct"));
        assert!(graph.contains("core.fn_once.drop"));
        let generated = codegen::emit_c(&program).unwrap();
        let mut failures = Vec::new();
        for (engine, result) in [
            (
                "interpreter",
                std::panic::catch_unwind(|| assert_eq!(run_interpreter(&source, label), expected)),
            ),
            (
                "native O0",
                std::panic::catch_unwind(|| {
                    assert_eq!(
                        run_native(&generated, "-O0", label, 36701),
                        (expected, 1, 1)
                    )
                }),
            ),
            (
                "native O2",
                std::panic::catch_unwind(|| {
                    assert_eq!(
                        run_native(&generated, "-O2", label, 36702),
                        (expected, 1, 1)
                    )
                }),
            ),
            (
                "Core Wasm",
                std::panic::catch_unwind(|| {
                    assert_eq!(run_core_wasm(&program, label), (expected, 1, 1))
                }),
            ),
        ] {
            if result.is_err() {
                failures.push(engine);
            }
        }
        assert!(failures.is_empty(), "{label}: failed engines {failures:?}");
    }
}

#[test]
fn affine_capture_rejects_reuse_and_unsupported_signatures_before_codegen() {
    let duplicate = SOURCE.replace("    run(moved)", "    let first = run(moved); run(moved)");
    let capture_reuse = SOURCE.replace(
        "    once fn() -> i64 { consume(payload) }",
        "    let callback = once fn() -> i64 { consume(payload) }; let invalid = consume(payload); callback",
    );
    for source in [duplicate, capture_reuse] {
        let errors = semaprax::check(&source, "affine-reuse.spx").unwrap_err();
        assert!(
            errors.iter().any(|error| error.code == "SPX-O101"),
            "{errors:?}"
        );
    }
    for source in [
        SOURCE.replace("callback: own FnOnce", "callback: FnOnce"),
        SOURCE.replace("FnOnce() -> i64", "FnOnce(i64) -> i64"),
    ] {
        let errors = semaprax::check(&source, "affine-profile.spx").unwrap_err();
        assert!(
            errors.iter().any(|error| error.code == "SPX-T308"),
            "{errors:?}"
        );
    }
}
