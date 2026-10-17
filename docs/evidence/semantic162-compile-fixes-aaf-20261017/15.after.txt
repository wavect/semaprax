//! Complete raw constructor parity and actual same-ledger tradeoff owners.
use super::super::{replay_error, Diagnostic, ReplayBudget, ReplayContext, ResolvedFunction};
use crate::cleanup::{CleanupInventory, InventoryFailure};
use crate::cleanup_plan::ReplayPhysicalReservation;
use crate::hir::{DeclarationId, ResolvedProgram, ResolvedType};
use crate::test_allocation_observer::{measure, Metrics};

fn program() -> ResolvedProgram {
    let source = crate::check(
        r#"module replay.inventory_coverage;
@id("coverage.fill") fn fill(data:own Bytes)->Bytes
requires 0usize<byte_len(bytes_as_slice(data)) {
 let mut next=data; next=bytes_set(next,0usize,65u8); next
}
@id("coverage.main") fn main()->i64 {
 let mut data=bytes_zeroed(1usize); data=fill(data); let mut at=0;
 while at<2 {data=fill(data);at=at+1;if at==1 {continue;}else{0} 0}
 let view=bytes_as_slice(data); if byte_len(view)==1usize {42}else{0}
}"#,
        "replay-inventory-coverage.spx",
    )
    .unwrap();
    crate::hir::resolve(&source).unwrap()
}

fn flat_program() -> ResolvedProgram {
    let source = crate::check(r#"module replay.flat_records;
@id("flat.Packet") record Packet {
 @id("flat.Packet.n") n:i64,@id("flat.Packet.text") text:string,@id("flat.Packet.data") data:Bytes,
}
@id("flat.identity") fn identity(value:own Packet,flag:bool)->Packet {
 let mut next=0;while next<1 {next=next+1;if flag {return value;} else {0} 0} value
}
@id("flat.make") fn make(input:borrow Slice<u8>)->Packet {
 let key=str"x";let value=Packet{n:7,text:"label",data:bytes_copy(input)};
 let mut next=0;while next<1 {next=next+1;if byte_len(input)>0usize {return value;} else {0} 0} value
}
@id("flat.read") fn read(value:borrow Packet)->i64 {value.n}
@id("flat.probe") fn probe(flag:bool)->i64 {
 let input=[1u8,2u8];let packet=identity(make(array_as_slice(input)),flag);
 let mut next=0;while next<2 {next=next+1;if flag {continue;} else {0} if next==2 {return read(packet);} else {0} 0} read(packet)
}
@id("app.main") fn main()->i64 {probe(false)}
"#, "selected-flat-records.spx").unwrap();
    crate::hir::resolve(&source).unwrap()
}

fn raw(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    physical: &mut ReplayPhysicalReservation,
    remaining: &mut usize,
    old: bool,
) -> Result<CleanupInventory, Diagnostic> {
    if old {
        crate::cleanup::build_flat_record_inventory_with_budget(
            program,
            function,
            physical,
            &mut |units| match remaining.checked_sub(units) {
                Some(next) => {
                    *remaining = next;
                    Ok(())
                }
                None => {
                    *remaining = 0;
                    Err(replay_error(function, "selected inventory work exhausted"))
                }
            },
        )
    } else {
        crate::cleanup::build_typed_flat_record_inventory(program, function, physical, remaining)
            .map_err(|failure| render(function, failure))
    }
}
fn render(function: &ResolvedFunction, failure: InventoryFailure) -> Diagnostic {
    if failure.prefixed() {
        replay_error(function, failure.detail())
    } else {
        Diagnostic::io("SPX-H006", failure.detail())
    }
}
fn observe(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    expected: &CleanupInventory,
    old: bool,
    bytes: usize,
    work: usize,
) -> ((bool, usize, usize, usize), Metrics) {
    crate::cleanup::reset_typed_inventory_metadata_work();
    measure(|| {
        let mut physical = ReplayPhysicalReservation::new(bytes);
        let mut remaining = work;
        let result = raw(program, function, &mut physical, &mut remaining, old);
        if let Ok(inventory) = &result {
            assert_eq!(inventory, expected);
        }
        let observation = (
            result.is_ok(),
            physical.charged(),
            remaining,
            crate::cleanup::typed_inventory_metadata_work(),
        );
        drop(result);
        drop(physical);
        observation
    })
}
#[test]
fn complete_raw_inventory_keeps_all_fields_original_work_and_reports_real_receiving_tradeoffs() {
    for program in [program(), flat_program()] {
        for function in program.functions.iter().filter(|f| {
            f.id.as_str().starts_with("coverage.") || f.id.as_str().starts_with("flat.")
        }) {
            let mut original_trace = Vec::new();
            let expected = crate::cleanup::build_flat_record_inventory_with_budget(
                &program,
                function,
                &mut ReplayPhysicalReservation::new(usize::MAX),
                &mut |units| {
                    original_trace.push(units);
                    Ok(())
                },
            )
            .unwrap();
            crate::cleanup::begin_typed_inventory_trace();
            let mut actual_work = usize::MAX;
            let actual = crate::cleanup::build_typed_flat_record_inventory(
                &program,
                function,
                &mut ReplayPhysicalReservation::new(usize::MAX),
                &mut actual_work,
            )
            .unwrap();
            let actual_trace = crate::cleanup::take_typed_inventory_trace();
            assert_eq!(actual, expected);
            assert_eq!(
                actual_trace, original_trace,
                "actual original logical work order"
            );
            drop(actual);
            let mut observations = [(false, 0, 0, 0); 2];
            for (index, old) in [true, false].into_iter().enumerate() {
                let (result, allocation) =
                    observe(&program, function, &expected, old, usize::MAX, usize::MAX);
                assert!(result.0);
                assert!(!allocation.invalid && allocation.live == 0);
                assert_eq!(allocation.allocated, allocation.freed);
                assert!(allocation.allocated <= result.1);
                eprintln!("raw inventory {} old={old} charged={} work={} metadata_work={} actual={allocation:?}",
                function.id,result.1,usize::MAX-result.2,result.3);
                observations[index] = result;
                for bytes in [0, result.1 - 1, result.1] {
                    let (cut, allocation) =
                        observe(&program, function, &expected, old, bytes, usize::MAX);
                    assert_eq!(cut.0, bytes == result.1);
                    assert!(!allocation.invalid && allocation.live == 0);
                    assert_eq!(allocation.allocated, allocation.freed);
                }
                let work = usize::MAX - result.2;
                for limit in [0, work - 1, work] {
                    let (cut, allocation) =
                        observe(&program, function, &expected, old, usize::MAX, limit);
                    assert_eq!(cut.0, limit == work);
                    assert!(!allocation.invalid && allocation.live == 0);
                    assert_eq!(allocation.allocated, allocation.freed);
                }
            }
            // New actual initializer work remains spent; no refund or forecast.
            // The original traversal/probe/clone work is exactly conserved.
            assert_eq!(
                observations[1].2.checked_add(observations[1].3),
                Some(observations[0].2)
            );
        }
    }
}
#[test]
fn raw_primitive_failures_keep_six_fields_sticky_ledger_and_attached_inventory_non_authority() {
    let program = program();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "coverage.fill")
        .unwrap();
    let mut malformed = function.clone();
    malformed.params[0].ty = ResolvedType::Nominal {
        declaration: DeclarationId::new("untrusted.missing.record"),
        arguments: Vec::new(),
    };
    let mut a_work = usize::MAX;
    let a = raw(
        &program,
        &malformed,
        &mut ReplayPhysicalReservation::new(usize::MAX),
        &mut a_work,
        true,
    )
    .unwrap_err();
    let mut b_work = usize::MAX;
    let b = raw(
        &program,
        &malformed,
        &mut ReplayPhysicalReservation::new(usize::MAX),
        &mut b_work,
        false,
    )
    .unwrap_err();
    assert_eq!(a, b, "complete six-field unsupported-type provenance");
    let mut sentinel = Diagnostic::io("SPX-CALLBACK", "original callback stays opaque");
    sentinel.help = Some("no typed reinterpretation of arbitrary Diagnostics".to_owned());
    assert_eq!(
        crate::cleanup::build_flat_record_inventory_with_budget(
            &program,
            function,
            &mut ReplayPhysicalReservation::new(usize::MAX),
            &mut |_| Err(sentinel.clone()),
        )
        .unwrap_err(),
        sentinel,
    );
    for old in [true, false] {
        enclosing_floor_preserves_first_refusal(&program, function, old);
        let mut physical = ReplayPhysicalReservation::new(1);
        physical.reserve(1).unwrap();
        let first = physical.reserve(1).unwrap_err();
        let charged = physical.charged();
        let mut refused_work = usize::MAX;
        assert_eq!(
            raw(&program, function, &mut physical, &mut refused_work, old)
                .unwrap_err()
                .code,
            "SPX-H006"
        );
        assert_eq!(physical.reserve(0), Err(first));
        assert_eq!(physical.charged(), charged);
        let mut forged = function.clone();
        forged.cleanup.slots[0].ty = ResolvedType::String;
        let mut budget = ReplayBudget::with_physical(&forged, usize::MAX).unwrap();
        let error = if old {
            super::validate_inventory_coverage(&program, &forged, &mut budget).unwrap_err()
        } else {
            match super::paid::validate_inventory_coverage(&program, &forged, &mut budget)
                .unwrap_err()
            {
                super::super::expression_facts::errors::EngineError::Owned(error) => error,
                super::super::expression_facts::errors::EngineError::Structural(error) => {
                    error.legacy(&forged)
                }
                other => panic!("unexpected failure {other:?}"),
            }
        };
        assert_eq!(error.code, "SPX-H006");
        assert!(error.message.contains("cleanup inventory disagrees"));
    }
}

fn enclosing_floor_preserves_first_refusal(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    old: bool,
) {
    use crate::bounded_output::{clear_active_floor, set_active_floor, with_limit_usage_refusal};
    let limit = 1 << 20;
    let ((), refused, _, first) = with_limit_usage_refusal(limit, || {
        assert!(set_active_floor(limit - 1));
        let mut physical = ReplayPhysicalReservation::new(usize::MAX);
        let mut remaining = usize::MAX;
        assert!(raw(program, function, &mut physical, &mut remaining, old).is_err());
        let failure = physical.reserve(0).unwrap_err();
        let charged = physical.charged();
        clear_active_floor();
        assert!(raw(program, function, &mut physical, &mut remaining, old).is_err());
        assert_eq!(physical.reserve(0), Err(failure));
        assert_eq!(physical.charged(), charged);
    });
    assert!(refused);
    let first = first.unwrap();
    assert_eq!(first.floor, limit - 1);
    assert!(first.requested > first.remaining - first.floor);
}

mod initial_tests;
