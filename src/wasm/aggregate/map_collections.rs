//! Private typed Map/Set host adapter. Checked failures settle canonical owners.
use super::*;
use crate::map_ops::MapOp;
pub(super) const CHECKED_ID: &str = "core.collection.wasm.checked.v2";
pub(super) const DROP_ID: &str = "core.collection.wasm.drop.v2";
pub(super) const IMPORT_COUNT: u32 = 2;
pub(in crate::wasm) fn uses(program: &ResolvedProgram) -> bool {
    if crate::map_ops::resolved_program_uses(program) {
        return true;
    }
    executable_functions(program).iter().any(|(f, _)| {
        crate::map_ops::is_collection(&f.return_type)
            || f.params
                .iter()
                .any(|p| crate::map_ops::is_collection(&p.ty))
            || f.requires
                .iter()
                .chain(std::iter::once(&f.body))
                .chain(&f.ensures)
                .any(|root| {
                    let mut pending = vec![root];
                    while let Some(e) = pending.pop() {
                        if crate::map_ops::is_collection(&e.ty) {
                            return true;
                        }
                        if let ResolvedExprKind::Call { callee, .. } = &e.kind {
                            if crate::string_ops::by_id(callee.as_str()).is_some_and(|op| {
                                op.is_collection() && op != crate::string_ops::StringOp::Compare
                            }) {
                                return true;
                            }
                        }
                        crate::hir::push_resolved_expression_children_in_authored_order(
                            e,
                            &mut pending,
                        );
                    }
                    false
                })
    })
}
pub(super) fn import_types(
    program: &ResolvedProgram,
    types: &mut Vec<Signature>,
    indexes: &mut HashMap<Signature, u32>,
) -> Option<[u32; 2]> {
    uses(program).then(|| {
        [
            intern_type(
                Signature {
                    params: vec![I32, I32, I64, I64, I64, I32],
                    results: vec![I32],
                },
                types,
                indexes,
            ),
            intern_type(
                Signature {
                    params: vec![I64],
                    results: Vec::new(),
                },
                types,
                indexes,
            ),
        ]
    })
}
pub(super) fn emit_imports(output: &mut Vec<u8>, types: [u32; 2]) {
    function_import(output, "env", "spx_collection_checked_v2", types[0]);
    function_import(output, "env", "spx_collection_drop_v2", types[1]);
}
pub(super) fn insert_indexes(indexes: &mut HashMap<FunctionExecutionId, u32>, base: u32) {
    indexes.insert(
        FunctionExecutionId::Monomorphic(DeclarationId::new(CHECKED_ID)),
        base,
    );
    indexes.insert(
        FunctionExecutionId::Monomorphic(DeclarationId::new(DROP_ID)),
        base + 1,
    );
}
fn tag(ty: &ResolvedType) -> Result<i32, Diagnostic> {
    Ok(match ty {
        ResolvedType::String => 1,
        ResolvedType::I64 => 2,
        ResolvedType::Bool => 3,
        ResolvedType::I32 => 4,
        ResolvedType::U8 => 5,
        ResolvedType::Usize => 6,
        ResolvedType::Char => 7,
        ResolvedType::F32 => 8,
        ResolvedType::F64 => 9,
        _ => return Err(error("closed collection atom type required")),
    })
}
pub(super) fn legacy_op(op: crate::string_ops::StringOp) -> Option<MapOp> {
    use crate::string_ops::StringOp as S;
    Some(match op {
        S::MapNew => MapOp::New,
        S::MapAdd => MapOp::Add,
        S::MapSet => MapOp::Set,
        S::MapRemove => MapOp::Remove,
        S::MapGetOr => MapOp::GetOr,
        S::MapHas => MapOp::Has,
        S::MapLen => MapOp::Len,
        S::MapKeyAt => MapOp::KeyAt,
        S::MapValueAt => MapOp::ValueAt,
        _ => return None,
    })
}
impl Emitter<'_> {
    fn collection_atom(&mut self, value: &Value) {
        self.get_scalar(value);
        match value_type(value) {
            ResolvedType::F64 => self.output.push(0xbd),
            ResolvedType::F32 => self.output.extend([0xbc, 0xad]),
            ResolvedType::I32 => self.output.push(0xac),
            ResolvedType::U8 | ResolvedType::Char | ResolvedType::Bool => self.output.push(0xad),
            _ => {}
        }
    }
    pub(super) fn emit_collection_operation(
        &mut self,
        expression: &ResolvedExpr,
        op: MapOp,
        types: &[ResolvedType],
        args: &[ResolvedExpr],
        legacy: bool,
    ) -> Result<Value, Diagnostic> {
        let (params, result) = op
            .resolved_signature(types)
            .ok_or_else(|| error("invalid collection signature"))?;
        require_type(&expression.ty, &result, "collection result")?;
        let mut values = Vec::new();
        for (index, (arg, param)) in args.iter().zip(&params).enumerate() {
            let value = self.emit_expr(arg)?;
            require_type(value_type(&value), &param.ty, "collection operand")?;
            let value = if param.ownership == crate::hir::OwnershipMode::Own {
                let storage = crate::cleanup_plan::StorageId::CallArgument {
                    call: expression.id.clone(),
                    parameter_index: index as u32,
                    value_expression: arg.id.clone(),
                };
                Value::Scalar {
                    local: *self
                        .plan
                        .cleanup_call_argument_carriers
                        .get(&storage)
                        .ok_or_else(|| {
                            error("collection owner lacks canonical argument carrier")
                        })?,
                    ty: param.ty.clone(),
                }
            } else {
                value
            };
            values.push(value);
        }
        let offset = *self
            .plan
            .call_out
            .get(&expression.id)
            .ok_or_else(|| error("collection output slot absent"))?;
        let pointer = Pointer {
            local: self.plan.frame_base,
            offset,
        };
        let operation = match op {
            MapOp::New | MapOp::SetNew => 1,
            MapOp::Set | MapOp::SetInsert => 2,
            MapOp::Add => 3,
            MapOp::Remove | MapOp::SetRemove => 4,
            MapOp::GetOr => 5,
            MapOp::Has | MapOp::SetHas => 6,
            MapOp::Len | MapOp::SetLen => 7,
            MapOp::KeyAt | MapOp::SetKeyAt => 8,
            MapOp::ValueAt => 9,
        };
        let kind = tag(&types[0])?
            | ((if op.is_set() { 3 } else { tag(&types[1])? }) << 8)
            | if legacy { 1 << 16 } else { 0 };
        self.output.push(0x41);
        write_i64(self.output, operation);
        self.output.push(0x41);
        write_i64(self.output, i64::from(kind));
        if matches!(op, MapOp::New | MapOp::SetNew) {
            self.output.extend([0x42, 0]);
            self.collection_atom(&values[0]);
            self.output.extend([0x42, 0]);
        } else {
            self.get_scalar(&values[0]);
            if values.len() > 1 {
                self.collection_atom(&values[1]);
            } else {
                self.output.extend([0x42, 0]);
            }
            if values.len() > 2 {
                self.collection_atom(&values[2]);
            } else {
                self.output
                    .extend([0x42, if op == MapOp::SetInsert { 1 } else { 0 }]);
            }
        }
        self.emit_pointer(pointer);
        self.output.push(0x10);
        write_u32(
            self.output,
            *self
                .function_indexes
                .get(&FunctionExecutionId::Monomorphic(DeclarationId::new(
                    CHECKED_ID,
                )))
                .ok_or_else(|| error("collection checked import absent"))?,
        );
        self.output.push(0x21);
        write_u32(self.output, self.plan.status);
        let base = if legacy { 25 } else { 29 };
        let statuses = match op {
            MapOp::New | MapOp::SetNew => vec![0, base + 3],
            MapOp::Add => vec![0, base + 1, base + 4],
            MapOp::Set | MapOp::SetInsert => vec![0, base + 1],
            MapOp::KeyAt | MapOp::ValueAt | MapOp::SetKeyAt => vec![0, base + 2],
            _ => vec![0],
        };
        for (i, status) in statuses.into_iter().enumerate() {
            self.output.push(0x20);
            write_u32(self.output, self.plan.status);
            self.output.push(0x41);
            write_i64(self.output, status);
            self.output.push(0x47);
            if i != 0 {
                self.output.push(0x71);
            }
        }
        self.trap_if();
        self.output.push(0x20);
        write_u32(self.output, self.plan.status);
        self.output.extend([0x04, 0x40]);
        self.emit_failure_cleanup(&expression.id, StatusLane::OperationFailure)?;
        self.output.push(0x0c);
        write_u32(
            self.output,
            self.control_depth + self.status_exit_extra_depth,
        );
        self.output.push(0x0b);
        // Host owns updates only after checked success. Failed calls retain their staged owner.
        self.apply_call_commit(&expression.id)?;
        self.emit_pointer(pointer);
        self.load_scalar(&ResolvedType::I64);
        match &result {
            ResolvedType::F64 => self.output.push(0xbf),
            ResolvedType::F32 => self.output.extend([0xa7, 0xbe]),
            ResolvedType::I32 | ResolvedType::Bool | ResolvedType::U8 | ResolvedType::Char => {
                self.output.push(0xa7)
            }
            _ => {}
        }
        let local = self.plan.expr_scalar(expression)?;
        self.output.push(0x21);
        write_u32(self.output, local);
        Ok(Value::Scalar { local, ty: result })
    }
}
