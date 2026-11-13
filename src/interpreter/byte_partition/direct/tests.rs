//! Source-entry controls for the sealed direct interpreter receiver.
use super::{
    execute, verify_partition_fault_cleanup, verify_unallocated_failure, Account, Outcome,
};
use crate::cleanup_plan::{ExitContinuation, StatusLane};
use crate::construction_fusion::{domain::Refusal, partition::Fault};
use crate::hir::byte_partition::ValidatedPartitionProgram;

const SHARED_SOURCE: &str = include_str!("../../../../tests/fixtures/byte-partition-private.spx");

fn source(extent: u64, count: u64, width: u64) -> String {
    format!(
        r#"module partition_direct;
@id("partition_direct.main") fn main()->i64 {{
 let whole=bytes_zeroed({extent}usize);
 let children=bytes_partition(whole,{count}usize,{width}usize);
 i64_from_usize(vec_len<Bytes>(children))
}}
"#
    )
}
#[test]
fn original_4096_source_uses_one_backing_and_canonical_scalar_publication() {
    for (text, extent, count) in [
        (source(12288, 4096, 3), 12288, 4096),
        (source(3, 1, 3), 3, 1),
        (source(0, 0, u64::MAX), 0, 0),
        (SHARED_SOURCE.to_owned(), 65_536, 4096),
    ] {
        assert!(crate::check(&text, "partition-direct.spx").is_err());
        let ast = crate::parse(&text, "partition-direct.spx").unwrap();
        let owner = ValidatedPartitionProgram::compile_source(&ast, &mut { usize::MAX }).unwrap();
        let (mut allocations, mut payload) = (0, 0);
        {
            // Explicit test funding; no forecast-derived production grant.
            let account = Account::funded(&mut allocations, &mut payload, 8_388_608).unwrap();
            for attempt in 1..=3 {
                let report = execute(&owner, &account, &mut { usize::MAX });
                assert_eq!(report.outcome, Outcome::Returned(count as i64));
                assert!(report.cleanup_complete);
                assert_eq!(
                    (report.allocations, report.payload),
                    (attempt, extent * u64::from(attempt))
                );
            }
        }
        assert_eq!((allocations, payload), (3, extent * 3));
    }
}
#[test]
fn semantic_faults_settle_the_original_staged_owner_and_preserve_status() {
    for (count, width, fault) in [
        (4096, 0, Fault::WidthZero),
        (u64::MAX, 2, Fault::ExtentOverflow),
        (4095, 16, Fault::ExtentMismatch),
        (65_536, 1, Fault::MetadataCapacity),
    ] {
        let shared =
            SHARED_SOURCE.replace("4096usize, 16usize", &format!("{count}usize, {width}usize"));
        let ast = crate::parse(&shared, "partition-fault.spx").unwrap();
        let owner = ValidatedPartitionProgram::compile_source(&ast, &mut { usize::MAX }).unwrap();
        let (mut allocations, mut payload) = (0, 0);
        let account = Account::funded(&mut allocations, &mut payload, 8_388_608).unwrap();
        for invocation in 1..=3 {
            let (report, metrics) =
                crate::test_allocation_observer::measure_with_capacity(1 << 16, || {
                    execute(&owner, &account, &mut { usize::MAX })
                });
            assert_eq!(report.outcome, Outcome::PartitionFault(fault));
            assert!(report.cleanup_complete);
            assert_eq!(
                (report.allocations, report.payload),
                (invocation, 65_536 * u64::from(invocation))
            );
            assert!(!metrics.invalid);
            assert!(metrics.allocated >= 65_536);
            assert_eq!(metrics.live, 0);
            assert_eq!(metrics.allocated, metrics.freed);
        }
    }
}

#[test]
fn partition_fault_cleanup_rejects_wrong_or_missing_staged_argument() {
    use crate::hir::{ResolvedExprKind, ResolvedStatement};
    let ast = crate::parse(&source(3, 1, 0), "partition-fault.spx").unwrap();
    let owner = ValidatedPartitionProgram::compile_source(&ast, &mut { usize::MAX }).unwrap();
    let entry = &owner.program().functions[0];
    let ResolvedExprKind::Block { statements, .. } = &entry.body.kind else {
        panic!("expected the checked entry block");
    };
    let ResolvedStatement::Let {
        value: operation, ..
    } = &statements[1]
    else {
        panic!("expected the checked partition binding");
    };
    let ResolvedExprKind::Call { args, .. } = &operation.kind else {
        panic!("expected the checked partition call");
    };
    let mut plan = entry.cleanup_plan.clone();
    let mut remaining = usize::MAX;
    verify_partition_fault_cleanup(&plan, operation, &args[0], &mut remaining).unwrap();
    let mut exact = usize::MAX - remaining;
    let mut short = exact - 1;
    verify_partition_fault_cleanup(&plan, operation, &args[0], &mut exact).unwrap();
    assert_eq!(exact, 0);
    assert_eq!(
        verify_partition_fault_cleanup(&plan, operation, &args[0], &mut short),
        Err(Refusal::Metadata)
    );
    assert_eq!(short, 0);
    assert_eq!(
        verify_partition_fault_cleanup(&plan, operation, &args[1], &mut { usize::MAX }),
        Err(Refusal::Identity)
    );
    let exit = plan
        .exits
        .iter_mut()
        .find(|exit| {
            matches!(&exit.continuation,
            ExitContinuation::ReturnFailure { source }
                if source.expression == operation.id && source.lane == StatusLane::OperationFailure)
        })
        .unwrap();
    exit.finalize_in_order.clear();
    assert_eq!(
        verify_partition_fault_cleanup(&plan, operation, &args[0], &mut { usize::MAX }),
        Err(Refusal::Identity)
    );
}
#[test]
fn unsupported_helper_and_receiving_shortage_refuse_before_payload_effects() {
    let text = source(3, 1, 3);
    let helper = text.replace("bytes_zeroed(3usize)", "allocate()")
        + r#"@id("partition_direct.allocate") fn allocate()->Bytes {bytes_zeroed(3usize)}"#;
    let helper_ast = crate::parse(&helper, "partition-refusal.spx").unwrap();
    assert!(ValidatedPartitionProgram::compile_source(&helper_ast, &mut { usize::MAX }).is_err());
    let extra = text.replace(
        " let children=",
        " let extra=0;
 let children=",
    );
    for (text, remaining) in [(text, 0), (extra, usize::MAX)] {
        let ast = crate::parse(&text, "partition-refusal.spx").unwrap();
        let owner = ValidatedPartitionProgram::compile_source(&ast, &mut { usize::MAX }).unwrap();
        let (mut allocations, mut payload) = (0, 0);
        let account = Account::funded(&mut allocations, &mut payload, 8_388_608).unwrap();
        let report = execute(&owner, &account, &mut { remaining });
        assert_eq!(report.outcome, Outcome::Refused);
        assert!(!report.cleanup_complete);
        assert_eq!((report.allocations, report.payload), (0, 0));
    }
    let ast = crate::parse(&source(3, 1, 3), "partition-unfunded.spx").unwrap();
    let owner = ValidatedPartitionProgram::compile_source(&ast, &mut { usize::MAX }).unwrap();
    let (mut allocations, mut payload) = (0, 0);
    let headers = std::alloc::Layout::new::<Account<'_>>().size()
        + std::alloc::Layout::new::<super::super::funding::Handle<'_, '_>>().size()
        + std::alloc::Layout::new::<super::super::funding::BorrowedAccount<'_>>().size();
    let account = Account::funded(&mut allocations, &mut payload, headers).unwrap();
    let report = execute(&owner, &account, &mut { usize::MAX });
    assert_eq!(report.outcome, Outcome::Refused);
    assert_eq!((report.allocations, report.payload), (0, 0));
}

#[test]
fn bounded_producer_checks_before_partition_and_keeps_the_account_for_reentry() {
    let (mut allocations, mut payload) = (0, 0);
    let account = Account::funded(&mut allocations, &mut payload, 8_388_608).unwrap();
    for (producer, width, outcome, counters) in [
        (
            "bytes_zeroed_bounded(65537usize, 65536usize)",
            "0usize",
            Outcome::BufferBoundExceeded,
            (0, 0),
        ),
        (
            "bytes_zeroed_bounded(65536usize, 65536usize)",
            "16usize",
            Outcome::Returned(4096),
            (1, 65536),
        ),
        (
            "bytes_zeroed_bounded(65536usize, 65536usize)",
            "0usize",
            Outcome::PartitionFault(Fault::WidthZero),
            (2, 131072),
        ),
    ] {
        let source = SHARED_SOURCE
            .replace("bytes_zeroed(65536usize)", producer)
            .replace("4096usize, 16usize", &format!("4096usize, {width}"));
        let ast = crate::parse(&source, "partition-bounded.spx").unwrap();
        let owner = ValidatedPartitionProgram::compile_source(&ast, &mut { usize::MAX }).unwrap();
        let report = execute(&owner, &account, &mut { usize::MAX });
        assert_eq!(report.outcome, outcome);
        assert!(report.cleanup_complete);
        assert_eq!((report.allocations, report.payload), counters);
    }
}

#[test]
fn bounded_producer_requires_one_empty_failure_exit_at_its_own_site() {
    use crate::hir::{ResolvedExprKind, ResolvedStatement};

    let text = SHARED_SOURCE.replace(
        "bytes_zeroed(65536usize)",
        "bytes_zeroed_bounded(65537usize, 65536usize)",
    );
    let ast = crate::parse(&text, "partition-bounded-plan.spx").unwrap();
    let owner = ValidatedPartitionProgram::compile_source(&ast, &mut { usize::MAX }).unwrap();
    let entry = &owner.program().functions[0];
    let ResolvedExprKind::Block { statements, .. } = &entry.body.kind else {
        panic!("expected the checked entry block");
    };
    let [ResolvedStatement::Let {
        value: producer, ..
    }, ResolvedStatement::Let {
        value: partition, ..
    }] = statements.as_slice()
    else {
        panic!("expected the checked producer and partition bindings");
    };
    let plan = &entry.cleanup_plan;
    let mut remaining = usize::MAX;
    verify_unallocated_failure(plan, producer, &mut remaining).unwrap();
    let mut exact = usize::MAX - remaining;
    let mut short = exact - 1;
    verify_unallocated_failure(plan, producer, &mut exact).unwrap();
    assert_eq!(exact, 0);
    assert_eq!(
        verify_unallocated_failure(plan, producer, &mut short),
        Err(Refusal::Metadata)
    );
    assert_eq!(short, 0);
    assert_eq!(
        verify_unallocated_failure(plan, partition, &mut { usize::MAX }),
        Err(Refusal::Identity)
    );

    let mut duplicate = plan.clone();
    let failure = plan
        .exits
        .iter()
        .find(|exit| {
            matches!(&exit.continuation, ExitContinuation::ReturnFailure { source }
            if source.expression == producer.id && source.lane == StatusLane::OperationFailure)
        })
        .unwrap();
    duplicate.exits.push(failure.clone());
    assert_eq!(
        verify_unallocated_failure(&duplicate, producer, &mut { usize::MAX }),
        Err(Refusal::Identity)
    );
}
