//! Test-hook isolation only; these tests make no publication concurrency claim.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};

use super::{run_test_after_create, set_test_after_create};

#[test]
fn another_thread_cannot_consume_the_installing_threads_one_shot_hook() {
    let calls = Arc::new(AtomicUsize::new(0));
    let phases = Arc::new(Barrier::new(2));
    let owner_calls = Arc::clone(&calls);
    let owner_phases = Arc::clone(&phases);
    let owner = std::thread::spawn(move || {
        let callback_calls = Arc::clone(&owner_calls);
        set_test_after_create(Box::new(move || {
            callback_calls.fetch_add(1, Ordering::SeqCst);
        }));
        owner_phases.wait(); // The other thread cannot run before installation.
        owner_phases.wait(); // It must finish both attempts before the owner runs.
        let before = owner_calls.load(Ordering::SeqCst);
        run_test_after_create();
        let first = owner_calls.load(Ordering::SeqCst);
        run_test_after_create();
        (before, first, owner_calls.load(Ordering::SeqCst))
    });
    let other_calls = Arc::clone(&calls);
    let other = std::thread::spawn(move || {
        phases.wait();
        run_test_after_create();
        run_test_after_create();
        let observed = other_calls.load(Ordering::SeqCst);
        phases.wait();
        observed
    });
    // Assertions happen after both barrier participants finish, so a failed
    // isolation observation cannot strand the other thread at a barrier.
    let other_observed = other.join().unwrap();
    let owner_observed = owner.join().unwrap();
    assert_eq!(other_observed, 0);
    assert_eq!(owner_observed, (0, 1, 1));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn callback_can_install_the_next_one_shot_hook_without_retaining_the_slot_borrow() {
    let observations = std::thread::spawn(|| {
        let first = Arc::new(AtomicUsize::new(0));
        let second = Arc::new(AtomicUsize::new(0));
        let callback_first = Arc::clone(&first);
        let callback_second = Arc::clone(&second);
        set_test_after_create(Box::new(move || {
            callback_first.fetch_add(1, Ordering::SeqCst);
            set_test_after_create(Box::new(move || {
                callback_second.fetch_add(1, Ordering::SeqCst);
            }));
        }));
        let mut observed = Vec::new();
        for _ in 0..3 {
            run_test_after_create();
            observed.push((first.load(Ordering::SeqCst), second.load(Ordering::SeqCst)));
        }
        observed
    })
    .join()
    .unwrap();
    assert_eq!(observations, [(1, 0), (1, 1), (1, 1)]);
}
