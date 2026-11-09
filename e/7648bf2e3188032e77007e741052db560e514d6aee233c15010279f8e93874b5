//! Actual uncached route, both whole predecessors, and independent source scopes.
use super::super::super::{
    before_shared_local_projection as before, cached_exact, dependency_identity_max,
    held_projection, shared_local_callees, uncached_peak, HIR_IDENTITY_COPY_FACTOR,
};
use super::*;
use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal};
use crate::test_allocation_observer::measure;

fn source(headers: usize, calls: usize) -> String {
    let mut source =
        String::from("module loan.local; @id(\"loan.helper\") fn helper(x:i64)->i64{x}\n");
    for i in 1..headers - 1 {
        source.push_str(&format!(
            "@id(\"loan.unused.{i}\") fn unused_{i}()->i64{{0}}\n"
        ));
    }
    source.push_str("@id(\"loan.main\") fn main()->i64{");
    source.push_str(
        &std::iter::repeat_n("helper(1)", calls)
            .collect::<Vec<_>>()
            .join("+"),
    );
    source.push('}');
    source
}
fn context<'a>(programs: &'a [Program], authored: &'a Index<'a>) -> Context<'a, 'a> {
    Context {
        program: &programs[0],
        authored,
        programs,
    }
}

#[test]
fn linear_and_indexed_joint_forecasts_keep_both_complete_predecessor_equations() {
    for headers in [2, 64, 129] {
        for calls in [1, 64] {
            let programs = vec![crate::parse(&source(headers, calls), "loan.spx").unwrap()];
            let authored = crate::workspace_graph::index_authored(&programs).unwrap();
            let ctx = context(&programs, &authored);
            let maximum = dependency_identity_max(ctx.program, &authored, &programs).unwrap();
            let old_slots = shared_local_callees::costs(ctx).unwrap();
            for imports in [false, true] {
                let raw = super::super::count(ctx.program, &authored, imports);
                let (auxiliary, local) = count(ctx, imports).unwrap();
                assert_eq!(
                    auxiliary.roots, raw.roots,
                    "auxiliary proof remains separate"
                );
                assert_eq!(local.slots, calls);
                assert!(local.slots <= old_slots.slots);
                let baseline = before::synthetic_builder_bytes_scoped_with_retention_with_registry(
                    ctx.program,
                    &authored,
                    &programs,
                    Some(maximum),
                    imports,
                    (6, cached_exact::Retention::Parsed),
                    None,
                )
                .unwrap();
                let old =
                    held_projection::synthetic_builder_bytes_scoped_with_retention_with_registry(
                        ctx.program,
                        &authored,
                        &programs,
                        Some(maximum),
                        imports,
                        (6, cached_exact::Retention::Parsed),
                        None,
                    )
                    .unwrap();
                let now = held_projection::coupled_uncached_estimate(
                    ctx.program,
                    &authored,
                    &programs,
                    Some(maximum),
                    6,
                    imports,
                    None,
                )
                .unwrap();
                let unit = maximum * HIR_IDENTITY_COPY_FACTOR;
                assert_eq!(HIR_IDENTITY_COPY_FACTOR, 16);
                assert_eq!(
                    old.retained_hir + old_slots.slots * unit,
                    baseline.retained_hir + old_slots.bytes
                );
                assert_eq!(
                    now.retained_hir + local.slots * unit + raw.bytes,
                    baseline.retained_hir + auxiliary.bytes + local.bytes
                );
                assert_eq!(
                    now.retained_clone_and_hir + local.slots * unit + raw.bytes,
                    baseline.retained_clone_and_hir + auxiliary.bytes + local.bytes
                );
                // This records the actual result, including a negative gain.
                // Removing the old optional-proof fee is not a baseline win.
                eprintln!("local-reuse headers={headers} calls={calls} imports={imports} baseline={} old={} current={} certificate={} slots={} traversal_old={} traversal_new={}",
                    baseline.retained_hir, old.retained_hir, now.retained_hir,
                    local.bytes, local.slots, raw.bytes, auxiliary.bytes);
            }
        }
    }
}

#[test]
fn actual_uncached_tiny_repeated_shipping_path_and_complete_proof_drop_are_observed() {
    for calls in [1, 64] {
        let programs = vec![crate::parse(&source(2, calls), "loan.spx").unwrap()];
        let authored = crate::workspace_graph::index_authored(&programs).unwrap();
        let ctx = context(&programs, &authored);
        let (((aux, local), allocation), refused, charged, first) =
            with_limit_usage_refusal(usize::MAX, || measure(|| count(ctx, true).unwrap()));
        assert!(!refused && first.is_none() && !allocation.invalid);
        assert_eq!(charged, local.bytes);
        assert_eq!(local.slots, calls);
        assert_eq!(allocation.live, 0);
        assert_eq!(allocation.allocated, allocation.freed);
        let ((layout, allocation), refused, charged, first) =
            with_limit_usage_refusal(usize::MAX, || {
                measure(|| uncached_peak::uncached_output_layout(&programs, &authored).unwrap())
            });
        assert!(!refused && first.is_none() && !allocation.invalid);
        assert_eq!(layout.order[0], 0);
        let maximum = dependency_identity_max(ctx.program, &authored, &programs).unwrap();
        let estimate = |imports, predecessor| match predecessor {
            0 => {
                before::synthetic_builder_bytes_scoped_with_retention_with_registry(
                    ctx.program,
                    &authored,
                    &programs,
                    Some(maximum),
                    imports,
                    (6, cached_exact::Retention::Parsed),
                    None,
                )
                .unwrap()
                .retained_hir
            }
            1 => {
                held_projection::synthetic_builder_bytes_scoped_with_retention_with_registry(
                    ctx.program,
                    &authored,
                    &programs,
                    Some(maximum),
                    imports,
                    (6, cached_exact::Retention::Parsed),
                    None,
                )
                .unwrap()
                .retained_hir
            }
            _ => {
                held_projection::coupled_uncached_estimate(
                    ctx.program,
                    &authored,
                    &programs,
                    Some(maximum),
                    6,
                    imports,
                    None,
                )
                .unwrap()
                .retained_hir
            }
        };
        // One module has no preceding retention: the real shipping peak is
        // exactly max(full, retained), rather than merely a positive number.
        let original = estimate(true, 0).max(estimate(false, 0));
        let immediate = estimate(true, 1).max(estimate(false, 1));
        let expected = estimate(true, 2).max(estimate(false, 2));
        assert_eq!(layout.forecast, expected);
        for limit in [expected, expected - 1, 0] {
            let ((result, allocation), refused, _, first) = with_limit_usage_refusal(limit, || {
                measure(|| {
                    let result = uncached_peak::uncached_output_layout(&programs, &authored);
                    let success = result.is_ok();
                    drop(result);
                    success
                })
            });
            assert_eq!(result, limit == expected, "real shipping frontier {limit}");
            assert!(!allocation.invalid && allocation.live == 0);
            assert_eq!(allocation.allocated, allocation.freed);
            if limit == 0 {
                assert!(
                    refused && first.is_some(),
                    "physical zero refusal remains sticky"
                );
            }
        }
        eprintln!("actual-one-module-shipping calls={calls} pre916={original} immediate2a={immediate} candidate={expected}");
        assert_eq!(allocation.live, 0);
        assert_eq!(allocation.allocated, allocation.freed);
        eprintln!("uncached-local-reuse calls={calls} forecast={} actual_paid={charged} allocated={} highwater={} proof={} traversal={}",
            layout.forecast, allocation.allocated, allocation.highwater, local.bytes, aux.bytes);
    }
}

#[test]
fn real_joint_lender_exact_short_zero_and_sticky_drop() {
    let programs = vec![crate::parse(&source(2, 64), "loan.spx").unwrap()];
    let authored = crate::workspace_graph::index_authored(&programs).unwrap();
    let ctx = context(&programs, &authored);
    let run = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let result = count(ctx, true);
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
    let ((success, allocation), refused, exact, first) = run(paid);
    assert!(success && !refused && first.is_none());
    assert_eq!(exact, paid);
    assert!(!allocation.invalid && allocation.live == 0);
    for limit in [0, paid - 1] {
        let ((success, allocation), refused, _, first) = run(limit);
        assert!(!success && refused && first.is_some());
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
    }
}

#[test]
fn unknown_shadow_generic_contract_and_default_routes_never_inflate_local_slots() {
    let source = source(2, 1);
    for source in [
        source.replace("{helper(1)}", "{let helper=1;helper(1)}"),
        source.replace("{helper(1)}", "{let x=match 1{0=>helper(1),_=>0,};0}"),
        source.replace("fn helper(x:i64)", "fn helper<T>(x:i64)"),
        source.replace("fn main()->i64", "fn main()->i64 requires helper(1)>0"),
        source.replace("@id(\"loan.helper\")", ""),
    ] {
        let programs = vec![crate::parse(&source, "hostile-loan.spx").unwrap()];
        // Malformed parser-only cases authenticate their exact raw source;
        // they never assert language or provider admission.
        let mut authored = Index::new();
        for program in &programs {
            for function in &program.functions {
                authored.insert(
                    function.stable_id.as_str(),
                    AuthoredDeclaration {
                        path: &program.path,
                        module: &program.module,
                        explicit: function.explicit_id,
                        kind: crate::workspace_graph::AuthoredKind::Function,
                        function: Some(function),
                        ty: None,
                    },
                );
            }
        }
        let ctx = context(&programs, &authored);
        let old = shared_local_callees::costs(ctx).unwrap();
        let (aux, current) = count(ctx, false).unwrap();
        assert!(current.slots <= old.slots);
        assert_eq!(
            aux.roots,
            super::super::count(ctx.program, &authored, false).roots
        );
    }
}

#[test]
fn all_current_source_entries_reconstruct_the_complete_immediate_predecessors() {
    use sha2::{Digest, Sha256};
    let hash = |raw: &str| {
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(raw.as_bytes()))
        )
    };
    let current = include_str!("../call_carriers.rs");
    let wiring = r#####"
#[path = "call_carriers_joint.rs"]
mod joint;
pub(super) use joint::count as count_joint;
"#####;
    assert_eq!(current.matches(wiring).count(), 1);
    let restored = current.strip_suffix(wiring).unwrap().to_owned();
    assert_eq!(
        hash(&restored),
        "d0f832f517a6037e1c3f8c7f5b1cca79af990ed2837b8c29b38903cb4baa6033"
    );
    let current = include_str!("../call_carriers_directory.rs");
    let wiring = r#####"
#[path = "call_carriers_directory/reused.rs"]
mod reused;
pub(super) use reused::count as count_joint;
"#####;
    assert_eq!(current.matches(wiring).count(), 1);
    let restored = current.strip_suffix(wiring).unwrap().to_owned();
    assert_eq!(
        hash(&restored),
        "c49b0a20bbb6681d67dfbf177fe3fd41a64c57079afa7d7713aaa78a42d6ab95"
    );
    let current = include_str!("../held_projection.rs");
    let wiring = r#####"
#[path = "shared_local_callees/coupled_projection.rs"]
mod coupled_projection;
pub(super) use coupled_projection::estimate as coupled_uncached_estimate;
"#####;
    assert_eq!(current.matches(wiring).count(), 1);
    let restored = current.strip_suffix(wiring).unwrap().to_owned();
    assert_eq!(
        hash(&restored),
        "1e6c9f4893c8710afd7897e8e128a238d9f5c209f05ed411732839bf4921f6c4"
    );
    let current = include_str!("../shared_local_callees.rs");
    let wiring = r#####"
#[path = "shared_local_callees/reused.rs"]
mod reused;
pub(super) use reused::Session;
"#####;
    assert_eq!(current.matches(wiring).count(), 1);
    let restored = current.strip_suffix(wiring).unwrap().to_owned();
    let restored = restored.replace(
        "pub(super) use invoice::Failure;\nuse invoice::Invoice;",
        "use invoice::{Failure, Invoice};",
    );
    assert_eq!(
        hash(&restored),
        "43ba5052c503b1560b6568853ff5e9929728f468f59a186f21226c95bea4ddef"
    );
    let current = include_str!("invoice.rs");
    let method = r#####"    // A distinct same-invocation constructor pays its own actual/formal and
    // returned owners before formation; the original certificate is unchanged.
    pub(super) fn acquire_with<T>() -> Result<Self, Failure> {
        let bytes = Self::setup_bytes()
            .checked_add(size_of::<T>())
            .ok_or(Failure::Arithmetic)?;
        if !crate::bounded_output::reserve_active_required(bytes) {
            return Err(Failure::Physical);
        }
        let mut ledger = ClassifierLedger::empty_receiving();
        ledger.capture_current_into();
        Ok(Self {
            bytes: Cell::new(bytes),
            remaining: Cell::new(WORK),
            ledger,
        })
    }

"#####;
    assert_eq!(current.matches(method).count(), 1);
    let restored = current.replacen(method, "", 1).replace(
        "pub(in crate::workspace_graph::expected_projection) enum Failure {",
        "pub(super) enum Failure {",
    );
    assert_eq!(
        hash(&restored),
        "d034d61bae469f0e68ab2c9a4cc9ee31e16ef62b57712af4293d1f035a46858d"
    );
    let current = include_str!("../uncached_peak.rs");
    let changed = "super::held_projection::coupled_uncached_estimate(";
    assert_eq!(current.matches(changed).count(), 2);
    let restored = current.replace(changed, "synthetic_builder_bytes_scoped_with_registry(");
    assert_eq!(
        hash(&restored),
        "3976a4f5d907c169b222c2568bdeb98257298b70a4db2f4824b5e063ec6a151f"
    );
}

#[test]
fn coupled_estimate_retains_the_whole_old_algorithm_and_rejects_other_modes() {
    use sha2::{Digest, Sha256};
    let source = include_str!("coupled_projection.rs");
    let start = source.find("    if layout_mode != 6 {").unwrap();
    let end = source.rfind("\n}").unwrap();
    let mut body = source[start..end].replace("super::super::", "super::");
    let changed = r#####"    if layout_mode != 6 {
        return Err(vec![limit_error("builder_bytes", active_builder_limit())]);
    }
    let core = None;
    let retention = cached_exact::Retention::Parsed;
"#####;
    let old = r#####"    let (registry, core) = proofs;
    let (layout_mode, retention) = profile;
"#####;
    assert_eq!(body.matches(changed).count(), 1);
    body = body.replacen(changed, old, 1);
    let changed = r#####"    let (call_data, shared_callees) = super::call_carriers::count_joint(
        super::shared_local_callees::Context {
            program,
            authored,
            programs,
        },
        include_imports,
    )?;
    raw.exact_carrier_free_calls = call_data.roots;
    let call_proof = call_data.bytes;
"#####;
    let old = r#####"    let call_proof = if layout_mode >= 5 {
        let proof = super::call_carriers::count(program, authored, include_imports);
        raw.exact_carrier_free_calls = proof.roots;
        proof.bytes
    } else {
        0
    };
"#####;
    assert_eq!(body.matches(changed).count(), 1);
    body = body.replacen(changed, old, 1);
    let changed = r#####"    let identity_slots = identity_slots
        .checked_sub(shared_callees.slots)"#####;
    let old = r#####"    let shared_callees = if layout_mode == 6 && retention == cached_exact::Retention::Parsed {
        super::shared_local_callees::costs(super::shared_local_callees::Context {
            program,
            authored,
            programs,
        })?
    } else {
        super::shared_local_callees::Costs { slots: 0, bytes: 0 }
    };
    let identity_slots = identity_slots
        .checked_sub(shared_callees.slots)"#####;
    assert_eq!(body.matches(changed).count(), 1);
    body = body.replacen(changed, old, 1);
    let changed = r#####"                        .and_then(|n| {
                            n.checked_mul(super::super::HIR_EXPR_FIXED_BUNDLE)
                        })"#####;
    let old = r#####"                        .and_then(|n| n.checked_mul(super::super::HIR_EXPR_FIXED_BUNDLE))"#####;
    assert_eq!(body.matches(changed).count(), 1);
    body = body.replacen(changed, old, 1);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(body.as_bytes()))
        ),
        "2ed613cad2b84da178d0a1f6101bd1f43d6dee8fcccdeb4c4122beefeb30abe9"
    );
    let programs = vec![crate::parse(&self::source(2, 1), "mode.spx").unwrap()];
    let authored = crate::workspace_graph::index_authored(&programs).unwrap();
    for mode in [0, 1, 2, 3, 4, 5, 7, u8::MAX] {
        assert!(held_projection::coupled_uncached_estimate(
            &programs[0],
            &authored,
            &programs,
            None,
            mode,
            true,
            None
        )
        .is_err());
    }
}

#[test]
fn nested_zero_ambiguous_and_imported_calls_preserve_independent_censuses() {
    for headers in [2, 64, 129] {
        let plain = source(headers, 1);
        let nested = plain.replace("{helper(1)}", "{helper(helper(1))}");
        let programs = [crate::parse(&nested, "nested.spx").unwrap()];
        let authored = crate::workspace_graph::index_authored(&programs).unwrap();
        let ctx = context(&programs, &authored);
        let (_, candidate) = count(ctx, true).unwrap();
        assert_eq!(candidate.slots, 2);
        assert_eq!(
            candidate.slots,
            shared_local_callees::costs(ctx).unwrap().slots
        );

        // Duplicate raw names have distinct authored identities. Neither the
        // linear scan nor a sorted directory may select its first occurrence.
        let mut programs = [crate::parse(&plain, "ambiguous.spx").unwrap()];
        let mut duplicate = programs[0].functions[0].clone();
        duplicate.stable_id = "loan.duplicate".into();
        programs[0].functions.push(duplicate);
        let authored = crate::workspace_graph::index_authored(&programs).unwrap();
        let ctx = context(&programs, &authored);
        for imports in [false, true] {
            let (auxiliary, candidate) = count(ctx, imports).unwrap();
            assert_eq!(candidate.slots, 0);
            assert_eq!(candidate.bytes, 0, "no constructor is reached");
            assert_eq!(shared_local_callees::costs(ctx).unwrap().slots, 0);
            assert_eq!(
                auxiliary.roots,
                super::super::count(ctx.program, &authored, imports).roots
            );
        }
    }
    for body in [
        "{0}",
        "{helper<i64>(1)}",
        "{let helper=1;helper(1)}",
        "{let outer={let helper=1;helper(1)};0}",
    ] {
        let programs =
            [crate::parse(&source(2, 1).replace("{helper(1)}", body), "excluded.spx").unwrap()];
        let authored = crate::workspace_graph::index_authored(&programs).unwrap();
        let ctx = context(&programs, &authored);
        let (auxiliary, candidate) = count(ctx, true).unwrap();
        assert_eq!((candidate.slots, candidate.bytes), (0, 0));
        assert_eq!(shared_local_callees::costs(ctx).unwrap().slots, 0);
        assert_eq!(
            auxiliary.roots,
            super::super::count(ctx.program, &authored, true).roots
        );
    }
    let main = source(2, 1)
        .replace(
            "module loan.local;",
            "module loan.local; use function @id(\"provider.remote\") from provider as remote;",
        )
        .replace("{helper(1)}", "{remote(1)+helper(1)}");
    let programs = [
        crate::parse(&main, "consumer.spx").unwrap(),
        crate::parse(
            "module provider; @id(\"provider.remote\") fn remote(x:i64)->i64{x}",
            "provider.spx",
        )
        .unwrap(),
    ];
    let authored = crate::workspace_graph::index_authored(&programs).unwrap();
    let ctx = context(&programs, &authored);
    for imports in [false, true] {
        let (auxiliary, candidate) = count(ctx, imports).unwrap();
        assert_eq!(
            (candidate.slots, candidate.bytes),
            (0, 0),
            "aliases exclude the callee certificate even when output omits imports"
        );
        assert!(candidate.slots <= shared_local_callees::costs(ctx).unwrap().slots);
        assert_eq!(
            auxiliary.roots,
            super::super::count(ctx.program, &authored, imports).roots
        );
    }
}
