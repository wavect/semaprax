use super::{legacy, Index};
use crate::hir::{DeclarationId, ResolvedProgram, ResolvedType};
use crate::workspace_graph::static_text::primitive::capture_with_matching;
use crate::workspace_graph::{index_authored, WorkspaceSource};

fn source(count: usize) -> String {
    let mut source = String::from("module matching;");
    for n in (0..count).rev() {
        source.push_str(&format!(
            "@id(\"matching.f{n:04}\") fn f{n:04}(n:i64)->i64 {{n+1}}"
        ));
    }
    source.push_str("@id(\"matching.main\") fn main()->i64 {f0000(41)}");
    source
}
fn checked(count: usize) -> ResolvedProgram {
    let p = crate::check(&source(count), "matching.spx").unwrap();
    let resolved = crate::hir::resolve(&p).unwrap();
    crate::hir::validate(&resolved).unwrap();
    resolved
}
fn all(index: &Index<'_>, program: &ResolvedProgram, work: &mut usize) {
    for _ in 0..2 {
        for function in &program.functions {
            assert!(std::ptr::eq(
                index.find(function.id.as_str(), work).unwrap(),
                function
            ));
        }
    }
}
#[test]
fn wide_checked_function_inventory_matches_original_scans_at_exact_work_and_backing() {
    let program = checked(512);
    let mut extra_work = usize::MAX;
    let ((indexed_work, exact_bytes), refused, used) =
        crate::bounded_output::with_limit_usage(usize::MAX, || {
            let mut work = usize::MAX;
            let index = Index::new(&program.functions, &mut work).unwrap();
            all(&index, &program, &mut work);
            (
                usize::MAX - work,
                std::mem::size_of::<Index<'_>>()
                    + program.functions.len()
                        * std::mem::size_of::<&crate::hir::ResolvedFunction>(),
            )
        });
    assert!(!refused);
    assert_eq!(used, exact_bytes);
    let mut old_work = usize::MAX;
    for _ in 0..2 {
        for function in &program.functions {
            let old = legacy(&program.functions, function.id.as_str(), &mut old_work).unwrap();
            assert!(std::ptr::eq(old, function));
        }
    }
    assert!(indexed_work < usize::MAX - old_work);
    let ((), refused, used) = crate::bounded_output::with_limit_usage(exact_bytes, || {
        let mut work = indexed_work;
        let first = Index::new(&program.functions, &mut work).unwrap();
        all(&first, &program, &mut work);
        assert_eq!(work, 0);
    });
    assert!(!refused);
    assert_eq!(used, exact_bytes);
    let mut short_work = indexed_work - 1;
    let index = Index::new(&program.functions, &mut short_work).unwrap();
    let mut refused_work = false;
    for _ in 0..2 {
        for function in &program.functions {
            if index.find(function.id.as_str(), &mut short_work).is_err() {
                refused_work = true;
                break;
            }
        }
        if refused_work {
            break;
        }
    }
    assert!(refused_work);
    assert_eq!(short_work, 0);
    let (short, refused, _) = crate::bounded_output::with_limit_usage(exact_bytes - 1, || {
        Index::new(&program.functions, &mut extra_work)
    });
    assert!(short.is_err() && refused);
    // Earlier index backing remains charged while another index is constructed.
    let ((), refused, used) = crate::bounded_output::with_limit_usage(2 * exact_bytes, || {
        let first = Index::new(&program.functions, &mut extra_work).unwrap();
        let second = Index::new(&program.functions, &mut extra_work).unwrap();
        assert!(std::ptr::eq(
            first.find("matching.main", &mut extra_work).unwrap(),
            second.find("matching.main", &mut extra_work).unwrap()
        ));
    });
    assert!(!refused);
    assert_eq!(used, 2 * exact_bytes);
    let (short_overlap, refused, _) =
        crate::bounded_output::with_limit_usage(2 * exact_bytes - 1, || {
            let first = Index::new(&program.functions, &mut extra_work).unwrap();
            let result = Index::new(&program.functions, &mut extra_work);
            assert!(crate::bounded_output::active_refused());
            assert!(super::reserve(0).is_err());
            assert!(std::ptr::eq(
                first.find("matching.main", &mut extra_work).unwrap(),
                program.functions.last().unwrap()
            ));
            result.is_err()
        });
    assert!(short_overlap && refused);
}
#[test]
fn capture_keeps_every_current_source_origin_signature_and_body_comparison() {
    let parsed = crate::check(&source(32), "matching.spx").unwrap();
    let canonical = crate::format::canonical(&parsed);
    let programs = [crate::check(&canonical, "matching.spx").unwrap()];
    let resolved = crate::hir::resolve(&programs[0]).unwrap();
    let authored = index_authored(&programs).unwrap();
    let sources = [WorkspaceSource {
        path: "matching.spx".into(),
        source: canonical,
    }];
    let new = capture_with_matching::<false>(&programs[0], &resolved, &authored, Some(&sources))
        .unwrap()
        .unwrap();
    let old = capture_with_matching::<true>(&programs[0], &resolved, &authored, Some(&sources))
        .unwrap()
        .unwrap();
    assert_eq!(new.source_revision, old.source_revision);
    assert_eq!(new.functions.len(), old.functions.len());
    for (new, old) in new.functions.iter().zip(&old.functions) {
        assert_eq!(
            (&new.id, new.explicit, &new.name, &new.result, new.body),
            (&old.id, old.explicit, &old.name, &old.result, old.body)
        );
        assert_eq!(new.params.len(), old.params.len());
        for (new, old) in new.params.iter().zip(&old.params) {
            assert_eq!(
                (&new.name, new.ownership, &new.ty),
                (&old.name, old.ownership, &old.ty)
            );
        }
    }
    let mut drift = resolved.clone();
    drift.functions[0].body = drift.functions[1].body.clone();
    assert!(
        !crate::workspace_graph::static_text::primitive::body_matches(
            &new,
            &new.functions[0],
            &drift.functions[0]
        )
    );
    for mutation in 0..4 {
        let mut candidate = resolved.clone();
        match mutation {
            0 => candidate.functions[0].return_type = ResolvedType::Bool,
            1 => candidate.functions[0].params[0].ownership = crate::hir::OwnershipMode::Own,
            2 => candidate.functions[0].params[0].name = "foreign".into(),
            _ => candidate.functions[0].id = DeclarationId::new("absent"),
        }
        if mutation == 2 {
            // Both scans compare current signatures by type and ownership; a
            // drifted current parameter name is bound by the typed body digest.
            let drifted =
                capture_with_matching::<false>(&programs[0], &candidate, &authored, Some(&sources))
                    .unwrap()
                    .unwrap();
            let legacy =
                capture_with_matching::<true>(&programs[0], &candidate, &authored, Some(&sources))
                    .unwrap()
                    .unwrap();
            assert_eq!(drifted.functions[0].body, legacy.functions[0].body);
            assert_ne!(drifted.functions[0].body, new.functions[0].body);
            assert!(
                !crate::workspace_graph::static_text::primitive::body_matches(
                    &new,
                    &new.functions[0],
                    &candidate.functions[0]
                )
            );
            continue;
        }
        for legacy in [false, true] {
            let result = if legacy {
                capture_with_matching::<true>(&programs[0], &candidate, &authored, Some(&sources))
            } else {
                capture_with_matching::<false>(&programs[0], &candidate, &authored, Some(&sources))
            };
            assert_eq!(result.err().unwrap()[0].code, "SPX-G410");
        }
    }
    // An absent or foreign source namespace cannot be supplied by index membership.
    let empty = std::collections::BTreeMap::new();
    assert_eq!(
        capture_with_matching::<false>(&programs[0], &resolved, &empty, Some(&sources))
            .err()
            .unwrap()[0]
            .code,
        "SPX-G410"
    );
    let mut foreign = sources.clone();
    foreign[0].path = "foreign.spx".into();
    assert_eq!(
        capture_with_matching::<false>(&programs[0], &resolved, &authored, Some(&foreign))
            .err()
            .unwrap()[0]
            .code,
        "SPX-G410"
    );
}
#[test]
fn capture_reuses_matched_function_backing_at_the_original_reservation() {
    use crate::bounded_output::with_limit_usage_refusal;
    use crate::test_allocation_observer::measure;

    let source = source(4);
    let programs = [crate::check(&source, "matching.spx").unwrap()];
    let resolved = crate::hir::resolve(&programs[0]).unwrap();
    let authored = index_authored(&programs).unwrap();
    let sources = [WorkspaceSource {
        path: "matching.spx".into(),
        source,
    }];
    let observe = |owned, limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let inventory = if owned {
                    super::super::capture_with_identity::<false, true>(
                        &programs[0],
                        &resolved,
                        &authored,
                        Some(&sources),
                    )
                } else {
                    capture_with_matching::<false>(
                        &programs[0],
                        &resolved,
                        &authored,
                        Some(&sources),
                    )
                }?;
                let inventory = inventory.unwrap();
                for fact in &inventory.functions {
                    let current = resolved.functions.iter().find(|f| f.id == fact.id).unwrap();
                    let matched = current.id.shared_allocation_key().unwrap();
                    let captured = fact.id.shared_allocation_key().unwrap();
                    assert_eq!(matched != captured, owned);
                }
                drop(inventory);
                Ok::<_, Vec<crate::diagnostic::Diagnostic>>(())
            })
        })
    };
    let ((old, old_alloc), refused, exact, _) = observe(true, usize::MAX);
    old.unwrap();
    assert!(!refused && exact > 0);
    let ((selected, new_alloc), refused, charged, _) = observe(false, exact);
    selected.unwrap();
    assert!(!refused);
    assert_eq!(charged, exact);
    for alloc in [old_alloc, new_alloc] {
        assert!(!alloc.invalid);
        assert_eq!(alloc.live, 0);
        assert_eq!(alloc.allocated, alloc.freed);
    }
    let removed = resolved
        .functions
        .iter()
        .map(|f| f.id.as_str().len() + f.id.owned_allocation_bytes())
        .sum::<usize>();
    assert_eq!(old_alloc.allocated - new_alloc.allocated, removed);
    assert_eq!(
        old_alloc.allocations - new_alloc.allocations,
        2 * resolved.functions.len()
    );
    assert_eq!(old_alloc.reallocations, new_alloc.reallocations);
    for limit in [exact - 1, 0] {
        let ((before, _), old_refused, old_paid, old_first) = observe(true, limit);
        let ((current, _), refused, paid, first) = observe(false, limit);
        let before = before.unwrap_err();
        assert_eq!(before[0].code, "SPX-G171");
        assert_eq!(current.unwrap_err(), before);
        assert!(old_refused && refused && old_first.is_some());
        assert_eq!((paid, first), (old_paid, old_first));
    }
}

#[test]
fn duplicate_missing_unicode_and_reordered_members_keep_exact_pointer_lookup() {
    let mut program = checked(8);
    let mut extra_work = usize::MAX;
    program.functions[0].id = DeclarationId::new("é:member");
    program.functions[1].id = DeclarationId::new("é:member:later");
    program.functions.reverse();
    let index = Index::new(&program.functions, &mut extra_work).unwrap();
    for function in &program.functions {
        let old = legacy(&program.functions, function.id.as_str(), &mut extra_work).unwrap();
        assert!(std::ptr::eq(
            index.find(function.id.as_str(), &mut extra_work).unwrap(),
            old
        ));
    }
    assert!(index.find("missing", &mut extra_work).is_err());
    assert!(legacy(&program.functions, "missing", &mut extra_work).is_err());
    drop(index);
    program.functions[1].id = program.functions[0].id.clone();
    assert!(Index::new(&program.functions, &mut extra_work).is_err());
    assert!(legacy(
        &program.functions,
        program.functions[0].id.as_str(),
        &mut extra_work
    )
    .is_err());
}
