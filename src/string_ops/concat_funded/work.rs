//! Test-only work-metered twins of the concat helpers. Admission, first-wins/last-wins
//! map semantics and the original `format!("{:?}")` containment verdict are
//! unchanged. Debug text is never replaced by an owner walk: it is produced by
//! the real `Debug` impls into a sink, and the sink announces each fragment and
//! each KMP step on `work` before consuming it. After a match the rest of the
//! argument is still formatted and charged, since the original builds the whole
//! string before `contains`. Every type comparison is metered; none uses the
//! derived `==`.
use super::*;
use crate::hir::funded::work::{self as paid, Disposable, Work};

impl Disposable for Needle {
    fn drop_units(&self) -> usize {
        self.bytes.drop_units() + self.table.drop_units()
    }
}

/// Debit-before-grow, announce-before-consume byte sink for the owner text.
struct PaidWork<'p, E> {
    bytes: Vec<u8>,
    work: Work<'p, E>,
    reserve: Reserve<'p, E>,
    failure: Option<FundedError<E>>,
}

impl<E> std::fmt::Write for PaidWork<'_, E> {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        if self.failure.is_some() {
            return Err(std::fmt::Error);
        }
        let step = paid::tick(&mut *self.work, text.len() + 1) // fragment + its bytes
            .and_then(|()| funded::grow(&mut self.bytes, text.len(), &mut *self.reserve));
        match step {
            Ok(()) => {
                self.bytes.extend_from_slice(text.as_bytes());
                Ok(())
            }
            Err(failure) => {
                self.failure = Some(failure);
                Err(std::fmt::Error)
            }
        }
    }
}

/// Storage-free streaming matcher; announces each fragment, byte and fallback step.
struct FindWork<'n, 'p, E> {
    needle: &'n [u8],
    table: &'n [usize],
    state: usize,
    found: bool,
    work: Work<'p, E>,
    failure: Option<FundedError<E>>,
}

impl<E> FindWork<'_, '_, E> {
    fn scan(&mut self, text: &str) -> Result<(), FundedError<E>> {
        paid::tick(&mut *self.work, text.len() + 1)?; // fragment + formatted bytes
        if self.found {
            return Ok(()); // the original has stopped searching, not formatting
        }
        for &byte in text.as_bytes() {
            paid::tick(&mut *self.work, 1)?;
            while self.state > 0 {
                paid::tick(&mut *self.work, 1)?;
                if self.needle[self.state] == byte {
                    break;
                }
                self.state = self.table[self.state - 1];
            }
            paid::tick(&mut *self.work, 1)?;
            if self.needle[self.state] == byte {
                self.state += 1;
            }
            if self.state == self.needle.len() {
                self.found = true;
                return Ok(());
            }
        }
        Ok(())
    }
}

impl<E> std::fmt::Write for FindWork<'_, '_, E> {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        if self.failure.is_some() {
            return Err(std::fmt::Error);
        }
        self.scan(text).map_err(|failure| {
            self.failure = Some(failure);
            std::fmt::Error
        })
    }
}

fn needle_with_work<E>(
    owner: &ValueId,
    work: Work<'_, E>,
    reserve: Reserve<'_, E>,
) -> Result<Needle, FundedError<E>> {
    paid::tick(work, 2)?;
    let mut sink = PaidWork {
        bytes: Vec::new(),
        work: &mut *work,
        reserve: &mut *reserve,
        failure: None,
    };
    let formatted = write!(sink, "{owner:?}");
    let PaidWork { bytes, failure, .. } = sink;
    if let Some(error) = failure {
        return Err(paid::abandon(bytes, error, work));
    }
    if formatted.is_err() {
        let error = FundedError::Layout("owner debug text failed to format");
        return Err(paid::abandon(bytes, error, work));
    }
    let mut table: Vec<usize> = Vec::new();
    let built = (|| -> Result<(), FundedError<E>> {
        paid::tick(work, 1)?;
        funded::grow(&mut table, bytes.len(), reserve)?;
        let mut matched = 0usize;
        for at in 0..bytes.len() {
            paid::tick(work, 1)?;
            if at == 0 {
                table.push(0);
                continue;
            }
            while matched > 0 {
                paid::tick(work, 1)?;
                if bytes[at] == bytes[matched] {
                    break;
                }
                matched = table[matched - 1];
            }
            paid::tick(work, 1)?;
            if bytes[at] == bytes[matched] {
                matched += 1;
            }
            table.push(matched);
        }
        Ok(())
    })();
    match built {
        Ok(()) => Ok(Needle { bytes, table }),
        Err(error) => Err(paid::abandon((bytes, table), error, work)),
    }
}

fn mentions_with_work<E>(
    argument: &ResolvedExpr,
    needle: &Needle,
    work: Work<'_, E>,
) -> Result<bool, FundedError<E>> {
    paid::tick(work, 2)?;
    let mut find = FindWork {
        needle: &needle.bytes,
        table: &needle.table,
        state: 0,
        found: false,
        work,
        failure: None,
    };
    let formatted = write!(find, "{argument:?}");
    if let Some(error) = find.failure {
        return Err(error);
    }
    if formatted.is_err() {
        return Err(FundedError::Layout("argument debug text failed to format"));
    }
    Ok(find.found)
}

/// Linear scan of the closed op table; equals `map_ops::by_id` provided that is
/// a find over `MapOp::ALL` by `id()` (asserted by the parity test below).
fn map_op_by_id_with_work<E>(
    id: &str,
    work: Work<'_, E>,
) -> Result<Option<crate::map_ops::MapOp>, FundedError<E>> {
    for op in crate::map_ops::MapOp::ALL {
        paid::tick(work, 1)?;
        paid::tick(work, paid::str_units(op.id(), id))?;
        if op.id() == id {
            return Ok(Some(op));
        }
    }
    Ok(None)
}

fn collection_with_work<E>(
    op: crate::map_ops::MapOp,
    type_arguments: &[crate::hir::ResolvedType],
    ty: &crate::hir::ResolvedType,
    work: Work<'_, E>,
) -> Result<bool, FundedError<E>> {
    use crate::hir::ResolvedType as T;
    use crate::map_ops::{MAP_ID, SET_ID};
    paid::tick(work, 1)?;
    if !op.is_set() {
        paid::tick(work, 2)?;
        if matches!(type_arguments, [T::String, T::I64]) {
            return Ok(matches!(ty, T::StringMap));
        }
    }
    paid::tick(work, 1)?;
    let T::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(false);
    };
    let id = if op.is_set() { SET_ID } else { MAP_ID };
    paid::tick(work, paid::str_units(declaration.as_str(), id))?;
    if declaration.as_str() != id {
        return Ok(false);
    }
    paid::slice_eq_with_work(arguments, type_arguments, work)
}

/// Work twin of `typed_reopen_admitted`, same checks in the same order.
fn typed_reopen_admitted_with_work<E>(
    op: crate::map_ops::MapOp,
    type_arguments: &[crate::hir::ResolvedType],
    args: &[ResolvedExpr],
    result: &crate::hir::ResolvedType,
    work: Work<'_, E>,
) -> Result<bool, FundedError<E>> {
    use crate::hir::ResolvedType as T;
    use crate::map_ops::{ast_key, ast_value, resolved_scalar_ast, MapOp};
    paid::tick(work, 1)?;
    if !op.reopens() {
        return Ok(false);
    }
    for ty in type_arguments {
        paid::tick(work, 1)?;
        if resolved_scalar_ast(ty).is_none() {
            return Ok(false);
        }
    }
    paid::tick(work, 1)?;
    if type_arguments.len() != op.type_arity() {
        return Ok(false);
    }
    paid::tick(work, 2)?;
    if !resolved_scalar_ast(&type_arguments[0]).is_some_and(|key| ast_key(&key)) {
        return Ok(false);
    }
    paid::tick(work, 1)?;
    if !op.is_set() {
        paid::tick(work, 2)?;
        let value = resolved_scalar_ast(&type_arguments[1]);
        if !value.as_ref().is_some_and(ast_value) {
            return Ok(false);
        }
        paid::tick(work, 1)?;
        if op == MapOp::Add && !paid::type_eq_with_work(&type_arguments[1], &T::I64, work)? {
            return Ok(false);
        }
    }
    let expected = 2 + usize::from(matches!(op, MapOp::Add | MapOp::Set));
    paid::tick(work, 1)?;
    if args.len() != expected {
        return Ok(false);
    }
    if !collection_with_work(op, type_arguments, result, work)?
        || !collection_with_work(op, type_arguments, &args[0].ty, work)?
        || !paid::type_eq_with_work(&args[1].ty, &type_arguments[0], work)?
    {
        return Ok(false);
    }
    paid::tick(work, 1)?;
    if expected == 2 {
        return Ok(true);
    }
    paid::type_eq_with_work(&args[2].ty, &type_arguments[1], work)
}

pub(crate) fn is_same_owner_concat_hir_funded_with_work<E>(
    value: &ResolvedExpr,
    owner: &ValueId,
    work: Work<'_, E>,
    reserve: Reserve<'_, E>,
) -> Result<bool, FundedError<E>> {
    funded::charge(reserve, 0)?;
    paid::tick(work, 1)?;
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance: None,
        args,
    } = &value.kind
    else {
        return Ok(false);
    };
    let typed = match map_op_by_id_with_work(callee.as_str(), work)? {
        Some(op) => typed_reopen_admitted_with_work(op, type_arguments, args, &value.ty, work)?,
        None => false,
    };
    let admitted = if typed {
        true
    } else {
        paid::tick(work, 1)?;
        if type_arguments.is_empty() {
            // REQUIRED source (section 4): work twin of `string_ops::by_id`.
            match super::super::by_id_with_work(callee.as_str(), work)? {
                Some(op) => {
                    paid::tick(work, 1)?;
                    if !op.is_same_owner_reopen() {
                        false
                    } else {
                        paid::tick(work, 1)?;
                        if args.len() != op.arity() {
                            false
                        } else {
                            paid::tick(work, 1)?; // `return_type()` temporary and its Drop
                            paid::type_eq_with_work(&value.ty, &op.return_type(), work)?
                        }
                    }
                }
                None => false,
            }
        } else {
            false
        }
    };
    paid::tick(work, 1)?;
    if !admitted {
        return Ok(false);
    }
    paid::tick(work, 1)?;
    let Some(ResolvedExprKind::Place(place)) = args.first().map(|argument| &argument.kind) else {
        return Ok(false);
    };
    // Assumes `ValueId::as_str` exists (section 4).
    paid::tick(work, paid::str_units(place.root.as_str(), owner.as_str()))?;
    if &place.root != owner {
        return Ok(false);
    }
    paid::tick(work, 1)?;
    if !place.projections.is_empty() {
        return Ok(false);
    }
    paid::tick(work, 1)?;
    if args.len() <= 1 {
        return Ok(true);
    }
    let needle = needle_with_work(owner, work, reserve)?;
    let verdict = (|| -> Result<bool, FundedError<E>> {
        for argument in &args[1..] {
            paid::tick(work, 1)?;
            if mentions_with_work(argument, &needle, work)? {
                return Ok(false);
            }
        }
        Ok(true)
    })();
    match verdict {
        Ok(verdict) => paid::retire(needle, verdict, work),
        Err(error) => Err(paid::abandon(needle, error, work)),
    }
}

fn appends_scan<E>(
    appends: &mut BTreeMap<ExpressionId, ExpressionId>,
    pending: &mut Vec<&ResolvedExpr>,
    work: Work<'_, E>,
    reserve: Reserve<'_, E>,
) -> Result<(), FundedError<E>> {
    loop {
        paid::tick(work, 1)?;
        let Some(expression) = pending.pop() else {
            return Ok(());
        };
        paid::tick(work, 1)?;
        if let ResolvedExprKind::Block { statements, .. }
        | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
        {
            for statement in statements {
                paid::tick(work, 1)?;
                if let ResolvedStatement::Assign {
                    binding,
                    field: None,
                    value,
                    ..
                } = statement
                {
                    if is_same_owner_concat_hir_funded_with_work(value, &binding.id, work, reserve)?
                    {
                        paid::tick(work, 1)?;
                        if let ResolvedExprKind::Call { args, .. } = &value.kind {
                            funded::charge_clone(&value.id, reserve)?;
                            paid::tick(work, paid::clone_units::<ExpressionId, E>(&value.id)?)?;
                            let moved = value.id.clone();
                            // consumes `moved` on every path, paying its Drop on failure
                            paid::insert_cloned_with_work(
                                appends,
                                &args[0].id,
                                moved,
                                work,
                                reserve,
                            )?;
                        }
                    }
                }
            }
        }
        paid::push_trace_children_with_work(expression, pending, work, reserve)?;
    }
}

pub(crate) fn same_owner_concat_appends_funded_with_work<E>(
    function: &ResolvedFunction,
    work: Work<'_, E>,
    reserve: Reserve<'_, E>,
) -> Result<BTreeMap<ExpressionId, ExpressionId>, FundedError<E>> {
    funded::charge(reserve, 0)?;
    paid::tick(work, 2)?;
    let mut appends = BTreeMap::new();
    paid::tick(work, 2)?;
    let mut pending = funded::singleton(&function.body, reserve)?;
    match appends_scan(&mut appends, &mut pending, work, reserve) {
        Ok(()) => paid::retire(pending, appends, work),
        Err(error) => Err(paid::abandon((pending, appends), error, work)),
    }
}

fn operands_fill<E>(
    keys: &mut std::collections::btree_map::IntoKeys<ExpressionId, ExpressionId>,
    operands: &mut BTreeSet<ExpressionId>,
    work: Work<'_, E>,
    reserve: Reserve<'_, E>,
) -> Result<(), FundedError<E>> {
    use paid::DropUnits;
    loop {
        // one iterator step plus the dropped map value it frees
        paid::tick(work, 1 + <ExpressionId as DropUnits>::DROP_UNITS)?;
        let Some(key) = keys.next() else {
            return Ok(());
        };
        let step = funded::reserve_node::<ExpressionId, (), E>(reserve)
            .and_then(|()| paid::search_set(operands, &key, work));
        match step {
            Ok(()) => {
                operands.insert(key);
            }
            Err(error) => return Err(paid::abandon_units(key, 1, error, work)),
        }
    }
}

pub(crate) fn same_owner_concat_operands_funded_with_work<E>(
    function: &ResolvedFunction,
    work: Work<'_, E>,
    reserve: Reserve<'_, E>,
) -> Result<BTreeSet<ExpressionId>, FundedError<E>> {
    let appends = same_owner_concat_appends_funded_with_work(function, work, reserve)?;
    if let Err(error) = paid::tick(work, 1) {
        return Err(paid::abandon(appends, error, work));
    }
    let mut keys = appends.into_keys();
    let mut operands = BTreeSet::new();
    match operands_fill(&mut keys, &mut operands, work, reserve) {
        Ok(()) => paid::retire(keys, operands, work),
        Err(error) => Err(paid::abandon((keys, operands), error, work)),
    }
}
