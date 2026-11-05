//! Full old index/receiver controls. These are source-authored, not runtime evidence.
use super::*;
use crate::test_allocation_observer::{current_metrics, measure};

const SOURCE: &str = r#"module nominal.shared;
@id("row.type") record Row { @id("row.x") x:i64, }
@id("base.type") class Base {
    @id("base.x") x:i64,
    @id("base.read") fn read(self:Base)->i64 {self.x}
}
@id("child.type") class Child : Base {
    @id("child.y") y:i64,
    @id("child.read") fn read(self:Child)->i64 {self.y}
}
@id("choice.type") variant Choice {
    @id("choice.left") Left { @id("choice.left.x") x:i64, },
    @id("choice.right") Right,
}
@id("app.main") fn main()->i64 {0}
"#;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Stop(usize);

fn old(program: &Program) -> Result<(ResolvedProgram, FunctionResolutionWork), Diagnostic> {
    // Full frozen receiver with the independently saved complete index. No
    // shared constructor, payload oracle, schema substitute, or partial view.
    (Resolver {
        program,
        byte_helpers: crate::loop_calls::byte_writer::SourceIndex::new(program),
        declarations: DeclarationIndex::nominal_original_with_prelude(
            program,
            crate::prelude::declarations_for_program(program),
        )?,
        reuse: None,
        function_work: FunctionResolutionWork::default(),
        fused_producer: None,
        partition_profile: false,
    })
    .resolve_agent_identity_original()
}
#[test]
fn nominal_actual_paid_receiver_preserves_complete_old_hir_wire_and_shares_all_nominal_views() {
    let program = crate::check(SOURCE, "nominal-shared.spx").unwrap();
    let old = old(&program).unwrap();
    let mut requests = Vec::new();
    let current = resolve_checked_nominal(&program, None, None, None, None, |n| {
        requests.push(n);
        Ok::<_, Stop>(())
    })
    .unwrap();
    assert_eq!(current.0, old.0);
    assert_eq!(
        crate::cache_codec::encode(&current.0).unwrap(),
        crate::cache_codec::encode(&old.0).unwrap()
    );
    assert_eq!(current.1.reused(), old.1.reused());
    assert!(!requests.is_empty());
    for declaration in &current.0.types {
        let index = &current.0.declarations;
        let parameters = index.parameter_payload(&declaration.id).unwrap();
        assert_eq!(parameters.as_ptr(), declaration.type_parameters.as_ptr());
        match &declaration.kind {
            ResolvedTypeDeclarationKind::Record { fields }
            | ResolvedTypeDeclarationKind::Class { fields, .. } => {
                assert_eq!(
                    fields.as_ptr(),
                    index.record_payload(&declaration.id).unwrap().as_ptr()
                );
            }
            ResolvedTypeDeclarationKind::Variant { cases } => {
                assert_eq!(
                    cases.as_ptr(),
                    index.case_payload(&declaration.id).unwrap().as_ptr()
                );
                for case in cases {
                    assert_eq!(
                        case.fields.as_ptr(),
                        index.case_fields(&case.id).unwrap().as_ptr()
                    );
                }
            }
            ResolvedTypeDeclarationKind::Resource { .. } => {}
        }
    }
    // Public projection of a missing fact must remain missing, not be repaired
    // from the canonical outer cases merely because IDs happen to match.
    let mut hostile = current.0.declarations.clone();
    hostile.case_fields.remove("choice.left");
    assert!(hostile
        .case_fields(&DeclarationId::new("choice.left"))
        .is_none());
    let ((), drop) = measure(|| drop(current));
    assert!(!drop.invalid);
    assert_eq!(drop.allocated, 0);
}
#[test]
fn nominal_full_constructor_refuses_at_every_actual_acquisition_and_releases_all_partial_owners() {
    let nested = SOURCE
        .replace(
            "@id(\"row.x\") x:i64",
            "@id(\"row.x\") x:Option<Option<i64>>",
        )
        .replace(
            "@id(\"choice.left.x\") x:i64",
            "@id(\"choice.left.x\") x:Option<Option<i64>>",
        );
    // Preserve the original inadmitted fixture as its complete negative control.
    let rejected = crate::check(&nested, "nominal-shared.spx").unwrap_err();
    let expected = [
        ("SPX-T223", "generic copy type `Option` accepts only direct `i64` or `bool` arguments", 65, 87, 2, 43),
        ("SPX-T223", "generic copy type `Option` accepts only direct `i64` or `bool` arguments", 411, 433, 12, 52),
        ("SPX-T215", "case field `Choice::Left.x` must have a direct admitted Copy scalar or an in-scope variant type parameter in Copy Variants v1", 411, 433, 12, 52),
    ].map(|(code, message, start, end, line, column)| {
        Diagnostic::error(code, message, crate::ast::Span { start, end, line, column })
            .at_path("nominal-shared.spx")
    });
    assert_eq!(rejected, expected);
    // Direct scalar instantiation is admitted for a record field, and the
    // generic variant's field uses its in-scope parameter. Both retain a real
    // recursive nominal payload without bypassing source verification.
    let admitted = SOURCE
        .replace("@id(\"row.x\") x:i64", "@id(\"row.x\") x:Option<i64>")
        .replace("variant Choice {", "variant Choice<T> {")
        .replace("@id(\"choice.left.x\") x:i64", "@id(\"choice.left.x\") x:T");
    let program = crate::check(&admitted, "nominal-shared.spx").unwrap();
    let prelude = crate::prelude::declarations_for_program(&program);
    let mut requests = Vec::new();
    let full = DeclarationIndex::from_verified_with_nominal_reservation(&program, prelude, |n| {
        requests.push(n);
        Ok::<_, Stop>(())
    })
    .unwrap();
    let original = DeclarationIndex::nominal_original_with_prelude(&program, prelude).unwrap();
    assert_eq!(full, original);
    assert_eq!(
        crate::cache_codec::encode(&full).unwrap(),
        crate::cache_codec::encode(&original).unwrap()
    );
    for stop in 0..requests.len() {
        let mut seen = 0;
        let mut at_refusal = 0;
        let (result, allocation) = measure(|| {
            DeclarationIndex::from_verified_with_nominal_reservation(&program, prelude, |n| {
                assert_eq!(n, requests[seen]);
                let at = seen;
                seen += 1;
                if at == stop {
                    at_refusal = current_metrics().allocated;
                    Err(Stop(at))
                } else {
                    Ok(())
                }
            })
        });
        assert!(matches!(result,Err(IdentityIndexError::Identity(Stop(at))) if at==stop));
        assert_eq!(seen, stop + 1, "no callback retry after the selected error");
        assert!(!allocation.invalid);
        assert_eq!(allocation.live, 0);
        assert_eq!(allocation.allocated, allocation.freed);
        assert_eq!(
            allocation.allocated, at_refusal,
            "no allocation after selected refusal, including local/index Drop"
        );
    }
}
#[test]
fn nominal_full_constructor_keeps_complete_old_semantic_diagnostic_order() {
    for source in [
        SOURCE.replace("Child : Base", "Child : Missing"),
        SOURCE.replace("class Base {", "class Base : Child {"),
        SOURCE.replace("@id(\"child.read\")", "@id(\"base.read\")"),
    ] {
        let program = crate::parse(&source, "nominal-invalid.spx").unwrap();
        let prelude = crate::prelude::declarations_for_program(&program);
        let old = DeclarationIndex::nominal_original_with_prelude(&program, prelude).unwrap_err();
        let IdentityIndexError::Semantic(current) =
            DeclarationIndex::from_verified_with_nominal_reservation(&program, prelude, |_| {
                Ok::<_, Stop>(())
            })
            .unwrap_err()
        else {
            panic!("unexpected payer refusal")
        };
        assert_eq!(current, old);
    }
}

#[test]
fn nominal_full_constructor_exact_short_zero_and_preselected_owner_refusal() {
    use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal};
    let program = crate::check(SOURCE, "nominal-owner.spx").unwrap();
    let prelude = crate::prelude::declarations_for_program(&program);
    // This constructor retains ordinary ambient type-fact/layout formatting
    // debits in addition to the explicit nominal/identity callback requests.
    // Observe the complete same-owner route rather than treating that callback
    // subtotal as its whole ledger. No charge or quota is changed.
    let mut explicit = 0usize;
    let (full, refused, exact, first) = with_limit_usage_refusal(usize::MAX, || {
        DeclarationIndex::from_verified_with_nominal_reservation(&program, prelude, |n| {
            explicit = explicit.checked_add(n).unwrap();
            if reserve_active_required(n) {
                Ok(())
            } else {
                Err(Stop(7))
            }
        })
    });
    assert!(full.is_ok(), "full owner route: {full:?}; first={first:?}");
    assert!(!refused);
    assert_eq!(first, None);
    assert!(exact >= explicit);
    drop(full);
    for cap in [0, exact - 1, exact] {
        let ((result, metrics), overflowed, used, first) = with_limit_usage_refusal(cap, || {
            measure(|| {
                DeclarationIndex::from_verified_with_nominal_reservation(&program, prelude, |n| {
                    if reserve_active_required(n) {
                        Ok(())
                    } else {
                        Err(Stop(7))
                    }
                })
            })
        });
        assert!(!metrics.invalid);
        if cap == exact {
            assert!(result.is_ok(), "cap={cap} exact={exact} explicit={explicit} used={used} first={first:?} result={result:?}");
            assert!(!overflowed);
            assert_eq!(used, exact);
            assert!(
                metrics.allocated <= used,
                "actual backing={metrics:?}; paid={used}"
            );
            assert!(first.is_none());
        } else {
            assert!(matches!(result, Err(IdentityIndexError::Identity(Stop(7)))));
            assert!(overflowed);
            assert!(first.is_some());
            assert_eq!(metrics.live, 0);
            if cap == 0 {
                assert_eq!(metrics.allocated, 0);
            }
        }
        drop(result);
    }
    let ((result, metrics), overflowed, used, first) = with_limit_usage_refusal(0, || {
        assert!(!reserve_active_required(1));
        measure(|| {
            DeclarationIndex::from_verified_with_nominal_reservation(&program, prelude, |n| {
                if reserve_active_required(n) {
                    Ok(())
                } else {
                    Err(Stop(9))
                }
            })
        })
    });
    assert!(matches!(result, Err(IdentityIndexError::Identity(Stop(9)))));
    assert!(overflowed);
    assert_eq!(used, 0);
    assert_eq!(
        first.unwrap().requested,
        1,
        "first actual owner refusal survives retries"
    );
    assert!(!metrics.invalid);
    assert_eq!(metrics.allocated, 0);
}

mod legacy;
