//! Scoped Vec reads stage a carrier reference before evaluating the index.
use super::*;
impl Evaluator<'_> {
    pub(super) fn evaluate_vec_field(
        &mut self,
        element: &ResolvedType,
        field: &hir::DeclarationId,
        bytes: bool,
        args: &[ResolvedExpr],
        environment: &mut Environment,
        depth: usize,
    ) -> Result<Value, Flow> {
        let [source, index] = args else {
            return Err(Flow::Guard("invalid scoped vector read arity"));
        };
        let ResolvedExprKind::Place(place) = &source.kind else {
            return Err(Flow::Guard("scoped vector read has no named carrier"));
        };
        let selected = hir::vec_field::field(self.declarations, element, field)
            .filter(|selected| selected.result_type(bytes).is_some())
            .ok_or(Flow::Guard("invalid scoped vector field identity"))?;
        let mut source = environment
            .get(&place.root)
            .ok_or(Flow::Guard("scoped vector root is unavailable"))?;
        for projection in &place.projections {
            let hir::PlaceProjection::Field(field) = projection else {
                return Err(Flow::Guard("invalid scoped vector carrier projection"));
            };
            let Value::Record(record) = source else {
                return Err(Flow::Guard("scoped vector projection is not a record"));
            };
            source = record
                .fields
                .get(field)
                .ok_or(Flow::Guard("missing scoped vector carrier field"))?;
        }
        let Value::Vec(vector) = source else {
            return Err(Flow::Guard("scoped vector carrier is not live"));
        };
        if vector.element != *element {
            return Err(Flow::Guard("scoped vector element identity mismatch"));
        }
        // Arc clone retains only the abstract carrier reference; no semantic
        // owner, element copy, payload materialization or generation renewal.
        let vector = Arc::clone(vector);
        let Value::Usize(index) = self.evaluate(index, environment, depth)? else {
            return Err(Flow::Guard("scoped vector index is not usize"));
        };
        let row = usize::try_from(index)
            .ok()
            .and_then(|index| vector.values.get(index))
            .ok_or_else(|| {
                Flow::Failure(owned_vec::normalize_vec(
                    crate::vec_ops::GET_OUT_OF_BOUNDS_CODE,
                ))
            })?;
        if !owned_vec::element_value_matches_type(self.declarations, row, element) {
            return Err(Flow::Guard("scoped vector row is unauthenticated"));
        }
        let Value::Record(record) = row else {
            return Err(Flow::Guard("scoped vector row is not a record"));
        };
        let value = record
            .fields
            .get(&selected.declaration.id)
            .ok_or(Flow::Guard("scoped vector field is missing"))?;
        Ok(match value {
            Value::Int(v) => Value::Int(*v),
            Value::Int32(v) => Value::Int32(*v),
            Value::Uint8(v) => Value::Uint8(*v),
            Value::Usize(v) => Value::Usize(*v),
            Value::Char(v) => Value::Char(*v),
            Value::Float32(v) => Value::Float32(*v),
            Value::Float64(v) => Value::Float64(*v),
            Value::Bool(v) => Value::Bool(*v),
            Value::Bytes(v) => Value::BorrowedSlice(BorrowedSliceValue::whole(
                place.root.clone(),
                Arc::clone(&v.bytes),
            )),
            Value::String(text) => {
                // Like projected String views, this immutable reference-model
                // backing incurs no language String/Bytes allocation charge.
                let contents = Arc::from(text.as_bytes());
                if bytes {
                    Value::BorrowedSlice(BorrowedSliceValue::whole(place.root.clone(), contents))
                } else {
                    Value::BorrowedStr(BorrowedStrValue {
                        invocation_root: place.root.clone(),
                        bytes: contents,
                    })
                }
            }
            _ => return Err(Flow::Guard("scoped vector field carrier is unsupported")),
        })
    }
}

#[cfg(test)]
mod tests;
