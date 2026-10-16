use super::KeyDirectory;
use crate::bounded_output::{
    active_remaining, reserve_active_required, with_limit, with_limit_usage,
    with_limit_usage_refusal,
};
use crate::test_allocation_observer::measure;
use std::mem::size_of;

#[test]
fn real_directory_zero_exact_short_and_sticky_owners_settle_through_drop() {
    let authored = [1usize, 2];
    let bytes = size_of::<KeyDirectory<'_, usize>>() + 5 * size_of::<usize>();
    for limit in [0, bytes - 1, bytes] {
        let (((), allocation), refused, used, receipt) = with_limit_usage_refusal(limit, || {
            measure(|| {
                let acquired = KeyDirectory::allocate(&authored, 3);
                if limit == bytes {
                    let mut owner = acquired.unwrap();
                    for value in [3, 4, 5] {
                        owner.push(value).unwrap();
                    }
                    let pointer = owner.as_ptr();
                    owner.validate(&authored).unwrap();
                    let values = owner.finish(&authored).unwrap();
                    assert_eq!(values, [1, 2, 3, 4, 5]);
                    assert_eq!(pointer, values.as_ptr());
                    assert_eq!(active_remaining(), Some(0));
                    drop(values);
                } else {
                    assert!(acquired.is_err());
                    assert!(!reserve_active_required(0));
                    assert!(KeyDirectory::allocate(&authored, 3).is_err());
                }
            })
        });
        assert_eq!(refused, limit < bytes);
        assert_eq!(receipt.is_some(), refused);
        assert_eq!(used, if refused { 0 } else { bytes });
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        assert!(allocation.allocated <= used && allocation.highwater <= used);
        if refused {
            assert_eq!(allocation.allocations, 0);
        }
    }
}

#[test]
fn paid_directory_rejects_detached_source_and_backing_without_refund() {
    let source = [11usize];
    let other = [11usize];
    for detached in [false, true] {
        let (_, refused, used, receipt) = with_limit_usage_refusal(4096, || {
            let mut owner = KeyDirectory::allocate(&source, 1).unwrap();
            owner.push(12).unwrap();
            let before = active_remaining();
            if detached {
                let (_, child_refused) = with_limit(4096, || {
                    assert!(owner.validate(&source).is_err());
                    assert!(owner.push(13).is_err());
                });
                assert!(!child_refused);
            } else {
                assert!(owner.validate(&other).is_err());
            }
            assert_eq!(before, active_remaining());
            assert!(!reserve_active_required(0));
            assert!(owner.validate(&source).is_err());
            assert!(owner.finish(&source).is_err());
        });
        assert!(refused && receipt.is_some());
        assert_eq!(
            used,
            size_of::<KeyDirectory<'_, usize>>() + 2 * size_of::<usize>()
        );
    }
    // These are actual independent paid owners, not external replacement Vecs.
    // Each terminal transfer consumes its sole owner. Even allocator address
    // reuse afterward cannot reconstruct a seal or attach an arbitrary Vec.
    let (_, refused) = with_limit(4096, || {
        let mut first = KeyDirectory::allocate(&source, 1).unwrap();
        first.push(12).unwrap();
        let old_pointer = first.as_ptr();
        drop(first.into_vec());
        let mut second = KeyDirectory::allocate(&source, 1).unwrap();
        second.push(12).unwrap();
        let new_pointer = second.as_ptr();
        second.validate(&source).unwrap();
        let values = second.finish(&source).unwrap();
        assert_eq!(values, [11, 12]);
        assert_eq!(new_pointer, values.as_ptr());
        let _address_may_be_reused = old_pointer == new_pointer;
        drop(values);
    });
    assert!(!refused);
    let source = include_str!("../key_directory.rs");
    assert!(source.contains("values: Vec<T>"));
    assert!(source.contains("fn validate(&self, authored: &[T])"));
    assert!(source.contains("fn finish(mut self, authored: &[T])"));
    assert!(!source.contains("DerefMut"));
    assert!(source
        .lines()
        .filter(|line| !line.starts_with("//!"))
        .all(|line| !line.contains("&mut Vec")));
}

#[test]
fn generated_count_and_sticky_push_cannot_grow_or_publish_after_refusal() {
    let source = [1usize];
    let (((), allocation), refused, used) = with_limit_usage(4096, || {
        measure(|| {
            let mut owner = KeyDirectory::allocate(&source, 1).unwrap();
            let before = active_remaining();
            owner.push(2).unwrap();
            let pointer = owner.as_ptr();
            assert!(owner.push(3).is_err());
            assert_eq!(&*owner, &[2]);
            assert_eq!(owner.as_ptr(), pointer);
            assert_eq!(active_remaining(), before);
            assert!(owner.finish(&source).is_err());
        })
    });
    assert!(refused && !allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    assert!(allocation.allocated <= used && allocation.highwater <= used);
}
