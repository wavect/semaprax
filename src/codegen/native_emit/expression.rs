use std::collections::BTreeSet;

use crate::aggregate_layout::AggregateLayout;
use crate::ast::{BinaryOp, UnaryOp};
use crate::bounded_output::BudgetedJoin as _;
use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, DeclarationId, ExpressionId, FunctionExecutionId, PlaceProjection, ResolvedExpr,
    ResolvedExprKind, ResolvedStatement, ResolvedType,
};
use crate::variant_layout::VariantLayout;

use super::{
    backend_error, c_case_symbol, c_field_symbol, c_i64, c_pattern_literal, c_string, c_value_type,
    is_aggregate_type, record_declaration_id, variant_declaration_id, CBinding, CEmitter, COutput,
    CValue,
};

mod host_command;
mod nested_owned;
mod owned_values;

#[derive(Clone)]
struct RecordMatchBindingMode<'a> {
    mode: hir::ResolvedMatchMode,
    source_storage: Option<&'a crate::cleanup_plan::StorageId>,
    source_path: Vec<DeclarationId>,
}

// `format!` resolves to the bounded codegen macro declared before
// `mod native_emit`; it must never fall back to `std::format!` here.
impl<'a, O: COutput> CEmitter<'a, O> {
    /// Refutable Match v1 native lowering: the scrutinee stages once, then
    /// every arm tests `!matched && (<literal equality>)` with an optional
    /// inner guard branch. `&&` short-circuits so a guard evaluates exactly
    /// once per reached arm whose pattern matched; failing guards leave
    /// `matched` false and fall through to the following arms. The resolver
    /// guarantees one trailing irrefutable guard-free catch-all, but the
    /// defensive no-arm check mirrors exhaustive matches.
    fn emit_scalar_match(
        &mut self,
        expr: &ResolvedExpr,
        scrutinee: &CValue,
        arms: &[hir::ResolvedMatchArm],
    ) -> Result<CValue, Diagnostic> {
        let staged = self.temporary(&scrutinee.ty)?;
        self.line(&format!("{staged} = {};", scrutinee.code));
        let result = if matches!(expr.ty, ResolvedType::Bytes) {
            self.bytes_plan
                .ok_or_else(|| backend_error("owned Bytes match has no cleanup plan"))?
                .value(&crate::cleanup_plan::StorageId::Temporary(expr.id.clone()))?
                .to_owned()
        } else {
            self.temporary(&expr.ty)?
        };
        let matched = self.temporary(&ResolvedType::Bool)?;
        self.line(&format!("{matched} = false;"));
        for arm in arms {
            let saved = self.variables.clone();
            if let hir::ResolvedMatchPattern::Binding(binding) = &arm.pattern {
                self.variables.insert(
                    binding.id.clone(),
                    CBinding {
                        name: staged.clone(),
                        ty: binding.ty.clone(),
                    },
                );
            }
            let test = match &arm.pattern {
                hir::ResolvedMatchPattern::Wildcard | hir::ResolvedMatchPattern::Binding(_) => None,
                hir::ResolvedMatchPattern::Literal(value) => {
                    Some(format!("{staged} == {}", c_pattern_literal(*value)))
                }
                hir::ResolvedMatchPattern::Or(alternatives) => Some(
                    alternatives
                        .iter()
                        .map(|alternative| match alternative {
                            hir::ResolvedMatchPattern::Literal(value) => {
                                format!("{staged} == {}", c_pattern_literal(*value))
                            }
                            _ => unreachable!("or-pattern alternatives are literals"),
                        })
                        .collect::<Vec<_>>()
                        .join(" || "),
                ),
                hir::ResolvedMatchPattern::Variant { .. }
                | hir::ResolvedMatchPattern::Record { .. } => {
                    return Err(backend_error(
                        "aggregate pattern has a Copy-scalar match scrutinee",
                    ));
                }
            };
            match &test {
                Some(test) => self.line(&format!("if (!{matched} && ({test})) {{")),
                None => self.line(&format!("if (!{matched}) {{")),
            }
            self.indent += 1;
            if let Some(guard) = &arm.guard {
                // The guard evaluates once here, after the pattern matched
                // and before any part of the arm value; a false guard leaves
                // `matched` untouched and falls through to the next arm.
                let flag = self.emit_expr(guard)?;
                self.require_type(&flag.ty, &ResolvedType::Bool, "match guard")?;
                self.line(&format!("if ({}) {{", flag.code));
                self.indent += 1;
                self.line(&format!("{matched} = true;"));
                let value = self.emit_expr(&arm.value)?;
                self.require_type(&value.ty, &expr.ty, "match arm result")?;
                if matches!(expr.ty, ResolvedType::Bytes) {
                    let transitions = self
                        .bytes_plan
                        .expect("checked above")
                        .apply_at(&arm.value.id)?;
                    for line in transitions.lines() {
                        self.line(line);
                    }
                } else if matches!(value.ty, ResolvedType::String) && self.owned_strings.is_some() {
                    self.string_move(&result, &value.code);
                } else {
                    self.line(&format!("{result} = {};", value.code));
                }
                self.indent -= 1;
                self.line("}");
            } else {
                self.line(&format!("{matched} = true;"));
                let value = self.emit_expr(&arm.value)?;
                self.require_type(&value.ty, &expr.ty, "match arm result")?;
                if matches!(expr.ty, ResolvedType::Bytes) {
                    let transitions = self
                        .bytes_plan
                        .expect("checked above")
                        .apply_at(&arm.value.id)?;
                    for line in transitions.lines() {
                        self.line(line);
                    }
                } else if matches!(value.ty, ResolvedType::String) && self.owned_strings.is_some() {
                    self.string_move(&result, &value.code);
                } else {
                    self.line(&format!("{result} = {};", value.code));
                }
            }
            self.variables = saved;
            self.indent -= 1;
            self.line("}");
        }
        self.line(&format!(
            "if (!{matched}) spx_runtime_invariant_failure(\"refutable match selected no arm\");"
        ));
        Ok(CValue {
            code: result,
            ty: expr.ty.clone(),
        })
    }

    fn emit_string_op(
        &mut self,
        op: crate::string_ops::StringOp,
        args: &[ResolvedExpr],
        result_type: &ResolvedType,
    ) -> Result<CValue, Diagnostic> {
        // Arguments stage left-to-right; every argument evaluation yields a
        // fresh caller-owned buffer, and consuming operations free their
        // inputs exactly at the operation site like owned string equality.
        let mut arguments = Vec::with_capacity(args.len());
        for (index, argument) in args.iter().enumerate() {
            let value = self.emit_expr(argument)?;
            self.require_type(
                &value.ty,
                &op.param_types()[index],
                "string operation argument",
            )?;
            arguments.push(value);
        }
        self.require_type(result_type, &op.return_type(), "string operation result")?;
        let temporary = self.temporary(&op.return_type())?;
        match op {
            crate::string_ops::StringOp::Len => {
                let input = &arguments[0].code;
                self.line(&format!("{temporary} = spx_string_len({input});"));
                self.string_drop(input);
            }
            crate::string_ops::StringOp::IsEmpty => {
                let input = &arguments[0].code;
                self.line(&format!("{temporary} = spx_string_is_empty({input});"));
                self.string_drop(input);
            }
            crate::string_ops::StringOp::Concat => {
                let left = &arguments[0].code;
                let right = &arguments[1].code;
                self.line(&format!(
                    "{temporary} = spx_string_concat({left}, {right});"
                ));
                self.string_drop(left);
                self.string_drop(right);
            }
            crate::string_ops::StringOp::StartsWith => {
                let value = &arguments[0].code;
                let prefix = &arguments[1].code;
                self.line(&format!(
                    "{temporary} = spx_string_starts_with({value}, {prefix});"
                ));
                self.string_drop(value);
                self.string_drop(prefix);
            }
            crate::string_ops::StringOp::Contains => {
                let value = &arguments[0].code;
                let needle = &arguments[1].code;
                self.line(&format!(
                    "{temporary} = spx_string_contains({value}, {needle});"
                ));
                self.string_drop(value);
                self.string_drop(needle);
            }
            crate::string_ops::StringOp::LenChars => {
                let input = &arguments[0].code;
                self.line(&format!("{temporary} = spx_string_len_chars({input});"));
                self.string_drop(input);
            }
            crate::string_ops::StringOp::FromChar => {
                let scalar = &arguments[0].code;
                self.line(&format!("{temporary} = spx_string_from_char({scalar});"));
            }
            crate::string_ops::StringOp::FromI64 => {
                let value = &arguments[0].code;
                self.line(&format!("{temporary} = spx_string_from_i64({value});"));
            }
            crate::string_ops::StringOp::FromUsize => {
                let value = &arguments[0].code;
                self.line(&format!("{temporary} = spx_string_from_usize({value});"));
            }
        }
        if matches!(op.return_type(), ResolvedType::String) {
            self.string_initialize(&temporary);
        }
        Ok(CValue {
            code: temporary,
            ty: op.return_type(),
        })
    }

    fn emit_str_op(
        &mut self,
        op: crate::str_ops::StrOp,
        args: &[ResolvedExpr],
        result_type: &ResolvedType,
    ) -> Result<CValue, Diagnostic> {
        // A borrowed view is copied as a two-word carrier only. Operations do
        // not allocate, clone, retain, consume, or drop either source view.
        let mut arguments = Vec::with_capacity(args.len());
        if args.len() != op.arity() {
            return Err(backend_error(format!(
                "borrowed str operation `{}` has {} arguments; expected {}",
                op.id(),
                args.len(),
                op.arity()
            )));
        }
        for argument in args {
            let value = self.emit_expr(argument)?;
            self.require_type(
                &value.ty,
                &ResolvedType::Str,
                "borrowed str operation argument",
            )?;
            arguments.push(value);
        }
        self.require_type(
            result_type,
            &op.return_type(),
            "borrowed str operation result",
        )?;
        let temporary = self.temporary(&op.return_type())?;
        match op {
            crate::str_ops::StrOp::LenBytes => self.line(&format!(
                "{temporary} = spx_str_len_bytes({});",
                arguments[0].code
            )),
            crate::str_ops::StrOp::IsEmpty => self.line(&format!(
                "{temporary} = spx_str_is_empty({});",
                arguments[0].code
            )),
            crate::str_ops::StrOp::StartsWith => self.line(&format!(
                "{temporary} = spx_str_starts_with({}, {});",
                arguments[0].code, arguments[1].code
            )),
            crate::str_ops::StrOp::Contains => self.line(&format!(
                "{temporary} = spx_str_contains({}, {});",
                arguments[0].code, arguments[1].code
            )),
        }
        Ok(CValue {
            code: temporary,
            ty: op.return_type(),
        })
    }

    fn emit_byte_op(
        &mut self,
        op: crate::byte_ops::ByteOp,
        args: &[ResolvedExpr],
        result_type: &ResolvedType,
        expression: &ExpressionId,
    ) -> Result<CValue, Diagnostic> {
        if args.len() != op.arity() {
            return Err(backend_error(format!(
                "byte operation `{}` has {} arguments; expected {}",
                op.id(),
                args.len(),
                op.arity()
            )));
        }
        let mut arguments = Vec::with_capacity(args.len());
        for (argument, expected) in args.iter().zip(op.param_types()) {
            let value = self.emit_expr(argument)?;
            self.require_type(&value.ty, expected, "byte operation argument")?;
            arguments.push(value);
        }
        let return_type = op.return_type();
        self.require_type(result_type, &return_type, "byte operation result")?;
        let temporary = if matches!(op, crate::byte_ops::ByteOp::Copy) {
            self.bytes_plan
                .as_ref()
                .ok_or_else(|| backend_error("bytes_copy has no canonical cleanup plan"))?
                .value(&crate::cleanup_plan::StorageId::Temporary(
                    expression.clone(),
                ))?
                .to_owned()
        } else {
            self.temporary(&return_type)?
        };
        match op {
            crate::byte_ops::ByteOp::Len => {
                self.line(&format!(
                    "{temporary} = spx_byte_len({});",
                    arguments[0].code
                ));
            }
            crate::byte_ops::ByteOp::Get => {
                let layout = self.variant_layout(&return_type)?;
                let none_id = DeclarationId::new(crate::prelude::OPTION_NONE_ID);
                let some_id = DeclarationId::new(crate::prelude::OPTION_SOME_ID);
                let value_id = DeclarationId::new(crate::prelude::OPTION_SOME_VALUE_ID);
                let none = layout.case(&none_id).ok_or_else(|| {
                    backend_error("Option<u8> layout has no compiler-owned None case")
                })?;
                let some = layout.case(&some_id).ok_or_else(|| {
                    backend_error("Option<u8> layout has no compiler-owned Some case")
                })?;
                let field = some.field(&value_id).ok_or_else(|| {
                    backend_error("Option<u8> layout has no compiler-owned Some payload")
                })?;
                self.require_type(&field.ty, &ResolvedType::U8, "byte_get Some payload")?;
                let slice = &arguments[0].code;
                let index = &arguments[1].code;
                self.line(&format!("spx_slice_u8_require_valid({slice});"));
                self.line(&format!("memset(&{temporary}, 0, sizeof({temporary}));"));
                self.line(&format!("if ({index} < ({slice}).len) {{"));
                self.indent += 1;
                self.line(&format!(
                    "{temporary}.spx_payload.{}.{} = ({slice}).ptr[{index}];",
                    c_case_symbol(&some_id),
                    c_field_symbol(&value_id)
                ));
                self.line(&format!("{temporary}.spx_tag = UINT32_C({});", some.tag));
                self.indent -= 1;
                self.line("} else {");
                self.indent += 1;
                self.line(&format!("{temporary}.spx_tag = UINT32_C({});", none.tag));
                self.indent -= 1;
                self.line("}");
            }
            crate::byte_ops::ByteOp::Copy => {
                self.line(&format!(
                    "{temporary} = spx_bytes_copy({});",
                    arguments[0].code
                ));
            }
            crate::byte_ops::ByteOp::Range => {
                return Err(backend_error(
                    "byte_range reached native lowering as an ordinary call",
                ));
            }
            crate::byte_ops::ByteOp::BytesAsSlice
            | crate::byte_ops::ByteOp::ArrayAsSlice
            | crate::byte_ops::ByteOp::StrAsBytes
            | crate::byte_ops::ByteOp::StringAsStr => {
                return Err(backend_error(format!(
                    "borrowed view `{}` reached native lowering without authenticated BorrowPlace HIR",
                    op.id()
                )));
            }
        }
        let mut code = temporary;
        if let Some(plan) = self.bytes_plan {
            let transitions = plan.apply_at(expression)?;
            for line in transitions.lines() {
                self.line(line);
            }
            if matches!(return_type, ResolvedType::Bytes) {
                code = plan
                    .result_at(expression)
                    .ok_or_else(|| backend_error("bytes_copy has no initialized result slot"))?
                    .to_owned();
            }
        }
        Ok(CValue {
            code,
            ty: return_type,
        })
    }

    fn emit_leaf_expr(&mut self, expr: &ResolvedExpr) -> Result<CValue, Diagnostic> {
        let value = match &expr.kind {
            ResolvedExprKind::Int(value) => {
                self.require_type(&expr.ty, &ResolvedType::I64, "integer literal")?;
                CValue {
                    code: c_i64(*value),
                    ty: ResolvedType::I64,
                }
            }
            ResolvedExprKind::Int32(value) => {
                self.require_type(&expr.ty, &ResolvedType::I32, "i32 literal")?;
                CValue {
                    code: format!("INT32_C({value})"),
                    ty: ResolvedType::I32,
                }
            }
            ResolvedExprKind::Char(value) => {
                self.require_type(&expr.ty, &ResolvedType::Char, "char literal")?;
                CValue {
                    code: format!("UINT32_C(0x{value:x})"),
                    ty: ResolvedType::Char,
                }
            }
            ResolvedExprKind::Uint8(value) => {
                self.require_type(&expr.ty, &ResolvedType::U8, "u8 literal")?;
                CValue {
                    code: format!("UINT8_C({value})"),
                    ty: ResolvedType::U8,
                }
            }
            ResolvedExprKind::Usize(value) => {
                self.require_type(&expr.ty, &ResolvedType::Usize, "usize literal")?;
                CValue {
                    code: format!("UINT64_C({value})"),
                    ty: ResolvedType::Usize,
                }
            }
            ResolvedExprKind::ArrayU8(values) => {
                let expected = ResolvedType::ArrayU8(
                    u32::try_from(values.len())
                        .map_err(|_| backend_error("fixed byte array length exceeds u32"))?,
                );
                self.require_type(&expr.ty, &expected, "fixed byte array literal")?;
                if values.is_empty() {
                    return Ok(CValue {
                        code: "UINT8_C(0)".to_owned(),
                        ty: expr.ty.clone(),
                    });
                } else {
                    let temporary = self.temporary(&expr.ty)?;
                    let bytes = values
                        .iter()
                        .map(|value| format!("UINT8_C({value})"))
                        .collect::<Vec<_>>()
                        .budgeted_join(", ");
                    self.line(&format!(
                        "{temporary} = (struct spx_array_u8_{}) {{ .spx_bytes = {{ {bytes} }} }};",
                        values.len()
                    ));
                    CValue {
                        code: temporary,
                        ty: expr.ty.clone(),
                    }
                }
            }
            ResolvedExprKind::RepeatArrayU8 { value, count } => {
                let expected = ResolvedType::ArrayU8(*count);
                self.require_type(&expr.ty, &expected, "repeated fixed byte array literal")?;
                if *count == 0 {
                    return Ok(CValue {
                        code: "UINT8_C(0)".to_owned(),
                        ty: expr.ty.clone(),
                    });
                } else {
                    let temporary = self.temporary(&expr.ty)?;
                    self.line(&format!(
                        "memset({temporary}.spx_bytes, UINT8_C({value}), UINT32_C({count}));"
                    ));
                    CValue {
                        code: temporary,
                        ty: expr.ty.clone(),
                    }
                }
            }
            ResolvedExprKind::Float32(bits) => {
                self.require_type(&expr.ty, &ResolvedType::F32, "float literal")?;
                CValue {
                    code: format!("{}f", crate::format::canonical_f32_bits(*bits)),
                    ty: ResolvedType::F32,
                }
            }
            ResolvedExprKind::Float64(bits) => {
                self.require_type(&expr.ty, &ResolvedType::F64, "float literal")?;
                CValue {
                    code: crate::format::canonical_f64_bits(*bits),
                    ty: ResolvedType::F64,
                }
            }
            ResolvedExprKind::Bool(value) => {
                self.require_type(&expr.ty, &ResolvedType::Bool, "boolean literal")?;
                CValue {
                    code: value.to_string(),
                    ty: ResolvedType::Bool,
                }
            }
            ResolvedExprKind::String(value) => {
                self.require_type(&expr.ty, &ResolvedType::String, "string literal")?;
                let temporary = self.temporary(&ResolvedType::String)?;
                self.line(&format!(
                    "{temporary} = spx_string_from_literal(\"{}\", UINT64_C({}));",
                    c_string(value),
                    value.len()
                ));
                self.string_initialize(&temporary);
                CValue {
                    code: temporary,
                    ty: ResolvedType::String,
                }
            }
            ResolvedExprKind::Place(place) => {
                let value = self.emit_place(place)?;
                self.require_type(&expr.ty, &value.ty, "place expression")?;
                // Every read of an owned string place yields a fresh buffer so
                // the source place keeps its unique owner.
                if matches!(value.ty, ResolvedType::String) {
                    let temporary = self.temporary(&ResolvedType::String)?;
                    self.line(&format!("{temporary} = spx_string_clone({});", value.code));
                    self.string_initialize(&temporary);
                    return Ok(CValue {
                        code: temporary,
                        ty: value.ty,
                    });
                }
                if is_aggregate_type(self.program, &value.ty)? {
                    self.apply_owned_plan_at_value(&expr.id, &value)?;
                }
                value
            }
            ResolvedExprKind::BorrowPlace { operation, place } => {
                let op = crate::byte_ops::by_id(operation.as_str()).ok_or_else(|| {
                    backend_error(format!(
                        "unknown compiler-owned byte view identity `{operation}`"
                    ))
                })?;
                if !op.is_view() {
                    return Err(backend_error(format!(
                        "non-view byte operation `{operation}` used BorrowPlace HIR"
                    )));
                }
                let source = self.emit_place(place)?;
                let temporary = self.temporary(&expr.ty)?;
                match op {
                    crate::byte_ops::ByteOp::BytesAsSlice => {
                        self.require_type(
                            &source.ty,
                            &ResolvedType::Bytes,
                            "owned byte borrow source",
                        )?;
                        self.line(&format!(
                            "{temporary} = spx_bytes_as_slice(&({}));",
                            source.code
                        ));
                    }
                    crate::byte_ops::ByteOp::ArrayAsSlice => {
                        let ResolvedType::ArrayU8(length) = source.ty else {
                            return Err(backend_error(
                                "fixed byte array borrow has a non-array source",
                            ));
                        };
                        if length == 0 {
                            self.line(&format!(
                                "{temporary} = (spx_slice_u8_v1) {{ .ptr = NULL, .len = UINT64_C(0) }};"
                            ));
                        } else {
                            self.line(&format!(
                                "{temporary} = (spx_slice_u8_v1) {{ .ptr = ({}).spx_bytes, .len = UINT64_C({length}) }};",
                                source.code
                            ));
                        }
                        self.line(&format!("spx_slice_u8_require_valid({temporary});"));
                    }
                    crate::byte_ops::ByteOp::StrAsBytes => {
                        self.require_type(
                            &source.ty,
                            &ResolvedType::Str,
                            "borrowed UTF-8 byte view source",
                        )?;
                        self.line(&format!("spx_str_require_valid({});", source.code));
                        self.line(&format!(
                            "{temporary} = (spx_slice_u8_v1) {{ .ptr = ({}).len == UINT64_C(0) ? NULL : (const uint8_t *)({}).data, .len = ({}).len }};",
                            source.code, source.code, source.code
                        ));
                        self.line(&format!("spx_slice_u8_require_valid({temporary});"));
                    }
                    crate::byte_ops::ByteOp::StringAsStr => {
                        self.require_type(
                            &source.ty,
                            &ResolvedType::String,
                            "owned UTF-8 borrow source",
                        )?;
                        self.line(&format!(
                            "{temporary} = spx_string_as_str({});",
                            source.code
                        ));
                        self.line(&format!("spx_str_require_valid({temporary});"));
                    }
                    crate::byte_ops::ByteOp::Len
                    | crate::byte_ops::ByteOp::Get
                    | crate::byte_ops::ByteOp::Range
                    | crate::byte_ops::ByteOp::Copy => unreachable!(),
                }
                CValue {
                    code: temporary,
                    ty: expr.ty.clone(),
                }
            }
            _ => unreachable!("non-leaf expression reached native leaf lowering"),
        };
        self.require_type(&value.ty, &expr.ty, "expression")?;
        Ok(value)
    }

    pub(super) fn emit_expr(&mut self, expr: &ResolvedExpr) -> Result<CValue, Diagnostic> {
        match &expr.kind {
            ResolvedExprKind::Int(_)
            | ResolvedExprKind::Int32(_)
            | ResolvedExprKind::Char(_)
            | ResolvedExprKind::Uint8(_)
            | ResolvedExprKind::Usize(_)
            | ResolvedExprKind::ArrayU8(_)
            | ResolvedExprKind::RepeatArrayU8 { .. }
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_)
            | ResolvedExprKind::Bool(_)
            | ResolvedExprKind::String(_)
            | ResolvedExprKind::Place(_)
            | ResolvedExprKind::BorrowPlace { .. } => self.emit_leaf_expr(expr),
            ResolvedExprKind::ByteRange {
                operation,
                source,
                start,
                end,
            } => self.emit_byte_range_expr(expr, operation, source, start, end),
            ResolvedExprKind::HostCommandCall(_) => self.emit_host_command_expr(expr),
            ResolvedExprKind::Call { .. } => self.emit_call_expr(expr),
            ResolvedExprKind::NativeRustImportCall(call) => Err(backend_error(format!(
                "native Rust import `{}` is unavailable in the ordinary native backend",
                call.import
            ))),
            ResolvedExprKind::Unary { op, value } => self.emit_unary_expr(expr, *op, value),
            ResolvedExprKind::Binary { op, left, right } => {
                self.emit_binary(*op, left, right, &expr.ty)
            }
            ResolvedExprKind::Block { statements, .. } if statements.is_empty() => {
                self.emit_empty_block_expr(expr)
            }
            ResolvedExprKind::Block { .. } => self.emit_block_expr(expr),
            ResolvedExprKind::If { .. } => self.emit_if_expr(expr),
            ResolvedExprKind::ConstructRecord { .. } => self.emit_construct_record_expr(expr),
            ResolvedExprKind::ConstructVariant { .. } => self.emit_construct_variant_expr(expr),
            ResolvedExprKind::Match { .. } => self.emit_match_expr(expr),
            ResolvedExprKind::Try { .. } => self.emit_try_expr(expr),
            ResolvedExprKind::TryOption { .. } => self.emit_try_option_expr(expr),
            ResolvedExprKind::Project { .. } => self.emit_project_expr(expr),
            ResolvedExprKind::Upcast { .. } => self.emit_upcast_expr(expr),
            ResolvedExprKind::UpdateRecord { .. } => self.emit_update_record_expr(expr),
        }
    }

    fn emit_byte_range_expr(
        &mut self,
        expr: &ResolvedExpr,
        operation: &DeclarationId,
        source: &ResolvedExpr,
        start: &ResolvedExpr,
        end: &ResolvedExpr,
    ) -> Result<CValue, Diagnostic> {
        if operation.as_str() != crate::byte_ops::RANGE_ID {
            return Err(backend_error(
                "byte_range HIR has an unknown operation identity",
            ));
        }
        let source = self.emit_expr(source)?;
        self.require_type(&source.ty, &ResolvedType::SliceU8, "byte_range source")?;
        let start = self.emit_expr(start)?;
        self.require_type(&start.ty, &ResolvedType::Usize, "byte_range start")?;
        let end = self.emit_expr(end)?;
        self.require_type(&end.ty, &ResolvedType::Usize, "byte_range end")?;
        self.require_type(&expr.ty, &ResolvedType::SliceU8, "byte_range result")?;
        let temporary = self.temporary(&ResolvedType::SliceU8)?;
        self.line(&format!(
            "spx_status = spx_byte_range_v1(spx_ctx, {}, {}, {}, &{temporary});",
            source.code, start.code, end.code
        ));
        self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
        if let Some(plan) = self.bytes_plan {
            let transitions = plan.apply_at(&expr.id)?;
            for line in transitions.lines() {
                self.line(line);
            }
        }
        Ok(CValue {
            code: temporary,
            ty: ResolvedType::SliceU8,
        })
    }

    fn emit_unary_expr(
        &mut self,
        expr: &ResolvedExpr,
        op: UnaryOp,
        operand: &ResolvedExpr,
    ) -> Result<CValue, Diagnostic> {
        let value = self.emit_expr(operand)?;
        self.emit_unary_value(expr, op, value)
    }

    fn emit_unary_value(
        &mut self,
        expr: &ResolvedExpr,
        op: UnaryOp,
        value: CValue,
    ) -> Result<CValue, Diagnostic> {
        let (ty, operand_type) = match op {
            UnaryOp::Neg => match &value.ty {
                ResolvedType::F32 => (ResolvedType::F32, ResolvedType::F32),
                ResolvedType::F64 => (ResolvedType::F64, ResolvedType::F64),
                ResolvedType::I32 => (ResolvedType::I32, ResolvedType::I32),
                _ => (ResolvedType::I64, ResolvedType::I64),
            },
            UnaryOp::Not => (ResolvedType::Bool, ResolvedType::Bool),
        };
        self.require_type(&value.ty, &operand_type, "unary operand")?;
        self.require_type(&expr.ty, &ty, "unary result")?;
        let temporary = self.temporary(&ty)?;
        match op {
            UnaryOp::Neg if matches!(ty, ResolvedType::F32 | ResolvedType::F64) => {
                self.line(&format!("{temporary} = (-({}));", value.code));
            }
            UnaryOp::Neg if ty == ResolvedType::I32 => {
                self.line(&format!(
                    "spx_status = spx_rt_neg_i32(spx_ctx, {}, &{temporary});",
                    value.code
                ));
                self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
            }
            UnaryOp::Neg => {
                self.line(&format!(
                    "spx_status = spx_rt_neg(spx_ctx, {}, &{temporary});",
                    value.code
                ));
                self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
            }
            UnaryOp::Not => self.line(&format!("{temporary} = (!{});", value.code)),
        }
        let value = CValue {
            code: temporary,
            ty,
        };
        self.require_type(&value.ty, &expr.ty, "expression")?;
        Ok(value)
    }

    fn emit_call_expr(&mut self, expr: &ResolvedExpr) -> Result<CValue, Diagnostic> {
        match &expr.kind {
            ResolvedExprKind::Call {
                callee,
                instance,
                args,
                ..
            } => {
                if instance.is_none() {
                    if crate::host_io_ops::by_id(callee.as_str()).is_some() {
                        if !self.output_profile.supports_stdout_transcript() {
                            return Err(backend_error(
                                "host stdout write requires the native stdout-transcript profile",
                            ));
                        }
                        if args.len() != 1 {
                            return Err(backend_error(
                                "host stdout write arity disagrees with resolved HIR",
                            ));
                        }
                        let value = self.emit_expr(&args[0])?;
                        self.require_type(
                            &value.ty,
                            &ResolvedType::SliceU8,
                            "host stdout write argument",
                        )?;
                        self.require_type(
                            &expr.ty,
                            &ResolvedType::Usize,
                            "host stdout write result",
                        )?;
                        let temporary = self.temporary(&ResolvedType::Usize)?;
                        let helper = if self.output_profile.is_language_command() {
                            "spx_host_command_stdout_write_v1"
                        } else {
                            "spx_host_stdout_write_v1"
                        };
                        self.line(&format!("{temporary} = {helper}(spx_ctx, {});", value.code));
                        return Ok(CValue {
                            code: temporary,
                            ty: ResolvedType::Usize,
                        });
                    }
                    if let Some(op) = crate::str_ops::by_id(callee.as_str()) {
                        return self.emit_str_op(op, args, &expr.ty);
                    }
                    if let Some(op) = crate::byte_ops::by_id(callee.as_str()) {
                        return self.emit_byte_op(op, args, &expr.ty, &expr.id);
                    }
                    if let Some(op) = crate::string_ops::by_id(callee.as_str()) {
                        return self.emit_string_op(op, args, &expr.ty);
                    }
                }
                self.emit_user_call_expr(expr, callee, instance.as_ref(), args)
            }
            _ => unreachable!("non-Call expression reached emit_call_expr"),
        }
    }

    fn emit_user_call_expr(
        &mut self,
        expr: &ResolvedExpr,
        callee: &hir::DeclarationId,
        instance: Option<&hir::FunctionInstanceId>,
        args: &[ResolvedExpr],
    ) -> Result<CValue, Diagnostic> {
        struct PendingCall<'a> {
            expr: &'a ResolvedExpr,
            args: &'a [ResolvedExpr],
            target: super::CFunction,
        }

        let mut pending = Vec::new();
        let mut current_expr = expr;
        let mut current_callee = callee;
        let mut current_instance = instance;
        let mut current_args = args;
        let mut value = loop {
            let execution = current_instance.map_or_else(
                || FunctionExecutionId::Monomorphic(current_callee.clone()),
                |instance| FunctionExecutionId::Generic(instance.clone()),
            );
            let target = self.functions.get(&execution).cloned().ok_or_else(|| {
                backend_error(format!("resolved callee `{current_callee}` is not indexed"))
            })?;
            if current_args.len() != target.params.len() {
                return Err(backend_error(format!(
                    "resolved call to `{current_callee}` has {} arguments; expected {}",
                    current_args.len(),
                    target.params.len()
                )));
            }
            pending.push(PendingCall {
                expr: current_expr,
                args: current_args,
                target,
            });
            let Some(first) = current_args.first() else {
                break None;
            };
            let ResolvedExprKind::Call {
                callee,
                instance,
                args,
                ..
            } = &first.kind
            else {
                break Some(self.emit_expr(first)?);
            };
            let execution = instance.as_ref().map_or_else(
                || FunctionExecutionId::Monomorphic(callee.clone()),
                |instance| FunctionExecutionId::Generic(instance.clone()),
            );
            if !self.functions.contains_key(&execution) {
                break Some(self.emit_expr(first)?);
            }
            current_expr = first;
            current_callee = callee;
            current_instance = instance.as_ref();
            current_args = args;
        };

        while let Some(call) = pending.pop() {
            let mut values = Vec::with_capacity(call.args.len());
            if let Some(first) = value.take() {
                values.push(self.stage_bytes_call_argument(
                    call.expr,
                    0,
                    &call.args[0],
                    call.target.param_ownerships[0],
                    first,
                )?);
            }
            for (index, argument) in call.args.iter().enumerate().skip(values.len()) {
                let value = self.emit_expr(argument)?;
                values.push(self.stage_bytes_call_argument(
                    call.expr,
                    index,
                    argument,
                    call.target.param_ownerships[index],
                    value,
                )?);
            }
            value = Some(self.emit_user_call_values(call.expr, &call.target, call.args, values)?);
        }
        value.ok_or_else(|| backend_error("user-call traversal produced no result"))
    }

    fn emit_user_call_values(
        &mut self,
        expr: &ResolvedExpr,
        target: &super::CFunction,
        args: &[ResolvedExpr],
        values: Vec<CValue>,
    ) -> Result<CValue, Diagnostic> {
        let mut arguments = Vec::with_capacity(args.len());
        let mut string_arguments = Vec::new();
        for (index, (expected, argument)) in target.params.iter().zip(values).enumerate() {
            self.require_type(&argument.ty, expected, &format!("call argument {index}"))?;
            arguments.push(
                if matches!(expected, ResolvedType::String) && self.owned_strings.is_some() {
                    // Aliases carry values only across the non-failing commit;
                    // the caller cells remain the sole owners until all staging ends.
                    let alias = format!("spx_string_argument_{}", self.next_local);
                    self.next_local += 1;
                    self.line(&format!("char *{alias} = {};", argument.code));
                    string_arguments.push(argument.code);
                    alias
                } else if matches!(expected, ResolvedType::Bytes) {
                    match target.param_ownerships[index] {
                        hir::OwnershipMode::Own => {
                            let plan = self.bytes_plan.ok_or_else(|| {
                                backend_error("owned Bytes call has no canonical cleanup plan")
                            })?;
                            let parameter_index = u32::try_from(index).map_err(|_| {
                                backend_error("native call has too many parameters")
                            })?;
                            let (value, _) = plan.call_argument(&expr.id, parameter_index)?;
                            if argument.code != value {
                                return Err(backend_error(
                                    "owned Bytes call argument was not staged in its canonical epoch",
                                ));
                            }
                            format!("spx_bytes_move(&{value})")
                        }
                        hir::OwnershipMode::Borrow => format!("&({})", argument.code),
                        _ => {
                            return Err(backend_error(
                                "Bytes call argument lacks validated ownership classification",
                            ));
                        }
                    }
                } else if is_aggregate_type(self.program, expected)? {
                    if target.param_ownerships[index] == hir::OwnershipMode::Own {
                        if let Some(plan) = self.bytes_plan {
                            let parameter_index = u32::try_from(index).map_err(|_| {
                                backend_error("native call has too many parameters")
                            })?;
                            let storage = plan.call_argument_storage(&expr.id, parameter_index)?;
                            let materialize = if plan.has_variant_leaves(&storage) {
                                let layout = self.variant_layout(expected)?;
                                plan.materialize_variant_carrier(&storage, &argument.code, &layout)?
                            } else {
                                plan.materialize_record_carrier(&storage, &argument.code)?
                            };
                            for line in materialize.lines() {
                                self.line(line);
                            }
                        }
                    }
                    format!("&({})", argument.code)
                } else {
                    argument.code
                },
            );
            if is_aggregate_type(self.program, expected)?
                && target.param_ownerships[index] == hir::OwnershipMode::Borrow
            {
                let ResolvedExprKind::Place(place) = &args[index].kind else {
                    return Err(backend_error(
                        "borrowed owned-record argument is not one authenticated place",
                    ));
                };
                if !place.projections.is_empty() {
                    return Err(backend_error(
                        "borrowed owned-record argument is not one root place",
                    ));
                }
                let storage = crate::cleanup_plan::StorageId::Value(place.root.clone());
                for path in super::borrowed_aggregate_byte_paths(
                    self.program,
                    self.record_layouts,
                    self.variant_layouts,
                    expected,
                )? {
                    let alias = self
                        .bytes_plan
                        .and_then(|plan| {
                            plan.value_at(&crate::cleanup_plan::CleanupPlace {
                                storage: storage.clone(),
                                projections: path.clone(),
                            })
                            .ok()
                            .map(str::to_owned)
                        })
                        .or_else(|| {
                            self.borrowed_aggregate_bytes
                                .get(&(place.root.clone(), path.clone()))
                                .cloned()
                        })
                        .ok_or_else(|| {
                            backend_error(
                                "borrowed owned-record call has no authenticated field alias",
                            )
                        })?;
                    arguments.push(format!("&({alias})"));
                }
            }
        }
        self.require_type(&expr.ty, &target.return_type, "call result")?;
        let temporary = if matches!(target.return_type, ResolvedType::Bytes) {
            self.bytes_plan
                .ok_or_else(|| backend_error("owned Bytes call result has no cleanup plan"))?
                .value(&crate::cleanup_plan::StorageId::Temporary(expr.id.clone()))?
                .to_owned()
        } else {
            self.call_result_temporary(&target.return_type)?
        };
        for source in &string_arguments {
            self.line(&format!(
                "if (!{source}_live) spx_runtime_invariant_failure(\"dead String argument\");"
            ));
        }
        for source in string_arguments {
            self.line(&format!("{source}_live = false;"));
            self.line(&format!("{source} = NULL;"));
        }
        self.line(&format!(
            "spx_status = {}(spx_ctx{}{}, &{temporary});",
            target.symbol,
            if arguments.is_empty() { "" } else { ", " },
            arguments.budgeted_join(", ")
        ));
        if let Some(plan) = self.bytes_plan {
            for (index, expected) in target.params.iter().enumerate() {
                if matches!(expected, ResolvedType::Bytes)
                    && target.param_ownerships[index] == hir::OwnershipMode::Own
                {
                    let (_, flag) = plan.call_argument(
                        &expr.id,
                        u32::try_from(index)
                            .map_err(|_| backend_error("native call has too many parameters"))?,
                    )?;
                    self.line(&format!("{flag} = false;"));
                }
            }
        }
        self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
        if matches!(target.return_type, ResolvedType::String) {
            self.string_initialize(&temporary);
        }
        if is_aggregate_type(self.program, &target.return_type)? {
            if let Some(plan) = self.bytes_plan {
                let storage = crate::cleanup_plan::StorageId::Temporary(expr.id.clone());
                if plan.has_projected_leaves(&storage) {
                    let initialize = if plan.has_variant_leaves(&storage) {
                        plan.initialize_variant_result_at(
                            &expr.id,
                            &temporary,
                            &self.variant_layout(&target.return_type)?,
                        )?
                    } else {
                        plan.initialize_record_result_at(&expr.id, &temporary)?
                    };
                    for line in initialize.lines() {
                        self.line(line);
                    }
                }
            }
        }
        let result = CValue {
            code: if matches!(target.return_type, ResolvedType::Bytes) {
                self.bytes_plan
                    .and_then(|plan| plan.result_at(&expr.id))
                    .ok_or_else(|| backend_error("owned call has no canonical result transfer"))?
                    .to_owned()
            } else {
                temporary
            },
            ty: target.return_type.clone(),
        };
        self.apply_owned_plan_at_value(&expr.id, &result)?;
        Ok(result)
    }

    fn emit_block_expr(&mut self, expr: &ResolvedExpr) -> Result<CValue, Diagnostic> {
        let value = match &expr.kind {
            ResolvedExprKind::Block { statements, tail } => {
                let saved = self.variables.clone();
                for statement in statements {
                    match statement {
                        ResolvedStatement::Let { binding, value, .. } => {
                            let value = self.emit_expr(value)?;
                            self.require_type(&value.ty, &binding.ty, "local binding")?;
                            let local = if matches!(binding.ty, ResolvedType::Bytes) {
                                let plan = self.bytes_plan.ok_or_else(|| {
                                    backend_error(
                                        "owned Bytes binding has no canonical cleanup plan",
                                    )
                                })?;
                                let storage =
                                    crate::cleanup_plan::StorageId::Value(binding.id.clone());
                                let expected = plan.value(&storage)?.to_owned();
                                if value.code != expected {
                                    let transitions = plan.transfer_to(&storage)?;
                                    for line in transitions.lines() {
                                        self.line(line);
                                    }
                                }
                                expected
                            } else if matches!(binding.ty, ResolvedType::String)
                                && self.owned_strings.is_some()
                            {
                                let local = self.temporary(&ResolvedType::String)?;
                                self.string_move(&local, &value.code);
                                local
                            } else if matches!(binding.ty, ResolvedType::ArrayU8(0)) {
                                // The expression has already been evaluated;
                                // its zero-sized Copy value has no C storage.
                                "UINT8_C(0)".to_owned()
                            } else if self.record_contains_owned_bytes(&binding.ty)? {
                                let local = format!("spx_local_{}", self.next_local);
                                self.next_local += 1;
                                self.line(&format!(
                                    "{} {local} = {{0}};",
                                    c_value_type(self.program, self.resource_abi, &binding.ty)?,
                                ));
                                self.move_owned_record_fields(&local, &value.code, &binding.ty)?;
                                self.line(&format!("(void){local};"));
                                local
                            } else {
                                let local = format!("spx_local_{}", self.next_local);
                                self.next_local += 1;
                                self.line(&format!(
                                    "{} {local} = {};",
                                    c_value_type(self.program, self.resource_abi, &binding.ty)?,
                                    value.code
                                ));
                                self.line(&format!("(void){local};"));
                                local
                            };
                            if self
                                .variables
                                .insert(
                                    binding.id.clone(),
                                    CBinding {
                                        name: local,
                                        ty: binding.ty.clone(),
                                    },
                                )
                                .is_some()
                            {
                                return Err(backend_error(format!(
                                    "duplicate resolved local identity `{}`",
                                    binding.id
                                )));
                            }
                        }
                        ResolvedStatement::Assign {
                            binding,
                            field,
                            value: assigned,
                            ..
                        } => {
                            // The assigned value is emitted fully first; the
                            // store is a plain C11 assignment into the local
                            // or, for Field Mutation v1, into its one direct
                            // scalar field.
                            let value = self.emit_expr(assigned)?;
                            match field {
                                Some(field_id) => {
                                    let layout = self.record_layout(&binding.ty)?;
                                    let field =
                                        layout.field(field_id).cloned().ok_or_else(|| {
                                            backend_error(format!(
                                                "native record `{}` has no assignment field `{field_id}`",
                                                layout.record
                                            ))
                                        })?;
                                    self.require_type(&value.ty, &field.ty, "field assignment")?;
                                    if matches!(field.ty, ResolvedType::String) {
                                        return Err(backend_error(
                                            "string field assignment has no admitted native lowering",
                                        ));
                                    }
                                    if matches!(field.ty, ResolvedType::Bytes) {
                                        return Err(backend_error(
                                            "owned Bytes field assignment has no admitted native lowering",
                                        ));
                                    }
                                    let target =
                                        self.variables.get(&binding.id).ok_or_else(|| {
                                            backend_error(format!(
                                                "assignment target `{}` has no native local",
                                                binding.id
                                            ))
                                        })?;
                                    if field.size != 0 {
                                        self.line(&format!(
                                            "{}.{} = {};",
                                            target.name,
                                            c_field_symbol(&field.field),
                                            value.code
                                        ));
                                    }
                                }
                                None => {
                                    self.require_type(&value.ty, &binding.ty, "assignment")?;
                                    if matches!(binding.ty, ResolvedType::String) {
                                        return Err(backend_error(
                                            "string assignment has no admitted native lowering",
                                        ));
                                    }
                                    if matches!(binding.ty, ResolvedType::Bytes) {
                                        return Err(backend_error(
                                            "owned Bytes assignment is outside the immutable data profile",
                                        ));
                                    }
                                    let target =
                                        self.variables.get(&binding.id).ok_or_else(|| {
                                            backend_error(format!(
                                                "assignment target `{}` has no native local",
                                                binding.id
                                            ))
                                        })?;
                                    if !matches!(binding.ty, ResolvedType::ArrayU8(0)) {
                                        self.line(&format!("{} = {};", target.name, value.code));
                                    }
                                }
                            }
                        }
                        ResolvedStatement::Unsafe { body, .. } => {
                            // Backends treat the boundary transparently: emit
                            // exactly the ordinary block body and discard its
                            // scalar Copy result.
                            let value = self.emit_expr(body)?;
                            if matches!(value.ty, ResolvedType::String) {
                                return Err(backend_error(
                                    "discarding an owned string has no admitted native lowering",
                                ));
                            }
                            self.line(&format!("(void)({});", value.code));
                        }
                        ResolvedStatement::While {
                            condition, body, ..
                        } => {
                            // Bounded While-Loops v1 lowers to a native C11
                            // loop. Because checked sub-expressions lower to
                            // statements, the condition re-evaluates at the
                            // top of every iteration and breaks out on false;
                            // checked-arithmetic failures inside the loop jump
                            // to the shared epilogue exactly like
                            // straight-line failures.
                            self.line("for (;;) {");
                            self.indent += 1;
                            let condition = self.emit_expr(condition)?;
                            self.require_type(
                                &condition.ty,
                                &ResolvedType::Bool,
                                "while condition",
                            )?;
                            self.line(&format!("if (!({})) break;", condition.code));
                            let body_value = self.emit_expr(body)?;
                            if matches!(body_value.ty, ResolvedType::String) {
                                return Err(backend_error(
                                    "discarding an owned string has no admitted native lowering",
                                ));
                            }
                            self.line(&format!("(void)({});", body_value.code));
                            self.indent -= 1;
                            self.line("}");
                        }
                    }
                }
                let mut tail = self.emit_expr(tail)?;
                self.require_type(&tail.ty, &expr.ty, "block result")?;
                // Owned string locals introduced in this block free exactly
                // their own buffer when the block exits; outer bindings and
                // the tail value are untouched. The order is sorted so the
                // projection stays byte-deterministic.
                let mut introduced_strings: Vec<String> = self
                    .variables
                    .iter()
                    .filter(|(id, binding)| {
                        matches!(binding.ty, ResolvedType::String) && !saved.contains_key(*id)
                    })
                    .map(|(_, binding)| binding.name.clone())
                    .collect();
                introduced_strings.sort();
                for name in introduced_strings {
                    self.string_drop(&name);
                }
                if matches!(tail.ty, ResolvedType::Bytes) {
                    let plan = self.bytes_plan.ok_or_else(|| {
                        backend_error("owned Bytes block has no canonical cleanup plan")
                    })?;
                    let transitions = plan.apply_at(&expr.id)?;
                    for line in transitions.lines() {
                        self.line(line);
                    }
                    tail.code = plan
                        .result_at(&expr.id)
                        .ok_or_else(|| {
                            backend_error("owned Bytes block has no canonical result transfer")
                        })?
                        .to_owned();
                } else if is_aggregate_type(self.program, &tail.ty)? {
                    self.apply_owned_plan_at_value(&expr.id, &tail)?;
                }
                if let Some(plan) = self.bytes_plan {
                    let anchors = statements
                        .iter()
                        .flat_map(|statement| {
                            let mut anchors = Vec::with_capacity(2);
                            if let ResolvedStatement::Let { binding, .. } = statement {
                                let storage =
                                    crate::cleanup_plan::StorageId::Value(binding.id.clone());
                                if binding.ty == ResolvedType::Bytes
                                    || plan.has_projected_leaves(&storage)
                                {
                                    anchors.push(storage);
                                }
                            }
                            let value = match statement {
                                ResolvedStatement::Let { value, .. }
                                | ResolvedStatement::Assign { value, .. } => Some(value),
                                ResolvedStatement::Unsafe { body, .. } => Some(body.as_ref()),
                                ResolvedStatement::While { .. } => None,
                            };
                            if let Some(value) = value {
                                let storage =
                                    crate::cleanup_plan::StorageId::Temporary(value.id.clone());
                                if value.ty == ResolvedType::Bytes
                                    || plan.has_projected_leaves(&storage)
                                {
                                    anchors.push(storage);
                                }
                            }
                            anchors
                        })
                        .collect::<BTreeSet<_>>();
                    let cleanup = plan.scope_exit(&anchors)?;
                    for line in cleanup.lines() {
                        self.line(line);
                    }
                }
                self.variables = saved;
                tail
            }
            _ => unreachable!("non-Block expression reached emit_block_expr"),
        };
        self.require_type(&value.ty, &expr.ty, "expression")?;
        Ok(value)
    }

    fn emit_empty_block_expr(&mut self, expr: &ResolvedExpr) -> Result<CValue, Diagnostic> {
        let mut blocks = Vec::new();
        let mut current = expr;
        loop {
            let ResolvedExprKind::Block { statements, tail } = &current.kind else {
                unreachable!("non-Block expression reached emit_empty_block_expr")
            };
            debug_assert!(statements.is_empty());
            blocks.push(current);
            match &tail.kind {
                ResolvedExprKind::Block { statements, .. } if statements.is_empty() => {
                    current = tail;
                }
                _ => {
                    current = tail;
                    break;
                }
            }
        }

        let mut value = self.emit_expr(current)?;
        while let Some(block) = blocks.pop() {
            self.require_type(&value.ty, &block.ty, "block result")?;
            if matches!(value.ty, ResolvedType::Bytes) {
                let plan = self.bytes_plan.ok_or_else(|| {
                    backend_error("owned Bytes block has no canonical cleanup plan")
                })?;
                let transitions = plan.apply_at(&block.id)?;
                for line in transitions.lines() {
                    self.line(line);
                }
                value.code = plan
                    .result_at(&block.id)
                    .ok_or_else(|| {
                        backend_error("owned Bytes block has no canonical result transfer")
                    })?
                    .to_owned();
            } else if is_aggregate_type(self.program, &value.ty)? {
                self.apply_owned_plan_at_value(&block.id, &value)?;
            }
            if let Some(plan) = self.bytes_plan {
                let cleanup = plan.scope_exit(&BTreeSet::new())?;
                for line in cleanup.lines() {
                    self.line(line);
                }
            }
        }
        Ok(value)
    }

    fn emit_if_expr(&mut self, expr: &ResolvedExpr) -> Result<CValue, Diagnostic> {
        struct Continuation<'a> {
            expr: &'a ResolvedExpr,
            then_branch: &'a ResolvedExpr,
            else_branch: &'a ResolvedExpr,
            temporary: String,
        }

        let mut continuations = Vec::new();
        let mut current = expr;
        let mut value = loop {
            let ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } = &current.kind
            else {
                unreachable!("non-If expression reached emit_if_expr")
            };
            let condition = self.emit_expr(condition)?;
            self.require_type(&condition.ty, &ResolvedType::Bool, "if condition")?;
            let temporary = if matches!(current.ty, ResolvedType::Bytes) {
                self.bytes_plan
                    .ok_or_else(|| backend_error("owned Bytes if has no cleanup plan"))?
                    .value(&crate::cleanup_plan::StorageId::Temporary(
                        current.id.clone(),
                    ))?
                    .to_owned()
            } else {
                self.temporary(&current.ty)?
            };
            self.line(&format!("if ({}) {{", condition.code));
            self.indent += 1;
            continuations.push(Continuation {
                expr: current,
                then_branch,
                else_branch,
                temporary,
            });
            if matches!(then_branch.kind, ResolvedExprKind::If { .. }) {
                current = then_branch;
            } else {
                break self.emit_expr(then_branch)?;
            }
        };

        while let Some(continuation) = continuations.pop() {
            self.require_type(&value.ty, &continuation.expr.ty, "then branch")?;
            self.assign_branch_result(
                continuation.expr,
                continuation.then_branch,
                &continuation.temporary,
                &value,
            )?;
            self.indent -= 1;
            self.line("} else {");
            self.indent += 1;
            let else_value = self.emit_expr(continuation.else_branch)?;
            self.require_type(&else_value.ty, &continuation.expr.ty, "else branch")?;
            self.assign_branch_result(
                continuation.expr,
                continuation.else_branch,
                &continuation.temporary,
                &else_value,
            )?;
            self.indent -= 1;
            self.line("}");
            value = CValue {
                code: continuation.temporary,
                ty: continuation.expr.ty.clone(),
            };
        }
        Ok(value)
    }

    fn assign_branch_result(
        &mut self,
        expr: &ResolvedExpr,
        branch: &ResolvedExpr,
        temporary: &str,
        value: &CValue,
    ) -> Result<(), Diagnostic> {
        if matches!(expr.ty, ResolvedType::Bytes) {
            let plan = self
                .bytes_plan
                .ok_or_else(|| backend_error("owned Bytes if has no cleanup plan"))?;
            let transitions = plan.transfer_branch_at(
                &expr.id,
                &value.code,
                &crate::cleanup_plan::CleanupPlace {
                    storage: crate::cleanup_plan::StorageId::Temporary(expr.id.clone()),
                    projections: Vec::new(),
                },
            )?;
            for line in transitions.lines() {
                self.line(line);
            }
        } else if variant_declaration_id(self.program, &expr.ty)?.is_some() {
            if let Some(plan) = self.bytes_plan {
                let layout = self.variant_layout(&expr.ty)?;
                let transitions = plan.transfer_variant_branch_to(
                    &branch.id,
                    &crate::cleanup_plan::StorageId::Temporary(expr.id.clone()),
                    &value.code,
                    &layout,
                )?;
                for line in transitions.lines() {
                    self.line(line);
                }
            }
            self.line(&format!("{temporary} = {};", value.code));
        } else if self.record_contains_owned_bytes(&expr.ty)? {
            self.zero_owned_record_bytes(temporary, &expr.ty)?;
            self.move_owned_record_fields(temporary, &value.code, &expr.ty)?;
        } else if matches!(expr.ty, ResolvedType::String) && self.owned_strings.is_some() {
            self.string_move(temporary, &value.code);
        } else if !matches!(expr.ty, ResolvedType::ArrayU8(0)) {
            self.line(&format!("{temporary} = {};", value.code));
        }
        Ok(())
    }

    fn emit_construct_record_expr(&mut self, expr: &ResolvedExpr) -> Result<CValue, Diagnostic> {
        let value = match &expr.kind {
            ResolvedExprKind::ConstructRecord { record, fields } => {
                let layout = self.record_layout(&expr.ty)?;
                if layout.record != *record {
                    return Err(backend_error(format!(
                        "native record constructor `{record}` has result type `{}`",
                        expr.ty.identity_key()
                    )));
                }
                let temporary = self.temporary(&expr.ty)?;
                self.initialize_record_carrier(&temporary, &layout);
                if self.record_contains_owned_bytes(&expr.ty)? {
                    self.zero_owned_record_bytes(&temporary, &expr.ty)?;
                }
                for initializer in fields {
                    let field = layout.field(&initializer.field).cloned().ok_or_else(|| {
                        backend_error(format!(
                            "native record `{record}` has no field `{}`",
                            initializer.field
                        ))
                    })?;
                    let value = self.emit_expr(&initializer.value)?;
                    self.require_type(&value.ty, &field.ty, "record field initializer")?;
                    if matches!(field.ty, ResolvedType::Bytes) {
                        let plan = self.bytes_plan.ok_or_else(|| {
                            backend_error("owned Bytes record field has no cleanup plan")
                        })?;
                        let transitions = plan.transfer_field_at(
                            &initializer.value.id,
                            &value.code,
                            &crate::cleanup_plan::CleanupPlace {
                                storage: crate::cleanup_plan::StorageId::Temporary(expr.id.clone()),
                                projections: vec![field.field.clone()],
                            },
                        )?;
                        for line in transitions.lines() {
                            self.line(line);
                        }
                    }
                    if field.size != 0 && self.record_contains_owned_bytes(&field.ty)? {
                        let destination = format!("{temporary}.{}", c_field_symbol(&field.field));
                        self.move_owned_record_fields(&destination, &value.code, &field.ty)?;
                    } else if field.size != 0 && !matches!(field.ty, ResolvedType::Bytes) {
                        self.line(&format!(
                            "{temporary}.{} = {};",
                            c_field_symbol(&field.field),
                            value.code
                        ));
                    }
                }
                if let Some(plan) = self.bytes_plan {
                    let transitions = plan.apply_at(&expr.id)?;
                    for line in transitions.lines() {
                        self.line(line);
                    }
                }
                CValue {
                    code: temporary,
                    ty: expr.ty.clone(),
                }
            }
            _ => unreachable!("non-ConstructRecord expression reached emit_construct_record_expr"),
        };
        self.require_type(&value.ty, &expr.ty, "expression")?;
        Ok(value)
    }

    fn emit_construct_variant_expr(&mut self, expr: &ResolvedExpr) -> Result<CValue, Diagnostic> {
        let value = match &expr.kind {
            ResolvedExprKind::ConstructVariant {
                variant,
                case,
                fields,
            } => {
                let layout = self.variant_layout(&expr.ty)?;
                if layout.variant != *variant {
                    return Err(backend_error(format!(
                        "native variant constructor `{variant}` has result type `{}`",
                        expr.ty.identity_key()
                    )));
                }
                let case_layout = layout.case(case).cloned().ok_or_else(|| {
                    backend_error(format!("native variant `{variant}` has no case `{case}`"))
                })?;
                let mut values = Vec::with_capacity(fields.len());
                for initializer in fields {
                    let field =
                        case_layout
                            .field(&initializer.field)
                            .cloned()
                            .ok_or_else(|| {
                                backend_error(format!(
                                    "native variant case `{case}` has no field `{}`",
                                    initializer.field
                                ))
                            })?;
                    let value = self.emit_expr(&initializer.value)?;
                    self.require_type(&value.ty, &field.ty, "variant field initializer")?;
                    if matches!(field.ty, ResolvedType::Bytes) {
                        let plan = self.bytes_plan.ok_or_else(|| {
                            backend_error("owned Bytes variant field has no cleanup plan")
                        })?;
                        let transitions = plan.transfer_field_at(
                            &initializer.value.id,
                            &value.code,
                            &crate::cleanup_plan::CleanupPlace {
                                storage: crate::cleanup_plan::StorageId::Temporary(expr.id.clone()),
                                projections: vec![case.clone(), field.field.clone()],
                            },
                        )?;
                        for line in transitions.lines() {
                            self.line(line);
                        }
                    }
                    values.push((field, value));
                }
                let temporary = self.temporary(&expr.ty)?;
                self.line(&format!("memset(&{temporary}, 0, sizeof({temporary}));"));
                let case_symbol = c_case_symbol(case);
                for (field, value) in values {
                    if field.size != 0 && !matches!(field.ty, ResolvedType::Bytes) {
                        self.line(&format!(
                            "{temporary}.spx_payload.{case_symbol}.{} = {};",
                            c_field_symbol(&field.field),
                            value.code
                        ));
                    }
                }
                self.line(&format!(
                    "{temporary}.spx_tag = UINT32_C({});",
                    case_layout.tag
                ));
                if let Some(plan) = self.bytes_plan {
                    let transitions = plan.apply_variant_case_at(&expr.id, case)?;
                    for line in transitions.lines() {
                        self.line(line);
                    }
                }
                CValue {
                    code: temporary,
                    ty: expr.ty.clone(),
                }
            }
            _ => {
                unreachable!("non-ConstructVariant expression reached emit_construct_variant_expr")
            }
        };
        self.require_type(&value.ty, &expr.ty, "expression")?;
        Ok(value)
    }

    fn emit_match_expr(&mut self, expr: &ResolvedExpr) -> Result<CValue, Diagnostic> {
        let value = match &expr.kind {
            ResolvedExprKind::Match {
                mode,
                scrutinee,
                arms,
            } => {
                if is_aggregate_type(self.program, &expr.ty)? {
                    return Err(backend_error("copy match arms must produce i64 or bool"));
                }
                let source_storage = match &scrutinee.kind {
                    ResolvedExprKind::Place(place) if place.projections.is_empty() => {
                        Some(crate::cleanup_plan::StorageId::Value(place.root.clone()))
                    }
                    _ => None,
                };
                let scrutinee = self.emit_expr(scrutinee)?;
                // Refutable Match v1: Copy-scalar scrutinees lower to the
                // literal/guard decision chain; aggregates keep the exact
                // pre-feature lowering below.
                if matches!(
                    scrutinee.ty,
                    ResolvedType::I64
                        | ResolvedType::I32
                        | ResolvedType::U8
                        | ResolvedType::Char
                        | ResolvedType::Bool
                ) {
                    return self.emit_scalar_match(expr, &scrutinee, arms);
                }
                if let Some(record) = record_declaration_id(self.program, &scrutinee.ty)?.cloned() {
                    let [arm] = arms.as_slice() else {
                        return Err(backend_error(
                            "irrefutable record match must have exactly one arm",
                        ));
                    };
                    if *mode == hir::ResolvedMatchMode::Own {
                        let preflight = self
                            .bytes_plan
                            .ok_or_else(|| backend_error("owned record match has no cleanup plan"))?
                            .authenticate_transfers_at(&expr.id)?;
                        for line in preflight.lines() {
                            self.line(line);
                        }
                    }
                    let contains_owned_bytes = self.record_contains_owned_bytes(&scrutinee.ty)?;
                    let staged = match mode {
                        hir::ResolvedMatchMode::Own => {
                            let staged = self.temporary(&scrutinee.ty)?;
                            if contains_owned_bytes {
                                self.zero_owned_record_bytes(&staged, &scrutinee.ty)?;
                                self.move_owned_record_fields(
                                    &staged,
                                    &scrutinee.code,
                                    &scrutinee.ty,
                                )?;
                            } else {
                                self.line(&format!("{staged} = {};", scrutinee.code));
                            }
                            self.line(&format!("(void){staged};"));
                            staged
                        }
                        hir::ResolvedMatchMode::Borrow => {
                            // A borrow match retains the caller's authoritative
                            // carrier. Bytes leaves bind through separately
                            // authenticated full-path aliases below; Copy leaves
                            // read directly from this carrier.
                            scrutinee.code.clone()
                        }
                        hir::ResolvedMatchMode::Value => {
                            if contains_owned_bytes {
                                return Err(backend_error(
                                    "Value record match cannot copy owned Bytes",
                                ));
                            }
                            let staged = self.temporary(&scrutinee.ty)?;
                            self.line(&format!("{staged} = {};", scrutinee.code));
                            staged
                        }
                    };
                    if *mode == hir::ResolvedMatchMode::Own {
                        let transitions = self
                            .bytes_plan
                            .expect("owned match plan checked above")
                            .apply_at(&expr.id)?;
                        for line in transitions.lines() {
                            self.line(line);
                        }
                    }
                    let saved = self.variables.clone();
                    match &arm.pattern {
                        hir::ResolvedMatchPattern::Wildcard => {}
                        hir::ResolvedMatchPattern::Record {
                            record: pattern_record,
                            instance,
                            fields,
                        } => self.bind_record_match_pattern(
                            &staged,
                            &scrutinee.ty,
                            pattern_record,
                            instance,
                            fields,
                            &RecordMatchBindingMode {
                                mode: *mode,
                                source_storage: source_storage.as_ref(),
                                source_path: Vec::new(),
                            },
                        )?,
                        hir::ResolvedMatchPattern::Variant { .. } => {
                            return Err(backend_error(
                                "variant pattern has a record match scrutinee",
                            ));
                        }
                        hir::ResolvedMatchPattern::Literal(_)
                        | hir::ResolvedMatchPattern::Or(_)
                        | hir::ResolvedMatchPattern::Binding(_) => {
                            return Err(backend_error(
                                "refutable pattern has an aggregate record match scrutinee",
                            ));
                        }
                    }
                    if record_declaration_id(self.program, &scrutinee.ty)? != Some(&record) {
                        return Err(backend_error(
                            "record match scrutinee identity changed during lowering",
                        ));
                    }
                    let value = self.emit_expr(&arm.value)?;
                    self.require_type(&value.ty, &expr.ty, "record match arm result")?;
                    if *mode == hir::ResolvedMatchMode::Own {
                        let hir::ResolvedMatchPattern::Record { fields, .. } = &arm.pattern else {
                            return Err(backend_error(
                                "owned record match requires one exact record pattern",
                            ));
                        };
                        let anchors = nested_owned::owned_record_pattern_anchors(fields)?;
                        let cleanup = self
                            .bytes_plan
                            .expect("owned match plan checked above")
                            .scope_exit(&anchors)?;
                        for line in cleanup.lines() {
                            self.line(line);
                        }
                    }
                    self.variables = saved;
                    return Ok(CValue {
                        code: value.code,
                        ty: expr.ty.clone(),
                    });
                }
                let layout = self.variant_layout(&scrutinee.ty)?;
                let staged = if *mode == hir::ResolvedMatchMode::Value {
                    // Preserve the frozen Copy-variant projection exactly.
                    let staged = self.temporary(&scrutinee.ty)?;
                    self.line(&format!("{staged} = {};", scrutinee.code));
                    staged
                } else {
                    // Owned/borrowed variants retain one authoritative carrier;
                    // a shallow union assignment would forge a second Bytes owner.
                    scrutinee.code.clone()
                };
                self.line(&format!(
                    "if ({staged}.spx_tag >= UINT32_C({})) spx_runtime_invariant_failure(\"invalid variant tag\");",
                    layout.cases.len()
                ));
                let result = if matches!(expr.ty, ResolvedType::Bytes) {
                    self.bytes_plan
                        .ok_or_else(|| backend_error("owned Bytes match has no cleanup plan"))?
                        .value(&crate::cleanup_plan::StorageId::Temporary(expr.id.clone()))?
                        .to_owned()
                } else {
                    self.temporary(&expr.ty)?
                };
                let matched = self.temporary(&ResolvedType::Bool)?;
                self.line(&format!("{matched} = false;"));
                for arm in arms {
                    let saved = self.variables.clone();
                    match &arm.pattern {
                        hir::ResolvedMatchPattern::Variant {
                            variant,
                            case,
                            fields,
                        } => {
                            if *variant != layout.variant {
                                return Err(backend_error(format!(
                                    "match arm variant `{variant}` disagrees with `{}`",
                                    layout.variant
                                )));
                            }
                            let case_layout = layout.case(case).cloned().ok_or_else(|| {
                                backend_error(format!("match arm references unknown case `{case}`"))
                            })?;
                            self.line(&format!(
                                "if (!{matched} && {staged}.spx_tag == UINT32_C({})) {{",
                                case_layout.tag
                            ));
                            self.indent += 1;
                            self.line(&format!("{matched} = true;"));
                            if *mode == hir::ResolvedMatchMode::Own {
                                let transitions = self
                                    .bytes_plan
                                    .ok_or_else(|| {
                                        backend_error("owned variant match has no cleanup plan")
                                    })?
                                    .apply_variant_case_at(&expr.id, case)?;
                                for line in transitions.lines() {
                                    self.line(line);
                                }
                            }
                            let case_symbol = c_case_symbol(case);
                            for pattern_field in fields {
                                let field = case_layout
                                    .field(&pattern_field.field)
                                    .cloned()
                                    .ok_or_else(|| {
                                        backend_error(format!(
                                            "match case `{case}` has no field `{}`",
                                            pattern_field.field
                                        ))
                                    })?;
                                self.require_type(
                                    &pattern_field.binding.ty,
                                    &field.ty,
                                    "match payload binding",
                                )?;
                                let name = if matches!(field.ty, ResolvedType::Bytes) {
                                    match (*mode, pattern_field.binding.ownership) {
                                        (hir::ResolvedMatchMode::Own, hir::OwnershipMode::Own) => {
                                            self.bytes_plan
                                                .ok_or_else(|| {
                                                    backend_error(
                                                        "owned variant binding has no cleanup plan",
                                                    )
                                                })?
                                                .value(&crate::cleanup_plan::StorageId::Value(
                                                    pattern_field.binding.id.clone(),
                                                ))?
                                                .to_owned()
                                        }
                                        (
                                            hir::ResolvedMatchMode::Borrow,
                                            hir::OwnershipMode::Borrow,
                                        ) => source_storage
                                            .as_ref()
                                            .and_then(|storage| {
                                                self.bytes_plan.and_then(|plan| {
                                                    plan.variant_value_if_present(
                                                        storage,
                                                        case,
                                                        &field.field,
                                                    )
                                                })
                                            })
                                            .map(str::to_owned)
                                            .or_else(|| {
                                                let crate::cleanup_plan::StorageId::Value(root) =
                                                    source_storage.as_ref()?
                                                else {
                                                    return None;
                                                };
                                                self.borrowed_aggregate_bytes
                                                    .get(&(
                                                        root.clone(),
                                                        vec![case.clone(), field.field.clone()],
                                                    ))
                                                    .cloned()
                                            })
                                            .ok_or_else(|| {
                                                backend_error(
                                                    "borrowed variant Bytes field has no authenticated alias",
                                                )
                                            })?,
                                        _ => {
                                            return Err(backend_error(
                                                "variant Bytes binding ownership disagrees with match mode",
                                            ));
                                        }
                                    }
                                } else if pattern_field.binding.ownership
                                    == hir::OwnershipMode::Value
                                {
                                    format!(
                                        "({staged}).spx_payload.{case_symbol}.{}",
                                        c_field_symbol(&field.field)
                                    )
                                } else {
                                    return Err(backend_error(
                                        "variant Copy binding has non-Value ownership",
                                    ));
                                };
                                self.variables.insert(
                                    pattern_field.binding.id.clone(),
                                    CBinding { name, ty: field.ty },
                                );
                            }
                        }
                        hir::ResolvedMatchPattern::Wildcard => {
                            if *mode == hir::ResolvedMatchMode::Own {
                                return Err(backend_error(
                                    "owned variant wildcard cannot hide a live payload",
                                ));
                            }
                            self.line(&format!("if (!{matched}) {{"));
                            self.indent += 1;
                            self.line(&format!("{matched} = true;"));
                        }
                        hir::ResolvedMatchPattern::Record { .. } => {
                            return Err(backend_error(
                                "record pattern has a variant match scrutinee",
                            ));
                        }
                        hir::ResolvedMatchPattern::Literal(_)
                        | hir::ResolvedMatchPattern::Or(_)
                        | hir::ResolvedMatchPattern::Binding(_) => {
                            return Err(backend_error(
                                "refutable pattern has an aggregate variant match scrutinee",
                            ));
                        }
                    }
                    let value = self.emit_expr(&arm.value)?;
                    self.require_type(&value.ty, &expr.ty, "match arm result")?;
                    if matches!(expr.ty, ResolvedType::Bytes) {
                        let transitions = self
                            .bytes_plan
                            .expect("checked above")
                            .apply_at(&arm.value.id)?;
                        for line in transitions.lines() {
                            self.line(line);
                        }
                    } else if matches!(value.ty, ResolvedType::String)
                        && self.owned_strings.is_some()
                    {
                        self.string_move(&result, &value.code);
                    } else {
                        self.line(&format!("{result} = {};", value.code));
                    }
                    if *mode == hir::ResolvedMatchMode::Own {
                        let anchors = match &arm.pattern {
                            hir::ResolvedMatchPattern::Variant { fields, .. } => fields
                                .iter()
                                .filter(|field| matches!(field.binding.ty, ResolvedType::Bytes))
                                .map(|field| {
                                    crate::cleanup_plan::StorageId::Value(field.binding.id.clone())
                                })
                                .collect::<BTreeSet<_>>(),
                            _ => BTreeSet::new(),
                        };
                        let cleanup = self
                            .bytes_plan
                            .ok_or_else(|| {
                                backend_error("owned variant match has no cleanup plan")
                            })?
                            .scope_exit(&anchors)?;
                        for line in cleanup.lines() {
                            self.line(line);
                        }
                    }
                    self.variables = saved;
                    self.indent -= 1;
                    self.line("}");
                }
                self.line(&format!(
                    "if (!{matched}) spx_runtime_invariant_failure(\"exhaustive variant match selected no arm\");"
                ));
                CValue {
                    code: result,
                    ty: expr.ty.clone(),
                }
            }
            _ => unreachable!("non-Match expression reached emit_match_expr"),
        };
        self.require_type(&value.ty, &expr.ty, "expression")?;
        Ok(value)
    }

    fn emit_try_expr(&mut self, expr: &ResolvedExpr) -> Result<CValue, Diagnostic> {
        let value = match &expr.kind {
            ResolvedExprKind::Try {
                operand,
                result,
                ok_case,
                ok_field,
                err_case,
                err_field,
                residual_type,
            } => {
                if !self.try_target_enabled {
                    return Err(backend_error(
                        "copy-result propagation is allowed only in a function body",
                    ));
                }
                self.require_type(
                    residual_type,
                    self.return_type,
                    "copy-result residual target",
                )?;
                let operand_layout = self.variant_layout(&operand.ty)?;
                let residual_layout = self.variant_layout(residual_type)?;
                if operand_layout.variant != *result || residual_layout.variant != *result {
                    return Err(backend_error(
                        "copy-result propagation does not reference its resolved Result declaration",
                    ));
                }
                let operand_ok = operand_layout
                    .case(ok_case)
                    .and_then(|case| case.field(ok_field).map(|field| (case, field)))
                    .ok_or_else(|| {
                        backend_error("copy-result propagation has no resolved Ok payload")
                    })?;
                let operand_err = operand_layout
                    .case(err_case)
                    .and_then(|case| case.field(err_field).map(|field| (case, field)))
                    .ok_or_else(|| {
                        backend_error("copy-result propagation has no resolved Err payload")
                    })?;
                let residual_err = residual_layout
                    .case(err_case)
                    .and_then(|case| case.field(err_field).map(|field| (case, field)))
                    .ok_or_else(|| {
                        backend_error("copy-result residual has no resolved Err payload")
                    })?;
                self.require_type(&operand_ok.1.ty, &expr.ty, "copy-result Ok payload")?;
                self.require_type(
                    &operand_err.1.ty,
                    &residual_err.1.ty,
                    "copy-result Err payload",
                )?;

                let operand_value = self.emit_expr(operand)?;
                self.require_type(&operand_value.ty, &operand.ty, "copy-result operand")?;
                let operand_stage = self.temporary(&operand.ty)?;
                self.line(&format!("{operand_stage} = {};", operand_value.code));
                self.line(&format!(
                    "if ({operand_stage}.spx_tag >= UINT32_C({})) spx_runtime_invariant_failure(\"invalid variant tag\");",
                    operand_layout.cases.len()
                ));
                self.line(&format!(
                    "if ({operand_stage}.spx_tag == UINT32_C({})) {{",
                    operand_err.0.tag
                ));
                self.indent += 1;
                self.line("memset(&spx_result, 0, sizeof(spx_result));");
                self.line(&format!(
                    "spx_result.spx_payload.{}.{} = {operand_stage}.spx_payload.{}.{};",
                    c_case_symbol(err_case),
                    c_field_symbol(err_field),
                    c_case_symbol(err_case),
                    c_field_symbol(err_field),
                ));
                self.line(&format!(
                    "spx_result.spx_tag = UINT32_C({});",
                    residual_err.0.tag
                ));
                self.line("spx_result_staged = true;");
                self.line("goto spx_postconditions;");
                self.indent -= 1;
                self.line("}");
                self.line(&format!(
                    "if ({operand_stage}.spx_tag != UINT32_C({})) spx_runtime_invariant_failure(\"invalid Result tag\");",
                    operand_ok.0.tag
                ));
                let output = self.temporary(&expr.ty)?;
                self.line(&format!(
                    "{output} = {operand_stage}.spx_payload.{}.{};",
                    c_case_symbol(ok_case),
                    c_field_symbol(ok_field),
                ));
                CValue {
                    code: output,
                    ty: expr.ty.clone(),
                }
            }
            _ => unreachable!("non-Try expression reached emit_try_expr"),
        };
        self.require_type(&value.ty, &expr.ty, "expression")?;
        Ok(value)
    }

    fn emit_try_option_expr(&mut self, expr: &ResolvedExpr) -> Result<CValue, Diagnostic> {
        let value = match &expr.kind {
            ResolvedExprKind::TryOption {
                operand,
                option,
                some_case,
                some_field,
                none_case,
                residual_type,
            } => {
                if !self.try_target_enabled {
                    return Err(backend_error(
                        "copy-Option propagation is allowed only in a function body",
                    ));
                }
                self.require_type(
                    residual_type,
                    self.return_type,
                    "copy-Option residual target",
                )?;
                let operand_layout = self.variant_layout(&operand.ty)?;
                let residual_layout = self.variant_layout(residual_type)?;
                if operand_layout.variant != *option || residual_layout.variant != *option {
                    return Err(backend_error(
                        "copy-Option propagation does not reference its resolved Option declaration",
                    ));
                }
                let operand_some = operand_layout
                    .case(some_case)
                    .and_then(|case| case.field(some_field).map(|field| (case, field)))
                    .ok_or_else(|| {
                        backend_error("copy-Option propagation has no resolved Some payload")
                    })?;
                let operand_none = operand_layout.case(none_case).ok_or_else(|| {
                    backend_error("copy-Option propagation has no resolved None case")
                })?;
                let residual_none = residual_layout.case(none_case).ok_or_else(|| {
                    backend_error("copy-Option residual has no resolved None case")
                })?;
                if !operand_none.fields.is_empty() || !residual_none.fields.is_empty() {
                    return Err(backend_error(
                        "copy-Option None case unexpectedly has a payload",
                    ));
                }
                self.require_type(&operand_some.1.ty, &expr.ty, "copy-Option Some payload")?;

                let operand_value = self.emit_expr(operand)?;
                self.require_type(&operand_value.ty, &operand.ty, "copy-Option operand")?;
                let operand_stage = self.temporary(&operand.ty)?;
                self.line(&format!("{operand_stage} = {};", operand_value.code));
                self.line(&format!(
                    "if ({operand_stage}.spx_tag >= UINT32_C({})) spx_runtime_invariant_failure(\"invalid variant tag\");",
                    operand_layout.cases.len()
                ));
                self.line(&format!(
                    "if ({operand_stage}.spx_tag == UINT32_C({})) {{",
                    operand_none.tag
                ));
                self.indent += 1;
                self.line("memset(&spx_result, 0, sizeof(spx_result));");
                self.line(&format!(
                    "spx_result.spx_tag = UINT32_C({});",
                    residual_none.tag
                ));
                self.line("spx_result_staged = true;");
                self.line("goto spx_postconditions;");
                self.indent -= 1;
                self.line("}");
                self.line(&format!(
                    "if ({operand_stage}.spx_tag != UINT32_C({})) spx_runtime_invariant_failure(\"invalid Option tag\");",
                    operand_some.0.tag
                ));
                let output = self.temporary(&expr.ty)?;
                self.line(&format!(
                    "{output} = {operand_stage}.spx_payload.{}.{};",
                    c_case_symbol(some_case),
                    c_field_symbol(some_field),
                ));
                CValue {
                    code: output,
                    ty: expr.ty.clone(),
                }
            }
            _ => unreachable!("non-TryOption expression reached emit_try_option_expr"),
        };
        self.require_type(&value.ty, &expr.ty, "expression")?;
        Ok(value)
    }

    fn emit_project_expr(&mut self, expr: &ResolvedExpr) -> Result<CValue, Diagnostic> {
        let value = match &expr.kind {
            ResolvedExprKind::Project { base, field } => {
                let base = self.emit_expr(base)?;
                let layout = self.record_layout(&base.ty)?;
                let field = layout.field(field).cloned().ok_or_else(|| {
                    backend_error(format!(
                        "native record `{}` has no projected field `{field}`",
                        layout.record
                    ))
                })?;
                self.require_type(&expr.ty, &field.ty, "record projection")?;
                if matches!(field.ty, ResolvedType::Bytes) {
                    let plan = self.bytes_plan.ok_or_else(|| {
                        backend_error("owned Bytes projection has no canonical cleanup plan")
                    })?;
                    // The plan owns the projected leaf, not the aggregate's
                    // C field expression. Apply its field-to-result chain once
                    // before handing the canonical slot to the consumer.
                    let transitions = plan.apply_at(&expr.id)?;
                    for line in transitions.lines() {
                        self.line(line);
                    }
                    CValue {
                        code: plan
                            .result_at(&expr.id)
                            .ok_or_else(|| {
                                backend_error("owned Bytes projection has no canonical result")
                            })?
                            .to_owned(),
                        ty: field.ty,
                    }
                } else if field.size == 0 {
                    self.emit_erased_record_field_value(&field.ty)?
                } else {
                    CValue {
                        code: format!("({}).{}", base.code, c_field_symbol(&field.field)),
                        ty: field.ty,
                    }
                }
            }
            _ => unreachable!("non-Project expression reached emit_project_expr"),
        };
        self.require_type(&value.ty, &expr.ty, "expression")?;
        Ok(value)
    }

    fn emit_upcast_expr(&mut self, expr: &ResolvedExpr) -> Result<CValue, Diagnostic> {
        let value = match &expr.kind {
            ResolvedExprKind::Upcast { source } => {
                // Class Inheritance v1: the ancestor prefix moves
                // field-by-field from the consumed descendant value; the
                // canonical layouts guarantee identical offsets.
                let source = self.emit_expr(source)?;
                let target_layout = self.record_layout(&expr.ty)?;
                let source_layout = self.record_layout(&source.ty)?;
                if target_layout.record == source_layout.record {
                    return Err(backend_error(format!(
                        "native upcast `{}` requires a descendant source",
                        expr.ty.identity_key()
                    )));
                }
                for field in &target_layout.fields {
                    let source_field =
                        source_layout.field(&field.field).cloned().ok_or_else(|| {
                            backend_error(format!(
                                "native upcast source `{}` has no inherited field `{}`",
                                source_layout.record, field.field
                            ))
                        })?;
                    if (source_field.offset, source_field.size, source_field.align)
                        != (field.offset, field.size, field.align)
                    {
                        return Err(backend_error(format!(
                            "native upcast field `{}` disagrees with the ancestor prefix layout",
                            field.field
                        )));
                    }
                }
                let temporary = self.temporary(&expr.ty)?;
                self.initialize_record_carrier(&temporary, &target_layout);
                for field in &target_layout.fields {
                    if field.size != 0 {
                        self.line(&format!(
                            "{temporary}.{} = ({}).{};",
                            c_field_symbol(&field.field),
                            source.code,
                            c_field_symbol(&field.field)
                        ));
                    }
                }
                CValue {
                    code: temporary,
                    ty: expr.ty.clone(),
                }
            }
            _ => unreachable!("non-Upcast expression reached emit_upcast_expr"),
        };
        self.require_type(&value.ty, &expr.ty, "expression")?;
        Ok(value)
    }

    fn emit_place(&mut self, place: &hir::Place) -> Result<CValue, Diagnostic> {
        let binding = self.variables.get(&place.root).cloned().ok_or_else(|| {
            backend_error(format!("resolved value `{}` is not in scope", place.root))
        })?;
        let mut code = binding.name;
        let mut ty = binding.ty;
        let storage = crate::cleanup_plan::StorageId::Value(place.root.clone());
        let mut field_path = Vec::with_capacity(place.projections.len());
        for projection in &place.projections {
            let PlaceProjection::Field(field) = projection else {
                return Err(backend_error(
                    "native variant-field projection is outside executable records v1",
                ));
            };
            let layout = self.record_layout(&ty)?;
            let field = layout.field(field).cloned().ok_or_else(|| {
                backend_error(format!(
                    "native record `{}` has no place field `{field}`",
                    layout.record
                ))
            })?;
            field_path.push(field.field.clone());
            code = if matches!(field.ty, ResolvedType::Bytes) {
                self.bytes_plan
                    .ok_or_else(|| backend_error("projected Bytes place has no cleanup plan"))?
                    .projected_value(&storage, &field_path)?
                    .to_owned()
            } else if field.size == 0 {
                self.emit_erased_record_field_value(&field.ty)?.code
            } else {
                format!("({code}).{}", c_field_symbol(&field.field))
            };
            ty = field.ty;
        }
        Ok(CValue { code, ty })
    }

    fn record_layout(&self, ty: &ResolvedType) -> Result<AggregateLayout, Diagnostic> {
        record_declaration_id(self.program, ty)?.ok_or_else(|| {
            backend_error(format!(
                "native aggregate operation requires a record, found `{}`",
                ty.identity_key()
            ))
        })?;
        let layout = self.record_layouts.layout(ty)?.clone();
        layout.validate(self.program)?;
        Ok(layout)
    }

    fn initialize_record_carrier(&mut self, temporary: &str, layout: &AggregateLayout) {
        if layout.fields.is_empty() {
            // Empty products own one frozen semantic byte on every target.
            self.line(&format!(
                "{temporary}.spx_empty_record_padding = UINT8_C(0);"
            ));
        } else if layout.size == 0 {
            // Nonempty records whose semantic fields are all zero-sized need
            // one physical C byte without acquiring semantic storage.
            self.line(&format!(
                "{temporary}.spx_zero_sized_record_carrier = UINT8_C(0);"
            ));
        }
    }

    fn emit_erased_record_field_value(&mut self, ty: &ResolvedType) -> Result<CValue, Diagnostic> {
        if matches!(ty, ResolvedType::ArrayU8(0)) {
            return Ok(CValue {
                code: "UINT8_C(0)".to_owned(),
                ty: ty.clone(),
            });
        }
        let layout = self.record_layout(ty)?;
        if layout.size != 0 || layout.fields.is_empty() {
            return Err(backend_error(format!(
                "erased native record field `{}` has a nonzero physical layout",
                ty.identity_key()
            )));
        }
        let temporary = self.temporary(ty)?;
        self.initialize_record_carrier(&temporary, &layout);
        Ok(CValue {
            code: temporary,
            ty: ty.clone(),
        })
    }

    fn variant_layout(&self, ty: &ResolvedType) -> Result<VariantLayout, Diagnostic> {
        variant_declaration_id(self.program, ty)?.ok_or_else(|| {
            backend_error(format!(
                "native variant operation requires a variant, found `{}`",
                ty.identity_key()
            ))
        })?;
        let layout = self.variant_layouts.layout(ty)?.clone();
        layout.validate(self.program)?;
        Ok(layout)
    }

    fn bind_record_match_pattern(
        &mut self,
        base: &str,
        expected: &ResolvedType,
        record: &DeclarationId,
        instance: &ResolvedType,
        fields: &[hir::ResolvedRecordMatchPatternField],
        binding_mode: &RecordMatchBindingMode<'_>,
    ) -> Result<(), Diagnostic> {
        nested_owned::bind_record_match_pattern(
            self,
            base,
            expected,
            record,
            instance,
            fields,
            binding_mode,
        )
    }

    fn emit_binary(
        &mut self,
        op: BinaryOp,
        left: &ResolvedExpr,
        right: &ResolvedExpr,
        result_type: &ResolvedType,
    ) -> Result<CValue, Diagnostic> {
        if matches!(op, BinaryOp::And | BinaryOp::Or) {
            return self.emit_lazy_binary(op, left, right, result_type);
        }
        let left = self.emit_expr(left)?;
        if matches!(op, BinaryOp::Eq | BinaryOp::Ne) && is_aggregate_type(self.program, &left.ty)? {
            return Err(backend_error(
                "aggregate equality is outside executable copy variants v1",
            ));
        }
        // Owned strings compare by UTF-8 contents; both operand buffers stay
        // owned by this expression and are freed right after the comparison.
        if matches!(op, BinaryOp::Eq | BinaryOp::Ne) && matches!(left.ty, ResolvedType::String) {
            let right = self.emit_expr(right)?;
            self.require_type(&right.ty, &ResolvedType::String, "binary right operand")?;
            self.require_type(result_type, &ResolvedType::Bool, "binary result")?;
            let temporary = self.temporary(&ResolvedType::Bool)?;
            let comparison = if op == BinaryOp::Eq {
                format!("spx_string_eq({}, {})", left.code, right.code)
            } else {
                format!("!spx_string_eq({}, {})", left.code, right.code)
            };
            self.line(&format!("{temporary} = {comparison};"));
            self.string_drop(&left.code);
            self.string_drop(&right.code);
            return Ok(CValue {
                code: temporary,
                ty: ResolvedType::Bool,
            });
        }
        if !matches!(op, BinaryOp::Eq | BinaryOp::Ne) && matches!(left.ty, ResolvedType::String) {
            return Err(backend_error(
                "string operands only support equality comparison",
            ));
        }
        let float_operand = matches!(left.ty, ResolvedType::F32 | ResolvedType::F64);
        // Chars compare by Unicode scalar value; C unsigned comparison on
        // uint32_t matches the verified ordering exactly.
        let char_operand = matches!(left.ty, ResolvedType::Char);
        let int32_operand = matches!(left.ty, ResolvedType::I32);
        let narrow_operand = matches!(left.ty, ResolvedType::U8);
        let usize_operand = matches!(left.ty, ResolvedType::Usize);
        let operand_type = match op {
            BinaryOp::And | BinaryOp::Or => ResolvedType::Bool,
            BinaryOp::Eq | BinaryOp::Ne => left.ty.clone(),
            _ if float_operand
                || char_operand
                || int32_operand
                || narrow_operand
                || usize_operand =>
            {
                left.ty.clone()
            }
            _ => ResolvedType::I64,
        };
        self.require_type(&left.ty, &operand_type, "binary left operand")?;
        let expected_result = match op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
                if float_operand || int32_operand =>
            {
                left.ty.clone()
            }
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
                if narrow_operand =>
            {
                ResolvedType::U8
            }
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
                if usize_operand =>
            {
                ResolvedType::Usize
            }
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                ResolvedType::I64
            }
            _ => ResolvedType::Bool,
        };
        self.require_type(result_type, &expected_result, "binary result")?;
        if float_operand && op == BinaryOp::Rem {
            return Err(backend_error(
                "floating-point remainder has no admitted native lowering",
            ));
        }
        if narrow_operand && op == BinaryOp::Rem {
            return Err(backend_error(
                "u8 remainder has no admitted native lowering",
            ));
        }
        if char_operand
            && matches!(
                op,
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
            )
        {
            return Err(backend_error(
                "char arithmetic has no admitted native lowering",
            ));
        }
        let right = self.emit_expr(right)?;
        self.require_type(&right.ty, &operand_type, "binary right operand")?;
        let temporary = self.temporary(&expected_result)?;
        if float_operand
            && matches!(
                op,
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div
            )
        {
            // IEEE-754 semantics are total: overflow, signed zero, and
            // division by zero follow the hardware rules and never select a
            // failure status.
            self.line(&format!(
                "{temporary} = ({} {} {});",
                left.code,
                op.text(),
                right.code
            ));
        } else if narrow_operand
            && matches!(
                op,
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div
            )
        {
            // Checked u8 arithmetic computes in int64_t and range-checks the
            // 0..=255 result before narrowing to the uint8_t temporary.
            let helper = match op {
                BinaryOp::Add => "spx_rt_u8_add",
                BinaryOp::Sub => "spx_rt_u8_sub",
                BinaryOp::Mul => "spx_rt_u8_mul",
                BinaryOp::Div => "spx_rt_u8_div",
                _ => unreachable!("u8 arithmetic operation was matched above"),
            };
            self.line(&format!(
                "spx_status = {helper}(spx_ctx, {}, {}, &{temporary});",
                left.code, right.code
            ));
            self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
        } else if usize_operand
            && matches!(
                op,
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
            )
        {
            let helper = match op {
                BinaryOp::Add => "spx_rt_usize_add",
                BinaryOp::Sub => "spx_rt_usize_sub",
                BinaryOp::Mul => "spx_rt_usize_mul",
                BinaryOp::Div => "spx_rt_usize_div",
                BinaryOp::Rem => "spx_rt_usize_rem",
                _ => unreachable!("usize arithmetic operation was matched above"),
            };
            self.line(&format!(
                "spx_status = {helper}(spx_ctx, {}, {}, &{temporary});",
                left.code, right.code
            ));
            self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
        } else if matches!(
            op,
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
        ) {
            let helper = match op {
                BinaryOp::Add if int32_operand => "spx_rt_add_i32",
                BinaryOp::Sub if int32_operand => "spx_rt_sub_i32",
                BinaryOp::Mul if int32_operand => "spx_rt_mul_i32",
                BinaryOp::Div if int32_operand => "spx_rt_div_i32",
                BinaryOp::Rem if int32_operand => "spx_rt_rem_i32",
                BinaryOp::Add => "spx_rt_add",
                BinaryOp::Sub => "spx_rt_sub",
                BinaryOp::Mul => "spx_rt_mul",
                BinaryOp::Div => "spx_rt_div",
                BinaryOp::Rem => "spx_rt_rem",
                _ => unreachable!("checked arithmetic operation was matched above"),
            };
            self.line(&format!(
                "spx_status = {helper}(spx_ctx, {}, {}, &{temporary});",
                left.code, right.code
            ));
            self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
        } else {
            self.line(&format!(
                "{temporary} = ({} {} {});",
                left.code,
                op.text(),
                right.code
            ));
        }
        Ok(CValue {
            code: temporary,
            ty: expected_result,
        })
    }

    fn emit_lazy_binary(
        &mut self,
        op: BinaryOp,
        left: &ResolvedExpr,
        right: &ResolvedExpr,
        result_type: &ResolvedType,
    ) -> Result<CValue, Diagnostic> {
        struct Continuation {
            op: BinaryOp,
            temporary: String,
        }

        self.require_type(result_type, &ResolvedType::Bool, "binary result")?;
        let mut continuations = Vec::new();
        let mut current_op = op;
        let mut current_left = left;
        let mut current_right = right;
        loop {
            let left = self.emit_expr(current_left)?;
            self.require_type(&left.ty, &ResolvedType::Bool, "binary left operand")?;
            let temporary = self.temporary(&ResolvedType::Bool)?;
            self.line(&format!("if ({}) {{", left.code));
            self.indent += 1;
            if current_op == BinaryOp::Or {
                self.line(&format!("{temporary} = true;"));
                self.indent -= 1;
                self.line("} else {");
                self.indent += 1;
            }
            continuations.push(Continuation {
                op: current_op,
                temporary,
            });
            match &current_right.kind {
                ResolvedExprKind::Binary {
                    op: next_op,
                    left: next_left,
                    right: next_right,
                } if matches!(next_op, BinaryOp::And | BinaryOp::Or) => {
                    self.require_type(&current_right.ty, &ResolvedType::Bool, "binary result")?;
                    current_op = *next_op;
                    current_left = next_left;
                    current_right = next_right;
                }
                _ => break,
            }
        }

        let mut value = self.emit_expr(current_right)?;
        self.require_type(&value.ty, &ResolvedType::Bool, "lazy right operand")?;
        while let Some(continuation) = continuations.pop() {
            self.line(&format!("{} = {};", continuation.temporary, value.code));
            if continuation.op == BinaryOp::And {
                self.indent -= 1;
                self.line("} else {");
                self.indent += 1;
                self.line(&format!("{} = false;", continuation.temporary));
            }
            self.indent -= 1;
            self.line("}");
            value = CValue {
                code: continuation.temporary,
                ty: ResolvedType::Bool,
            };
        }
        Ok(value)
    }
}

#[cfg(test)]
#[path = "expression/tests.rs"]
mod tests;
