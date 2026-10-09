//! Private Core-Wasm carrier for directly owned String/Bytes vector leaves.
//! The old tag-10 two-Bytes carrier remains byte-for-byte on its old routes.
use super::*;
use sha2::{Digest, Sha256};

pub(super) const IMPORT_COUNT: u32 = 9;
const NEW: u32 = 0;
const PUSH: u32 = 1;
const CLONE_AT: u32 = 2;
const REPLACE: u32 = 3;
const RESERVE: u32 = 4;
const SORT: u32 = 5;
pub(super) const INTO_ITER: u32 = 6;
pub(super) const ITER_NEXT: u32 = 7;
pub(super) const ITER_DROP: u32 = 8;
const NEW_TAG: i64 = 11;

/// Each field byte is `(type_tag << 4) | physical_slot`. Type tags 1..8
/// match scalar Vec, 9 is Bytes and 10 is String. Zero bytes terminate a
/// descriptor shorter than eight fields. The separate identity is a digest
/// of the authored declaration and ordered field identities and types.
pub(super) struct Descriptor {
    pub(super) identity: i64,
    pub(super) shape: i64,
    pub(super) tag: i64,
    pub(super) fields: Vec<(Option<DeclarationId>, ResolvedType)>,
}

pub(super) fn descriptor(program: &ResolvedProgram, element: &ResolvedType) -> Result<Descriptor, Diagnostic> {
    let semantic = crate::hir::owned_leaf_collection::layout(&program.declarations, element)
        .ok_or_else(|| error("owned-leaf Vec element declaration is not admitted"))?;
    let legacy = crate::hir::owned_record_collection::is_admitted_owned_record_collection_element(
        &program.declarations, element);
    let mut fields = Vec::new();
    if let Some(declared) = semantic.fields {
        fields.extend(declared.iter().map(|field| (Some(field.id.clone()), field.ty.clone())));
    } else {
        fields.push((None, ResolvedType::String));
    }
    let mut shape = 0_u64;
    let mut digest = Sha256::new();
    digest.update(b"semaprax.wasm.owned-leaf-vec.v1\0");
    digest.update(element.identity_key().as_bytes());
    digest.update([0]);
    let legacy_fields = legacy.then(||
        crate::hir::owned_record_collection::owned_record_element_fields(
            &program.declarations, element).expect("admitted legacy fields"));
    for (position, (id, ty)) in fields.iter().enumerate() {
        let code = match ty {
            ResolvedType::Bytes => 9_u8,
            ResolvedType::String => 10,
            _ => u8::try_from(vec_element_tag(ty)?).map_err(|_| error("invalid Vec field tag"))?,
        };
        let slot = if let (Some(legacy), Some(id)) = (&legacy_fields, id) {
            if id == &legacy.owned[0].id { 0 }
            else if id == &legacy.owned[1].id { 1 }
            else { 2 }
        } else { position };
        let byte = (code << 4) | u8::try_from(slot).map_err(|_| error("Vec field slot exceeds descriptor"))?;
        shape |= u64::from(byte) << (position * 8);
        if let Some(id) = id { digest.update(id.as_str().as_bytes()); }
        digest.update([0, code]);
        digest.update(ty.identity_key().as_bytes());
        digest.update([0]);
    }
    let hash = digest.finalize();
    Ok(Descriptor {
        identity: i64::from_le_bytes(hash[..8].try_into().expect("eight digest bytes")),
        shape: shape as i64,
        tag: if legacy { 10 } else { NEW_TAG },
        fields,
    })
}

pub(super) fn import_types(
    enabled: bool,
    types: &mut Vec<Signature>,
    indexes: &mut HashMap<Signature, u32>,
) -> Option<[u32; IMPORT_COUNT as usize]> {
    enabled.then(|| {
        let signatures = [
            (vec![I64, I64, I64], vec![I64]),
            (vec![I64; 11], vec![I64]),
            (vec![I64, I64, I64, I64, I32], vec![I32]),
            (vec![I64; 12], vec![I64]),
            (vec![I64, I64, I64, I64], vec![I64]),
            (vec![I64, I64, I64], vec![I64]),
            (vec![I64, I64, I64, I32], vec![I32]),
            (vec![I64, I64, I64, I64, I32], vec![I32]),
            (vec![I64, I64, I64, I64], Vec::new()),
        ];
        signatures.map(|(params, results)| intern_type(Signature {
            params, results,
        }, types, indexes))
    })
}

pub(super) fn emit_imports(output: &mut Vec<u8>, types: [u32; IMPORT_COUNT as usize]) {
    for (name, ty) in [
        "spx_vec_leaf_new_v1", "spx_vec_leaf_push_v1", "spx_vec_leaf_clone_at_v1",
        "spx_vec_leaf_replace_v1", "spx_vec_leaf_reserve_v1", "spx_vec_leaf_sort_v1",
        "spx_vec_leaf_into_iter_v1", "spx_vec_leaf_iter_next_v1", "spx_vec_leaf_iter_drop_v1",
    ].into_iter().zip(types) {
        function_import(output, "env", name, ty);
    }
}

pub(super) fn import_base(program: &ResolvedProgram) -> u32 {
    box_import_base(program)
        + if super::super::program_uses_box(program) { BOX_IMPORT_COUNT } else { 0 }
        + if string_runtime::program_uses_runtime(program) {
            string_runtime::import_count(program)
        } else { 0 }
        + if map_collections::uses(program) { map_collections::IMPORT_COUNT } else { 0 }
}

impl Emitter<'_> {
    fn leaf_const(&mut self, value: i64) {
        self.output.push(0x42);
        write_i64(self.output, value);
    }

    fn leaf_call(&mut self, offset: u32) {
        self.output.push(0x10);
        write_u32(self.output, import_base(self.program) + offset);
    }

    fn leaf_metadata(&mut self, descriptor: &Descriptor) {
        self.leaf_const(descriptor.identity);
        self.leaf_const(descriptor.shape);
    }

    fn leaf_owner_epoch(&self, expr: &ResolvedExpr, args: &[ResolvedExpr]) -> Result<Value, Diagnostic> {
        let epoch = crate::cleanup_plan::StorageId::CallArgument {
            call: expr.id.clone(), parameter_index: 0, value_expression: args[0].id.clone(),
        };
        Ok(Value::Scalar {
            local: *self.plan.cleanup_call_argument_carriers.get(&epoch)
                .ok_or_else(|| error("owned-leaf Vec owner has no checked call epoch"))?,
            ty: args[0].ty.clone(),
        })
    }

    fn leaf_result(&self, expr: &ResolvedExpr) -> Result<Value, Diagnostic> {
        if is_aggregate(self.program, &expr.ty)? {
            Ok(Value::Aggregate { pointer: self.plan.expr_pointer(expr)?, ty: expr.ty.clone() })
        } else {
            Ok(Value::Scalar { local: self.plan.expr_scalar(expr)?, ty: expr.ty.clone() })
        }
    }

    fn leaf_push_words(&mut self, value: &Value, descriptor: &Descriptor) -> Result<(), Diagnostic> {
        for (id, ty) in &descriptor.fields {
            let field = match id { Some(id) => self.project_value(value, id)?, None => value.clone() };
            self.require_scalar(&field, ty, "owned-leaf Vec field")?;
            if matches!(ty, ResolvedType::Bytes | ResolvedType::String) {
                self.get_scalar(&field);
            } else {
                self.emit_vec_element_bits(&field, ty)?;
            }
        }
        for _ in descriptor.fields.len()..8 { self.leaf_const(0); }
        Ok(())
    }

    fn leaf_clear_words(&mut self, value: &Value, descriptor: &Descriptor) -> Result<(), Diagnostic> {
        for (id, ty) in &descriptor.fields {
            if matches!(ty, ResolvedType::Bytes | ResolvedType::String) {
                let field = match id { Some(id) => self.project_value(value, id)?, None => value.clone() };
                self.clear_scalar(&field)?;
            }
        }
        Ok(())
    }

    pub(in crate::wasm::aggregate) fn leaf_validate_words(
        &mut self, scratch: Pointer, descriptor: &Descriptor,
    ) -> Result<(), Diagnostic> {
        let mut owned_positions = Vec::new();
        for (position, (_, ty)) in descriptor.fields.iter().enumerate() {
            let pointer = Pointer { local: scratch.local, offset: scratch.offset + position as u32 * 8 };
            if matches!(ty, ResolvedType::Bytes | ResolvedType::String) {
                owned_positions.push(position);
                self.emit_pointer(pointer);
                self.load_scalar(&ResolvedType::I64);
                self.output.extend([0x50, 0x04, 0x40, 0x05]); // zero carrier is empty
                self.emit_pointer(pointer);
                self.load_scalar(&ResolvedType::I64);
                self.leaf_const(i64::MIN);
                self.output.extend([0x83, 0x50]); // owned high bit absent
                self.trap_if();
                self.emit_pointer(pointer);
                self.load_scalar(&ResolvedType::I64);
                self.output.push(0x10);
                write_u32(self.output, BYTE_AS_SLICE_IMPORT);
                self.output.push(0x1a);
                self.output.push(0x0b);
            } else {
                self.validate_record_scalar_bits(pointer, ty)?;
            }
        }
        if let [first, second] = owned_positions.as_slice() {
            let first = Pointer { local: scratch.local, offset: scratch.offset + *first as u32 * 8 };
            let second = Pointer { local: scratch.local, offset: scratch.offset + *second as u32 * 8 };
            self.emit_pointer(first);
            self.load_scalar(&ResolvedType::I64);
            self.output.push(0x50);
            self.output.push(0x45);
            self.emit_pointer(first);
            self.load_scalar(&ResolvedType::I64);
            self.emit_pointer(second);
            self.load_scalar(&ResolvedType::I64);
            self.output.extend([0x51, 0x71]);
            self.trap_if();
        }
        for position in descriptor.fields.len()..8 {
            self.emit_pointer(Pointer { local: scratch.local, offset: scratch.offset + position as u32 * 8 });
            self.load_scalar(&ResolvedType::I64);
            self.output.push(0x50);
            self.output.push(0x45);
            self.trap_if();
        }
        Ok(())
    }

    pub(in crate::wasm::aggregate) fn leaf_store_words(&mut self, destination: &Value, scratch: Pointer, descriptor: &Descriptor)
        -> Result<(), Diagnostic> {
        for (position, (id, ty)) in descriptor.fields.iter().enumerate() {
            let target = match id { Some(id) => self.project_value(destination, id)?, None => destination.clone() };
            if let Value::ScalarMemory { pointer, .. } = &target { self.emit_pointer(*pointer); }
            self.emit_pointer(Pointer {
                local: scratch.local,
                offset: scratch.offset + position as u32 * 8,
            });
            self.load_scalar(&ResolvedType::I64);
            match ty {
                ResolvedType::Bytes | ResolvedType::String | ResolvedType::I64 | ResolvedType::Usize => {},
                ResolvedType::F64 => self.output.push(0xbf),
                ResolvedType::F32 => self.output.extend([0xa7, 0xbe]),
                ResolvedType::I32 | ResolvedType::U8 | ResolvedType::Char | ResolvedType::Bool => self.output.push(0xa7),
                _ => return Err(error("owned-leaf result field is outside admitted scalar profile")),
            }
            match target {
                Value::ScalarMemory { .. } => self.store_scalar(ty),
                Value::Scalar { local, .. } => { self.output.push(0x21); write_u32(self.output, local); },
                Value::Aggregate { .. } => return Err(error("owned-leaf result field is aggregate")),
            }
        }
        Ok(())
    }

    fn leaf_null_result(&mut self, expr: &ResolvedExpr, result: &Value, status: i32)
        -> Result<(), Diagnostic> {
        self.output.push(0x21);
        write_u32(self.output, scalar_local(result)?);
        self.get_scalar(result);
        self.output.push(0x50);
        self.emit_vec_failure_if(expr, status)
    }

    fn leaf_preflight_index(&mut self, expr: &ResolvedExpr, owner: &Value, tag: i64, index: &Value)
        -> Result<(), Diagnostic> {
        self.get_scalar(index);
        self.get_scalar(owner);
        self.output.push(0x41);
        write_i64(self.output, tag);
        self.output.push(0x10);
        write_u32(self.output, vec_import_base(self.program) + 2);
        self.output.push(0x5a); // i64.ge_u
        self.emit_vec_failure_if(expr, STATUS_VEC_GET_OUT_OF_BOUNDS)
    }

    fn leaf_preflight_push(&mut self, expr: &ResolvedExpr, owner: &Value, tag: i64)
        -> Result<(), Diagnostic> {
        let base = vec_import_base(self.program);
        for offset in [2, 3] {
            self.get_scalar(owner);
            self.output.push(0x41); write_i64(self.output, tag);
            self.output.push(0x10); write_u32(self.output, base + offset);
        }
        self.output.push(0x5a); // len >= capacity
        self.emit_vec_failure_if(expr, STATUS_VEC_PUSH_FULL)
    }

    pub(super) fn emit_owned_leaf_vec(
        &mut self, expr: &ResolvedExpr, op: crate::vec_ops::VecOp,
        element: &ResolvedType, args: &[ResolvedExpr],
    ) -> Result<Value, Diagnostic> {
        use crate::vec_ops::VecOp;
        let descriptor = descriptor(self.program, element)?;
        if !op.admits_owned_leaf() || args.len() != op.arity()
            || args.iter().enumerate().any(|(i, arg)| !op.accepts_resolved(i, &arg.ty, element))
        {
            return Err(error("owned-leaf Vec operation disagrees with checked HIR"));
        }
        require_type(&expr.ty, &op.resolved_return_type(element), "owned-leaf Vec result")?;
        let result = self.leaf_result(expr)?;
        match op {
            VecOp::WithCapacity => {
                let capacity = self.emit_expr(&args[0])?;
                self.require_scalar(&capacity, &ResolvedType::Usize, "Vec capacity")?;
                self.leaf_metadata(&descriptor);
                self.get_scalar(&capacity);
                self.leaf_call(NEW);
                self.leaf_null_result(expr, &result, STATUS_VEC_ALLOCATION_FAILURE)?;
            }
            VecOp::Push | VecOp::Replace => {
                let _ = self.emit_expr(&args[0])?;
                let index = if op == VecOp::Replace {
                    let value = self.emit_expr(&args[1])?;
                    self.require_scalar(&value, &ResolvedType::Usize, "Vec index")?;
                    Some(value)
                } else { None };
                let element_index = if index.is_some() { 2 } else { 1 };
                let mut element_value = self.emit_expr(&args[element_index])?;
                require_type(value_type(&element_value), element, "Vec owned element")?;
                let epoch = crate::cleanup_plan::StorageId::CallArgument {
                    call: expr.id.clone(), parameter_index: element_index as u32,
                    value_expression: args[element_index].id.clone(),
                };
                if !self.plan.cleanup_storage_types.contains_key(&epoch) {
                    return Err(error("owned-leaf Vec element has no checked call epoch"));
                }
                if matches!(element_value, Value::Aggregate { .. }) {
                    if self.call_argument_values.insert(epoch, element_value.clone()).is_some() {
                        return Err(error("owned-leaf Vec element call epoch repeats"));
                    }
                } else {
                    element_value = Value::Scalar {
                        local: *self.plan.cleanup_call_argument_carriers.get(&epoch)
                            .ok_or_else(|| error("owned-leaf Vec element carrier is not staged"))?,
                        ty: element.clone(),
                    };
                }
                let owner = self.leaf_owner_epoch(expr, args)?;
                if let Some(index) = &index { self.leaf_preflight_index(expr, &owner, descriptor.tag, index)?; }
                else { self.leaf_preflight_push(expr, &owner, descriptor.tag)?; }
                self.get_scalar(&owner);
                self.leaf_metadata(&descriptor);
                if let Some(index) = &index { self.get_scalar(index); }
                self.leaf_push_words(&element_value, &descriptor)?;
                self.leaf_call(if index.is_some() { REPLACE } else { PUSH });
                self.leaf_null_result(expr, &result, STATUS_VEC_ALLOCATION_FAILURE)?;
                self.apply_call_commit(&expr.id)?;
                self.clear_scalar(&owner)?;
                self.leaf_clear_words(&element_value, &descriptor)?;
            }
            VecOp::CloneAt => {
                let borrowed = self.emit_vec_borrow_place(&args[0], element)?;
                let index = self.emit_expr(&args[1])?;
                self.require_scalar(&index, &ResolvedType::Usize, "Vec clone index")?;
                self.leaf_preflight_index(expr, &borrowed, descriptor.tag, &index)?;
                let scratch = Pointer { local: self.plan.frame_base,
                    offset: *self.plan.owned_leaf_scratch.get(&expr.id)
                        .ok_or_else(|| error("owned-leaf clone scratch is absent"))? };
                self.emit_pointer(scratch);
                self.output.push(0x41); write_i64(self.output, 0xa5);
                self.output.push(0x41); write_i64(self.output, 64);
                self.output.extend([0xfc, 0x0b, 0x00]);
                self.get_scalar(&borrowed);
                self.leaf_metadata(&descriptor);
                self.get_scalar(&index);
                self.emit_pointer(scratch);
                self.leaf_call(CLONE_AT);
                self.output.push(0x22); // retain host status before selection
                write_u32(self.output, self.plan.status);
                self.output.extend([0x41, 0x01, 0x46]); // allocation refusal
                self.emit_vec_failure_if(expr, STATUS_VEC_ALLOCATION_FAILURE)?;
                self.output.push(0x20); write_u32(self.output, self.plan.status);
                self.output.extend([0x45, 0x45]); // any other nonzero is a broken host
                self.trap_if();
                self.leaf_validate_words(scratch, &descriptor)?;
                self.leaf_store_words(&result, scratch, &descriptor)?;
            }
            VecOp::ReserveOwned | VecOp::SortOwned | VecOp::Clear => {
                let _ = self.emit_expr(&args[0])?;
                let additional = if op == VecOp::ReserveOwned {
                    let value = self.emit_expr(&args[1])?;
                    self.require_scalar(&value, &ResolvedType::Usize, "Vec reserve additional")?;
                    Some(value)
                } else { None };
                let owner = self.leaf_owner_epoch(expr, args)?;
                self.get_scalar(&owner);
                if op == VecOp::Clear {
                    self.output.push(0x41); write_i64(self.output, descriptor.tag);
                    self.output.push(0x10);
                    write_u32(self.output, vec_import_base(self.program) + VEC_IMPORT_COUNT + 2);
                } else {
                    self.leaf_metadata(&descriptor);
                    if let Some(value) = &additional { self.get_scalar(value); }
                    self.leaf_call(if additional.is_some() { RESERVE } else { SORT });
                }
                self.output.push(0x21);
                write_u32(self.output, scalar_local(&result)?);
                self.get_scalar(&result);
                self.output.push(0x50);
                if op == VecOp::Clear { self.trap_if(); }
                else { self.emit_vec_failure_if(expr, STATUS_VEC_ALLOCATION_FAILURE)?; }
                self.apply_call_commit(&expr.id)?;
                self.clear_scalar(&owner)?;
            }
            VecOp::Len | VecOp::Capacity => {
                let borrowed = self.emit_vec_borrow_place(&args[0], element)?;
                self.get_scalar(&borrowed);
                self.output.push(0x41);
                write_i64(self.output, descriptor.tag);
                self.output.push(0x10);
                write_u32(self.output, vec_import_base(self.program) + if op == VecOp::Len { 2 } else { 3 });
                self.output.push(0x21);
                write_u32(self.output, scalar_local(&result)?);
            }
            VecOp::Get | VecOp::Set | VecOp::ReserveExact | VecOp::Sort =>
                return Err(error("owned-leaf Vec old operation is not admitted")),
        }
        Ok(result)
    }
}
