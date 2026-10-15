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
        at: Link,
        parent: Link,
        low: Option<usize>,
        high: Option<usize>,
    ) -> (usize, u8) {
        if at == NONE {
            return (0, 0);
        }
        let node = &weights.nodes[at as usize];
        assert_eq!(node.parent, parent);
        assert!(low.is_none_or(|low| low < node.key));
        assert!(high.is_none_or(|high| node.key < high));
        let (left, lh) = walk(weights, node.left, at, low, Some(node.key));
        let (right, rh) = walk(weights, node.right, at, Some(node.key), high);
        assert_eq!(node.height, 1 + lh.max(rh));
        assert!(lh.abs_diff(rh) <= 1);
        (left + right + 1, node.height)
    }
    if weights.used == 1 && weights.nodes.is_empty() {
        assert_eq!(weights.len(), 1);
        assert!(weights.nodes.is_empty());
        assert_eq!(weights.root, NONE);
        return;
    }
    assert_eq!(
        walk(weights, weights.root, NONE, None, None).0,
        weights.len()
    );
    for node in &weights.nodes[weights.used..] {
        assert_eq!(node.height, 0);
        assert_eq!((node.left, node.right, node.parent), (NONE, NONE, NONE));
    }
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
            if weights.needs_capacity() {
                weights.grow(&function, &mut budget).unwrap();
            }
            weights.insert_prepaid(key, value);
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
                if weights.needs_capacity() {
                    if let Err(error) = weights.grow(function, &mut budget) {
                        assert_eq!(error.code, "SPX-H006");
                        assert_eq!(weights.len(), previous);
                        drop(error);
                        let stopped = current_metrics().allocated;
                        assert!(weights.grow(function, &mut budget).is_err());
                        assert_eq!(current_metrics().allocated, stopped);
                        assert_eq!(weights.len(), previous);
                        complete = false;
                        break;
                    }
                }
                weights.insert_prepaid(*key, 1);
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
fn ordered_growth_receiving_and_initializers_are_paid_then_warm_insertion_constructs_no_result() {
    let function = fixture();
    assert_eq!(
        OrderedWeights::GROW_CONTROLS,
        4 * size_of::<Result<(), Diagnostic>>()
    );
    assert!(MAX_KEYS <= MAX_SLOTS && MAX_SLOTS < u32::MAX as usize);
    let mut probe = ReplayBudget::with_physical(&function, usize::MAX).unwrap();
    probe
        .reserve_bytes(&function, OrderedWeights::CONTROLS, "ordered formula owner")
        .unwrap();
    let mut weights = OrderedWeights::new();
    let prefix = probe.physical.as_ref().unwrap().charged();
    let ((), physical) = measure(|| weights.insert_prepaid(0, 7));
    assert_eq!(physical.allocated, 0);
    assert_eq!(probe.physical.as_ref().unwrap().charged(), prefix);
    assert!(weights.used == 1 && weights.nodes.is_empty());
    weights.grow(&function, &mut probe).unwrap();
    let growth = probe.physical.as_ref().unwrap().charged() - prefix;
    assert_eq!(
        growth,
        OrderedWeights::GROW_CONTROLS + 2 * weights.nodes.len() * size_of::<Node>()
    );
    let capacity = weights.nodes.capacity();
    let paid = probe.physical.as_ref().unwrap().charged();
    let ((), physical) = measure(|| weights.insert_prepaid(usize::MAX, 11));
    assert_eq!(physical.allocated, 0);
    assert_eq!(probe.physical.as_ref().unwrap().charged(), paid);
    assert_eq!(weights.nodes.capacity(), capacity);
    invariant(&weights);
    for remaining in [
        0,
        OrderedWeights::GROW_CONTROLS - 1,
        OrderedWeights::GROW_CONTROLS,
        growth - 1,
        growth,
    ] {
        let mut budget = ReplayBudget::with_physical(&function, prefix + remaining).unwrap();
        budget
            .reserve_bytes(&function, OrderedWeights::CONTROLS, "ordered formula owner")
            .unwrap();
        let mut weights = OrderedWeights::new();
        weights.insert_prepaid(0, 7);
        let result = weights.grow(&function, &mut budget);
        assert_eq!(result.is_ok(), remaining == growth);
        assert_eq!(weights.len(), 1);
        assert_eq!(weights.get(&0), Some(&7));
        if result.is_ok() {
            assert_eq!(weights.nodes.len(), capacity);
            assert_eq!(weights.nodes.capacity(), capacity);
            weights.insert_prepaid(usize::MAX, 11);
            assert_eq!(weights.get(&usize::MAX), Some(&11));
            invariant(&weights);
        } else {
            drop(result);
            assert_eq!(
                weights.root, NONE,
                "refused promotion never publishes an arena root"
            );
            assert!(weights.nodes.is_empty());
            assert!(weights.iter().eq(std::iter::once((&0, &7))));
            if remaining == growth - 1 {
                assert_eq!(
                    weights.nodes.capacity(),
                    capacity,
                    "backing acquisition precedes the refused empty-node initializer payment"
                );
            }
            let ((), physical) = measure(|| {
                let at = current_metrics().allocated;
                assert!(weights.grow(&function, &mut budget).is_err());
                assert_eq!(current_metrics().allocated, at);
            });
            assert_eq!(physical.allocated, 0);
            assert!(!physical.invalid && physical.live == 0);
            assert_eq!(weights.get(&0), Some(&7));
            assert!(weights.iter().eq(std::iter::once((&0, &7))));
        }
    }
    // Repeat the initializer refusal after a real AVL arena is already live.
    let arena_prefix = probe.physical.as_ref().unwrap().charged();
    let next_backing = 2 * capacity * size_of::<Node>();
    let next_initializers = capacity * size_of::<Node>();
    let next_growth = OrderedWeights::GROW_CONTROLS + next_backing + next_initializers;
    for remaining in [
        0,
        OrderedWeights::GROW_CONTROLS - 1,
        OrderedWeights::GROW_CONTROLS + next_backing,
        next_growth - 1,
        next_growth,
    ] {
        let mut budget = ReplayBudget::with_physical(&function, arena_prefix + remaining).unwrap();
        budget
            .reserve_bytes(&function, OrderedWeights::CONTROLS, "ordered formula owner")
            .unwrap();
        let mut weights = OrderedWeights::new();
        weights.insert_prepaid(0, 7);
        weights.grow(&function, &mut budget).unwrap();
        weights.insert_prepaid(usize::MAX, 11);
        let root = weights.root;
        let left_height = weights.nodes[0].height;
        let second_parent = weights.nodes[1].parent;
        let result = weights.grow(&function, &mut budget);
        assert_eq!(result.is_ok(), remaining == next_growth);
        assert_eq!(weights.len(), 2);
        assert_eq!(weights.root, root);
        assert_eq!(weights.nodes[0].height, left_height);
        assert_eq!(weights.nodes[1].parent, second_parent);
        assert!(
            weights.iter().eq([(0, 7), (usize::MAX, 11)]
                .iter()
                .map(|(key, weight)| (key, weight)))
        );
        if result.is_err() {
            drop(result);
            assert_eq!(weights.nodes.len(), capacity);
            if remaining >= OrderedWeights::GROW_CONTROLS + next_backing {
                assert_eq!(weights.nodes.capacity(), 2 * capacity);
            }
            assert!(weights.grow(&function, &mut budget).is_err());
            assert_eq!(weights.root, root);
            assert!(
                weights.iter().eq([(0, 7), (usize::MAX, 11)]
                    .iter()
                    .map(|(key, weight)| (key, weight)))
            );
        }
        invariant(&weights);
    }
}

#[test]
fn ordered_private_index_growth_refuses_before_allocation_or_mutation() {
    let function = fixture();
    let mut budget = ReplayBudget::with_physical(&function, usize::MAX).unwrap();
    budget
        .reserve_bytes(&function, OrderedWeights::CONTROLS, "ordered hostile owner")
        .unwrap();
    let mut weights = OrderedWeights::new();
    weights.used = MAX_KEYS; // Deliberate private metadata corruption, no huge allocation.
    let (result, physical) = measure(|| weights.grow(&function, &mut budget));
    let error = result.unwrap_err();
    assert_eq!(error.code, "SPX-H006");
    assert!(error.message.contains("ordered arena index bound exceeded"));
    assert_eq!(weights.used, MAX_KEYS);
    assert_eq!(weights.nodes.capacity(), 0);
    assert!(weights.nodes.is_empty());
    assert_eq!(physical.allocated, 0);
}
