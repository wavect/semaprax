//! Native lowering of the independently checked flat Copy-record Vec slice.
use super::super::super::{backend_error, c_field_symbol, CEmitter, COutput, CValue};
use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedExpr, ResolvedType};

impl<'a, O: COutput> CEmitter<'a, O> {
    pub(super) fn emit_copy_record_vec(
        &mut self,
        expr: &ResolvedExpr,
        op: crate::vec_ops::VecOp,
        element: &ResolvedType,
        args: &[ResolvedExpr],
    ) -> Result<CValue, Diagnostic> {
        use crate::vec_ops::VecOp;
        let fields =
            crate::hir::copy_record_collection::fields(&self.program.declarations, element)
                .ok_or_else(|| backend_error("Copy record Vec element is not admitted"))?
                .to_vec();
        if args.len() != op.arity()
            || args
                .iter()
                .enumerate()
                .any(|(i, a)| !op.accepts_resolved(i, &a.ty, element))
        {
            return Err(backend_error("Copy record Vec argument shape is invalid"));
        }
        let width = fields.len();
        self.require_type(
            &expr.ty,
            &op.resolved_return_type(element),
            "Copy record Vec result",
        )?;
        let mut values = Vec::with_capacity(args.len());
        for (index, argument) in args.iter().enumerate() {
            let value = self.emit_expr(argument)?;
            values.push(self.stage_bytes_call_argument(
                &expr.id,
                index,
                argument,
                op.param_ownership(index),
                value,
            )?);
        }
        if matches!(op, VecOp::Len | VecOp::Capacity) {
            let temporary = self.temporary(&ResolvedType::Usize)?;
            let helper = if op == VecOp::Len {
                "spx_vec_len"
            } else {
                "spx_vec_capacity"
            };
            self.line(&format!(
                "(void)spx_copy_vec_check(spx_ctx, &({}), UINT64_C({width}));",
                values[0].code
            ));
            self.line(&format!(
                "{temporary} = {helper}(spx_ctx, &({}), UINT32_C(1)) / UINT64_C({width});",
                values[0].code
            ));
            return Ok(CValue {
                code: temporary,
                ty: ResolvedType::Usize,
            });
        }
        if op == VecOp::Get {
            let temporary = self.temporary(element)?;
            self.line(&format!(
                "(void)spx_copy_vec_check(spx_ctx, &({}), UINT64_C({width}));",
                values[0].code
            ));
            self.line(&format!("if ({} >= spx_vec_len(spx_ctx, &({}), UINT32_C(1)) / UINT64_C({width})) {{ spx_status = spx_vec_failure(spx_ctx, UINT32_C(2)); goto spx_epilogue; }}", values[1].code,values[0].code));
            for (index, field) in fields.iter().enumerate() {
                let bits = self.temporary(&ResolvedType::Usize)?;
                self.line(&format!("spx_status = spx_vec_get(spx_ctx, &({}), UINT32_C(1), {} * UINT64_C({width}) + UINT64_C({index}), &{bits});",values[0].code,values[1].code));
                self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
                let scalar = super::vec_bits_to_scalar(&bits, &field.ty)?;
                self.line(&format!(
                    "{temporary}.{} = {scalar};",
                    c_field_symbol(&field.id)
                ));
            }
            return Ok(CValue {
                code: temporary,
                ty: element.clone(),
            });
        }
        let plan = self
            .bytes_plan
            .ok_or_else(|| backend_error("Copy record Vec has no cleanup plan"))?;
        let destination = plan
            .value(&crate::cleanup_plan::StorageId::Temporary(expr.id.clone()))?
            .to_owned();
        if op == VecOp::WithCapacity {
            self.line(&format!(
                "spx_status = spx_copy_vec_new(spx_ctx, UINT64_C({width}), {}, &{destination});",
                values[0].code
            ));
        } else {
            let (source, flag, _) = plan.call_argument(&expr.id, 0)?;
            let source = source.to_owned();
            let flag = flag.to_owned();
            match op {
                VecOp::Push | VecOp::Set => {
                    let item = &values[if op == VecOp::Push { 1 } else { 2 }];
                    let row = format!("spx_copy_row_{}", self.next_local);
                    self.next_local += 1;
                    let words = fields
                        .iter()
                        .map(|field| {
                            super::vec_scalar_to_bits(&CValue {
                                code: format!("({}).{}", item.code, c_field_symbol(&field.id)),
                                ty: field.ty.clone(),
                            })
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    self.line(&format!("const uint64_t {row}[{width}] = {{ {words} }};"));
                    if op == VecOp::Push {
                        self.line(&format!("spx_status = spx_copy_vec_push(spx_ctx, &{source}, UINT64_C({width}), {row}, &{destination});"));
                    } else {
                        self.line(&format!("spx_status = spx_copy_vec_set(spx_ctx, &{source}, UINT64_C({width}), {}, {row}, &{destination});",values[1].code));
                    }
                }
                VecOp::ReserveExact => {
                    self.line(&format!(
                        "(void)spx_copy_vec_check(spx_ctx, &{source}, UINT64_C({width}));"
                    ));
                    self.line(&format!("if ({} > UINT64_C(8192) / UINT64_C({width})) {{ spx_status = spx_vec_failure(spx_ctx, UINT32_C(3)); goto spx_epilogue; }}",values[1].code));
                    self.line(&format!("spx_status = spx_vec_reserve_exact(spx_ctx, UINT32_C(1), &{source}, {} * UINT64_C({width}), &{destination});",values[1].code));
                }
                VecOp::Clear => self.line(&format!(
                    "spx_status = spx_vec_clear(spx_ctx, UINT32_C(1), &{source}, &{destination});"
                )),
                VecOp::Sort => {
                    let name = format!("spx_copy_tags_{}", self.next_local);
                    self.next_local += 1;
                    let tags = fields
                        .iter()
                        .map(|field| {
                            super::vec_element_tag(&field.ty).map(|tag| format!("UINT32_C({tag})"))
                        })
                        .collect::<Result<Vec<_>, _>>()?
                        .join(", ");
                    self.line(&format!("const uint32_t {name}[{width}] = {{ {tags} }};"));
                    self.line(&format!("spx_status = spx_copy_vec_sort(spx_ctx, &{source}, UINT64_C({width}), {name}, &{destination});"));
                }
                _ => return Err(backend_error("Copy record Vec owner operation is invalid")),
            }
            self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
            self.line(&format!("{flag} = false;"));
        }
        self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
        for line in plan.apply_at(&expr.id)?.lines() {
            self.line(line);
        }
        Ok(CValue {
            code: plan.result_at(&expr.id).unwrap_or(&destination).to_owned(),
            ty: expr.ty.clone(),
        })
    }
}
