//! Source and attached plans are prepared outside the actual query observer.
//! This is ordinary status comparison, not selected admission or whole replay funding.
use super::*;
use crate::test_allocation_observer::measure;

fn program(count: usize) -> ResolvedProgram {
    let mut source = String::from(
        "module replay.borrowed_status;\n@id(\"status.add\") fn add(x:i64)->i64 requires x>=0 ensures result>=0 {x+1}\n@id(\"status.main\") fn main()->i64 requires true ensures result>=0 {\n",
    );
    for n in 0..count {
        source.push_str(&format!("let v{n}=add({n})+1;\n"));
    }
    source.push_str("0}\n");
    crate::hir::resolve(&crate::check(&source, "borrowed-status.spx").unwrap()).unwrap()
}
fn same(program: &ResolvedProgram, function: &ResolvedFunction) -> Result<(), Diagnostic> {
    let expected = old::validate_required_status_sources(program, function);
    let actual = validate(program, function);
    assert_eq!(
        actual, expected,
        "complete diagnostic and producer-order oracle"
    );
    actual
}

#[test]
fn borrowed_status_comparison_preserves_owned_oracle_and_late_error_priority() {
    let program = program(4);
    for function in &program.functions {
        same(&program, function).unwrap();
        for index in 0..function.cleanup_plan.status_sources.len() {
            let mut changed = function.clone();
            let row = &mut changed.cleanup_plan.status_sources[index];
            row.id.lane = StatusLane::OwnerAdmission;
            assert!(same(&program, &changed).is_err());
            let mut changed = function.clone();
            match &mut changed.cleanup_plan.status_sources[index].producer {
                StatusProducer::PropagatedCall { callee } => {
                    *callee = DeclarationId::new("forged.call")
                }
                StatusProducer::CheckedArithmetic {
                    normalized_cases, ..
                } => normalized_cases.reverse(),
                StatusProducer::ContractFalse { ordinal, .. } => *ordinal += 1,
                StatusProducer::CheckedAllocation { .. } => {
                    panic!("ordinary source must not select allocation provenance")
                }
            }
            // A one-case reversal is unchanged; mutate its actual case too.
            if let StatusProducer::CheckedArithmetic {
                normalized_cases, ..
            } = &mut changed.cleanup_plan.status_sources[index].producer
            {
                normalized_cases.push(StatusCase::DivisionByZero);
            }
            assert!(same(&program, &changed).is_err());
        }
        let mut changed = function.clone();
        changed.cleanup_plan.status_sources.pop();
        assert!(same(&program, &changed).is_err());
        let mut changed = function.clone();
        changed
            .cleanup_plan
            .status_sources
            .push(function.cleanup_plan.status_sources[0].clone());
        assert!(same(&program, &changed).is_err());
        let mut changed = function.clone();
        changed.cleanup_plan.status_sources.swap(0, 1);
        assert!(same(&program, &changed).is_err());
    }
    let original = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "status.main")
        .unwrap();
    let mut changed = original.clone();
    changed.cleanup_plan.status_sources.clear(); // earlier contract table mismatch
    changed.body.kind = ResolvedExprKind::Call {
        callee: DeclarationId::new("missing.after.contract"),
        type_arguments: vec![],
        instance: None,
        args: vec![],
    };
    let error = same(&program, &changed).unwrap_err();
    assert!(error
        .message
        .contains("unknown callee `missing.after.contract`"));
    assert!(!error.message.contains("do not exactly cover"));
    for depth in [512, 514] {
        let mut changed = original.clone();
        changed.cleanup_plan.status_sources.clear();
        let mut expression = changed.body.clone();
        expression.kind = ResolvedExprKind::Int(0);
        for _ in 0..depth {
            expression = ResolvedExpr {
                kind: ResolvedExprKind::Unary {
                    op: UnaryOp::Neg,
                    value: Box::new(expression),
                },
                ..changed.body.clone()
            };
        }
        changed.body = expression;
        let error = same(&program, &changed).unwrap_err();
        assert_eq!(error.message.contains("depth exceeds 512"), depth == 514);
    }
}

#[test]
fn borrowed_status_comparison_removes_actual_owned_table_backing_through_drop() {
    for count in [1, 32, 128] {
        let program = program(count);
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "status.main")
            .unwrap();
        same(&program, function).unwrap();
        let (result, borrowed) = measure(|| validate(&program, function));
        result.unwrap();
        assert!(!borrowed.invalid);
        assert_eq!(borrowed.allocations, 0);
        assert_eq!(borrowed.reallocations, 0);
        assert_eq!(borrowed.allocated, 0);
        assert_eq!(borrowed.live, 0);
        let (result, owned) = measure(|| old::validate_required_status_sources(&program, function));
        result.unwrap();
        assert!(!owned.invalid);
        // Each authored + has an independently known one-case Vec in the saved
        // producer, in addition to the owned status table and call ID strings.
        assert!(owned.allocations > count);
        assert!(owned.allocated >= count * std::mem::size_of::<StatusCase>());
        assert_eq!(owned.allocated, owned.freed);
        assert_eq!(owned.live, 0);
        assert_eq!(
            program
                .functions
                .iter()
                .find(|f| f.id.as_str() == "status.main")
                .unwrap() as *const _,
            function as *const _
        );
    }
}
