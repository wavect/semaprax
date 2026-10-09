use super::*;

fn owners() -> [FunctionExecutionId; 4] {
    [
        FunctionExecutionId::Monomorphic(DeclarationId::new("owner:é.λ")),
        FunctionExecutionId::Monomorphic(DeclarationId::new("owner:é.λ:extra")),
        FunctionExecutionId::Generic(FunctionInstanceId("owner:é.λ".to_owned())),
        FunctionExecutionId::Generic(FunctionInstanceId("owner:é.λ:extra".to_owned())),
    ]
}

#[test]
fn streaming_identity_comparison_agrees_with_allocating_encoder_and_rejects_forgery() {
    let deep = "body.s123.value.arm.45.binding.67.".repeat(128);
    for owner in owners() {
        for path in ["", "body", "body.tail", "body:é.λ", deep.as_str()] {
            for kind in ["expression", "value:local", "value:result", "value:param"] {
                let encoded = scoped_identity(&owner, kind, path);
                let matches =
                    |actual: &str| matches_scoped_identity(actual, &owner, kind, path.len(), path);
                assert!(matches(&encoded));
                assert!(!matches(&std::format!("{encoded}:")));
                assert!(!matches(&std::format!(":{encoded}")));
                // Every valid UTF-8 boundary is a distinct truncation; every
                // character replacement attacks length, separator, kind,
                // owner, or path bytes using the independent encoder's output.
                for (offset, character) in encoded.char_indices() {
                    assert!(!matches(&encoded[..offset]));
                    let mut forged = encoded.clone();
                    forged.replace_range(offset..offset + character.len_utf8(), "!");
                    assert!(!matches(&forged), "accepted modified byte at {offset}");
                }
                for other in owners() {
                    if owner != other {
                        assert!(!matches(&scoped_identity(&other, kind, path)));
                    }
                }
            }
        }
    }
}

#[test]
fn retained_identity_checks_need_no_temporary_string_budget() {
    for owner in owners() {
        let path = "body.s9.value.arm.1.binding.0";
        let expression = ExpressionId::new(&owner, path);
        let local = ValueId::local(&owner, path);
        let result = ValueId::result(&owner);
        let mut corrupt_fingerprint = local.clone();
        corrupt_fingerprint.1 ^= 1;
        assert!(!corrupt_fingerprint.matches_local(&owner, path));
        for index in [0, 9, 10, 99, 100, usize::MAX] {
            let parameter = ValueId::parameter(&owner, index);
            let (matched, overflowed, used) = crate::bounded_output::with_limit_usage(0, || {
                expression.matches(&owner, path)
                    && local.matches_local(&owner, path)
                    && result.matches_result(&owner)
                    && parameter.matches_parameter(&owner, index)
                    && !expression.matches(&owner, "body.s9.value.arm.1.binding.00")
                    && !local.matches_result(&owner)
                    && !parameter.matches_parameter(&owner, index.wrapping_add(1))
            });
            assert!(matched);
            assert!(!overflowed);
            assert_eq!(used, 0);
        }
        let (_, overflowed, materialized) =
            crate::bounded_output::with_limit_usage(usize::MAX, || ExpressionId::new(&owner, path));
        assert!(!overflowed);
        assert!(materialized >= expression.as_str().len());
        let (_, overflowed, _) =
            crate::bounded_output::with_limit_usage(0, || ExpressionId::new(&owner, path));
        assert!(overflowed, "owned construction remains budgeted");
    }
}

#[test]
fn identity_validation_preserves_frozen_graph_bytes_and_rejects_forged_hir() {
    let ast = crate::parse(include_str!("../../../examples/meaning.spx"), "meaning.spx").unwrap();
    let expected = include_str!("../../../tests/snapshots/meaning.graph.json").trim_end();
    assert_eq!(crate::graph::to_json(&ast).unwrap(), expected);
    let resolved = crate::hir::resolve(&ast).unwrap();
    let revision = crate::graph::revision(&ast);
    assert_eq!(
        crate::graph::to_hir_json(&resolved, &revision).unwrap(),
        expected
    );
    for role in ["expression", "parameter", "result"] {
        let mut forged = resolved.clone();
        let function = forged
            .functions
            .iter_mut()
            .find(|function| !function.params.is_empty())
            .unwrap();
        match role {
            "expression" => {
                function.body.id = ExpressionId::from_owned(std::format!("{}:", function.body.id));
            }
            "parameter" => {
                function.params[0].id = ValueId::new(std::format!("{}:", function.params[0].id))
            }
            "result" => function.result_id = ValueId::new(std::format!("{}:", function.result_id)),
            _ => unreachable!(),
        }
        let error = crate::hir::validate(&forged).unwrap_err();
        assert_eq!(error.code, "SPX-H006");
        assert!(
            error.message.contains("non-canonical identity")
                || error.message.contains("non-canonical result identity")
        );
        assert_eq!(
            crate::graph::to_hir_json(&forged, &revision)
                .unwrap_err()
                .code,
            "SPX-H006"
        );
    }
}
