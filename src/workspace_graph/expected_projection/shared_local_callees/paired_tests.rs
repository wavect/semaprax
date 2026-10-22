//! Fresh pair handoffs, complete independent producers, and one-use lenders.
use super::*;
use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal};
use crate::test_allocation_observer::measure;
use sha2::{Digest, Sha256};
const SOURCE: &str = "module pair.local; @id(\"helper\") fn helper(x:i64)->i64{x} @id(\"main\") fn main()->i64{helper(1)+helper(2)}";
fn context<'a>(programs: &'a [Program], authored: &'a Index<'a>) -> Context<'a, 'a> {
    Context {
        program: &programs[0],
        authored,
        programs,
    }
}
fn components(cost: &SyntheticBuilderCosts) -> [usize; 6] {
    [
        cost.raw_clone_and_hir,
        cost.retained_clone_and_hir,
        cost.transient_import_clone,
        cost.runtime,
        cost.retained_hir,
        cost.synthetic_ast,
    ]
}

// Exact current→416 entry stages precede every unchanged older inverse. The
// packet authenticates whole files as well as each unique literal splice.
pub(in crate::workspace_graph::expected_projection) fn restore(
    path: &str,
    current: &str,
) -> String {
    let packet: serde_json::Value =
        serde_json::from_str(include_str!("paired_inverse.json")).unwrap();
    let entry = &packet["entries"][path];
    let hash = |raw: &str| {
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(raw.as_bytes()))
        )
    };
    assert_eq!(hash(current), entry["current_sha256"].as_str().unwrap());
    let mut restored = current.to_owned();
    for splice in entry["splices"].as_array().unwrap() {
        let after = splice["current"].as_str().unwrap();
        let before = splice["prior"].as_str().unwrap();
        assert_eq!(restored.matches(after).count(), 1);
        restored = restored.replacen(after, before, 1);
    }
    assert_eq!(hash(&restored), entry["prior_sha256"].as_str().unwrap());
    restored
}

#[test]
fn alias_free_pair_uses_one_finished_census_with_distinct_paid_handoffs() {
    let programs = [crate::parse(SOURCE, "pair.spx").unwrap()];
    let authored = crate::workspace_graph::index_authored(&programs).unwrap();
    let ctx = context(&programs, &authored);
    let ((first, second, fee), refused, paid, status) =
        with_limit_usage_refusal(usize::MAX, || {
            let mut census = Census::acquire(ctx, 6, true).unwrap();
            assert!(census.reusable && census.cached.is_none());
            let (first_proof, first) = census.read(ctx, true).unwrap();
            let held = census.cached.as_ref().unwrap();
            let core = held.1.bytes;
            let first_fee = census.invoice.bytes();
            assert_eq!(first.bytes, core + first_fee);
            let (second_proof, second) = census.read(ctx, false).unwrap();
            assert_eq!(
                (first_proof.roots, first_proof.bytes),
                (second_proof.roots, second_proof.bytes)
            );
            assert_eq!(census.cached.as_ref().unwrap().1.bytes, core);
            assert_eq!(second.bytes, core + census.invoice.bytes());
            assert!(
                second.bytes > first.bytes,
                "the second receiver is not free"
            );
            assert_eq!((first.slots, second.slots), (2, 2));
            (first.bytes, second.bytes, census.invoice.bytes())
        });
    assert!(!refused && status.is_none());
    assert_eq!(
        paid, second,
        "one core Invoice plus the cumulative handoff Invoice"
    );
    assert!(first < second && fee > 0);
    for imports in [true, false] {
        let ((_, independent), refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
            call_carriers::count_joint(ctx, imports).unwrap()
        });
        assert!(!refused);
        assert_eq!(independent.slots, 2);
        assert_eq!(independent.bytes + fee, second);
    }
}

#[test]
fn paired_complete_estimates_retain_independent_equations_and_real_shipping_peak() {
    let programs = [crate::parse(SOURCE, "pair.spx").unwrap()];
    let authored = crate::workspace_graph::index_authored(&programs).unwrap();
    let ctx = context(&programs, &authored);
    let maximum =
        super::super::super::dependency_identity_max(ctx.program, &authored, &programs).unwrap();
    let (full, retained) = estimates(
        ctx.program,
        &authored,
        &programs,
        Some(maximum),
        6,
        true,
        None,
    )
    .unwrap();
    let mut previous_fee = 0;
    for (imports, paired) in [(true, full), (false, retained)] {
        let old = held_projection::coupled_uncached_estimate(
            ctx.program,
            &authored,
            &programs,
            Some(maximum),
            6,
            imports,
            None,
        )
        .unwrap();
        let delta = paired.retained_hir.checked_sub(old.retained_hir).unwrap();
        assert!(delta > previous_fee);
        previous_fee = delta;
        let a = components(&paired);
        let b = components(&old);
        assert_eq!(a[0], b[0] + delta);
        assert_eq!(a[1], b[1] + delta);
        assert_eq!(a[2], b[2]);
        assert_eq!(a[3], b[3]);
        assert_eq!(a[5], b[5]);
        eprintln!(
            "paired-census imports={imports} immediate416={} paired={} handoff={delta}",
            old.retained_hir, paired.retained_hir
        );
    }
    let ((full, retained), refused, paid_pair, _) = with_limit_usage_refusal(usize::MAX, || {
        estimates(
            ctx.program,
            &authored,
            &programs,
            Some(maximum),
            6,
            true,
            None,
        )
        .unwrap()
    });
    let expected = full.retained_hir.max(retained.retained_hir);
    let (layout, refused_shipping, paid_shipping, _) = with_limit_usage_refusal(usize::MAX, || {
        super::super::super::uncached_peak::uncached_output_layout(&programs, &authored).unwrap()
    });
    assert!(!refused && !refused_shipping);
    assert_eq!(layout.forecast, expected);
    eprintln!("paired-real-shipping forecast={expected} paid_pair={paid_pair} paid_shipping={paid_shipping}");
}

#[test]
fn import_surface_keeps_two_independent_actual_censuses_and_profile_exclusions() {
    let source = SOURCE.replace(
        "module pair.local;",
        "module pair.local; use function @id(\"remote.f\") from provider as remote;",
    );
    let programs = [
        crate::parse(&source, "pair.spx").unwrap(),
        crate::parse(
            "module provider; @id(\"remote.f\") fn remote(x:i64)->i64{x}",
            "provider.spx",
        )
        .unwrap(),
    ];
    let authored = crate::workspace_graph::index_authored(&programs).unwrap();
    let ctx = context(&programs, &authored);
    with_limit_usage_refusal(usize::MAX, || {
        let mut census = Census::acquire(ctx, 6, true).unwrap();
        assert!(!census.reusable);
        for imports in [true, false] {
            let (proof, cost) = census.read(ctx, imports).unwrap();
            assert!(census.cached.is_none());
            let (independent, original) = call_carriers::count_joint(ctx, imports).unwrap();
            assert_eq!(
                (proof.roots, proof.bytes),
                (independent.roots, independent.bytes)
            );
            assert_eq!(cost.slots, original.slots);
            assert_eq!(cost.bytes, original.bytes + census.invoice.bytes());
        }
        assert!(census.read(ctx, false).is_err());
        for mode in [0, 1, 2, 3, 4, 5, 7, 255] {
            assert!(Census::acquire(ctx, mode, true).is_err());
        }
        assert!(Census::acquire(ctx, 6, false).is_err());
    });
}

#[test]
fn phase_replay_foreign_context_and_foreign_lender_never_publish_cached_data() {
    let programs = [crate::parse(SOURCE, "pair.spx").unwrap()];
    let authored = crate::workspace_graph::index_authored(&programs).unwrap();
    let foreign = crate::workspace_graph::index_authored(&programs).unwrap();
    let ctx = context(&programs, &authored);
    let wrong = Context {
        authored: &foreign,
        ..ctx
    };
    let ((), refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
        let mut census = Census::acquire(ctx, 6, true).unwrap();
        assert!(census.read(ctx, false).is_err());
        assert!(census.cached.is_none());
        assert!(census.read(wrong, true).is_err());
        assert!(census.cached.is_none());
        census.read(ctx, true).unwrap();
        assert!(census.read(ctx, true).is_err());
        let ((), inner_refused, _, inner_first) = with_limit_usage_refusal(usize::MAX, || {
            assert!(census.read(ctx, false).is_err());
        });
        assert!(!inner_refused && inner_first.is_none());
        assert!(census.read(ctx, false).is_err());
        assert!(!reserve_active_required(0));
    });
    assert!(refused && first.is_some());
}

#[test]
fn actual_pair_lender_exact_short_zero_and_complete_drop() {
    let programs = [crate::parse(SOURCE, "pair.spx").unwrap()];
    let authored = crate::workspace_graph::index_authored(&programs).unwrap();
    let ctx = context(&programs, &authored);
    let run = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let result = (|| {
                    let mut census = Census::acquire(ctx, 6, true).map_err(ReadError::Census)?;
                    census.read(ctx, true)?;
                    census.read(ctx, false)
                })();
                let success = result.is_ok();
                drop(result);
                if !success {
                    assert!(!reserve_active_required(0));
                }
                success
            })
        })
    };
    let ((success, allocation), refused, paid, first) = run(usize::MAX);
    assert!(success && !refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    for (limit, succeeds) in [(paid, true), (paid - 1, false), (0, false)] {
        let ((success, allocation), refused, used, first) = run(limit);
        assert_eq!(success, succeeds);
        if succeeds {
            assert!(!refused && first.is_none());
            assert_eq!(used, paid);
        } else {
            assert!(refused && first.is_some());
        }
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
    }
}

#[test]
fn whole416_entry_sources_and_complete_estimator_body_remain_reconstructible() {
    restore(
        "call_carriers_inverse.rs",
        include_str!("../call_carriers_inverse.rs"),
    );
    restore("held_projection.rs", include_str!("../held_projection.rs"));
    restore(
        "shared_local_callees.rs",
        include_str!("../shared_local_callees.rs"),
    );
    restore("uncached_peak.rs", include_str!("../uncached_peak.rs"));
    let prior = restore(
        "shared_local_callees/paired_projection.rs",
        include_str!("paired_projection.rs"),
    );
    let original_producer = restore(
        "shared_local_callees/coupled_projection.rs",
        include_str!("coupled_projection.rs"),
    );
    assert_eq!(prior, original_producer);
}
