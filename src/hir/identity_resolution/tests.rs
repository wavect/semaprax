use super::*;
use crate::test_allocation_observer::measure;

const SOURCE: &str = r#"module identity.paid;
@id("range.type") record Range {
    @id("range.lo") lo:i64,
    @id("range.hi") hi:i64,
} requires lo <= hi
@id("app.main") fn main()->i64 {
    let row = Range { lo:1, hi:2 };
    row.hi
}
"#;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Stop(usize);
fn backing(text: &str) -> usize {
    let word = std::mem::size_of::<usize>();
    (2 * word + text.len()).div_ceil(word) * word
}

#[test]
fn both_invariant_passes_keep_exact_identity_refusal_and_partial_index_drop() {
    let program = crate::check(SOURCE, "identity-paid.spx").unwrap();
    // Independent source identities include the actual compiler-generated
    // invariant and check functions, with the original source order.
    const FIRST: &[&str] = &[
        "range.type",
        "core.option",
        "core.result",
        "app.main",
        "range.type#invariant",
        "range.lo",
        "range.hi",
        "core.option.none",
        "core.option.some",
        "core.option.some.value",
        "core.result.ok",
        "core.result.ok.value",
        "core.result.err",
        "core.result.err.error",
    ];
    const SECOND: &[&str] = &[
        "range.type",
        "core.option",
        "core.result",
        "app.main",
        "range.type#invariant",
        "range.type#check",
        "range.lo",
        "range.hi",
        "core.option.none",
        "core.option.some",
        "core.option.some.value",
        "core.result.ok",
        "core.result.ok.value",
        "core.result.err",
        "core.result.err.error",
    ];
    let requests: Vec<_> = FIRST
        .iter()
        .chain(SECOND)
        .map(|text| backing(text))
        .collect();
    let old = super::super::resolve_verified(&program, None).unwrap();
    let mut seen = 0;
    let current = resolve(&program, None, |bytes| {
        assert_eq!(bytes, requests[seen]);
        seen += 1;
        Ok::<_, Stop>(())
    })
    .unwrap();
    assert_eq!(seen, 29);
    assert_eq!(current.0, old.0);
    assert_eq!(current.1.reused(), old.1.reused());
    for stop in [0, 13, 14, 28] {
        let mut seen = 0;
        let (error, observed) = measure(|| {
            resolve(&program, None, |bytes| {
                assert_eq!(bytes, requests[seen]);
                let at = seen;
                seen += 1;
                if at == stop {
                    Err(Stop(at))
                } else {
                    Ok(())
                }
            })
            .err()
            .unwrap()
        });
        assert!(matches!(error, Failure::Identity(Stop(at)) if at == stop));
        assert_eq!(seen, stop + 1);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
        assert!(!observed.invalid);
    }
    // The original source-verifier error wins before any identity request.
    let mut malformed = program.clone();
    malformed.functions[0].stable_id.clear();
    let old = super::super::resolve_with_function_reuse(&malformed, None, None, None, None)
        .err()
        .unwrap();
    let current = resolve(&malformed, None, |_| -> Result<(), Stop> {
        panic!("request after verifier refusal")
    })
    .err()
    .unwrap();
    let Failure::Source(current) = current else {
        panic!("source error remapped")
    };
    assert_eq!(current, old);
}

const PLAIN: &str = r#"module identity.plain;
@id("plain.type") record Pair {
    @id("plain.a") a:i64,
    @id("plain.b") b:i64,
}
@id("plain.helper") fn helper(value:i64)->i64 { value + 1 }
@id("plain.main") fn main()->i64 {
    let pair = Pair { a:1, b:2 };
    helper(pair.b)
}
"#;

#[test]
fn paid_checked_and_borrowed_siblings_match_legacy_requests_and_body_reuse() {
    let program = crate::check(PLAIN, "identity-plain.spx").unwrap();
    let (resolved, work) =
        super::super::resolve_with_function_reuse(&program, None, None, None, None).unwrap();
    let mut checked_requests = Vec::new();
    let checked = resolve_checked(&program, None, None, None, None, |bytes| {
        checked_requests.push(bytes);
        Ok::<_, Stop>(())
    })
    .unwrap();
    let mut borrowed_requests = Vec::new();
    let borrowed = resolve_source(&program, None, |bytes| {
        borrowed_requests.push(bytes);
        Ok::<_, Stop>(())
    })
    .unwrap();
    assert!(!checked_requests.is_empty());
    assert_eq!(checked_requests, borrowed_requests);
    assert_eq!(checked.0, resolved);
    assert_eq!(borrowed.0, resolved);
    assert_eq!(
        super::super::resolve_source(&program, None).unwrap().0,
        resolved
    );
    assert_eq!(
        (checked.1.reused(), checked.1.static_text()),
        (work.reused(), work.static_text())
    );
    // A prior exact checked module reuses function bodies; the index is still
    // built and paid in the same SourceIndex-then-DeclarationIndex order.
    let costs = work.costs();
    let legacy = super::super::resolve_with_function_reuse(
        &program,
        Some(&program),
        Some(&resolved),
        Some(&costs),
        None,
    )
    .unwrap();
    let mut reuse_requests = Vec::new();
    let paid = resolve_checked(
        &program,
        Some(&program),
        Some(&resolved),
        Some(&costs),
        None,
        |bytes| {
            reuse_requests.push(bytes);
            Ok::<_, Stop>(())
        },
    )
    .unwrap();
    assert_eq!(reuse_requests, checked_requests);
    assert_eq!(paid.0, legacy.0);
    assert_eq!(paid.1.reused(), legacy.1.reused());
    assert!(paid.1.reused() > 0);
}

#[test]
fn paid_siblings_refuse_initial_middle_and_final_identity_with_the_exact_caller_error() {
    let program = crate::check(PLAIN, "identity-plain.spx").unwrap();
    let mut count = 0;
    resolve_checked(&program, None, None, None, None, |_| {
        count += 1;
        Ok::<_, Stop>(())
    })
    .unwrap();
    assert!(count > 2);
    for stop in [0, count / 2, count - 1] {
        for borrowed in [false, true] {
            let mut seen = 0;
            let mut reserve = |_bytes: usize| {
                let at = seen;
                seen += 1;
                if at == stop {
                    Err(Stop(at))
                } else {
                    Ok(())
                }
            };
            let (error, observed) = measure(|| {
                if borrowed {
                    resolve_source(&program, None, &mut reserve).err().unwrap()
                } else {
                    resolve_checked(&program, None, None, None, None, &mut reserve)
                        .err()
                        .unwrap()
                }
            });
            assert!(matches!(error, Failure::Identity(Stop(at)) if at == stop));
            assert_eq!(seen, stop + 1);
            assert_eq!(observed.live, 0);
            assert_eq!(observed.allocated, observed.freed);
            assert!(!observed.invalid);
        }
    }
}

#[test]
fn paid_borrowed_source_keeps_verifier_and_invariant_refusals_before_identity() {
    let program = crate::check(SOURCE, "identity-paid.spx").unwrap();
    // Valid invariants now use the same two-pass source transformation as the
    // owned receiver. Keep a literal request census and complete owned oracle.
    let expected = super::super::resolve_verified(&program, None).unwrap();
    const IDS: &[&str] = &[
        "range.type",
        "core.option",
        "core.result",
        "app.main",
        "range.type#invariant",
        "range.lo",
        "range.hi",
        "core.option.none",
        "core.option.some",
        "core.option.some.value",
        "core.result.ok",
        "core.result.ok.value",
        "core.result.err",
        "core.result.err.error",
        "range.type",
        "core.option",
        "core.result",
        "app.main",
        "range.type#invariant",
        "range.type#check",
        "range.lo",
        "range.hi",
        "core.option.none",
        "core.option.some",
        "core.option.some.value",
        "core.result.ok",
        "core.result.ok.value",
        "core.result.err",
        "core.result.err.error",
    ];
    assert_eq!(IDS.len(), 29);
    let mut requests = 0;
    let paid = resolve_source(&program, None, |bytes| {
        assert_eq!(bytes, backing(IDS[requests]));
        requests += 1;
        Ok::<_, Stop>(())
    })
    .unwrap();
    assert_eq!(requests, 29);
    assert_eq!(paid.0, expected.0);
    assert_eq!(paid.1.reused(), expected.1.reused());
    // A genuine malformed invariant remains a source refusal before the first
    // identity request; the old valid-invariant exclusion is no longer current.
    let invalid = crate::parse(
        &SOURCE.replace("lo <= hi", "lo"),
        std::path::Path::new("identity-paid.spx"),
    )
    .unwrap();
    let legacy = super::super::resolve_source(&invalid, None).err().unwrap();
    assert!(legacy.iter().any(|error| error.severity.is_error()));
    let Failure::Source(paid) = resolve_source(&invalid, None, |_| -> Result<(), Stop> {
        panic!("identity request before malformed invariant refusal")
    })
    .err()
    .unwrap() else {
        panic!("invariant source refusal remapped")
    };
    assert_eq!(paid, legacy);
    let mut malformed = program.clone();
    malformed.functions[0].stable_id.clear();
    let legacy = super::super::resolve_source(&malformed, None)
        .err()
        .unwrap();
    let Failure::Source(paid) = resolve_source(&malformed, None, |_| -> Result<(), Stop> {
        panic!("identity request after verifier refusal")
    })
    .err()
    .unwrap() else {
        panic!("verifier refusal remapped")
    };
    assert_eq!(paid, legacy);
}

#[test]
fn paid_siblings_keep_unicode_empty_nul_and_duplicate_identity_outcomes() {
    let base = crate::check(PLAIN, "identity-plain.spx").unwrap();
    let duplicate = base.functions[0].stable_id.clone();
    for text in ["\u{e9}\u{2713}.main", "", "nul\0id", duplicate.as_str()] {
        let mut changed = base.clone();
        changed.functions[1].stable_id = text.to_owned();
        let legacy = super::super::resolve_with_function_reuse(&changed, None, None, None, None);
        let paid = resolve_checked(&changed, None, None, None, None, |_| Ok::<_, Stop>(()));
        match (legacy, paid) {
            (Ok(old), Ok(new)) => assert_eq!(old.0, new.0),
            (Err(old), Err(Failure::Source(new))) => assert_eq!(old, new),
            (Err(old), Err(Failure::Semantic(new))) => assert_eq!(old, vec![new]),
            _ => panic!("paid sibling changed the outcome for {text:?}"),
        }
    }
}

#[test]
fn actual_identity_caller_setup_refuses_without_diagnostic_or_report_allocation() {
    let program = crate::check(SOURCE, "identity-paid.spx").unwrap();
    let ((error, observed), failed, used) = crate::bounded_output::with_limit_usage(0, || {
        measure(|| resolve_with_report(&program, None).err().unwrap())
    });
    assert!(matches!(error, CallerFailure::Setup(_)));
    assert!(failed);
    assert_eq!(used, 0);
    assert_eq!(observed.allocations, 0);
    assert_eq!(observed.live, 0);
}

#[test]
fn actual_nonzero_resolver_identity_refusal_transfers_paid_report_and_first_receipt_through_drop() {
    use crate::bounded_output::identity_reservation::IdentityReservation;
    use crate::bounded_output::{
        active_refused, active_remaining, reserve_active_required, with_limit_usage_refusal,
    };
    use crate::diagnostic::Diagnostic;
    use std::mem::size_of;
    let program = crate::check(
        "module identity.caller; @id(\"main\") fn main()->i64 {0}",
        "identity-caller.spx",
    )
    .unwrap();
    // Warm only process-static ordinary tables outside the observation.
    super::super::resolve_verified(&program, None).unwrap();
    // With no authored types, the original first initial identity is Option.
    // Its 11 bytes require two Arc-control words plus padded text, independently.
    assert_eq!(crate::prelude::OPTION_ID, "core.option");
    let word = size_of::<usize>();
    let identity = (2 * word + 11).div_ceil(word) * word;
    let report_bytes = size_of::<IdentityReservation>() + size_of::<Diagnostic>() + 48;
    let limit = report_bytes + identity - 1;
    let (((), observed), failed, used, receipt) = with_limit_usage_refusal(limit, || {
        measure(|| {
            let mut pointers = None;
            let failure = resolve_with_report_observation(&program, None, |owner| {
                pointers = Some(owner.pointers());
            })
            .err()
            .unwrap();
            let CallerFailure::Identity(report) = failure else {
                panic!("nonzero actual identity refusal lost its paid publication")
            };
            let pointers = pointers.unwrap();
            assert_eq!(report.as_ptr(), pointers.0);
            assert_eq!(report[0].message.as_ptr(), pointers.1);
            assert_eq!(report.len(), 1);
            assert_eq!(report[0].code, "SPX-H006");
            assert_eq!(
                report[0].message,
                "declaration identity backing reservation refused"
            );
            assert!(active_refused());
            assert_eq!(active_remaining(), Some(identity - 1));
            assert!(!reserve_active_required(0));
            assert!(!reserve_active_required(1));
            assert_eq!(active_remaining(), Some(identity - 1));
            drop(report);
        })
    });
    assert!(failed);
    assert_eq!(used, report_bytes);
    let receipt = receipt.unwrap();
    assert_eq!(receipt.requested, identity);
    assert_eq!(receipt.remaining, identity - 1);
    assert_eq!(receipt.floor, 0);
    assert!(!observed.invalid);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
    // This whole caller observation includes legacy verifier/index scratch;
    // only report and initial identity backing are claimed prepaid here.
}
