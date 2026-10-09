//! Explicit clone-out and stable whole-carrier ordering for owned leaves.
use super::super::OwnedRecordValue;
use super::*;
use std::collections::BTreeMap;

impl Evaluator<'_> {
    pub(super) fn clone_owned_vec_element(
        &mut self,
        element: &ResolvedType,
        values: &[Value],
    ) -> Result<Value, Flow> {
        let [Value::Vec(vector), Value::Usize(index)] = values else {
            return Err(Flow::Guard("invalid owned Vec clone operands"));
        };
        if vector.element != *element {
            return Err(Flow::Guard("forged owned Vec element type"));
        }
        let value = usize::try_from(*index)
            .ok()
            .and_then(|index| vector.values.get(index))
            .ok_or_else(|| Flow::Failure(normalize_vec(crate::vec_ops::GET_OUT_OF_BOUNDS_CODE)))?;
        if !element_value_matches_type(self.declarations, value, element) {
            return Err(Flow::Guard("forged owned Vec clone element"));
        }
        let layout = crate::hir::owned_leaf_collection::layout(self.declarations, element)
            .ok_or(Flow::Guard("invalid owned Vec clone layout"))?;
        if let Some(fields) = layout.fields {
            let Value::Record(record) = value else {
                return Err(Flow::Guard("missing owned record"));
            };
            let mut copied = BTreeMap::new();
            // Declaration order is evaluation order. On a later failure the local
            // map drops every completed clone, leaving the borrowed vector intact.
            for field in fields {
                self.charge()?;
                copied.insert(
                    field.id.clone(),
                    self.clone_owned_vec_leaf(&record.fields[&field.id])?,
                );
            }
            Ok(Value::Record(Arc::new(OwnedRecordValue {
                record: record.record.clone(),
                fields: copied,
            })))
        } else {
            self.clone_owned_vec_leaf(value)
        }
    }
    fn clone_owned_vec_leaf(&mut self, value: &Value) -> Result<Value, Flow> {
        if let Value::Bytes(value) = value {
            let length = value.bytes.len() as u64;
            let count = self
                .next_byte_allocation
                .checked_add(1)
                .filter(|count| *count <= crate::byte_data_capacity::MAX_BYTES_COPY_SITES);
            let payload = self
                .allocated_byte_payload
                .checked_add(length)
                .filter(|payload| {
                    *payload <= crate::byte_data_capacity::MAX_OWNED_BYTE_PAYLOAD_BYTES
                });
            let (Some(count), Some(payload)) = (count, payload) else {
                return Err(Flow::Failure(normalize_vec(
                    crate::vec_ops::ALLOCATION_FAILURE_CODE,
                )));
            };
            if length > crate::byte_ops::MAX_OWNED_BYTE_VALUE_BYTES {
                return Err(Flow::Failure(normalize_vec(
                    crate::vec_ops::ALLOCATION_FAILURE_CODE,
                )));
            }
            let mut bytes = Vec::new();
            bytes.try_reserve_exact(value.bytes.len()).map_err(|_| {
                Flow::Failure(normalize_vec(crate::vec_ops::ALLOCATION_FAILURE_CODE))
            })?;
            bytes.extend_from_slice(&value.bytes);
            self.next_byte_allocation = count;
            self.allocated_byte_payload = payload;
            return Ok(Value::Bytes(super::super::OwnedBytesValue {
                allocation: count,
                bytes: Arc::from(bytes),
            }));
        }
        // String copies charge the established UTF-8 materialization budget;
        // scalar copies preserve floating-point bits. Records never reach here.
        self.clone_value(value)
    }
    pub(super) fn sort_owned_vec_elements(
        &mut self,
        element: &ResolvedType,
        values: Vec<Value>,
    ) -> Result<Value, Flow> {
        let mut values = values.into_iter();
        let (Some(Value::Vec(vector)), None) = (values.next(), values.next()) else {
            return Err(Flow::Guard("invalid owned Vec sort operands"));
        };
        let layout = crate::hir::owned_leaf_collection::layout(self.declarations, element)
            .ok_or(Flow::Guard("invalid owned Vec sort layout"))?;
        if vector.element != *element
            || vector
                .values
                .iter()
                .any(|value| !element_value_matches_type(self.declarations, value, element))
        {
            return Err(Flow::Guard("forged owned Vec sort element"));
        }
        let mut vector =
            Arc::try_unwrap(vector).map_err(|_| Flow::Guard("aliased owned Vec sort carrier"))?;
        // Stable insertion sort has no payload clone or temporary allocation.
        for index in 1..vector.values.len() {
            let mut cursor = index;
            while cursor > 0
                && compare(
                    &vector.values[cursor],
                    &vector.values[cursor - 1],
                    layout.fields,
                )
                .is_lt()
            {
                vector.values.swap(cursor, cursor - 1);
                cursor -= 1;
            }
        }
        vector.generation = vector
            .generation
            .checked_add(1)
            .ok_or(Flow::Guard("owned Vec generation overflowed"))?;
        Ok(Value::Vec(Arc::new(vector)))
    }
}
fn leaf_compare(a: &Value, b: &Value) -> std::cmp::Ordering {
    match (a, b) {
        (Value::String(a), Value::String(b)) => a.as_bytes().cmp(b.as_bytes()),
        (Value::Bytes(a), Value::Bytes(b)) => a.bytes.as_ref().cmp(b.bytes.as_ref()),
        _ => super::compare_scalar(a, b),
    }
}
fn compare(
    a: &Value,
    b: &Value,
    fields: Option<&[crate::hir::ResolvedFieldDeclaration]>,
) -> std::cmp::Ordering {
    if let Some(fields) = fields {
        let (Value::Record(a), Value::Record(b)) = (a, b) else {
            unreachable!("authenticated record carriers")
        };
        fields
            .iter()
            .map(|field| leaf_compare(&a.fields[&field.id], &b.fields[&field.id]))
            .find(|order| !order.is_eq())
            .unwrap_or(std::cmp::Ordering::Equal)
    } else {
        leaf_compare(a, b)
    }
}
