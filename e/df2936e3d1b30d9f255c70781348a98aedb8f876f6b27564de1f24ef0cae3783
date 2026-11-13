//! Direct physical controls for the lexical publisher and its unchanged helper.
mod paid_many {
    pub(super) use super::super::*;
}
use super::super::{own_weighted, physical_clone, Rc, SharedWeighted, SmallWeighted};
use super::*;
use crate::test_allocation_observer::{current_metrics, measure};

fn fixture() -> ResolvedFunction {
    super::super::super::lexical_plan_tests::fixture(super::super::super::lexical_plan_tests::WHILE)
        .1
}
fn publish(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    old: bool,
) -> Result<SharedWeighted, Diagnostic> {
    let mut flat = Vec::new();
    budget.reserve_vec(function, &mut flat, 3, "lexical test complete flat backing")?;
    flat.extend([(1, 0), (17, 7), (91, u64::MAX)]);
    let targets = Targets::PaidMany(flat);
    if old {
        budget.reserve_bytes(
            function,
            FINISH_RECEIVING,
            "factored composed flat finish receivers",
        )?;
        let Targets::PaidMany(flat) = targets else {
            unreachable!()
        };
        own_weighted(function, budget, flat)
    } else {
        Ok(paid_many::finish!(function, budget, targets))
    }
}

#[test]
fn lexical_finish_real_selected_and_builder_owners_reduce_only_removed_call_shapes() {
    let function = fixture();
    assert_eq!(
        LEXICAL_FINISH_TOTAL - LEXICAL_FINISH_RECEIVING,
        physical_clone::rc_backing_bytes::<SmallWeighted>()
    );
    let old_controls = FINISH_RECEIVING
        + 2 * size_of::<SmallWeighted>()
        + size_of::<Result<SharedWeighted, Diagnostic>>()
        + size_of::<Result<(), Diagnostic>>();
    assert!(LEXICAL_FINISH_RECEIVING < old_controls);
    for selected in [false, true] {
        let observe = |old, limit| {
            let entries = PUBLICATIONS.with(|value| value.get());
            let ((success, charged), metrics) = measure(|| {
                let execute = || {
                    let mut budget = if selected {
                        ReplayBudget::with_physical(&function, limit)?
                    } else {
                        ReplayBudget::new()
                    };
                    let remaining = budget.remaining;
                    let result = publish(&function, &mut budget, old);
                    let success = result.is_ok();
                    if success {
                        assert_eq!(
                            remaining, budget.remaining,
                            "publication performs no logical work"
                        );
                    }
                    if let Ok(shared) = &result {
                        assert_eq!(
                            shared.as_ref().as_slice(),
                            &[(1, 0), (17, 7), (91, u64::MAX)]
                        );
                    } else {
                        assert_eq!(result.as_ref().unwrap_err().code, "SPX-H006");
                    }
                    let charged = if selected {
                        budget.physical.as_ref().unwrap().charged()
                    } else {
                        0
                    };
                    let stopped = current_metrics().allocated;
                    drop(result);
                    drop(budget);
                    assert_eq!(
                        current_metrics().allocated,
                        stopped,
                        "Drop allocates nothing"
                    );
                    Ok::<_, Diagnostic>((success, charged))
                };
                if selected {
                    execute().unwrap_or((false, 0))
                } else {
                    let (result, _, debit, _) =
                        crate::bounded_output::with_limit_usage_refusal(limit, execute);
                    let (success, _) = result.unwrap_or((false, 0));
                    (success, debit)
                }
            });
            assert_eq!(
                PUBLICATIONS.with(|value| value.get()) - entries,
                usize::from(success && !old),
                "combined admission precedes every Vec extraction and Rc publication"
            );
            assert!(!metrics.invalid);
            assert_eq!(metrics.live, 0);
            assert_eq!(metrics.allocated, metrics.freed);
            if success {
                assert!(metrics.allocated <= charged && metrics.highwater <= charged);
            }
            (success, charged, metrics)
        };
        let old = observe(true, 1 << 20);
        let current = observe(false, 1 << 20);
        assert!(old.0 && current.0);
        assert_eq!(old.1 - current.1, old_controls - LEXICAL_FINISH_RECEIVING);
        assert_eq!(
            old.2.allocated, current.2.allocated,
            "same Vec and Rc backing"
        );
        for limit in 0..=current.1 {
            assert_eq!(observe(false, limit).0, limit == current.1);
        }
        eprintln!(
            "lexical finish selected={selected} old/current={}/{} controls saved={}",
            old.1,
            current.1,
            old_controls - LEXICAL_FINISH_RECEIVING
        );
    }
}

#[test]
fn lexical_finish_oracle_retains_same_work_and_unobserved_helper_stays_independent() {
    use crate::cleanup_plan::replay::allocation::OracleScope;
    let function = fixture();
    let mut prices = Vec::new();
    for old in [true, false] {
        let scope = OracleScope::open_with_work(1 << 20, 1 << 20).unwrap();
        let (price, metrics) = measure(|| {
            let mut budget = ReplayBudget::with_skeleton_limit_oracle(0, scope.lease());
            let before = scope.receipt();
            let shared = publish(&function, &mut budget, old).unwrap();
            let after = scope.receipt();
            assert_eq!(before.work, after.work);
            let price = after.charged - before.charged;
            drop(shared);
            drop(budget);
            price
        });
        prices.push(price);
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
    }
    assert!(prices[1] < prices[0]);
    let mut budget = ReplayBudget::unobserved();
    let shared = publish(&function, &mut budget, true).unwrap();
    assert_eq!(
        shared.as_ref().as_slice(),
        &[(1, 0), (17, 7), (91, u64::MAX)]
    );
}

fn denied_publication(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<SharedWeighted, Diagnostic> {
    // No backing allocation: admission must fail before extracting this header.
    let targets = Targets::PaidMany(Vec::new());
    Ok(paid_many::finish!(function, budget, targets))
}
#[test]
fn lexical_finish_refusal_is_sticky_and_keeps_selected_six_field_diagnostic() {
    let function = fixture();
    let probe = ReplayBudget::with_physical(&function, 1 << 20).unwrap();
    let setup = probe.physical.as_ref().unwrap().charged();
    drop(probe);
    let mut budget =
        ReplayBudget::with_physical(&function, setup + LEXICAL_FINISH_TOTAL - 1).unwrap();
    let selected = denied_publication(&function, &mut budget).unwrap_err();
    let expected = Diagnostic::io("SPX-H006", format!(
        "cleanup plan for function `{}` failed independent replay: cleanup replay physical reservation exhausted during factored lexical flat publication receivers", function.id));
    assert_eq!(selected, expected);
    let charged = budget.physical.as_ref().unwrap().charged();
    let retry = denied_publication(&function, &mut budget).unwrap_err();
    assert_eq!(retry.code, "SPX-H006");
    assert_eq!(budget.remaining, 0);
    assert_eq!(budget.physical.as_ref().unwrap().charged(), charged);
    assert_eq!(
        selected, expected,
        "all six originally selected fields remain live through retry"
    );
}

#[test]
fn lexical_finish_complete_before_sources_and_old_publication_paths_are_preserved() {
    // Only the two historical whole-source expressions acquire their exact
    // complete lexical predecessor. Every original assertion below is unchanged.
    macro_rules! include_str {
        ("../../canonical_shared.rs") => { &super::source::whole_source_value("src/cleanup_plan/replay/factored/canonical_shared.rs", 64422, "33e45de2e06bd0a251c7e6c22c0b953b4d32991ca488becdfccfe29209380111", ::std::include_str!("../../canonical_shared.rs")) };
        ("../borrowed_labels.rs") => { &super::source::whole_source_value("src/cleanup_plan/replay/factored/canonical_shared/borrowed_labels.rs", 28835, "f4fd89c5c28b47d826968d935f51dac66a49e8fd859bfa519e98013913a3923a", ::std::include_str!("../borrowed_labels.rs")) };
        ($($path:tt)*) => { ::std::include_str!($($path)*) };
    }
    use sha2::{Digest, Sha256};
    let packet: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/paid-many-lexical-finish-20261017/before-inputs.json"
    )))
    .unwrap();
    assert_eq!(packet["base"], "761351d847a3bd4d7d86bb81206aa1c89cdfb620");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for row in packet["subjects"].as_array().unwrap() {
        let path = row["path"].as_str().unwrap();
        let before = std::fs::read(
            root.join("docs/evidence/paid-many-lexical-finish-20261017/before")
                .join(path),
        )
        .unwrap();
        assert_eq!(before.len() as u64, row["before_length"]);
        assert_eq!(
            format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(&before))),
            row["before_sha256"]
        );
    }
    let before = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/paid-many-lexical-finish-20261017/before/src/cleanup_plan/replay/factored/canonical_shared.rs"));
    let current = include_str!("../../canonical_shared.rs");
    let mut expected = before.to_owned();
    for indentation in [24, 20] {
        let i = " ".repeat(indentation);
        let old = format!("{i}paid_many::Targets::PaidMany(_) => {{\n{i}    budget.reserve_bytes(function, paid_many::FINISH_RECEIVING,\n{i}        \"factored composed flat finish receivers\")?;\n{i}    let paid_many::Targets::PaidMany(flat) = targets else {{ unreachable!(\"paid flat finish\") }};\n{i}    own_weighted(function, budget, flat)?\n{i}}}");
        let new = format!("{i}paid_many::Targets::PaidMany(_) => match budget {{\n{i}    ReplayBudget {{ payer: AllocationPayer::LegacyUnobserved, .. }} => {{\n{i}        budget.reserve_bytes(function, paid_many::FINISH_RECEIVING,\n{i}            \"factored composed flat finish receivers\")?;\n{i}        let paid_many::Targets::PaidMany(flat) = targets else {{ unreachable!(\"paid flat finish\") }};\n{i}        own_weighted(function, budget, flat)?\n{i}    }}\n{i}    _ => paid_many::finish!(function, budget, targets),\n{i}}},");
        assert_eq!(expected.matches(&old).count(), 1);
        expected = expected.replacen(&old, &new, 1);
    }
    assert_eq!(
        current, &expected,
        "only the two complete finalizer arms change in the entire canonical source"
    );
    let start = before.find("fn own_weighted(").unwrap();
    let end = before[start..].find("// Vec's FromIterator").unwrap() + start;
    assert!(
        current.contains(&before[start..end]),
        "complete original owned publication helper"
    );
    assert_eq!(current.matches("paid_many::FINISH_RECEIVING").count(), 2);
    assert_eq!(
        current
            .matches("own_weighted(function, budget, flat)?")
            .count(),
        2
    );
    assert_eq!(
        current
            .matches("paid_many::finish!(function, budget, targets)")
            .count(),
        2
    );
    let before = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/paid-many-lexical-finish-20261017/before/src/cleanup_plan/replay/factored/canonical_shared/borrowed_labels.rs"));
    let current = include_str!("../borrowed_labels.rs");
    let end = before.find("pub(super) fn ordinary_successors(").unwrap();
    assert!(
        current.starts_with(&before[..end]),
        "complete original Directory, traversal and tests"
    );
    assert!(current
        .contains("lexical_finish::successors(function, budget, automaton, closed, accepted)"));
    let current = include_str!("../borrowed_labels/lexical_finish.rs");
    let start = before.find("    fn finish(self,").unwrap();
    let end = before[start..]
        .find("\n}\npub(super) fn successors")
        .unwrap()
        + start;
    let finish = before[start..end].replacen("fn finish(self,", "fn finish_lexical(self,", 1);
    let old_arm = "                Some(paid_many::Targets::PaidMany(flat)) => {\n                    budget.reserve_bytes(function, paid_many::FINISH_RECEIVING, \"factored composed flat finish receivers\")?;\n                    own_weighted(function, budget, flat)?\n                }";
    let new_arm = "                Some(paid_many::Targets::PaidMany(_)) => {\n                    paid_many::finish!(function, budget, targets, optional)\n                }";
    assert_eq!(finish.matches(old_arm).count(), 1);
    let finish = finish.replacen(old_arm, new_arm, 1);
    let start = before.find("pub(super) fn successors<C: Closed>").unwrap();
    let end = before[start..].find("\n#[test]").unwrap() + start;
    let traversal = before[start..end].replace(
        "labels.finish(function, budget)?",
        "labels.finish_lexical(function, budget)?",
    );
    let expected = format!("//! Same immutable census and target traversal; lexical paid publication only.\nuse super::*;\nimpl<'a> Directory<'a> {{\n{finish}\n}}\n{traversal}");
    assert_eq!(
        current, expected,
        "whole original algorithm differs only at lexical publication"
    );
}

#[test]
fn lexical_finish_rc_layout_const_qualification_is_exact_and_not_a_second_layout() {
    let before = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/paid-many-lexical-finish-20261017/before/src/cleanup_plan/replay/physical_clone.rs"));
    let current = include_str!("../../../physical_clone.rs");
    let from = "pub(super) fn rc_backing_bytes<T>() -> usize {";
    let to = "pub(super) const fn rc_backing_bytes<T>() -> usize {";
    assert_eq!(before.matches(from).count(), 1);
    assert_eq!(current.matches(to).count(), 1);
    assert_eq!(
        current,
        before.replacen(from, to, 1),
        "complete original helper except const qualifier"
    );
    assert_eq!(current.replacen(to, from, 1), before);
    for bad in [
        String::new(),
        current[..current.len() - 1].to_owned(),
        format!("{current} "),
        before.to_owned(),
    ] {
        assert_ne!(bad, before.replacen(from, to, 1));
    }
    let current = include_str!("../paid_many.rs");
    assert!(
        current.contains("+ super::physical_clone::rc_backing_bytes::<super::SmallWeighted>();")
    );
    let start = current
        .find("($function:ident, $budget:ident, $targets:ident, $pattern:pat => $flat:ident)")
        .unwrap();
    let body = &current[start..];
    assert!(body.contains("paid_many::LEXICAL_FINISH_TOTAL,"));
    assert!(
        !body.contains("rc_backing_bytes"),
        "no runtime helper/result or math before payment"
    );
}

#[test]
fn lexical_finish_active_and_oracle_combined_frontier_retains_sticky_floor() {
    use crate::cleanup_plan::replay::allocation::OracleScope;
    let function = fixture();
    let (_, _, setup, _) = crate::bounded_output::with_limit_usage_refusal(1 << 20, || {
        ReplayBudget::new()
            .reserve_bytes(&function, 1, "lexical frontier preparation")
            .unwrap();
    });
    crate::bounded_output::with_limit_usage_refusal(setup + LEXICAL_FINISH_TOTAL - 1, || {
        let mut budget = ReplayBudget::new();
        budget
            .reserve_bytes(&function, 1, "lexical frontier preparation")
            .unwrap();
        let entries = PUBLICATIONS.with(|value| value.get());
        let error = denied_publication(&function, &mut budget).unwrap_err();
        let expected = Diagnostic::io("SPX-H006", format!(
            "cleanup plan for function `{}` failed independent replay: cleanup replay physical reservation exhausted during factored lexical flat publication receivers", function.id));
        assert_eq!(error, expected);
        let remaining = crate::bounded_output::active_remaining();
        assert!(denied_publication(&function, &mut budget).is_err());
        assert_eq!(crate::bounded_output::active_remaining(), remaining);
        assert_eq!(budget.remaining, 0);
        assert_eq!(PUBLICATIONS.with(|value| value.get()), entries);
        assert_eq!(error, expected);
    });
    crate::bounded_output::with_limit_usage_refusal(setup + LEXICAL_FINISH_TOTAL, || {
        let mut budget = ReplayBudget::new();
        budget
            .reserve_bytes(&function, 1, "lexical frontier preparation")
            .unwrap();
        assert_eq!(
            crate::bounded_output::active_remaining(),
            Some(LEXICAL_FINISH_TOTAL)
        );
        assert!(crate::bounded_output::set_active_floor(1));
        let entries = PUBLICATIONS.with(|value| value.get());
        let before = crate::bounded_output::active_remaining();
        let error = denied_publication(&function, &mut budget).unwrap_err();
        assert_eq!(error.code, "SPX-H006");
        assert_eq!(
            crate::bounded_output::active_remaining(),
            before,
            "refusal never spends the held floor"
        );
        assert!(denied_publication(&function, &mut budget).is_err());
        assert_eq!(crate::bounded_output::active_remaining(), before);
        assert_eq!(PUBLICATIONS.with(|value| value.get()), entries);
        assert_eq!(budget.remaining, 0);
        crate::bounded_output::clear_active_floor();
    });
    let scope = OracleScope::open_with_work(1 << 20, 1 << 20).unwrap();
    let mut budget = ReplayBudget::with_skeleton_limit_oracle(0, scope.lease());
    budget
        .reserve_bytes(&function, 1, "lexical frontier preparation")
        .unwrap();
    let setup = scope.receipt().charged;
    drop(budget);
    drop(scope);
    let scope = OracleScope::open_with_work(setup + LEXICAL_FINISH_TOTAL - 1, 1 << 20).unwrap();
    let mut budget = ReplayBudget::with_skeleton_limit_oracle(0, scope.lease());
    budget
        .reserve_bytes(&function, 1, "lexical frontier preparation")
        .unwrap();
    let entries = PUBLICATIONS.with(|value| value.get());
    let error = denied_publication(&function, &mut budget).unwrap_err();
    assert_eq!(error.code, "SPX-H006");
    let charged = scope.receipt().charged;
    assert!(denied_publication(&function, &mut budget).is_err());
    assert_eq!(scope.receipt().charged, charged);
    assert_eq!(budget.remaining, 0);
    assert_eq!(PUBLICATIONS.with(|value| value.get()), entries);
}
