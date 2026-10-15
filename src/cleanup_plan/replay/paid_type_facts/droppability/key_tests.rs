//! Raw, strict and capped formatter events of the outer identity key. The
//! unpaid comparisons use a payer with no physical ledger to isolate the original
//! logical frontier; they do not claim any unpriced comparison is closed. Physical
//! funding is exercised separately with the real caller payer through Drop.
use super::super::{
    tests::{fixtures, payer},
    ReplayBudget,
};
use super::tests::SemanticPayer;
use super::text::Storage;
use crate::bounded_output::{
    active_refused, budgeted_format, set_active_floor, with_limit_usage_refusal, CappedString,
};
use crate::hir::{DeclarationId, ResolvedType, TypeFactsPayer};
use crate::test_allocation_observer::{current_metrics, measure};
use std::cell::Cell;
use std::fmt::{self, Write as _};

/// Counts formatter progress: 1 on entry, +10 only if the first write continued.
struct Pieces<'a>(&'a Cell<usize>);
impl fmt::Display for Pieces<'_> {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.set(self.0.get() + 1);
        output.write_str("aaaaa")?;
        self.0.set(self.0.get() + 10);
        output.write_str("b")
    }
}

#[test]
fn strict_capped_and_raw_events_match_their_original_sinks() {
    for (limit, floor) in [(10, 6), (10, 0), (5, 0), (6, 0), (0, 0), (17, 0)] {
        let calls = Cell::new(0);
        let old = with_limit_usage_refusal(limit, || {
            assert!(set_active_floor(floor));
            budgeted_format(format_args!("{}", Pieces(&calls)))
        });
        let old_calls = calls.replace(0);
        let new = with_limit_usage_refusal(limit, || {
            assert!(set_active_floor(floor));
            let mut payer = SemanticPayer;
            let mut storage = Storage {
                payer: &mut payer,
                first: None,
            };
            let text = storage
                .strict_text(format_args!("{}", Pieces(&calls)))
                .unwrap();
            assert!(storage.first.is_none());
            text
        });
        assert_eq!(old, new, "strict at {limit}/{floor}");
        assert_eq!(old_calls, calls.replace(0), "strict formatter events");

        let old = with_limit_usage_refusal(limit, || {
            assert!(set_active_floor(floor));
            let mut output = CappedString::new();
            write!(&mut output, "{}", Pieces(&calls)).unwrap();
            output.into_string()
        });
        let old_calls = calls.replace(0);
        let new = with_limit_usage_refusal(limit, || {
            assert!(set_active_floor(floor));
            let mut payer = SemanticPayer;
            let mut storage = Storage {
                payer: &mut payer,
                first: None,
            };
            let mut output = storage.encoded().unwrap();
            storage
                .append(&mut output, format_args!("{}", Pieces(&calls)))
                .unwrap();
            output.into_string()
        });
        assert_eq!(old, new, "capped at {limit}/{floor}");
        assert_eq!(old_calls, calls.replace(0), "capped formatter events");
        // Capped declines are suppressed: the formatter always runs to the end.
        assert_eq!(old_calls, 11);
    }
    // Raw text debits no logical sink at all, even under a zero budget.
    let calls = Cell::new(0);
    let (text, refused, used, receipt) = with_limit_usage_refusal(0, || {
        let mut payer = SemanticPayer;
        let mut storage = Storage {
            payer: &mut payer,
            first: None,
        };
        storage.text(format_args!("{}", Pieces(&calls))).unwrap()
    });
    assert_eq!(
        (text.as_str(), refused, used, receipt),
        ("aaaaab", false, 0, None)
    );
    assert_eq!(calls.get(), 11);
    let old_calls = Cell::new(0);
    let original = with_limit_usage_refusal(0, || {
        let mut raw = String::new();
        write!(&mut raw, "{}", Pieces(&old_calls)).unwrap();
        raw
    });
    assert_eq!((text, refused, used, receipt), original);
    assert_eq!(calls.get(), old_calls.get());
}

#[test]
fn every_original_key_shape_matches_across_logical_limits() {
    let id = DeclarationId::new("scan.type");
    let argumented = ResolvedType::Nominal {
        declaration: id.clone(),
        arguments: vec![ResolvedType::I64, ResolvedType::ArrayU8(12)],
    };
    let types = [
        ResolvedType::Nominal {
            declaration: id.clone(),
            arguments: vec![],
        },
        ResolvedType::ArrayU8(12),
        ResolvedType::TypeParameter {
            owner: id.clone(),
            index: 2,
        },
        argumented.clone(),
        ResolvedType::Function {
            parameters: vec![ResolvedType::I64, argumented],
            result: Box::new(ResolvedType::TypeParameter {
                owner: id,
                index: 2,
            }),
        },
    ];
    for ty in &types {
        for limit in 0..=160 {
            let old = with_limit_usage_refusal(limit, || ty.identity_key());
            let new = with_limit_usage_refusal(limit, || {
                super::key::identity(ty, &mut SemanticPayer).unwrap()
            });
            assert_eq!(old, new, "{ty:?} at {limit}");
        }
    }
}

#[test]
fn real_payer_key_funding_exact_short_zero_and_sticky_through_drop() {
    let program = fixtures().into_iter().next().unwrap();
    let function = &program.functions[0];
    let ty = ResolvedType::Function {
        parameters: vec![
            ResolvedType::I64,
            ResolvedType::Nominal {
                declaration: DeclarationId::new("fund.type"),
                arguments: vec![ResolvedType::Bytes, ResolvedType::ArrayU8(3)],
            },
        ],
        result: Box::new(ResolvedType::Bool),
    };
    let expected = ty.identity_key();
    let observe = |limit: usize| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let result = super::key::identity(&ty, &mut payer(function, &mut budget));
                let matches = result.as_ref().is_ok_and(|key| *key == expected);
                if active_refused() {
                    let before = current_metrics();
                    assert!(payer(function, &mut budget).bytes(0).is_err());
                    assert_eq!(before.allocated, current_metrics().allocated);
                }
                drop(result);
                drop(budget);
                matches
            })
        })
    };
    let ((ok, wide), refused, used, receipt) = observe(usize::MAX);
    assert!(ok && !refused && receipt.is_none() && used > 0);
    assert!(!wide.invalid && wide.allocated > 0 && wide.live == 0);
    assert_eq!(wide.allocated, wide.freed);
    assert!(wide.allocated <= used && wide.highwater <= used);
    let ((ok, exact), refused, charged, _) = observe(used);
    assert!(ok && !refused && charged == used && !exact.invalid && exact.live == 0);
    assert_eq!(exact.allocated, exact.freed);
    assert_eq!(exact, wide, "same real requests and Drop at exact funding");
    let ((ok, short), refused, _, receipt) = observe(used - 1);
    assert!(!ok && refused && receipt.is_some() && !short.invalid && short.live == 0);
    assert_eq!(short.allocated, short.freed);
    let ((ok, zero), refused, charged, receipt) = observe(0);
    assert!(!ok && refused && receipt.is_some() && charged == 0);
    assert!(!zero.invalid && zero.allocated == 0 && zero.live == 0);
}
