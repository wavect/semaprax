//! Runtime carrier typing is independent of source and HIR admission.
use super::*;
impl Evaluator<'_> {
    /// Site 5 of the five admitted-variant-payload-profile classifiers named
    /// in issue #261: sites 1 (`check_byte_data_declarations`, `SPX-T268`)
    /// and 2 (`TypeTable::is_flat_owned_string_variant`, `SPX-O002`/`SPX-O104`)
    /// gate source `Type`s; sites 3 (`is_admitted_owned_string_variant`,
    /// `SPX-O117`) and 4 (`hir/validation.rs`, `SPX-H006`) gate resolved
    /// `ResolvedType`s. This site checks a runtime `Value` instead, so it
    /// cannot share either owning field predicate directly, but the Record
    /// arm below still consults the same resolved-level
    /// `is_admitted_copy_aggregate_variant_field` that sites 3 and 4 do.
    /// Missing this disjunct is the worst-case symptom of the five: a
    /// verified program that fails construction at run time on the
    /// `SPX-F105` "impossible post-verify state" guard.
    pub(super) fn value_has_type(&self, value: &Value, ty: &ResolvedType) -> bool {
        match (value, ty) {
            (Value::Int(_), ResolvedType::I64)
            | (Value::Int32(_), ResolvedType::I32)
            | (Value::Uint8(_), ResolvedType::U8)
            | (Value::Usize(_), ResolvedType::Usize)
            | (Value::Char(_), ResolvedType::Char)
            | (Value::Float32(_), ResolvedType::F32)
            | (Value::Float64(_), ResolvedType::F64)
            | (Value::Bool(_), ResolvedType::Bool)
            | (
                Value::OnceClosure(_),
                ResolvedType::OnceFunction
                | ResolvedType::OnceFunctionI64
                | ResolvedType::OnceFunctionI64Pair,
            )
            | (Value::Bytes(_), ResolvedType::Bytes)
            | (Value::Map(_), ResolvedType::StringMap)
            | (Value::String(_), ResolvedType::String) => true,
            (Value::Vec(carrier), expected) => {
                hir::owned_leaf_collection::is_copy_or_leaf_vec(self.declarations, expected)
                    && matches!(expected, ResolvedType::Nominal{arguments,..} if arguments.as_slice()==[carrier.element.clone()])
                    && carrier.values.len() <= carrier.capacity
                    && carrier.capacity
                        <= hir::owned_leaf_collection::capacity(self.declarations, &carrier.element)
                            as usize
                    && carrier.values.iter().all(|value| {
                        owned_vec::element_value_matches_type(
                            self.declarations,
                            value,
                            &carrier.element,
                        )
                    })
            }
            (Value::Collection(carrier), expected) => {
                crate::map_ops::is_collection(expected) && &carrier.ty == expected
            }
            (Value::Closure(value), ResolvedType::MutFunctionI64) => value.mutable.is_some(),
            (Value::Variant(carrier), expected) => &carrier.ty == expected,
            (Value::Iter(carrier), expected) => {
                crate::iterator_ops::is_iter(expected)
                    && crate::iterator_ops::element(expected) == Some(&carrier.vector.element)
            }
            (Value::List(_), expected) => crate::list_ops::is_list(expected),
            (Value::Record(carrier), ResolvedType::Nominal { declaration, .. }) => {
                &carrier.record == declaration
                    && (record_construction_is_admitted(self.declarations, ty)
                        // Copy Aggregate Variant Payload v1 (see the doc
                        // comment above `value_has_type`).
                        || crate::hir::is_admitted_copy_aggregate_variant_field(
                            self.declarations,
                            ty,
                        ))
            }
            _ => false,
        }
    }
}
