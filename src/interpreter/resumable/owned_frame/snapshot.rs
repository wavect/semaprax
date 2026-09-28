//! Borrowed inert projection. This module never creates an owner.
use super::*;

pub(crate) fn argument_binding(argument: &OwnedFrameArgument) -> &str {
    argument.plan.binding()
}

#[cfg(test)]
pub(crate) fn argument_weak(argument: &OwnedFrameArgument) -> Vec<std::sync::Weak<[u8]>> {
    weak_leaves(argument.root.as_ref().expect("admitted root"))
}
#[cfg(test)]
pub(super) fn weak_leaves(root: &Value) -> Vec<std::sync::Weak<[u8]>> {
    let Value::Record(record) = root else {
        panic!("checked record")
    };
    record
        .fields
        .values()
        .filter_map(|value| match value {
            Value::Bytes(bytes) => Some(Arc::downgrade(&bytes.bytes)),
            _ => None,
        })
        .collect()
}

pub(crate) fn validate_input(
    plan: &CheckedOwnedFramePlan,
    input: &OwnedFrameInput,
) -> Result<(), Diagnostic> {
    validate_fields(
        plan,
        &input.declaration,
        input.fields.len(),
        input.fields.iter().map(|field| {
            (
                &field.identity,
                match &field.value {
                    OwnedFrameInputValue::Bytes(bytes) => InputRef::Bytes(bytes),
                    OwnedFrameInputValue::Scalar(value) => InputRef::Scalar(value),
                },
            )
        }),
    )
}

pub(in crate::interpreter) fn root_input(
    plan: &CheckedOwnedFramePlan,
    root: &Value,
) -> Result<OwnedFrameInput, Diagnostic> {
    if !exclusive(root) || !root_matches(plan, root) {
        return Err(rejected("snapshot root/leaf identity or alias mismatch"));
    }
    let Value::Record(record) = root else {
        unreachable!()
    };
    let declared = plan
        .program()
        .declarations
        .record_fields(&record.record)
        .expect("checked record");
    let input = OwnedFrameInput {
        declaration: record.record.clone(),
        fields: declared
            .iter()
            .map(|field| {
                let value = match &record.fields[&field.id] {
                    Value::Bytes(bytes) => OwnedFrameInputValue::Bytes(bytes.bytes.to_vec()),
                    scalar => OwnedFrameInputValue::Scalar(
                        super::super::argument_of(scalar).expect("checked scalar"),
                    ),
                };
                OwnedFrameInputField {
                    identity: field.id.clone(),
                    value,
                }
            })
            .collect(),
    };
    validate_input(plan, &input)?;
    Ok(input)
}

pub(crate) fn argument_input(argument: &OwnedFrameArgument) -> Result<OwnedFrameInput, Diagnostic> {
    root_input(
        &argument.plan,
        argument
            .root
            .as_ref()
            .ok_or_else(|| rejected("argument consumed"))?,
    )
}

pub(crate) fn request_valid(plan: &CheckedOwnedFramePlan, value: &ArgumentValue) -> bool {
    scalar_valid(
        &plan
            .function()
            .yields
            .as_ref()
            .expect("checked yield")
            .request_type,
        value,
    )
}
pub(crate) fn answer_valid(plan: &CheckedOwnedFramePlan, value: &ArgumentValue) -> bool {
    scalar_valid(
        &plan
            .function()
            .yields
            .as_ref()
            .expect("checked yield")
            .response_type,
        value,
    )
}
