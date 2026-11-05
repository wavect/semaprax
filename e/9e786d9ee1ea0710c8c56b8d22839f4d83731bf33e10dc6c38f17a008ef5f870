//! Metered primitive renewal reconstruction. The selected entry independently
//! closes Vec/iterator/String-replacement families before using this subset.
use super::*;
mod append;
#[cfg(test)]
mod cells;
use crate::cleanup_plan::ReplayPhysicalReservation;
#[cfg(test)]
use crate::cleanup_plan::replay_btree_node_upper;
#[cfg(test)]
use crate::hir::{ResolvedExprKind, ResolvedStatement, ValueId};
pub(crate) use append::admitted as selected_string_append;
#[cfg(test)]
use std::collections::BTreeSet;
const DEPTH: usize = 514;
#[cfg(test)]
const WORK: usize = crate::cache_codec::MAX_NODES;
#[cfg(test)]
type Frame<'a> = Option<(crate::hir::lexical_exit::ChildCursor<'a>, bool, bool)>;
fn error() -> crate::diagnostic::Diagnostic {
    crate::diagnostic::Diagnostic::io("SPX-H006", "selected primitive renewal census refused")
}
#[cfg(test)]
pub(crate) fn bindings_with_reservation<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    physical: &mut ReplayPhysicalReservation,
) -> Result<RenewalBindings<'a>, crate::diagnostic::Diagnostic> {
    let mut work = WORK;
    bindings_with_reservation_and_work(program, function, physical, &mut |units| {
        work = work.checked_sub(units).ok_or_else(error)?;
        Ok(())
    })
}
#[cfg(test)]
pub(crate) fn bindings_with_reservation_and_work<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    physical: &mut ReplayPhysicalReservation,
    charge: &mut dyn FnMut(usize) -> Result<(), crate::diagnostic::Diagnostic>,
) -> Result<RenewalBindings<'a>, crate::diagnostic::Diagnostic> {
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
                let appended = selected_string_append(binding, value, physical, charge)?;
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
                        || (in_loop && cells::vector(program, binding, value, charge)?))
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
    let helpers = crate::loop_calls::byte_writer::checked_bindings_with_reservation_and_work(
        program, function, physical, charge,
    )?;
    Ok(RenewalBindings {
        legacy,
        appends,
        helpers: HelperSites::Owned(helpers),
    })
}
#[cfg(test)]
fn walk<'a>(
    function: &'a ResolvedFunction,
    frames: &mut [Frame<'a>; DEPTH],
    charge: &mut dyn FnMut(usize) -> Result<(), crate::diagnostic::Diagnostic>,
    mut statement: impl FnMut(
        &'a ResolvedStatement,
        bool,
        &mut dyn FnMut(usize) -> Result<(), crate::diagnostic::Diagnostic>,
    ) -> Result<(), crate::diagnostic::Diagnostic>,
) -> Result<(), crate::diagnostic::Diagnostic> {
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

#[cfg(test)]
fn walk_with_work_for_test<'a>(
    function: &'a ResolvedFunction,
    frames: &mut [Frame<'a>; DEPTH],
    work: &mut usize,
    mut visit: impl FnMut(&'a ResolvedStatement) -> Result<(), crate::diagnostic::Diagnostic>,
) -> Result<(), crate::diagnostic::Diagnostic> {
    walk(
        function,
        frames,
        &mut |units| {
            *work = work.checked_sub(units).ok_or_else(error)?;
            Ok(())
        },
        |item, _, _| visit(item),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_primitive_renewals_match_legacy_and_reserve_before_one_short_growth() {
        let checked = crate::check(
            include_str!("../../../tests/owned_data/byte_helper_renewal.spx"),
            "selected-renewal.spx",
        )
        .unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap();
        let legacy = super::super::bindings(&program, function);
        let mut physical = ReplayPhysicalReservation::new(usize::MAX);
        let selected = bindings_with_reservation(&program, function, &mut physical).unwrap();
        let sites = crate::loop_calls::byte_writer::checked_bindings(&program, function).unwrap();
        assert_eq!(sites.len(), 2);
        for (at, _) in sites {
            assert_eq!(
                selected.get(at).map(|b| &b.id),
                legacy.get(at).map(|b| &b.id)
            );
        }
        let bytes = physical.charged();
        let mut exact = ReplayPhysicalReservation::new(bytes);
        let exact_sites = bindings_with_reservation(&program, function, &mut exact).unwrap();
        assert_eq!(
            exact_sites
                .legacy
                .iter()
                .map(|(at, b)| (at, &b.id))
                .collect::<Vec<_>>(),
            selected
                .legacy
                .iter()
                .map(|(at, b)| (at, &b.id))
                .collect::<Vec<_>>()
        );
        let leaf = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "helper.leaf")
            .unwrap();
        let leaf_legacy = super::super::bindings(&program, leaf);
        let mut leaf_physical = ReplayPhysicalReservation::new(usize::MAX);
        let leaf_selected = bindings_with_reservation(&program, leaf, &mut leaf_physical).unwrap();
        assert_eq!(leaf_selected.legacy.len(), 1);
        assert_eq!(
            leaf_selected
                .legacy
                .iter()
                .map(|(at, b)| (at, &b.id))
                .collect::<Vec<_>>(),
            leaf_legacy
                .legacy
                .iter()
                .map(|(at, b)| (at, &b.id))
                .collect::<Vec<_>>()
        );
        assert_eq!(exact.charged(), bytes);
        let mut short = ReplayPhysicalReservation::new(bytes - 1);
        assert_eq!(
            bindings_with_reservation(&program, function, &mut short)
                .err()
                .unwrap()
                .code,
            "SPX-H006"
        );
        assert!(short.reserve(0).is_err());
    }
    #[test]
    fn cursor_walk_retains_statement_order_and_exact_linear_work_debits() {
        let mut source = String::from("module app; @id(\"app.main\") fn main()->i64 {");
        for index in 0..32 {
            source.push_str(&format!("let value_{index}={index};"));
        }
        source.push_str("if true {let inside=4; inside} else {let later=5;later}}");
        let parsed = crate::parse(&source, "renewal-cursor-order.spx").unwrap();
        let program = crate::hir::resolve(&parsed).unwrap();
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap();
        let mut reference = Vec::new();
        let mut pending = vec![(&function.body, usize::MAX)];
        let mut expected_work = WORK;
        while let Some((expression, next)) = pending.pop() {
            expected_work -= 1;
            if next == usize::MAX {
                if let ResolvedExprKind::Block { statements, .. }
                | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
                {
                    expected_work -= statements.len();
                    reference.extend(statements.iter().map(|statement| statement as *const _));
                }
                pending.push((expression, 0));
                continue;
            }
            if matches!(expression.kind, ResolvedExprKind::Closure { .. }) {
                continue;
            }
            // Indexed lookup is an independent order oracle here. The
            // saved-cursor contract charges this advance, not its old prefix.
            if let Some(child) = crate::hir::lexical_exit::child(expression, next) {
                pending.push((expression, next + 1));
                pending.push((child, usize::MAX));
            }
        }
        let mut frames = [None; DEPTH];
        let mut actual = Vec::new();
        let mut work = WORK;
        walk_with_work_for_test(function, &mut frames, &mut work, |statement| {
            actual.push(statement as *const _);
            Ok(())
        })
        .unwrap();
        assert_eq!(actual, reference);
        assert_eq!(work, expected_work);
        // 41 nodes, 40 edges, 34 statements: entries+terminal advances,
        // edges and one complete statement census = 2*41+40+34.
        assert_eq!(WORK - work, 156);
        let mut short_work = WORK - expected_work - 1;
        assert_eq!(
            walk_with_work_for_test(function, &mut frames, &mut short_work, |_| Ok(()))
                .unwrap_err()
                .code,
            "SPX-H006"
        );
    }
    #[test]
    fn cursor_walk_keeps_exact_fixed_depth_boundary_and_refuses_one_more() {
        let parsed = crate::parse(
            "module app; @id(\"app.main\") fn main()->i64 {0}",
            "renewal-cursor-depth.spx",
        )
        .unwrap();
        let program = crate::hir::resolve(&parsed).unwrap();
        let mut function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap()
            .clone();
        let ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
            panic!("block")
        };
        let leaf = tail.as_ref().clone();
        let mut nested = leaf.clone();
        for _ in 1..DEPTH {
            let mut parent = leaf.clone();
            parent.kind = ResolvedExprKind::Block {
                statements: Vec::new(),
                tail: Box::new(nested),
            };
            nested = parent;
        }
        function.body = nested;
        let mut work = WORK;
        {
            let mut frames = [None; DEPTH];
            walk_with_work_for_test(&function, &mut frames, &mut work, |_| Ok(())).unwrap();
        }
        let mut parent = leaf;
        parent.kind = ResolvedExprKind::Block {
            statements: Vec::new(),
            tail: Box::new(function.body),
        };
        function.body = parent;
        let mut frames = [None; DEPTH];
        assert_eq!(
            walk_with_work_for_test(&function, &mut frames, &mut work, |_| Ok(()))
                .unwrap_err()
                .code,
            "SPX-H006"
        );
    }
}

#[cfg(test)]
mod cursor_work_tests {
    use super::*;
    #[test]
    fn both_renewal_passes_share_exact_linear_work_and_visit_every_statement() {
        for width in [0, 1, 64, 512] {
            let statements = (0..width)
                .map(|i| format!("let n{i}=0;"))
                .collect::<String>();
            let source = format!("module app; @id(\"app.main\") fn main()->i64 {{{statements}0}}");
            let program =
                crate::hir::resolve(&crate::parse(&source, "linear-renewals.spx").unwrap())
                    .unwrap();
            let function = &program.functions[0];
            let mut frames = [None; DEPTH];
            let mut work = 2 * (4 * width + 5);
            let mut visits = 0;
            for _ in 0..2 {
                walk_with_work_for_test(function, &mut frames, &mut work, |_| {
                    visits += 1;
                    Ok(())
                })
                .unwrap();
            }
            assert_eq!(work, 0);
            assert_eq!(visits, 2 * width);
            let mut short = 2 * (4 * width + 5) - 1;
            walk_with_work_for_test(function, &mut frames, &mut short, |_| Ok(())).unwrap();
            assert_eq!(
                walk_with_work_for_test(function, &mut frames, &mut short, |_| Ok(()))
                    .unwrap_err()
                    .code,
                "SPX-H006"
            );
        }
    }
}
