//! Source-owned transitive scalar providers and independent local controls.
use super::*;
// A loop-free return normalizes into an ordinary tail and retains no lexical
// inventory; the one-iteration loop keeps the selected source route.
const CALLER: &str = r#"module scalar.app;
use function @id("helper.step") from scalar.helper as step;
@id("app.main") fn main()->i64 {let label=str"é\u{0}";let outcome=step(41);let mut tick=0usize;while tick<1usize {tick=tick+1usize;return outcome;} 0}
"#;
const PROVIDER: &str = r#"module scalar.helper;
use function @id("leaf.step") from scalar.leaf as leaf_step;
@id("helper.step") fn step(n:i64)->i64 {leaf_step(n)}
@id("unrelated") fn unrelated()->Bytes {bytes_zeroed(1usize)}
"#;
const LEAF: &str = r#"module scalar.leaf; @id("leaf.step") fn step(n:i64)->i64 {n+1}"#;
fn build(caller: &str, provider: &str, leaf: &str) -> Result<WorkspaceGraphBuild, Vec<Diagnostic>> {
    build_sources(&[
        ("caller.spx", caller),
        ("provider.spx", provider),
        ("leaf.spx", leaf),
    ])
}
fn build_sources(sources: &[(&str, &str)]) -> Result<WorkspaceGraphBuild, Vec<Diagnostic>> {
    let sources = sources
        .iter()
        .map(|(path, source)| WorkspaceSource {
            path: (*path).into(),
            source: crate::format::canonical(&crate::parse(source, path).unwrap()),
        })
        .collect();
    build_owned_retaining_sources_for_change(sources, MAX_CHANGE_BUILDER_BYTES)
        .map(|(build, _)| build)
}

fn caller(build: &WorkspaceGraphBuild) -> &WorkspaceResolvedModule {
    build
        .hir
        .modules
        .iter()
        .find(|m| m.module == "scalar.app")
        .unwrap()
}
fn schema(build: &WorkspaceGraphBuild) -> Result<&'static str, Vec<Diagnostic>> {
    crate::workspace_graph::source_schema::schema(
        caller(build),
        &build.hir.modules,
        &build.hir.declarations,
    )
}
fn local(leaf: &str) -> hir::ResolvedProgram {
    let start = leaf.find("@id(").unwrap();
    let source = format!(
        "module scalar.local;\n{}\n@id(\"helper.step\") fn helper(n:i64)->i64 {{step(n)}}\n@id(\"app.main\") fn main()->i64 {{let label=str\"é\\u{{0}}\";let outcome=helper(41);return outcome;}}",
        &leaf[start..]
    );
    crate::hir::resolve(&crate::check(&source, "local.spx").unwrap()).unwrap()
}
#[test]
fn three_modules_use_actual_scalar_providers_without_literal_inventories_or_name_collisions() {
    let build = build(CALLER, PROVIDER, LEAF).unwrap();
    assert_eq!(schema(&build).unwrap(), "semaprax.graph.v77");
    for name in ["scalar.helper", "scalar.leaf"] {
        let provider = build.hir.modules.iter().find(|m| m.module == name).unwrap();
        assert!(provider.static_text.is_none());
        assert!(provider.scalar_helpers.is_some());
    }
    let provider = build
        .hir
        .modules
        .iter()
        .find(|m| m.module == "scalar.helper")
        .unwrap();
    assert_eq!(
        provider
            .scalar_helpers
            .as_ref()
            .unwrap()
            .functions
            .iter()
            .map(|f| f.id.as_str())
            .collect::<Vec<_>>(),
        ["helper.step"]
    );
    let linked = build
        .linked_owned_data_api_program_with_roots("scalar.app", &[])
        .unwrap();
    let control = local(LEAF);
    for _ in 0..3 {
        let actual =
            crate::interpreter::evaluate_resolved_zero_arg_i64(&linked, "app.main", 100_000)
                .unwrap();
        let expected =
            crate::interpreter::evaluate_resolved_zero_arg_i64(&control, "app.main", 100_000)
                .unwrap();
        assert_eq!(actual.outcome, expected.outcome);
        assert_eq!(
            actual.outcome,
            crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(42)
        );
    }
}
#[test]
fn checked_arithmetic_and_contract_failure_match_saved_all_local_controls() {
    for leaf in [
        LEAF.to_owned(),
        LEAF.replace("n+1", "n+9223372036854775807"),
        LEAF.replace("n+1", "n/0"),
        LEAF.replace("->i64 {", "->i64 requires false {"),
        LEAF.replace("->i64 {", "->i64 ensures false {"),
    ] {
        let build = build(CALLER, PROVIDER, &leaf).unwrap();
        let linked = build
            .linked_owned_data_api_program_with_roots("scalar.app", &[])
            .unwrap();
        let control = local(&leaf);
        for _ in 0..3 {
            assert_eq!(
                crate::interpreter::evaluate_resolved_zero_arg_i64(&linked, "app.main", 100_000)
                    .unwrap()
                    .outcome,
                crate::interpreter::evaluate_resolved_zero_arg_i64(&control, "app.main", 100_000)
                    .unwrap()
                    .outcome
            );
        }
        runtime::run(&linked, &control);
    }
}
#[test]
fn current_source_body_contract_target_and_namespace_guards_refuse_stale_providers() {
    for attack in 0..9 {
        let mut build = build(CALLER, PROVIDER, LEAF).unwrap();
        match attack {
            0 => {
                let module = build
                    .hir
                    .modules
                    .iter_mut()
                    .find(|m| m.module == "scalar.leaf")
                    .unwrap();
                module.functions[0].name = "forged".into();
            }
            1 => {
                let module = build
                    .hir
                    .modules
                    .iter_mut()
                    .find(|m| m.module == "scalar.leaf")
                    .unwrap();
                module.functions[0].body.span.end += 1;
            }
            2 => {
                let module = build
                    .hir
                    .modules
                    .iter_mut()
                    .find(|m| m.module == "scalar.leaf")
                    .unwrap();
                let body = module.functions[0].body.clone();
                module.functions[0].ensures.push(body);
            }
            3 => {
                build
                    .hir
                    .modules
                    .iter_mut()
                    .find(|m| m.module == "scalar.leaf")
                    .unwrap()
                    .scalar_helpers = None
            }
            4 => {
                build.hir.declarations.get_mut("leaf.step").unwrap().kind =
                    hir::DeclarationKind::Record
            }
            5 => {
                build.hir.declarations.get_mut("leaf.step").unwrap().origin =
                    hir::IdentityOrigin::Automatic
            }
            6 => {
                build.hir.declarations.get_mut("leaf.step").unwrap().owner =
                    Some("helper.step".into())
            }
            7 => {
                build.hir.declarations.get_mut("leaf.step").unwrap().path =
                    Some("forged.spx".into())
            }
            _ => {
                build.hir.declarations.get_mut("leaf.step").unwrap().module =
                    Some("scalar.helper".into())
            }
        }
        assert!(
            schema(&build)
                .unwrap_err()
                .iter()
                .any(|e| e.code == "SPX-G410"),
            "attack {attack}"
        );
    }
}
#[test]
fn actual_provider_plan_mutations_cannot_borrow_source_digest_authority() {
    for attack in 0..9 {
        let mut build = build(CALLER, PROVIDER, LEAF).unwrap();
        let certificate = caller(&build).functions[0]
            .cleanup_plan
            .lexical_control
            .as_ref()
            .unwrap()
            .clone();
        let module = build
            .hir
            .modules
            .iter_mut()
            .find(|m| m.module == "scalar.leaf")
            .unwrap();
        let function = &mut module.functions[0];
        match attack {
            0 => {
                function.cleanup_plan.status_sources.pop().unwrap();
            }
            1 => {
                let exit = function
                    .cleanup_plan
                    .exits
                    .iter_mut()
                    .find(|exit| {
                        matches!(
                            exit.continuation,
                            crate::cleanup_plan::ExitContinuation::CommitResult { .. }
                        )
                    })
                    .unwrap();
                let crate::cleanup_plan::ExitContinuation::CommitResult {
                    source: crate::cleanup_plan::CleanupResultSource::Scalar { expression },
                } = &mut exit.continuation
                else {
                    panic!("scalar result")
                };
                *expression = hir::ExpressionId::new(
                    &hir::FunctionExecutionId::Monomorphic(function.id.clone()),
                    "forged.result",
                );
            }
            2 => function.cleanup_plan.lexical_control = Some(certificate),
            3 => {
                let edge = &mut function.cleanup_plan.edges[0];
                edge.to = crate::cleanup_plan::BlockId(u32::MAX);
            }
            4 => function.loan_plan.schema = "forged.loan.schema",
            5 => {
                let block = function
                    .cleanup_plan
                    .blocks
                    .iter_mut()
                    .find(|b| {
                        b.transitions.iter().any(|t| {
                            matches!(
                                t,
                                crate::cleanup_plan::CleanupTransition::SelectFailure { .. }
                            )
                        })
                    })
                    .unwrap();
                let transition = block
                    .transitions
                    .iter()
                    .find(|t| {
                        matches!(
                            t,
                            crate::cleanup_plan::CleanupTransition::SelectFailure { .. }
                        )
                    })
                    .unwrap()
                    .clone();
                block.transitions.push(transition);
            }
            6 => {
                let edge = function
                    .cleanup_plan
                    .edges
                    .iter_mut()
                    .find(|e| {
                        matches!(
                            e.condition,
                            crate::cleanup_plan::EdgeCondition::StatusZero(_)
                        )
                    })
                    .unwrap();
                let crate::cleanup_plan::EdgeCondition::StatusZero(id) = &edge.condition else {
                    unreachable!()
                };
                edge.condition = crate::cleanup_plan::EdgeCondition::StatusNonzero(id.clone());
            }
            7 => {
                let exit = function
                    .cleanup_plan
                    .exits
                    .iter_mut()
                    .find(|e| {
                        matches!(
                            e.continuation,
                            crate::cleanup_plan::ExitContinuation::CommitResult { .. }
                        )
                    })
                    .unwrap();
                exit.continuation = crate::cleanup_plan::ExitContinuation::ReturnUnit;
            }
            _ => {
                let block = &mut function.cleanup_plan.blocks[0];
                block
                    .transitions
                    .push(crate::cleanup_plan::CleanupTransition::Initialize {
                        at: function.body.id.clone(),
                        destination: crate::cleanup_plan::CleanupPlace {
                            storage: crate::cleanup_plan::StorageId::ProvisionalResult,
                            projections: Vec::new(),
                        },
                    });
            }
        }
        assert!(
            schema(&build)
                .unwrap_err()
                .iter()
                .any(|e| e.code == "SPX-G410"),
            "plan attack {attack}"
        );
    }
}

fn measured(
    build: &WorkspaceGraphBuild,
    work: usize,
    bytes: usize,
) -> (
    Result<(usize, usize), Vec<Diagnostic>>,
    crate::test_allocation_observer::Metrics,
) {
    crate::test_allocation_observer::measure(|| {
        let module = caller(build);
        let anchor = &module.functions[0];
        // Keep each replay refusal's own diagnostic so a wide-limit failure
        // names its phase instead of the stable public G410 wrapper.
        let mut resources =
            SelectedReplayPreparation::with_limits(anchor, work, bytes).map_err(|e| vec![e])?;
        let context = authenticate(
            module,
            &build.hir.modules,
            &build.hir.declarations,
            &mut resources,
        )
        .map_err(legacy)?;
        let mut session = SelectedReplaySession::from_preparation(&context, anchor, resources)
            .map_err(|e| vec![e])?;
        for helper in &context.helpers {
            session
                .validate_scalar(helper.function)
                .map_err(|e| vec![e])?;
        }
        for function in &context.lexical {
            session.validate(function).map_err(|e| vec![e])?;
        }
        let (remaining, charged) = session.metrics();
        drop(session);
        drop(context);
        Ok((work.checked_sub(remaining).unwrap(), charged))
    })
}
#[test]
fn aggregate_authentication_to_final_drop_work_and_backing_are_exact_and_one_short() {
    let build = build(CALLER, PROVIDER, LEAF).unwrap();
    let (wide, allocation) = measured(&build, 32_000_000, 1 << 26);
    let (work, bytes) = wide.unwrap();
    assert!(!allocation.invalid && allocation.allocated <= bytes && allocation.highwater <= bytes);
    assert_eq!(allocation.live, 0);
    let (exact, allocation) = measured(&build, work, bytes);
    assert_eq!(exact.unwrap(), (work, bytes));
    assert!(!allocation.invalid && allocation.allocated <= bytes && allocation.highwater <= bytes);
    assert_eq!(allocation.live, 0);
    assert!(measured(&build, work - 1, bytes).0.is_err());
    assert!(measured(&build, work, bytes - 1).0.is_err());
}
#[test]
fn cycles_and_unsupported_scalar_provider_extensions_remain_closed() {
    // The ordinary checked constructor is broader than this selected profile.
    // Receive its actual source schema before testing selected helper refusal.
    fn build(
        caller: &str,
        provider: &str,
        leaf: &str,
    ) -> Result<WorkspaceGraphBuild, Vec<Diagnostic>> {
        let built = self::build(caller, provider, leaf)?;
        schema(&built)?;
        Ok(built)
    }
    for provider in [
        PROVIDER.replace("{leaf_step(n)}", "{step(n)}"),
        PROVIDER.replace(
            "fn step(n:i64)->i64 {leaf_step(n)}",
            "fn step(n:i64)->i64 {let label=str\"x\";if str_len_bytes(label)==1usize {n}else{0}}",
        ),
        PROVIDER.replace(
            "fn step(n:i64)->i64 {leaf_step(n)}",
            "fn step(n:i64)->i64 {let values=vec_with_capacity<i64>(1usize);n}",
        ),
    ] {
        assert!(build(CALLER, &provider, LEAF).is_err());
    }
}
#[test]
fn complete_ordinary_provider_sources_remain_checked_while_selected_schema_refuses() {
    for provider in [
        PROVIDER.replace("{leaf_step(n)}", "{step(n)}"),
        PROVIDER.replace(
            "fn step(n:i64)->i64 {leaf_step(n)}",
            "fn step(n:i64)->i64 {let label=str\"x\";if str_len_bytes(label)==1 {n}else{0}}",
        ),
        PROVIDER.replace(
            "fn step(n:i64)->i64 {leaf_step(n)}",
            "fn step(n:i64)->i64 {let values=vec_with_capacity<i64>(1usize);n}",
        ),
    ] {
        assert_ne!(
            provider, PROVIDER,
            "every hostile source mutation must occur"
        );
        let ordinary = build(CALLER, &provider, LEAF).unwrap();
        assert_eq!(ordinary.hir.modules.len(), 3);
        assert_eq!(caller(&ordinary).functions[0].id.as_str(), "app.main");
        let errors = schema(&ordinary).unwrap_err();
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|error| error.code == "SPX-G410"));
    }
    let admitted = build(CALLER, PROVIDER, LEAF).unwrap();
    assert_eq!(schema(&admitted).unwrap(), "semaprax.graph.v77");
}

#[test]
fn selected_refusal_fixture_keeps_complete_original_test_and_ordinary_receiver() {
    let current = concat!(
        include_str!("tests.rs"),
        include_str!("tests/borrowed.rs"),
        include_str!("tests/closure_graph.rs"),
        include_str!("tests/closure_graph/entry.rs"),
        include_str!("tests/closure_graph/oracle.rs"),
        include_str!("tests/runtime.rs"),
    );
    let original = include_str!(
        "../../../../../../docs/evidence/scalar-provider-selected-refusal/original_test.txt"
    );
    let start = current
        .find("#[test]\nfn cycles_and_unsupported_scalar_provider_extensions_remain_closed()")
        .unwrap();
    let tail = &current[start..];
    let end = tail.find("\n#[test]").unwrap();
    let complete = &tail[..end];
    let hook_start = complete
        .find("    // The ordinary checked constructor")
        .unwrap();
    let hook_tail = &complete[hook_start..];
    let hook_end = hook_start
        + hook_tail.find("        Ok(built)\n    }\n").unwrap()
        + "        Ok(built)\n    }\n".len();
    assert_eq!(
        format!("{}{}\n", &complete[..hook_start], &complete[hook_end..]),
        original
    );
    let ordinary = include_str!(
        "../../../../../../docs/evidence/scalar-provider-selected-refusal/ordinary_receiver.txt"
    );
    assert!(current.starts_with(ordinary.trim_end_matches('\n')));
}

#[test]
fn cloned_function_and_failed_later_provider_cannot_restart_a_lease_or_allowance() {
    let build = build(CALLER, PROVIDER, LEAF).unwrap();
    let module = caller(&build);
    let anchor = &module.functions[0];
    let mut resources = SelectedReplayPreparation::new(
        anchor,
        crate::cleanup_plan::ReplayPhysicalReservation::new(1 << 26),
    )
    .unwrap();
    let context = authenticate(
        module,
        &build.hir.modules,
        &build.hir.declarations,
        &mut resources,
    )
    .unwrap();
    let cloned = context.helpers[0].function.clone();
    let mut session = SelectedReplaySession::from_preparation(&context, anchor, resources).unwrap();
    assert_eq!(
        session.validate_scalar(&cloned).unwrap_err().code,
        "SPX-H006"
    );
    let metrics = session.metrics();
    assert_eq!(session.validate(anchor).unwrap_err().code, "SPX-H006");
    assert_eq!(session.metrics(), metrics);
    drop(session);
    drop(context);
    let mut malformed = self::build(CALLER, PROVIDER, LEAF).unwrap();
    malformed
        .hir
        .declarations
        .get_mut("leaf.step")
        .unwrap()
        .owner = Some("wrong.owner".into());
    let module = caller(&malformed);
    let anchor = &module.functions[0];
    let mut resources = SelectedReplayPreparation::new(
        anchor,
        crate::cleanup_plan::ReplayPhysicalReservation::new(1 << 26),
    )
    .unwrap();
    assert!(authenticate(
        module,
        &malformed.hir.modules,
        &malformed.hir.declarations,
        &mut resources
    )
    .is_err());
    assert_eq!(
        resources
            .charge(anchor, 0, "forbidden retry")
            .unwrap_err()
            .code,
        "SPX-H006"
    );
}

fn call_mut(expression: &mut hir::ResolvedExpr) -> Option<&mut hir::DeclarationId> {
    match &mut expression.kind {
        hir::ResolvedExprKind::Call { callee, .. } => Some(callee),
        hir::ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                if let hir::ResolvedStatement::Let { value, .. } = statement {
                    if let Some(id) = call_mut(value) {
                        return Some(id);
                    }
                }
            }
            call_mut(tail)
        }
        _ => None,
    }
}
#[test]
fn equal_name_providers_are_bound_by_exact_ids_and_authored_aliases() {
    let caller = CALLER
        .replace(
            "as step;",
            "as first_step;\nuse function @id(\"leaf.step\") from scalar.leaf as second_step;",
        )
        .replace("step(41)", "first_step(30)+second_step(0)");
    let provider = PROVIDER.replace("{leaf_step(n)}", "{leaf_step(n)+10}");
    let build = build(&caller, &provider, LEAF).unwrap();
    assert_eq!(schema(&build).unwrap(), "semaprax.graph.v77");
    let linked = build
        .linked_owned_data_api_program_with_roots("scalar.app", &[])
        .unwrap();
    let local = local(LEAF);
    assert_eq!(
        crate::interpreter::evaluate_resolved_zero_arg_i64(&linked, "app.main", 100_000)
            .unwrap()
            .outcome,
        crate::interpreter::evaluate_resolved_zero_arg_i64(&local, "app.main", 100_000)
            .unwrap()
            .outcome
    );
    runtime::run(&linked, &local);
}
#[test]
fn target_alias_missing_duplicate_and_stale_generation_do_not_reuse_provider_authority() {
    for attack in 0..7 {
        let mut build = build(CALLER, PROVIDER, LEAF).unwrap();
        match attack {
            0 => {
                let module = build
                    .hir
                    .modules
                    .iter_mut()
                    .find(|m| m.module == "scalar.app")
                    .unwrap();
                *call_mut(&mut module.functions[0].body).unwrap() =
                    hir::DeclarationId::new("leaf.step");
            }
            1 => {
                let module = build
                    .hir
                    .modules
                    .iter_mut()
                    .find(|m| m.module == "scalar.helper")
                    .unwrap();
                *call_mut(&mut module.functions[0].body).unwrap() =
                    hir::DeclarationId::new("helper.step");
            }
            2 => {
                build.hir.modules.retain(|m| m.module != "scalar.leaf");
            }
            3 => {
                let mut another = self::build(CALLER, PROVIDER, LEAF).unwrap();
                let position = another
                    .hir
                    .modules
                    .iter()
                    .position(|m| m.module == "scalar.leaf")
                    .unwrap();
                build.hir.modules.push(another.hir.modules.remove(position));
            }
            4 => {
                let mut another = self::build(CALLER, PROVIDER, LEAF).unwrap();
                let another = another
                    .hir
                    .modules
                    .iter_mut()
                    .find(|m| m.module == "scalar.leaf")
                    .unwrap();
                let duplicate = another.scalar_helpers.as_mut().unwrap().functions.remove(0);
                let current = build
                    .hir
                    .modules
                    .iter_mut()
                    .find(|m| m.module == "scalar.leaf")
                    .unwrap();
                current
                    .scalar_helpers
                    .as_mut()
                    .unwrap()
                    .functions
                    .push(duplicate);
            }
            5 => {
                let mut another =
                    self::build(CALLER, PROVIDER, &LEAF.replace("n+1", "n+2")).unwrap();
                let another = another
                    .hir
                    .modules
                    .iter_mut()
                    .find(|m| m.module == "scalar.leaf")
                    .unwrap();
                let current = build
                    .hir
                    .modules
                    .iter_mut()
                    .find(|m| m.module == "scalar.leaf")
                    .unwrap();
                current.scalar_helpers = another.scalar_helpers.take();
            }
            _ => {
                let current = build
                    .hir
                    .modules
                    .iter_mut()
                    .find(|m| m.module == "scalar.app")
                    .unwrap();
                current
                    .static_text
                    .as_mut()
                    .unwrap()
                    .lexical
                    .as_mut()
                    .unwrap()
                    .imported = false;
            }
        }
        assert!(
            schema(&build)
                .unwrap_err()
                .iter()
                .any(|e| e.code == "SPX-G410"),
            "provider attack {attack}"
        );
    }
}
#[test]
fn unrelated_generic_provider_declaration_cannot_expand_the_selected_closure() {
    let provider =
        format!("{PROVIDER}\n@id(\"unused.identity\") fn identity<T>(value:T)->T{{value}}");
    let build = build(CALLER, &provider, LEAF).unwrap();
    assert_eq!(schema(&build).unwrap(), "semaprax.graph.v77");
    let module = build
        .hir
        .modules
        .iter()
        .find(|m| m.module == "scalar.helper")
        .unwrap();
    assert!(!module.function_templates.is_empty());
    assert_eq!(module.scalar_helpers.as_ref().unwrap().functions.len(), 1);
}

#[test]
fn two_module_continue_and_return_use_the_actual_provider() {
    let caller_source = r#"module scalar.app;
use function @id("leaf.step") from scalar.leaf as step;
@id("app.main") fn main()->i64 {
 let label=str"é\u{0}";
 let mut i=0;
 while i<2 { i=step(i); if i==1 {continue;}else{0} 0 }
 return i;
}"#;
    let build = build_sources(&[("caller.spx", caller_source), ("leaf.spx", LEAF)]).unwrap();
    assert_eq!(schema(&build).unwrap(), "semaprax.graph.v77");
    let linked = build
        .linked_owned_data_api_program_with_roots("scalar.app", &[])
        .unwrap();
    let local_source = caller_source
        .replace("module scalar.app;", "module scalar.local;")
        .replace(
            "use function @id(\"leaf.step\") from scalar.leaf as step;",
            "",
        );
    let local_source = format!("{local_source}\n{}", &LEAF[LEAF.find("@id(").unwrap()..]);
    let local = hir::resolve(&crate::check(&local_source, "local-loop.spx").unwrap()).unwrap();
    runtime::run(&linked, &local);
}
#[test]
fn generic_option_nominal_and_vec_provider_bodies_keep_the_route_closed() {
    let generic_caller = CALLER.replace("step(41)", "step<i64>(41)");
    let generic_provider = r#"module scalar.helper; @id("helper.step") fn step<T>(n:T)->T{n}"#;
    let rejected = build(&generic_caller, generic_provider, LEAF)
        .err()
        .expect("generic provider remains closed");
    // A generic target is refused at the caller's import, before any provider
    // body can reach the selected closure.
    assert!(
        rejected.iter().any(|e| e.code == "SPX-G172"
            && e.path.as_deref() == Some("caller.spx")
            && e.message
                == "function target must be monomorphic with admitted value parameters, or borrowed byte-slice parameters and a scalar return"),
        "generic provider rejection: {rejected:?}"
    );
    for (label, leaf) in [
        (
            "Option<i64>",
            LEAF.replace("n+1", "let choice=Option<i64>::Some {value:n}; n"),
        ),
        (
            "private nominal",
            "module scalar.leaf; @id(\"private.Row\") record Row {@id(\"private.Row.n\") n:i64,} @id(\"leaf.step\") fn step(n:i64)->i64{let row=Row {n:n};row.n}".to_owned(),
        ),
        (
            "owned Vec",
            LEAF.replace("n+1", "let values=vec_with_capacity<i64>(1usize);n"),
        ),
    ] {
        // The builder admits each monomorphic scalar-signature import with any
        // checked body; scalar-helper capture omits a non-pure leaf instead of
        // refusing it. The selected source schema is the receiving gate.
        let build = build(CALLER, PROVIDER, &leaf)
            .unwrap_or_else(|e| panic!("{label} carrier provider builds: {e:?}"));
        let carrier = build
            .hir
            .modules
            .iter()
            .find(|m| m.module == "scalar.leaf")
            .unwrap();
        assert!(
            carrier.scalar_helpers.is_none(),
            "{label} carrier captured no scalar facts"
        );
        let rejected = schema(&build)
            .err()
            .unwrap_or_else(|| panic!("{label} carrier provider remains closed"));
        assert!(
            rejected.iter().any(|e| e.code == "SPX-G410"),
            "{label} provider rejection: {rejected:?}"
        );
    }
}

#[test]
fn failed_later_body_authentication_keeps_actual_work_and_poisoned_preparation() {
    let mut build = build(CALLER, PROVIDER, LEAF).unwrap();
    let leaf = build
        .hir
        .modules
        .iter_mut()
        .find(|m| m.module == "scalar.leaf")
        .unwrap();
    let hir::ResolvedExprKind::Block { tail, .. } = &mut leaf.functions[0].body.kind else {
        panic!("source body")
    };
    let hir::ResolvedExprKind::Binary { right, .. } = &mut tail.kind else {
        panic!("arithmetic body")
    };
    right.kind = hir::ResolvedExprKind::Int(2);
    let module = caller(&build);
    let anchor = &module.functions[0];
    let mut resources =
        SelectedReplayPreparation::with_limits(anchor, 32_000_000, 1 << 26).unwrap();
    assert!(authenticate(
        module,
        &build.hir.modules,
        &build.hir.declarations,
        &mut resources
    )
    .is_err());
    let remaining = resources.remaining();
    let spent = 32_000_000 - remaining;
    assert!(
        spent > 0 && remaining > 0,
        "failure does not mint or refund work"
    );
    assert_eq!(
        resources
            .charge(anchor, 0, "failed body retry")
            .unwrap_err()
            .code,
        "SPX-H006"
    );
    assert_eq!(resources.remaining(), remaining);
    let mut short = SelectedReplayPreparation::with_limits(anchor, spent - 1, 1 << 26).unwrap();
    assert!(authenticate(
        module,
        &build.hir.modules,
        &build.hir.declarations,
        &mut short
    )
    .is_err());
    assert_eq!(
        short.charge(anchor, 0, "one-short retry").unwrap_err().code,
        "SPX-H006"
    );
    assert!(short.remaining() < spent - 1);
}

mod borrowed;
mod closure_graph;
mod runtime;

#[test]
fn borrowed_provider_roots_and_transitive_length_reads_replay_actual_contracts() {
    let ordinary_caller_source = r#"module scalar.app;
use function @id("helper.length") from scalar.helper as length;
@id("app.main") fn main()->i64 {let text=str"é\u{0}";let size=length(str_as_bytes(text));return if size==3usize {42}else{0};}
"#;
    let provider_source = r#"module scalar.helper;
use function @id("leaf.length") from scalar.leaf as leaf_length;
@id("helper.length") fn length(view:borrow Slice<u8>)->usize {leaf_length(view)}
"#;
    let leaf_source = r#"module scalar.leaf;
@id("leaf.length") fn length(view:borrow Slice<u8>)->usize ensures result==byte_len(view) {byte_len(view)}
"#;
    // Keep the original tail-return source as an ordinary execution control;
    // it is not evidence for the selected physical helper lease.
    let ordinary = build(ordinary_caller_source, provider_source, leaf_source).unwrap();
    let ordinary_linked = ordinary
        .linked_owned_data_api_program_with_roots("scalar.app", &[])
        .unwrap();
    assert_eq!(
        crate::interpreter::evaluate_resolved_zero_arg_i64(&ordinary_linked, "app.main", 100_000)
            .unwrap()
            .outcome,
        crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(42)
    );
    let caller_source = selected_borrowed_caller(ordinary_caller_source);
    let build = build(&caller_source, provider_source, leaf_source).unwrap();
    assert_selected_borrowed_inventory(&build);
    assert_eq!(schema(&build).unwrap(), "semaprax.graph.v77");
    let linked = build
        .linked_owned_data_api_program_with_roots("scalar.app", &[])
        .unwrap();
    for _ in 0..3 {
        assert_eq!(
            crate::interpreter::evaluate_resolved_zero_arg_i64(&linked, "app.main", 100_000)
                .unwrap()
                .outcome,
            crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(42)
        );
    }
    let ordinary_local_source = r#"module scalar.local;
@id("leaf.length") fn leaf_length(view:borrow Slice<u8>)->usize ensures result==byte_len(view) {byte_len(view)}
@id("helper.length") fn length(view:borrow Slice<u8>)->usize {leaf_length(view)}
@id("app.main") fn main()->i64 {let text=str"é\u{0}";let size=length(str_as_bytes(text));return if size==3usize {42}else{0};}
"#;
    let ordinary_local =
        hir::resolve(&crate::check(ordinary_local_source, "borrowed-ordinary-local.spx").unwrap())
            .unwrap();
    assert_eq!(
        crate::interpreter::evaluate_resolved_zero_arg_i64(&ordinary_local, "app.main", 100_000)
            .unwrap()
            .outcome,
        crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(42)
    );
    let local_source = selected_borrowed_caller(ordinary_local_source);
    let control =
        hir::resolve(&crate::check(&local_source, "borrowed-local.spx").unwrap()).unwrap();
    assert!(control
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap()
        .cleanup_plan
        .lexical_control
        .is_some());
    runtime::run(&linked, &control);
    let (wide, allocation) = measured(&build, 32_000_000, 1 << 26);
    let (work, bytes) =
        wide.expect("selected borrowed replay must expose its actual refusing phase");
    assert!(!allocation.invalid && allocation.allocated <= bytes && allocation.live == 0);
    assert_eq!(measured(&build, work, bytes).0.unwrap(), (work, bytes));
    assert!(measured(&build, work - 1, bytes).0.is_err());
    assert!(measured(&build, work, bytes - 1).0.is_err());
}

#[test]
fn borrowed_provider_parameter_modes_roots_and_loan_proofs_cannot_be_forged() {
    let ordinary_caller_source = r#"module scalar.app; use function @id("leaf.length") from scalar.leaf as length;
@id("app.main") fn main()->i64 {let text=str"abc";let size=length(str_as_bytes(text));return if size==3usize {42}else{0};}"#;
    let leaf_source = r#"module scalar.leaf; @id("leaf.length") fn length(view:borrow Slice<u8>)->usize {byte_len(view)}"#;
    let caller_source = selected_borrowed_caller(ordinary_caller_source);
    let ordinary = build_sources(&[
        ("caller.spx", ordinary_caller_source),
        ("leaf.spx", leaf_source),
    ])
    .unwrap();
    let ordinary_linked = ordinary
        .linked_owned_data_api_program_with_roots("scalar.app", &[])
        .unwrap();
    assert_eq!(
        crate::interpreter::evaluate_resolved_zero_arg_i64(&ordinary_linked, "app.main", 100_000)
            .unwrap()
            .outcome,
        crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(42)
    );
    for (mutation, label) in [
        (0, "parameter ownership"),
        (1, "parameter root identity"),
        (2, "loan schema"),
    ] {
        let mut build =
            build_sources(&[("caller.spx", &caller_source), ("leaf.spx", leaf_source)]).unwrap();
        assert_selected_borrowed_inventory(&build);
        assert_eq!(
            schema(&build).unwrap(),
            "semaprax.graph.v77",
            "baseline {label}"
        );
        let function = &mut build
            .hir
            .modules
            .iter_mut()
            .find(|m| m.module == "scalar.leaf")
            .unwrap()
            .functions[0];
        match mutation {
            0 => function.params[0].ownership = hir::OwnershipMode::Own,
            1 => function.params[0].id = hir::ValueId::intrinsic_parameter("forged.root", 0),
            _ => function.loan_plan.schema = "forged.loan.schema",
        }
        let module = caller(&build);
        let anchor = &module.functions[0];
        let mut resources =
            SelectedReplayPreparation::with_limits(anchor, 32_000_000, 1 << 26).unwrap();
        let rejected = authenticate(
            module,
            &build.hir.modules,
            &build.hir.declarations,
            &mut resources,
        )
        .err()
        .expect("the actual helper receiving seam must refuse mutated authority");
        // The inline refusal is the receiving seam's own Copy cause, not a
        // diagnostic Vec rebuilt after the fact.
        assert!(
            matches!(
                rejected.cause(),
                crate::cleanup_plan::SelectedReplayCause::Mismatch
                    | crate::cleanup_plan::SelectedReplayCause::Constructor
            ),
            "mutation {mutation} ({label}): {rejected:?}"
        );
        assert_eq!(
            resources
                .charge(anchor, 0, "mutated helper retry")
                .unwrap_err()
                .code,
            "SPX-H006",
            "mutation {mutation} ({label})"
        );
        let observed = schema(&build);
        assert!(
            observed.is_err(),
            "mutation {mutation} ({label}): {observed:?}"
        );
    }
}

// A loop-containing return cannot normalize into an ordinary tail value. The
// helper read remains before the one-iteration loop and executes exactly once.
fn selected_borrowed_caller(original: &str) -> String {
    let needle = "return if size==3usize {42}else{0};";
    assert_eq!(original.matches(needle).count(), 1);
    original.replace(needle, "let mut tick=0usize;while tick<1usize {tick=tick+1usize;return if size==3usize {42}else{0};} 0")
}
fn assert_selected_borrowed_inventory(build: &WorkspaceGraphBuild) {
    let module = caller(build);
    assert!(
        module.functions[0].cleanup_plan.lexical_control.is_some(),
        "actual lexical caller"
    );
    let inventory = module
        .static_text
        .as_ref()
        .expect("literal source inventory")
        .lexical
        .as_ref()
        .expect("source-bound primitive inventory");
    assert!(inventory.imported, "selected imported-helper source route");
}

#[test]
fn imported_record_signature_equality_debits_the_actual_nominal_prefix() {
    fn nominal(id: &str) -> hir::ResolvedType {
        hir::ResolvedType::Nominal {
            declaration: hir::DeclarationId::new(id),
            arguments: Vec::new(),
        }
    }
    let a = nominal("rows.Row");
    for (b, expected, units) in [
        (nominal("rows.Row"), true, 10),
        (nominal("rows.Rox"), false, 10),
        (nominal("other.Id"), false, 3),
        (nominal("short"), false, 2),
        (hir::ResolvedType::I64, false, 1),
    ] {
        let mut spent = 0;
        assert_eq!(
            same_type(&a, &b, |n| {
                spent += n;
                Ok(())
            })
            .unwrap(),
            expected
        );
        assert_eq!(spent, units);
        for available in [units, units - 1] {
            let mut left = available;
            let result = same_type(&a, &b, |n| {
                left = left.checked_sub(n).ok_or(Refused::MISMATCH)?;
                Ok(())
            });
            assert_eq!(result.is_ok(), available == units);
        }
    }
}

#[test]
fn original_static_text_provider_numeric_mismatch_keeps_checked_refusal() {
    let provider = PROVIDER.replace(
        "fn step(n:i64)->i64 {leaf_step(n)}",
        "fn step(n:i64)->i64 {let label=str\"x\";if str_len_bytes(label)==1usize {n}else{0}}",
    );
    assert_ne!(provider, PROVIDER);
    let errors = build(CALLER, &provider, LEAF)
        .err()
        .expect("source mismatch");
    assert!(errors.iter().any(|error| error.code == "SPX-T207"));
}

#[test]
fn a_large_comment_keeps_ordinary_imports_admitted_without_authorizing_a_selected_helper() {
    // The full comment-bearing source needs the comment-aware formatter;
    // formatting only its declaration does not canonicalize comment placement.
    let leaf = format!(
        "{}\n//{}\n",
        crate::format::canonical(&crate::parse(LEAF, "leaf.spx").unwrap()),
        "x".repeat(1_048_576)
    );
    let (program, comments) =
        crate::parse_with_comments(&leaf, std::path::Path::new("leaf.spx")).unwrap();
    assert_eq!(comments.items.len(), 1);
    assert_eq!(comments.items[0].text.len(), 1_048_576);
    let leaf = crate::format::comments::canonical_with_comments(&program, &comments);
    assert!(leaf.len() > 1_048_576);
    let (canonical_program, canonical_comments) =
        crate::parse_with_comments(&leaf, std::path::Path::new("leaf.spx")).unwrap();
    assert_eq!(canonical_comments.items[0].text, comments.items[0].text);
    assert_eq!(
        leaf,
        crate::format::comments::canonical_with_comments(&canonical_program, &canonical_comments)
    );
    let build_with_comment = |caller: &str| {
        let sources = [
            ("caller.spx", caller),
            ("provider.spx", PROVIDER),
            ("leaf.spx", &leaf),
        ]
        .into_iter()
        .map(|(path, source)| WorkspaceSource {
            path: path.into(),
            source: if path == "leaf.spx" {
                source.to_owned()
            } else {
                crate::format::canonical(&crate::parse(source, path).unwrap())
            },
        })
        .collect();
        build_owned_retaining_sources_for_change(sources, MAX_CHANGE_BUILDER_BYTES)
            .map(|(build, _)| build)
    };
    let ordinary_caller = r#"module scalar.app;
use function @id("helper.step") from scalar.helper as step;
@id("app.main") fn main()->i64 {step(41)}
"#;
    let ordinary = build_with_comment(ordinary_caller).unwrap();
    let provider = ordinary
        .hir
        .modules
        .iter()
        .find(|module| module.module == "scalar.leaf")
        .unwrap();
    assert!(provider.scalar_helpers.is_none());
    assert!(schema(&ordinary).is_ok());
    let linked = ordinary
        .linked_owned_data_api_program_with_roots("scalar.app", &[])
        .unwrap();
    crate::hir::validate(&linked).unwrap();

    // The same source cannot supply a partial or unhashed proof to the static
    // receiver. Its existing full-inventory guard remains the refusal gate.
    let selected = build_with_comment(CALLER).unwrap();
    let provider = selected
        .hir
        .modules
        .iter()
        .find(|module| module.module == "scalar.leaf")
        .unwrap();
    assert!(provider.scalar_helpers.is_none());
    let errors = schema(&selected).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, "SPX-G410");
    assert_eq!(
        errors[0].message,
        "retained static UTF-8 source-site authority disagrees"
    );
}
