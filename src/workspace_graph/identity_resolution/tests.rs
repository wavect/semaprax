use super::*;
use crate::ast::program_source::SyntheticProgramView;
use crate::bounded_output::{with_limit_usage, with_limit_usage_refusal};
use crate::test_allocation_observer::{current_metrics, measure};
use std::mem::size_of;
mod legacy_receiver;

const SOURCE: &str = "module identity.workspace; @id(\"helper\") fn helper(value:i64)->i64 {value+1} @id(\"main\") fn main()->i64 {helper(1)}";

fn report_bytes() -> usize {
    // Self + one Diagnostic slot + fixed 48-byte message, on the actual ledger.
    size_of::<IdentityReservation>() + size_of::<Diagnostic>() + 48
}

#[test]
fn owned_and_borrowed_inputs_match_the_legacy_oracle_and_pay_one_report_each() {
    let program = crate::check(SOURCE, "identity-workspace.spx").unwrap();
    let oracle = crate::hir::resolve_with_function_reuse(&program, None, None, None, None).unwrap();
    let owned = Input::Owned(program.clone());
    let borrowed = Input::Borrowed(SyntheticProgramView::new(&program, Vec::new()).unwrap());
    // Each full resolution runs on its own equivalent, unchanged real
    // workspace builder ledger; one shared arbitrary 1 MiB ledger across both
    // refused the second one's independent-replay cursor.
    let before = super::acquisitions();
    let (first, failed, first_used) =
        with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
            resolve_module(None, &program.path, &owned, None).unwrap()
        });
    assert!(!failed);
    assert_eq!(super::acquisitions(), before + 1);
    let (second, failed, second_used) =
        with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
            resolve_module(None, &program.path, &borrowed, None).unwrap()
        });
    assert!(!failed);
    assert_eq!(super::acquisitions(), before + 2);
    for parts in [&first, &second] {
        assert_eq!(parts.0, oracle.0);
        assert_eq!((parts.2, parts.3), (0, oracle.1.static_text()));
        assert!(parts.1.is_empty());
    }
    // Each cold module acquired one fixed report plus its identity backings.
    assert!(first_used + second_used > 2 * report_bytes());
    assert!(first_used > report_bytes());
    assert!(second_used > report_bytes());
}

/// Additive observed control for the earlier shared arbitrary 1 MiB ledger:
/// each successful invocation retains complete old HIR/work/static parity.
/// Any actual physical refusal and all retries preserve its sticky first receipt.
/// Both results remain live until Drop inside the allocation observation.
#[test]
fn one_mib_ledger_preserves_first_cursor_refusal_and_sticky_retries() {
    crate::cleanup_plan::with_financed_test_oracles(
        oracle_owner_before::one_mib_ledger_preserves_first_cursor_refusal_and_sticky_retries,
    );
}

#[test]
fn complete_owned_and_borrowed_actual_payment_exact_short_and_sticky_receivers() {
    let program = crate::check(SOURCE, "identity-workspace.spx").unwrap();
    let owned = Input::Owned(program.clone());
    let borrowed = Input::Borrowed(SyntheticProgramView::new(&program, Vec::new()).unwrap());
    let (oracle, legacy_refused, _) =
        with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
            legacy_receiver::resolve_module(None, &program.path, &owned, None).unwrap()
        });
    assert!(!legacy_refused);
    let expected_reused = oracle.2;
    let expected_static = oracle.3;
    let expected_costs = &oracle.1;
    with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
        resolve_module(None, &program.path, &owned, None).unwrap()
    });
    for input in [&owned, &borrowed] {
        let observe = |limit, expect_refused| {
            with_limit_usage_refusal(limit, || {
                measure(|| {
                    let result = resolve_module(None, &program.path, input, None);
                    if expect_refused {
                        assert!(result.is_err() && crate::bounded_output::active_refused());
                        let remaining = crate::bounded_output::active_remaining();
                        assert!(!crate::bounded_output::reserve_active_required(0));
                        assert!(!crate::bounded_output::reserve_active_required(1));
                        let live = current_metrics();
                        let retry = resolve_module(None, &program.path, input, None);
                        let after = current_metrics();
                        assert_eq!(
                            (after.allocations, after.live),
                            (live.allocations, live.live)
                        );
                        assert!(matches!(
                            retry,
                            Err(ModuleFailure::Setup(Refused::Budget(_)))
                        ));
                        assert_eq!(remaining, crate::bounded_output::active_remaining());
                        drop(retry);
                    } else {
                        let parts = result.as_ref().unwrap();
                        assert_eq!(parts.0, oracle.0);
                        assert_eq!(&parts.1, expected_costs);
                        assert_eq!(parts.2, expected_reused);
                        assert_eq!(parts.3, expected_static);
                        assert!(!crate::bounded_output::active_refused());
                    }
                    drop(result);
                })
            })
        };
        let (((), allocation), failed, required, first) =
            observe(crate::workspace_graph::MAX_BUILDER_BYTES, false);
        assert!(!failed && first.is_none() && required > report_bytes());
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        assert!(allocation.allocated <= required && allocation.highwater <= required);
        let (((), exact), failed, used, first) = observe(required, false);
        assert!(!failed && first.is_none() && used == required);
        assert!(!exact.invalid && exact.live == 0);
        assert_eq!(exact.allocated, exact.freed);
        assert!(exact.allocated <= used && exact.highwater <= used);
        let (((), short), failed, used, first) = observe(required - 1, true);
        let first = first.expect("one-short selects a real original request");
        assert!(failed && first.requested > first.remaining && used < required);
        assert!(!short.invalid && short.live == 0);
        assert_eq!(short.allocated, short.freed);
        assert!(short.allocated <= used && short.highwater <= used);
    }
}

#[test]
fn zero_and_one_short_setup_is_inline_copy_with_no_allocation_and_sticky_budget() {
    let program = crate::check(SOURCE, "identity-workspace.spx").unwrap();
    let owned = Input::Owned(program.clone());
    for limit in [0, report_bytes() - 1] {
        let ((failure, observed), failed, used) = with_limit_usage(limit, || {
            measure(|| {
                resolve_module(None, &program.path, &owned, None)
                    .err()
                    .unwrap()
            })
        });
        assert!(matches!(failure, ModuleFailure::Setup(Refused::Budget(_))));
        assert!(failed);
        assert_eq!(used, 0);
        assert_eq!(observed.allocations, 0);
        assert_eq!(observed.live, 0);
    }
}

/// Independent Arc<str> backing: two control words plus text, word padded.
fn backing(text_bytes: usize) -> usize {
    let word = size_of::<usize>();
    (2 * word + text_bytes).div_ceil(word) * word
}

/// Observes the ACTUAL `resolve_module` acquisition (test-only hook, no
/// authority change) through refusal, sticky retry and returned-owner Drop.
/// Only the module-resolution boundary is claimed; the public outer
/// Setup-to-Vec graph/report layer stays explicitly unqualified.
fn assert_refusal(
    input: &Input<'_>,
    path: &str,
    limit: usize,
    charged: usize,
    requested: usize,
    remaining: usize,
) {
    // Warm process-static ordinary tables outside the observation.
    with_limit_usage(1 << 20, || resolve_module(None, path, input, None).unwrap());
    let before = super::acquisitions();
    let (((), observed), failed, used, receipt) = with_limit_usage_refusal(limit, || {
        measure(|| {
            let failure = resolve_module(None, path, input, None).err().unwrap();
            let ModuleFailure::Report(report) = failure else {
                panic!("actual identity refusal lost its published report")
            };
            // Exactly one acquisition; its original pointers are the returned ones.
            assert_eq!(super::acquisitions(), before + 1);
            let (report_pointer, text_pointer) = super::ACQUIRED.with(|a| a.get().1.unwrap());
            assert_eq!(report.as_ptr() as usize, report_pointer);
            assert_eq!(report[0].message.as_ptr() as usize, text_pointer);
            // Original layout: one slot and 48 text bytes, never regrown or cloned.
            assert_eq!(report.len(), 1);
            assert_eq!(report.capacity(), 1);
            assert_eq!(report[0].message.len(), 48);
            assert_eq!(report[0].message_owned_capacity(), 48);
            assert_eq!(report[0].code, "SPX-H006");
            // Sticky: zero and fitting retries refuse, and a fresh module
            // attempt stops at setup with the first receipt and no allocation.
            assert!(crate::bounded_output::active_refused());
            assert_eq!(crate::bounded_output::active_remaining(), Some(remaining));
            assert!(!crate::bounded_output::reserve_active_required(0));
            assert!(!crate::bounded_output::reserve_active_required(1));
            let live = current_metrics();
            let retry = resolve_module(None, path, input, None).err().unwrap();
            let ModuleFailure::Setup(Refused::Budget(retried)) = retry else {
                panic!("sticky retry acquired another report")
            };
            assert_eq!(
                (retried.requested, retried.remaining),
                (requested, remaining)
            );
            let after = current_metrics();
            assert_eq!(after.allocations, live.allocations);
            assert_eq!(after.live, live.live);
            assert_eq!(super::acquisitions(), before + 1);
            // The returned owner drops inside the observation.
            drop(report);
        })
    });
    assert!(failed);
    assert_eq!(used, charged);
    let receipt = receipt.unwrap();
    assert_eq!(receipt.requested, requested);
    assert_eq!(receipt.remaining, remaining);
    assert_eq!(receipt.floor, 0);
    assert!(!observed.invalid);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
}

fn refusal_inputs() -> (crate::ast::Program, usize) {
    let program = crate::check(SOURCE, "identity-workspace.spx").unwrap();
    // Third initial identity is the first function's explicit stable id.
    let function = backing(program.functions[0].stable_id.len());
    (program, function)
}

#[test]
fn first_identity_refusal_publishes_the_acquired_report_and_keeps_the_first_receipt() {
    let (program, _) = refusal_inputs();
    let owned = Input::Owned(program.clone());
    let borrowed = Input::Borrowed(SyntheticProgramView::new(&program, Vec::new()).unwrap());
    // The first initial identity is the prelude Option; 11 text bytes.
    let identity = backing(11);
    for input in [&owned, &borrowed] {
        let report = report_bytes();
        assert_refusal(input, &program.path, report, report, identity, 0);
        assert_refusal(
            input,
            &program.path,
            report + identity - 1,
            report,
            identity,
            identity - 1,
        );
    }
}

#[test]
fn later_identity_refusal_transfers_the_acquired_report_after_funded_identities() {
    let (program, function) = refusal_inputs();
    let owned = Input::Owned(program.clone());
    let borrowed = Input::Borrowed(SyntheticProgramView::new(&program, Vec::new()).unwrap());
    // Option then Result (both 11 bytes) precede the first function.
    let identity = backing(11);
    let report = report_bytes();
    for input in [&owned, &borrowed] {
        // Second identity one short, then at zero remaining.
        assert_refusal(
            input,
            &program.path,
            report + 2 * identity - 1,
            report + identity,
            identity,
            identity - 1,
        );
        assert_refusal(
            input,
            &program.path,
            report + identity,
            report + identity,
            identity,
            0,
        );
        // Exactly two funded: the fitting larger budget reaches the function.
        assert_refusal(
            input,
            &program.path,
            report + 2 * identity,
            report + 2 * identity,
            function,
            0,
        );
    }
}

#[test]
fn source_errors_stay_legacy_owned_after_one_report_acquisition() {
    let mut malformed = crate::parse(SOURCE, "identity-workspace.spx").unwrap();
    malformed.functions[1].stable_id = malformed.functions[0].stable_id.clone();
    let legacy = crate::hir::resolve_with_function_reuse(&malformed, None, None, None, None)
        .err()
        .unwrap();
    let input = Input::Owned(malformed);
    let (failure, failed, used) = with_limit_usage(1 << 20, || {
        resolve_module(None, "identity-workspace.spx", &input, None)
            .err()
            .unwrap()
    });
    let ModuleFailure::Report(report) = failure else {
        panic!("source error was routed as setup")
    };
    assert_eq!(report, legacy);
    assert!(!failed);
    // The fixed report is acquired first; the owned diagnostic itself is
    // legacy and equal to the unpaid oracle whatever stage raised it.
    assert!(used >= report_bytes());
}

fn linked_import_resolution<T>(operation: impl FnOnce() -> T) -> T {
    crate::vec_ops::with_authenticated_linked_source(|| {
        crate::box_ops::with_authenticated_linked_source(operation)
    })
}

#[test]
fn actual_selective_import_receiver_matches_fresh_owned_for_vec_and_box() {
    use crate::workspace_graph::{expected_projection, index_authored};
    use expected_projection::private_layout::registry::LayoutRegistry;
    // Exact transparent wrappers authenticate their canonical declaration name
    // as well as persistent identity and body. Keep those names in the linked
    // projection; imported aliases are a separate source-admission concern.
    let cases = [
        (
            include_str!("../../../std/collections/src/collections.spx"),
            "module receiver.vec; use function @id(\"std.collections.vec.with-capacity\") from std.collections as with_capacity; use function @id(\"std.collections.vec.push\") from std.collections as push; use function @id(\"std.collections.vec.get\") from std.collections as get; @id(\"receiver.helper\") fn helper()->i64 {1} @id(\"receiver.main\") fn main()->i64 {let empty=with_capacity<i64>(2usize); let values=push<i64>(empty,42); get<i64>(values,0usize)}",
        ),
        (
            include_str!("../../../std/mem/src/mem.spx"),
            "module receiver.box; use function @id(\"std.mem.box.new\") from std.mem as new; use function @id(\"std.mem.box.into-inner\") from std.mem as into_inner; @id(\"receiver.helper\") fn helper()->i64 {1} @id(\"receiver.main\") fn main()->i64 {into_inner<i64>(new<i64>(42))}",
        ),
    ];
    for (provider, caller) in cases {
        let previous = [
            crate::parse(caller, "caller.spx").unwrap(),
            crate::parse(provider, "provider.spx").unwrap(),
        ];
        let authored = index_authored(&previous).unwrap();
        let registry = LayoutRegistry::new(&previous, &authored);
        let old_key = expected_projection::synthetic_program_with_registry(
            &previous[0],
            &authored,
            &previous,
            Some(&registry),
        )
        .unwrap();
        let layout = registry.get(&previous[0], &authored, &previous).unwrap();
        let (old_hir, old_work) = linked_import_resolution(|| {
            crate::hir::resolve_with_function_reuse(
                &old_key,
                None,
                None,
                None,
                layout.presentation(&previous[0], &old_key).unwrap(),
            )
        })
        .unwrap();
        let pass = FrontendPass::paid_receiver_candidate(old_key, old_hir, old_work.costs());
        // The edited main misses the complete-key cache; an unchanged helper
        // remains a genuine selective candidate under the original environment.
        let edited = caller.replace("42", "43");
        let current = [
            crate::parse(&edited, "caller.spx").unwrap(),
            crate::parse(provider, "provider.spx").unwrap(),
        ];
        let authored = index_authored(&current).unwrap();
        let registry = LayoutRegistry::new(&current, &authored);
        let owned = Input::Owned(
            expected_projection::synthetic_program_with_registry(
                &current[0],
                &authored,
                &current,
                Some(&registry),
            )
            .unwrap(),
        );
        let borrowed =
            Input::construct(&current[0], &authored, &current, &registry, false).unwrap();
        assert!(borrowed.borrowed().is_some());
        let layout = registry.get(&current[0], &authored, &current).unwrap();
        let oracle = linked_import_resolution(|| {
            crate::hir::resolve_with_function_reuse(
                owned.owned().unwrap(),
                None,
                None,
                None,
                layout.presentation(&current[0], owned.source()).unwrap(),
            )
        })
        .unwrap();
        for input in [&owned, &borrowed] {
            // The public unauthenticated entry continues to reject foreign
            // retained wrappers: moving a caller scope does not widen it.
            let outside = crate::hir::resolve_source(
                input.source(),
                layout.presentation(&current[0], input.source()).unwrap(),
            );
            assert!(outside.is_err());
            let before = super::acquisitions();
            let (result, refused, _) =
                with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
                    resolve_module(
                        Some(&pass),
                        "caller.spx",
                        input,
                        layout.presentation(&current[0], input.source()).unwrap(),
                    )
                });
            assert!(!refused);
            let result = result.unwrap();
            assert_eq!(result.0, oracle.0);
            assert_eq!(result.3, oracle.1.static_text());
            assert!(result.2 > 0);
            assert_eq!(super::acquisitions(), before + 1);
            assert!(crate::hir::resolve_source(
                input.source(),
                layout.presentation(&current[0], input.source()).unwrap()
            )
            .is_err());
        }
        // A retained generic wrapper with the wrong persistent identity must
        // still undergo ordinary source verification, even with a warm cache.
        let mut hostile = owned.into_owned().unwrap();
        let wrapper = hostile
            .functions
            .iter_mut()
            .find(|function| !function.type_parameters.is_empty())
            .unwrap();
        wrapper.stable_id = "receiver.wrong.wrapper".to_owned();
        // An unreserved persistent identity describes an ordinary generic
        // forwarding function. Retain that entire old fixture as admission
        // parity, rather than treating an ordinary ID as reserved authority.
        let ordinary = Input::Owned(hostile.clone());
        let expected_ordinary = linked_import_resolution(|| {
            crate::hir::resolve_source(
                ordinary.source(),
                layout.presentation(&current[0], ordinary.source()).unwrap(),
            )
        })
        .unwrap();
        let (actual_ordinary, refused, _) =
            with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
                resolve_module(
                    Some(&pass),
                    "caller.spx",
                    &ordinary,
                    layout.presentation(&current[0], ordinary.source()).unwrap(),
                )
            });
        assert!(!refused);
        let actual_ordinary = actual_ordinary.unwrap();
        assert_eq!(actual_ordinary.0, expected_ordinary.0);
        assert_eq!(actual_ordinary.3, expected_ordinary.1.static_text());
        // Preserve the complete selected wrapper signature/body, but spoof a
        // different known reserved operation whose descriptor cannot match it.
        let wrapper = hostile
            .functions
            .iter_mut()
            .find(|function| !function.type_parameters.is_empty())
            .unwrap();
        wrapper.stable_id = if provider.contains("module std.collections") {
            "std.collections.vec.push"
        } else {
            "std.mem.box.into-inner"
        }
        .to_owned();
        let hostile = Input::Owned(hostile);
        let expected = linked_import_resolution(|| {
            crate::hir::resolve_source(
                hostile.source(),
                layout.presentation(&current[0], hostile.source()).unwrap(),
            )
        })
        .err()
        .expect("hostile wrapper must preserve source refusal");
        let (actual, refused, _) =
            with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
                resolve_module(
                    Some(&pass),
                    "caller.spx",
                    &hostile,
                    layout.presentation(&current[0], hostile.source()).unwrap(),
                )
            });
        assert!(!refused);
        let ModuleFailure::Report(actual) = actual.unwrap_err() else {
            panic!("wrong identity must preserve semantic report");
        };
        assert_eq!(actual, expected);
    }
}

#[path = "tests/refusal_reference.rs"]
mod refusal_reference;

#[test]
fn direct_paid_inner_receivers_preserve_complete_independent_legacy_function_work() {
    let program = crate::check(SOURCE, "identity-workspace.spx").unwrap();
    let owned = Input::Owned(program.clone());
    let borrowed = Input::Borrowed(SyntheticProgramView::new(&program, Vec::new()).unwrap());
    // FunctionResolutionWork::costs records active cumulative builder debits.
    // Keep the original unbounded receiver as a semantic control, and acquire
    // the same application ledger before comparing its physical work report.
    let unbounded =
        crate::hir::resolve_with_function_reuse(&program, None, None, None, None).unwrap();
    let (oracle, failed, _) = with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
        crate::hir::resolve_with_function_reuse(&program, None, None, None, None).unwrap()
    });
    assert!(!failed);
    assert_eq!(oracle.0, unbounded.0);
    assert_eq!(oracle.1.reused(), unbounded.1.reused());
    assert_eq!(oracle.1.static_text(), unbounded.1.static_text());
    assert!(unbounded.1.costs().values().all(|cost| *cost == 0));
    let expected_reused = oracle.1.reused();
    let expected_static = oracle.1.static_text();
    let expected_costs = oracle.1.costs();
    for input in [&owned, &borrowed] {
        let (actual, failed, _) =
            with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
                let mut owner = IdentityReservation::acquire().unwrap();
                crate::vec_ops::with_authenticated_linked_source(|| {
                    crate::box_ops::with_authenticated_linked_source(|| {
                        input.resolve_paid(None, &mut owner).unwrap()
                    })
                })
            });
        assert!(!failed);
        assert_eq!(actual.0, oracle.0);
        assert_eq!(actual.1.reused(), expected_reused);
        assert_eq!(actual.1.static_text(), expected_static);
        assert_eq!(actual.1.costs(), expected_costs);
    }
}

mod oracle_owner_before;
