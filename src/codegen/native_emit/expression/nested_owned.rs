//! Fieldwise C lowering for validated acyclic owned-byte records.

use std::collections::BTreeSet;

use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, DeclarationId, ResolvedExpr, ResolvedExprKind, ResolvedType, ResolvedTypeDeclarationKind,
};

use super::{
    backend_error, c_field_symbol, CBinding, CEmitter, COutput, CValue, RecordMatchBindingMode,
};

pub(super) fn owned_record_pattern_anchors(
    fields: &[crate::hir::ResolvedRecordMatchPatternField],
) -> Result<BTreeSet<crate::cleanup_plan::StorageId>, Diagnostic> {
    let mut pending = fields
        .iter()
        .map(|field| (&field.pattern, 1usize))
        .collect::<Vec<_>>();
    let mut anchors = BTreeSet::new();
    let mut visited = 0usize;
    while let Some((pattern, depth)) = pending.pop() {
        if depth > crate::cleanup::MAX_CLEANUP_SHAPE_DEPTH {
            return Err(backend_error(
                "owned record match cleanup exceeds the pattern depth limit",
            ));
        }
        visited = visited
            .checked_add(1)
            .ok_or_else(|| backend_error("owned record match field count overflowed"))?;
        if visited > crate::cleanup::MAX_CLEANUP_VISITED_FIELDS {
            return Err(backend_error(
                "owned record match cleanup exceeds the field limit",
            ));
        }
        match pattern {
            crate::hir::ResolvedRecordMatchFieldPattern::Binding(binding)
                if binding.ty == ResolvedType::Bytes =>
            {
                anchors.insert(crate::cleanup_plan::StorageId::Value(binding.id.clone()));
            }
            crate::hir::ResolvedRecordMatchFieldPattern::Record { fields, .. } => {
                pending.extend(fields.iter().rev().map(|field| (&field.pattern, depth + 1)));
            }
            crate::hir::ResolvedRecordMatchFieldPattern::Binding(_)
            | crate::hir::ResolvedRecordMatchFieldPattern::Wildcard => {}
        }
    }
    Ok(anchors)
}

pub(super) fn bind_record_match_pattern<O: COutput>(
    emitter: &mut CEmitter<'_, O>,
    base: &str,
    expected: &ResolvedType,
    record: &DeclarationId,
    instance: &ResolvedType,
    fields: &[hir::ResolvedRecordMatchPatternField],
    binding_mode: &RecordMatchBindingMode<'_>,
) -> Result<(), Diagnostic> {
    struct Frame<'p, 's> {
        base: String,
        expected: ResolvedType,
        record: &'p DeclarationId,
        instance: &'p ResolvedType,
        fields: &'p [hir::ResolvedRecordMatchPatternField],
        binding_mode: RecordMatchBindingMode<'s>,
        index: usize,
        seen: BTreeSet<DeclarationId>,
        depth: usize,
    }
    let mut pending = vec![Frame {
        base: base.to_owned(),
        expected: expected.clone(),
        record,
        instance,
        fields,
        binding_mode: binding_mode.clone(),
        index: 0,
        seen: BTreeSet::new(),
        depth: 1,
    }];
    let mut visited_fields = 0usize;
    while let Some(mut frame) = pending.pop() {
        if frame.depth > crate::cleanup::MAX_CLEANUP_SHAPE_DEPTH {
            return Err(backend_error(
                "nested record pattern exceeds the native depth limit",
            ));
        }
        emitter.require_type(&frame.expected, frame.instance, "record pattern instance")?;
        let layout = emitter.record_layout(&frame.expected)?;
        if layout.record != *frame.record || frame.fields.len() != layout.fields.len() {
            return Err(backend_error(
                "record pattern disagrees with its exact aggregate layout",
            ));
        }
        if frame.index == 0 {
            visited_fields = visited_fields
                .checked_add(frame.fields.len())
                .ok_or_else(|| backend_error("record pattern field count overflowed"))?;
            if visited_fields > crate::cleanup::MAX_CLEANUP_VISITED_FIELDS {
                return Err(backend_error(
                    "nested record pattern exceeds the native field limit",
                ));
            }
        }
        let Some(field) = frame.fields.get(frame.index) else {
            continue;
        };
        let layout_field = layout.field(&field.field).cloned().ok_or_else(|| {
            backend_error(format!(
                "record pattern `{}` has unknown field `{}`",
                frame.record, field.field
            ))
        })?;
        if !frame.seen.insert(field.field.clone()) {
            return Err(backend_error(format!(
                "record pattern `{}` repeats field `{}`",
                frame.record, field.field
            )));
        }
        let field_code = if layout_field.size == 0 {
            emitter
                .emit_erased_record_field_value(&layout_field.ty)?
                .code
        } else {
            format!("({}).{}", frame.base, c_field_symbol(&layout_field.field))
        };
        match &field.pattern {
            hir::ResolvedRecordMatchFieldPattern::Binding(binding) => {
                emitter.require_type(&binding.ty, &layout_field.ty, "record pattern binding")?;
                let name = if matches!(layout_field.ty, ResolvedType::Bytes) {
                    match frame.binding_mode.mode {
                        hir::ResolvedMatchMode::Own
                            if binding.ownership == hir::OwnershipMode::Own =>
                        {
                            emitter
                                .bytes_plan
                                .ok_or_else(|| {
                                    backend_error("owned record pattern has no Bytes cleanup plan")
                                })?
                                .value(&crate::cleanup_plan::StorageId::Value(binding.id.clone()))?
                                .to_owned()
                        }
                        hir::ResolvedMatchMode::Borrow
                            if binding.ownership == hir::OwnershipMode::Borrow =>
                        {
                            let source = frame.binding_mode.source_storage.ok_or_else(|| {
                                backend_error("borrowed record match source is not one owned place")
                            })?;
                            let path = frame
                                .binding_mode
                                .source_path
                                .iter()
                                .chain(std::iter::once(&layout_field.field))
                                .cloned()
                                .collect::<Vec<_>>();
                            let planned = emitter.bytes_plan.and_then(|plan| {
                                plan.projected_value_if_present(source, &path)
                                    .map(str::to_owned)
                            });
                            if let Some(planned) = planned {
                                planned
                            } else {
                                let crate::cleanup_plan::StorageId::Value(root) = source else {
                                    return Err(backend_error(
                                        "borrowed record parameter alias is not value-rooted",
                                    ));
                                };
                                emitter.borrowed_aggregate_bytes
                                    .get(&(
                                        root.clone(),
                                        path,
                                    ))
                                    .cloned()
                                    .ok_or_else(|| {
                                        backend_error(
                                            "borrowed record Bytes field has no authenticated alias",
                                        )
                                    })?
                            }
                        }
                        _ => {
                            return Err(backend_error(
                                "record Bytes binding ownership disagrees with match mode",
                            ));
                        }
                    }
                } else if !nested_record_binding_is_exact(
                    emitter.record_contains_owned_bytes(&layout_field.ty)?,
                ) {
                    return Err(backend_error(
                        "owning nested record binding reached exact destructuring lowering",
                    ));
                } else if binding.ownership == hir::OwnershipMode::Value {
                    field_code
                } else {
                    return Err(backend_error(
                        "scalar record binding has non-Value ownership",
                    ));
                };
                if emitter
                    .variables
                    .insert(
                        binding.id.clone(),
                        CBinding {
                            name,
                            ty: layout_field.ty,
                        },
                    )
                    .is_some()
                {
                    return Err(backend_error("record pattern binding is not fresh"));
                }
            }
            hir::ResolvedRecordMatchFieldPattern::Wildcard => {
                let nested = emitter.record_contains_owned_bytes(&layout_field.ty)?;
                let direct = layout_field.ty == ResolvedType::Bytes;
                // The resolver admits a wildcard over a direct droppable leaf
                // under a borrow and rejects a nested owning subtree in either
                // mode. Match it exactly, so a program the front end admits is
                // never refused by a backend.
                if !wildcard_is_exact(frame.binding_mode.mode, nested)
                    || (direct && matches!(frame.binding_mode.mode, hir::ResolvedMatchMode::Own))
                {
                    return Err(backend_error(
                        "owned Bytes subtree reached a record pattern wildcard",
                    ));
                }
            }
            hir::ResolvedRecordMatchFieldPattern::Record {
                record,
                instance,
                fields,
            } => {
                let mut nested_mode = frame.binding_mode.clone();
                nested_mode.source_path.push(layout_field.field.clone());
                let child_depth = frame.depth + 1;
                let next = Frame {
                    base: frame.base,
                    expected: frame.expected,
                    record: frame.record,
                    instance: frame.instance,
                    fields: frame.fields,
                    binding_mode: frame.binding_mode,
                    index: frame.index + 1,
                    seen: frame.seen,
                    depth: frame.depth,
                };
                pending.push(next);
                pending.push(Frame {
                    base: field_code,
                    expected: layout_field.ty,
                    record,
                    instance,
                    fields,
                    binding_mode: nested_mode,
                    index: 0,
                    seen: BTreeSet::new(),
                    depth: child_depth,
                });
                continue;
            }
        }
        frame.index += 1;
        pending.push(frame);
    }
    Ok(())
}

fn wildcard_is_exact(mode: hir::ResolvedMatchMode, owns_bytes: bool) -> bool {
    !owns_bytes
        || !matches!(
            mode,
            hir::ResolvedMatchMode::Own | hir::ResolvedMatchMode::Borrow
        )
}

fn nested_record_binding_is_exact(contains_owned_bytes: bool) -> bool {
    !contains_owned_bytes
}

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
        if self.record_update_uses_owned_plan(&expr.ty)? {
            return self.emit_nested_update_record(expr, base, record, fields);
        }
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

    fn emit_nested_update_record(
        &mut self,
        expr: &ResolvedExpr,
        base_expr: &ResolvedExpr,
        record: &DeclarationId,
        fields: &[crate::hir::ResolvedFieldInitializer],
    ) -> Result<CValue, Diagnostic> {
        let base = self.emit_expr(base_expr)?;
        self.require_type(&base.ty, &expr.ty, "nested record update base")?;
        let layout = self.record_layout(&expr.ty)?;
        if layout.record != *record {
            return Err(backend_error("nested record update identity changed"));
        }
        let destination = self.temporary(&expr.ty)?;
        self.zero_owned_record_bytes(&destination, &expr.ty)?;
        let mut seen = BTreeSet::new();
        for replacement in fields {
            let field = layout.field(&replacement.field).cloned().ok_or_else(|| {
                backend_error(format!(
                    "native record `{record}` has no update field `{}`",
                    replacement.field
                ))
            })?;
            if !seen.insert(field.field.clone()) {
                return Err(backend_error("nested record update repeats a field"));
            }
            let value = self.emit_expr(&replacement.value)?;
            self.require_type(&value.ty, &field.ty, "nested record update field")?;
            if field.size == 0 || field.ty == ResolvedType::Bytes {
                continue;
            }
            let target = format!("{destination}.{}", c_field_symbol(&field.field));
            if self.record_contains_owned_bytes(&field.ty)? {
                self.move_owned_record_fields(&target, &value.code, &field.ty)?;
            } else {
                self.line(&format!("{target} = {};", value.code));
            }
        }
        let preflight = self
            .bytes_plan
            .ok_or_else(|| backend_error("nested record update has no cleanup plan"))?
            .authenticate_transfers_at(&expr.id)?;
        for line in preflight.lines() {
            self.line(line);
        }
        for field in &layout.fields {
            if seen.contains(&field.field) || field.size == 0 || field.ty == ResolvedType::Bytes {
                continue;
            }
            let source = format!("({}).{}", base.code, c_field_symbol(&field.field));
            let target = format!("{destination}.{}", c_field_symbol(&field.field));
            if self.record_contains_owned_bytes(&field.ty)? {
                self.move_owned_record_fields(&target, &source, &field.ty)?;
            } else {
                self.line(&format!("{target} = {source};"));
            }
        }
        let transitions = self
            .bytes_plan
            .expect("nested update plan checked above")
            .apply_at(&expr.id)?;
        for line in transitions.lines() {
            self.line(line);
        }
        let cleanup = self
            .bytes_plan
            .expect("nested update plan checked above")
            .scope_exit(&BTreeSet::from([
                crate::cleanup_plan::StorageId::Temporary(base_expr.id.clone()),
            ]))?;
        for line in cleanup.lines() {
            self.line(line);
        }
        Ok(CValue {
            code: destination,
            ty: expr.ty.clone(),
        })
    }

    pub(crate) fn record_contains_owned_bytes(
        &self,
        ty: &ResolvedType,
    ) -> Result<bool, Diagnostic> {
        if !self.is_exact_record(ty)? {
            return Ok(false);
        }
        if !crate::hir::resolved_type_contains_owned_bytes(self.program, ty) {
            return Ok(false);
        }
        self.classify_owned_record(ty)
    }

    fn record_is_nested_owned(&self, ty: &ResolvedType) -> Result<bool, Diagnostic> {
        if !self.record_contains_owned_bytes(ty)? {
            return Ok(false);
        }
        for field in &self.record_layout(ty)?.fields {
            if self.record_contains_owned_bytes(&field.ty)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn record_update_uses_owned_plan(&self, ty: &ResolvedType) -> Result<bool, Diagnostic> {
        let generic_flat = matches!(ty, ResolvedType::Nominal { arguments, .. } if !arguments.is_empty())
            && crate::hir::is_flat_owned_byte_record(&self.program.declarations, ty);
        Ok(self.record_is_nested_owned(ty)? || generic_flat)
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
        Ok(item.type_parameters.len() == arguments.len()
            && matches!(item.kind, ResolvedTypeDeclarationKind::Record { .. }))
    }

    fn classify_owned_record(&self, root: &ResolvedType) -> Result<bool, Diagnostic> {
        enum Frame {
            Enter(ResolvedType, usize),
            Leave(String),
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
                    let identity = ty.identity_key();
                    if !active.insert(identity.clone()) {
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
                    pending.push(Frame::Leave(identity));
                    for field in layout.fields.into_iter().rev() {
                        pending.push(Frame::Enter(field.ty, depth + 1));
                    }
                }
                Frame::Enter(_, _) => {
                    return Err(backend_error(
                        "closed field kind reached nested owned native lowering",
                    ));
                }
                Frame::Leave(identity) => {
                    active.remove(&identity);
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

#[cfg(test)]
mod match_admission_tests {
    use super::{nested_record_binding_is_exact, wildcard_is_exact};
    use crate::hir::ResolvedMatchMode;

    #[test]
    fn hostile_ownership_aware_hir_cannot_hide_owned_subtrees_with_wildcards() {
        assert!(!wildcard_is_exact(ResolvedMatchMode::Own, true));
        assert!(!wildcard_is_exact(ResolvedMatchMode::Borrow, true));
    }

    #[test]
    fn hostile_hir_cannot_bind_an_owning_record_as_one_terminal() {
        assert!(!nested_record_binding_is_exact(true));
        assert!(nested_record_binding_is_exact(false));
    }

    #[test]
    fn copy_only_wildcards_remain_admitted() {
        assert!(wildcard_is_exact(ResolvedMatchMode::Own, false));
        assert!(wildcard_is_exact(ResolvedMatchMode::Borrow, false));
    }
}
