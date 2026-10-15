//! Independent boundary controls for the legacy `DeclarationId::new`.
//!
//! The legacy constructor is explicitly unfunded and infallible. These
//! controls observe what it actually does with an arbitrary `Into<String>`:
//! the conversion runs once inside the call, may allocate scratch and excess
//! capacity the constructor cannot know in advance, and the call never
//! charges, refuses or reopens any ledger. Expected totals are computed from
//! capacities read off the live owners and an independent owned-String
//! oracle, not from the constructor. They claim no funded migration, no
//! closure and no application fit.
use super::{CompilerDeclaration, DeclarationId};
use crate::bounded_output::{
    active_remaining, reserve_active_required, with_limit_usage, with_limit_usage_refusal,
};
use crate::test_allocation_observer::{current_metrics, measure, Metrics};
use std::cell::Cell;

/// (text, extra scratch capacity, extra output capacity).
const CASES: [(&str, usize, usize); 8] = [
    ("", 0, 0),
    ("", 3, 9),
    ("app.main", 0, 0),
    ("app.main", 13, 40),
    ("app.main\0forged", 64, 1),
    ("λ.函数.main", 100, 17),
    ("🦀e\u{301}", 8, 200),
    ("x", 0, 4095),
];

#[derive(Default)]
struct Record {
    calls: Cell<usize>,
    scratch_capacity: Cell<usize>,
    scratch_sum: Cell<u64>,
    string_capacity: Cell<usize>,
}

/// An arbitrary opaque conversion: it allocates a scratch buffer larger than
/// the payload, reads it back, then returns a `String` with excess capacity.
struct Probe<'a> {
    text: &'a str,
    scratch_extra: usize,
    spare: usize,
    record: &'a Record,
}

impl From<Probe<'_>> for String {
    fn from(probe: Probe<'_>) -> String {
        let record = probe.record;
        record.calls.set(record.calls.get() + 1);
        let mut scratch = Vec::<u8>::with_capacity(probe.text.len() + probe.scratch_extra);
        scratch.extend_from_slice(probe.text.as_bytes());
        record.scratch_capacity.set(scratch.capacity());
        record
            .scratch_sum
            .set(scratch.iter().map(|byte| u64::from(*byte)).sum());
        std::hint::black_box(&scratch);
        let mut output = String::with_capacity(probe.text.len() + probe.spare);
        output.push_str(std::str::from_utf8(&scratch).unwrap());
        record.string_capacity.set(output.capacity());
        // `scratch` is released here, before the String is handed back.
        output
    }
}

/// Independent of the constructor: two counters, the bytes, padded to usize.
fn expected_backing(len: usize) -> usize {
    let counters = 2 * std::mem::size_of::<usize>();
    let align = std::mem::align_of::<usize>();
    (counters + len).div_ceil(align) * align
}

fn oracle_sum(text: &str) -> u64 {
    text.bytes().map(u64::from).sum()
}

fn allocating(bytes: usize) -> usize {
    usize::from(bytes > 0)
}

fn construct(text: &str, extra: usize, spare: usize) -> (DeclarationId, Record, Metrics) {
    let record = Record::default();
    let (id, observed) = measure(|| {
        DeclarationId::new(Probe {
            text,
            scratch_extra: extra,
            spare,
            record: &record,
        })
    });
    (id, record, observed)
}

#[test]
fn arbitrary_conversion_runs_once_and_its_scratch_and_excess_capacity_are_observable() {
    for (text, extra, spare) in CASES {
        let oracle = text.to_owned();
        let (id, record, observed) = construct(text, extra, spare);
        let scratch = record.scratch_capacity.get();
        let string = record.string_capacity.get();
        let backing = expected_backing(oracle.len());
        assert_eq!(record.calls.get(), 1);
        assert!(!observed.invalid);
        assert_eq!(observed.reallocations, 0);
        // Capacities are read from the live owners, never normalized.
        assert!(scratch >= oracle.len() + extra);
        assert!(string >= oracle.len() + spare);
        if extra > 0 {
            assert!(scratch > oracle.len());
        }
        if spare > 0 {
            assert!(string > oracle.len());
        }
        // The scratch really held the payload bytes.
        assert_eq!(record.scratch_sum.get(), oracle_sum(&oracle));
        assert_eq!(
            observed.allocations,
            allocating(scratch) + allocating(string) + 1
        );
        assert_eq!(observed.allocated, scratch + string + backing);
        assert_eq!(observed.freed, scratch + string);
        assert_eq!(observed.live, backing);
        assert_eq!(observed.highwater, (scratch + string).max(string + backing));
        assert_eq!(id.as_str().as_bytes(), oracle.as_bytes());
        assert_eq!(id.as_str().len(), oracle.len());
        assert_eq!(id.owned_allocation_bytes(), backing);
    }
}

#[test]
fn excess_capacity_changes_the_scratch_total_but_never_the_shared_backing() {
    let text = "app.main";
    let mut previous: Option<(usize, usize)> = None;
    for spare in [0usize, 1, 64, 4096] {
        let (id, record, observed) = construct(text, 0, spare);
        let string = record.string_capacity.get();
        assert!(string >= text.len() + spare);
        assert_eq!(
            observed.allocated,
            record.scratch_capacity.get() + string + id.owned_allocation_bytes()
        );
        // The retained backing depends on the text length alone.
        assert_eq!(id.owned_allocation_bytes(), expected_backing(text.len()));
        if let Some((previous_string, previous_allocated)) = previous {
            assert!(string > previous_string);
            assert!(observed.allocated > previous_allocated);
        }
        previous = Some((string, observed.allocated));
    }
}

#[test]
fn full_drop_returns_every_scratch_temporary_and_backing_to_the_allocator() {
    for (text, extra, spare) in CASES {
        let oracle = text.to_owned();
        let record = Record::default();
        let backing = expected_backing(oracle.len());
        let (settled, observed) = measure(|| {
            let id = DeclarationId::new(Probe {
                text,
                scratch_extra: extra,
                spare,
                record: &record,
            });
            let live = current_metrics();
            // Scratch and the converted String are already released.
            assert_eq!(live.live, id.owned_allocation_bytes());
            let copies = [id.clone(), id.clone(), id.clone()];
            assert_eq!(current_metrics(), live);
            for copy in &copies {
                assert_eq!(copy.shared_allocation_key(), id.shared_allocation_key());
            }
            drop(copies);
            // Dropping clones frees nothing while the last owner lives.
            assert_eq!(current_metrics(), live);
            drop(id);
            current_metrics()
        });
        let scratch = record.scratch_capacity.get();
        let string = record.string_capacity.get();
        assert_eq!(record.calls.get(), 1);
        assert!(!observed.invalid);
        assert_eq!(settled.live, 0);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, scratch + string + backing);
        assert_eq!(observed.freed, observed.allocated);
        assert_eq!(observed.freed, settled.freed);
    }
}

#[test]
fn owned_string_payload_is_consumed_unchanged_and_released_after_the_copy() {
    for (text, _, spare) in CASES {
        let oracle = text.to_owned();
        let backing = expected_backing(oracle.len());

        // Owner created before observation: only the constructor's own
        // request is visible, and the payload bytes are copied unchanged.
        let mut owned = String::with_capacity(oracle.len() + spare);
        owned.push_str(text);
        assert!(owned.capacity() >= oracle.len() + spare);
        let (before, observed) = measure(|| DeclarationId::new(owned));
        assert!(!observed.invalid);
        assert_eq!(observed.allocations, 1);
        assert_eq!(observed.allocated, backing);
        assert_eq!(observed.highwater, backing);
        assert_eq!(observed.live, backing);
        assert_eq!(before.as_str().as_bytes(), oracle.as_bytes());

        // Owner created inside observation: its excess capacity is real
        // allocation, overlaps the backing, and is freed after the copy.
        let ((inside, capacity), observed) = measure(|| {
            let mut owned = String::with_capacity(oracle.len() + spare);
            owned.push_str(text);
            let capacity = owned.capacity();
            (DeclarationId::new(owned), capacity)
        });
        assert!(capacity >= oracle.len() + spare);
        assert!(!observed.invalid);
        assert_eq!(observed.allocations, allocating(capacity) + 1);
        assert_eq!(observed.allocated, capacity + backing);
        assert_eq!(observed.freed, capacity);
        assert_eq!(observed.highwater, capacity + backing);
        assert_eq!(observed.live, backing);
        assert_eq!(inside, before);
        assert_eq!(inside.as_str().as_bytes(), oracle.as_bytes());
    }
}

#[test]
fn legacy_borrowed_text_adds_a_temporary_string_that_paid_borrowed_does_not() {
    for (text, _, _) in CASES {
        let backing = expected_backing(text.len());
        let (legacy, observed) = measure(|| DeclarationId::new(text));
        assert_eq!(observed.allocations, allocating(text.len()) + 1);
        assert_eq!(observed.allocated, text.len() + backing);
        assert_eq!(observed.freed, text.len());
        assert_eq!(observed.highwater, text.len() + backing);
        assert_eq!(observed.live, backing);

        let requested = Cell::new(usize::MAX);
        let (paid, observed) = measure(|| {
            DeclarationId::paid_borrowed(text, |bytes| {
                requested.set(bytes);
                Ok::<(), ()>(())
            })
        });
        let paid = paid.unwrap();
        assert_eq!(requested.get(), backing);
        assert_eq!(observed.allocations, 1);
        assert_eq!(observed.allocated, backing);
        assert_eq!(observed.freed, 0);
        assert_eq!(observed.live, backing);
        assert_eq!(paid, legacy);
    }
}

#[test]
fn legacy_construction_neither_charges_nor_reopens_nor_refuses_any_ledger() {
    let text = "λ.函数.main\0tail";
    let backing = expected_backing(text.len());

    // Zero ledger: legacy construction succeeds, charges nothing and does
    // not refuse. A zero-byte request is still admitted afterwards.
    let record = Record::default();
    let (((id, observed), still_open, remaining), overflowed, used, refusal) =
        with_limit_usage_refusal(0, || {
            let constructed = measure(|| {
                DeclarationId::new(Probe {
                    text,
                    scratch_extra: 11,
                    spare: 5,
                    record: &record,
                })
            });
            (constructed, reserve_active_required(0), active_remaining())
        });
    assert!(still_open);
    assert_eq!(remaining, Some(0));
    assert!(!overflowed);
    assert_eq!(used, 0);
    assert!(refusal.is_none());
    assert_eq!(record.calls.get(), 1);
    assert_eq!(observed.live, backing);
    assert_eq!(id.as_str(), text);

    // Positive ledger: remaining balance is untouched by legacy allocation.
    let ((id, before, after), overflowed, used) = with_limit_usage(1 << 20, || {
        let before = active_remaining();
        let id = DeclarationId::new(text);
        (id, before, active_remaining())
    });
    assert_eq!(before, Some(1 << 20));
    assert_eq!(after, before);
    assert!(!overflowed);
    assert_eq!(used, 0);
    assert_eq!(id.as_str(), text);

    // Pre-refused ledger: the first sticky refusal survives, legacy builds a
    // valid identity anyway, and nothing is reopened. A funded constructor
    // on the same ledger still refuses.
    let ((id, first, paid_refused, retry), overflowed, used, refusal) =
        with_limit_usage_refusal(1 << 10, || {
            let first = reserve_active_required(2 << 10);
            let id = DeclarationId::new(text);
            let paid = DeclarationId::paid_borrowed(text, |bytes| {
                if reserve_active_required(bytes) {
                    Ok(())
                } else {
                    Err(())
                }
            });
            (id, first, paid.is_err(), reserve_active_required(0))
        });
    assert!(!first);
    assert!(paid_refused);
    assert!(!retry);
    assert!(overflowed);
    assert_eq!(used, 0);
    assert_eq!(
        refusal,
        Some(crate::bounded_output::ReservationRefusal {
            requested: 2 << 10,
            remaining: 1 << 10,
            floor: 0,
            stage: None,
        })
    );
    assert_eq!(id.as_str(), text);
}

#[test]
fn compiler_static_identity_stays_free_and_independent_of_legacy_and_refused_ledgers() {
    let ((statics, observed, retry), overflowed, used, refusal) =
        with_limit_usage_refusal(0, || {
            let first = reserve_active_required(1);
            assert!(!first);
            let (statics, observed) = measure(|| {
                let id = DeclarationId::compiler(CompilerDeclaration::Vec);
                drop(id.clone());
                id
            });
            (statics, observed, reserve_active_required(0))
        });
    assert!(overflowed);
    assert!(!retry);
    assert_eq!(used, 0);
    assert_eq!(
        refusal,
        Some(crate::bounded_output::ReservationRefusal {
            requested: 1,
            remaining: 0,
            floor: 0,
            stage: None,
        })
    );
    assert!(!observed.invalid);
    assert_eq!(observed.allocations, 0);
    assert_eq!(observed.allocated, 0);
    assert_eq!(observed.highwater, 0);
    assert_eq!(observed.live, 0);
    assert!(statics.is_compiler_static());
    assert_eq!(statics.owned_allocation_bytes(), 0);
    assert_eq!(statics.shared_allocation_key(), None);

    // The legacy constructor for the same spelling does allocate, is not
    // static, and is interchangeable by value; neither affects the other.
    let legacy = DeclarationId::new(crate::prelude::VEC_ID);
    assert!(!legacy.is_compiler_static());
    assert!(legacy.shared_allocation_key().is_some());
    assert_eq!(
        legacy.owned_allocation_bytes(),
        expected_backing(crate::prelude::VEC_ID.len())
    );
    assert_eq!(statics, legacy);
    assert_eq!(statics.as_str(), crate::prelude::VEC_ID);
}
