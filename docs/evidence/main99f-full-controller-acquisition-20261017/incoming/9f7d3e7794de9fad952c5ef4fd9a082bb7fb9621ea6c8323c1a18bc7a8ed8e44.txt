use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;
const LIMIT: usize = 64 * 1024 * 1024;
fn program() -> ResolvedProgram {
    crate::hir::resolve(&crate::check(
        "module test.paid.directory; @id(\"take\") fn take(x:i64)->i64 { x } @id(\"main\") fn main()->i64 { take(3) }",
        "paid-directory.spx",
    ).unwrap()).unwrap()
}
#[test]
fn owned_function_directory_shares_exact_program_and_keeps_full_original_lookup_order() {
    let program = program();
    let old = original::authenticate(&program, None).unwrap();
    let (((), actual), refused, paid, first) = with_limit_usage_refusal(LIMIT, || {
        measure(|| {
            let current = authenticate(&program, None).unwrap();
            assert!(matches!(current, Lookup::Owned { .. }));
            for id in old.keys() {
                assert!(std::ptr::eq(
                    *old.get(id).unwrap(),
                    *current.get(id).unwrap()
                ));
            }
            let shared = current.shared_directory().unwrap();
            let same = authenticate_shared(&program, None, Some(shared.clone())).unwrap();
            assert!(std::rc::Rc::ptr_eq(
                &shared,
                &same.shared_directory().unwrap()
            ));
            assert_eq!(current.len(), same.len());
            drop(same);
            drop(current);
            drop(shared);
        })
    });
    assert!(!refused && first.is_none() && !actual.invalid && actual.live == 0);
    assert!(
        actual.allocated <= paid && actual.highwater <= paid && actual.allocated == actual.freed
    );
    let foreign = program.clone();
    let current = authenticate(&program, None).unwrap();
    assert!(matches!(current, Lookup::Legacy(_)));
    let shared = crate::hir::function_directory::caller::prepare_shared(&program).unwrap();
    assert_eq!(
        authenticate_shared(&foreign, None, Some(shared))
            .err()
            .unwrap(),
        hir_error("function directory is not bound to the current resolved program")
    );
}
#[test]
fn owned_directory_complete_receiving_drop_keeps_exact_short_zero_and_sticky_first() {
    let program = program();
    let observe = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let current = authenticate(&program, None);
                let ok = current.is_ok();
                if !ok {
                    assert!(authenticate(&program, None).is_err());
                }
                drop(current);
                ok
            })
        })
    };
    let ((ok, actual), refused, paid, first) = observe(LIMIT);
    assert!(ok && !refused && first.is_none() && !actual.invalid && actual.live == 0);
    assert!(
        actual.allocated <= paid && actual.highwater <= paid && actual.allocated == actual.freed
    );
    let ((ok, actual), refused, used, first) = observe(paid);
    assert!(ok && !refused && used == paid && first.is_none() && actual.live == 0);
    for limit in [0, paid - 1] {
        let ((ok, actual), refused, used, first) = observe(limit);
        assert!(!ok && refused && first.is_some() && used <= limit);
        assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
    }
}
#[test]
fn full_original_constructor_seed_and_paid_seed_preserve_all_ordered_semantic_errors() {
    let base = program();
    for case in 0..4 {
        let mut program = base.clone();
        match case {
            1 => program.entrypoint = DeclarationId::new("bad\0identity"),
            2 => {
                program.functions[0].name.push_str(".forged");
            }
            3 => {
                let copy = program.functions[0].clone();
                program.functions.push(copy);
            }
            _ => {}
        }
        let old = constructor_seed::original::ConstructorSeed::authenticate(&program, None).err();
        let (((), actual), refused, paid, first) = with_limit_usage_refusal(LIMIT, || {
            measure(|| {
                let current = constructor_seed::ConstructorSeed::authenticate(&program, None).err();
                // Compare every diagnostic field before dropping the complete receiving result.
                // Returning an owned error would keep its backing outside this Drop observation.
                assert_eq!(current, old);
                drop(current);
            })
        });
        assert!(!refused && first.is_none() && !actual.invalid && actual.live == 0);
        // These are complete setup observations, not substituted smaller validators.
        // Other unchanged preflight helpers remain separate unproved owners.
        assert!(actual.allocated == actual.freed && actual.highwater <= actual.allocated);
        assert!(paid <= LIMIT);
    }
}
#[test]
fn actual_compatibility_constructor_is_paid_through_drop_at_exact_short_and_zero() {
    let program = program();
    let observe = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let validator = HirValidator::new(&program);
                let ok = validator.is_ok();
                drop(validator);
                ok
            })
        })
    };
    let ((ok, actual), refused, paid, first) = observe(LIMIT);
    assert!(ok && !refused && first.is_none() && !actual.invalid && actual.live == 0);
    assert!(
        actual.allocated <= paid && actual.highwater <= paid && actual.allocated == actual.freed
    );
    let ((ok, actual), refused, used, first) = observe(paid);
    assert!(ok && !refused && used == paid && first.is_none() && actual.live == 0);
    for limit in [0, paid - 1] {
        let ((ok, actual), refused, used, first) = observe(limit);
        assert!(!ok && refused && first.is_some() && used <= limit);
        assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
    }
}

/// Strip only this batch's uniquely recorded whole-source edits before older gates.
pub(in crate::hir::validation) fn project_current(path: &str, source: &str) -> String {
    let records: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../docs/evidence/compat-constructor-preflight-824/exact-source-inverses.json"
    ))
    .unwrap();
    let record = records
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["source"].as_str() == Some(path))
        .unwrap();
    let mut current = source.to_owned();
    for edit in record["edits"].as_array().unwrap().iter().rev() {
        let from = edit["after"].as_str().unwrap();
        let to = edit["before"].as_str().unwrap();
        assert_eq!(
            current.matches(from).count(),
            1,
            "unique named {path} inverse"
        );
        current = current.replacen(from, to, 1);
    }
    current
}
#[test]
fn complete_constructor_preflight_and_lookup_sources_preserve_full_original_bytes() {
    for (path, current, original) in [
        ("src/hir/validation/constructor_seed.rs", ci_str!("../constructor_seed.rs"), ci_str!("../../../../docs/evidence/compat-constructor-preflight-824/src__hir__validation__constructor_seed.rs.before.txt")),
        ("src/hir/validation/function_lookup.rs", ci_str!("../function_lookup.rs"), ci_str!("../../../../docs/evidence/compat-constructor-preflight-824/src__hir__validation__function_lookup.rs.before.txt")),
        ("src/hir/function_directory/caller.rs", ci_str!("../../function_directory/caller.rs"), ci_str!("../../../../docs/evidence/compat-constructor-preflight-824/src__hir__function_directory__caller.rs.before.txt")),
        ("src/hir/validation.rs", ci_str!("../../validation.rs"), ci_str!("../../../../docs/evidence/compat-constructor-preflight-824/src__hir__validation.rs.before.txt")),
        ("src/hir/validation/closure.rs", ci_str!("../closure.rs"), ci_str!("../../../../docs/evidence/compat-constructor-preflight-824/src__hir__validation__closure.rs.before.txt")),
        ("src/hir/validation/stdin_stream.rs", ci_str!("../stdin_stream.rs"), ci_str!("../../../../docs/evidence/compat-constructor-preflight-824/src__hir__validation__stdin_stream.rs.before.txt")),
    ] { assert_eq!(project_current(path, current), original); }
    assert_eq!(ci_str!("../../../stdin_stream_ops/analysis.rs"), ci_str!("../../../../docs/evidence/compat-constructor-preflight-824/src__stdin_stream_ops__analysis.rs.before.txt"));
    assert_eq!(ci_str!("../../../stdin_stream_ops.rs"), ci_str!("../../../../docs/evidence/compat-constructor-preflight-824/src__stdin_stream_ops.rs.before.txt"));
}

#[test]
fn complete_independently_compiled_seed_and_legacy_lookup_are_exact_original_receivers() {
    let old = include_str!("../../../../docs/evidence/compat-constructor-preflight-824/src__hir__validation__constructor_seed.rs.before.txt");
    let current = include_str!("../constructor_seed/original.rs");
    let alias = "use super::super::function_lookup::original as function_lookup;\n";
    assert_eq!(current.matches(alias).count(), 1);
    let saved_nul = "use crate::hir::inspection::validate_nul_free_identities_original as validate_nul_free_identities;\n";
    assert_eq!(current.matches(saved_nul).count(), 1);
    let current = current.replacen(saved_nul, "", 1);
    let saved_generic_graph =
        "use super::super::generic_template::original_call_graph as generic_template;\n";
    assert_eq!(current.matches(saved_generic_graph).count(), 1);
    let current = current.replacen(saved_generic_graph, "", 1);
    let saved_intrinsics = "use super::original_intrinsics as box_intrinsic;\n";
    assert_eq!(current.matches(saved_intrinsics).count(), 1);
    let current = current.replacen(saved_intrinsics, "", 1);
    let current = current.replacen(alias, "", 1).replacen(
        "Option<&'a crate::hir::function_directory::Directory<'a>>",
        "Option<&'a super::super::function_directory::Directory<'a>>",
        1,
    );
    let current = current.replacen(
        "crate::hir::closure::once::reject_reserved_identities",
        "super::super::closure::once::reject_reserved_identities",
        1,
    );
    assert_eq!(current, old);
    let old = include_str!("../../../../docs/evidence/compat-constructor-preflight-824/src__hir__validation__function_lookup.rs.before.txt");
    let current = include_str!("original.rs")
        .replacen(
            "pub(in crate::hir::validation) enum Lookup<'a>",
            "pub(super) enum Lookup<'a>",
            1,
        )
        .replacen(
            "pub(in crate::hir::validation) fn authenticate<'a>",
            "pub(super) fn authenticate<'a>",
            1,
        );
    assert_eq!(current.matches("use super::Permits;").count(), 1);
    let current = current.replacen(
        "use super::Permits;\nuse super::*;\nuse crate::hir::function_directory::Directory;",
        "use super::*;\nuse crate::hir::function_directory::Directory;\nmod permits;\npub(super) use permits::Permits;",
        1,
    ) + "\n#[cfg(test)]\nmod tests;\n";
    assert_eq!(current, old);
}
