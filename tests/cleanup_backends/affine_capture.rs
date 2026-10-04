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

fn mixed_source() -> String {
    SOURCE
        .replace("FnOnce()", "FnOnceI64()")
        .replace(
            "consume(payload: own Bytes) -> i64 { 42 }",
            "consume(payload: own Bytes, offset: i64) -> i64 { offset + 2 }",
        )
        .replace("fn make()", "fn make(offset: i64)")
        .replace("consume(payload) }", "consume(payload, offset) }")
        .replace("make();", "make(40);")
}

#[test]
fn mixed_affine_capture_retains_scalar_and_owner_on_all_backends() {
    assert!(command_available("clang") && command_available("node"));
    for (label, source, expected) in [
        ("mixed-called", mixed_source(), 42),
        (
            "mixed-other-snapshot",
            mixed_source().replace("make(40)", "make(17)"),
            19,
        ),
        (
            "mixed-unused",
            mixed_source().replace("    run(moved)", "    99"),
            99,
        ),
    ] {
        let program = checked_program(&source, label);
        let canonical = semaprax::format::canonical(&program);
        assert!(canonical.contains("FnOnceI64() -> i64"));
        assert_eq!(
            canonical,
            semaprax::format::canonical(&checked_program(&canonical, label))
        );
        let graph = semaprax::graph::to_json(&program).unwrap();
        semaprax::graph::verify_json(&program, &graph).unwrap();
        for fact in [
            "semaprax.graph.v63",
            "bytes-i64-to-i64.v2",
            "core.fn_once_i64.construct.v2",
            "core.fn_once_i64.invoke.v2",
            "core.fn_once_i64.drop.v2",
        ] {
            assert!(graph.contains(fact), "missing {fact}");
        }
        let c = codegen::emit_c(&program).unwrap();
        assert_eq!(run_interpreter(&source, label), expected);
        for optimization in ["-O0", "-O2"] {
            assert_eq!(run_native(&c, optimization, label, 36801), (expected, 1, 1));
        }
        assert_eq!(run_core_wasm(&program, label), (expected, 1, 1));
    }
}

#[test]
fn mixed_affine_capture_snapshots_mutable_scalar_without_aliasing() {
    assert!(command_available("clang") && command_available("node"));
    let source = mixed_source().replace(
        "    let payload = bytes_zeroed(4usize);\n    once fn() -> i64 { consume(payload, offset) }",
        "    let mut captured_offset = offset;\n    let payload = bytes_zeroed(4usize);\n    let callback = once fn() -> i64 { consume(payload, captured_offset) };\n    captured_offset = 1;\n    callback",
    );
    let program = checked_program(&source, "mixed-mutable-snapshot");
    let canonical = semaprax::format::canonical(&program);
    assert_eq!(
        canonical,
        semaprax::format::canonical(&checked_program(&canonical, "mixed-mutable-snapshot"))
    );
    let graph = semaprax::graph::to_json(&program).unwrap();
    semaprax::graph::verify_json(&program, &graph).unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    assert_eq!(run_interpreter(&source, "mixed-mutable-snapshot"), 42);
    for optimization in ["-O0", "-O2"] {
        assert_eq!(
            run_native(&generated, optimization, "mixed-mutable-snapshot", 36803),
            (42, 1, 1)
        );
    }
    assert_eq!(
        run_core_wasm(&program, "mixed-mutable-snapshot"),
        (42, 1, 1)
    );
}

#[test]
fn mixed_affine_capture_rejects_schema_drift_and_reuse() {
    for (source, code) in [
        (
            mixed_source().replace("    run(moved)", "    let first = run(moved); run(moved)"),
            "SPX-O101",
        ),
        (
            mixed_source().replace(
                "once fn() -> i64 { consume(payload, offset) }",
                "once fn() -> i64 { consume(payload, offset + 1) }",
            ),
            "SPX-T308",
        ),
        (
            mixed_source().replace("callback: own FnOnceI64", "callback: FnOnceI64"),
            "SPX-T308",
        ),
    ] {
        let errors = semaprax::check(&source, "mixed-negative.spx").unwrap_err();
        assert!(errors.iter().any(|e| e.code == code), "{errors:?}");
    }
    let source = mixed_source().replace("FnOnceI64()", "FnOnce()");
    assert!(semaprax::check(&source, "mixed-wrong-type.spx").is_err());
    let program = checked_program(&mixed_source(), "mixed-hostile");
    let graph = semaprax::graph::to_json(&program).unwrap();
    assert!(semaprax::graph::verify_json(
        &program,
        &graph.replace("bytes-i64-to-i64.v2", "bytes-to-i64.v1")
    )
    .is_err());
}

#[test]
fn mixed_affine_capture_hir_replay_refuses_wrong_scalar_and_cleanup_identity() {
    use semaprax::hir::{ResolvedExprKind, ResolvedType};
    let checked = checked_program(&mixed_source(), "mixed-hir");
    let original = semaprax::hir::resolve(&checked).unwrap();
    semaprax::hir::validate(&original).unwrap();
    for mutation in 0..3 {
        let mut hostile = original.clone();
        let factory = hostile
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "once.make")
            .unwrap();
        let ResolvedExprKind::Block { tail, .. } = &mut factory.body.kind else {
            panic!("factory block");
        };
        let ResolvedExprKind::Closure { captures, .. } = &mut tail.kind else {
            panic!("factory closure");
        };
        match mutation {
            0 => captures[1].value.ty = ResolvedType::Bool,
            1 => captures[1].binding.id = captures[0].binding.id.clone(),
            _ => tail.ty = ResolvedType::OnceFunction,
        }
        assert!(
            semaprax::hir::validate(&hostile).is_err(),
            "accepted mutation {mutation}"
        );
    }
}

#[test]
fn affine_capture_unused_parameter_helpers_select_the_owned_runtime() {
    for signature in ["FnOnce", "FnOnceI64"] {
        let source = format!(
            r#"module test.affine_unused;
@id("once.run") fn run(callback: own {signature}() -> i64) -> i64 {{ callback() }}
@id("app.main") fn main() -> i64 {{ 42 }}
"#
        );
        let program = checked_program(&source, "unused-affine-parameter");
        let c = codegen::emit_c(&program).unwrap();
        assert_eq!(run_native(&c, "-O2", signature, 36802), (42, 0, 0));
        assert_eq!(run_core_wasm(&program, signature), (42, 0, 0));
    }
}
