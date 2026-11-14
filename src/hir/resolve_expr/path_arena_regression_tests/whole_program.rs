//! Independent full assembly with saved text-expression lowering, beside the
//! ordinary current route. This does not claim a frozen pre-arena whole driver:
//! signatures/types/helpers remain the unchanged shared resolution services.
use crate::hir::resolve_statement::text_reference;
use crate::hir::{DeclarationId, ExpressionId, FunctionExecutionId, ResolvedProgram};
use crate::Program;
use std::path::Path;

fn saved_program(program: &Program) -> Result<ResolvedProgram, Vec<crate::diagnostic::Diagnostic>> {
    let diagnostics = crate::source_verify::verify(program);
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity.is_error())
    {
        return Err(diagnostics);
    }
    let (result, expressions) = text_reference::with(|| {
        crate::hir::resolve_program::agent_identity_original::resolve_verified_original(
            program, None,
        )
        .map(|(resolved, _)| resolved)
        .map_err(|error| vec![error])
    });
    assert!(
        expressions > 0,
        "the full old arm executed text expression lowering"
    );
    assert!(!text_reference::active());
    result
}

fn checked_pair(program: &Program) -> (ResolvedProgram, ResolvedProgram) {
    let current = crate::hir::resolve(program).unwrap();
    let old = saved_program(program).unwrap();
    crate::hir::validate(&current).unwrap();
    crate::hir::validate(&old).unwrap();
    crate::loan_plan::validate_program(&current).unwrap();
    crate::loan_plan::validate_program(&old).unwrap();
    assert_eq!(current, old, "entire independently assembled HIR");
    assert_eq!(
        crate::cache_codec::encode(&current).unwrap(),
        crate::cache_codec::encode(&old).unwrap()
    );
    let revision = crate::graph::revision(program);
    assert_eq!(
        crate::graph::to_hir_json(&current, &revision).unwrap(),
        crate::graph::to_hir_json(&old, &revision).unwrap()
    );
    for (current, old) in current.functions.iter().zip(&old.functions) {
        assert_eq!(current.requires, old.requires);
        assert_eq!(current.ensures, old.ensures);
        assert_eq!(current.cleanup, old.cleanup);
        assert_eq!(
            current.cleanup_plan, old.cleanup_plan,
            "complete canonical order"
        );
        assert_eq!(current.loan_plan, old.loan_plan, "complete canonical order");
    }
    for (current, old) in current
        .function_instances
        .iter()
        .zip(&old.function_instances)
    {
        assert_eq!(current.function.cleanup, old.function.cleanup);
        assert_eq!(current.function.cleanup_plan, old.function.cleanup_plan);
        assert_eq!(current.function.loan_plan, old.function.loan_plan);
    }
    (current, old)
}

fn run(body: fn()) {
    // The already-established stack accommodation is for the recursive oracle.
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(body)
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn whole_program_text_receiver_keeps_complete_hir_graph_cache_and_ordered_plans() {
    run(|| {
        let program = crate::parse(&super::source(), Path::new("path-arena-whole.spx")).unwrap();
        let (current, old) = checked_pair(&program);
        assert_eq!(
            crate::graph::to_json(&program).unwrap(),
            crate::graph::to_hir_json(&old, &crate::graph::revision(&program)).unwrap()
        );
        let contracted = super::source().replace(
            "fn deep(x: i64) -> i64 {",
            "fn deep(x: i64) -> i64 requires x >= 0 ensures result >= 0 {",
        );
        assert_ne!(contracted, super::source());
        let contracted = crate::parse(&contracted, Path::new("path-arena-contracts.spx")).unwrap();
        let (contract_current, contract_old) = checked_pair(&contracted);
        let function = contract_current
            .functions
            .iter()
            .find(|f| f.id.as_str() == "deep")
            .unwrap();
        assert_eq!(function.requires.len(), 1);
        assert_eq!(function.ensures.len(), 1);
        assert_eq!(contract_current, contract_old);
        let decoded: ResolvedProgram =
            crate::cache_codec::decode(&crate::cache_codec::encode(&old).unwrap()).unwrap();
        crate::hir::validate(&decoded).unwrap();
        assert_eq!(
            decoded, current,
            "decoded data still requires complete ordinary validation"
        );
    });
}

#[test]
fn text_receiver_matches_selected_status_and_lazy_owned_argument_order() {
    use crate::cleanup_plan::{CleanupTransition, StorageId};
    use crate::interpreter::{
        evaluate_resolved_profile_i64_entry, ResolvedEvaluationOutcome, ResolvedFunctionProfile,
    };

    for (body, returns) in [
        (
            r#"if true || string_len(fail("ignored")) == 0 { pair("a", "abc") } else { 0 }"#,
            true,
        ),
        (r#"pair("a", fail("abc"))"#, false),
    ] {
        let source = format!(
            r#"module test.path_arena_execution;
@id("fail") fn fail(text: string) -> string {{ string_slice(text, 0, 99) }}
@id("pair") fn pair(left: string, right: string) -> i64 {{
    string_len(left) * 10 + string_len(right)
}}
@id("main") fn main() -> i64 {{ {body} }}
"#,
        );
        let program = crate::parse(&source, Path::new("path-arena-execution.spx")).unwrap();
        let (current, old) = checked_pair(&program);
        let evaluate = |program| {
            evaluate_resolved_profile_i64_entry(
                program,
                "main",
                1000,
                ResolvedFunctionProfile::StreamText,
            )
            .unwrap()
        };
        let result = evaluate(&current);
        assert_eq!(result, evaluate(&old));
        if returns {
            assert_eq!(result.outcome, ResolvedEvaluationOutcome::ReturnedI64(13));
            let main = current
                .functions
                .iter()
                .find(|f| f.id.as_str() == "main")
                .unwrap();
            let mut stages = Vec::new();
            let mut commits = Vec::new();
            for transition in main.cleanup_plan.blocks.iter().flat_map(|b| &b.transitions) {
                match transition {
                    CleanupTransition::Transfer { destination, .. } => {
                        if let StorageId::CallArgument {
                            parameter_index, ..
                        } = &destination.storage
                        {
                            stages.push(*parameter_index);
                        }
                    }
                    CleanupTransition::CallCommit { arguments, .. } if arguments.len() == 2 => {
                        commits.push([arguments[0].parameter_index, arguments[1].parameter_index]);
                    }
                    _ => {}
                }
            }
            assert!(stages.windows(2).any(|pair| pair == [0, 1]));
            assert_eq!(commits, [[0, 1]]);
        } else {
            let ResolvedEvaluationOutcome::LanguageFailure(status) = result.outcome else {
                panic!("expected selected Text failure, got {:?}", result.outcome);
            };
            assert_eq!(status.domain_id(), "semaprax.text.v1");
            assert_eq!(status.code(), 1);
        }
    }
}

#[test]
fn full_text_receiver_keeps_foreign_identity_and_cached_mutation_refusals() {
    run(|| {
        let program =
            crate::parse(&super::source(), Path::new("path-arena-association.spx")).unwrap();
        let (current, old) = checked_pair(&program);
        // Owned facades are value-based; equality is not a pointer admission proof.
        let detached = current.clone();
        assert!(!std::ptr::eq(&current, &detached));
        assert_eq!(current, detached);
        crate::hir::validate(&detached).unwrap();
        let foreign = FunctionExecutionId::Monomorphic(DeclarationId::new("foreign.function"));
        for pair in [(current.clone(), old.clone()), (detached, old.clone())] {
            let (mut current, mut old) = pair;
            current.functions[0].body.id = ExpressionId::new(&foreign, "body");
            old.functions[0].body.id = ExpressionId::new(&foreign, "body");
            let current_error = crate::hir::validate(&current).unwrap_err();
            assert_eq!(current_error, crate::hir::validate(&old).unwrap_err());
            assert_eq!(
                crate::graph::to_hir_json(&current, "same-revision").unwrap_err(),
                crate::graph::to_hir_json(&old, "same-revision").unwrap_err()
            );
            let decoded: ResolvedProgram =
                crate::cache_codec::decode(&crate::cache_codec::encode(&old).unwrap()).unwrap();
            assert_eq!(crate::hir::validate(&decoded).unwrap_err(), current_error);
        }
        // Same-source old graph bytes cannot authenticate a changed program.
        let graph = crate::graph::to_json(&program).unwrap();
        crate::graph::verify_json(&program, &graph).unwrap();
        let foreign_source = super::source().replace(
            "module test.path_arena_oracle;",
            "module foreign.path_arena_oracle;",
        );
        let foreign_source =
            crate::parse(&foreign_source, Path::new("path-arena-foreign.spx")).unwrap();
        assert_eq!(
            crate::graph::verify_json(&foreign_source, &graph).unwrap_err()[0].code,
            "SPX-G411"
        );
        let (mut corrupt_current, mut corrupt_old) = (current.clone(), old.clone());
        corrupt_current.functions[0].cleanup_plan.entry = crate::cleanup_plan::BlockId(u32::MAX);
        corrupt_old.functions[0].cleanup_plan.entry = crate::cleanup_plan::BlockId(u32::MAX);
        let error = crate::hir::validate(&corrupt_current).unwrap_err();
        assert_eq!(error, crate::hir::validate(&corrupt_old).unwrap_err());
        let decoded: ResolvedProgram =
            crate::cache_codec::decode(&crate::cache_codec::encode(&corrupt_old).unwrap()).unwrap();
        assert_eq!(error, crate::hir::validate(&decoded).unwrap_err());
        let crossed = super::source().replace(
            "let outcome = -(picked * 2) + scalar(s999) + deep(s1000);\n  outcome",
            "-(picked * 2) + scalar(s999) + deep(s1000)",
        );
        let crossed = crate::parse(&crossed, Path::new("path-arena-crossed.spx")).unwrap();
        assert_eq!(
            crate::hir::resolve(&crossed).unwrap_err(),
            saved_program(&crossed).unwrap_err()
        );
    });
}

#[test]
fn text_reference_scope_restores_current_dispatch_after_unwind_and_nested_use() {
    assert!(!text_reference::active());
    let panic = std::panic::catch_unwind(|| text_reference::with(|| panic!("oracle unwind")));
    assert!(panic.is_err());
    assert!(!text_reference::active());
    let (_, count) = text_reference::with(|| {
        assert!(text_reference::active());
        text_reference::note_expression();
        let (_, inner) = text_reference::with(text_reference::note_expression);
        assert_eq!(inner, 1);
        assert!(text_reference::active());
    });
    assert_eq!(count, 1);
    assert!(!text_reference::active());
}
