//! Additional arm-context controls; complete old validators remain executable.
use super::*;

fn program(source: &str) -> ResolvedProgram {
    let checked = crate::check(source, "arm-context.spx").unwrap();
    crate::hir::resolve(&checked).unwrap()
}

fn arms(expression: &mut ResolvedExpr) -> &mut Vec<ResolvedMatchArm> {
    match &mut expression.kind {
        ResolvedExprKind::Match { arms, .. } => arms,
        ResolvedExprKind::Block { tail, .. } => arms(tail),
        _ => panic!("the owning fixture must actually contain its match"),
    }
}

fn main_body(program: &ResolvedProgram) -> &ResolvedExpr {
    &program
        .functions
        .iter()
        .find(|f| f.name == "main")
        .unwrap()
        .body
}

fn compare_expression(program: &ResolvedProgram, expression: &ResolvedExpr, fails: bool) {
    let function = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(function.params.is_empty());
    let execution = FunctionExecutionId::Monomorphic(function.id.clone());
    let mut current = HirValidator::new(program).unwrap();
    let mut recursive = HirValidator::new(program).unwrap();
    let mut owned = HirValidator::new(program).unwrap();
    let mut current_scope = BTreeMap::new();
    let mut recursive_scope = BTreeMap::new();
    let mut owned_scope = BTreeMap::new();
    let actual = current.validate_expr(
        &execution,
        expression,
        &mut current_scope,
        "body",
        true,
        None,
    );
    let expected = recursive.validate_expr_recursive_reference_body(
        &execution,
        expression,
        &mut recursive_scope,
        "body",
        true,
        None,
    );
    let saved = owned.validate_expr_iterative_owned_reference_body(
        &execution,
        expression,
        &mut owned_scope,
        "body",
        true,
        None,
    );
    for (outcome, validator, scope) in [
        (&expected, &recursive, &recursive_scope),
        (&saved, &owned, &owned_scope),
    ] {
        // This compares all six diagnostic fields, expression/value identities
        // and the complete published map, including failed partial prefixes.
        HirValidator::assert_validation_oracle(
            &actual,
            outcome,
            &current,
            validator,
            &current_scope,
            scope,
            "body",
        );
        HirValidator::assert_scope_auxiliary_oracle(&current, validator);
        assert_eq!(current.static_text.get(), validator.static_text.get());
    }
    assert_eq!(
        actual.is_err(),
        fails,
        "the mutation must be genuinely hostile: {actual:?}"
    );
    if let Err(error) = actual {
        assert_eq!(error.code, "SPX-H006");
    }
}

fn on_complete_oracle_stack(body: impl FnOnce() + Send + 'static) {
    // Match the existing complete recursive oracle's bounded test stack. The
    // original depth64 body is retained, never replaced by a partial oracle.
    std::thread::Builder::new()
        .name("904-arm-context-oracles".to_owned())
        .stack_size(32 * 1024 * 1024)
        .spawn(body)
        .unwrap()
        .join()
        .unwrap_or_else(|payload| std::panic::resume_unwind(payload));
}

#[test]
fn scalar_constructor_contexts_and_late_arm_mutations_match_both_complete_validators() {
    on_complete_oracle_stack(|| {
        // Every admitted scalar pattern constructor, false guard, lexical arm
        // binding and nested result is exercised by an actual resolved body.
        for (scrutinee, patterns) in [
            (
                "2",
                "0 | 1 if false => 7, n => match n { 2 => 9, _ => 8, },",
            ),
            ("2i32", "0i32 | 1i32 if false => 7, n => 8,"),
            ("2u8", "0u8 | 1u8 if false => 7, n => 8,"),
            ("true", "false if false => 7, n => 8,"),
            ("'é'", "'a' | 'b' if false => 7, n => 8,"),
        ] {
            let source = format!(
                "module arm.scalar; @id(\"main\") fn main()->i64 {{match {scrutinee} {{{patterns}}}}}"
            );
            let program = program(&source);
            compare_expression(&program, main_body(&program), false);
            for attack in 0..6 {
                let mut expression = main_body(&program).clone();
                let values = arms(&mut expression);
                match attack {
                    0 => values.last_mut().unwrap().value.ty = ResolvedType::Bool,
                    1 => values.last_mut().unwrap().value.ownership = OwnershipMode::Own,
                    2 => values[0].guard.as_mut().unwrap().ty = ResolvedType::I64,
                    3 => values[0].guard.as_mut().unwrap().ownership = OwnershipMode::Own,
                    4 => {
                        values.pop().unwrap();
                    }
                    5 => {
                        let duplicate = values[0].value.id.clone();
                        values.last_mut().unwrap().value.id = duplicate;
                    }
                    _ => unreachable!(),
                }
                compare_expression(&program, &expression, true);
            }
        }
    });
}

#[test]
fn guarded_variant_contexts_preserve_guard_before_value_and_exhaustiveness() {
    on_complete_oracle_stack(|| {
        let source = r#"module arm.variant;
@id("choice") variant Choice {
 @id("choice.a") A { @id("choice.a.value") value:i64, },
 @id("choice.b") B,
}
@id("main") fn main()->i64 {match Choice::A {value:2} {
 Choice::A {value} if value>0 => value,
 Choice::A {value} => value+1,
 Choice::B {} => 0,
}}
"#;
        let program = program(source);
        compare_expression(&program, main_body(&program), false);
        for attack in 0..5 {
            let mut expression = main_body(&program).clone();
            let values = arms(&mut expression);
            match attack {
                0 => {
                    // Both are malformed: the guard must still select the
                    // complete original diagnostic before the later arm value.
                    values[0].guard.as_mut().unwrap().ty = ResolvedType::I64;
                    values[0].value.ownership = OwnershipMode::Own;
                }
                1 => values[1].value.ty = ResolvedType::Bool,
                2 => {
                    values.pop().unwrap();
                }
                3 => {
                    let duplicate = values[0].value.id.clone();
                    values[0].guard.as_mut().unwrap().id = duplicate;
                }
                4 => values.last_mut().unwrap().value.ownership = OwnershipMode::Own,
                _ => unreachable!(),
            }
            compare_expression(&program, &expression, true);
        }
    });
}

#[test]
fn generic_result_and_borrowed_arm_release_keep_complete_function_publication() {
    on_complete_oracle_stack(|| {
        for source in [
            r#"module arm.generic;
@id("identity") fn identity<T>(value:T)->T {value}
@id("main") fn main()->i64 {match 0 {0=>identity<i64>(1),n=>identity<i64>(n),}}
"#,
            r#"module arm.borrowed;
@id("consume") fn consume(value:own Result<Bytes,Bytes>)->i64 {
 let borrowed=match borrow value {
  Result::Ok {value:payload}=>if byte_len(bytes_as_slice(payload))==0usize {0}else{1},
  Result::Err {error:payload}=>if byte_len(bytes_as_slice(payload))==0usize {0}else{2},
 };
 match own value {Result::Ok {value:payload}=>borrowed,Result::Err {error:payload}=>borrowed,}
}
@id("main") fn main()->i64 {0}
"#,
        ] {
            let program = program(source);
            crate::hir::validate(&program).unwrap();
            let mut current = HirValidator::new(&program).unwrap();
            let mut previous = current.clone();
            // Actual parameters, borrow release, owning re-match, contracts,
            // final publication and all materialized generic functions run
            // through the complete independent function receiver.
            let mut instances = 0;
            for (function, execution) in program
                .functions
                .iter()
                .map(|f| (f, FunctionExecutionId::Monomorphic(f.id.clone())))
                .chain(program.function_instances.iter().map(|instance| {
                    instances += 1;
                    (
                        &instance.function,
                        FunctionExecutionId::Generic(instance.id.clone()),
                    )
                }))
            {
                let actual = current.validate_function(function, &execution);
                let expected = previous.validate_function_before_frame_reuse(function, &execution);
                HirValidator::assert_validation_oracle(
                    &actual,
                    &expected,
                    &current,
                    &previous,
                    &BTreeMap::new(),
                    &BTreeMap::new(),
                    function.id.as_str(),
                );
                HirValidator::assert_scope_auxiliary_oracle(&current, &previous);
                actual.unwrap();
            }
            if source.contains("identity<T>") {
                assert!(instances > 0, "actual generic materialization is required");
                let mut expression = main_body(&program).clone();
                arms(&mut expression).last_mut().unwrap().value.ty = ResolvedType::Bool;
                compare_expression(&program, &expression, true);
            }
        }
    });
}

/// The exact new test registration retains its complete fixed previous owner.
/// Existing producer/control bodies and every old downstream target are kept.
fn prior_source(source: &str, current: &str) -> Option<String> {
    crate::source_reflows_merge::Main44Fixture::prior_source(source, current)
}

#[test]
fn complete_context_source_inverses_reject_drift_and_keep_whole_previous_owners() {
    for (source, current) in [
        (
            "src/hir/validation/lexical_exit.rs",
            include_str!("../lexical_exit.rs"),
        ),
        (
            "src/source_reflows_merge/main44_custody.rs",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/source_reflows_merge/main44_custody.rs"
            )),
        ),
    ] {
        let before = prior_source(source, current).expect("complete current owner");
        assert_ne!(before, current);
        assert!(
            prior_source(source, &before).is_none(),
            "the prior image is not the new current authority"
        );
        assert!(prior_source(source, &(current.to_owned() + " ")).is_none());
        let mut drift = current.as_bytes().to_vec();
        drift[0] ^= 1;
        assert!(prior_source(source, std::str::from_utf8(&drift).unwrap()).is_none());
    }
    assert!(prior_source("unlisted.rs", include_str!("arm_context_tests.rs")).is_none());
    // Invoke the actual owning gate: new physical/compiled -> entire eb8
    // registration predecessor -> unchanged Main44 -> unchanged #913 target.
    let current = include_str!("../lexical_exit.rs");
    let (view, edits) = crate::source_reflows_merge::Main44Fixture::closure_epoch(
        "src/hir/validation/lexical_exit.rs",
        "5610b10bf134638d5a30dbee84058895d40263d46ac07057f154d47ff8487c0e",
        current,
    );
    let mut full = current.to_owned();
    for (start, after, before) in edits.into_iter().rev() {
        assert_eq!(full.get(start..start + after.len()), Some(after));
        full.replace_range(start..start + after.len(), before);
    }
    assert_eq!(
        full, view,
        "returned complete inverse includes the new registration"
    );
    let complete =
        crate::source_reflows::batch_913_entry("src/hir/validation/lexical_exit.rs", current)
            .unwrap();
    use sha2::{Digest, Sha256};
    assert_eq!(complete.len(), 47202);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(complete.as_bytes()))
        ),
        "ed95841d0c0bb977e8120046a0182ff2dcef8731f443d0d1c17af2233ce15bc4"
    );
    assert!(
        std::panic::catch_unwind(|| crate::source_reflows::batch_913_entry(
            "src/hir/validation/lexical_exit.rs",
            &(current.to_owned() + " ")
        ))
        .is_err()
    );
}
