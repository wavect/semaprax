use super::*;
use std::fmt::Write as _;

fn source(statements: usize, literal: &str) -> String {
    let mut source =
        String::from("module test.block_work;\n@id(\"work.main\") fn main() -> i64 {\n");
    for index in 0..statements {
        writeln!(source, "let value_{index} = {literal};").unwrap();
    }
    source.push_str("42\n}\n");
    source
}

fn checked(statements: usize, literal: &str) -> crate::ast::Program {
    let parsed = parse(&source(statements, literal), Path::new("block-work.spx")).unwrap();
    assert!(crate::verify::verify(&parsed).is_empty());
    parsed
}

fn metered_body(statements: usize, literal: &str) -> (ResolvedProgram, ResolvedExpr) {
    let program = hir::resolve(&checked(1, literal)).unwrap();
    let mut expression = program.functions[0].body.clone();
    let ResolvedExprKind::Block {
        statements: values, ..
    } = &mut expression.kind
    else {
        panic!("seed function must retain its flat block");
    };
    assert_eq!(values.len(), 1);
    // Private counter model only: repeated seed identities carry no cleanup
    // storage, decisions or observations in these literal cases. This body is
    // never admitted, emitted, or offered to a production validator. The
    // separate source-resolution regression below uses real unique bindings.
    let seed = values[0].clone();
    values.clear();
    values.resize(statements, seed);
    (program, expression)
}

fn measure(
    program: &ResolvedProgram,
    expression: &ResolvedExpr,
    limit: usize,
) -> (Result<Vec<ExprSkeletonPath>, Diagnostic>, usize, usize) {
    reset_skeleton_materializations();
    let mut budget = ReplayBudget::with_skeleton_limit(limit);
    let result = expression_skeleton(
        program,
        &program.functions[0],
        expression,
        &mut SkeletonWork {
            function: &program.functions[0],
            budget: &mut budget,
        },
    );
    (
        result,
        limit - budget.skeleton_remaining,
        skeleton_materializations(),
    )
}

fn successful_path(paths: Vec<ExprSkeletonPath>) {
    assert_eq!(paths.len(), 1);
    assert!(paths[0].observations.is_empty());
    assert!(paths[0].owned_source.is_none());
    assert!(!paths[0].failed && !paths[0].residual);
}

#[test]
fn flat_literal_block_meter_charges_before_each_materialization() {
    for literal in ["7", "\"x\""] {
        for statements in [0, 1, 1_000] {
            let (program, expression) = metered_body(statements, literal);
            // Root Eval + block root path: 2. Per binding: two continuation
            // pushes, one literal path, four sequencing operations: 7.
            // Tail: two pushes, one literal path, four sequencing ops: 7.
            let expected = 7 * statements + 9;
            assert!(expected < MAX_REPLAY_WORK_UNITS);
            let (result, used, materialized) = measure(&program, &expression, expected);
            successful_path(result.unwrap());
            assert_eq!(used, expected);
            assert_eq!(materialized, expected);

            for limit in [0, expected - 1] {
                let (result, used, materialized) = measure(&program, &expression, limit);
                let error = match result {
                    Err(error) => error,
                    Ok(_) => panic!("short work budget unexpectedly succeeded"),
                };
                assert_eq!(error.code, "SPX-H006");
                assert!(error.message.contains("work budget exhausted during"));
                assert_eq!(used, limit);
                assert_eq!(materialized, limit);
            }
        }
    }
}

#[test]
fn flat_literal_block_census_covers_actual_metered_work() {
    for literal in ["7", "\"x\""] {
        for statements in [0, 1, 1_000] {
            let (program, expression) = metered_body(statements, literal);
            let derived =
                expression_skeleton_work_upper(&program, &program.functions[0], &expression)
                    .unwrap();
            let expected = 7 * statements + 9;
            assert!(
                derived >= expected,
                "{statements} literal {literal} bindings: census {derived} < actual {expected}"
            );
            assert!(derived < MAX_REPLAY_WORK_UNITS);
            let (result, used, materialized) = measure(&program, &expression, derived);
            successful_path(result.unwrap());
            assert_eq!(used, expected);
            assert_eq!(materialized, expected);
        }
    }
}

#[test]
fn shallow_wide_scalar_and_string_bindings_resolve_without_budget_underestimation() {
    for literal in ["7", "\"x\""] {
        for statements in [0, 1, 1_000] {
            let parsed = checked(statements, literal);
            let program = hir::resolve(&parsed).unwrap_or_else(|errors| {
                panic!("{statements} literal {literal} bindings: {errors:?}")
            });
            hir::validate(&program).unwrap();
            let ResolvedExprKind::Block {
                statements: actual, ..
            } = &program.functions[0].body.kind
            else {
                panic!("source body must retain its block");
            };
            assert_eq!(actual.len(), statements);
        }
    }
}

fn wide_constructor_source(variant: bool, fields: usize) -> String {
    let mut source = String::from("module test.wide_constructor;\n");
    if variant {
        source.push_str("@id(\"work.wide\") variant Wide { @id(\"work.wide.payload\") Payload {\n");
    } else {
        source.push_str("@id(\"work.wide\") record Wide {\n");
    }
    for index in 0..fields {
        if variant {
            writeln!(source, "@id(\"work.wide.payload.f{index}\") f{index}: i64,").unwrap();
        } else {
            writeln!(source, "@id(\"work.wide.f{index}\") f{index}: i64,").unwrap();
        }
    }
    source.push_str(if variant { "}, }\n" } else { "}\n" });
    source.push_str("@id(\"work.main\") fn main() -> i64 {\nlet value = ");
    source.push_str(if variant {
        "Wide::Payload { "
    } else {
        "Wide { "
    });
    for index in 0..fields {
        write!(source, "f{index}: {index}, ").unwrap();
    }
    source.push_str("};\n0\n}\n");
    source
}

#[test]
fn wide_record_and_variant_constructor_census_covers_real_replay_work() {
    for variant in [false, true] {
        let source = wide_constructor_source(variant, 64);
        let parsed = parse(&source, Path::new("wide-constructor.spx")).unwrap();
        assert!(crate::verify::verify(&parsed).is_empty());
        let program = hir::resolve(&parsed).unwrap();
        hir::validate(&program).unwrap();
        let ResolvedExprKind::Block { statements, .. } = &program.functions[0].body.kind else {
            panic!("main body must remain a block");
        };
        let constructor = statements[0].value();
        assert!(matches!(
            &constructor.kind,
            ResolvedExprKind::ConstructRecord { .. } | ResolvedExprKind::ConstructVariant { .. }
        ));
        let derived =
            expression_skeleton_work_upper(&program, &program.functions[0], constructor).unwrap();
        let (result, used, materialized) = measure(&program, constructor, derived);
        let paths = result.unwrap();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].observations.is_empty());
        assert!(paths[0].owned_source.is_some());
        assert!(!paths[0].failed && !paths[0].residual);
        assert!(used <= derived, "census {derived} < actual {used}");
        assert_eq!(used, materialized);
    }
}

#[test]
fn valid_resource_bindings_and_block_results_include_transfer_work() {
    for (body, expected, transfers) in [("value", 16, 1), ("let moved = value; moved", 30, 2)] {
        let source = format!(
            "module test.resource_block_work;\n\
             @id(\"work.token\") resource Token {{ @id(\"work.drop\") drop trivial; }}\n\
             @id(\"work.forward\") fn forward(value: own Token) -> Token {{ {body} }}\n\
             @id(\"work.main\") fn main() -> i64 {{ 0 }}\n"
        );
        let parsed = parse(&source, Path::new("resource-block-work.spx")).unwrap();
        assert!(crate::verify::verify(&parsed).is_empty());
        let program = hir::resolve(&parsed).unwrap();
        hir::validate(&program).unwrap();
        let function = &program.functions[0];
        assert_eq!(function.id.as_str(), "work.forward");
        assert_eq!(function.body.ownership, OwnershipMode::Own);
        assert!(type_needs_drop(&program, function, &function.body.ty).unwrap());

        // Direct return: root/block setup 2, tail pushes 2, owned place 2,
        // sequencing 4, block-result transfer 6 = 16. One owned binding adds
        // pushes 2, owned place 2, sequencing 4 and binding transfer 6 = 14.
        let (result, used, materialized) = measure(&program, &function.body, expected);
        let paths = result.unwrap();
        assert_eq!(used, expected);
        assert_eq!(materialized, expected);
        assert_eq!(paths.len(), 1);
        assert!(!paths[0].failed && !paths[0].residual);
        assert_eq!(paths[0].observations.len(), transfers);
        assert!(paths[0]
            .observations
            .iter()
            .all(|event| matches!(event, SkeletonObservation::Transfer { .. })));
        assert_eq!(
            paths[0].owned_source,
            Some(CleanupPlace {
                storage: StorageId::Temporary(function.body.id.clone()),
                projections: Vec::new(),
            })
        );

        let (result, used, materialized) = measure(&program, &function.body, expected - 1);
        let error = match result {
            Err(error) => error,
            Ok(_) => panic!("short resource transfer budget unexpectedly succeeded"),
        };
        assert_eq!(error.code, "SPX-H006");
        assert!(error.message.contains("work budget exhausted during"));
        assert_eq!(used, expected - 1);
        assert_eq!(materialized, expected - 1);

        let derived = expression_skeleton_work_upper(&program, function, &function.body).unwrap();
        assert!(
            derived >= expected,
            "{body}: census {derived} < actual {expected}"
        );
        assert!(derived < MAX_REPLAY_WORK_UNITS);
    }
}
