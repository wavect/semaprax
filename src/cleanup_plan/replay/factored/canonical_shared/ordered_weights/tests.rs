//! Actual arena rotations, sparse-key allocation and whole-owner Drop controls.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::{current_metrics, measure};
use std::collections::BTreeMap;

fn fixture() -> ResolvedFunction {
    super::super::super::lexical_plan_tests::fixture(super::super::super::lexical_plan_tests::WHILE)
        .1
}
fn invariant(weights: &OrderedWeights) {
    fn walk(
        weights: &OrderedWeights,
        at: usize,
        parent: usize,
        low: Option<usize>,
        high: Option<usize>,
    ) -> (usize, u8) {
        if at == NONE {
            return (0, 0);
        }
        let node = &weights.nodes[at];
        assert_eq!(node.parent, parent);
        assert!(low.is_none_or(|low| low < node.key));
        assert!(high.is_none_or(|high| node.key < high));
        let (left, lh) = walk(weights, node.left, at, low, Some(node.key));
        let (right, rh) = walk(weights, node.right, at, Some(node.key), high);
        assert_eq!(node.height, 1 + lh.max(rh));
        assert!(lh.abs_diff(rh) <= 1);
        (left + right + 1, node.height)
    }
    assert_eq!(
        walk(weights, weights.root, NONE, None, None).0,
        weights.len()
    );
}

#[test]
fn ordered_arena_all_rotations_numeric_extremes_and_dense_permutations_match_btree() {
    let function = fixture();
    // LL, RR, LR, RL, ascending/descending and a permutation with every rank.
    let cases = [
        vec![3, 2, 1],
        vec![1, 2, 3],
        vec![3, 1, 2],
        vec![1, 3, 2],
        (0..257).collect(),
        (0..257).rev().collect(),
        (0..257).map(|rank| (rank * 127) % 257).collect(),
        vec![usize::MAX, 0, usize::MAX / 2, 1, usize::MAX - 1],
    ];
    for keys in cases {
        let mut budget = ReplayBudget::unobserved();
        let mut weights = OrderedWeights::new();
        let mut old = BTreeMap::new();
        for key in keys {
            let value = (key as u64).wrapping_mul(17);
            weights
                .insert_new(&function, &mut budget, key, value)
                .unwrap();
            old.insert(key, value);
            invariant(&weights);
            assert!(weights.iter().eq(old.iter()));
            assert_eq!(weights.get(&key), old.get(&key));
            *weights.get_mut(&key).unwrap() = value.wrapping_add(1);
            *old.get_mut(&key).unwrap() = value.wrapping_add(1);
            assert!(weights.iter().eq(old.iter()));
        }
        assert_eq!(weights.get(&258), old.get(&258));
    }
}

fn observe(
    function: &ResolvedFunction,
    keys: &[usize],
    limit: usize,
) -> (
    (bool, crate::test_allocation_observer::Metrics),
    bool,
    usize,
    Option<crate::bounded_output::ReservationRefusal>,
) {
    with_limit_usage_refusal(limit, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            if budget
                .reserve_bytes(function, OrderedWeights::CONTROLS, "ordered arena controls")
                .is_err()
            {
                return false;
            }
            let mut weights = OrderedWeights::new();
            let mut complete = true;
            for key in keys {
                let previous = weights.len();
                if let Err(error) = weights.insert_new(function, &mut budget, *key, 1) {
                    assert_eq!(error.code, "SPX-H006");
                    assert_eq!(weights.len(), previous);
                    drop(error);
                    let stopped = current_metrics().allocated;
                    assert!(weights.insert_new(function, &mut budget, *key, 1).is_err());
                    assert_eq!(current_metrics().allocated, stopped);
                    assert_eq!(weights.len(), previous);
                    complete = false;
                    break;
                }
            }
            let stopped = current_metrics().allocated;
            drop(weights);
            drop(budget);
            assert_eq!(current_metrics().allocated, stopped);
            complete
        })
    })
}

#[test]
fn ordered_arena_each_growth_zero_exact_short_keeps_actual_allocation_separate_and_drops() {
    let function = fixture();
    let keys = [
        usize::MAX,
        1,
        usize::MAX / 2,
        0,
        9,
        8,
        7,
        6,
        5,
        4,
        3,
        2,
        11,
        13,
        12,
        17,
        16,
    ];
    // Every prefix covers the initial arena and each geometric growth boundary.
    for count in 1..=keys.len() {
        let full = observe(&function, &keys[..count], usize::MAX);
        assert!(full.0.0 && !full.1 && full.3.is_none());
        assert!(!full.0.1.invalid && full.0.1.live == 0);
        assert_eq!(full.0.1.allocated, full.0.1.freed);
        assert!(full.0.1.allocated <= full.2 && full.0.1.highwater <= full.2);
        for bytes in [0, full.2 - 1, full.2] {
            let actual = observe(&function, &keys[..count], bytes);
            assert_eq!(actual.0.0, bytes == full.2);
            assert_eq!(actual.1, bytes < full.2);
            assert_eq!(actual.3.is_some(), bytes < full.2);
            assert!(!actual.0.1.invalid && actual.0.1.live == 0);
            assert_eq!(actual.0.1.allocated, actual.0.1.freed);
            if bytes == 0 {
                assert_eq!(actual.0.1.allocated, 0);
            }
        }
    }
    let sparse = observe(&function, &[0, usize::MAX], usize::MAX);
    let dense = observe(&function, &[0, 1], usize::MAX);
    assert_eq!(sparse.2, dense.2);
    assert_eq!(sparse.0.1.allocated, dense.0.1.allocated);
    eprintln!(
        "ordered sparse arena reservation={} allocation={:?}",
        sparse.2, sparse.0.1
    );
}

#[test]
fn ordered_insertion_all_three_receiving_results_are_paid_even_without_growth() {
    let function = fixture();
    assert_eq!(
        OrderedWeights::INSERT_CONTROLS,
        3 * std::mem::size_of::<Result<(), Diagnostic>>() + std::mem::size_of::<Node>()
    );
    let mut probe = ReplayBudget::with_physical(&function, usize::MAX).unwrap();
    probe
        .reserve_bytes(&function, OrderedWeights::CONTROLS, "ordered formula owner")
        .unwrap();
    let mut weights = OrderedWeights::new();
    let before = probe.physical.as_ref().unwrap().charged();
    weights.insert_new(&function, &mut probe, 0, 7).unwrap();
    let prefix = probe.physical.as_ref().unwrap().charged();
    assert_eq!(
        prefix - before,
        OrderedWeights::INSERT_CONTROLS + weights.nodes.capacity() * std::mem::size_of::<Node>()
    );
    let capacity = weights.nodes.capacity();
    assert!(capacity > weights.len());
    let at = probe.physical.as_ref().unwrap().charged();
    weights
        .insert_new(&function, &mut probe, usize::MAX, 11)
        .unwrap();
    assert_eq!(weights.nodes.capacity(), capacity);
    assert_eq!(
        probe.physical.as_ref().unwrap().charged() - at,
        3 * std::mem::size_of::<Result<(), Diagnostic>>() + std::mem::size_of::<Node>()
    );
    for remaining in [
        0,
        OrderedWeights::INSERT_CONTROLS - 1,
        OrderedWeights::INSERT_CONTROLS,
    ] {
        let mut budget = ReplayBudget::with_physical(&function, prefix + remaining).unwrap();
        budget
            .reserve_bytes(&function, OrderedWeights::CONTROLS, "ordered formula owner")
            .unwrap();
        let mut weights = OrderedWeights::new();
        weights.insert_new(&function, &mut budget, 0, 7).unwrap();
        let (result, physical) =
            measure(|| weights.insert_new(&function, &mut budget, usize::MAX, 11));
        assert_eq!(result.is_ok(), remaining == OrderedWeights::INSERT_CONTROLS);
        assert_eq!(physical.allocated, 0);
        assert_eq!(weights.get(&0), Some(&7));
        assert_eq!(
            weights.get(&usize::MAX),
            if remaining == OrderedWeights::INSERT_CONTROLS {
                Some(&11)
            } else {
                None
            }
        );
    }
}
