//! Full old ordered storage, real receiving requests and every staged boundary.
use super::*;

type HeldRoots<K> = (Option<Rc<Node<K>>>, Option<std::rc::Weak<Node<K>>>);
use crate::test_allocation_observer::{current_metrics, measure, Metrics};
const LIMIT: usize = 64 * 1024 * 1024;
fn keys() -> (Vec<ExpressionId>, Vec<ValueId>) {
    let execution = FunctionExecutionId::Monomorphic(DeclarationId::new("identity.owner"));
    (
        (0..128)
            .map(|n| ExpressionId::new(&execution, &format!("body.arg.{n:04}")))
            .collect(),
        (0..128)
            .map(|n| ValueId::new(format!("identity.value.{n:04}")))
            .collect(),
    )
}
fn check_tree<K: Key>(root: &Root<K>, low: Option<&K>, high: Option<&K>) -> (usize, usize) {
    let Some(node) = root else {
        return (0, 0);
    };
    assert!(low.is_none_or(|key| key < &node.key));
    assert!(high.is_none_or(|key| &node.key < key));
    let (lh, lc) = check_tree(&node.left, low, Some(&node.key));
    let (rh, rc) = check_tree(&node.right, Some(&node.key), high);
    assert!(lh.abs_diff(rh) <= 1);
    assert_eq!(node.height, 1 + lh.max(rh));
    assert_eq!(node.count, 1 + lc + rc);
    assert!(node.height < iter::HEIGHT);
    (node.height, node.count)
}
#[derive(Clone, Copy, Debug)]
enum Sharing {
    Unique,
    Root,
    Descendant,
    WeakRoot,
    WeakDescendant,
    DeepDescendant,
    WeakDeepDescendant,
}
const SHARING: [Sharing; 5] = [
    Sharing::Unique,
    Sharing::Root,
    Sharing::Descendant,
    Sharing::WeakRoot,
    Sharing::WeakDescendant,
];
const ROTATIONS: [([usize; 2], usize); 4] = [([2, 1], 0), ([0, 1], 2), ([2, 0], 1), ([0, 2], 1)];
fn descendant<'a, K: Key>(set: &'a Set<K>, key: &K) -> Option<&'a Rc<Node<K>>> {
    let root = set.root.as_ref().unwrap();
    if key < &root.key {
        root.left.as_ref()
    } else {
        root.right.as_ref()
    }
}
fn hold<K: Key>(set: &Set<K>, key: &K, sharing: Sharing) -> HeldRoots<K> {
    match sharing {
        Sharing::Unique => (None, None),
        Sharing::Root => (set.root.clone(), None),
        Sharing::Descendant => (descendant(set, key).cloned(), None),
        Sharing::WeakRoot => (None, set.root.as_ref().map(Rc::downgrade)),
        Sharing::WeakDescendant => (None, descendant(set, key).map(Rc::downgrade)),
        Sharing::DeepDescendant | Sharing::WeakDeepDescendant => {
            let node = descendant(set, key).unwrap();
            let next = if key < &node.key {
                node.left.as_ref()
            } else {
                node.right.as_ref()
            };
            if matches!(sharing, Sharing::DeepDescendant) {
                (next.cloned(), None)
            } else {
                (None, next.map(Rc::downgrade))
            }
        }
    }
}
type State<'a, K> = (usize, &'a K, usize, usize, usize, usize);
fn states<'a, K>(root: &'a Root<K>, out: &mut Vec<State<'a, K>>) {
    if let Some(node) = root {
        states(&node.left, out);
        out.push((
            Rc::as_ptr(node) as usize,
            &node.key,
            node.height,
            node.count,
            node.left.as_ref().map_or(0, |n| Rc::as_ptr(n) as usize),
            node.right.as_ref().map_or(0, |n| Rc::as_ptr(n) as usize),
        ));
        states(&node.right, out);
    }
}
// Preserve metadata/pointer state without retaining a rollback Rc root. Keys
// clone shared backing only; snapshot creation is outside the receiving window.
fn owned_states<K: Key>(root: &Root<K>) -> Vec<(usize, K, usize, usize, usize, usize)> {
    let mut borrowed = Vec::new();
    states(root, &mut borrowed);
    borrowed
        .into_iter()
        .map(|(p, k, h, c, l, r)| (p, k.clone(), h, c, l, r))
        .collect()
}
fn unchanged<K: Key>(root: &Root<K>, expected: &[(usize, K, usize, usize, usize, usize)]) {
    fn visit<K: Key>(
        root: &Root<K>,
        expected: &mut std::slice::Iter<'_, (usize, K, usize, usize, usize, usize)>,
    ) {
        if let Some(node) = root {
            visit(&node.left, expected);
            let (p, k, h, c, l, r) = expected.next().unwrap();
            assert_eq!(*p, Rc::as_ptr(node) as usize);
            assert_eq!(k, &node.key);
            assert_eq!((*h, *c), (node.height, node.count));
            assert_eq!(*l, node.left.as_ref().map_or(0, |n| Rc::as_ptr(n) as usize));
            assert_eq!(
                *r,
                node.right.as_ref().map_or(0, |n| Rc::as_ptr(n) as usize)
            );
            visit(&node.right, expected);
        }
    }
    let mut entries = expected.iter();
    visit(root, &mut entries);
    assert!(entries.next().is_none());
}
fn receiving<K: Key>(
    keys: &[K],
    order: &[usize],
    index: usize,
    sharing: Sharing,
    limit: usize,
) -> (bool, Metrics, bool, usize) {
    let mut set = Set::new();
    let mut prior = BTreeSet::new();
    for &n in order {
        assert!(set.insert_paid(&keys[n]).unwrap());
        prior.insert(keys[n].clone());
    }
    let before = owned_states(&set.root);
    let (strong, weak) = hold(&set, &keys[index], sharing);
    let held_before = strong
        .as_ref()
        .map(|node| owned_states(&Some(node.clone())));
    let mut expected = prior.clone();
    expected.insert(keys[index].clone());
    let ((ok, observed), refused, paid) = crate::bounded_output::with_limit_usage(limit, || {
        measure(|| {
            let result = set.insert_paid(&keys[index]);
            let ok = result.is_ok();
            if ok {
                assert!(set.iter().eq(expected.iter()));
            } else {
                assert!(set.iter().eq(prior.iter()));
                unchanged(&set.root, &before);
                assert!(!crate::bounded_output::reserve_active_required(0));
                let start = current_metrics();
                let retry = set.insert_paid(&keys[index]);
                assert!(retry.is_err());
                drop(retry);
                let end = current_metrics();
                assert_eq!(start.allocations, end.allocations);
                assert_eq!(start.allocated, end.allocated);
                unchanged(&set.root, &before);
            }
            check_tree(&set.root, None, None);
            if let (Some(node), Some(state)) = (&strong, &held_before) {
                unchanged(&Some(node.clone()), state);
            }
            // Failed staging preserves weak upgrade authority and old state.
            if !ok {
                if let Some(owner) = &weak {
                    assert!(owner.upgrade().is_some());
                }
            }
            let remaining = crate::bounded_output::active_remaining();
            drop(result);
            drop(set);
            drop(strong);
            drop(weak);
            assert_eq!(remaining, crate::bounded_output::active_remaining());
            ok
        })
    });
    assert!(!observed.invalid && observed.live == 0);
    assert_eq!(observed.allocated, observed.freed);
    assert!(observed.allocated <= paid && observed.highwater <= paid);
    (ok, observed, refused, paid)
}
#[test]
fn all_rotation_shapes_unique_shared_and_weak_keep_full_old_order_and_metadata() {
    let (expressions, values) = keys();
    fn run<K: Key>(keys: &[K]) {
        for (order, index) in ROTATIONS {
            for sharing in SHARING {
                let (ok, wide, refused, total) = receiving(keys, &order, index, sharing, LIMIT);
                assert!(ok && !refused && total > 0);
                let (ok, exact, refused, paid) = receiving(keys, &order, index, sharing, total);
                assert!(ok && !refused && paid == total);
                assert_eq!(wide, exact);
                if matches!(sharing, Sharing::Unique) {
                    assert_eq!(wide.allocations, 1);
                }
                for limit in 0..total {
                    let (ok, _, refused, paid) = receiving(keys, &order, index, sharing, limit);
                    assert!(!ok && refused && paid <= limit);
                }
            }
        }
    }
    run(&expressions);
    run(&values);
}
#[test]
fn whole_directory_snapshots_ordered_cursors_and_drop_keep_complete_btree_state() {
    let (expressions, values) = keys();
    fn run<K: Key>(keys: &[K]) {
        let expected = keys.iter().cloned().collect::<BTreeSet<_>>();
        for reversed in [false, true] {
            let (((), actual), refused, paid) =
                crate::bounded_output::with_limit_usage(LIMIT, || {
                    measure(|| {
                        let mut current = Set::new();
                        for n in 0..keys.len() {
                            let index = if reversed { keys.len() - 1 - n } else { n };
                            assert!(current.insert_paid(&keys[index]).unwrap());
                        }
                        assert!(current.iter().eq(expected.iter()));
                        check_tree(&current.root, None, None);
                        let old = current.clone();
                        let before = current_metrics();
                        let another = current.clone();
                        assert_eq!(before, current_metrics());
                        assert_eq!(current, another);
                        let mut cursor = current.iter();
                        let mut oracle = expected.iter();
                        for n in 0..keys.len() {
                            if n % 2 == 0 {
                                assert_eq!(cursor.next(), oracle.next());
                            } else {
                                assert_eq!(cursor.next_back(), oracle.next_back());
                            }
                        }
                        assert_eq!(cursor.len(), 0);
                        assert!(cursor.next().is_none() && cursor.next_back().is_none());
                        assert_eq!(current, old);
                        drop(current);
                        drop(old);
                        drop(another);
                    })
                });
            assert!(!refused && !actual.invalid && actual.live == 0);
            assert_eq!(actual.allocated, actual.freed);
            assert!(actual.allocated <= paid && actual.highwater <= paid);
        }
    }
    run(&expressions);
    run(&values);
}
#[test]
fn duplicate_priority_and_sticky_absent_insert_have_zero_requests_and_unchanged_roots() {
    let (_, keys) = keys();
    let mut current = Set::new();
    assert!(current.insert_paid(&keys[0]).unwrap());
    let prior = owned_states(&current.root);
    let (((), actual), refused, paid) = crate::bounded_output::with_limit_usage(0, || {
        measure(|| {
            assert!(!current.insert_paid(&keys[0]).unwrap());
            assert!(!crate::bounded_output::active_refused());
            assert!(!crate::bounded_output::reserve_active_required(1));
            assert!(!current.insert_paid(&keys[0]).unwrap());
            let error = current.insert_paid(&keys[1]).unwrap_err();
            unchanged(&current.root, &prior);
            drop(error);
        })
    });
    assert!(refused && paid == 0 && !actual.invalid);
    assert_eq!(actual.allocations, 0);
    assert_eq!(actual.allocated, 0);
    assert_eq!(actual.live, 0);
}
#[test]
fn shared_snapshot_success_preserves_every_old_identity_and_unwind_drops_real_nodes() {
    let (_, keys) = keys();
    let expected = keys[..64].iter().cloned().collect::<BTreeSet<_>>();
    let message = "real identity directory unwind".to_owned();
    let ((caught, actual), refused, paid) = crate::bounded_output::with_limit_usage(LIMIT, || {
        measure(|| {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut current = Set::new();
                for key in &keys[..64] {
                    current.insert_paid(key).unwrap();
                }
                let snapshot = current.clone();
                for key in &keys[64..] {
                    current.insert_paid(key).unwrap();
                }
                assert!(snapshot.iter().eq(expected.iter()));
                assert_eq!(current.len(), 128);
                check_tree(&current.root, None, None);
                check_tree(&snapshot.root, None, None);
                scope_checkpoint::charge::reserve(Some(std::mem::size_of::<String>())).unwrap();
                std::panic::resume_unwind(Box::new(message));
            }));
            let caught = result.is_err();
            drop(result);
            caught
        })
    });
    assert!(caught && !refused && !actual.invalid && actual.live == 0);
    assert_eq!(actual.allocated, actual.freed);
    // The real panic payload Box is also constructed inside the window.
    assert!(actual.allocated <= paid && actual.highwater <= paid);
}

#[test]
fn deep_unique_prefix_shared_and_weak_suffixes_preserve_every_original_node_at_each_boundary() {
    let (expressions, values) = keys();
    let order = (0..64).collect::<Vec<_>>();
    fn run<K: Key>(keys: &[K], order: &[usize]) {
        for sharing in [
            Sharing::Unique,
            Sharing::Root,
            Sharing::Descendant,
            Sharing::WeakRoot,
            Sharing::WeakDescendant,
            Sharing::DeepDescendant,
            Sharing::WeakDeepDescendant,
        ] {
            let (ok, wide, refused, total) = receiving(keys, order, 127, sharing, LIMIT);
            assert!(ok && !refused && total > 0);
            let (ok, exact, refused, paid) = receiving(keys, order, 127, sharing, total);
            assert!(ok && !refused && paid == total);
            assert_eq!(wide, exact);
            if matches!(sharing, Sharing::Unique) {
                assert_eq!(wide.allocations, 1);
            }
            for limit in 0..total {
                let (ok, _, refused, paid) = receiving(keys, order, 127, sharing, limit);
                assert!(!ok && refused && paid <= limit);
            }
        }
    }
    run(&expressions, &order);
    run(&values, &order);
}

#[test]
fn exact_empty_nul_unicode_and_independent_backings_keep_full_old_key_order_and_duplicates() {
    let spellings = ["", "\0", "a", "a\0", "a\0b", "é", "e\u{301}", "😀", "z"];
    let inputs = spellings
        .iter()
        .map(|text| ValueId::new((*text).to_owned()))
        .collect::<Vec<_>>();
    let independent = spellings
        .iter()
        .map(|text| ValueId::new((*text).to_owned()))
        .collect::<Vec<_>>();
    let expected = inputs.iter().cloned().collect::<BTreeSet<_>>();
    let mut current = Set::new();
    let mut old = Set::new();
    old.use_legacy();
    for key in &inputs {
        assert_eq!(current.insert_paid(key).unwrap(), old.insert(key.clone()));
    }
    assert_eq!(current, old);
    assert!(current.iter().eq(expected.iter()));
    let (((), actual), refused, paid) = crate::bounded_output::with_limit_usage(0, || {
        measure(|| {
            for key in &independent {
                assert!(current.contains(key) && old.contains(key));
                assert!(!current.insert_paid(key).unwrap());
                assert!(!old.insert(key.clone()));
            }
            assert_eq!(current, old);
        })
    });
    assert!(!refused && paid == 0 && !actual.invalid);
    assert_eq!(actual.allocations, 0);
    assert_eq!(actual.allocated, 0);
    assert_eq!(actual.live, 0);
    assert!(current.iter().eq(expected.iter()));
}
