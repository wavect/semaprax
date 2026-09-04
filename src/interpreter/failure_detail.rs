//! Contract-failure detail retained by the reference interpreter.
//!
//! A violated `requires` or `ensures` clause produces the normalized contract
//! status that every backend agrees on. That status names the phase and
//! nothing else, which is correct for equivalence and useless for repair. The
//! evaluator therefore also records, at the failing frame, which function and
//! clause failed and what the call's arguments were. The record is proof-free
//! data for reporting: it changes no status, no cleanup, and no result.

use crate::ast::Span;
use crate::cleanup_plan::ContractPhase;
use crate::hir::ResolvedFunction;
use crate::runtime_status::normalize_contract;

use super::{Environment, Evaluator, Flow, Value};

/// The frame facts of one violated contract clause.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractFailureDetail {
    /// Stable identity of the function whose clause failed.
    pub function_id: String,
    pub phase: ContractPhase,
    /// Position of the clause within the function's `requires` or `ensures`
    /// list, in declaration order.
    pub clause_index: usize,
    /// Revision-scoped identity of the clause expression.
    pub clause_id: String,
    /// Source span of the clause expression in the declaring file.
    pub clause_span: Span,
    /// The call's parameters in declaration order with their rendered values.
    pub arguments: Vec<ContractArgument>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractArgument {
    pub name: String,
    /// Name-independent type key, `i64` or `bool` for the scalar types.
    pub ty: String,
    pub value: String,
}

impl ContractFailureDetail {
    pub const fn phase_text(&self) -> &'static str {
        match self.phase {
            ContractPhase::Requires => "requires",
            ContractPhase::Ensures => "ensures",
        }
    }
}

impl Evaluator<'_> {
    /// Record the failing clause and frame, then produce the ordinary sticky
    /// contract failure. The last recorded detail is the one that propagates:
    /// a contract failure is never caught by language code, so the failure
    /// that ends evaluation is the last one recorded.
    pub(super) fn contract_failure(
        &mut self,
        function: &ResolvedFunction,
        frame: &Environment,
        phase: ContractPhase,
        clause_index: usize,
    ) -> Flow {
        let clause = match phase {
            ContractPhase::Requires => &function.requires[clause_index],
            ContractPhase::Ensures => &function.ensures[clause_index],
        };
        let arguments = function
            .params
            .iter()
            .map(|param| ContractArgument {
                name: param.name.clone(),
                ty: param.ty.identity_key(),
                value: frame
                    .iter()
                    .find(|(id, _)| *id == param.id)
                    .map_or_else(|| "<moved>".to_owned(), |(_, value)| value_text(value)),
            })
            .collect();
        self.failure_detail = Some(ContractFailureDetail {
            function_id: function.id.as_str().to_owned(),
            phase,
            clause_index,
            clause_id: clause.id.as_str().to_owned(),
            clause_span: clause.span,
            arguments,
        });
        Flow::Failure(normalize_contract(phase))
    }
}

/// Render one frame value for a report. Scalars render as source literals;
/// owned or borrowed data renders as its kind and length so a report never
/// copies payload bytes.
pub(super) fn value_text(value: &Value) -> String {
    match value {
        Value::Int(value) => value.to_string(),
        Value::Int32(value) => format!("{value}i32"),
        Value::Uint8(value) => format!("{value}u8"),
        Value::Usize(value) => format!("{value}usize"),
        Value::Char(value) => {
            char::from_u32(*value).map_or_else(|| format!("<char {value}>"), |c| format!("{c:?}"))
        }
        Value::Float32(value) => format!("{value:?}f32"),
        Value::Float64(value) => format!("{value:?}"),
        Value::Bool(value) => value.to_string(),
        Value::ArrayU8(bytes) => format!("<array u8 x{}>", bytes.len()),
        Value::String(text) => format!("<string {} bytes>", text.len()),
        Value::Record(_) => "<record>".to_owned(),
        Value::Variant(_) => "<variant>".to_owned(),
        Value::Moved => "<moved>".to_owned(),
        _ => "<data>".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_values_render_as_source_literals_and_data_as_kinds() {
        assert_eq!(value_text(&Value::Int(-7)), "-7");
        assert_eq!(value_text(&Value::Int32(3)), "3i32");
        assert_eq!(value_text(&Value::Uint8(255)), "255u8");
        assert_eq!(value_text(&Value::Usize(4)), "4usize");
        assert_eq!(value_text(&Value::Char(u32::from('a'))), "'a'");
        assert_eq!(value_text(&Value::Bool(false)), "false");
        assert_eq!(value_text(&Value::Float64(1.5)), "1.5");
        assert_eq!(value_text(&Value::Float32(0.25)), "0.25f32");
        assert_eq!(
            value_text(&Value::String("abc".to_owned())),
            "<string 3 bytes>"
        );
        assert_eq!(value_text(&Value::Moved), "<moved>");
    }
}
