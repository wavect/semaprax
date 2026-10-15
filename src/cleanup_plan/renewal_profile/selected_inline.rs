//! Complete selected renewal reconstruction with finite inline refusals.
//! The old Diagnostic receiver remains independent in selected.rs.
use super::child_refusal::Refusal;
use super::{HelperSites, RenewalBindings};
use crate::cleanup_plan::{ReplayPhysicalReservation, replay_btree_node_upper};
use crate::hir::{
    ExpressionId, ResolvedBinding, ResolvedExprKind, ResolvedFunction, ResolvedStatement, ValueId,
};
use std::collections::{BTreeMap, BTreeSet};
mod append;
mod cells;
const DEPTH: usize = 514;
type Frame<'a> = Option<(crate::hir::lexical_exit::ChildCursor<'a>, bool, bool)>;
fn error() -> Refusal {
    Refusal::RenewalCensus
}
/// Complete current receiving/refusal controls, not a former-header delta.
fn prepay<T>(
    physical: &mut ReplayPhysicalReservation,
    charge: &mut dyn FnMut(usize) -> Result<(), Refusal>,
) -> Result<(), Refusal> {
    physical
        .reserve(std::mem::size_of::<(
            Refusal,
            Result<T, Refusal>,
            Result<(), Refusal>,
            Result<bool, Refusal>,
            Option<Refusal>,
            BTreeSet<ValueId>,
            BTreeMap<ExpressionId, ExpressionId>,
            BTreeMap<ExpressionId, &ResolvedBinding>,
        )>())
        .map_err(|_| error())?;
    charge(8)
}
pub(crate) fn bindings_with_reservation_and_work_inline<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    physical: &mut ReplayPhysicalReservation,
    charge: &mut dyn FnMut(usize) -> Result<(), Refusal>,
) -> Result<RenewalBindings<'a>, Refusal> {
    prepay::<RenewalBindings<'a>>(physical, charge)?;
    physical
        .reserve(std::mem::size_of::<[Frame<'_>; DEPTH]>())
        .map_err(|_| error())?;
    let mut frames = [None; DEPTH];
    let mut mutable = BTreeSet::new();
    walk(
        function,
        &mut frames,
        charge,
        |statement, _in_loop, charge| {
            if let ResolvedStatement::Let {
                binding,
                mutable: true,
                ..
            } = statement
            {
                charge(
                    (mutable.len() + 1)
                        .checked_mul(binding.id.as_str().len() + 1)
                        .and_then(|n| n.checked_mul(2))
                        .ok_or_else(error)?,
                )?;
                if !mutable.contains(&binding.id) {
                    physical
                        .reserve(replay_btree_node_upper::<ValueId, ()>().ok_or_else(error)?)
                        .map_err(|_| error())?;
                }
                mutable.insert(binding.id.clone());
            }
            Ok(())
        },
    )?;
    let mut legacy = BTreeMap::new();
    let mut appends = BTreeMap::new();
    walk(
        function,
        &mut frames,
        charge,
        |statement, in_loop, charge| {
            if let ResolvedStatement::Assign {
                binding,
                field: None,
                value,
                ..
            } = statement
            {
                charge(
                    (mutable.len() + legacy.len() + 2)
                        .checked_mul(binding.id.as_str().len() + value.id.as_str().len() + 2)
                        .and_then(|n| n.checked_mul(2))
                        .ok_or_else(error)?,
                )?;
                let appended = append::admitted(binding, value, physical, charge)?;
                if appended {
                    if !mutable.contains(&binding.id) {
                        return Err(error());
                    }
                    let ResolvedExprKind::Call { args, .. } = &value.kind else {
                        unreachable!()
                    };
                    charge(
                        (appends.len() + 1)
                            .checked_mul(args[0].id.as_str().len() + 1)
                            .and_then(|n| n.checked_mul(2))
                            .ok_or_else(error)?,
                    )?;
                    if !appends.contains_key(&args[0].id) {
                        physical
                            .reserve(
                                replay_btree_node_upper::<ExpressionId, ExpressionId>()
                                    .ok_or_else(error)?,
                            )
                            .map_err(|_| error())?;
                    }
                    appends.insert(args[0].id.clone(), value.id.clone());
                }
                if mutable.contains(&binding.id)
                    && (crate::byte_ops::is_same_owner_set_hir(value, &binding.id)
                        || (crate::string_ops::replacement::admitted(binding, value) && !appended)
                        || (in_loop && {
                            prepay::<bool>(physical, charge)?;
                            cells::vector(program, binding, value, charge)?
                        }))
                {
                    if !legacy.contains_key(&value.id) {
                        physical
                            .reserve(
                                replay_btree_node_upper::<ExpressionId, &ResolvedBinding>()
                                    .ok_or_else(error)?,
                            )
                            .map_err(|_| error())?;
                    }
                    legacy.insert(value.id.clone(), binding);
                }
            }
            Ok(())
        },
    )?;
    let helpers =
        crate::loop_calls::byte_writer::checked_bindings_with_reservation_and_work_inline(
            program, function, physical, charge,
        )?;
    Ok(RenewalBindings {
        legacy,
        appends,
        helpers: HelperSites::Owned(helpers),
    })
}
fn walk<'a>(
    function: &'a ResolvedFunction,
    frames: &mut [Frame<'a>; DEPTH],
    charge: &mut dyn FnMut(usize) -> Result<(), Refusal>,
    mut statement: impl FnMut(
        &'a ResolvedStatement,
        bool,
        &mut dyn FnMut(usize) -> Result<(), Refusal>,
    ) -> Result<(), Refusal>,
) -> Result<(), Refusal> {
    frames.fill(None);
    frames[0] = Some((
        crate::hir::lexical_exit::ChildCursor::new(&function.body),
        false,
        false,
    ));
    let mut depth = 1;
    while depth != 0 {
        charge(1)?;
        let (cursor, entered, in_loop) = frames[depth - 1].ok_or_else(error)?;
        let expression = cursor.expression();
        if !entered {
            if let ResolvedExprKind::Block { statements, .. }
            | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
            {
                charge(statements.len())?;
                for item in statements {
                    statement(item, in_loop, charge)?;
                }
            }
            frames[depth - 1] = Some((cursor, true, in_loop));
            continue;
        }
        if matches!(expression.kind, ResolvedExprKind::Closure { .. }) {
            depth -= 1;
            continue;
        }
        // Block boundaries are prepaid by the entry statement census;
        // the saved cursor never restarts a statement or match prefix.
        let mut cursor = cursor;
        if let Some(child) = cursor.next() {
            if depth == DEPTH {
                return Err(error());
            }
            frames[depth - 1] = Some((cursor, true, in_loop));
            let mut child_loop = in_loop;
            if let Some((
                ResolvedStatement::While {
                    condition, body, ..
                }
                | ResolvedStatement::LexicalWhile {
                    condition, body, ..
                },
                _,
            )) = cursor.last_statement_child()
            {
                if std::ptr::eq(condition.as_ref(), child) {
                    child_loop = false;
                }
                if std::ptr::eq(body.as_ref(), child) {
                    child_loop = crate::hir::iterator_loop::recognize(condition, body).is_none();
                }
            }
            frames[depth] = Some((
                crate::hir::lexical_exit::ChildCursor::new(child),
                false,
                child_loop,
            ));
            depth += 1;
        } else {
            depth -= 1;
        }
    }
    Ok(())
}
