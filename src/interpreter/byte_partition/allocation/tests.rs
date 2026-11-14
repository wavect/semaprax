//! Receiving controls only; these do not claim complete source execution.
use super::*;
use crate::interpreter::byte_partition::{funding::Account, invocation::Invocation};
use crate::interpreter::Value;
use std::{alloc::Layout, collections::BTreeMap};

fn headers() -> usize {
    Layout::new::<Account<'_>>().size()
        + Layout::new::<super::super::funding::Handle<'_, '_>>().size()
        + Layout::new::<BorrowedAccount<'_>>().size()
}
#[test]
fn borrowed_handles_share_actual_counters_and_exact_one_short_backing() {
    let (mut events, mut payload) = (0, 0);
    {
        let account = Account::funded(&mut events, &mut payload, headers() + 7).unwrap();
        let first = account.borrowed();
        let second = account.borrowed();
        let ((), observed) = crate::test_allocation_observer::measure(|| {
            let (id, original) = first.allocate_zeroed(3).unwrap();
            assert_eq!(id, 1);
            let (id, copied) = second.copy(&[7, 0, 9, 4]).unwrap();
            assert_eq!(id, 2);
            assert_ne!(original.as_ptr(), copied.as_ptr());
            assert_eq!(first.counters(), (2, 7));
            assert_eq!(copied.as_ref(), &[7, 0, 9, 4]);
            assert!(matches!(
                first.allocate_zeroed(1),
                Err(AllocationError::Authority(_))
            ));
            assert!(second.allocate_zeroed(0).is_err());
            assert_eq!(second.counters(), (2, 7));
            drop(copied);
            drop(original);
        });
        assert!(!observed.invalid);
        assert_eq!((observed.allocations, observed.reallocations), (2, 0));
        assert_eq!(
            (
                observed.allocated,
                observed.freed,
                observed.highwater,
                observed.live
            ),
            (7, 7, 7, 0)
        );
    }
    assert_eq!((events, payload), (2, 7));
    let (mut events, mut payload) = (0, 0);
    {
        let account = Account::funded(&mut events, &mut payload, headers() + 6).unwrap();
        let first = account.borrowed();
        first.allocate_zeroed(3).unwrap();
        assert!(first.copy(&[7, 0, 9, 4]).is_err());
        assert_eq!(first.counters(), (1, 3));
        assert!(first.allocate_zeroed(0).is_err());
    }
    assert_eq!((events, payload), (1, 3));
}
#[test]
fn empty_backings_count_and_logical_failure_is_distinct_from_funding_refusal() {
    let (mut events, mut payload) = (0, 0);
    {
        let account = Account::funded(&mut events, &mut payload, headers()).unwrap();
        let first = account.borrowed();
        let second = account.borrowed();
        for expected in 1..=crate::byte_data_capacity::MAX_BYTES_COPY_SITES {
            assert_eq!(first.allocate_zeroed(0).unwrap().0, expected);
        }
        assert!(matches!(
            second.allocate_zeroed(0),
            Err(AllocationError::Logical)
        ));
        assert!(first.logical_refused());
        assert_eq!(first.counters(), (32, 0));
    }
    assert_eq!((events, payload), (32, 0));
    let (mut events, mut payload) = (0, 0);
    {
        let account = Account::funded(&mut events, &mut payload, headers()).unwrap();
        let handle = account.borrowed();
        assert!(matches!(
            handle.allocate_zeroed(1),
            Err(AllocationError::Authority(_))
        ));
        assert!(!handle.logical_refused());
        assert_eq!(handle.counters(), (0, 0));
    }
}
#[test]
fn evaluator_account_reads_ignore_legacy_fields_and_whole_handoff_keeps_pointer() {
    let source = crate::parse(
        r#"module partition.account;
        @id("partition.account.split") fn split()->Vec<Bytes> {
            let whole=bytes_zeroed(6usize);bytes_partition(whole,2usize,3usize)
        }
        @id("partition.account.main") fn main()->i64 {0}"#,
        "partition-account.spx",
    )
    .unwrap();
    let owner =
        crate::hir::byte_partition::ValidatedPartitionProgram::compile_source(&source, &mut {
            usize::MAX
        })
        .unwrap();
    let program = owner.program();
    let entry = &program.functions[0];
    assert_eq!(entry.id.as_str(), "partition.account.split");
    assert!(matches!(
        entry.return_type,
        crate::hir::ResolvedType::Nominal { .. }
    ));
    assert_ne!(entry.id, program.entrypoint);
    let functions = program
        .functions
        .iter()
        .map(|function| (function.id.as_str(), function))
        .collect::<BTreeMap<_, _>>();
    let (mut events, mut payload) = (0, 0);
    {
        // Explicit test funding, not a production entry policy or forecast grant.
        let account = Account::funded(&mut events, &mut payload, usize::MAX).unwrap();
        let mut evaluator = crate::interpreter::Evaluator::new_prepared(
            crate::interpreter::FunctionLookup::Borrowed(&functions),
            BTreeMap::new(),
            &program.declarations,
            100,
            0,
            crate::interpreter::PreparedCancellation::Never,
        );
        assert_eq!(evaluator.byte_allocation_count(), 0);
        evaluator.partition = Some(
            Invocation::prepare(&owner, entry, &mut { usize::MAX }, account.borrowed()).unwrap(),
        );
        assert!(std::ptr::eq(
            evaluator.partition.as_ref().unwrap().entry(),
            entry
        ));
        assert!(evaluator.partition.as_ref().unwrap().simultaneous_views() <= 256);
        evaluator.next_byte_allocation = 31; // deliberately stale legacy storage
        evaluator.allocated_byte_payload = 2_097_152;
        let whole = evaluator.zeroed_bytes(6).unwrap();
        assert_eq!(evaluator.byte_allocation_count(), 1);
        let Backing::Whole(bytes) = &whole.bytes else {
            panic!("direct boxed whole")
        };
        let pointer = bytes.bytes.as_ptr();
        assert!(whole.borrow_alias().is_err());
        let clone = evaluator.clone_accounted_bytes(&whole).unwrap();
        assert_eq!(evaluator.byte_allocation_count(), 2);
        assert_ne!(clone.bytes.as_ref().as_ptr(), pointer);
        let slice = BorrowedSliceValue::whole(
            crate::hir::ValueId::intrinsic_parameter("partition.account.array", 0),
            Arc::<[u8]>::from([7, 0, 9].as_slice()),
        );
        let copied = evaluator.copy_bytes(&slice).unwrap();
        assert_eq!(copied.bytes.as_ref(), &[7, 0, 9]);
        assert_eq!(evaluator.byte_allocation_count(), 3);
        drop(copied);
        let original_count = account.borrowed().counters();
        let receiver = &mut evaluator.partition.as_mut().unwrap().loans;
        let (error, whole) =
            super::super::values::partition_whole(receiver, whole, 2, 4).unwrap_err();
        assert!(matches!(
            error,
            crate::construction_fusion::partition::Preparation::Fault(_)
        ));
        assert_eq!(whole.bytes.as_ref().as_ptr(), pointer);
        use super::super::values;
        let value = values::partition_whole(receiver, whole, 2, 3).unwrap();
        let Value::PartitionIter(mut iter) = values::into_iterator(value, receiver).unwrap() else {
            panic!("partition iterator");
        };
        let mut child = iter.next_bytes(receiver).unwrap().unwrap();
        let sibling = iter.next_bytes(receiver).unwrap().unwrap();
        assert_eq!((child.allocation, sibling.allocation), (1, 1));
        iter.settle(receiver).unwrap();
        for index in [3, u64::MAX] {
            let fault = values::store_byte(&mut child, receiver, index, 99).unwrap_err();
            assert!(matches!(fault, Flow::Failure(status)
                if status == crate::interpreter::owned_buffer::normalize_byte_buffer(
                    crate::byte_ops::SET_INDEX_OUT_OF_BOUNDS_CODE)));
        }
        let Backing::Partition(unchanged) = &child.bytes else {
            panic!("staged owner");
        };
        assert_eq!(receiver.extent(unchanged).unwrap(), 3);
        let (_, measured) = crate::test_allocation_observer::measure(|| {
            values::store_byte(&mut child, receiver, 2, 99).unwrap();
        });
        assert_eq!(
            measured,
            crate::test_allocation_observer::Metrics::default()
        );
        let Backing::Partition(updated) = &child.bytes else {
            panic!("renewed owner");
        };
        assert_eq!(receiver.deep_clone(updated).unwrap().as_ref(), &[0, 0, 99]);
        let Backing::Partition(sibling_owner) = &sibling.bytes else {
            panic!("sibling owner");
        };
        assert_eq!(
            receiver.deep_clone(sibling_owner).unwrap().as_ref(),
            &[0, 0, 0]
        );
        let mut sibling = Value::Bytes(sibling);
        values::settle(&mut sibling, receiver).unwrap();
        let mut child = Value::Bytes(child);
        values::settle(&mut child, receiver).unwrap();
        assert_eq!(
            account.borrowed().counters(),
            (original_count.0 + 2, original_count.1 + 6)
        );
        drop(clone);
        // A logical ID alone cannot authorize a whole from another account.
        let (mut foreign_events, mut foreign_payload) = (0, 0);
        let foreign =
            Account::funded(&mut foreign_events, &mut foreign_payload, headers() + 1).unwrap();
        let foreign_value = super::whole(
            foreign.borrowed(),
            foreign.borrowed().allocate_zeroed(1).unwrap(),
        );
        assert!(evaluator.clone_accounted_bytes(&foreign_value).is_err());
        assert!(evaluator.zeroed_bytes(0).is_err());
        // No ordinary source evaluator entry was opened by preparing this receiver.
        assert!(crate::check(
            r#"module closed; @id("closed.main")
            fn main()->Vec<Bytes>{
             let bytes=bytes_zeroed(1usize);bytes_partition(bytes,1usize,1usize)
            }"#,
            "closed.spx"
        )
        .is_err());
    }
    assert_eq!((events, payload), (5, 21));
}
