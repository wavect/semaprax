//! Saved schema oracle and actual provider profile controls; no admission hook.
use super::*;
fn line_end() -> crate::hir::ResolvedProgram {
    line_end_source(include_str!("../../../../../std/io-lines/src/lines.spx"))
}
fn original_line_end() -> crate::hir::ResolvedProgram {
    use sha2::Digest;
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/e/12be4b613bd80f0aa93ef65b87c3c59a09454918ceea9d3952f48735ad23fd44"
    ));
    assert_eq!(source.len(), 4129);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(sha2::Sha256::digest(source.as_bytes()))
        ),
        "12be4b613bd80f0aa93ef65b87c3c59a09454918ceea9d3952f48735ad23fd44"
    );
    line_end_source(source)
}
fn line_end_source(source: &str) -> crate::hir::ResolvedProgram {
    let start = source.find("@id(\"std.io.lines.line-end\")").unwrap();
    let rest = &source[start..];
    let end = rest[4..]
        .find("@id(")
        .map_or(rest.len(), |offset| offset + 4);
    let selected = format!(
        "module std.io.lines;\n{}\n@id(\"main\") fn main()->i64 {{0}}",
        &rest[..end]
    );
    crate::hir::resolve(&crate::check(&selected, "actual-line-end.spx").unwrap()).unwrap()
}
fn at_is() -> crate::hir::ResolvedProgram {
    let source = include_str!("../../../../../std/data-json-utf8/src/utf8.spx");
    let start = source.find("@id(\"std.data.json.utf8.at_is\")").unwrap();
    let rest = &source[start..];
    let end = rest[4..]
        .find("@id(")
        .map_or(rest.len(), |offset| offset + 4);
    let selected = format!(
        "module std.data.json.utf8;\n{}\n@id(\"main\") fn main()->i64 {{0}}",
        &rest[..end]
    );
    crate::hir::resolve(&crate::check(&selected, "actual-at-is.spx").unwrap()).unwrap()
}
#[test]
fn actual_provider_profile_is_current_and_keeps_the_original_value_match_schema_oracle() {
    let program = line_end();
    let function = &program.functions[0];
    let original = selected_schema_with_index(&program, function, None).unwrap();
    assert_eq!(original, function.cleanup_plan.schema);
    let mut budget = ReplayBudget::with_physical(function, 1 << 24).unwrap();
    let profile = build(&program, function, &mut budget).unwrap();
    assert!(profile.matches(&program, function));
    assert!(!profile.matches(&program, &function.clone()));
    assert!(!profile.matches(&program.clone(), function));
    assert!(!profile.renewals(&program, function).unwrap().has_helpers());
    assert!(profile.renewals(&program, &function.clone()).is_none());
    assert!(profile.renewals(&program.clone(), function).is_none());
    let before = budget.remaining;
    assert_eq!(
        profile.schema(&program, function, &mut budget).unwrap(),
        original
    );
    // Nineteen evaluated guard probes, one string-length probe, and every
    // byte up to the differing inventory-version digit; no hidden tree scan.
    assert_eq!(
        before - budget.remaining,
        20 + crate::cleanup::CLEANUP_INVENTORY_SCHEMA_V1.len()
    );
    assert_eq!(
        profile
            .schema(&program, &function.clone(), &mut budget)
            .unwrap_err()
            .code,
        "SPX-H006"
    );
    assert_eq!(
        profile
            .schema(&program.clone(), function, &mut budget)
            .unwrap_err()
            .code,
        "SPX-H006"
    );
    assert_eq!(
        profile.inventory(),
        &crate::cleanup::build_inventory(&program, function).unwrap()
    );
    // Value Option elimination does not grant v6. The current proof must agree
    // with the independent unmodified selector, rather than the attached plan.
    assert_eq!(
        selected_schema_with_index(&program, function, None).unwrap(),
        original
    );
    let mut forged = line_end();
    forged.functions[0].cleanup_plan.schema = CLEANUP_PLAN_SCHEMA_V6;
    forged.functions[0].cleanup.schema = crate::cleanup::CLEANUP_INVENTORY_SCHEMA_V2;
    let function = &forged.functions[0];
    let mut budget = ReplayBudget::with_physical(function, 1 << 24).unwrap();
    let profile = build(&forged, function, &mut budget).unwrap();
    assert_eq!(
        profile.schema(&forged, function, &mut budget).unwrap(),
        original
    );
    // The second independently retained provider has a Value Option match at
    // the function root, rather than inside line_end's ordinary loop.
    let direct = at_is();
    let function = &direct.functions[0];
    let ordinary = selected_schema_with_index(&direct, function, None).unwrap();
    let mut budget = ReplayBudget::with_physical(function, 1 << 24).unwrap();
    let profile = build(&direct, function, &mut budget).unwrap();
    assert_eq!(
        profile.schema(&direct, function, &mut budget).unwrap(),
        ordinary
    );
    assert_eq!(ordinary, CLEANUP_PLAN_SCHEMA_V2);
}
#[test]
fn complete_profile_and_inventory_share_exact_work_backing_and_held_pool_through_drop() {
    let program = line_end();
    let function = &program.functions[0];
    fn measured(
        program: &crate::hir::ResolvedProgram,
        work: usize,
        bytes: usize,
    ) -> (
        Result<(usize, usize), Diagnostic>,
        crate::test_allocation_observer::Metrics,
    ) {
        crate::test_allocation_observer::measure(|| {
            let function = &program.functions[0];
            let mut budget = ReplayBudget::with_physical(function, bytes)?;
            budget.remaining = work;
            budget.skeleton_remaining = 101;
            let profile = build(program, function, &mut budget)?;
            assert!(profile.matches(program, function));
            assert_eq!(
                profile.schema(program, function, &mut budget)?,
                CLEANUP_PLAN_SCHEMA_V2
            );
            assert_eq!(budget.skeleton_remaining, 101);
            assert!(!budget.lent_skeleton);
            let receipt = (
                work - budget.remaining,
                budget.physical.as_ref().unwrap().charged(),
            );
            drop(profile);
            drop(budget);
            Ok(receipt)
        })
    }
    let (wide, allocation) = measured(&program, MAX_REPLAY_WORK_UNITS - 101, 1 << 24);
    let (work, bytes) = wide.unwrap();
    assert!(!allocation.invalid && allocation.allocated <= bytes && allocation.live == 0);
    let (exact, allocation) = measured(&program, work, bytes);
    assert_eq!(exact.unwrap(), (work, bytes));
    assert!(!allocation.invalid && allocation.allocated <= bytes && allocation.live == 0);
    assert_eq!(
        measured(&program, work - 1, bytes).0.unwrap_err().code,
        "SPX-H006"
    );
    assert_eq!(
        measured(&program, work, bytes - 1).0.unwrap_err().code,
        "SPX-H006"
    );
    // Pointer-only clones cannot inherit the current function/context witness.
    let clone = function.clone();
    let mut budget = ReplayBudget::with_physical(function, 1 << 24).unwrap();
    assert_eq!(
        build(&program, &clone, &mut budget).err().unwrap().code,
        "SPX-H006"
    );
}
#[test]
fn independently_mutated_modes_roots_option_metadata_and_loan_generation_refuse() {
    for mutation in 0..4 {
        let mut program = original_line_end();
        match mutation {
            0 => program.functions[0].params[0].ownership = OwnershipMode::Own,
            1 => {
                program.functions[0].params[0].id =
                    crate::hir::ValueId::intrinsic_parameter("forged.view", 0)
            }
            2 => crate::hir::borrowed_scalar::option_u8::forge_parameter_index_for_test(
                &mut program.declarations,
                1,
            ),
            _ => program.functions[0].loan_plan.schema = "forged.loan.generation",
        }
        let function = &program.functions[0];
        let mut budget = ReplayBudget::with_physical(function, 1 << 24).unwrap();
        assert_eq!(
            build(&program, function, &mut budget).err().unwrap().code,
            "SPX-H006",
            "profile mutant {mutation}"
        );
    }
}
#[test]
fn original_lexical_and_writer_priorities_are_not_conferred_by_the_borrowed_profile() {
    let source = "module lexical; @id(\"lexical.main\") fn main()->i64 {while true {if true {return 42;}else{0} 0} return 0;}";
    let program =
        crate::hir::resolve(&crate::check(source, "lexical-profile.spx").unwrap()).unwrap();
    let function = &program.functions[0];
    assert!(function.cleanup_plan.lexical_control.is_some());
    assert_eq!(
        selected_schema_with_index(&program, function, None).unwrap(),
        CLEANUP_PLAN_SCHEMA_V18
    );
    let mut budget = ReplayBudget::with_physical(function, 1 << 24).unwrap();
    assert!(build(&program, function, &mut budget).is_err());
    let source = include_str!("../../../../../tests/owned_data/byte_helper_renewal.spx");
    let program =
        crate::hir::resolve(&crate::check(source, "writer-profile.spx").unwrap()).unwrap();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    assert_eq!(
        selected_schema_with_index(&program, function, None).unwrap(),
        crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19
    );
    let mut budget = ReplayBudget::with_physical(function, 1 << 24).unwrap();
    assert!(build(&program, function, &mut budget).is_err());
}

#[test]
fn exact_value_option_profile_does_not_hide_the_original_borrow_match_schema() {
    let mut program = original_line_end();
    let function = &mut program.functions[0];
    let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!("actual line-end block");
    };
    let body = statements
        .iter_mut()
        .find_map(|statement| match statement {
            ResolvedStatement::While { body, .. } => Some(body),
            _ => None,
        })
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &mut body.kind else {
        panic!("actual while block");
    };
    let ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!("actual terminator binding");
    };
    let ResolvedExprKind::Match { mode, .. } = &mut value.kind else {
        panic!("actual optional byte match");
    };
    *mode = crate::hir::ResolvedMatchMode::Borrow;
    let function = &program.functions[0];
    assert_eq!(
        selected_schema_with_index(&program, function, None).unwrap(),
        CLEANUP_PLAN_SCHEMA_V6
    );
    let mut budget = ReplayBudget::with_physical(function, 1 << 24).unwrap();
    assert!(build(&program, function, &mut budget).is_err());
}

#[test]
fn prefix_class_profile_requires_every_actual_read_operand_and_exact_identity() {
    let original = crate::hir::resolve(
        &crate::check(
            "module prefix.profile; @id(\"prefix.read\") fn read(input: borrow Slice<u8>, index: usize, count: usize)->usize { byte_prefix_class(input, index, count, 0u8, 127u8, 10u8, 13u8) } @id(\"main\") fn main()->i64 {0}",
            "prefix-profile.spx",
        ).unwrap(),
    ).unwrap();
    let index = original
        .functions
        .iter()
        .position(|f| f.name == "read")
        .unwrap();
    for mutant in 0..17 {
        let mut program = original.clone();
        {
            let function = &mut program.functions[index];
            let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
                panic!("actual prefix helper block");
            };
            let ResolvedExprKind::Call {
                callee,
                type_arguments,
                args,
                ..
            } = &mut tail.kind
            else {
                panic!("actual prefix read call");
            };
            match mutant {
                0 => {}
                1 => {
                    args.pop();
                }
                2 => args.push(args[6].clone()),
                3 => args[0].ownership = crate::hir::OwnershipMode::Value,
                4 => args[0].ty = ResolvedType::Str,
                5 => args[1].ty = ResolvedType::I64,
                6 => args[2].ownership = crate::hir::OwnershipMode::Borrow,
                7 => args[3].ty = ResolvedType::Bool,
                8 => args[6].ownership = crate::hir::OwnershipMode::Borrow,
                9 => type_arguments.push(ResolvedType::U8),
                10 => *callee = crate::hir::DeclarationId::new("core.bytes.prefix-class.forged"),
                11 => tail.ty = ResolvedType::Bool,
                12 => tail.ownership = crate::hir::OwnershipMode::Borrow,
                13 => function.params[0].ownership = crate::hir::OwnershipMode::Value,
                14 => function.params[0].ty = ResolvedType::Str,
                15 => {
                    function.params[0].id =
                        crate::hir::ValueId::intrinsic_parameter("forged.view", 0)
                }
                16 => function.loan_plan.schema = "forged.loan.generation",
                _ => unreachable!(),
            }
        }
        let function = &program.functions[index];
        let mut budget = ReplayBudget::with_physical(function, 1 << 24).unwrap();
        let result = build(&program, function, &mut budget);
        assert_eq!(
            result.is_ok(),
            mutant == 0,
            "exact prefix signature mutant={mutant}"
        );
        if let Ok(profile) = result {
            assert_eq!(
                profile.schema(&program, function, &mut budget).unwrap(),
                selected_schema_with_index(&program, function, None).unwrap()
            );
        }
    }
}
