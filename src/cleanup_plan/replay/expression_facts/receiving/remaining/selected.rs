//! Full ordered pure scan; no selected-status Vec acquisition.
use super::*;
pub(in crate::cleanup_plan::replay::expression_facts::receiving) fn unique_status(
    transitions: &[CleanupTransition],
    source: &StatusSourceId,
) -> bool {
    let mut count = 0usize;
    let mut same = true;
    for transition in transitions {
        if let CleanupTransition::SelectFailure { source: selected } = transition {
            // Count cannot exceed this retained slice's length. Visit the entire
            // slice even after mismatch/duplicate, just like the original collect.
            count += 1;
            same &= selected == source;
        }
    }
    count == 1 && same
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::bounded_output::with_limit_usage_refusal;
    use crate::test_allocation_observer::measure;
    fn source(text: &str) -> StatusSourceId {
        StatusSourceId {
            expression: ExpressionId::from_owned(text.to_owned()),
            lane: crate::cleanup_plan::StatusLane::OperationFailure,
        }
    }
    fn original(transitions: &[CleanupTransition], source: &StatusSourceId) -> bool {
        let selected = transitions
            .iter()
            .filter_map(|transition| match transition {
                CleanupTransition::SelectFailure { source } => Some(source),
                _ => None,
            })
            .collect::<Vec<_>>();
        selected.as_slice() == [source]
    }
    #[test]
    fn complete_ordered_selection_keeps_original_empty_duplicate_foreign_and_unicode_truth() {
        let a = source("status.λ\0🙂");
        let b = source("status.λ\0🙃");
        for list in [
            vec![],
            vec![a.clone()],
            vec![b.clone()],
            vec![a.clone(), a.clone()],
            vec![a.clone(), b.clone()],
            vec![b.clone(), a.clone()],
        ] {
            let transitions: Vec<_> = list
                .into_iter()
                .map(|source| CleanupTransition::SelectFailure { source })
                .collect();
            assert_eq!(unique_status(&transitions, &a), original(&transitions, &a));
        }
    }
    #[test]
    fn ordered_selection_acquires_no_backing_at_zero_or_sticky_and_preserves_first_refusal() {
        let a = source("status.λ\0🙂");
        let transitions = vec![CleanupTransition::SelectFailure { source: a.clone() }];
        for sticky in [false, true] {
            let (((), actual), refused, paid, first) = with_limit_usage_refusal(0, || {
                measure(|| {
                    if sticky {
                        assert!(!crate::bounded_output::reserve_active_required(1));
                    }
                    assert!(unique_status(&transitions, &a));
                })
            });
            assert!(!actual.invalid && actual.allocated == 0 && actual.live == 0);
            assert_eq!(paid, 0);
            assert_eq!(refused, sticky);
            if sticky {
                assert_eq!(first.unwrap().requested, 1);
            } else {
                assert!(first.is_none());
            }
        }
    }
}
