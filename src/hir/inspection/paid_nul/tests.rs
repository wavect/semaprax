//! Complete old/current audit verdicts and whole current backing through Drop.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;
const LIMIT: usize = 8 * 1024 * 1024;
fn nominal(id: &str, arguments: Vec<ResolvedType>) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(id),
        arguments,
    }
}
fn program() -> ResolvedProgram {
    crate::hir::resolve(&crate::check("module test.nul.paid; @id(\"helper\") fn helper(value:i64)->i64 { value } @id(\"app.main\") fn main()->i64 { helper(1) }", "nul-paid.spx").unwrap()).unwrap()
}
fn expression(width: usize) -> ResolvedExpr {
    let program = program();
    let mut root = program.functions[0].body.clone();
    let mut leaf = root.clone();
    leaf.kind = ResolvedExprKind::Int(0);
    root.kind = ResolvedExprKind::Invoke {
        callable: Box::new(leaf.clone()),
        args: vec![leaf; width],
    };
    root
}
fn shape(width: usize) -> crate::cleanup::FieldLivenessShape {
    crate::cleanup::FieldLivenessShape::Record {
        declaration: DeclarationId::new("record.λ"),
        fields: (0..width)
            .map(|i| crate::cleanup::FieldLiveness {
                field: DeclarationId::new(format!("field.{i}")),
                field_index: i as u32,
                shape: crate::cleanup::FieldLivenessShape::NoDrop,
            })
            .collect(),
    }
}
fn compare(old: Result<(), Diagnostic>, call: impl FnOnce() -> Result<(), Diagnostic>) {
    let (((), actual), refused, paid, first) = with_limit_usage_refusal(LIMIT, || {
        measure(|| {
            let current = call();
            assert_eq!(current, old);
            drop(current);
        })
    });
    assert!(!refused && first.is_none());
    assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
    assert!(actual.allocated <= paid && actual.highwater <= paid);
}
#[test]
fn every_closed_subject_keeps_full_original_diagnostic_without_acquisition() {
    let subjects: Vec<String> = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/paid-nul-audit-824/closed-subjects.json"
    )))
    .unwrap();
    for subject in subjects {
        let old =
            crate::hir::inspection::reject_nul_identity(&subject, "left\0λright").unwrap_err();
        let (((), actual), refused, paid, first) = with_limit_usage_refusal(0, || {
            measure(|| {
                assert!(text::message(&subject).is_some());
                let current = text::reject_nul_identity(&subject, "left\0λright").unwrap_err();
                assert_eq!(current, old);
                drop(current);
            })
        });
        assert!(!refused && paid == 0 && first.is_none());
        assert!(!actual.invalid && actual.allocated == 0 && actual.live == 0);
    }
}
#[test]
fn unknown_utf8_subject_fallback_prepays_full_retained_diagnostic_at_exact_short_and_zero() {
    let subject = "unknown caller λ🙂";
    let old = crate::hir::inspection::reject_nul_identity(subject, "bad\0identity").unwrap_err();
    let observe = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let result = text::reject_nul_identity(subject, "bad\0identity");
                let semantic = result.as_ref().err().is_some_and(|error| *error == old);
                drop(result);
                semantic
            })
        })
    };
    let ((semantic, actual), refused, paid, first) = observe(LIMIT);
    assert!(semantic && !refused && first.is_none() && paid > 0);
    assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
    assert!(actual.allocated <= paid && actual.highwater <= paid);
    let ((semantic, actual), refused, used, first) = observe(paid);
    assert!(semantic && !refused && used == paid && first.is_none());
    assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
    for limit in [0, paid - 1] {
        let ((semantic, actual), refused, used, first) = observe(limit);
        assert!(!semantic && refused && used <= limit && first.is_some());
        assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
        assert!(actual.allocated <= used);
    }
}
#[test]
fn complete_type_expression_and_shape_walks_preserve_old_priority_and_deep_spill() {
    for width in [0, 1, 31, 32, 33, 65, 1024] {
        let ty = nominal("type.λ", vec![ResolvedType::I64; width]);
        compare(crate::hir::inspection::audit_resolved_type(&ty), || {
            audit_resolved_type(&ty)
        });
        let expr = expression(width);
        compare(
            crate::hir::inspection::audit_resolved_expression(&expr),
            || audit_resolved_expression(&expr),
        );
        let shape = shape(width);
        compare(
            crate::hir::inspection::audit_field_liveness_shape(&shape),
            || audit_field_liveness_shape(&shape),
        );
    }
    let ty = ResolvedType::Function {
        parameters: vec![nominal("parameter\0bad", vec![])],
        result: Box::new(nominal("result\0bad", vec![])),
    };
    let old = crate::hir::inspection::audit_resolved_type(&ty);
    assert_eq!(
        old.as_ref().unwrap_err().message,
        "resolved nominal type identity contains NUL"
    );
    compare(old, || audit_resolved_type(&ty));
    let mut expr = expression(65);
    let owner = FunctionExecutionId::Monomorphic(program().functions[0].id.clone());
    if let ResolvedExprKind::Invoke { args, .. } = &mut expr.kind {
        args[0].id = ExpressionId::new(&owner, "first\0bad");
        args[64].kind = ResolvedExprKind::FunctionReference {
            target: DeclarationId::new("last\0bad"),
        };
    }
    let old = crate::hir::inspection::audit_resolved_expression(&expr);
    assert_eq!(
        old.as_ref().unwrap_err().message,
        "function reference identity contains NUL"
    );
    compare(old, || audit_resolved_expression(&expr));
    let mut deep = nominal("leaf\0bad", vec![]);
    for _ in 0..128 {
        deep = nominal("branch.λ", vec![deep]);
    }
    compare(crate::hir::inspection::audit_resolved_type(&deep), || {
        audit_resolved_type(&deep)
    });
}
#[test]
fn pending_spill_refill_preserves_lifo_and_sticky_original_refusal_through_drop() {
    let values: Vec<_> = (0..130).collect();
    let observe = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let result = (|| -> Result<(), Diagnostic> {
                    let mut pending = Pending::new(&values[0])?;
                    pending.extend(&values[1..])?;
                    for value in values.iter().rev() {
                        assert_eq!(pending.pop()?, Some(value));
                    }
                    assert_eq!(pending.pop()?, None);
                    let capacity = pending.capacity();
                    pending.extend(&values[..65])?;
                    assert_eq!(pending.capacity(), capacity);
                    for value in values[..65].iter().rev() {
                        assert_eq!(pending.pop()?, Some(value));
                    }
                    assert_eq!(pending.pop()?, None);
                    drop(pending);
                    Ok(())
                })();
                let ok = result.is_ok();
                drop(result);
                ok
            })
        })
    };
    let ((ok, actual), refused, paid, first) = observe(LIMIT);
    assert!(ok && !refused && first.is_none());
    assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
    assert!(actual.allocated <= paid && actual.highwater <= paid);
    let ((ok, actual), refused, used, first) = observe(paid);
    assert!(ok && !refused && used == paid && first.is_none());
    assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
    for limit in [0, paid - 1] {
        let ((ok, actual), refused, used, first) = observe(limit);
        assert!(!ok && refused && used <= limit && first.is_some());
        assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
        assert!(actual.allocated <= used);
    }
    let (((), actual), was_refused, used, first) = with_limit_usage_refusal(paid, || {
        measure(|| {
            let mut pending = Pending::new(&values[0]).unwrap();
            pending.extend(&values[1..]).unwrap();
            for _ in 0..values.len() {
                pending.pop().unwrap();
            }
            assert!(!crate::bounded_output::reserve_active_required(1));
            assert_eq!(pending.push(&values[0]).unwrap_err(), text::refused());
            assert_eq!(pending.pop().unwrap_err(), text::refused());
            drop(pending);
        })
    });
    assert!(was_refused && used == paid && first.unwrap().requested == 1);
    assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
    assert!(actual.allocated <= used);
}
#[test]
fn complete_program_and_attached_receivers_keep_original_first_errors_and_drop() {
    let base = program();
    for case in 0..4 {
        let mut program = base.clone();
        let owner = FunctionExecutionId::Monomorphic(program.functions[0].id.clone());
        match case {
            1 => program.entrypoint = DeclarationId::new("entry\0bad"),
            2 => program.functions[0].body.id = ExpressionId::new(&owner, "body\0bad"),
            3 => program.functions[0].result_id = ValueId::new("result\0bad".to_owned()),
            _ => {}
        }
        compare(
            crate::hir::inspection::validate_nul_free_identities_original(&program),
            || validate_nul_free_identities(&program),
        );
        compare(
            crate::hir::inspection::validate_attached_identity_references_original(&program),
            || validate_attached_identity_references(&program),
        );
        let old = crate::hir::validation::constructor_seed::original_authentication_error(&program);
        let (((), actual), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
            measure(|| {
                let current =
                    crate::hir::validation::constructor_seed::ConstructorSeed::authenticate(
                        &program, None,
                    )
                    .err();
                assert_eq!(current, old);
                drop(current);
            })
        });
        assert!(!refused && first.is_none());
        assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
    }
}
#[test]
fn borrowed_execution_text_has_exact_serialized_nul_verdict_without_key_acquisition() {
    for owner in [
        FunctionExecutionId::Monomorphic(DeclarationId::new("function.λ🙂")),
        FunctionExecutionId::Monomorphic(DeclarationId::new("function\0bad")),
        FunctionExecutionId::Generic(FunctionInstanceId::derive(
            &DeclarationId::new("generic.λ"),
            &[ResolvedType::I64],
        )),
    ] {
        for target in [
            ControlTarget::Function(owner.clone()),
            ControlTarget::Loop {
                owner: owner.clone(),
                site: ExpressionId::new(&owner, "loop\0bad"),
            },
        ] {
            let old = crate::hir::inspection::audit_control_target(&target);
            let (((), actual), refused, paid, first) = with_limit_usage_refusal(0, || {
                measure(|| {
                    let current = audit_control_target(&target);
                    assert_eq!(current, old);
                    drop(current);
                })
            });
            assert!(!refused && paid == 0 && first.is_none());
            assert!(!actual.invalid && actual.allocated == 0 && actual.live == 0);
        }
    }
}

#[test]
fn complete_nul_receiving_and_saved_constructor_sources_preserve_every_original_byte() {
    use sha2::{Digest, Sha256};
    let records: serde_json::Value = serde_json::from_str(ci_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/paid-nul-audit-824/exact-source-inverses.json"
    )))
    .unwrap();
    let paid_nul_before =
        reuse_source_before("src/hir/inspection/paid_nul.rs", ci_str!("../paid_nul.rs"));
    let currents = [
        ci_str!("../../inspection.rs"),
        ci_str!("../../validation/constructor_seed/original.rs"),
        ci_str!("../../validation/function_lookup/paid_custody.rs"),
        paid_nul_before.as_str(),
    ];
    let originals = [
        ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/paid-nul-audit-824/src__hir__inspection.rs.before.txt")),
        ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/paid-nul-audit-824/src__hir__validation__constructor_seed__original.rs.before.txt")),
        ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/paid-nul-audit-824/src__hir__validation__function_lookup__paid_custody.rs.before.txt")),
        ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/paid-nul-audit-824/complete-audit-body.before.txt")),
    ];
    let hashes = [
        "6af9ee1f630c86ce48a7e8f8020883f82780be36edfe2c14cbadb3ced3fc5870",
        "403d702555a21135b65fdf4195190c3526c8068eff28487874be2d1de34a68a1",
        "ae8571c20fef71437703acb6f867f202108f1e0d4695ee38f44fd1098d2d01e1",
        "beb6804024d90b2250954cad53e0105c24ccc5cc29e6e1066363adc33a3f1480",
    ];
    assert_eq!(records.as_array().unwrap().len(), currents.len());
    for (((record, current), original), hash) in records
        .as_array()
        .unwrap()
        .iter()
        .zip(currents)
        .zip(originals)
        .zip(hashes)
    {
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            hash
        );
        let mut current = current.to_owned();
        for edit in record["edits"].as_array().unwrap().iter().rev() {
            let after = edit["after"].as_str().unwrap();
            assert_eq!(current.matches(after).count(), 1);
            current = current.replacen(after, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(current, original);
    }
    let start = originals[0]
        .find("pub(super) fn validate_nul_free_identities(")
        .unwrap();
    let end = originals[0][start..]
        .find("\npub(super) fn path_is_prefix")
        .unwrap()
        + start;
    let control = &originals[0][originals[0].find("fn audit_control_target(").unwrap()..];
    assert_eq!(originals[3], originals[0][start..end].to_owned() + control);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(ci_str!("../audit_pending.rs").as_bytes()))
        ),
        "746c98ae21a2fc52029d943b65c237608ae6923bb80968ab282c16e2268ad770"
    );
}

// BEGIN INVOCATION EXPRESSION PENDING REUSE CONTROLS
fn reuse_source_before(path: &str, supplied: &str) -> String {
    use sha2::{Digest, Sha256};
    let hash = |bytes: &[u8]| format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let packet_bytes = include_bytes!("reuse_before/inverse.json");
    assert_eq!(
        std::fs::read(root.join("src/hir/inspection/paid_nul/reuse_before/inverse.json")).unwrap(),
        packet_bytes
    );
    assert_eq!(
        hash(packet_bytes),
        "34e8f79564b7d6ec98359f85eec368edb38b4384725014938ce9475b02acf2e2"
    );
    let packet: serde_json::Value = serde_json::from_slice(packet_bytes).unwrap();
    assert_eq!(packet.as_array().unwrap().len(), 2);
    let row = packet
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["path"] == path)
        .unwrap();
    let compiled = match path {
        "src/hir/inspection/paid_nul.rs" => include_str!("../paid_nul.rs"),
        "src/hir/inspection/paid_nul/pending.rs" => include_str!("pending.rs"),
        _ => panic!("foreign pending reuse owner"),
    };
    assert_eq!(supplied, compiled);
    assert_eq!(std::fs::read(root.join(path)).unwrap(), compiled.as_bytes());
    assert_eq!(
        compiled.len(),
        row["current_length"].as_u64().unwrap() as usize
    );
    assert_eq!(
        hash(compiled.as_bytes()),
        row["current_sha256"].as_str().unwrap()
    );
    let mut restored = compiled.to_owned();
    for edit in row["edits"].as_array().unwrap().iter().rev() {
        let start = edit["start"].as_u64().unwrap() as usize;
        let end = edit["end"].as_u64().unwrap() as usize;
        assert_eq!(&restored[start..end], edit["current"].as_str().unwrap());
        restored.replace_range(start..end, edit["before"].as_str().unwrap());
    }
    let before = match path {
        "src/hir/inspection/paid_nul.rs" => include_str!("reuse_before/paid_nul.rs.txt"),
        "src/hir/inspection/paid_nul/pending.rs" => include_str!("reuse_before/pending.rs.txt"),
        _ => unreachable!(),
    };
    let fixture = match path {
        "src/hir/inspection/paid_nul.rs" => "paid_nul.rs.txt",
        "src/hir/inspection/paid_nul/pending.rs" => "pending.rs.txt",
        _ => unreachable!(),
    };
    assert_eq!(
        std::fs::read(
            root.join("src/hir/inspection/paid_nul/reuse_before")
                .join(fixture)
        )
        .unwrap(),
        before.as_bytes()
    );
    assert_eq!(restored, before);
    assert_eq!(
        restored.len(),
        row["before_length"].as_u64().unwrap() as usize
    );
    assert_eq!(
        hash(restored.as_bytes()),
        row["before_sha256"].as_str().unwrap()
    );
    assert_eq!(std::fs::read(root.join(path)).unwrap(), compiled.as_bytes());
    assert_eq!(
        std::fs::read(root.join("src/hir/inspection/paid_nul/reuse_before/inverse.json")).unwrap(),
        packet_bytes
    );
    restored
}

fn reuse_program(widths: &[usize]) -> ResolvedProgram {
    let mut p = program();
    let template = p.functions[0].clone();
    p.functions = widths
        .iter()
        .map(|width| {
            let mut f = template.clone();
            f.requires.clear();
            f.ensures.clear();
            f.body = expression(*width);
            f
        })
        .collect();
    p
}
type ReuseObservation = (
    Result<(), Diagnostic>,
    crate::test_allocation_observer::Metrics,
    bool,
    usize,
    Option<crate::bounded_output::ReservationRefusal>,
);
fn reuse_observe(p: &ResolvedProgram, old: bool, limit: usize) -> ReuseObservation {
    let ((result, metrics), refused, paid, first) = with_limit_usage_refusal(limit, || {
        measure(|| {
            if old {
                reuse_before::validate_nul_free_identities(p)
            } else {
                validate_nul_free_identities(p)
            }
        })
    });
    (result, metrics, refused, paid, first)
}
#[test]
fn invocation_pending_reuse_keeps_complete_old_invoice_without_padding() {
    let mut p = reuse_program(&[0; 12]);
    p.functions[0].requires = vec![expression(0); 2];
    p.functions[0].ensures = vec![expression(0)];
    let roots: usize = p
        .functions
        .iter()
        .map(|f| f.requires.len() + 1 + f.ensures.len())
        .sum();
    assert_eq!(roots, 15);
    let (old, old_physical, old_refused, old_paid, old_first) = reuse_observe(&p, true, LIMIT);
    let (new, new_physical, new_refused, new_paid, new_first) = reuse_observe(&p, false, LIMIT);
    assert_eq!(new, old);
    assert!(
        new.is_ok() && !old_refused && !new_refused && old_first.is_none() && new_first.is_none()
    );
    let frame = std::mem::size_of::<Pending<'_, ResolvedExpr>>();
    assert_eq!(
        new_paid + roots * frame,
        old_paid + REUSE_SELECTION + frame + roots * REUSE_CALL
    );
    assert!(new_paid < old_paid);
    for (physical, paid) in [(old_physical, old_paid), (new_physical, new_paid)] {
        assert!(!physical.invalid && physical.live == 0 && physical.allocated == physical.freed);
        assert!(physical.allocated <= paid && physical.highwater <= paid);
    }
}
#[test]
fn invocation_pending_reuse_retains_spill_only_until_whole_audit_drop() {
    let p = reuse_program(&[130, 1, 65, 130, 1, 130]);
    let (old, a, ar, ap, af) = reuse_observe(&p, true, LIMIT);
    let (new, b, br, bp, bf) = reuse_observe(&p, false, LIMIT);
    assert_eq!(new, old);
    assert!(new.is_ok() && !ar && !br && af.is_none() && bf.is_none());
    assert!(b.allocated < a.allocated && bp < ap);
    for (physical, paid) in [(a, ap), (b, bp)] {
        assert!(!physical.invalid && physical.live == 0 && physical.allocated == physical.freed);
        assert!(physical.allocated <= paid && physical.highwater <= paid);
    }
}
#[test]
fn invocation_pending_reuse_preserves_all_diagnostic_fields_and_first_error_order() {
    for case in 0..4 {
        let mut p = reuse_program(&[65, 1, 130]);
        match case {
            0 => p.functions[0].result_id = ValueId::new("first-result\0bad".to_owned()),
            1 => p.functions[1].result_id = ValueId::new("second-result\0bad".to_owned()),
            2 => {
                let owner = FunctionExecutionId::Monomorphic(p.functions[0].id.clone());
                p.functions[1].body.id = ExpressionId::new(&owner, "second-body\0bad");
                p.functions[2].result_id = ValueId::new("last-result\0bad".to_owned());
            }
            _ => p.functions[2].result_id = ValueId::new("last-result\0bad".to_owned()),
        }
        let (old, a, ar, ap, af) = reuse_observe(&p, true, LIMIT);
        let (new, b, br, bp, bf) = reuse_observe(&p, false, LIMIT);
        assert!(old.is_err());
        assert_eq!(new, old);
        assert!(!ar && !br && af.is_none() && bf.is_none());
        if case == 0 {
            assert_eq!(
                bp,
                ap + REUSE_SELECTION,
                "first metadata failure must precede frame formation"
            );
        }
        for (physical, paid) in [(a, ap), (b, bp)] {
            assert!(
                !physical.invalid && physical.live == 0 && physical.allocated == physical.freed
            );
            assert!(physical.allocated <= paid && physical.highwater <= paid);
        }
    }
}
#[test]
fn invocation_pending_reuse_exact_short_zero_sticky_and_scope_isolation() {
    let mut empty = program();
    empty.functions.clear();
    let old_empty = reuse_observe(&empty, true, LIMIT);
    let new_empty = reuse_observe(&empty, false, LIMIT);
    assert_eq!(old_empty.0, new_empty.0);
    assert!(new_empty.0.is_ok() && !old_empty.2 && !new_empty.2);
    assert_eq!(new_empty.3, old_empty.3 + REUSE_SELECTION);
    assert_eq!(new_empty.1.allocated, 0);
    let empty_zero = reuse_observe(&empty, false, 0);
    assert_eq!(empty_zero.0.unwrap_err(), text::refused());
    assert!(empty_zero.2 && empty_zero.3 == 0);
    assert_eq!(empty_zero.4.unwrap().requested, REUSE_SELECTION);
    let p = reuse_program(&[65, 1, 130]);
    let (result, _, refused, exact, first) = reuse_observe(&p, false, LIMIT);
    assert!(result.is_ok() && !refused && first.is_none());
    for limit in [0, exact - 1, exact] {
        let (result, physical, refused, paid, first) = reuse_observe(&p, false, limit);
        assert_eq!(result.is_ok(), limit == exact);
        assert_eq!(refused, limit != exact);
        assert_eq!(first.is_none(), limit == exact);
        if limit != exact {
            assert_eq!(result.unwrap_err(), text::refused());
        }
        assert!(
            paid <= limit
                && !physical.invalid
                && physical.live == 0
                && physical.allocated == physical.freed
        );
    }
    let (((), physical), refused, paid, first) = with_limit_usage_refusal(exact, || {
        measure(|| {
            validate_nul_free_identities(&p).unwrap();
            let child = with_limit_usage_refusal(exact, || validate_nul_free_identities(&p));
            assert!(child.0.is_ok() && !child.1 && child.2 == exact && child.3.is_none());
            assert!(!crate::bounded_output::reserve_active_required(1));
            assert_eq!(
                validate_nul_free_identities(&p).unwrap_err(),
                text::refused()
            );
        })
    });
    assert!(refused && paid == exact && first.unwrap().requested == 1);
    assert!(!physical.invalid && physical.live == 0 && physical.allocated == physical.freed);
    assert_eq!(reuse_observe(&p, false, exact).3, exact);
}
#[test]
fn invocation_pending_reuse_authenticates_both_complete_immediate_old_sources() {
    use sha2::{Digest, Sha256};
    let audit = reuse_source_before(
        "src/hir/inspection/paid_nul.rs",
        include_str!("../paid_nul.rs"),
    );
    let pending = reuse_source_before(
        "src/hir/inspection/paid_nul/pending.rs",
        include_str!("pending.rs"),
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for (path, compiled) in [
        (
            "src/hir/inspection/paid_nul/reuse_before.rs",
            include_str!("reuse_before.rs"),
        ),
        (
            "src/hir/inspection/paid_nul/reuse_before/pending.rs",
            include_str!("reuse_before/pending.rs"),
        ),
        (
            "src/hir/inspection/paid_nul/text.rs",
            include_str!("text.rs"),
        ),
        (
            "src/hir/inspection/paid_nul/reuse_before/tests.rs.txt",
            include_str!("reuse_before/tests.rs.txt"),
        ),
    ] {
        assert_eq!(std::fs::read(root.join(path)).unwrap(), compiled.as_bytes());
    }
    let before = include_str!("reuse_before.rs")
        .replace(
            "#[path = \"reuse_before/pending.rs\"]\nmod pending;",
            "mod pending;",
        )
        .replace("#[path = \"text.rs\"]\nmod text;", "mod text;");
    assert_eq!(before + "\n#[cfg(test)]\nmod tests;\n", audit);
    assert_eq!(include_str!("reuse_before/pending.rs"), pending);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(include_bytes!("text.rs")))
        ),
        "b3c25a540ff9ec25804e6847016eb637ff9b56f8147d180d748e5f9dfeb47beb"
    );
    // Entire existing harness is retained, including its old #824 inverse.
    let harness = include_str!("tests.rs");
    let (prefix, _) = harness
        .split_once("// BEGIN INVOCATION EXPRESSION PENDING REUSE CONTROLS")
        .unwrap();
    let prefix = prefix.replace(
        "    let paid_nul_before =\n        reuse_source_before(\"src/hir/inspection/paid_nul.rs\", ci_str!(\"../paid_nul.rs\"));\n",
        "",
    ).replace("        paid_nul_before.as_str(),", "        ci_str!(\"../paid_nul.rs\"),");
    assert_eq!(
        prefix.strip_suffix('\n').unwrap(),
        include_str!("reuse_before/tests.rs.txt")
    );
    let current = include_str!("../paid_nul.rs");
    let mutated = current.replacen("const REUSE_CALL", "const FOREIGN_CALL", 1);
    assert!(std::panic::catch_unwind(|| reuse_source_before(
        "src/hir/inspection/paid_nul.rs",
        &mutated
    ))
    .is_err());
}
