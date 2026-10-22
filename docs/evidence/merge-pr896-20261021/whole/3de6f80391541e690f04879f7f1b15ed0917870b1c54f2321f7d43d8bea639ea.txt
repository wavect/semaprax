//! Complete immediate-old finite admission predicate.
use super::scalar_signature::ScalarSignature;
impl<'a> ScalarSignature<'a> {
    pub(crate) fn checked_before_string(
        provider: &'a crate::ast::Program,
        original: &'a crate::ast::Function,
        alias: &'a str,
    ) -> Option<Self> {
        use crate::ast::{ParamMode, Type};
        let mut scalar = original.type_parameters.is_empty()
            && original.yields.is_none()
            && original.follows.is_none()
            && !original.stable_id.starts_with("std.")
            && matches!(
                original.return_type,
                Type::I64
                    | Type::I32
                    | Type::Char
                    | Type::U8
                    | Type::Usize
                    | Type::F32
                    | Type::F64
                    | Type::Bool
            );
        // Complete bounded checks: the caller's source-length invoice pays
        // exactly these iterations, including both terminal iterator results.
        for param in &original.params {
            scalar &= param.mode == ParamMode::Value
                && matches!(
                    param.ty,
                    Type::I64
                        | Type::I32
                        | Type::Char
                        | Type::U8
                        | Type::Usize
                        | Type::F32
                        | Type::F64
                        | Type::Bool
                );
        }
        let mut member = false;
        for function in &provider.functions {
            member |= std::ptr::eq(function, original);
        }
        if !scalar || !member {
            return None;
        }

        Some(Self {
            view: super::FunctionView {
                _provider: None,
                original,
                _alias: alias,
            },
        })
    }
}
