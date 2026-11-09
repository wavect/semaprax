//! Real imported-module shipping forecasts against BOTH complete old equations.
use super::super::super::{
    before_shared_local_projection as pre916, cached_exact, dependency_identity_max, uncached_peak,
    HIR_IDENTITY_COPY_FACTOR,
};
use super::super::modular_session::tests::{context, programs};
use super::*;
use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal};
use crate::test_allocation_observer::measure;

#[test]
fn real_imports_force_two_independent_censuses_and_never_reuse_full_as_retained() {
    let p = programs();
    let i = crate::workspace_graph::index_authored(&p).unwrap();
    let ctx = context(&p, &i);
    with_limit_usage_refusal(usize::MAX, || {
        let mut census = Census::acquire(ctx, 6, true).unwrap();
        assert!(!census.reusable && census.cached.is_none());
        let full = census.read(ctx, true).unwrap();
        let retained = census.read(ctx, false).unwrap();
        assert!(census.cached.is_none());
        assert!(
            full.0.roots > retained.0.roots,
            "real imported call is present only in full auxiliary census"
        );
        assert_eq!((full.1.slots, retained.1.slots), (2, 2));
        for (imports, actual) in [(true, full), (false, retained)] {
            let independent = call_carriers::count_modular(ctx, imports).unwrap();
            assert_eq!(
                (actual.0.roots, actual.0.bytes),
                (independent.0.roots, independent.0.bytes)
            );
            assert_eq!(actual.1.slots, independent.1.slots);
            assert!(
                actual.1.bytes > independent.1.bytes,
                "each handoff has its own invoice"
            );
        }
    });
}

#[test]
fn full_old_pre916_and_original916_equations_keep_all_remaining_identity_terms() {
    let p = programs();
    let i = crate::workspace_graph::index_authored(&p).unwrap();
    let ctx = context(&p, &i);
    let maximum = dependency_identity_max(ctx.program, &i, &p).unwrap();
    let unit = maximum * HIR_IDENTITY_COPY_FACTOR;
    assert_eq!(HIR_IDENTITY_COPY_FACTOR, 16);
    let original_proof = super::super::costs(ctx).unwrap();
    let mut census = Census::acquire(ctx, 6, true).unwrap();
    for imports in [true, false] {
        let old = pre916::synthetic_builder_bytes_scoped_with_retention_with_registry(
            ctx.program,
            &i,
            &p,
            Some(maximum),
            imports,
            (6, cached_exact::Retention::Parsed),
            None,
        )
        .unwrap();
        let original916 =
            held_projection::synthetic_builder_bytes_scoped_with_retention_with_registry(
                ctx.program,
                &i,
                &p,
                Some(maximum),
                imports,
                (6, cached_exact::Retention::Parsed),
                None,
            )
            .unwrap();
        let raw = call_carriers::count(ctx.program, &i, imports);
        let (proof, local) = census.read(ctx, imports).unwrap();
        let current = held_projection::modular_projection::estimate_with_census(
            ctx.program,
            &i,
            &p,
            Some(maximum),
            6,
            imports,
            None,
            &mut Census::acquire(ctx, 6, true).unwrap(),
        );
        // The retained-only phase must follow a full phase in its own fresh
        // lender. Use the real pair API for the complete current estimate.
        if imports {
            assert!(current.is_ok());
        } else {
            assert!(current.is_err());
        }
        let pair = estimates(ctx.program, &i, &p, Some(maximum), 6, true, None).unwrap();
        let current = if imports { pair.0 } else { pair.1 };
        assert_eq!(local.slots, 2);
        assert_eq!(
            original916.retained_hir + original_proof.slots * unit,
            old.retained_hir + original_proof.bytes
        );
        assert_eq!(
            current.retained_hir + local.slots * unit + raw.bytes,
            old.retained_hir + proof.bytes + local.bytes
        );
        assert_eq!(
            current.retained_clone_and_hir + local.slots * unit + raw.bytes,
            old.retained_clone_and_hir + proof.bytes + local.bytes
        );
        assert_eq!(current.synthetic_ast, old.synthetic_ast);
        assert_eq!(current.runtime, old.runtime);
        assert_eq!(current.transient_import_clone, old.transient_import_clone);
        eprintln!("modular-proof imports={imports} pre916={} original916={} current={} slot={} proof={} originalfee={}",old.retained_hir,original916.retained_hir,current.retained_hir,local.slots,local.bytes,original_proof.bytes);
    }
}

#[test]
fn actual_two_module_uncached_shipping_peak_and_full_drop_are_observed() {
    let p = programs();
    let i = crate::workspace_graph::index_authored(&p).unwrap();
    let mut entries = Vec::new();
    for (index, program) in p.iter().enumerate() {
        let maximum = Some(dependency_identity_max(program, &i, &p).unwrap());
        let (full, retained) = if program.module_uses.is_empty() {
            super::super::paired::estimates(program, &i, &p, maximum, 6, true, None)
        } else {
            estimates(program, &i, &p, maximum, 6, true, None)
        }
        .unwrap();
        entries.push((index, full.retained_hir, retained.retained_hir));
    }
    entries.sort_unstable_by(|left, right| {
        right
            .1
            .saturating_sub(right.2)
            .cmp(&left.1.saturating_sub(left.2))
            .then_with(|| p[left.0].path.cmp(&p[right.0].path))
    });
    let mut retained = 0;
    let mut peak = 0;
    for &(_, full, held) in &entries {
        peak = peak.max(retained + full);
        retained += held;
    }
    let ((layout, allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| uncached_peak::uncached_output_layout(&p, &i).unwrap())
    });
    assert!(!refused && first.is_none());
    assert_eq!(layout.forecast, peak.max(retained));
    assert_eq!(layout.order[..2], [entries[0].0, entries[1].0]);
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    eprintln!(
        "modular-two-module-shipping forecast={} actualpaid={paid} allocated={} highwater={}",
        layout.forecast, allocation.allocated, allocation.highwater
    );
}

#[test]
fn selector_and_real_modular_pair_keep_exact_short_zero_sticky_and_full_drop() {
    let p = programs();
    let i = crate::workspace_graph::index_authored(&p).unwrap();
    let ctx = context(&p, &i);
    let run = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                // The physical selector/census frontier is distinct from the
                // estimator's logical final-forecast limit (covered above).
                let result = (|| {
                    super::super::super::reserve_builder_structure(SELECT)?;
                    let mut census = Census::acquire(ctx, 6, true)?;
                    let full = read_or_count(Some(&mut census), ctx, true)?;
                    let retained = read_or_count(Some(&mut census), ctx, false)?;
                    Ok::<_, Vec<Diagnostic>>((full, retained))
                })();
                let ok = result.is_ok();
                drop(result);
                if !ok {
                    assert!(!reserve_active_required(0));
                }
                ok
            })
        })
    };
    let ((ok, allocation), refused, paid, first) = run(usize::MAX);
    assert!(ok && !refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    for (limit, expected) in [(paid, true), (paid - 1, false), (0, false)] {
        let ((ok, allocation), refused, _, first) = run(limit);
        assert_eq!(ok, expected);
        assert_eq!(refused, !expected);
        assert_eq!(first.is_some(), !expected);
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
    }
}

#[test]
fn alias_free_dispatch_keeps_the_complete_original_pair_and_cache_predicate() {
    let p = [crate::parse(
        "module scalar.local; @id(\"m\") fn main()->i64{1}",
        "scalar.spx",
    )
    .unwrap()];
    let i = crate::workspace_graph::index_authored(&p).unwrap();
    let maximum = Some(dependency_identity_max(&p[0], &i, &p).unwrap());
    let original = super::super::paired::estimates(&p[0], &i, &p, maximum, 6, true, None).unwrap();
    let selected = selected_estimates(&p[0], &i, &p, maximum, 6, true, None).unwrap();
    assert_eq!(
        (original.0.retained_hir, original.1.retained_hir),
        (selected.0.retained_hir, selected.1.retained_hir)
    );
    let source = include_str!("modular_pair.rs");
    assert!(source.contains(
        "context.program.module_uses.is_empty() && context.program.interfaces.is_empty()"
    ));
    let old = include_str!("paired.rs");
    assert!(old.contains(
        "context.program.module_uses.is_empty() && context.program.interfaces.is_empty()"
    ));
}
