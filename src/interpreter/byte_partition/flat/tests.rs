use super::{PartitionIterator, PartitionVec};
use crate::interpreter::byte_partition::{authority::LoanCapacity, loans::Loans, tests::account};
use std::alloc::Layout;

fn fixture() -> crate::hir::ResolvedProgram {
    let checked = crate::check(
        "module partition.flat; @id(\"flat.main\") fn main()->i64 {let owned=bytes_zeroed(1usize);let view=bytes_as_slice(owned);0}",
        "partition-flat.spx",
    ).unwrap();
    crate::hir::resolve(&checked).unwrap()
}
#[test]
fn paid_array_moves_into_iterator_and_returned_child_survives_container_cleanup() {
    let program = fixture();
    let authority = LoanCapacity::checked(&program, &program.functions[0]).unwrap();
    let probe = account(usize::MAX);
    let mut receiver = Loans::new(authority, probe.clone()).unwrap();
    let loan_charge = probe.charged();
    let short = account(loan_charge + Layout::new::<PartitionVec>().size() - 1);
    let short_authority = LoanCapacity::checked(&program, &program.functions[0]).unwrap();
    let mut short_receiver = Loans::new(short_authority, short.clone()).unwrap();
    let original = vec![4].into_boxed_slice();
    let pointer = original.as_ptr();
    let (error, original) = PartitionVec::partition(&mut short_receiver, original, 1, 1)
        .err()
        .unwrap();
    assert!(matches!(
        error,
        crate::construction_fusion::partition::Preparation::Authority(
            crate::construction_fusion::domain::Refusal::Metadata
        )
    ));
    assert_eq!(original.as_ptr(), pointer);
    assert_eq!(short.charged(), loan_charge);
    assert!(PartitionVec::partition(&mut short_receiver, original, 1, 1).is_err());
    assert_eq!(short.charged(), loan_charge); // sticky retry does not allocate or spend
    let value = crate::interpreter::byte_partition::values::partition(
        &mut receiver,
        vec![1, 2, 3].into_boxed_slice(),
        3,
        1,
    )
    .ok()
    .unwrap();
    let crate::interpreter::Value::PartitionVec(mut vector) = value else {
        panic!("checked partition changed its flat Value carrier");
    };
    assert_eq!((vector.len(), vector.capacity()), (3, 3));
    let mut returned = vector.take_last(&mut receiver).unwrap().unwrap();
    let before = probe.charged();
    let value = crate::interpreter::byte_partition::values::into_iterator(
        crate::interpreter::Value::PartitionVec(vector),
        &mut receiver,
    )
    .ok()
    .unwrap();
    let crate::interpreter::Value::PartitionIter(mut iterator) = value else {
        panic!("checked consuming conversion changed its iterator Value carrier");
    };
    assert_eq!(
        probe.charged() - before,
        Layout::new::<PartitionIterator>().size()
    );
    assert_eq!(iterator.remaining(), 2);
    let mut first = iterator.next(&mut receiver).unwrap().unwrap();
    iterator.settle(&mut receiver).unwrap();
    assert_eq!(iterator.remaining(), 0);
    assert!(iterator.next(&mut receiver).unwrap().is_none());
    receiver.store(&mut returned, 0, &[9]).unwrap();
    receiver.store(&mut first, 0, &[7]).unwrap();
    assert_eq!(&*receiver.copy_out_and_settle(&mut returned).unwrap(), &[9]);
    assert_eq!(&*receiver.copy_out_and_settle(&mut first).unwrap(), &[7]);
}
#[test]
fn live_child_loan_refuses_clear_before_any_leaf_and_cleanup_remains_available() {
    let program = fixture();
    let function = &program.functions[0];
    let authority = LoanCapacity::checked(&program, function).unwrap();
    let mut receiver = Loans::new(authority, account(usize::MAX)).unwrap();
    let mut vector = PartitionVec::partition(&mut receiver, vec![1, 2, 3].into_boxed_slice(), 3, 1)
        .ok()
        .unwrap();
    let loan = &function.loan_plan.loans[0];
    while function.loan_plan.endpoints[receiver.endpoint() as usize].point != loan.start {
        let edge = function
            .loan_plan
            .edges
            .iter()
            .find(|edge| edge.from == receiver.endpoint())
            .unwrap();
        receiver.advance(edge.to).unwrap();
    }
    let view = receiver.borrow(vector.owner(1).unwrap(), loan.id).unwrap();
    assert_eq!(receiver.read(view, |bytes| bytes[0]).unwrap(), 2);
    assert_eq!(
        vector.clear(&mut receiver),
        Err(crate::construction_fusion::domain::Refusal::Borrowed)
    );
    assert_eq!(
        vector.settle(&mut receiver),
        Err(crate::construction_fusion::domain::Refusal::Borrowed)
    );
    assert_eq!(vector.len(), 3);
    assert!(receiver.extent(vector.owner(0).unwrap()).is_err());
    receiver.abandon_views().unwrap();
    vector.settle(&mut receiver).unwrap();
    assert_eq!(vector.len(), 0);
}
