use super::{ContextSlot, Core, Issuer, ModuleIdentity, Registry, State};
use crate::construction_fusion::domain::owner_core::Child;
use crate::interpreter::byte_partition::{
    authority::LoanCapacity,
    loans::Loans,
    tests::{account, Probe},
};
use std::alloc::Layout;
fn fixture() -> crate::hir::ResolvedProgram {
    let source = crate::check(
        r#"module partition.whole;
        @id("partition.whole.main") fn main()->i64 {
            let bytes=bytes_zeroed(2usize);let view=bytes_as_slice(bytes);0
        }"#,
        "partition-whole.spx",
    )
    .unwrap();
    crate::hir::resolve(&source).unwrap()
}
#[test]
fn whole_views_end_on_exact_edge_before_original_backing_moves() {
    let program = fixture();
    let function = &program.functions[0];
    let probe = account(usize::MAX);
    let mut receiver = Loans::new(
        LoanCapacity::checked(&program, function).unwrap(),
        probe.clone(),
    )
    .unwrap();
    let original = vec![7, 0].into_boxed_slice();
    let pointer = original.as_ptr();
    let mut owner = receiver.hold_whole(original).unwrap();
    let loan = &function.loan_plan.loans[0];
    while function.loan_plan.endpoints[receiver.endpoint() as usize].point != loan.start {
        let next = function
            .loan_plan
            .edges
            .iter()
            .find(|edge| edge.from == receiver.endpoint())
            .unwrap()
            .to;
        receiver.advance(next).unwrap();
    }
    let view = receiver.borrow(&owner, loan.id).unwrap();
    assert_eq!(
        receiver
            .read(view, |bytes| <[u8; 2]>::try_from(bytes).unwrap())
            .unwrap(),
        [7, 0]
    );
    // Use actual CFG termination, not node-union kills or a fabricated Borrow plan.
    loop {
        let (index, edge) = function
            .loan_plan
            .edges
            .iter()
            .enumerate()
            .find(|(_, edge)| edge.from == receiver.endpoint())
            .unwrap();
        let last = loan.end_edges.contains(&(index as u16));
        receiver.advance(edge.to).unwrap();
        if last {
            break;
        }
    }
    let paid = probe.charged();
    let original = receiver.take_whole(&mut owner).unwrap();
    assert_eq!(original.as_ptr(), pointer);
    assert_eq!(original.as_ref(), &[7, 0]);
    assert_eq!(probe.charged(), paid);
    assert!(receiver.extent(&owner).is_err());
    assert!(receiver
        .read(view, |_| panic!("expired whole view"))
        .is_err());
    drop(receiver);
    drop(original);
}
#[test]
fn whole_metadata_has_independent_exact_short_and_children_cannot_mint_whole() {
    let program = fixture();
    let capacity = LoanCapacity::checked(&program, &program.functions[0]).unwrap();
    type R = Probe;
    let units = Core::<R>::whole_test_table_units();
    let required = Layout::new::<Registry<R>>().size()
        + Layout::new::<Vec<ContextSlot<R>>>().size()
        + Layout::array::<ContextSlot<R>>(1).unwrap().size()
        + Layout::new::<State<R>>().size()
        + Layout::new::<Core<'static, R>>().size()
        + units[0].size()
        + units[1].size().checked_mul(capacity.slots()).unwrap()
        + Layout::array::<Child<'static>>(1).unwrap().size()
        + units[2].size()
        + units[3].size()
        + Layout::new::<Vec<Child<'static>>>().size()
        + Layout::new::<Issuer>().size()
        + Layout::new::<ModuleIdentity>().size();
    let probe = account(required);
    let mut registry = Registry::new(probe.clone()).unwrap();
    let original = vec![7, 0].into_boxed_slice();
    let pointer = original.as_ptr();
    let mut owner = registry.hold_whole(original, &capacity).unwrap();
    assert_eq!(probe.charged(), required);
    let original = registry.take_whole(&mut owner).unwrap();
    assert_eq!(original.as_ptr(), pointer);
    drop(original);
    drop(registry);

    let probe = account(required - 1);
    let mut registry = Registry::new(probe.clone()).unwrap();
    let original = vec![7, 0].into_boxed_slice();
    let pointer = original.as_ptr();
    let (refused, observed) =
        crate::test_allocation_observer::measure(|| registry.hold_whole(original, &capacity));
    let (_, original) = refused.unwrap_err();
    assert_eq!(original.as_ptr(), pointer);
    assert_eq!(
        observed,
        crate::test_allocation_observer::Metrics::default()
    );
    let before = probe.charged();
    assert!(registry.hold_whole(original, &capacity).is_err());
    assert_eq!(probe.charged(), before);

    let probe = account(usize::MAX);
    let mut registry = Registry::new(probe).unwrap();
    let mut children = registry
        .partition(vec![7, 0].into_boxed_slice(), 1, 2)
        .unwrap();
    assert!(
        registry.take_whole(&mut children[0]).is_err(),
        "even a single full-range partition child is not whole-source authority"
    );
    registry.settle(&mut children[0]).unwrap();

    // Empty whole ownership is different from semantic partition width zero.
    let probe = account(usize::MAX);
    let mut registry = Registry::new(probe).unwrap();
    let mut empty = registry
        .hold_whole(Vec::<u8>::new().into_boxed_slice(), &capacity)
        .unwrap();
    assert_eq!(registry.extent(&empty).unwrap(), 0);
    let view = registry.borrow(&empty).unwrap();
    assert_eq!(registry.read(&view, <[u8]>::len).unwrap(), 0);
    let mut view = view;
    registry.end_view(&mut view).unwrap();
    assert!(registry.take_whole(&mut empty).unwrap().is_empty());
}
