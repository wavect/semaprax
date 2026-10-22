//! Actual selected supplemental walk, with complete original child controls.
use super::*;
use crate::test_allocation_observer::measure;
fn program() -> crate::hir::ResolvedProgram {
    crate::hir::resolve(
        &crate::check(
            r#"module selected.supplemental.family;
@id("supp.fill") fn fill(data:own Bytes)->Bytes {data}
@id("supp.main") fn main()->i64 {
 let data=fill(bytes_zeroed(1usize));
 if byte_len(bytes_as_slice(data))==1usize {7}else{0}
}
"#,
            "selected-supplemental-family.spx",
        )
        .unwrap(),
    )
    .unwrap()
}
fn call(expression: &ResolvedExpr) -> Option<&ResolvedExpr> {
    if matches!(&expression.kind,ResolvedExprKind::Call{callee,..} if callee.as_str()=="supp.fill")
    {
        return Some(expression);
    }
    let mut next = 0;
    while let Some(child) = replay_expression_child(expression, next) {
        if let Some(found) = call(child) {
            return Some(found);
        }
        next += 1;
    }
    None
}
fn render(error: EngineError, function: &ResolvedFunction) -> Diagnostic {
    match error {
        EngineError::Owned(error) => error,
        EngineError::Structural(receipt) => receipt.legacy(function),
        other => panic!("unexpected supplemental refusal {other:?}"),
    }
}
#[test]
fn selected_supplemental_success_and_hostile_children_match_complete_original() {
    let program = program();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "supp.main")
        .unwrap();
    let original = call(&function.body).unwrap();
    for case in 0..6 {
        let mut expression = original.clone();
        if let ResolvedExprKind::Call {
            callee,
            type_arguments,
            args,
            ..
        } = &mut expression.kind
        {
            match case {
                0 => {}
                1 => type_arguments.push(ResolvedType::I64),
                2 => args.clear(),
                3 => *callee = DeclarationId::new("supp.absent"),
                4 => args[0].ty = ResolvedType::String,
                5 => {
                    let extra = args[0].clone();
                    args.push(extra);
                }
                _ => unreachable!(),
            }
        } else {
            unreachable!()
        }
        let mut before = ReplayBudget::with_physical(function, usize::MAX).unwrap();
        let mut after = ReplayBudget::with_physical(function, usize::MAX).unwrap();
        let (mut a, mut b) = (Vec::new(), Vec::new());
        let (mut af, mut bf) = (0, 0);
        let old = super::super::super::collect_supplemental(
            &program,
            function,
            &expression,
            &mut af,
            &mut a,
            &mut before,
        );
        let current =
            collect_supplemental(&program, function, &expression, &mut bf, &mut b, &mut after)
                .map_err(|e| render(e, function));
        assert_eq!(
            current, old,
            "complete six fields and first refusal case {case}"
        );
        assert_eq!(af, bf);
        assert_eq!(a.len(), b.len());
        for (old, current) in a.iter().zip(&b) {
            assert_eq!(old.storage, current.storage);
            assert_eq!(old.ty, current.ty);
            assert_eq!(old.shape, current.shape);
        }
        assert_eq!(a.len(), usize::from(case == 0));
    }
}
#[test]
fn selected_supplemental_depth_keeps_original_bound_refusal_and_complete_drop() {
    let program = program();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "supp.main")
        .unwrap();
    let template = call(&function.body).unwrap();
    let mut deep = template.clone();
    for _ in 0..600 {
        let mut next = template.clone();
        let ResolvedExprKind::Call { args, .. } = &mut next.kind else {
            unreachable!()
        };
        args[0] = deep;
        deep = next;
    }
    let mut reference_budget = ReplayBudget::with_physical(function, usize::MAX).unwrap();
    let expected = super::super::super::collect_supplemental(
        &program,
        function,
        &deep,
        &mut 0,
        &mut Vec::new(),
        &mut reference_budget,
    )
    .unwrap_err();
    assert!(expected
        .message
        .contains("supplemental-slot traversal exceeds the admitted depth"));
    drop(reference_budget);
    for old in [true, false] {
        let ((flags, slots), observed) = measure(|| {
            let mut budget = ReplayBudget::with_physical(function, usize::MAX).unwrap();
            let mut flags = 0;
            let mut slots = Vec::new();
            let result = if old {
                super::super::super::collect_supplemental(
                    &program,
                    function,
                    &deep,
                    &mut flags,
                    &mut slots,
                    &mut budget,
                )
            } else {
                collect_supplemental(
                    &program,
                    function,
                    &deep,
                    &mut flags,
                    &mut slots,
                    &mut budget,
                )
                .map_err(|e| render(e, function))
            };
            assert_eq!(
                result.unwrap_err(),
                expected,
                "complete original bound and all diagnostic fields"
            );
            (flags, slots.len())
        });
        assert_eq!(flags, 0);
        assert_eq!(slots, 0);
        assert!(!observed.invalid && observed.live == 0);
        assert_eq!(observed.allocated, observed.freed);
    }
}
