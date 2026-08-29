use std::collections::BTreeMap;

use crate::ast::UnaryOp;
use crate::bounded_output::{self, BudgetedJoin as _};
use crate::hir::{
    OwnershipMode, Place, PlaceProjection, ResolvedExpr, ResolvedExprKind, ResolvedFunction,
    ValueId,
};

use super::model::type_json;
use super::projection_error;
use super::report_quote_json as quote_json;

macro_rules! bf {
    ($($argument:tt)*) => { bounded_output::budgeted_format(format_args!($($argument)*)) };
}

/// Normalize both contract vectors without expression IDs, display names, or
/// revision-scoped local identities. Unknown roots and constructs that would
/// require a local/pattern identity make the whole export explicitly
/// unproven.
pub(super) fn normalize(
    function: &ResolvedFunction,
) -> Result<(Vec<String>, Vec<String>), crate::diagnostic::Diagnostic> {
    let roots = function
        .params
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            (
                parameter.id.clone(),
                bf!(
                    "{{\"kind\":\"parameter\",\"function\":{},\"index\":{index}}}",
                    quote_json(function.id.as_str())
                ),
            )
        })
        .chain(std::iter::once((
            function.result_id.clone(),
            bf!(
                "{{\"kind\":\"result\",\"function\":{}}}",
                quote_json(function.id.as_str())
            ),
        )))
        .collect::<BTreeMap<_, _>>();
    Ok((
        function
            .requires
            .iter()
            .map(|expression| expression_json(expression, &roots))
            .collect::<Result<Vec<_>, _>>()?,
        function
            .ensures
            .iter()
            .map(|expression| expression_json(expression, &roots))
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

fn expression_json(
    expression: &ResolvedExpr,
    roots: &BTreeMap<ValueId, String>,
) -> Result<String, crate::diagnostic::Diagnostic> {
    let header = bf!(
        "\"type\":{},\"ownership\":{}",
        type_json(&expression.ty),
        quote_json(ownership_text(expression.ownership))
    );
    let output = match &expression.kind {
        ResolvedExprKind::Int(value) => bf!(
            "{{{header},\"kind\":\"int\",\"value\":\"{value}\"}}"
        ),
        ResolvedExprKind::Int32(value) => {
            bf!("{{{header},\"kind\":\"int32\",\"value\":{value}}}")
        }
        ResolvedExprKind::Char(value) => {
            bf!("{{{header},\"kind\":\"char\",\"value\":{value}}}")
        }
        ResolvedExprKind::Uint8(value) => {
            bf!("{{{header},\"kind\":\"uint8\",\"value\":{value}}}")
        }
        ResolvedExprKind::Usize(value) => bf!(
            "{{{header},\"kind\":\"usize\",\"value\":\"{value}\"}}"
        ),
        ResolvedExprKind::ArrayU8(values) => bf!(
            "{{{header},\"kind\":\"array_u8\",\"values\":[{}]}}",
            values
                .iter()
                .map(|value| bf!("{value}"))
                .collect::<Vec<_>>()
                .budgeted_join(",")
        ),
        ResolvedExprKind::RepeatArrayU8 { value, count } => bf!(
            "{{{header},\"kind\":\"repeat_array_u8\",\"value\":{value},\"count\":{count}}}"
        ),
        ResolvedExprKind::Float32(bits) => bf!(
            "{{{header},\"kind\":\"float32\",\"bits\":{}}}",
            quote_json(&bf!("{bits:08x}"))
        ),
        ResolvedExprKind::Float64(bits) => bf!(
            "{{{header},\"kind\":\"float64\",\"bits\":{}}}",
            quote_json(&bf!("{bits:016x}"))
        ),
        ResolvedExprKind::Bool(value) => {
            bf!("{{{header},\"kind\":\"bool\",\"value\":{value}}}")
        }
        ResolvedExprKind::String(value) => bf!(
            "{{{header},\"kind\":\"string\",\"value\":{}}}",
            quote_json(value)
        ),
        ResolvedExprKind::Place(place) => bf!(
            "{{{header},\"kind\":\"place\",\"place\":{}}}",
            place_json(place, roots)?
        ),
        ResolvedExprKind::BorrowPlace { operation, place } => bf!(
            "{{{header},\"kind\":\"borrow_place\",\"operation\":{},\"place\":{}}}",
            quote_json(operation.as_str()),
            place_json(place, roots)?
        ),
        ResolvedExprKind::ByteRange {
            operation,
            source,
            start,
            end,
        } => bf!(
            "{{{header},\"kind\":\"byte_range\",\"operation\":{},\"source\":{},\"start\":{},\"end\":{}}}",
            quote_json(operation.as_str()),
            expression_json(source, roots)?,
            expression_json(start, roots)?,
            expression_json(end, roots)?
        ),
        ResolvedExprKind::Call {
            callee,
            type_arguments,
            instance,
            args,
        } => bf!(
            "{{{header},\"kind\":\"call\",\"callee\":{},\"instance\":{},\"type_arguments\":[{}],\"args\":[{}]}}",
            quote_json(callee.as_str()),
            instance.as_ref().map_or_else(
                || bounded_output::budgeted_clone("\"none\""),
                |value| quote_json(value.as_str())
            ),
            type_arguments.iter().map(type_json).collect::<Vec<_>>().budgeted_join(","),
            args.iter().map(|argument| expression_json(argument, roots)).collect::<Result<Vec<_>, _>>()?.budgeted_join(",")
        ),
        ResolvedExprKind::Unary { op, value } => bf!(
            "{{{header},\"kind\":\"unary\",\"op\":{},\"value\":{}}}",
            quote_json(unary_text(*op)),
            expression_json(value, roots)?
        ),
        ResolvedExprKind::Binary { op, left, right } => bf!(
            "{{{header},\"kind\":\"binary\",\"op\":{},\"left\":{},\"right\":{}}}",
            quote_json(op.text()),
            expression_json(left, roots)?,
            expression_json(right, roots)?
        ),
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => bf!(
            "{{{header},\"kind\":\"if\",\"condition\":{},\"then\":{},\"else\":{}}}",
            expression_json(condition, roots)?,
            expression_json(then_branch, roots)?,
            expression_json(else_branch, roots)?
        ),
        ResolvedExprKind::ConstructRecord { record, fields } => bf!(
            "{{{header},\"kind\":\"construct_record\",\"record\":{},\"fields\":[{}]}}",
            quote_json(record.as_str()),
            fields.iter().map(|field| Ok(bf!("{{\"field\":{},\"value\":{}}}", quote_json(field.field.as_str()), expression_json(&field.value, roots)?))).collect::<Result<Vec<_>, crate::diagnostic::Diagnostic>>()?.budgeted_join(",")
        ),
        ResolvedExprKind::ConstructVariant {
            variant,
            case,
            fields,
        } => bf!(
            "{{{header},\"kind\":\"construct_variant\",\"variant\":{},\"case\":{},\"fields\":[{}]}}",
            quote_json(variant.as_str()),
            quote_json(case.as_str()),
            fields.iter().map(|field| Ok(bf!("{{\"field\":{},\"value\":{}}}", quote_json(field.field.as_str()), expression_json(&field.value, roots)?))).collect::<Result<Vec<_>, crate::diagnostic::Diagnostic>>()?.budgeted_join(",")
        ),
        ResolvedExprKind::UpdateRecord {
            base,
            record,
            fields,
        } => bf!(
            "{{{header},\"kind\":\"update_record\",\"base\":{},\"record\":{},\"fields\":[{}]}}",
            expression_json(base, roots)?,
            quote_json(record.as_str()),
            fields.iter().map(|field| Ok(bf!("{{\"field\":{},\"value\":{}}}", quote_json(field.field.as_str()), expression_json(&field.value, roots)?))).collect::<Result<Vec<_>, crate::diagnostic::Diagnostic>>()?.budgeted_join(",")
        ),
        ResolvedExprKind::Project { base, field } => bf!(
            "{{{header},\"kind\":\"project\",\"base\":{},\"field\":{}}}",
            expression_json(base, roots)?,
            quote_json(field.as_str())
        ),
        ResolvedExprKind::Upcast { source } => bf!(
            "{{{header},\"kind\":\"upcast\",\"source\":{}}}",
            expression_json(source, roots)?
        ),
        ResolvedExprKind::NativeRustImportCall(_)
        | ResolvedExprKind::HostCommandCall(_)
        | ResolvedExprKind::Block { .. }
        | ResolvedExprKind::Match { .. }
        | ResolvedExprKind::Try { .. }
        | ResolvedExprKind::TryOption { .. } => {
            return Err(projection_error(
                "contract requires a revision-local or unsupported identity",
            ));
        }
    };
    Ok(output)
}

fn place_json(
    place: &Place,
    roots: &BTreeMap<ValueId, String>,
) -> Result<String, crate::diagnostic::Diagnostic> {
    let root = roots.get(&place.root).ok_or_else(|| {
        projection_error("contract place is not rooted in a stable parameter or result slot")
    })?;
    Ok(bf!(
        "{{\"root\":{root},\"projections\":[{}]}}",
        place
            .projections
            .iter()
            .map(|projection| match projection {
                PlaceProjection::Field(field) => bf!(
                    "{{\"kind\":\"field\",\"field\":{}}}",
                    quote_json(field.as_str())
                ),
                PlaceProjection::VariantField { case, field } => bf!(
                    "{{\"kind\":\"variant_field\",\"case\":{},\"field\":{}}}",
                    quote_json(case.as_str()),
                    quote_json(field.as_str())
                ),
            })
            .collect::<Vec<_>>()
            .budgeted_join(",")
    ))
}

fn unary_text(op: UnaryOp) -> &'static str {
    match op {
        UnaryOp::Neg => "-",
        UnaryOp::Not => "!",
    }
}

fn ownership_text(ownership: OwnershipMode) -> &'static str {
    match ownership {
        OwnershipMode::Value => "value",
        OwnershipMode::Own => "own",
        OwnershipMode::Borrow => "borrow",
        OwnershipMode::Shared => "shared",
    }
}
