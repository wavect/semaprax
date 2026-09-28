//! Test-only independent ordinary oracle and thread-scoped entry witness.
use super::*;

#[cfg(test)]
thread_local! { pub(super) static CONTINUE_OBSERVE_ENTRIES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
#[cfg(test)]
pub(crate) fn test_continue_observe_entries_v8() -> usize {
    CONTINUE_OBSERVE_ENTRIES.with(std::cell::Cell::get)
}
/// Inert independent evaluator arguments; physical State is only borrowed.
#[cfg(test)]
pub(crate) fn test_continue_observe_oracle_v8(
    held: &HeldExecutedOwnedStepV2<'_>,
) -> (ResumableChannelValue, usize) {
    use crate::interpreter::retained_call::{
        RetainedCallOutcome, RetainedField, RetainedRecord, RetainedValue as R,
    };
    let Some(OwnedStepTransferV2::Continue(state)) = held.owner.as_ref() else {
        panic!("actual Continue")
    };
    let Some(Value::Record(record)) = state.root.as_ref() else {
        panic!("actual State")
    };
    let proof = held.inputs.as_ref().unwrap().execution.wait().observe();
    let program = proof.helper().program();
    let fields = program.declarations.record_fields(&record.record).unwrap();
    let argument = R::Record(RetainedRecord {
        record: record.record.clone(),
        fields: fields
            .iter()
            .map(|field| RetainedField {
                field: field.id.clone(),
                value: match &record.fields[&field.id] {
                    Value::Bytes(v) => R::Bytes(v.bytes.to_vec()),
                    Value::Int(v) => R::I64(*v),
                    Value::Int32(v) => R::I32(*v),
                    Value::Bool(v) => R::Bool(*v),
                    Value::Uint8(v) => R::U8(*v),
                    Value::Usize(v) => R::Usize(*v),
                    _ => panic!("checked actual fixture leaf"),
                },
            })
            .collect(),
    });
    let prepared = crate::interpreter::retained_call::prepare_retained_call(
        program,
        proof.function().id.as_str(),
    )
    .unwrap();
    let ordinary = crate::interpreter::retained_call::evaluate_retained_call(
        program,
        &prepared,
        &[argument],
        held.inputs.as_ref().unwrap().execution.evaluation_fuel(),
    )
    .unwrap();
    let RetainedCallOutcome::Returned(R::Record(result)) = ordinary.outcome else {
        panic!("ordinary Observe")
    };
    let value = ResumableChannelValue::Record {
        declaration: result.record,
        fields: result
            .fields
            .into_iter()
            .map(|field| match field.value {
                R::I64(v) => ArgumentValue::Int(v),
                R::I32(v) => ArgumentValue::Int32(v),
                R::Bool(v) => ArgumentValue::Bool(v),
                R::U8(v) => ArgumentValue::Uint8(v),
                R::Usize(v) => ArgumentValue::Usize(v),
                _ => panic!("checked Copy Observation"),
            })
            .collect(),
    };
    (value, ordinary.steps_used)
}
