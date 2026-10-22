use super::*;
use crate::bounded_output::{self, with_limit_usage_refusal};
use crate::test_allocation_observer::{current_metrics, measure};

const SOURCE: &str = r#"module top.identity;
@id("top.row") record Row { @id("top.row.value") value:i64, }
@id("top.choice") variant Choice {
 @id("top.choice.some") Some { @id("top.choice.value") value:i64, },
 @id("top.choice.none") None,
}
@id("top.main") fn main()->i64 {7}
"#;

fn declarations() -> (Vec<Program>, Vec<(String, ResolvedModuleInput)>) {
    let program = crate::parse(SOURCE, "top-identity.spx").unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let modules = vec![(program.module.clone(), resolved.into())];
    (vec![program], modules)
}

#[test]
fn every_kind_and_empty_nul_unicode_root_keeps_old_bytes_order_and_original_debit() {
    use hir::DeclarationKind::*;
    let index = hir::DeclarationIndex::default();
    for kind in [
        Resource,
        ResourceDrop,
        Record,
        Field,
        Class,
        Variant,
        VariantCase,
        CaseField,
        Interface,
        Import,
        Function,
    ] {
        for text in ["", "a", "root\0member", "decl.λ.日本語", "same-name"] {
            let declaration = hir::Declaration {
                id: hir::DeclarationId::new(text),
                name: "display-name-is-not-identity".into(),
                kind,
                identity_origin: hir::IdentityOrigin::Explicit,
                owner: None,
            };
            for limit in [0, text.len().saturating_sub(1), text.len(), text.len() + 32] {
                let old = with_limit_usage_refusal(limit, || {
                    let old = before785::top_level_declaration(&index, &declaration);
                    old.as_str().to_owned()
                });
                let current = with_limit_usage_refusal(limit, || {
                    top_level_declaration(&index, &declaration)
                        .as_str()
                        .to_owned()
                });
                assert_eq!(current, old, "kind={kind:?} text={text:?} limit={limit}");
            }
        }
    }
    // The original walk stops at the same missing owner without fabricating
    // membership or turning the spelling into provider authority.
    let declaration = hir::Declaration {
        id: hir::DeclarationId::new("orphan.child"),
        name: "same-name".into(),
        kind: Field,
        identity_origin: hir::IdentityOrigin::Automatic,
        owner: Some(hir::DeclarationId::new("absent.parent")),
    };
    assert_eq!(
        top_level_declaration(&index, &declaration).as_str(),
        "orphan.child"
    );
    assert_eq!(
        before785::top_level_declaration(&index, &declaration).as_str(),
        "orphan.child"
    );
}

#[test]
fn immutable_index_walk_lends_the_exact_existing_root_without_identity_allocation() {
    let (_, modules) = declarations();
    let index = &modules[0].1.declarations;
    for declaration in index.declarations() {
        let old = before785::top_level_declaration(index, declaration);
        let root = index.declaration(&old).unwrap();
        let view = top_level_declaration(index, declaration);
        assert_eq!(view.as_str(), old.as_str());
        assert_eq!(view.as_str().as_ptr(), root.id.as_str().as_ptr());
    }
}

#[test]
fn zero_short_exact_floor_sticky_nested_and_restored_accounts_match_original_clone_gate() {
    let index = hir::DeclarationIndex::default();
    let declaration = hir::Declaration {
        id: hir::DeclarationId::new("ledger.λ"),
        name: "ledger".into(),
        kind: hir::DeclarationKind::Function,
        identity_origin: hir::IdentityOrigin::Explicit,
        owner: None,
    };
    let len = declaration.id.as_str().len();
    let run = |current: bool, limit, floor, sticky, nested| {
        with_limit_usage_refusal(limit, || {
            assert!(bounded_output::set_active_floor(floor));
            if sticky {
                assert!(!bounded_output::reserve_active_required(usize::MAX));
            }
            if nested {
                let (_, refused, used, _) = with_limit_usage_refusal(0, || {
                    if current {
                        assert!(top_level_declaration(&index, &declaration)
                            .as_str()
                            .is_empty());
                    } else {
                        assert!(before785::top_level_declaration(&index, &declaration)
                            .as_str()
                            .is_empty());
                    }
                });
                assert!(refused && used == 0);
            }
            let first = if current {
                top_level_declaration(&index, &declaration)
                    .as_str()
                    .to_owned()
            } else {
                before785::top_level_declaration(&index, &declaration)
                    .as_str()
                    .to_owned()
            };
            // Retry on the same real account, including a zero-length clone.
            let second = if current {
                bounded_output::borrowed_clone_view("").to_owned()
            } else {
                bounded_output::budgeted_clone("")
            };
            (first, second)
        })
    };
    for (limit, floor, sticky, nested) in [
        (0, 0, false, false),
        (len - 1, 0, false, false),
        (len, 0, false, false),
        (len + 1, 0, true, false),
        (len, 1, false, false),
        (len + 1, 1, false, false),
        (len, 0, false, true),
    ] {
        assert_eq!(
            run(true, limit, floor, sticky, nested),
            run(false, limit, floor, sticky, nested)
        );
    }
    // No owner means the original infallible/unfunded compatibility context.
    assert_eq!(
        bounded_output::borrowed_clone_view("unowned"),
        bounded_output::budgeted_clone("unowned")
    );
}

#[test]
fn actual_old_string_arc_overlap_and_full_drop_disappear_without_refunding_the_old_debit() {
    let index = hir::DeclarationIndex::default();
    for text in ["", "root.λ日本語", "root.long.identity.with.a.real.backing"] {
        let declaration = hir::Declaration {
            id: hir::DeclarationId::new(text),
            name: "same".into(),
            kind: hir::DeclarationKind::Function,
            identity_origin: hir::IdentityOrigin::Explicit,
            owner: None,
        };
        let backing = hir::DeclarationId::shared_backing_bytes(text.len()).unwrap();
        let old = with_limit_usage_refusal(text.len(), || {
            measure(|| {
                let value = before785::top_level_declaration(&index, &declaration);
                assert_eq!(value.as_str(), text);
                let live = current_metrics();
                assert_eq!(live.allocated, text.len() + backing);
                assert_eq!(live.live, backing);
                assert_eq!(live.highwater, text.len() + backing);
                drop(value);
            })
        });
        let current = with_limit_usage_refusal(text.len(), || {
            measure(|| {
                let value = top_level_declaration(&index, &declaration);
                assert_eq!(value.as_str(), text);
                assert_eq!(value.as_str().as_ptr(), declaration.id.as_str().as_ptr());
            })
        });
        assert_eq!(old.1, current.1);
        assert_eq!(old.2, current.2);
        assert_eq!(old.3, current.3);
        assert_eq!(old.2, text.len());
        let old_m = old.0 .1;
        let new_m = current.0 .1;
        assert!(!old_m.invalid && !new_m.invalid);
        assert_eq!(old_m.reallocations, 0);
        assert_eq!(old_m.allocated, old_m.freed);
        assert_eq!(old_m.live, 0);
        assert_eq!(new_m.allocations, 0);
        assert_eq!(new_m.reallocations, 0);
        assert_eq!(new_m.allocated, 0);
        assert_eq!(new_m.freed, 0);
        assert_eq!(new_m.live, 0);
        eprintln!(
            "785 root text_len={} old={old_m:?} borrowed={new_m:?} unchanged_paid={}",
            text.len(),
            old.2
        );
    }
}

type Caller = fn(
    bool,
    &[Program],
    &[(String, ResolvedModuleInput)],
) -> Result<BTreeMap<String, WorkspaceDeclarationFact>, Vec<Diagnostic>>;

fn ordinary(
    current: bool,
    programs: &[Program],
    modules: &[(String, ResolvedModuleInput)],
) -> Result<BTreeMap<String, WorkspaceDeclarationFact>, Vec<Diagnostic>> {
    let authored = index_authored(programs).unwrap();
    if current {
        reconstruct_workspace_declaration_facts_with_authority(modules, programs, &authored, None)
    } else {
        before_caller785::reconstruct_workspace_declaration_facts_with_authority(
            modules, programs, &authored, None,
        )
    }
}

fn compact(
    current: bool,
    programs: &[Program],
    modules: &[(String, ResolvedModuleInput)],
) -> Result<BTreeMap<String, WorkspaceDeclarationFact>, Vec<Diagnostic>> {
    let authored = index_authored(programs).unwrap();
    if current {
        let mut receiver = validation::WorkspaceValidationIndex::new(programs)?;
        for (module, resolved) in modules {
            receiver.record_module_authored(module, resolved, programs, &authored, None)?;
        }
        receiver.validate_stub_signatures_authored(programs, &authored, None)?;
        receiver.finish(programs)
    } else {
        let mut receiver = before_validation785::WorkspaceValidationIndex::new(programs)?;
        for (module, resolved) in modules {
            receiver.record_module_authored(module, resolved, programs, &authored, None)?;
        }
        receiver.validate_stub_signatures_authored(programs, &authored, None)?;
        receiver.finish(programs)
    }
}

#[test]
fn both_complete_callers_keep_original_results_all_six_diagnostic_fields_and_budget_order() {
    let (programs, modules) = declarations();
    for caller in [ordinary as Caller, compact as Caller] {
        // Warm only shared process metadata, using complete receivers.
        assert!(caller(false, &programs, &modules).is_ok());
        let success =
            with_limit_usage_refusal(MAX_BUILDER_BYTES, || caller(false, &programs, &modules));
        assert!(success.0.is_ok() && !success.1);
        for limit in [0, 1, success.2 / 2, success.2 - 1, success.2] {
            let old = with_limit_usage_refusal(limit, || caller(false, &programs, &modules));
            let current = with_limit_usage_refusal(limit, || caller(true, &programs, &modules));
            assert_eq!(current, old, "limit={limit}"); // Diagnostic Eq covers all six fields.
        }
        let mut stale = programs.clone();
        stale[0].functions[0].stable_id = "stale.main".to_owned();
        let old = caller(false, &stale, &modules);
        assert!(old.is_err());
        assert_eq!(caller(true, &stale, &modules), old);
    }
}

#[test]
fn original_empty_identity_g173_priority_survives_the_complete_caller_frontier() {
    let (programs, modules) = declarations();
    for caller in [ordinary as Caller, compact as Caller] {
        let exact =
            with_limit_usage_refusal(MAX_BUILDER_BYTES, || caller(false, &programs, &modules)).2;
        let mut found = false;
        // This is bounded by the actual small fixture receipt, not a guessed
        // external cap, and stops at the first original empty-root diagnostic.
        for limit in 0..=exact {
            let old = with_limit_usage_refusal(limit, || caller(false, &programs, &modules));
            if old.0.as_ref().err().is_some_and(|ds| ds.iter().any(|d|
                d.code == "SPX-G173" && d.message == "resolved workspace declaration has an unauthenticated synthetic or rogue root")) {
                assert_eq!(with_limit_usage_refusal(limit, || caller(true, &programs, &modules)), old);
                found = true;
                break;
            }
        }
        assert!(
            found,
            "the unchanged helper refusal must still reach its complete caller"
        );
    }
}

#[test]
fn whole_caller_physical_observers_include_results_receivers_and_complete_drop() {
    let (programs, modules) = declarations();
    for caller in [ordinary as Caller, compact as Caller] {
        assert!(caller(false, &programs, &modules).is_ok());
        let observe = |current| {
            with_limit_usage_refusal(MAX_BUILDER_BYTES, || {
                measure(|| {
                    let result = caller(current, &programs, &modules);
                    let ok = result.is_ok();
                    drop(result);
                    ok
                })
            })
        };
        for (a, b) in [(false, true), (true, false)] {
            let first = observe(a);
            let second = observe(b);
            let (old, current) = if a { (second, first) } else { (first, second) };
            assert!(old.0 .0 && current.0 .0 && !old.1 && !current.1);
            assert_eq!(old.2, current.2);
            assert_eq!(old.3, current.3);
            for m in [&old.0 .1, &current.0 .1] {
                assert!(!m.invalid && m.live == 0 && m.allocated == m.freed);
            }
            assert!(old.0 .1.allocations > current.0 .1.allocations);
            assert!(old.0 .1.allocated > current.0 .1.allocated);
            eprintln!(
                "785 complete caller old={:?} borrowed={:?} unchanged_paid={}",
                old.0 .1, current.0 .1, old.2
            );
        }
    }
}
