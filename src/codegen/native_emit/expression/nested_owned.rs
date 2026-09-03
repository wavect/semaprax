//! Fieldwise C lowering for validated acyclic owned-byte records.

use std::collections::BTreeSet;

use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationId, ResolvedExpr, ResolvedExprKind, ResolvedType, ResolvedTypeDeclarationKind,
};

use super::{backend_error, c_field_symbol, CEmitter, COutput, CValue};

impl<'a, O: COutput> CEmitter<'a, O> {
    pub(super) fn emit_update_record_expr(
        &mut self,
        expr: &ResolvedExpr,
    ) -> Result<CValue, Diagnostic> {
        let ResolvedExprKind::UpdateRecord {
            base,
            record,
            fields,
        } = &expr.kind
        else {
            unreachable!("non-UpdateRecord expression reached emit_update_record_expr")
        };
        let base = self.emit_expr(base)?;
        self.require_type(&base.ty, &expr.ty, "record update base")?;
        let layout = self.record_layout(&expr.ty)?;
        if layout.record != *record {
            return Err(backend_error(format!(
                "native record update `{record}` has result type `{}`",
                expr.ty.identity_key()
            )));
        }
        let temporary = self.temporary(&expr.ty)?;
        if self.record_contains_owned_bytes(&expr.ty)? {
            self.zero_owned_record_bytes(&temporary, &expr.ty)?;
            self.move_owned_record_fields(&temporary, &base.code, &expr.ty)?;
        } else {
            self.line(&format!("{temporary} = {};", base.code));
        }
        for replacement in fields {
            let field = layout.field(&replacement.field).cloned().ok_or_else(|| {
                backend_error(format!(
                    "native record `{record}` has no update field `{}`",
                    replacement.field
                ))
            })?;
            let value = self.emit_expr(&replacement.value)?;
            self.require_type(&value.ty, &field.ty, "record update field")?;
            if field.size != 0 && self.record_contains_owned_bytes(&field.ty)? {
                let destination = format!("{temporary}.{}", c_field_symbol(&field.field));
                self.move_owned_record_fields(&destination, &value.code, &field.ty)?;
            } else if field.size != 0 {
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
        let value = CValue {
            code: temporary,
            ty: expr.ty.clone(),
        };
        self.require_type(&value.ty, &expr.ty, "expression")?;
        Ok(value)
    }

    pub(crate) fn record_contains_owned_bytes(
        &self,
        ty: &ResolvedType,
    ) -> Result<bool, Diagnostic> {
        if !self.is_exact_record(ty)? {
            return Ok(false);
        }
        self.classify_owned_record(ty)
    }

    pub(crate) fn move_owned_record_fields(
        &mut self,
        destination: &str,
        source: &str,
        ty: &ResolvedType,
    ) -> Result<(), Diagnostic> {
        if !self.classify_owned_record(ty)? {
            return Err(backend_error(
                "owned-record move received a record without owned Bytes",
            ));
        }
        self.move_fields(destination, source, ty)
    }

    pub(super) fn zero_owned_record_bytes(
        &mut self,
        destination: &str,
        ty: &ResolvedType,
    ) -> Result<(), Diagnostic> {
        if !self.classify_owned_record(ty)? {
            return Err(backend_error(
                "owned-record initialization received a record without owned Bytes",
            ));
        }
        self.zero_bytes(destination, ty)
    }

    fn is_exact_record(&self, ty: &ResolvedType) -> Result<bool, Diagnostic> {
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = ty
        else {
            return Ok(false);
        };
        let item = self
            .program
            .types
            .iter()
            .find(|item| item.id == *declaration)
            .ok_or_else(|| backend_error(format!("unknown native type `{declaration}`")))?;
        Ok(arguments.is_empty()
            && item.type_parameters.is_empty()
            && matches!(item.kind, ResolvedTypeDeclarationKind::Record { .. }))
    }

    fn classify_owned_record(&self, root: &ResolvedType) -> Result<bool, Diagnostic> {
        enum Frame {
            Enter(ResolvedType, usize),
            Leave(DeclarationId),
        }

        let mut pending = vec![Frame::Enter(root.clone(), 1)];
        let mut active = BTreeSet::new();
        let mut fields = 0usize;
        let mut leaves = 0usize;
        let mut contains = false;
        while let Some(frame) = pending.pop() {
            match frame {
                Frame::Enter(ResolvedType::Bytes, _) => {
                    contains = true;
                    leaves = leaves
                        .checked_add(1)
                        .ok_or_else(|| backend_error("native owned-byte leaf count overflowed"))?;
                    if leaves > crate::cleanup::MAX_CLEANUP_OWNED_LEAVES {
                        return Err(backend_error(
                            "nested owned native lowering exceeds its owned-leaf limit",
                        ));
                    }
                }
                Frame::Enter(
                    ResolvedType::I64
                    | ResolvedType::I32
                    | ResolvedType::Char
                    | ResolvedType::U8
                    | ResolvedType::Usize
                    | ResolvedType::F32
                    | ResolvedType::F64
                    | ResolvedType::Bool,
                    _,
                ) => {}
                Frame::Enter(ty @ ResolvedType::Nominal { .. }, depth) => {
                    if depth > crate::cleanup::MAX_CLEANUP_SHAPE_DEPTH {
                        return Err(backend_error(
                            "nested owned native lowering exceeds its record-depth limit",
                        ));
                    }
                    if !self.is_exact_record(&ty)? {
                        return Err(backend_error(
                            "non-record nominal reached nested owned native lowering",
                        ));
                    }
                    let layout = self.record_layout(&ty)?;
                    if !active.insert(layout.record.clone()) {
                        return Err(backend_error(
                            "cyclic record reached nested owned native lowering",
                        ));
                    }
                    fields = fields.checked_add(layout.fields.len()).ok_or_else(|| {
                        backend_error("native owned-record field count overflowed")
                    })?;
                    if fields > crate::cleanup::MAX_CLEANUP_VISITED_FIELDS {
                        return Err(backend_error(
                            "nested owned native lowering exceeds its field limit",
                        ));
                    }
                    pending.push(Frame::Leave(layout.record));
                    for field in layout.fields.into_iter().rev() {
                        pending.push(Frame::Enter(field.ty, depth + 1));
                    }
                }
                Frame::Enter(_, _) => {
                    return Err(backend_error(
                        "closed field kind reached nested owned native lowering",
                    ));
                }
                Frame::Leave(record) => {
                    active.remove(&record);
                }
            }
        }
        Ok(contains)
    }

    fn move_fields(
        &mut self,
        destination: &str,
        source: &str,
        ty: &ResolvedType,
    ) -> Result<(), Diagnostic> {
        enum Action {
            Record(String, String, ResolvedType),
            Scalar(String, String),
        }
        let mut pending = vec![Action::Record(
            destination.to_owned(),
            source.to_owned(),
            ty.clone(),
        )];
        while let Some(action) = pending.pop() {
            let (destination, source, ty) = match action {
                Action::Record(destination, source, ty) => (destination, source, ty),
                Action::Scalar(destination, source) => {
                    self.line(&format!("{destination} = {source};"));
                    continue;
                }
            };
            let layout = self.record_layout(&ty)?;
            for field in layout.fields.into_iter().rev() {
                if field.size == 0 || field.ty == ResolvedType::Bytes {
                    continue;
                }
                let symbol = c_field_symbol(&field.field);
                let target = format!("({destination}).{symbol}");
                let source_field = format!("({source}).{symbol}");
                if self.is_exact_record(&field.ty)? {
                    pending.push(Action::Record(target, source_field, field.ty));
                } else if matches!(
                    field.ty,
                    ResolvedType::Nominal { .. } | ResolvedType::String
                ) {
                    return Err(backend_error(
                        "closed field kind reached nested owned native move",
                    ));
                } else {
                    pending.push(Action::Scalar(target, source_field));
                }
            }
        }
        Ok(())
    }

    fn zero_bytes(&mut self, destination: &str, ty: &ResolvedType) -> Result<(), Diagnostic> {
        enum Action {
            Record(String, ResolvedType),
            Bytes(String),
        }
        let mut pending = vec![Action::Record(destination.to_owned(), ty.clone())];
        while let Some(action) = pending.pop() {
            let (destination, ty) = match action {
                Action::Record(destination, ty) => (destination, ty),
                Action::Bytes(destination) => {
                    self.line(&format!("{destination} = (spx_bytes_v1) {{0}};"));
                    continue;
                }
            };
            let layout = self.record_layout(&ty)?;
            for field in layout.fields.into_iter().rev() {
                let symbol = c_field_symbol(&field.field);
                let target = format!("({destination}).{symbol}");
                if field.ty == ResolvedType::Bytes {
                    pending.push(Action::Bytes(target));
                } else if self.is_exact_record(&field.ty)? {
                    pending.push(Action::Record(target, field.ty));
                }
            }
        }
        Ok(())
    }
}
