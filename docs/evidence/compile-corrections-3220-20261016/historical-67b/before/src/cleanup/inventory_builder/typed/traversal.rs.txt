use super::{receiving, reserve, reserve_legacy, InventoryBuilder, InventoryFailure, Kind, Reason};
use crate::cleanup::CleanupStorageOrigin;
use crate::hir::{
    OwnershipMode, ResolvedBinding, ResolvedExpr, ResolvedExprKind, ResolvedMatchPattern,
    ResolvedRecordMatchFieldPattern, ResolvedStatement, ResolvedType,
};
impl InventoryBuilder<'_> {
    fn collect_expression(&mut self, expression: &ResolvedExpr) -> Result<(), InventoryFailure> {
        receiving!(&self.ledger, ());
        enum Frame<'a> {
            Enter(&'a ResolvedExpr),
            Children(&'a ResolvedExpr, usize),
            Finish(&'a ResolvedExpr),
            AddBinding(&'a ResolvedBinding),
            AddPatternBindings(&'a crate::hir::ResolvedMatchPattern),
            AddUpdateBase(&'a ResolvedExpr),
        }

        const { assert!(std::mem::size_of::<Frame<'static>>() == 24) };

        let mut frames = { self.singleton(Frame::Enter(expression))? };
        while let Some(frame) = frames.pop() {
            self.charge(1)?;
            match frame {
                Frame::Enter(expression) => {
                    {
                        self.needs_drop(&expression.ty)?;
                    }
                    reserve_legacy(
                        &self.ledger,
                        &mut frames,
                        2,
                        "cleanup traversal capacity exceeds address space",
                    )?;
                    frames.push(Frame::Finish(expression));
                    frames.push(Frame::Children(expression, 0));
                }
                Frame::Children(expression, index) => {
                    reserve_legacy(
                        &self.ledger,
                        &mut frames,
                        2,
                        "cleanup traversal capacity exceeds address space",
                    )?;
                    let mut enter = None;
                    let mut action = None;
                    match &expression.kind {
                        ResolvedExprKind::Closure { captures, .. } => {
                            enter = captures.get(index).map(|capture| &capture.value);
                        }
                        ResolvedExprKind::FunctionReference { .. } => {}
                        ResolvedExprKind::Invoke { callable, args } => {
                            enter = if index == 0 {
                                Some(callable.as_ref())
                            } else {
                                args.get(index - 1)
                            };
                        }
                        ResolvedExprKind::Call { args, .. }
                        | ResolvedExprKind::LiteralFormat { args, .. }
                        | ResolvedExprKind::VecFieldRead { args, .. }
                        | ResolvedExprKind::RowLoan { args, .. } => {
                            enter = args.get(index);
                        }
                        ResolvedExprKind::NativeRustImportCall(call) => {
                            enter = call.args.get(index);
                        }
                        ResolvedExprKind::HostCommandCall(call) => {
                            enter = call.args.get(index);
                        }
                        ResolvedExprKind::ByteRange {
                            source, start, end, ..
                        } => {
                            enter = [source.as_ref(), start.as_ref(), end.as_ref()]
                                .get(index)
                                .copied();
                        }
                        ResolvedExprKind::Unary { value, .. }
                        | ResolvedExprKind::Project { base: value, .. }
                        | ResolvedExprKind::Try { operand: value, .. }
                        | ResolvedExprKind::TryOption { operand: value, .. }
                        | ResolvedExprKind::Upcast { source: value }
                        | ResolvedExprKind::Yield { request: value } => {
                            enter = (index == 0).then_some(value.as_ref());
                        }
                        ResolvedExprKind::LexicalExit { value, .. } => {
                            enter = value.as_deref().filter(|_| index == 0);
                        }
                        ResolvedExprKind::Binary { left, right, .. } => {
                            enter = match index {
                                0 => Some(left),
                                1 => Some(right),
                                _ => None,
                            };
                        }
                        ResolvedExprKind::Block { statements, tail }
                        | ResolvedExprKind::VecConstructor {
                            statements, tail, ..
                        } => {
                            // Only lets add a trailing binding step; other statements keep walking.
                            let per_statement = |statement: &ResolvedStatement| {
                                statement.child_count().saturating_add(usize::from(matches!(
                                    statement,
                                    ResolvedStatement::Let { .. }
                                )))
                            };
                            let mut offset = 0usize;
                            let mut matched = None;
                            for statement in statements.iter() {
                                self.charge(1)?;
                                let steps = per_statement(statement);
                                if index < offset + steps {
                                    let within = index - offset;
                                    if within < statement.child_count() {
                                        enter = statement.child(within);
                                    }
                                    // Only `let` statements carry bindings;
                                    // assignment, unsafe, and while
                                    // statements add none here.
                                    if within == statement.child_count() {
                                        if let ResolvedStatement::Let { binding, .. } = statement {
                                            action = Some(Frame::AddBinding(binding));
                                        }
                                    }
                                    matched = Some(());
                                    break;
                                }
                                offset += steps;
                            }
                            if matched.is_none() && index == offset {
                                enter = Some(tail);
                            }
                        }
                        ResolvedExprKind::If {
                            condition,
                            then_branch,
                            else_branch,
                        } => {
                            enter = match index {
                                0 => Some(condition),
                                1 => Some(then_branch),
                                2 => Some(else_branch),
                                _ => None,
                            };
                        }
                        ResolvedExprKind::ConstructRecord { fields, .. }
                        | ResolvedExprKind::ConstructVariant { fields, .. } => {
                            enter = fields.get(index).map(|field| &field.value);
                        }
                        ResolvedExprKind::Match {
                            scrutinee, arms, ..
                        } => {
                            enter = if index == 0 {
                                Some(scrutinee)
                            } else {
                                let mut cursor = index - 1;
                                let mut found = None;
                                for arm in arms {
                                    self.charge(1)?;
                                    if cursor == 0 {
                                        action = Some(Frame::AddPatternBindings(&arm.pattern));
                                        break;
                                    }
                                    cursor -= 1;
                                    if let Some(guard) = &arm.guard {
                                        if cursor == 0 {
                                            found = Some(guard.as_ref());
                                            break;
                                        }
                                        cursor -= 1;
                                    }
                                    if cursor == 0 {
                                        found = Some(&arm.value);
                                        break;
                                    }
                                    cursor -= 1;
                                }
                                found
                            };
                        }
                        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                            if index == 0 {
                                enter = Some(base);
                            } else if index == 1 {
                                action = Some(Frame::AddUpdateBase(base));
                            } else {
                                enter = fields.get(index - 2).map(|field| &field.value);
                            }
                        }
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
                        | ResolvedExprKind::BorrowedUtf8Literal(_)
                        | ResolvedExprKind::Place(_)
                        | ResolvedExprKind::BorrowPlace { .. } => {}
                    }
                    if enter.is_some() || action.is_some() {
                        frames.push(Frame::Children(expression, index + 1));
                        if let Some(action) = action {
                            frames.push(action);
                        }
                        if let Some(child) = enter {
                            frames.push(Frame::Enter(child));
                        }
                    }
                }
                Frame::Finish(expression) => self.collect_owned_temporary(expression)?,
                Frame::AddBinding(binding) => {
                    if binding.ownership == OwnershipMode::Own && self.needs_drop(&binding.ty)? {
                        self.add_slot(
                            CleanupStorageOrigin::Binding {
                                value: self.clone_owned(&binding.id)?,
                            },
                            self.clone_owned(&binding.ty)?,
                        )?;
                    }
                }
                Frame::AddPatternBindings(pattern) => {
                    self.collect_pattern_bindings(pattern)?;
                }
                Frame::AddUpdateBase(base) => {
                    // A place expression normally needs no temporary. Record
                    // update deliberately materializes an owned base epoch.
                    if matches!(base.kind, ResolvedExprKind::Place(_))
                        && base.ownership == OwnershipMode::Own
                        && self.needs_drop(&base.ty)?
                    {
                        self.add_slot(
                            CleanupStorageOrigin::Temporary {
                                expression: self.clone_owned(&base.id)?,
                            },
                            self.clone_owned(&base.ty)?,
                        )?;
                    }
                }
            }
        }
        Ok(())
    }

    fn collect_pattern_bindings(
        &mut self,
        pattern: &crate::hir::ResolvedMatchPattern,
    ) -> Result<(), InventoryFailure> {
        receiving!(&self.ledger, (), super::receiving::REASON, 1);
        use crate::hir::{ResolvedMatchPattern, ResolvedRecordMatchFieldPattern};
        {
            self.charge(1)?;
            match pattern {
                ResolvedMatchPattern::Variant { .. }
                    if crate::hir::borrowed_scalar::option_u8::pattern(pattern) => {}
                ResolvedMatchPattern::Binding(binding)
                    if binding.ownership == OwnershipMode::Value
                        && crate::hir::is_scalar_resolved_type(&binding.ty) => {}
                ResolvedMatchPattern::Wildcard | ResolvedMatchPattern::Literal(_) => {}
                _ => {
                    return Err(InventoryFailure(Kind::Finite(
                        Reason::BudgetedHelperInventoryPatternIsOutsideExactCopyMetadata,
                    )));
                }
            }
        }

        enum Item<'a> {
            Pattern(&'a ResolvedMatchPattern),
            RecordField(&'a ResolvedRecordMatchFieldPattern),
        }
        let mut pending = { self.singleton(Item::Pattern(pattern))? };
        while let Some(item) = pending.pop() {
            self.charge(1)?;
            match item {
                Item::Pattern(ResolvedMatchPattern::Binding(binding)) => {
                    if binding.ownership == OwnershipMode::Own && self.needs_drop(&binding.ty)? {
                        self.add_slot(
                            CleanupStorageOrigin::Binding {
                                value: self.clone_owned(&binding.id)?,
                            },
                            self.clone_owned(&binding.ty)?,
                        )?;
                    }
                }
                Item::Pattern(ResolvedMatchPattern::Record { record, fields, .. }) => {
                    let declared = self
                        .program
                        .declarations()
                        .record_fields(record)
                        .ok_or_else(|| {
                            InventoryFailure(Kind::Finite(Reason::RecordPatternHasNoFieldInventory))
                        })?;
                    for declaration in declared.iter().rev() {
                        let field = fields
                            .iter()
                            .find(|field| field.field == declaration.id)
                            .ok_or_else(|| {
                                InventoryFailure(Kind::Finite(
                                    Reason::RecordPatternFieldInventoryIsIncomplete,
                                ))
                            })?;
                        reserve(&self.ledger, &mut pending, 1)?;
                        pending.push(Item::RecordField(&field.pattern));
                    }
                }
                Item::Pattern(ResolvedMatchPattern::Variant { fields, .. }) => {
                    for field in fields {
                        self.charge(1)?;
                        if field.binding.ownership == OwnershipMode::Own
                            && self.needs_drop(&field.binding.ty)?
                        {
                            self.add_slot(
                                CleanupStorageOrigin::Binding {
                                    value: self.clone_owned(&field.binding.id)?,
                                },
                                self.clone_owned(&field.binding.ty)?,
                            )?;
                        }
                    }
                }
                Item::Pattern(
                    ResolvedMatchPattern::Wildcard
                    | ResolvedMatchPattern::Literal(_)
                    | ResolvedMatchPattern::Or(_),
                ) => {}
                Item::RecordField(ResolvedRecordMatchFieldPattern::Binding(binding)) => {
                    if binding.ownership == OwnershipMode::Own && self.needs_drop(&binding.ty)? {
                        self.add_slot(
                            CleanupStorageOrigin::Binding {
                                value: self.clone_owned(&binding.id)?,
                            },
                            self.clone_owned(&binding.ty)?,
                        )?;
                    }
                }
                Item::RecordField(ResolvedRecordMatchFieldPattern::Record {
                    record,
                    fields,
                    ..
                }) => {
                    let declared = self
                        .program
                        .declarations()
                        .record_fields(record)
                        .ok_or_else(|| {
                            InventoryFailure(Kind::Finite(
                                Reason::NestedRecordPatternHasNoFieldInventory,
                            ))
                        })?;
                    for declaration in declared.iter().rev() {
                        let field = fields
                            .iter()
                            .find(|field| field.field == declaration.id)
                            .ok_or_else(|| {
                                InventoryFailure(Kind::Finite(
                                    Reason::NestedRecordPatternFieldInventoryIsIncomplete,
                                ))
                            })?;
                        reserve(&self.ledger, &mut pending, 1)?;
                        pending.push(Item::RecordField(&field.pattern));
                    }
                }
                Item::RecordField(ResolvedRecordMatchFieldPattern::Wildcard) => {}
            }
        }
        Ok(())
    }

    fn collect_owned_temporary(
        &mut self,
        expression: &ResolvedExpr,
    ) -> Result<(), InventoryFailure> {
        receiving!(&self.ledger, ());
        if expression.ownership == OwnershipMode::Own
            && self.needs_drop(&expression.ty)?
            && (expression.ty == ResolvedType::String
                || !matches!(expression.kind, ResolvedExprKind::Place(_)))
        {
            self.add_slot(
                CleanupStorageOrigin::Temporary {
                    expression: self.clone_owned(&expression.id)?,
                },
                self.clone_owned(&expression.ty)?,
            )?;
        }
        Ok(())
    }
}
