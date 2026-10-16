//! Complete ordered structural equality for independently synthesized DAGs.
//! Test drivers call this outside allocation observers. Production Observations
//! equality deliberately compares merged-root identity and remains unchanged.
use crate::cleanup_plan::replay::skeleton_work::{
    ObservationDag, ObservationDagKind, Observations,
};
use crate::cleanup_plan::replay::{SkeletonObservation, StatusLane, StatusSourceId};
use crate::hir::ExpressionId;
use std::rc::Rc;

const MAX_NODES: usize = 4096;
#[derive(Debug, Eq, PartialEq)]
enum Failure {
    Bound,
    Allocation,
    Tail,
    Prefix,
    Kind,
    Length,
}

fn compare(left: &Observations, right: &Observations, limit: usize) -> Result<(), Failure> {
    if limit == 0 || limit > MAX_NODES {
        return Err(Failure::Bound);
    }
    // Equality of Rc<Observation> compares every complete observation value,
    // including status/source/type/owner fields; no Debug or hash is consulted.
    if left.items != right.items {
        return Err(Failure::Tail);
    }
    let mut pending = Vec::new();
    pending
        .try_reserve_exact(limit)
        .map_err(|_| Failure::Allocation)?;
    pending.push((left.prefix.as_deref(), right.prefix.as_deref()));
    let mut visited = 0;
    while let Some((left, right)) = pending.pop() {
        if visited == limit {
            return Err(Failure::Bound);
        }
        visited += 1;
        let (left, right) = match (left, right) {
            (None, None) => continue,
            (Some(left), Some(right)) => (left, right),
            _ => return Err(Failure::Prefix),
        };
        match (&left.kind, &right.kind) {
            (ObservationDagKind::Seq(a, at), ObservationDagKind::Seq(b, bt)) => {
                if at != bt {
                    return Err(Failure::Tail);
                }
                if pending.len() == limit {
                    return Err(Failure::Bound);
                }
                pending.push((a.as_deref(), b.as_deref()));
            }
            (ObservationDagKind::Cat(a), ObservationDagKind::Cat(b))
            | (ObservationDagKind::Alt(a), ObservationDagKind::Alt(b)) => {
                if a.len() != b.len() {
                    return Err(Failure::Length);
                }
                if a.len() > limit - pending.len() {
                    return Err(Failure::Bound);
                }
                // Reverse only the pending stack traversal: each original
                // child is compared with the same ordered peer. No sorting,
                // language flattening, branch dropping, or identity shortcut.
                pending.extend(
                    a.iter()
                        .zip(b)
                        .rev()
                        .map(|(a, b)| (Some(a.as_ref()), Some(b.as_ref()))),
                );
            }
            _ => return Err(Failure::Kind),
        }
    }
    Ok(())
}

pub(super) fn assert_equal(left: &Observations, right: &Observations) {
    assert_eq!(compare(left, right, MAX_NODES), Ok(()));
}

pub(super) fn verify_contract(first: &ExpressionId, second: &ExpressionId) {
    assert_ne!(first, second);
    let status = |expression: &ExpressionId, success| {
        Rc::new(SkeletonObservation::Status {
            source: StatusSourceId {
                expression: expression.clone(),
                lane: StatusLane::OperationFailure,
            },
            success,
        })
    };
    let leaf = |value| {
        Rc::new(ObservationDag::legacy(ObservationDagKind::Seq(
            None,
            vec![value],
        )))
    };
    let a = leaf(status(first, true));
    let b = leaf(status(first, false));
    let make = |kind| Observations {
        prefix: Some(Rc::new(ObservationDag::legacy(kind))),
        items: vec![status(first, true)],
    };
    let left = make(ObservationDagKind::Cat(vec![a.clone(), b.clone()]));
    let right = make(ObservationDagKind::Cat(vec![
        leaf(status(first, true)),
        leaf(status(first, false)),
    ]));
    assert!(!Rc::ptr_eq(
        left.prefix.as_ref().unwrap(),
        right.prefix.as_ref().unwrap()
    ));
    assert_equal(&left, &right); // Distinct roots with every field/order equal.
    let kind = make(ObservationDagKind::Alt(vec![a.clone(), b.clone()]));
    assert_eq!(format!("{left:?}"), format!("{kind:?}"));
    assert_eq!(compare(&left, &kind, MAX_NODES), Err(Failure::Kind));
    let other_alt = make(ObservationDagKind::Alt(vec![
        leaf(status(first, true)),
        leaf(status(first, false)),
    ]));
    assert_equal(&kind, &other_alt);
    let alt_order = make(ObservationDagKind::Alt(vec![b.clone(), a.clone()]));
    assert_eq!(compare(&kind, &alt_order, MAX_NODES), Err(Failure::Tail));
    let order = make(ObservationDagKind::Cat(vec![b.clone(), a.clone()]));
    assert_eq!(compare(&left, &order, MAX_NODES), Err(Failure::Tail));
    let length = make(ObservationDagKind::Cat(vec![a.clone()]));
    assert_eq!(compare(&left, &length, MAX_NODES), Err(Failure::Length));
    let source = make(ObservationDagKind::Cat(vec![
        leaf(status(second, true)),
        b.clone(),
    ]));
    assert_eq!(compare(&left, &source, MAX_NODES), Err(Failure::Tail));
    let changed_status = make(ObservationDagKind::Cat(vec![
        leaf(status(first, false)),
        b.clone(),
    ]));
    assert_eq!(
        compare(&left, &changed_status, MAX_NODES),
        Err(Failure::Tail)
    );
    let nested = make(ObservationDagKind::Seq(
        Some(a.clone()),
        vec![status(first, true)],
    ));
    let nested_equal = make(ObservationDagKind::Seq(
        Some(leaf(status(first, true))),
        vec![status(first, true)],
    ));
    assert_equal(&nested, &nested_equal);
    let no_prefix = make(ObservationDagKind::Seq(None, vec![status(first, true)]));
    assert_eq!(
        compare(&nested, &no_prefix, MAX_NODES),
        Err(Failure::Prefix)
    );
    let tail = Observations {
        prefix: left.prefix.clone(),
        items: vec![],
    };
    assert_eq!(compare(&left, &tail, MAX_NODES), Err(Failure::Tail));
    assert_eq!(compare(&left, &right, 0), Err(Failure::Bound));
    assert_eq!(compare(&left, &right, 1), Err(Failure::Bound));
    assert_eq!(compare(&left, &right, MAX_NODES + 1), Err(Failure::Bound));
}
