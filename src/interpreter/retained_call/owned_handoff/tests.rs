use super::*;
use std::cell::{Cell, RefCell};
use std::sync::{Arc, Weak};

const SOURCE: &str = include_str!("../../../kernel_zero/rung_two_owned_handoff/handoff.spx");
const ENTRY: &str = "kernel-zero.owned-handoff.move";

thread_local! {
    static STAGED: Cell<usize> = const { Cell::new(0) };
    static OBSERVE_LIFETIME: Cell<bool> = const { Cell::new(false) };
    static PANIC: Cell<bool> = const { Cell::new(false) };
    static RETAIN: Cell<bool> = const { Cell::new(false) };
    static RETAINED: RefCell<Option<Arc<[u8]>>> = const { RefCell::new(None) };
    static OBSERVER: RefCell<Option<Weak<[u8]>>> = const { RefCell::new(None) };
    static OBSERVED_THREAD: RefCell<Option<std::thread::ThreadId>> = const { RefCell::new(None) };
}

// A weak witness retains the Arc allocation after its bytes have dropped.
// Acquire that test instrumentation only for explicit lifetime assertions.
struct LifetimeObservation;
fn observe_lifetime() -> LifetimeObservation {
    assert!(!OBSERVE_LIFETIME.with(|enabled| enabled.replace(true)));
    LifetimeObservation
}
impl Drop for LifetimeObservation {
    fn drop(&mut self) {
        OBSERVE_LIFETIME.with(|enabled| enabled.set(false));
        OBSERVER.with(|slot| drop(slot.borrow_mut().take()));
    }
}

pub(super) fn after_staging(observer: &Weak<[u8]>) {
    assert!(
        observer.upgrade().is_some(),
        "the negative control must see a real live owner"
    );
    STAGED.with(|count| count.set(count.get() + 1));
    OBSERVED_THREAD.with(|thread| *thread.borrow_mut() = Some(std::thread::current().id()));
    if OBSERVE_LIFETIME.with(Cell::get) {
        OBSERVER.with(|slot| *slot.borrow_mut() = Some(observer.clone()));
    }
    if RETAIN.with(|retain| retain.replace(false)) {
        RETAINED.with(|slot| *slot.borrow_mut() = Some(observer.upgrade().unwrap()));
    }
    PANIC.with(|panic| assert!(!panic.replace(false), "injected after owner staging"));
}

pub(super) fn staged_count() -> usize {
    STAGED.with(Cell::get)
}
pub(super) fn panic_on_next_staging() {
    PANIC.with(|panic| panic.set(true));
}

fn program(source: &str) -> hir::ResolvedProgram {
    let parsed = crate::parse(source, "owned-handoff.spx").unwrap();
    hir::resolve(&parsed).unwrap()
}

fn prepared() -> PreparedOwnedHandoff {
    PreparedOwnedHandoff::new(program(SOURCE), ENTRY).unwrap()
}

#[test]
fn synchronous_handoff_transfers_and_releases_one_actual_owner() {
    let _lifetime = observe_lifetime();
    let prepared = prepared();
    for input in [b"".as_slice(), b"x", b"abcdefghijklmnopqrst"] {
        let before = STAGED.with(Cell::get);
        let result = prepared.execute(input, MAX_FUEL).unwrap();
        assert!(result.released);
        assert_eq!(
            result.evaluation.cleanup_events,
            [OwnedDataCleanupEvent::CopyOutAndSettleBytes]
        );
        assert_eq!(STAGED.with(Cell::get), before + 1);
        assert_eq!(
            OBSERVER.with(|slot| slot.borrow().as_ref().unwrap().strong_count()),
            0
        );
        assert_eq!(
            OBSERVED_THREAD.with(|thread| *thread.borrow()),
            Some(std::thread::current().id())
        );
        let legacy = evaluate_retained_call(
            &prepared.program,
            &prepared.prepared,
            &[RetainedValue::Bytes(input.to_vec())],
            MAX_FUEL,
        )
        .unwrap();
        assert_eq!(
            result.evaluation, legacy,
            "factoring must preserve legacy evaluation facts"
        );
        assert_eq!(result.into_bytes().unwrap(), input);
    }
}

#[test]
fn bounds_refuse_before_staging_and_exhaustion_cleans_without_publication() {
    let _lifetime = observe_lifetime();
    let prepared = prepared();
    let before = STAGED.with(Cell::get);
    for (bytes, fuel) in [
        (b"x".as_slice(), 0),
        (b"x", MAX_FUEL + 1),
        (&[7; 21], MAX_FUEL),
    ] {
        assert!(prepared.execute(bytes, fuel).is_err());
    }
    assert_eq!(
        STAGED.with(Cell::get),
        before,
        "refusals must allocate no owner"
    );
    let exhausted = prepared.execute(b"abc", 1).unwrap();
    assert_eq!(
        exhausted.evaluation.outcome,
        RetainedCallOutcome::FuelExhausted
    );
    assert!(exhausted.released);
    assert!(
        exhausted.evaluation.cleanup_events.is_empty(),
        "a failed return cannot invent a harvest event"
    );
    assert_eq!(
        OBSERVER.with(|slot| slot.borrow().as_ref().unwrap().strong_count()),
        0
    );
    assert!(exhausted.into_bytes().is_err());
    assert_eq!(
        prepared
            .execute(b"abc", MAX_FUEL)
            .unwrap()
            .into_bytes()
            .unwrap(),
        b"abc"
    );
}

#[test]
fn ownership_body_depth_and_receipt_substitution_cannot_publish() {
    let _lifetime = observe_lifetime();
    let before = STAGED.with(Cell::get);
    for source in [
        SOURCE.replace("let next = input;\n    next", "input"),
        SOURCE.replace("let next = input;", "let next = { input };"),
        SOURCE.replace(
            "let next = input;",
            "let next = bytes_copy(bytes_as_slice(input));",
        ),
        format!("{SOURCE}\n@id(\"hostile.record\") record Hostile {{ @id(\"hostile.field\") value: i64, }}\n"),
    ] {
        let error = PreparedOwnedHandoff::new(program(&source), ENTRY).err().unwrap();
        assert_eq!(error[0].code, "SPX-F105");
    }
    assert!(PreparedOwnedHandoff::new(program(SOURCE), "missing.entry").is_err());
    assert_eq!(STAGED.with(Cell::get), before);
    let prepared = prepared();
    RETAIN.with(|retain| retain.set(true));
    let retained = prepared.execute(b"retained-owner", MAX_FUEL).err().unwrap();
    assert_eq!(retained[0].code, "SPX-F105");
    assert_eq!(
        OBSERVER.with(|slot| slot.borrow().as_ref().unwrap().strong_count()),
        1
    );
    RETAINED.with(|slot| drop(slot.borrow_mut().take()));
    assert_eq!(
        OBSERVER.with(|slot| slot.borrow().as_ref().unwrap().strong_count()),
        0
    );
    let mut missing_receipt = prepared.execute(b"abc", MAX_FUEL).unwrap();
    missing_receipt.released = false;
    assert!(missing_receipt.into_bytes().is_err());
    let mut missing_settlement = prepared.execute(b"abc", MAX_FUEL).unwrap();
    missing_settlement.evaluation.cleanup_events.clear();
    assert!(missing_settlement.into_bytes().is_err());
}

#[test]
fn panic_drops_the_staged_owner_and_shallow_handoff_runs_on_two_mib_stack() {
    // The test alone creates a thread; the production seam remains synchronous.
    std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            let _lifetime = observe_lifetime();
            let prepared = prepared();
            PANIC.with(|panic| panic.set(true));
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = prepared.execute(b"non-palindrome", MAX_FUEL);
            }));
            assert!(panic.is_err());
            assert_eq!(
                OBSERVER.with(|slot| slot.borrow().as_ref().unwrap().strong_count()),
                0
            );
            assert_eq!(
                prepared
                    .execute(b"reentry", MAX_FUEL)
                    .unwrap()
                    .into_bytes()
                    .unwrap(),
                b"reentry"
            );
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn inactive_lifetime_observation_releases_complete_handoff_backing() {
    let prepared = prepared();
    assert!(!OBSERVE_LIFETIME.with(Cell::get));
    let (_, observed) = crate::test_allocation_observer::measure(|| {
        drop(prepared.execute(b"abc", MAX_FUEL).unwrap());
    });
    assert!(!observed.invalid);
    assert!(observed.allocated > 0);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
    OBSERVER.with(|slot| assert!(slot.borrow().is_none()));
}
