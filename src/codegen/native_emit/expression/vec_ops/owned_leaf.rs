//! Typed pack/unpack at the canonical owning call boundary.
use super::super::super::c_field_symbol;
use super::*;

impl<O: COutput> CEmitter<'_, O> {
    pub(super) fn emit_owned_leaf_vec(
        &mut self,
        expr: &ResolvedExpr,
        op: crate::vec_ops::VecOp,
        element: &ResolvedType,
        args: &[ResolvedExpr],
    ) -> Result<CValue, Diagnostic> {
        use crate::vec_ops::VecOp;
        if !crate::hir::owned_leaf_collection::admits_operation(
            &self.program.declarations,
            op,
            element,
        ) || args.len() != op.arity()
            || args
                .iter()
                .enumerate()
                .any(|(i, a)| !op.accepts_resolved(i, &a.ty, element))
        {
            return Err(backend_error("invalid owned leaf Vec operation"));
        }
        let layout = crate::codegen::native_vec::owned_leaf::layout(self.program, element)
            .ok_or_else(|| backend_error("missing owned leaf Vec descriptor"))?;
        let fields = crate::hir::owned_leaf_collection::layout(&self.program.declarations, element)
            .and_then(|l| l.fields)
            .map(<[_]>::to_vec);
        let descriptor = format!("&{}", layout.symbol);
        let return_type = op.resolved_return_type(element);
        self.require_type(&expr.ty, &return_type, "owned leaf Vec result")?;
        let mut values = Vec::with_capacity(args.len());
        for (i, argument) in args.iter().enumerate() {
            let value = self.emit_expr(argument)?;
            values.push(self.stage_bytes_call_argument(
                &expr.id,
                i,
                argument,
                op.param_ownership_for(i, element),
                value,
            )?);
        }
        if matches!(op, VecOp::Len | VecOp::Capacity) {
            let out = self.temporary(&ResolvedType::Usize)?;
            let member = if op == VecOp::Len { "len" } else { "capacity" };
            self.line(&format!(
                "(void)spx_leaf_check(spx_ctx, &({}), {descriptor});",
                values[0].code
            ));
            self.line(&format!("{out} = ({}).{member};", values[0].code));
            return Ok(CValue {
                code: out,
                ty: return_type,
            });
        }
        let plan = self
            .bytes_plan
            .ok_or_else(|| backend_error("owned leaf Vec lacks cleanup plan"))?;
        let out = if op == VecOp::CloneAt && fields.is_some() {
            self.call_result_temporary(element)?
        } else {
            plan.value(&crate::cleanup_plan::StorageId::Temporary(expr.id.clone()))?
                .to_owned()
        };
        let row = format!("spx_leaf_row_{}", self.next_local);
        self.next_local += 1;
        match op {
            VecOp::WithCapacity => self.line(&format!(
                "spx_status = spx_leaf_new(spx_ctx, {descriptor}, {}, &{out});",
                values[0].code
            )),
            VecOp::CloneAt => {
                self.line(&format!("unsigned char {row}[{}] = {{0}};", layout.stride));
                self.line(&format!(
                    "spx_status = spx_leaf_clone(spx_ctx, &({}), {descriptor}, {}, {row});",
                    values[0].code, values[1].code
                ));
                self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
                for (k, kind) in layout.kinds.iter().enumerate() {
                    let (code, ty) = if let Some(fields) = &fields {
                        (
                            format!("{out}.{}", c_field_symbol(&fields[k].id)),
                            fields[k].ty.clone(),
                        )
                    } else {
                        (out.clone(), ResolvedType::String)
                    };
                    let offset = layout.offsets[k];
                    if *kind >= 9 {
                        self.line(&format!(
                            "memcpy(&({code}), {row} + {offset}, sizeof({code}));"
                        ));
                    } else {
                        let bits = self.temporary(&ResolvedType::Usize)?;
                        self.line(&format!(
                            "memcpy(&{bits}, {row} + {offset}, sizeof({bits}));"
                        ));
                        self.line(&format!(
                            "{code} = {};",
                            super::vec_bits_to_scalar(&bits, &ty)?
                        ));
                    }
                }
            }
            VecOp::Push | VecOp::Replace => {
                let (source, flag, _) = plan.call_argument(&expr.id, 0)?;
                let (source, flag) = (source.to_owned(), flag.to_owned());
                let index = if op == VecOp::Push {
                    "UINT64_C(0)".to_owned()
                } else {
                    values[1].code.clone()
                };
                let push = if op == VecOp::Push { "true" } else { "false" };
                self.line(&format!("spx_status = spx_leaf_write_check(spx_ctx, &{source}, {descriptor}, {index}, {push});"));
                self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
                let item_index = if op == VecOp::Push { 1 } else { 2 };
                let item = &values[item_index];
                if fields.is_some() {
                    let storage = plan.call_argument_storage(&expr.id, item_index as u32)?;
                    for line in plan
                        .materialize_record_carrier(&storage, &item.code)?
                        .lines()
                    {
                        self.line(line);
                    }
                }
                self.line(&format!("unsigned char {row}[{}] = {{0}};", layout.stride));
                for (k, kind) in layout.kinds.iter().enumerate() {
                    let (code, ty) = if let Some(fields) = &fields {
                        (
                            format!("({}).{}", item.code, c_field_symbol(&fields[k].id)),
                            fields[k].ty.clone(),
                        )
                    } else {
                        (item.code.clone(), ResolvedType::String)
                    };
                    let offset = layout.offsets[k];
                    if *kind >= 9 {
                        self.line(&format!(
                            "memcpy({row} + {offset}, &({code}), sizeof({code}));"
                        ));
                    } else {
                        let bits = self.temporary(&ResolvedType::Usize)?;
                        self.line(&format!(
                            "{bits} = {};",
                            super::vec_scalar_to_bits(&CValue { code, ty })
                        ));
                        self.line(&format!(
                            "memcpy({row} + {offset}, &{bits}, sizeof({bits}));"
                        ));
                    }
                }
                if fields.is_none() {
                    let (_, item_flag, _) = plan.call_argument(&expr.id, item_index as u32)?;
                    self.line(&format!("{item_flag} = false;"));
                }
                self.line(&format!("spx_leaf_write(spx_ctx, &{source}, {descriptor}, {index}, {push}, {row}, &{out});"));
                self.line(&format!("{flag} = false;"));
            }
            VecOp::Clear | VecOp::ReserveOwned | VecOp::SortOwned => {
                let (source, flag, _) = plan.call_argument(&expr.id, 0)?;
                let (source, flag) = (source.to_owned(), flag.to_owned());
                let helper = match op {
                    VecOp::Clear => "spx_leaf_clear",
                    VecOp::ReserveOwned => "spx_leaf_reserve",
                    _ => "spx_leaf_sort",
                };
                let additional = if op == VecOp::ReserveOwned {
                    format!("{}, ", values[1].code)
                } else {
                    String::new()
                };
                self.line(&format!(
                    "spx_status = {helper}(spx_ctx, &{source}, {descriptor}, {additional}&{out});"
                ));
                self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
                self.line(&format!("{flag} = false;"));
            }
            _ => return Err(backend_error("unsupported owned leaf Vec operation")),
        }
        self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
        if op == VecOp::CloneAt && fields.is_some() {
            for line in plan.initialize_record_result_at(&expr.id, &out)?.lines() {
                self.line(line);
            }
        }
        for line in plan.apply_at(&expr.id)?.lines() {
            self.line(line);
        }
        Ok(CValue {
            code: plan.result_at(&expr.id).unwrap_or(&out).to_owned(),
            ty: return_type,
        })
    }
}
