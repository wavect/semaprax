//! Exact-source certificate hostiles and real proof allocation frontiers.
//! Parser-only malformed fixtures below test conservative forecast refusal;
//! none grants language, imported-source or HIR authority.
use super::super::*;
use crate::bounded_output::{
    active_remaining, reserve_active_required, set_active_floor, with_limit_usage_refusal,
};
use crate::test_allocation_observer::measure;

const SOURCE: &str = r#"module audit.local;
@id("audit.helper") fn helper(x:i64)->i64{x}
@id("audit.main") fn main()->i64{helper(1)}
"#;
fn parsed(s: &str) -> Program {
    crate::parse(s, "callee-audit.spx").unwrap()
}
fn index(programs: &[Program]) -> Index<'_> {
    let mut rows = Index::new();
    for p in programs {
        for f in &p.functions {
            rows.insert(
                f.stable_id.as_str(),
                AuthoredDeclaration {
                    path: &p.path,
                    module: &p.module,
                    explicit: f.explicit_id,
                    kind: crate::workspace_graph::AuthoredKind::Function,
                    function: Some(f),
                    ty: None,
                },
            );
        }
    }
    rows
}
fn context<'a>(p: &'a [Program], i: &'a Index<'a>) -> Context<'a, 'a> {
    Context {
        program: &p[0],
        authored: i,
        programs: p,
    }
}
fn counted(p: &[Program], i: &Index<'_>) -> usize {
    let (c, refused, paid, first) =
        with_limit_usage_refusal(usize::MAX, || costs(context(p, i)).unwrap());
    assert!(!refused && first.is_none());
    assert_eq!(c.bytes, paid);
    c.slots
}
#[test]
fn local_plain_call_certificate_excludes_shadow_prelude_generic_default_and_import_routes() {
    let p = [parsed(SOURCE)];
    let i = index(&p);
    assert_eq!(counted(&p, &i), 1);
    let p = [parsed(&SOURCE.replace(
        "fn main()->i64{helper(1)}",
        "fn main()->i64 requires helper(1)>0 {helper(1)}",
    ))];
    let i = index(&p);
    assert_eq!(
        counted(&p, &i),
        1,
        "contract callee slots retain their complete old bound"
    );
    for body in [
        "{let helper=1;helper(1)}",
        "{let outer={let helper=1;helper(1)};0}",
        "{helper<i64>(1)}",
        "{let hidden=match 1 {0=>helper(1),_=>0,};0}",
    ] {
        let p = [parsed(&SOURCE.replace("{helper(1)}", body))];
        let i = index(&p);
        assert_eq!(
            counted(&p, &i),
            0,
            "parser-only unsupported/shadowed body {body}"
        );
    }
    let p = [parsed(
        &SOURCE.replace("fn helper(x:i64)", "fn helper<T>(x:i64)"),
    )];
    let i = index(&p);
    assert_eq!(counted(&p, &i), 0);
    let p = [parsed(&SOURCE.replace("helper", "string_len"))];
    let i = index(&p);
    let (is_compiler, refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
        let invoice = Invoice::acquire().unwrap();
        compiler("string_len", &invoice).unwrap()
    });
    assert!(!refused && first.is_none());
    assert!(is_compiler);
    assert_eq!(counted(&p, &i), 0);
    // Generated/default prototypes are not explicitly authored local bodies.
    let mut p = [parsed(SOURCE)];
    p[0].functions[0].explicit_id = false;
    let i = index(&p);
    assert_eq!(counted(&p, &i), 0);
    let mut p = [parsed(SOURCE)];
    p[0].functions[1].explicit_id = false;
    let i = index(&p);
    assert_eq!(counted(&p, &i), 0);
    // Alias collision cannot be relabeled as the same-module function, even if
    // an output-only forecast later omits its independently priced import stub.
    let mut p = [parsed(SOURCE)];
    p[0].module_uses.push(crate::ast::ModuleUse {
        kind: crate::ast::ModuleUseKind::Function,
        persistent_id: "provider.helper".into(),
        target_module: "provider".into(),
        alias: "helper".into(),
        span: crate::ast::Span::default(),
    });
    let i = index(&p);
    assert_eq!(counted(&p, &i), 0);
}
#[test]
fn forged_authored_kind_path_module_origin_and_function_pointer_never_certify() {
    for attack in 0..5 {
        let p = [parsed(SOURCE)];
        let mut i = index(&p);
        let row = i.get_mut("audit.helper").unwrap();
        match attack {
            0 => row.kind = crate::workspace_graph::AuthoredKind::Type,
            1 => row.path = "foreign.spx",
            2 => row.module = "foreign.module",
            3 => row.explicit = false,
            _ => row.function = Some(&p[0].functions[1]),
        }
        assert_eq!(counted(&p, &i), 0, "forged authored row {attack}");
    }
    // Equal text is insufficient: the target Program must be the actual held
    // member of the exact provider slice, not a copied or stale source image.
    let p = [parsed(SOURCE)];
    let i = index(&p);
    let copy = p[0].clone();
    let (c, refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
        costs(Context {
            program: &copy,
            authored: &i,
            programs: &p,
        })
        .unwrap()
    });
    assert!(!refused);
    assert_eq!(c.slots, 0);
}
#[test]
fn certificate_consume_rejects_equal_copies_stale_context_and_wrong_lender() {
    let p = [
        parsed(SOURCE),
        parsed("module other;@id(\"other.main\") fn main()->i64{0}"),
    ];
    let i = index(&p);
    let other = index(&p);
    let copy = p[0].clone();
    let provider_copy = p.to_vec();
    let ((), refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
        for attack in 0..4 {
            let invoice = Invoice::acquire().unwrap();
            let c = context(&p, &i);
            let cert = Certificate::acquire(c, &invoice).unwrap();
            let bad = match attack {
                0 => Context {
                    program: &copy,
                    ..c
                },
                1 => Context {
                    authored: &other,
                    ..c
                },
                2 => Context {
                    programs: &provider_copy,
                    ..c
                },
                _ => Context {
                    programs: &p[..1],
                    ..c
                },
            };
            assert_eq!(cert.consume(bad, &invoice).err(), Some(Failure::Foreign));
        }
        let original = Invoice::acquire().unwrap();
        let alien = Invoice::acquire().unwrap();
        let cert = Certificate::acquire(context(&p, &i), &original).unwrap();
        let alien_bytes = alien.bytes();
        assert_eq!(
            cert.consume(context(&p, &i), &alien).err(),
            Some(Failure::Foreign)
        );
        assert_eq!(
            alien.bytes(),
            alien_bytes,
            "incoming lender is never charged"
        );
    });
    assert!(!refused);
}
#[test]
fn foreign_scope_never_charges_current_foreign_lender_and_old_owner_stays_refused() {
    let p = [parsed(SOURCE)];
    let i = index(&p);
    let ((), outer_refused, _, outer_first) = with_limit_usage_refusal(usize::MAX, || {
        let original = Invoice::acquire().unwrap();
        let cert = Certificate::acquire(context(&p, &i), &original).unwrap();
        let original_bytes = original.bytes();
        let ((), inner_refused, _, inner_first) = with_limit_usage_refusal(usize::MAX, || {
            let alien = Invoice::acquire().unwrap();
            let before = active_remaining();
            let bytes = alien.bytes();
            assert_eq!(
                cert.consume(context(&p, &i), &alien).err(),
                Some(Failure::Foreign)
            );
            assert_eq!(active_remaining(), before);
            assert_eq!(alien.bytes(), bytes);
        });
        assert!(!inner_refused && inner_first.is_none());
        assert_eq!(original.bytes(), original_bytes);
        assert_eq!(
            original.pay::<()>(),
            Err(Failure::Physical),
            "restoring scope cannot restore refused credit"
        );
    });
    assert!(outer_refused && outer_first.is_some());
}
#[test]
fn exact_short_zero_floor_and_first_sticky_refusal_release_every_proof_owner() {
    let p = [parsed(SOURCE)];
    let i = index(&p);
    let ((result, heap), refused, paid, first) =
        with_limit_usage_refusal(usize::MAX, || measure(|| costs(context(&p, &i))));
    let c = result.unwrap();
    assert_eq!(c.slots, 1);
    assert_eq!(c.bytes, paid);
    assert!(!refused && first.is_none());
    assert!(!heap.invalid && heap.live == 0);
    assert_eq!(heap.allocated, heap.freed);
    assert!(heap.allocated <= paid);
    for (limit, floor, success) in [
        (paid, 0, true),
        (paid - 1, 0, false),
        (0, 0, false),
        (paid, paid, false),
    ] {
        // A compact Failure remains inline inside the measurement. Legacy diagnostic
        // rendering belongs to the already-owned outer compatibility boundary.
        let ((result, heap), refused, spent, first) = with_limit_usage_refusal(limit, || {
            measure(|| {
                assert!(set_active_floor(floor));
                let invoice = Invoice::acquire()?;
                Certificate::acquire(context(&p, &i), &invoice)?.consume(context(&p, &i), &invoice)
            })
        });
        assert_eq!(
            result.is_ok() && !refused,
            success,
            "limit={limit} floor={floor}"
        );
        if success {
            assert_eq!(result.unwrap().slots, 1);
            assert_eq!(spent, paid);
        } else {
            assert_eq!(result.err(), Some(Failure::Physical));
            assert!(refused && first.is_some());
        }
        assert!(!heap.invalid && heap.live == 0);
        assert_eq!(heap.allocated, heap.freed);
    }
    let ((), refused, _, first) = with_limit_usage_refusal(paid, || {
        let invoice = Invoice::acquire().unwrap();
        let _ = Certificate::acquire(context(&p, &i), &invoice);
        // Select a real refusal first. A zero-size role still needs its renewed
        // gateway, and neither later requests nor floor restoration may recover.
        assert!(!reserve_active_required(usize::MAX));
        let before = active_remaining();
        assert!(set_active_floor(0));
        assert_eq!(invoice.pay::<()>(), Err(Failure::Physical));
        assert_eq!(active_remaining(), before);
    });
    assert!(refused && first.is_some());
}
#[test]
fn exhausted_work_keeps_spent_invoice_and_cannot_publish_partial_slots_or_new_credit() {
    let p = [parsed(SOURCE)];
    let i = index(&p);
    let ((), refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
        let invoice = Invoice::acquire().unwrap();
        invoice.work(WORK).unwrap();
        let before = invoice.bytes();
        assert_eq!(
            Certificate::acquire(context(&p, &i), &invoice).err(),
            Some(Failure::Work)
        );
        assert!(invoice.bytes() > before);
        let paid = invoice.bytes();
        assert_eq!(invoice.work(1), Err(Failure::Work));
        assert!(invoice.bytes() > paid);
    });
    assert!(
        !refused,
        "logical proof decline must retain charges without fabricating a physical refusal"
    );
    // A parser-independent hostile AST exhausts the bounded optional proof at
    // its first header. The input backing is prepared outside the measured
    // invocation, and no partial certificate or fresh work budget is returned.
    let mut bounded = [parsed(SOURCE)];
    bounded[0].functions[0].name = "n".repeat(WORK);
    let authored = index(&bounded);
    let (cost, refused, spent, first) =
        with_limit_usage_refusal(usize::MAX, || costs(context(&bounded, &authored)).unwrap());
    assert_eq!(cost.slots, 0);
    assert_eq!(cost.bytes, spent);
    assert!(spent > Invoice::setup_bytes());
    assert!(!refused && first.is_none());
}

#[test]
fn synthesized_vector_default_keeps_all_fresh_ids_and_carriers_separately_priced() {
    let source =
        format!("{SOURCE}\n@id(\"audit.vector\") fn vector(rows:borrow Vec<i64>)->i64{{0}}");
    let p = [parsed(&source)];
    let i = index(&p);
    let ty = Type::Named {
        name: "Vec".to_owned(),
        arguments: vec![Type::I64],
    };
    let default = || {
        let mut memo = [BTreeMap::new(), BTreeMap::new()];
        let mut visiting = std::collections::BTreeSet::new();
        super::super::super::defaults::default_expr_expanded_cost(
            &ty,
            &p[0].module,
            &p[0],
            &i,
            &p,
            &mut memo,
            &mut visiting,
            true,
        )
        .unwrap()
    };
    let before = default();
    assert_eq!(before.identity_slots, 9);
    assert_eq!(before.identity_carriers, 6);
    assert!(before.bytes >= 2 * size_of::<Expr>());
    assert_eq!(counted(&p, &i), 1);
    let after = default();
    assert_eq!(
        (
            before.bytes,
            before.string_bytes,
            before.identity_slots,
            before.identity_carriers
        ),
        (
            after.bytes,
            after.string_bytes,
            after.identity_slots,
            after.identity_carriers
        ),
        "a local-source certificate cannot claim the default's fresh constructor slots",
    );
}
