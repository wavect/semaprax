//! A single Copy slot store: preserve row and leaf owners in place.
use super::*;
impl Evaluator<'_> {
    pub(super) fn update_owned_vec_field(
        &mut self,
        element: &ResolvedType,
        identity: &str,
        values: Vec<Value>,
    ) -> Result<Value, Flow> {
        let descriptor = crate::vec_ops::field_update::replay(self.declarations, element, identity)
            .ok_or(Flow::Guard("forged scalar update descriptor"))?;
        let (_, field_type) =
            crate::vec_ops::field_update::position(self.declarations, element, identity)
                .ok_or(Flow::Guard("missing scalar update field"))?;
        let mut values = values.into_iter();
        let (Some(Value::Vec(mut vector)), Some(Value::Usize(index)), Some(value), None) =
            (values.next(), values.next(), values.next(), values.next())
        else {
            return Err(Flow::Guard("invalid scalar update operands"));
        };
        if vector.element != *element || !scalar_value_matches_type(&value, &field_type) {
            return Err(Flow::Guard("forged scalar update type"));
        }
        let index = usize::try_from(index)
            .ok()
            .filter(|i| *i < vector.values.len())
            .ok_or_else(|| Flow::Failure(normalize_vec(crate::vec_ops::GET_OUT_OF_BOUNDS_CODE)))?;
        if !element_value_matches_type(self.declarations, &vector.values[index], element) {
            return Err(Flow::Guard("forged scalar update row"));
        }
        let generation = vector
            .generation
            .checked_add(1)
            .ok_or(Flow::Guard("scalar update generation overflow"))?;
        self.fused.preflight_value(&vector.values[index])?;
        let carrier =
            Arc::get_mut(&mut vector).ok_or(Flow::Guard("aliased scalar update vector"))?;
        let Value::Record(record) = &mut carrier.values[index] else {
            return Err(Flow::Guard("missing scalar update row"));
        };
        let row = Arc::get_mut(record).ok_or(Flow::Guard("aliased scalar update row"))?;
        let slot = row
            .fields
            .get_mut(&descriptor.field)
            .ok_or(Flow::Guard("missing scalar update slot"))?;
        // All fallible authentication and uniqueness work precedes this commit.
        // The replaced value is Copy; no owned leaf is dropped or materialized.
        *slot = value;
        carrier.generation = generation;
        Ok(Value::Vec(vector))
    }
}
