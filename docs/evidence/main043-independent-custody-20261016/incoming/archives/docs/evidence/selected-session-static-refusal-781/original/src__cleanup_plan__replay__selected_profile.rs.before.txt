//! The first physically reserved retained lexical profile is deliberately
//! primitive/direct-flat-record and module-local. Refusal happens before delegated nested nominal,
//! dynamic provider, replacement, format or iterator constructors allocate.
use super::*;
pub(super) mod collections;
pub(super) mod conditions;
#[cfg(test)]
mod receiving_tests;
mod row_loan;
pub(super) mod string_transfer;
const DEPTH: usize = 514;
type Frame<'a> = Option<(crate::hir::lexical_exit::ChildCursor<'a>, bool)>;
pub(super) fn validate(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    #[cfg(test)]
    if let Some(lease) = program.borrowed_helper_lease(function) {
        return lease.validate(program, function, budget);
    }
    if !budget.selected() {
        return Ok(());
    }
    flat_records::module(program, function, budget)?;
    if matches!(function.return_type, ResolvedType::Nominal { .. }) {
        collections::prepay(
            program,
            function,
            collections::type_key_bytes(&function.return_type)?,
            budget,
        )?;
    }
    if !program.interfaces().is_empty()
        || !program.function_templates().is_empty()
        || !program.function_instances().is_empty()
        || !function.effects.is_empty()
        || function.yields.is_some()
        || !flat_records::admitted(program, &function.return_type)?
    {
        return Err(replay_error(
            function,
            "selected lexical replay requires a module-local primitive context",
        ));
    }
    for parameter in &function.params {
        if matches!(parameter.ty, ResolvedType::Nominal { .. }) {
            collections::prepay(
                program,
                function,
                collections::type_key_bytes(&parameter.ty)?,
                budget,
            )?;
        }
        if !flat_records::admitted(program, &parameter.ty)? {
            return Err(replay_error(
                function,
                "selected parameter type is outside direct records",
            ));
        }
    }
    budget.reserve_array::<Frame<'_>>(function, DEPTH, "selected primitive profile cursor")?;
    let mut frames = [None; DEPTH];
    for root in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        frames[0] = Some((crate::hir::lexical_exit::ChildCursor::new(root), false));
        let mut depth = 1;
        while depth != 0 {
            budget.charge(function, 1, "selected primitive profile node")?;
            let (cursor, entered) = frames[depth - 1]
                .ok_or_else(|| replay_error(function, "selected profile cursor is empty"))?;
            let expression = cursor.expression();
            if !entered {
                if matches!(expression.ty, ResolvedType::Nominal { .. }) {
                    collections::prepay(
                        program,
                        function,
                        collections::type_key_bytes(&expression.ty)?,
                        budget,
                    )?;
                }
                if expression.ty != ResolvedType::Never
                    && !flat_records::admitted(program, &expression.ty)?
                {
                    return Err(replay_error(function, "selected lexical replay does not admit nominal or callable expression storage"));
                }
                if let ResolvedExprKind::Project { base, .. } = &expression.kind {
                    collections::prepay(
                        program,
                        function,
                        collections::type_key_bytes(&base.ty)?,
                        budget,
                    )?;
                }
                flat_records::expression(program, function, expression)?;
                match &expression.kind {
                    ResolvedExprKind::ConstructVariant { .. }
                    | ResolvedExprKind::UpdateRecord { .. }
                    | ResolvedExprKind::Upcast { .. }
                    | ResolvedExprKind::LiteralFormat { .. }
                    | ResolvedExprKind::Try { .. }
                    | ResolvedExprKind::TryOption { .. }
                    | ResolvedExprKind::Closure { .. }
                    | ResolvedExprKind::Invoke { .. }
                    | ResolvedExprKind::FunctionReference { .. }
                    | ResolvedExprKind::NativeRustImportCall(_)
                    | ResolvedExprKind::HostCommandCall(_)
                    | ResolvedExprKind::Yield { .. } => {
                        return Err(replay_error(function, "selected lexical replay provider is outside the reserved primitive profile"));
                    }
                    ResolvedExprKind::VecFieldRead { .. } => {
                        collections::field(program, function, expression, budget)?
                    }
                    ResolvedExprKind::RowLoan { .. } => {
                        row_loan::validate(program, function, expression, budget)?
                    }
                    ResolvedExprKind::BorrowedUtf8Literal(value) => {
                        if expression.ty != ResolvedType::Str
                            || expression.ownership != OwnershipMode::Borrow
                            || !value.is_backed()
                        {
                            return Err(replay_error(
                                function,
                                "selected static literal has invalid type, ownership or backing",
                            ));
                        }
                        crate::hir::static_text::validate_literal(value)?;
                    }
                    ResolvedExprKind::Block { statements, .. }
                    | ResolvedExprKind::VecConstructor { statements, .. } => {
                        budget.charge(
                            function,
                            statements.len(),
                            "selected primitive statements",
                        )?;
                        for statement in statements {
                            if let crate::hir::ResolvedStatement::Let { binding, .. }
                            | crate::hir::ResolvedStatement::Assign { binding, .. } = statement
                            {
                                if matches!(binding.ty, ResolvedType::Nominal { .. }) {
                                    collections::prepay(
                                        program,
                                        function,
                                        collections::type_key_bytes(&binding.ty)?,
                                        budget,
                                    )?;
                                }
                            }
                            match statement {
                                crate::hir::ResolvedStatement::Let { binding, .. }
                                | crate::hir::ResolvedStatement::Assign { binding, .. }
                                    if !flat_records::admitted(program, &binding.ty)? =>
                                {
                                    return Err(replay_error(
                                        function,
                                        "selected block binding requires unsupported storage",
                                    ));
                                }
                                crate::hir::ResolvedStatement::Assign {
                                    field: Some(_), ..
                                } => {
                                    return Err(replay_error(function, "selected primitive assignment cannot target a nominal field"));
                                }
                                crate::hir::ResolvedStatement::LexicalWhile {
                                    condition,
                                    body,
                                    latch,
                                    ..
                                } => {
                                    if matches!(
                                        latch,
                                        crate::hir::LoopLatchKind::OwnedIteratorStep { .. }
                                            | crate::hir::LoopLatchKind::SourceForStep {
                                                kind: crate::hir::SourceForKind::OwnedIterator,
                                                ..
                                            }
                                    ) {
                                        return Err(replay_error(function,"selected lexical replay owning iterator latch remains closed"));
                                    }
                                    if matches!(
                                        latch,
                                        crate::hir::LoopLatchKind::SourceForStep {
                                            kind: crate::hir::SourceForKind::Copy,
                                            ..
                                        }
                                    ) {
                                        let ResolvedExprKind::Block { statements, .. } = &body.kind
                                        else {
                                            return Err(replay_error(
                                                function,
                                                "selected Copy-for body is absent",
                                            ));
                                        };
                                        let Some(crate::hir::ResolvedStatement::Let {
                                            binding,
                                            ..
                                        }) = statements.first()
                                        else {
                                            return Err(replay_error(
                                                function,
                                                "selected Copy-for item is absent",
                                            ));
                                        };
                                        if !crate::vec_ops::resolved_element_is_admitted(
                                            &binding.ty,
                                        ) {
                                            return Err(replay_error(function,"selected Copy-for item is outside its scalar family"));
                                        }
                                        // Ordinary latch validation constructs exactly one Vec type
                                        // (one argument and its owned declaration spelling).
                                        budget.reserve_array::<ResolvedType>(
                                            function,
                                            1,
                                            "selected Copy-for latch Vec argument",
                                        )?;
                                        budget.reserve_bytes(
                                            function,
                                            crate::prelude::VEC_ID.len(),
                                            "selected Copy-for latch Vec identity",
                                        )?;
                                    }
                                    crate::hir::lexical_exit::validate_latch(
                                        condition, body, latch,
                                    )?;
                                }
                                crate::hir::ResolvedStatement::Assign {
                                    binding, value, ..
                                } if binding.ty == ResolvedType::String => {
                                    collections::append(function, binding, value, budget)?;
                                }
                                crate::hir::ResolvedStatement::Unsafe { .. } => {
                                    return Err(replay_error(
                                        function,
                                        "selected lexical replay has an unsafe statement",
                                    ));
                                }
                                _ => {}
                            }
                        }
                    }
                    ResolvedExprKind::Match {
                        scrutinee, arms, ..
                    } => {
                        if matches!(scrutinee.ty, ResolvedType::Nominal { .. }) {
                            return Err(replay_error(function, "selected direct records do not admit owning or destructuring matches"));
                        }
                        for arm in arms {
                            validate_pattern(function, &arm.pattern, budget, 0)?;
                        }
                    }
                    ResolvedExprKind::Call {
                        callee,
                        instance,
                        type_arguments,
                        ..
                    } => {
                        string_transfer::operand(function, expression, budget)?;
                        if collections::call(program, function, expression, budget)? {
                            frames[depth - 1] = Some((cursor, true));
                            continue;
                        }
                        if instance.is_some() || !type_arguments.is_empty() {
                            return Err(replay_error(
                                function,
                                "selected lexical replay call needs an unsupported provider",
                            ));
                        }
                        if crate::byte_ops::by_id(callee.as_str()).is_none()
                            && crate::string_ops::by_id(callee.as_str()).is_none()
                            && crate::str_ops::by_id(callee.as_str()).is_none()
                        {
                            budget.charge(
                                function,
                                program.function_count(),
                                "selected current callee lookup",
                            )?;
                            if program.resolve_call_target(callee, None).is_none() {
                                return Err(replay_error(
                                    function,
                                    "selected lexical replay call has no current primitive target",
                                ));
                            }
                        }
                    }
                    _ => {}
                }
                frames[depth - 1] = Some((cursor, true));
                continue;
            }
            // The entry census prepays block statement boundaries. Saved
            // cursor progress makes each advance (including None) constant.
            let mut cursor = cursor;
            if let Some(child) = cursor.next() {
                if depth == DEPTH {
                    return Err(replay_error(
                        function,
                        "selected primitive profile exceeds ordinary HIR depth",
                    ));
                }
                frames[depth - 1] = Some((cursor, true));
                frames[depth] = Some((crate::hir::lexical_exit::ChildCursor::new(child), false));
                depth += 1;
            } else {
                depth -= 1;
            }
        }
    }
    Ok(())
}

// Match bindings remain on the frozen primitive pattern boundary. Direct
// records are admitted as ordinary construction/calls/projection, not patterns.
fn primitive(ty: &ResolvedType) -> bool {
    crate::hir::is_scalar_resolved_type(ty)
        || matches!(
            ty,
            ResolvedType::Bytes
                | ResolvedType::String
                | ResolvedType::Str
                | ResolvedType::SliceU8
                | ResolvedType::ArrayU8(_)
        )
}

fn validate_pattern(
    function: &ResolvedFunction,
    pattern: &ResolvedMatchPattern,
    budget: &mut ReplayBudget,
    depth: usize,
) -> Result<(), Diagnostic> {
    budget.charge(function, 1, "selected primitive match pattern")?;
    if depth > 256 {
        return Err(replay_error(
            function,
            "selected match pattern exceeds depth",
        ));
    }
    match pattern {
        ResolvedMatchPattern::Wildcard | ResolvedMatchPattern::Literal(_) => Ok(()),
        ResolvedMatchPattern::Binding(binding) if primitive(&binding.ty) => Ok(()),
        ResolvedMatchPattern::Or(alternatives) => {
            for pattern in alternatives {
                validate_pattern(function, pattern, budget, depth + 1)?;
            }
            Ok(())
        }
        ResolvedMatchPattern::Binding(_)
        | ResolvedMatchPattern::Variant { .. }
        | ResolvedMatchPattern::Record { .. } => Err(replay_error(
            function,
            "selected match pattern requires unsupported storage",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn checked() -> crate::hir::ResolvedProgram {
        let parsed = crate::parse(
            "module app; @id(\"app.main\") fn main()->i64 {if true {0} else {1}}",
            "selected-profile-cursor.spx",
        )
        .unwrap();
        crate::hir::resolve(&parsed).unwrap()
    }
    fn budget(limit: usize) -> ReplayBudget {
        let mut budget = ReplayBudget::new();
        budget.admission = AdmissionMode::SelectedRetained;
        budget.payer = AllocationPayer::SelectedReservation;
        budget.physical = Some(Box::new(
            super::super::physical_storage::PhysicalReservation::new(limit),
        ));
        budget
    }
    #[test]
    fn child_cursor_frames_reserve_exact_and_one_short_before_profile_scan() {
        let program = checked();
        let context = super::super::primitive_fixture::Context::new(&program);
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap();
        let bytes = std::mem::size_of::<[Frame<'_>; DEPTH]>();
        let mut exact = budget(bytes);
        validate(&context, function, &mut exact).unwrap();
        assert_eq!(exact.physical.as_ref().unwrap().charged(), bytes);
        let mut short = budget(bytes - 1);
        assert_eq!(
            validate(&context, function, &mut short).unwrap_err().code,
            "SPX-H006"
        );
        assert!(short.physical.as_mut().unwrap().reserve(0).is_err());
    }
    #[test]
    fn child_cursor_checks_the_late_branch_without_granting_hostile_nominal_storage() {
        let program = checked();
        let context = super::super::primitive_fixture::Context::new(&program);
        let mut function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap()
            .clone();
        let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
            panic!("body block")
        };
        let ResolvedExprKind::If { else_branch, .. } = &mut tail.kind else {
            panic!("if tail")
        };
        else_branch.ty = ResolvedType::Nominal {
            declaration: crate::hir::DeclarationId::new("forged.record"),
            arguments: Vec::new(),
        };
        let error = validate(&context, &function, &mut budget(usize::MAX)).unwrap_err();
        assert_eq!(error.code, "SPX-H006");
        assert!(error
            .message
            .contains("unauthenticated or unsupported fields"));
    }
}

#[cfg(test)]
mod cursor_work_tests {
    use super::*;
    use crate::workspace_graph::SourceRecordReplayFixture;

    fn source(width: usize) -> String {
        let statements = (0..width)
            .map(|i| format!("let n{i}=0;"))
            .collect::<String>();
        format!("module app; @id(\"app.main\") fn main()->i64 {{{statements}0}}")
    }
    fn authenticated_fixture(source: &str) -> SourceRecordReplayFixture {
        // The measured main is unchanged. This separate ordinary source helper
        // supplies the real static literal and lexical profile required by the
        // retained source-bound context; no declaration facts are discarded.
        SourceRecordReplayFixture::new(&authenticated_source(source))
    }
    fn authenticated_source(source: &str) -> String {
        format!(
            r#"{source}
@id("app.cursor_witness") fn cursor_witness()->i64 {{
    let key=str"x";
    let mut next=0;
    while next<1 {{
        next=next+1;
        if str_len_bytes(key)==1 {{return 1;}} else {{0}}
        0
    }}
    0
}}"#
        )
    }
    fn main(fixture: &SourceRecordReplayFixture) -> &ResolvedFunction {
        fixture
            .functions()
            .iter()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap()
    }
    fn run(
        context: &dyn crate::cleanup::ReplayContext,
        function: &ResolvedFunction,
        limit: usize,
    ) -> Result<usize, Diagnostic> {
        let mut budget = ReplayBudget::new();
        budget.remaining = limit;
        budget.admission = AdmissionMode::SelectedRetained;
        budget.payer = AllocationPayer::SelectedReservation;
        budget.physical = Some(Box::new(
            super::super::physical_storage::PhysicalReservation::new(usize::MAX),
        ));
        validate(context, function, &mut budget)?;
        Ok(budget.remaining)
    }
    fn standalone_stays_closed(source: &str) {
        // Keep every compiler-owned declaration and its identity facts intact.
        // Full standalone metadata is not an authenticated module projection.
        let parsed = crate::check(source, "linear-profile-full.spx").unwrap();
        let program = crate::hir::resolve(&parsed).unwrap();
        assert!(program.types.iter().any(|ty| {
            program
                .declarations
                .declaration(&ty.id)
                .is_some_and(|fact| {
                    fact.identity_origin == crate::hir::IdentityOrigin::CompilerOwned
                })
        }));
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap();
        let error = run(&program, function, usize::MAX).unwrap_err();
        assert_eq!(error.code, "SPX-H006");
        assert!(
            error
                .message
                .contains("unauthenticated or unsupported fields"),
            "{error:?}"
        );
    }
    #[test]
    fn wide_statement_census_and_terminal_advances_have_exact_linear_work() {
        for width in [0, 1, 64, 512] {
            let source = source(width);
            standalone_stays_closed(&source);
            let fixture = authenticated_fixture(&source);
            fixture.with_context(|context| {
                let function = main(&fixture);
                // Block+tail+initializers: entry and terminal probes, each edge,
                // and exactly one statement census; no prefix re-scan term.
                let exact = 4 * width + 5;
                assert_eq!(run(context, function, exact).unwrap(), 0);
                assert_eq!(
                    run(context, function, exact - 1).unwrap_err().code,
                    "SPX-H006"
                );
            });
        }
    }
    #[test]
    fn guarded_match_patterns_and_all_contract_roots_keep_their_work_debits() {
        // Real checked source gives every arm/guard its own source identity;
        // the final wildcard makes the scalar match exhaustive.
        let arms = (0..4)
            .map(|i| {
                let pattern = if i == 3 {
                    "_".to_owned()
                } else {
                    i.to_string()
                };
                let guard = if i % 2 == 0 { " if true" } else { "" };
                format!("{pattern}{guard}=>0,")
            })
            .collect::<String>();
        let matched_source =
            format!("module app; @id(\"app.main\") fn main()->i64 {{match 0 {{{arms}}}}}");
        standalone_stays_closed(&matched_source);
        let matched = authenticated_fixture(&matched_source);
        matched.with_context(|context| {
            let function = main(&matched);
            let ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
                panic!("body block")
            };
            let ResolvedExprKind::Match { arms, .. } = &tail.kind else {
                panic!("match tail")
            };
            assert_eq!(arms.len(), 4);
            assert_eq!(arms.iter().filter(|arm| arm.guard.is_some()).count(), 2);
            let exact = 8 + 4 * 4 + 3 * 2;
            assert_eq!(run(context, function, exact).unwrap(), 0);
            assert_eq!(
                run(context, function, exact - 1).unwrap_err().code,
                "SPX-H006"
            );
        });
        // Retain the previous 16-arm witness at its actual admission boundary:
        // symbolic guarded histories exhaust the unchanged branch-state budget
        // before the selected cursor proof can execute.
        let branch_arms = (0..16)
            .map(|i| {
                let pattern = if i == 15 {
                    "_".to_owned()
                } else {
                    i.to_string()
                };
                let guard = if i % 2 == 0 { " if true" } else { "" };
                format!("{pattern}{guard}=>0,")
            })
            .collect::<String>();
        let branch_source =
            format!("module app; @id(\"app.main\") fn main()->i64 {{match 0 {{{branch_arms}}}}}");
        let Err(branch_error) =
            SourceRecordReplayFixture::try_new(&authenticated_source(&branch_source))
        else {
            panic!("guarded histories must exhaust the authenticated plan budget");
        };
        assert!(branch_error.iter().any(|error| error.code == "SPX-H006"
            && error
                .message
                .contains("cleanup replay work budget exhausted during plan branch state push")));
        // Preserve the original large guarded source as an explicit unchanged
        // terminal-path limit refusal, independently of this cursor-work gate.
        let oversized_arms = (0..64)
            .map(|i| {
                let pattern = if i == 63 {
                    "_".to_owned()
                } else {
                    i.to_string()
                };
                let guard = if i % 2 == 0 { " if true" } else { "" };
                format!("{pattern}{guard}=>0,")
            })
            .collect::<String>();
        let oversized_source = format!(
            "module app; @id(\"app.main\") fn main()->i64 {{match 0 {{{oversized_arms}}}}}"
        );
        let Err(error) =
            SourceRecordReplayFixture::try_new(&authenticated_source(&oversized_source))
        else {
            panic!("oversized histories must exhaust the authenticated terminal-path budget");
        };
        assert!(error.iter().any(|error| error.code == "SPX-H006"
            && error.message.contains("65537 terminal control-flow paths")));
        // Separate roots share the same global ReplayBudget; no fresh permit
        // appears at requires/body/ensures transitions.
        let contracts_source =
            "module app; @id(\"app.main\") fn main()->i64 requires true ensures true {0}";
        standalone_stays_closed(contracts_source);
        let contracts = authenticated_fixture(contracts_source);
        contracts.with_context(|context| {
            let function = main(&contracts);
            assert_eq!(function.requires.len(), 1);
            assert_eq!(function.ensures.len(), 1);
            assert_eq!(run(context, function, 9).unwrap(), 0);
            assert_eq!(run(context, function, 8).unwrap_err().code, "SPX-H006");
        });
    }
    #[test]
    fn deep_cursor_has_exact_work_and_retains_depth_and_late_type_refusal() {
        let deep_source = format!(
            "module app; @id(\"app.main\") fn main()->i64 {{{}0}}",
            "-".repeat(32)
        );
        standalone_stays_closed(&deep_source);
        let fixture = authenticated_fixture(&deep_source);
        fixture.with_context(|context| {
            let function = main(&fixture);
            let ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
                panic!("body block")
            };
            let mut expression = tail.as_ref();
            let mut count = 0;
            while let ResolvedExprKind::Unary { value, .. } = &expression.kind {
                count += 1;
                expression = value;
            }
            assert_eq!(
                count, 32,
                "the actual checked source retains every unary node"
            );
            assert_eq!(run(context, function, 5 + 3 * 32).unwrap(), 0);
            assert_eq!(
                run(context, function, 5 + 3 * 32 - 1).unwrap_err().code,
                "SPX-H006"
            );
            // After authenticating the unmodified source, mutate only a cloned
            // function for the hostile depth gate; held source facts stay intact.
            let mut forged = function.clone();
            let ResolvedExprKind::Block { tail, .. } = &mut forged.body.kind else {
                panic!("body block")
            };
            for _ in 32..DEPTH {
                let inner = tail.clone();
                tail.kind = ResolvedExprKind::Unary {
                    op: crate::ast::UnaryOp::Neg,
                    value: inner,
                };
            }
            let error = run(context, &forged, usize::MAX).unwrap_err();
            assert_eq!(error.code, "SPX-H006");
            assert!(
                error.message.contains("ordinary HIR depth"),
                "work cannot buy depth: {error:?}"
            );
        });
        let late = authenticated_fixture(&source(64));
        late.with_context(|context| {
            let function = main(&late);
            assert_eq!(run(context, function, 4 * 64 + 5).unwrap(), 0);
            let mut forged = function.clone();
            let ResolvedExprKind::Block { tail, .. } = &mut forged.body.kind else {
                panic!("body block")
            };
            tail.ty = ResolvedType::Nominal {
                declaration: crate::hir::DeclarationId::new("forged"),
                arguments: Vec::new(),
            };
            let error = run(context, &forged, usize::MAX).unwrap_err();
            assert_eq!(error.code, "SPX-H006");
            assert!(
                error
                    .message
                    .contains("unauthenticated or unsupported fields"),
                "late unsupported storage remains checked: {error:?}"
            );
        });
    }
}
