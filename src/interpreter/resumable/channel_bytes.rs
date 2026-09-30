//! Owned `Bytes` leaves for the bounded direct sequential yield channel.

use super::super::OwnedBytesValue;
use super::{
    argument_of, hir, ArgumentValue, OwnedRecordValue, OwnedVariantValue, ResolvedType,
    ResumableChannelValue, ResumableScalar, Value,
};
use std::collections::BTreeMap;
use std::sync::Arc;

pub(crate) const MAX_CHANNEL_BYTES: usize = 1024;

#[derive(Clone, Debug, PartialEq)]
pub enum ChannelField {
    Scalar(ArgumentValue),
    Bytes(Vec<u8>),
}

fn field_of(value: &Value) -> Option<ChannelField> {
    match value {
        Value::Bytes(bytes) if bytes.bytes.len() <= MAX_CHANNEL_BYTES => {
            Some(ChannelField::Bytes(bytes.bytes.to_vec()))
        }
        value => argument_of(value).map(ChannelField::Scalar),
    }
}

pub(super) fn channel_of(
    declarations: &hir::DeclarationIndex,
    value: &Value,
) -> Option<ResumableChannelValue> {
    match value {
        Value::Record(record) => {
            let fields = declarations
                .record_fields(&record.record)?
                .iter()
                .map(|field| record.fields.get(&field.id).and_then(field_of))
                .collect::<Option<Vec<_>>>()?;
            if fields
                .iter()
                .all(|field| matches!(field, ChannelField::Scalar(_)))
            {
                Some(ResumableChannelValue::Record {
                    declaration: record.record.clone(),
                    fields: fields
                        .into_iter()
                        .map(|field| match field {
                            ChannelField::Scalar(value) => value,
                            ChannelField::Bytes(_) => unreachable!(),
                        })
                        .collect(),
                })
            } else {
                Some(ResumableChannelValue::RecordBytes {
                    declaration: record.record.clone(),
                    fields,
                })
            }
        }
        Value::Variant(variant) => {
            let fields = declarations
                .case_fields(&variant.case)?
                .iter()
                .map(|field| variant.fields.get(&field.id).and_then(field_of))
                .collect::<Option<Vec<_>>>()?;
            if fields
                .iter()
                .all(|field| matches!(field, ChannelField::Scalar(_)))
            {
                Some(ResumableChannelValue::Variant {
                    declaration: variant.variant.clone(),
                    case: variant.case.clone(),
                    fields: fields
                        .into_iter()
                        .map(|field| match field {
                            ChannelField::Scalar(value) => value,
                            ChannelField::Bytes(_) => unreachable!(),
                        })
                        .collect(),
                })
            } else {
                Some(ResumableChannelValue::VariantBytes {
                    declaration: variant.variant.clone(),
                    case: variant.case.clone(),
                    fields,
                })
            }
        }
        _ => None,
    }
}

fn field_value(ty: &ResolvedType, field: &ChannelField, allocation: &mut u32) -> Option<Value> {
    match (ty, field) {
        (ResolvedType::Bytes, ChannelField::Bytes(bytes)) if bytes.len() <= MAX_CHANNEL_BYTES => {
            *allocation = allocation.checked_add(1)?;
            Some(Value::Bytes(OwnedBytesValue {
                allocation: *allocation,
                bytes: Arc::from(bytes.as_slice()),
            }))
        }
        (_, ChannelField::Scalar(value)) => super::scalar_of(ty, value),
        _ => None,
    }
}

pub(super) fn value_of(
    declarations: &hir::DeclarationIndex,
    declared: &ResolvedType,
    supplied: &ResumableChannelValue,
    allocation: &mut u32,
) -> Option<Value> {
    let (declaration, case, fields) = match supplied {
        ResumableChannelValue::RecordBytes {
            declaration,
            fields,
        } => (declaration, None, fields),
        ResumableChannelValue::VariantBytes {
            declaration,
            case,
            fields,
        } => (declaration, Some(case), fields),
        _ => return None,
    };
    let ResolvedType::Nominal {
        declaration: expected,
        arguments,
    } = declared
    else {
        return None;
    };
    if expected != declaration || !arguments.is_empty() {
        return None;
    }
    let canonical = match case {
        None => declarations.record_fields(declaration)?,
        Some(case) => {
            if !declarations
                .variant_cases(declaration)?
                .iter()
                .any(|candidate| &candidate.id == case)
            {
                return None;
            }
            declarations.case_fields(case)?
        }
    };
    if canonical.len() != fields.len() {
        return None;
    }
    let mut built = BTreeMap::new();
    for (field, value) in canonical.iter().zip(fields) {
        built.insert(field.id.clone(), field_value(&field.ty, value, allocation)?);
    }
    match case {
        None => Some(Value::Record(Arc::new(OwnedRecordValue {
            record: declaration.clone(),
            fields: built,
        }))),
        Some(case) => Some(Value::Variant(Arc::new(OwnedVariantValue {
            ty: declared.clone(),
            variant: declaration.clone(),
            case: case.clone(),
            fields: built,
        }))),
    }
}

pub(super) fn binding(value: &ResumableChannelValue) -> Option<ResumableScalar> {
    let fields = match value {
        ResumableChannelValue::RecordBytes { fields, .. }
        | ResumableChannelValue::VariantBytes { fields, .. } => fields,
        _ => return None,
    };
    let fields = fields
        .iter()
        .map(|field| match field {
            ChannelField::Scalar(value) => super::resumable_scalar_of(value),
            ChannelField::Bytes(bytes) if bytes.len() <= MAX_CHANNEL_BYTES => {
                Some(ResumableScalar::Bytes(bytes.clone()))
            }
            ChannelField::Bytes(_) => None,
        })
        .collect::<Option<Vec<_>>>()?;
    match value {
        ResumableChannelValue::RecordBytes { .. } => Some(ResumableScalar::Record(fields)),
        ResumableChannelValue::VariantBytes { case, .. } => Some(ResumableScalar::Variant {
            case: case.as_str().to_owned(),
            fields,
        }),
        _ => None,
    }
}

pub(crate) fn contains(value: &ResumableChannelValue) -> bool {
    matches!(
        value,
        ResumableChannelValue::RecordBytes { .. } | ResumableChannelValue::VariantBytes { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injected_byte_leaves_receive_distinct_monotonic_allocations() {
        let mut allocation = 0;
        let first = field_value(
            &ResolvedType::Bytes,
            &ChannelField::Bytes(vec![1]),
            &mut allocation,
        )
        .unwrap();
        let second = field_value(
            &ResolvedType::Bytes,
            &ChannelField::Bytes(vec![2]),
            &mut allocation,
        )
        .unwrap();
        let Value::Bytes(first) = first else {
            panic!("expected Bytes")
        };
        let Value::Bytes(second) = second else {
            panic!("expected Bytes")
        };
        assert_eq!((first.allocation, second.allocation), (1, 2));
    }

    #[test]
    fn byte_leaf_binds_exact_payload_and_refuses_the_cap_overrun() {
        let value = ResumableChannelValue::RecordBytes {
            declaration: hir::DeclarationId::new("app.prompt"),
            fields: vec![ChannelField::Bytes(vec![1, 2, 3])],
        };
        assert_eq!(
            binding(&value),
            Some(ResumableScalar::Record(vec![ResumableScalar::Bytes(vec![
                1, 2, 3
            ])]))
        );
        let over = ResumableChannelValue::RecordBytes {
            declaration: hir::DeclarationId::new("app.prompt"),
            fields: vec![ChannelField::Bytes(vec![0; MAX_CHANNEL_BYTES + 1])],
        };
        assert_eq!(binding(&over), None);
    }
}
