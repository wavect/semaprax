//! Legacy text/order oracle and inline selected fact receipts.
use super::*;
use crate::cleanup_plan::replay::expression_facts::errors as policies;
pub(in crate::cleanup_plan::replay::expression_facts) mod old;

#[test]
fn typed_fact_policy_matches_saved_legacy_text_order_and_full_facts() {
    let program = program();
    for original in &program.functions {
        let old = old::expression_facts_with_reservation(
            original,
            &mut PhysicalReservation::new(usize::MAX),
        )
        .unwrap();
        let current =
            expression_facts_with_reservation(original, &mut PhysicalReservation::new(usize::MAX))
                .unwrap();
        same_owned(&old, &current);
    }
    for name in [
        "fact.read",
        "λ長",
        "long.unicode.λ.識別子.repeat.repeat.repeat",
    ] {
        let mut function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "fact.read")
            .unwrap()
            .clone();
        function.id = DeclarationId::new(name);
        function.body.id = ExpressionId::from_owned(format!("{name}.{}", "λ".repeat(512)));
        let duplicate = function.body.id.clone();
        let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
            panic!("block")
        };
        tail.id = duplicate.clone();
        // Duplicate precedes a second hostile descendant. It must be refused
        // before that descendant is visited or its fact backing is requested.
        tail.kind = ResolvedExprKind::Unary {
            op: crate::ast::UnaryOp::Neg,
            value: Box::new(
                function
                    .requires
                    .first()
                    .cloned()
                    .unwrap_or_else(|| tail.as_ref().clone()),
            ),
        };
        let old = old::expression_facts(&function).err().unwrap();
        let current = expression_facts(&function).err().unwrap();
        assert_eq!((current.code, &current.message), (old.code, &old.message));
        let selected =
            with_policy::<policies::Selected>(&function, &mut PhysicalReservation::new(usize::MAX))
                .err()
                .unwrap();
        let policies::EngineError::Fact(receipt) = selected else {
            panic!("typed semantic receipt")
        };
        assert_eq!(receipt.source, function.body.id);
        assert_eq!(receipt.expression.as_ref(), Some(&duplicate));
        assert_eq!(receipt.detail(), "HIR expression identity is repeated");
        assert!(!receipt.physical());
        for limit in [0, 1] {
            let old = old::expression_facts_with_reservation(
                &function,
                &mut PhysicalReservation::new(limit),
            )
            .err()
            .unwrap();
            let current =
                expression_facts_with_reservation(&function, &mut PhysicalReservation::new(limit))
                    .err()
                    .unwrap();
            assert_eq!((current.code, current.message), (old.code, old.message));
        }
    }
    let mut deep = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "fact.read")
        .unwrap()
        .clone();
    let ResolvedExprKind::Block { tail, .. } = &mut deep.body.kind else {
        panic!("block")
    };
    let template = tail.as_ref().clone();
    let mut nested = template.clone();
    for index in 0..520 {
        let mut parent = template.clone();
        parent.id = ExpressionId::from_owned(format!("depth.{index}"));
        parent.kind = ResolvedExprKind::Unary {
            op: crate::ast::UnaryOp::Neg,
            value: Box::new(nested),
        };
        nested = parent;
    }
    **tail = nested;
    let old = old::expression_facts(&deep).err().unwrap();
    let current = expression_facts(&deep).err().unwrap();
    assert_eq!((current.code, current.message), (old.code, old.message));
    let typed = with_policy::<policies::Selected>(&deep, &mut PhysicalReservation::new(usize::MAX))
        .err()
        .unwrap();
    let policies::EngineError::Fact(receipt) = typed else {
        panic!("typed depth receipt")
    };
    assert_eq!(
        receipt.detail(),
        "HIR expression fact traversal exceeds the admitted depth"
    );
    assert_eq!(receipt.source, deep.body.id);
    assert!(receipt.expression.is_none());
    assert!(!receipt.physical());
}

pub(in crate::cleanup_plan::replay::expression_facts) fn same_owned(
    old: &BTreeMap<ExpressionId, Option<old::CallFact>>,
    current: &BTreeMap<ExpressionId, Option<CallFact<'_>>>,
) {
    assert_eq!(old.len(), current.len());
    for (key, old) in old {
        match (old, current.get(key).expect("same complete identity set")) {
            (None, None) => {}
            (Some(old), Some(current)) => {
                assert_eq!(&old.callee, current.callee.as_ref());
                assert_eq!(old.instance.as_ref(), current.instance);
                assert!(old.arguments.iter().eq(current.arguments.iter()));
                assert_eq!(
                    old.type_arguments.as_slice(),
                    current.type_arguments.as_slice()
                );
            }
            _ => panic!("saved call fact kind differs"),
        }
    }
}
