//! Source-authored only: the parent batch must execute these owning controls.
use super::super::{before_registry, LayoutClosure, WORK};
use super::before_retained::LayoutRegistry;
use crate::bounded_output::with_limit_usage_refusal;
use crate::workspace_graph::index_authored;

const CALLER: &str = include_str!("../function_caller.spx");
const PROVIDER: &str = include_str!("../function_provider.spx");
fn sources(count: usize) -> Vec<crate::ast::Program> {
    let mut programs = (0..count)
        .map(|n| {
            let source = CALLER
                .replace("function_layout.app", &format!("function_layout.app{n}"))
                .replace("app.main", &format!("app{n}.main"))
                .replace("report", &format!("_spx_layout_{}", n * 97 + 7));
            crate::parse(&source, format!("caller{n}.spx")).unwrap()
        })
        .collect::<Vec<_>>();
    programs.push(crate::parse(PROVIDER, "provider.spx").unwrap());
    programs
}
fn same(old: &LayoutClosure<'_>, current: &LayoutClosure<'_>) {
    assert_eq!(old.entries.len(), current.entries.len());
    for (a, b) in old.entries.iter().zip(&current.entries) {
        assert_eq!(
            (a.module, a.declaration, &a.alias),
            (b.module, b.declaration, &b.alias)
        );
    }
}

#[test]
fn distinct_census_callers_keep_complete_old_layouts_and_reset_paid_scratch() {
    let programs = sources(3);
    let authored = index_authored(&programs).unwrap();
    let registry = LayoutRegistry::new(&programs, &authored);
    let old = before_registry::LayoutRegistry::new(&programs, &authored);
    let mut pointers = None;
    for index in [0, 1, 0, 2, 1] {
        let actual = registry
            .get(&programs[index], &authored, &programs)
            .unwrap();
        let expected = old.get(&programs[index], &authored, &programs).unwrap();
        same(&expected, &actual);
        let state = registry.census.borrow().as_ref().unwrap().state();
        assert_eq!(
            (state.1, state.2, state.3, state.4),
            ([513; 3], WORK, Some(0), true)
        );
        if let Some(pointers) = pointers {
            assert_eq!(state.0, pointers);
        }
        pointers = Some(state.0);
        let target = authored.get("make").unwrap();
        let ty = &target.function.unwrap().return_type;
        assert_eq!(
            expected
                .signature_owner(&programs[index], target, ty, &authored, &programs)
                .unwrap(),
            actual
                .signature_owner(&programs[index], target, ty, &authored, &programs)
                .unwrap()
        );
    }
    assert_eq!(registry.derivations.get(), 3);
    let mut scratch = None;
    for (ordinal, caller) in programs[..3].iter().enumerate() {
        let mut old_work = WORK;
        let mut current_work = WORK;
        let expected =
            LayoutClosure::derive_before_scratch(caller, &authored, &programs, &mut old_work, None)
                .unwrap();
        let actual = LayoutClosure::derive_with_scratch(
            caller,
            &authored,
            &programs,
            &mut current_work,
            None,
            &mut scratch,
        )
        .unwrap();
        same(&expected, &actual);
        eprintln!(
            "833 caller={ordinal} old_declared_work={} current_declared_work={}",
            WORK - old_work,
            WORK - current_work
        );
        assert_eq!(
            old_work - current_work,
            if ordinal == 0 { 7 } else { 6 },
            "six genuine selection/reset operations plus one prepaid teardown per acquired buffer generation"
        );
    }
}

#[test]
fn census_scratch_release_retains_receipt_and_late_cold_caller_reacquires() {
    let programs = sources(3);
    let authored = index_authored(&programs).unwrap();
    let registry = LayoutRegistry::new(&programs, &authored);
    drop(registry.get(&programs[0], &authored, &programs).unwrap());
    let receipt = registry.receipt();
    assert!(registry.census.borrow().is_some());
    registry.release_scratch();
    assert!(registry.census.borrow().is_none());
    assert_eq!(registry.receipt(), receipt);
    drop(registry.get(&programs[0], &authored, &programs).unwrap());
    assert!(
        registry.census.borrow().is_none(),
        "held hit does not reacquire"
    );
    let (_, overflowed, consumed, _) = with_limit_usage_refusal(1 << 24, || {
        drop(registry.get(&programs[1], &authored, &programs).unwrap());
    });
    assert!(!overflowed);
    assert!(registry.census.borrow().is_some());
    assert_eq!(registry.receipt(), receipt + consumed);
    let retained = registry.receipt();
    registry.release_scratch();
    assert_eq!(
        registry.receipt(),
        retained,
        "no refund after second release"
    );
    assert_eq!(registry.derivations.get(), 2);
}

#[test]
fn complete_old_and_reused_census_keep_malformed_diagnostics_and_poisoned_drop() {
    let mut programs = sources(2);
    programs[1].module = "_spx_layout_18446744073709551615".to_owned();
    let authored = index_authored(&programs).unwrap();
    let registry = LayoutRegistry::new(&programs, &authored);
    let old = before_registry::LayoutRegistry::new(&programs, &authored);
    drop(registry.get(&programs[0], &authored, &programs).unwrap());
    drop(old.get(&programs[0], &authored, &programs).unwrap());
    let expected = old.get(&programs[1], &authored, &programs).err().unwrap();
    let actual = registry
        .get(&programs[1], &authored, &programs)
        .err()
        .unwrap();
    assert_eq!(actual, expected, "all six Diagnostic fields and order");
    assert!(registry.is_failed());
    assert!(registry.census.borrow().is_none());
    let receipt = registry.receipt();
    registry.set_remaining(WORK);
    let (result, _, consumed, _) =
        with_limit_usage_refusal(0, || registry.get(&programs[0], &authored, &programs));
    assert!(result.is_err());
    assert_eq!(consumed, 0);
    assert_eq!(registry.receipt(), receipt);
}

#[test]
fn census_registry_exact_short_zero_work_and_byte_limits_keep_sticky_refusal() {
    let programs = sources(2);
    let authored = index_authored(&programs).unwrap();
    let run = |bytes, work| {
        let registry = LayoutRegistry::new(&programs, &authored);
        registry.remaining.set(work);
        let (answer, overflowed, consumed, refusal) = with_limit_usage_refusal(bytes, || {
            for caller in &programs[..2] {
                drop(registry.get(caller, &authored, &programs)?);
            }
            Ok::<_, Vec<crate::diagnostic::Diagnostic>>(())
        });
        let work_used = work - registry.work_remaining();
        if answer.is_err() {
            assert!(registry.is_failed());
            assert!(registry.census.borrow().is_none());
            registry.set_remaining(WORK);
            assert!(registry.get(&programs[0], &authored, &programs).is_err());
        }
        assert_eq!(registry.receipt(), consumed);
        (answer, overflowed, consumed, refusal, work_used)
    };
    let ample = run(1 << 24, WORK);
    assert!(ample.0.is_ok());
    let exact = run(ample.2, ample.4);
    assert!(exact.0.is_ok());
    assert_eq!(exact.2, ample.2);
    for (bytes, work) in [
        (0, WORK),
        (ample.2 - 1, WORK),
        (1 << 24, 0),
        (1 << 24, ample.4 - 1),
    ] {
        assert!(run(bytes, work).0.is_err());
    }
}

#[test]
fn census_scratch_preserves_nested_floors_foreign_and_restored_owner_refusal() {
    let programs = sources(2);
    let authored = index_authored(&programs).unwrap();
    // Real distinct child owners must pay the original census permission
    // branch before any retained backing is borrowed, even if pointers match.
    let mut scratch = None;
    let mut work = WORK;
    super::super::names::first_reusing(&programs[0], &mut scratch, &mut work).unwrap();
    let old = with_limit_usage_refusal(1 << 24, || super::super::names::first(&programs[1]));
    let needed = old.2;
    assert!(old.0.is_ok());
    for limit in [0, needed - 1, needed] {
        let expected = with_limit_usage_refusal(limit, || super::super::names::first(&programs[1]));
        let current = with_limit_usage_refusal(limit, || {
            super::super::names::first_reusing(&programs[1], &mut scratch, &mut work)
        });
        assert_eq!(
            current, expected,
            "complete old prepayment/floor/refusal branch in each fresh child"
        );
    }
    drop(scratch); // Retired owners are gone; empty vectors only deallocate.
    let registry = LayoutRegistry::new(&programs, &authored);
    let outer = with_limit_usage_refusal(1 << 24, || {
        assert!(crate::bounded_output::set_active_floor(128));
        drop(registry.get(&programs[0], &authored, &programs).unwrap());
        let retained = registry.receipt();
        let inner =
            with_limit_usage_refusal(0, || registry.get(&programs[1], &authored, &programs));
        assert!(inner.0.is_err());
        assert!(registry.is_failed());
        assert!(registry.census.borrow().is_none());
        assert!(registry.receipt() >= retained);
        let remaining = crate::bounded_output::active_remaining().unwrap();
        assert!(
            !crate::bounded_output::reserve_active(remaining - 127),
            "the original enclosing floor is still installed"
        );
        assert_eq!(crate::bounded_output::active_remaining(), Some(remaining));
        crate::bounded_output::clear_active_floor();
    });
    assert!(outer.2 > 0);
    assert!(
        registry.get(&programs[0], &authored, &programs).is_err(),
        "restoring the parent cannot revive a failed registry"
    );
    for foreign_kind in 0..3 {
        let candidate = LayoutRegistry::new(&programs, &authored);
        drop(candidate.get(&programs[0], &authored, &programs).unwrap());
        let foreign_caller = programs[1].clone();
        let foreign_programs = programs.clone();
        let foreign_authored = index_authored(&foreign_programs).unwrap();
        let result = match foreign_kind {
            0 => candidate.get(&foreign_caller, &authored, &programs),
            1 => candidate.get(&programs[1], &authored, &foreign_programs),
            _ => candidate.get(&programs[1], &foreign_authored, &programs),
        };
        assert!(result.is_err());
        drop(result);
        assert!(candidate.census.borrow().is_none());
        assert!(candidate.get(&programs[0], &authored, &programs).is_err());
    }
}

#[test]
fn distinct_census_registry_observes_actual_allocated_reserved_peak_and_complete_drop() {
    for count in [0, 1, 4] {
        let programs = sources(count);
        let authored = index_authored(&programs).unwrap();
        // Source/index setup and observer setup are identical and outside both
        // measured routes. Each route includes its actual registry through Drop.
        let old = with_limit_usage_refusal(1 << 24, || {
            crate::test_allocation_observer::measure(|| {
                let registry = before_registry::LayoutRegistry::new(&programs, &authored);
                for caller in &programs[..count] {
                    drop(registry.get(caller, &authored, &programs).unwrap());
                }
                if count == 0 {
                    assert!(registry
                        .get(&programs[0], &authored, &programs)
                        .unwrap()
                        .is_empty());
                }
                let receipt = registry.receipt();
                drop(registry);
                receipt
            })
        });
        let current = with_limit_usage_refusal(1 << 24, || {
            crate::test_allocation_observer::measure(|| {
                let registry = LayoutRegistry::new(&programs, &authored);
                for caller in &programs[..count] {
                    drop(registry.get(caller, &authored, &programs).unwrap());
                }
                registry.release_scratch();
                if count == 0 {
                    assert!(registry
                        .get(&programs[0], &authored, &programs)
                        .unwrap()
                        .is_empty());
                }
                let receipt = registry.receipt();
                drop(registry);
                receipt
            })
        });
        assert!(!old.1 && !current.1);
        assert_eq!(old.0 .0, old.2);
        assert_eq!(current.0 .0, current.2);
        for (metrics, reserved) in [(old.0 .1, old.2), (current.0 .1, current.2)] {
            assert!(!metrics.invalid);
            assert_eq!(metrics.live, 0);
            assert_eq!(metrics.allocated, metrics.freed);
            assert!(
                metrics.allocated <= reserved,
                "all actual requested backing remains paid"
            );
        }
        if count == 0 {
            assert_eq!(old.0 .1, current.0 .1);
            assert_eq!(old.2, current.2);
        }
        if count > 1 {
            assert!(current.0 .1.allocations < old.0 .1.allocations);
            assert!(current.0 .1.allocated < old.0 .1.allocated);
            assert!(current.2 >= old.2, "every distinct child still pays full backing and the larger registry header; no reservation-saving claim");
        }
        eprintln!("833 callers={count} old={old:?} current={current:?}");
    }
}

#[test]
fn complete_old_registry_census_derivation_and_current_scratch_sources_remain_bound() {
    use sha2::{Digest, Sha256};
    let packet: serde_json::Value =
        serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(
            crate::source_reflows_merge::ReceiverFixture::RegistryScratch,
        ))
        .unwrap();
    for row in packet["owners"].as_array().unwrap() {
        let (mut current, original) = match row["source"].as_str().unwrap() {
            "src/workspace_graph.rs" => (include_str!("../../../../workspace_graph.rs").to_owned(), include_str!("../../../../../docs/evidence/registry-census-scratch-833/original/workspace_graph.rs")),
            "src/workspace_graph/expected_projection/private_layout.rs" => (include_str!("../../private_layout.rs").to_owned(), include_str!("../../../../../docs/evidence/registry-census-scratch-833/original/private_layout.rs")),
            "src/workspace_graph/expected_projection/private_layout/names.rs" => (include_str!("../names.rs").to_owned(), include_str!("../../../../../docs/evidence/registry-census-scratch-833/original/names.rs")),
            "src/workspace_graph/expected_projection/private_layout/registry.rs" => (include_str!("../registry.rs").to_owned(), include_str!("../../../../../docs/evidence/registry-census-scratch-833/original/registry.rs")),
            "tests/workspace/semantic_graph/source_inventory.rs" => (include_str!("../../../../../tests/workspace/semantic_graph/source_inventory.rs").to_owned(), include_str!("../../../../../docs/evidence/registry-census-scratch-833/original/source_inventory.rs")),
            _ => panic!("unexpected full source owner"),
        };
        current = crate::source_reflows_merge::consumer_input(
            "registry833",
            row["source"].as_str().unwrap(),
            &current,
        );
        for edit in row["edits"].as_array().unwrap() {
            let new = edit["new"].as_str().unwrap();
            assert_eq!(current.matches(new).count(), 1);
            current = current.replacen(new, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(current, original);
    }
    let old_registry = include_str!(
        "../../../../../docs/evidence/registry-census-scratch-833/original/registry.rs"
    );
    assert_eq!(
        crate::source_reflows_merge::receiver_child_entry(
            "src/workspace_graph/expected_projection/private_layout/before_registry.rs",
            include_str!("../before_registry.rs"),
        ),
        old_registry
            .replace(
                "LayoutClosure::derive_with_work(",
                "LayoutClosure::derive_before_scratch("
            )
            .replace(
                "#[cfg(test)]\n#[path = \"registry/tests.rs\"]\nmod tests;\n",
                ""
            )
    );
    let old_names =
        include_str!("../../../../../docs/evidence/registry-census-scratch-833/original/names.rs");
    let old_first = old_names
        .split("pub(super) fn first(")
        .nth(1)
        .unwrap()
        .split("\nfn refusal()")
        .next()
        .unwrap();
    assert!(include_str!("../names.rs").contains(&format!("pub(super) fn first({old_first}")));
    let old_layout = include_str!(
        "../../../../../docs/evidence/registry-census-scratch-833/original/private_layout.rs"
    );
    let old_derive = old_layout
        .split("    fn derive_with_work(")
        .nth(1)
        .unwrap()
        .split("    pub(super) fn add_costs(")
        .next()
        .unwrap();
    assert!(include_str!("../../private_layout.rs")
        .contains(&format!("    fn derive_before_scratch({old_derive}")));
    for (path, bytes) in [
        (
            "src/workspace_graph/expected_projection/private_layout/before_registry.rs",
            include_str!("../before_registry.rs"),
        ),
        (
            "src/workspace_graph/expected_projection/private_layout/registry/scratch_tests.rs",
            include_str!("scratch_tests.rs"),
        ),
    ] {
        let row = packet["children"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["source"] == path)
            .unwrap();
        let projected = crate::source_reflows_merge::consumer_child("registry833", path, bytes);
        let bytes = projected.as_str();
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(bytes.as_bytes()))
            ),
            row["sha256"].as_str().unwrap()
        );
    }
}
