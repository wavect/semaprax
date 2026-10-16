//! Complete Main8fb capacity-witness receiver and all three original controls.
use super::*;

fn capacity_witness_receiving_bytes() -> usize {
    use std::mem::size_of;
    size_of::<String>()
        + 3 * size_of::<usize>()
        + 2 * size_of::<&str>()
        + size_of::<Option<usize>>()
        + size_of::<Result<(), std::collections::TryReserveError>>()
        + 2 * size_of::<bool>()
}

/// A causal witness supplements the exact authoring repair instead of replacing
/// it. If optional composition cannot be funded, retain the complete witness
/// and the original primary diagnostic, as the previous receiver did.
pub(super) fn with_capacity_witness(mut diagnostic: Diagnostic, witness: String) -> Diagnostic {
    let repair = diagnostic.help.as_deref().filter(|help| {
        diagnostic.code == "SPX-T269"
            && Some(*help) == authoring_repair_help(diagnostic.code, &diagnostic.message)
    });
    let Some(repair) = repair else {
        return diagnostic.with_help(witness);
    };
    let length = witness
        .len()
        .checked_add(2)
        .and_then(|n| n.checked_add(repair.len()));
    let payment = length.and_then(|n| n.checked_add(capacity_witness_receiving_bytes()));
    let (Some(length), Some(payment)) = (length, payment) else {
        return diagnostic.with_help(witness);
    };
    if !crate::bounded_output::reserve_active_preserving(payment, 0) {
        return diagnostic.with_help(witness);
    }
    let mut composed = String::new();
    if composed.try_reserve_exact(length).is_err() || composed.capacity() != length {
        return diagnostic.with_help(witness);
    }
    composed.push_str(&witness);
    composed.push_str("\n\n");
    composed.push_str(repair);
    diagnostic.help = Some(composed);
    diagnostic
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publication_witness_keeps_exact_repair_and_primary_at_optional_limits() {
        use crate::bounded_output::with_limit_usage_refusal;
        let original = with_authoring_repair_help(Diagnostic::io_static(
            "SPX-T269",
            "stdout_write path reaches 2 sites; limit is 1",
        ));
        let witness = "call path: app.main -> app.write; first and second stdout sites";
        let expected = format!("{witness}\n\n{}", original.help.as_deref().unwrap());
        let exact = expected.len() + capacity_witness_receiving_bytes();
        for limit in [0, exact - 1, exact] {
            let input = original.clone();
            let proof = witness.to_owned();
            let (actual, refused, used, first) =
                with_limit_usage_refusal(limit, || with_capacity_witness(input, proof));
            assert!(!refused && first.is_none());
            if limit == exact {
                assert_eq!(used, exact);
                assert_eq!(actual, original.clone().with_help(&expected));
                assert!(actual.help.as_deref().unwrap().starts_with(witness));
            } else {
                assert_eq!(used, 0);
                assert_eq!(actual, original.clone().with_help(witness));
            }
        }
    }

    #[test]
    fn publication_witness_never_replaces_a_sticky_refusal_or_invents_a_repair() {
        use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal};
        let original = with_authoring_repair_help(Diagnostic::io_static(
            "SPX-T269",
            "stderr_write path reaches 2 sites; limit is 1",
        ));
        let expected = original.clone().with_help("original witness");
        let witness = "original witness".to_owned();
        let (actual, refused, used, first) = with_limit_usage_refusal(0, || {
            assert!(!reserve_active_required(1));
            with_capacity_witness(original, witness)
        });
        assert!(refused);
        assert_eq!(used, 0);
        assert_eq!(first.unwrap().requested, 1);
        assert_eq!(actual, expected);
        let unrelated = Diagnostic::io_static("SPX-T269", "stdout_write expects Slice<u8>")
            .with_help("existing unrelated help");
        assert_eq!(
            with_capacity_witness(unrelated.clone(), "witness".to_owned()),
            unrelated.with_help("witness")
        );
    }

    #[test]
    fn publication_composition_pays_all_backing_and_complete_drop() {
        use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal};
        use crate::test_allocation_observer::measure;
        let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let diagnostic = with_authoring_repair_help(Diagnostic::io_static(
                    "SPX-T269",
                    "stdout_write path reaches 2 sites; limit is 1",
                ));
                let witness = "original causal witness";
                assert!(reserve_active_required(witness.len()));
                let diagnostic = with_capacity_witness(diagnostic, witness.to_owned());
                assert!(diagnostic.help.as_deref().unwrap().starts_with(witness));
                assert!(diagnostic
                    .help
                    .as_deref()
                    .unwrap()
                    .contains("publish once per stream"));
                drop(diagnostic);
            })
        });
        assert!(!refused && first.is_none());
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        assert!(allocation.allocated <= paid);
    }
}
