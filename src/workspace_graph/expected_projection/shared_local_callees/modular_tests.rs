//! Local loans stay separate from every imported alias and its target.
use super::super::WORK;
use super::*;
use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal};
use crate::test_allocation_observer::measure;
use sha2::{Digest, Sha256};
pub(in crate::workspace_graph::expected_projection) const SOURCE: &str = "module modular.local; use function @id(\"provider.remote\") from provider as remote; @id(\"helper\") fn helper(x:i64)->i64{x} @id(\"other\") fn other(x:i64)->i64{x} @id(\"main\") fn main()->i64{remote(0)+helper(1)+helper(2)}";
pub(in crate::workspace_graph::expected_projection) fn programs() -> [Program; 2] {
    [
        crate::parse(SOURCE, "modular.spx").unwrap(),
        crate::parse(
            "module provider; @id(\"provider.remote\") fn remote(x:i64)->i64{x}",
            "provider.spx",
        )
        .unwrap(),
    ]
}
pub(in crate::workspace_graph::expected_projection) fn context<'a>(
    programs: &'a [Program],
    index: &'a Index<'a>,
) -> Context<'a, 'a> {
    Context {
        program: &programs[0],
        authored: index,
        programs,
    }
}

pub(in crate::workspace_graph::expected_projection) fn restore(path: &str, source: &str) -> String {
    let imports = imports_before(path, source);
    let source = imports.as_str();
    let record = super::record_signatures::tests::restore(path, source);
    let source = record.as_str();
    let packet: serde_json::Value =
        serde_json::from_str(include_str!("modular_inverse.json")).unwrap();
    let canonical = if path.starts_with("src/") {
        path.to_owned()
    } else {
        format!("src/workspace_graph/expected_projection/{path}")
    };
    let Some(entry) = packet["entries"].get(&canonical) else {
        return source.to_owned();
    };
    let hash = |s: &str| {
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(s.as_bytes()))
        )
    };
    assert_eq!(hash(source), entry["current_sha256"].as_str().unwrap());
    let mut prior = source.to_owned();
    for splice in entry["splices"].as_array().unwrap() {
        let current = splice["current"].as_str().unwrap();
        assert_eq!(prior.matches(current).count(), 1);
        prior = prior.replacen(current, splice["prior"].as_str().unwrap(), 1);
    }
    assert_eq!(hash(&prior), entry["prior_sha256"].as_str().unwrap());
    prior
}

#[test]
fn actual_linear_and_directory_loans_never_promote_use_to_local() {
    let p = programs();
    let i = crate::workspace_graph::index_authored(&p).unwrap();
    let ctx = context(&p, &i);
    for imports in [false, true] {
        for name in ["helper", "remote"] {
            let expected = if name == "helper" {
                (true, true)
            } else {
                (imports, false)
            };
            assert_eq!(
                super::super::super::call_carriers::modular_linear_loan(ctx, imports, name, 1),
                expected
            );
            assert_eq!(
                super::super::super::call_carriers::modular_indexed_loan::<128>(
                    ctx, imports, name, 1
                ),
                expected
            );
        }
        let (aux, proof) = super::super::super::call_carriers::count_modular(ctx, imports).unwrap();
        assert_eq!(proof.slots, 2);
        assert_eq!(
            aux.roots,
            super::super::super::call_carriers::count(ctx.program, &i, imports).roots
        );
        assert_eq!(
            super::super::super::call_carriers::count_joint(ctx, imports)
                .unwrap()
                .1
                .slots,
            0
        );
    }
}

#[test]
fn every_competing_alias_kind_closes_owner_or_target_even_without_provider() {
    for alias in ["helper", "main"] {
        for kind in [
            crate::ast::ModuleUseKind::Function,
            crate::ast::ModuleUseKind::Type,
            crate::ast::ModuleUseKind::Protocol,
        ] {
            let mut p = programs();
            p[0].module_uses.push(crate::ast::ModuleUse {
                kind,
                persistent_id: "missing.id".into(),
                target_module: "missing.provider".into(),
                alias: alias.into(),
                span: crate::ast::Span::default(),
            });
            let i = crate::workspace_graph::index_authored(&p).unwrap();
            let ctx = context(&p, &i);
            for imports in [false, true] {
                // Parser-only malformed namespace/provider hostiles claim no
                // workspace validity. The retained flag cannot bypass alias proof.
                let (_, proof) =
                    super::super::super::call_carriers::count_modular(ctx, imports).unwrap();
                assert_eq!(proof.slots, 0, "{alias} {kind:?} imports={imports}");
            }
        }
    }
}

#[test]
fn duplicate_local_names_generic_default_shadow_and_unknown_bodies_keep_full_slots() {
    for replacement in [
        "{remote(0)+helper<i64>(1)}",
        "{let helper=1;helper(1)}",
        "{let x=match 1{0=>helper(1),_=>0,};0}",
    ] {
        let mut p = programs();
        p[0] = crate::parse(
            &SOURCE.replace("{remote(0)+helper(1)+helper(2)}", replacement),
            "modular.spx",
        )
        .unwrap();
        let i = crate::workspace_graph::index_authored(&p).unwrap();
        assert_eq!(
            super::super::super::call_carriers::count_modular(context(&p, &i), false)
                .unwrap()
                .1
                .slots,
            0
        );
    }
    for attack in 0..3 {
        let mut p = programs();
        match attack {
            0 => {
                let mut duplicate = p[0].functions[0].clone();
                duplicate.stable_id = "duplicate".into();
                p[0].functions.push(duplicate);
            }
            1 => p[0].functions[0].explicit_id = false,
            _ => {
                p[0] = crate::parse(
                    &SOURCE.replace("fn helper(x:i64)", "fn helper<T>(x:i64)"),
                    "modular.spx",
                )
                .unwrap();
            }
        }
        let i = crate::workspace_graph::index_authored(&p).unwrap();
        assert_eq!(
            super::super::super::call_carriers::count_modular(context(&p, &i), false)
                .unwrap()
                .1
                .slots,
            0
        );
    }
}

#[test]
fn foreign_function_context_lender_and_optional_work_never_mint_modular_slots() {
    let p = programs();
    let i = crate::workspace_graph::index_authored(&p).unwrap();
    let other = crate::workspace_graph::index_authored(&p).unwrap();
    let ctx = context(&p, &i);
    let owner = &p[0].functions[2];
    let target = &p[0].functions[0];
    let copy = target.clone();
    let ((), refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
        let mut current = None;
        Session::observe(&mut current, ctx, owner, target, 1).unwrap();
        assert_eq!(Session::checkpoint(&current), 1);
        Session::observe(&mut current, ctx, owner, &copy, 1).unwrap();
        assert_eq!(Session::checkpoint(&current), 1);
        assert!(Session::observe(
            &mut current,
            Context {
                authored: &other,
                ..ctx
            },
            owner,
            target,
            1
        )
        .is_err());
        let ((), foreign_refused, paid, foreign_first) =
            with_limit_usage_refusal(usize::MAX, || {
                assert!(Session::observe(&mut current, ctx, owner, target, 1).is_err());
            });
        assert!(!foreign_refused && foreign_first.is_none());
        assert_eq!(paid, 0);
        assert!(Session::finish(current, ctx).is_err());
        assert!(!reserve_active_required(0));
    });
    assert!(refused && first.is_some());
    with_limit_usage_refusal(usize::MAX, || {
        let mut current = None;
        Session::observe(&mut current, ctx, owner, target, 1).unwrap();
        assert_eq!(
            current.as_ref().unwrap().invoice.work(WORK),
            Err(Failure::Work)
        );
        Session::observe(&mut current, ctx, owner, &p[0].functions[1], 1).unwrap();
        assert_eq!(Session::checkpoint(&current), 0);
        Session::rollback(&mut current, 1).unwrap();
        assert_eq!(Session::finish(current, ctx).unwrap().slots, 0);
    });
}

#[test]
fn actual_imported_module_proof_pays_exact_short_zero_and_drops_completely() {
    let p = programs();
    let i = crate::workspace_graph::index_authored(&p).unwrap();
    let ctx = context(&p, &i);
    let run = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let result = super::super::super::call_carriers::count_modular(ctx, false);
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
    for (limit, success) in [(paid, true), (paid - 1, false), (0, false)] {
        let ((ok, allocation), refused, _, first) = run(limit);
        assert_eq!(ok, success);
        assert_eq!(refused, !success);
        assert_eq!(first.is_some(), !success);
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
    }
}

#[test]
fn all_old_source_entries_reconstruct_complete_558_without_reinterpreting_oracles() {
    for (path, current) in [
        ("call_carriers.rs", include_str!("../call_carriers.rs")),
        (
            "call_carriers_joint.rs",
            include_str!("../call_carriers_joint.rs"),
        ),
        (
            "call_carriers_directory.rs",
            include_str!("../call_carriers_directory.rs"),
        ),
        ("held_projection.rs", include_str!("../held_projection.rs")),
        (
            "shared_local_callees.rs",
            include_str!("../shared_local_callees.rs"),
        ),
        ("uncached_peak.rs", include_str!("../uncached_peak.rs")),
    ] {
        let path = format!("src/workspace_graph/expected_projection/{path}");
        assert!(
            serde_json::from_str::<serde_json::Value>(include_str!("modular_inverse.json"))
                .unwrap()["entries"]
                .get(&path)
                .is_some()
        );
        restore(&path, current);
    }
}

// Exact entry only: explicit imports and the two scoped compiler annotations
// precede the record/modular/paired historical stages. Unknown scopes are not
// rewritten, and a known scope must authenticate its COMPLETE current bytes.
pub(in crate::workspace_graph::expected_projection) fn imports_before(
    path: &str,
    source: &str,
) -> String {
    let canonical_entry = if path.starts_with("src/") {
        path.to_owned()
    } else {
        format!("src/workspace_graph/expected_projection/{path}")
    };
    let source903 = crate::source_reflows::next_custody_before("constructor903", &canonical_entry, source);
    let source = source903.as_str();
    let production =
        crate::source_reflows::next_custody_before("production7834", &canonical_entry, source);
    let entry = crate::source_reflows::next_custody_before(
        "modular916",
        &canonical_entry,
        production.as_str(),
    );
    let source = entry.as_str();
    let packet: serde_json::Value =
        serde_json::from_str(include_str!("modular_inverse.json")).unwrap();
    let canonical = if path.starts_with("src/") {
        path.to_owned()
    } else {
        format!("src/workspace_graph/expected_projection/{path}")
    };
    let Some(entry) = packet["import_entry_stages"].get(&canonical) else {
        return source.to_owned();
    };
    let hash = |raw: &str| {
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(raw.as_bytes()))
        )
    };
    assert_eq!(
        (source.len(), hash(source)),
        (
            entry["current_bytes"].as_u64().unwrap() as usize,
            entry["current_sha256"].as_str().unwrap().to_owned()
        )
    );
    let mut restored = source.to_owned();
    for splice in entry["splices"].as_array().unwrap() {
        let current = splice["current"].as_str().unwrap();
        assert_eq!(restored.matches(current).count(), 1);
        restored = restored.replacen(current, splice["prior"].as_str().unwrap(), 1);
    }
    assert_eq!(
        (restored.len(), hash(&restored)),
        (
            entry["prior_bytes"].as_u64().unwrap() as usize,
            entry["prior_sha256"].as_str().unwrap().to_owned()
        )
    );
    restored
}
