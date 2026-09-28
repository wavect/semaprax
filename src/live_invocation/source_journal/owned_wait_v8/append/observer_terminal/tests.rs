//! Genuine failed Decision observation ACK. No State release or terminal claim.
use super::*;
use crate::agent_runtime::AgentCancellation;
use crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::test_observer_failed_receipt;
use crate::live_invocation::{InvocationClock, SourceInvocationClock};
use crate::resumable_effects::CapabilityPolicy;
struct Clock;
impl InvocationClock for Clock {
    fn now_millis(&self) -> i64 {
        1
    }
}
impl SourceInvocationClock for Clock {
    fn clock_domain(&self) -> &str {
        "owned.wait.test"
    }
}

#[test]
fn owned_observer_terminal_expected_normal_poison_refusal_does_not_retire_live_seal() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|c, l, k| {
        let c = c.with_initialization(&l).unwrap();
        let j = SourceOwnedWaitJournalV8::open(Arc::new(c), k, l).unwrap();
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        test_observer_failed_receipt(&j, &cancel, &policy, &Clock, |owner, weak| {
            owner.validate_live().unwrap();
            assert!(matches!(j.hold(), Err(SourceJournalError::Poisoned)));
            assert!(matches!(
                j.begin_session(),
                Err(SourceJournalError::Poisoned)
            ));
            owner.validate_live().unwrap();
            assert!(j.poisoned.get());
            assert!(!j.poisoned.retired.get());
            assert_eq!(weak[0].strong_count(), 1);
            assert!(weak[1].upgrade().is_none());
            drop(owner);
            assert!(j.poisoned.retired.get());
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    });
}

#[cfg(unix)]
#[test]
fn owned_observer_terminal_detected_pin_loss_then_restore_never_revives_state_work() {
    use std::os::unix::fs::MetadataExt;
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime_store(true, |c, l, k, directory| {
        let c = c.with_initialization(&l).unwrap();
        let j = SourceOwnedWaitJournalV8::open(Arc::new(c), k, l).unwrap();
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        test_observer_failed_receipt(&j, &cancel, &policy, &Clock, |owner, weak| {
            owner.validate_live().unwrap();
            let pin = j.context.registration().identity();
            let matches: Vec<_> = std::fs::read_dir(directory)
                .unwrap()
                .map(|e| e.unwrap().path())
                .filter(|p| {
                    std::fs::metadata(p).is_ok_and(|m| {
                        m.is_file() && m.dev() == pin.file_device && m.ino() == pin.file_inode
                    })
                })
                .collect();
            assert_eq!(
                matches.len(),
                1,
                "select only actual retained journal identity"
            );
            let entry = &matches[0];
            let moved = directory.join("observer-original-entry");
            let before = j.lease.try_borrow_mut().unwrap().read().unwrap();
            std::fs::rename(entry, &moved).unwrap();
            std::fs::write(entry, b"replacement").unwrap();
            assert!(owner.validate_live().is_err());
            assert!(j.poisoned.retired.get());
            std::fs::remove_file(entry).unwrap();
            std::fs::rename(&moved, entry).unwrap();
            let state_work = Cell::new(0usize);
            if owner.validate_live().is_ok() {
                state_work.set(state_work.get() + 1);
            }
            assert_eq!(state_work.get(), 0);
            assert!(owner.validate_live().is_err());
            assert_eq!(j.lease.try_borrow_mut().unwrap().read().unwrap(), before);
            assert_eq!(weak[0].strong_count(), 1);
            assert!(weak[1].upgrade().is_none());
            assert!(j.hold().is_err());
            assert!(j.begin_session().is_err());
            drop(owner);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    });
}
#[test]
fn owned_observer_terminal_general_quarantine_retires_even_already_poisoned_seal() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|c, l, k| {
        let c = c.with_initialization(&l).unwrap();
        let j = SourceOwnedWaitJournalV8::open(Arc::new(c), k, l).unwrap();
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        test_observer_failed_receipt(&j, &cancel, &policy, &Clock, |owner, weak| {
            owner.validate_live().unwrap();
            j.quarantine();
            assert!(owner.validate_live().is_err());

            assert!(j.poisoned.get());
            assert!(j.poisoned.retired.get());
            assert_eq!(weak[0].strong_count(), 1);
            drop(owner);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    });
}
