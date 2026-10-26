use super::*;
use crate::ast::program_source::ProgramSource;
use crate::bounded_output::{active_remaining, with_limit_usage_refusal};
use crate::test_allocation_observer::measure;
use crate::workspace_graph::index_authored;
const LIMIT: usize = 32 * 1024 * 1024;
const PROVIDER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/std/collections/src/collections.spx"
));
const CALLER: &str = r#"module alias.current;
use function @id("std.collections.vec.with-capacity") from std.collections as with_capacity;
use function @id("std.collections.vec.push") from std.collections as push;
use function @id("std.collections.vec.get") from std.collections as get;
@id("alias.current.main") fn main()->i64 {let values=with_capacity<i64>(2usize);let appended=push<i64>(values,42);get<i64>(appended,0usize)}"#;
fn sources(caller: &str) -> [Program; 2] {
    [
        crate::parse(caller, "caller.spx").unwrap(),
        crate::parse(PROVIDER, "provider.spx").unwrap(),
    ]
}
fn linked<T>(f: impl FnOnce() -> T) -> T {
    crate::vec_ops::with_authenticated_linked_source(|| {
        crate::box_ops::with_authenticated_linked_source(f)
    })
}
#[test]
fn retained_same_name_views_lend_actual_provider_payload_in_original_order_and_hir() {
    let programs = sources(CALLER);
    let authored = index_authored(&programs).unwrap();
    let ((), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
        let registry = LayoutRegistry::new(&programs, &authored);
        let mut owner = Receiver::acquire().unwrap();
        let old = super::super::before_alias::construct_using(
            &programs[0],
            &authored,
            &programs,
            &registry,
            None,
            &mut owner,
        )
        .unwrap();
        let view = construct_transient(
            &programs[0],
            &authored,
            &programs,
            &registry,
            None,
            &mut owner,
        )
        .unwrap();
        assert_eq!(
            view.functions().iter().collect::<Vec<_>>(),
            old.functions().iter().collect::<Vec<_>>()
        );
        assert_eq!(
            view.types().iter().collect::<Vec<_>>(),
            old.types().iter().collect::<Vec<_>>()
        );
        for import in &programs[0].module_uses {
            let provider = programs[1]
                .functions
                .iter()
                .find(|f| f.stable_id == import.persistent_id)
                .unwrap();
            let actual = view
                .functions()
                .iter()
                .find(|f| f.stable_id == import.persistent_id)
                .unwrap();
            assert!(std::ptr::eq(actual, provider));
            assert!(std::ptr::eq(&actual.body, &provider.body));
            assert!(!std::ptr::eq(
                old.functions()
                    .iter()
                    .find(|f| f.stable_id == import.persistent_id)
                    .unwrap(),
                provider
            ));
        }
        let layout = registry.get(&programs[0], &authored, &programs).unwrap();
        let expected = linked(|| {
            crate::hir::identity_resolution::before_function_alias::resolve_source_nominal(
                &old,
                layout.presentation(&programs[0], &old).unwrap(),
                |bytes| crate::workspace_graph::reserve_builder_structure(bytes),
            )
        })
        .unwrap()
        .0;
        let actual = linked(|| {
            crate::hir::identity_resolution::resolve_source_nominal(
                &view,
                layout.presentation(&programs[0], &view).unwrap(),
                |bytes| crate::workspace_graph::reserve_builder_structure(bytes),
            )
        })
        .unwrap()
        .0;
        assert_eq!(actual, expected);
        crate::hir::validate(&actual).unwrap();
        // Exercise the complete immediate-old owning and base-prelude
        // receivers as well as its borrowed pass, retaining their full bodies.
        let old_key =
            crate::workspace_graph::expected_projection::borrowed_leaf::checked_key_owned(old)
                .unwrap();
        let owning = linked(|| {
            crate::hir::identity_resolution::before_function_alias::resolve_checked_nominal(
                &old_key,
                None,
                None,
                None,
                None,
                |bytes| crate::workspace_graph::reserve_builder_structure(bytes),
            )
        })
        .unwrap()
        .0;
        assert_eq!(owning, expected);
        let mut identity_owner =
            crate::bounded_output::identity_reservation::IdentityReservation::acquire().unwrap();
        let mut invocation = crate::hir::prelude_bundle::Invocation::new();
        let with_base = linked(|| {
            crate::hir::identity_resolution::before_function_alias::resolve_source_checked_previous_with_base(&old_key,None,None,None,None,&mut invocation,|bytes|identity_owner.reserve(bytes))
        });
        match with_base {
            Ok((resolved,_))=>assert_eq!(resolved,expected),
            Err(crate::hir::identity_resolution::before_function_alias::NominalReceiverFailure::Resolve(error))=>panic!("old resolver refusal: {error:?}"),
            Err(crate::hir::identity_resolution::before_function_alias::NominalReceiverFailure::Base(error))=>panic!("old base refusal: {error:?}"),
        }
        drop((owning, old_key, invocation, identity_owner));
        drop((actual, expected, layout, view, owner, registry));
    });
    assert!(!refused && first.is_none());
}
#[test]
fn renamed_and_malformed_wrapper_payloads_keep_the_complete_owned_fallback() {
    for caller in [
        CALLER
            .replace("as get;", "as read_value;")
            .replace("get<i64>(appended", "read_value<i64>(appended"),
        CALLER.to_owned(),
    ] {
        let mut programs = sources(&caller);
        if caller == CALLER {
            programs[1]
                .functions
                .iter_mut()
                .find(|f| f.stable_id == "std.collections.vec.get")
                .unwrap()
                .effects
                .push("hostile.effect".to_owned());
        }
        let authored = index_authored(&programs).unwrap();
        let ((), refused, _, _) = with_limit_usage_refusal(LIMIT, || {
            let registry = LayoutRegistry::new(&programs, &authored);
            let mut owner = Receiver::acquire().unwrap();
            let old = super::super::before_alias::construct_using(
                &programs[0],
                &authored,
                &programs,
                &registry,
                None,
                &mut owner,
            );
            let view = construct_transient(
                &programs[0],
                &authored,
                &programs,
                &registry,
                None,
                &mut owner,
            );
            match (old, view) {
                (Ok(old), Ok(view)) => {
                    assert_eq!(
                        view.functions().iter().collect::<Vec<_>>(),
                        old.functions().iter().collect::<Vec<_>>()
                    );
                    let original = programs[1]
                        .functions
                        .iter()
                        .find(|f| f.stable_id == "std.collections.vec.get")
                        .unwrap();
                    let current = view
                        .functions()
                        .iter()
                        .find(|f| f.stable_id == original.stable_id)
                        .unwrap();
                    assert!(!std::ptr::eq(current, original));
                }
                (Err(Failure::One(old)), Err(Failure::One(current))) => assert_eq!(old, current),
                (Err(Failure::Many(old)), Err(Failure::Many(current))) => assert_eq!(old, current),
                _ => panic!("old and current semantic refusal shapes differ"),
            }
        });
        assert!(!refused);
    }
}
#[test]
fn whole_constructor_owner_and_drop_report_actual_costs_without_universal_gain_claim() {
    let programs = sources(CALLER);
    let authored = index_authored(&programs).unwrap();
    let observe = |current, limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let registry = LayoutRegistry::new(&programs, &authored);
                let mut owner = match Receiver::acquire() {
                    Ok(owner) => owner,
                    Err(_) => return false,
                };
                let ok = if current {
                    match construct_transient(
                        &programs[0],
                        &authored,
                        &programs,
                        &registry,
                        None,
                        &mut owner,
                    ) {
                        Ok(view) => {
                            drop(view);
                            true
                        }
                        Err(failure) => {
                            drop(failure);
                            false
                        }
                    }
                } else {
                    match super::super::before_alias::construct_using(
                        &programs[0],
                        &authored,
                        &programs,
                        &registry,
                        None,
                        &mut owner,
                    ) {
                        Ok(view) => {
                            drop(view);
                            true
                        }
                        Err(failure) => {
                            drop(failure);
                            false
                        }
                    }
                };
                drop(owner);
                drop(registry);
                ok
            })
        })
    };
    // Initialize shared process metadata with the complete original constructor;
    // both measured sides then use the same process/cache/source state.
    assert!(observe(false, LIMIT).0 .0);
    let old = observe(false, LIMIT);
    let new = observe(true, LIMIT);
    assert!(old.0 .0 && new.0 .0 && !old.1 && !new.1);
    for measured in [&old, &new] {
        let metrics = &measured.0 .1;
        assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
    }
    eprintln!(
        "847 whole constructor old paid={} physical={:?}; current paid={} physical={:?}",
        old.2, old.0 .1, new.2, new.0 .1
    );
    for limit in [0, new.2 - 1, new.2] {
        let observed = observe(true, limit);
        assert_eq!(observed.0 .0, limit == new.2);
        let m = observed.0 .1;
        assert!(!m.invalid && m.live == 0 && m.allocated == m.freed);
    }
}
#[test]
fn foreign_and_restored_owner_fail_before_any_projection_or_allocation() {
    let programs = sources(CALLER);
    let authored = index_authored(&programs).unwrap();
    let ((), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
        let registry = LayoutRegistry::new(&programs, &authored);
        let mut owner = Receiver::acquire().unwrap();
        let (((), metrics), child_refused, child_paid, child_first) =
            with_limit_usage_refusal(0, || {
                measure(|| {
                    assert!(matches!(
                        construct_transient(
                            &programs[0],
                            &authored,
                            &programs,
                            &registry,
                            None,
                            &mut owner
                        ),
                        Err(Failure::Setup(_))
                    ));
                })
            });
        assert!(!child_refused && child_paid == 0 && child_first.is_none());
        assert!(!metrics.invalid && metrics.allocated == 0);
        let before = active_remaining();
        let ((), metrics) = measure(|| {
            assert!(matches!(
                construct_transient(
                    &programs[0],
                    &authored,
                    &programs,
                    &registry,
                    None,
                    &mut owner
                ),
                Err(Failure::Setup(_))
            ));
        });
        assert_eq!(active_remaining(), before);
        assert!(!metrics.invalid && metrics.allocated == 0);
    });
    assert!(refused && first.is_some());
}

#[test]
fn mixed_borrowed_renamed_and_owned_type_roots_keep_partial_recursive_drop() {
    let provider=format!("{PROVIDER}\n@id(\"alias.row\") record Row {{ @id(\"alias.row.value\") value:i64, }}\n@id(\"alias.plain\") fn plain(value:Row)->Row {{value}}\n");
    let caller=CALLER.replace("@id(\"alias.current.main\")", "use type @id(\"alias.row\") from std.collections as Row; use function @id(\"alias.plain\") from std.collections as ordinary; @id(\"alias.current.main\")").replace("as get;","as read_value;").replace("get<i64>(appended","read_value<i64>(appended");
    let programs = [
        crate::parse(&caller, "caller.spx").unwrap(),
        crate::parse(&provider, "provider.spx").unwrap(),
    ];
    let authored = index_authored(&programs).unwrap();
    let observe = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let registry = LayoutRegistry::new(&programs, &authored);
                let mut owner = match Receiver::acquire() {
                    Ok(owner) => owner,
                    Err(_) => return false,
                };
                let result = construct_transient(
                    &programs[0],
                    &authored,
                    &programs,
                    &registry,
                    None,
                    &mut owner,
                );
                let success = result.is_ok();
                drop(result);
                drop(owner);
                drop(registry);
                success
            })
        })
    };
    let ((), refused, _, _) = with_limit_usage_refusal(LIMIT, || {
        let registry = LayoutRegistry::new(&programs, &authored);
        let mut owner = Receiver::acquire().unwrap();
        let old = super::super::before_alias::construct_using(
            &programs[0],
            &authored,
            &programs,
            &registry,
            None,
            &mut owner,
        )
        .unwrap();
        let current = construct_transient(
            &programs[0],
            &authored,
            &programs,
            &registry,
            None,
            &mut owner,
        )
        .unwrap();
        assert_eq!(
            old.functions().iter().collect::<Vec<_>>(),
            current.functions().iter().collect::<Vec<_>>()
        );
        assert_eq!(
            old.types().iter().collect::<Vec<_>>(),
            current.types().iter().collect::<Vec<_>>()
        );
        for (id, borrowed) in [
            ("std.collections.vec.push", true),
            ("std.collections.vec.get", false),
            ("alias.plain", false),
        ] {
            let source = programs[1]
                .functions
                .iter()
                .find(|f| f.stable_id == id)
                .unwrap();
            let value = current
                .functions()
                .iter()
                .find(|f| f.stable_id == id)
                .unwrap();
            assert_eq!(std::ptr::eq(source, value), borrowed);
        }
    });
    assert!(!refused);
    let success = observe(LIMIT);
    assert!(success.0 .0 && !success.1);
    for limit in [0, success.2 - 1, success.2] {
        let result = observe(limit);
        assert_eq!(result.0 .0, limit == success.2);
        let m = result.0 .1;
        assert!(!m.invalid && m.live == 0 && m.allocated == m.freed);
    }
}

#[test]
fn complete_immediate_sources_and_independent_old_receivers_remain_authenticated() {
    use sha2::{Digest, Sha256};
    let manifest = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/retained-function-alias-847/current-to-7a87.json"
    ));
    let rows: serde_json::Value = serde_json::from_str(manifest).unwrap();
    for row in rows.as_array().unwrap() {
        let source = row["source"].as_str().unwrap();
        let current =
            std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(source))
                .unwrap();
        let current = crate::source_reflows_merge::retained_function847_input(source, &current);
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
            ),
            row["current_sha256"]
        );
        let edits = row["edits"].as_array().unwrap();
        assert_eq!(edits.len(), 1);
        assert_eq!(current, edits[0]["new"].as_str().unwrap());
        let old = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(row["archive"].as_str().unwrap()),
        )
        .unwrap();
        assert_eq!(old, edits[0]["old"].as_str().unwrap());
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(old.as_bytes()))
            ),
            row["original_sha256"]
        );
    }
    let owners: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/retained-function-alias-847/new-owners.json"
    )))
    .unwrap();
    for owner in owners.as_array().unwrap() {
        let source = owner["source"].as_str().unwrap();
        let current =
            std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(source))
                .unwrap();
        let current = crate::source_reflows_merge::retained_function847_input(source, &current);
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
            ),
            owner["sha256"]
        );
    }
    let receivers: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/retained-function-alias-847/independent-receivers.json"
    )))
    .unwrap();
    for row in receivers.as_array().unwrap() {
        let source = row["source"].as_str().unwrap();
        let current =
            std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(source))
                .unwrap();
        let current = crate::source_reflows_merge::retained_function847_input(source, &current);
        assert_eq!(current, row["compiled"].as_str().unwrap());
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
            ),
            row["compiled_sha256"]
        );
        let old = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(row["archive"].as_str().unwrap()),
        )
        .unwrap();
        assert_eq!(old, row["original"].as_str().unwrap());
    }
}
