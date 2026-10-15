//! Replay fixed slot-call identities from the canonical authored syntax.
use crate::workspace_graph::{graph_error, reserve_builder_structure};
use crate::{
    ast::{ExprKind, Program},
    diagnostic::Diagnostic,
    hir,
};
fn fail() -> Vec<Diagnostic> {
    vec![graph_error(
        "SPX-G173",
        "String-slot source identity disagrees with retained HIR",
    )]
}
fn identity(owner: &str, path: &str) -> Result<String, Vec<Diagnostic>> {
    let value = hir::budgeted_workspace_expression_identity_text(owner, path);
    if crate::bounded_output::active_refused() {
        return Err(fail());
    }
    Ok(value)
}
pub(super) fn verify(
    source: &Program,
    resolved: &hir::ResolvedProgram,
) -> Result<(), Vec<Diagnostic>> {
    let source_uses = source.functions.iter().any(|function| {
        function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
            .any(contains)
    });
    if !source_uses && !crate::vec_ops::string_slot::resolved_uses(resolved) {
        return Ok(());
    }
    let work = std::cell::Cell::new(0usize);
    let mut expected = Vec::<(String, String, String)>::new();
    let mut fused = Vec::<String>::new();
    for function in &source.functions {
        let owner = function.stable_id.as_str();
        for (root, expression) in function
            .requires
            .iter()
            .enumerate()
            .map(|(i, e)| {
                (
                    crate::bounded_output::budgeted_format(format_args!("requires.{i}")),
                    e,
                )
            })
            .chain(std::iter::once((
                crate::bounded_output::budgeted_format(format_args!("body")),
                &function.body,
            )))
            .chain(function.ensures.iter().enumerate().map(|(i, e)| {
                (
                    crate::bounded_output::budgeted_format(format_args!("ensures.{i}")),
                    e,
                )
            }))
        {
            super::super::source_callables::visit_ast_call_expressions(
                expression,
                &root,
                &mut |expression, path| {
                    debit(&work, 1)?;
                    let ExprKind::Call {
                        name,
                        type_arguments,
                        args,
                    } = &expression.kind
                    else {
                        unreachable!()
                    };
                    let (target, slot) = if name == crate::byte_ops::STR_AS_BYTES_NAME {
                        let [inner] = args.as_slice() else {
                            return Ok(());
                        };
                        let ExprKind::Call { name, .. } = &inner.kind else {
                            return Ok(());
                        };
                        if name != crate::vec_ops::string_slot::NAME {
                            return Ok(());
                        }
                        let inner_path =
                            crate::bounded_output::budgeted_format(format_args!("{path}.arg.0"));
                        let inner_site = identity(owner, &inner_path)?;
                        let at = search(&fused, &inner_site, &work)?.unwrap_or_else(|at| at);
                        grow(&mut fused)?;
                        debit(&work, fused.len() - at)?;
                        fused.insert(at, inner_site);
                        (crate::vec_ops::string_slot::BYTES_ID, inner)
                    } else if name == crate::vec_ops::string_slot::NAME {
                        let site = identity(owner, path)?;
                        if search(&fused, &site, &work)?.is_ok() {
                            return Ok(());
                        }
                        (crate::vec_ops::string_slot::ID, expression)
                    } else {
                        return Ok(());
                    };
                    let ExprKind::Call {
                        type_arguments: slot_types,
                        args: slot_args,
                        ..
                    } = &slot.kind
                    else {
                        return Err(fail());
                    };
                    if !slot_types.is_empty()
                        || slot_args.len() != 2
                        || !matches!(slot_args[0].kind, ExprKind::Var(_))
                        || (target == crate::vec_ops::string_slot::BYTES_ID
                            && !type_arguments.is_empty())
                    {
                        return Err(fail());
                    }
                    grow(&mut expected)?;
                    reserve_builder_structure(function.stable_id.len() + target.len())?;
                    expected.push((
                        function.stable_id.clone(),
                        identity(owner, path)?,
                        target.to_owned(),
                    ));
                    Ok(())
                },
            )?;
        }
    }
    let mut actual = Vec::new();
    hir::visit_workspace_call_sites(resolved, &mut |owner, expression, target| {
        if matches!(
            target.as_str(),
            crate::vec_ops::string_slot::ID | crate::vec_ops::string_slot::BYTES_ID
        ) {
            debit(&work, 1)?;
            grow(&mut actual)?;
            reserve_builder_structure(
                owner.as_str().len() + expression.len() + target.as_str().len(),
            )?;
            actual.push((
                owner.as_str().to_owned(),
                expression.to_owned(),
                target.as_str().to_owned(),
            ));
        }
        Ok::<(), Vec<Diagnostic>>(())
    })?;
    sort(&mut expected, &work)?;
    sort(&mut actual, &work)?;
    for (left, right) in expected.iter().zip(&actual) {
        debit(&work, comparison_bytes(left, right))?;
    }
    if expected.len() != actual.len()
        || !expected
            .iter()
            .zip(&actual)
            .all(|(e, a)| e.0 == a.0 && e.1 == a.1 && e.2 == a.2)
    {
        return Err(fail());
    }
    Ok(())
}

fn contains(root: &crate::ast::Expr) -> bool {
    let mut stack = [None; 514];
    stack[0] = Some((root, 0usize));
    let mut len = 1usize;
    while len != 0 {
        len -= 1;
        let (expression, next) = stack[len].take().expect("slot census frame");
        if next == 0
            && matches!(&expression.kind, ExprKind::Call{name,..} if name == crate::vec_ops::string_slot::NAME)
        {
            return true;
        }
        if let Some(child) = expression.child(next) {
            if len + 2 > stack.len() {
                return true;
            }
            stack[len] = Some((expression, next + 1));
            stack[len + 1] = Some((child, 0));
            len += 2;
        }
    }
    false
}

// Charge each full replacement backing before allocation; old backing is never refunded.
fn grow<T>(values: &mut Vec<T>) -> Result<(), Vec<Diagnostic>> {
    if values.len() < values.capacity() {
        return Ok(());
    }
    let capacity = values.capacity().saturating_mul(2).max(4);
    let bytes = capacity
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(fail)?;
    reserve_builder_structure(bytes)?;
    values
        .try_reserve_exact(capacity - values.len())
        .map_err(|_| fail())?;
    if values.capacity() > capacity {
        return Err(fail());
    }
    Ok(())
}

fn debit(work: &std::cell::Cell<usize>, amount: usize) -> Result<(), Vec<Diagnostic>> {
    let used = work.get().checked_add(amount).ok_or_else(fail)?;
    if used > 1_000_000 {
        return Err(fail());
    }
    work.set(used);
    Ok(())
}
fn search(
    values: &[String],
    key: &str,
    work: &std::cell::Cell<usize>,
) -> Result<Result<usize, usize>, Vec<Diagnostic>> {
    let (mut left, mut right) = (0, values.len());
    while left < right {
        let middle = left + (right - left) / 2;
        debit(
            work,
            values[middle]
                .len()
                .checked_add(key.len())
                .ok_or_else(fail)?,
        )?;
        match values[middle].as_str().cmp(key) {
            std::cmp::Ordering::Less => left = middle + 1,
            std::cmp::Ordering::Greater => right = middle,
            std::cmp::Ordering::Equal => return Ok(Ok(middle)),
        }
    }
    Ok(Err(left))
}

type Site = (String, String, String);
fn comparison_bytes(left: &Site, right: &Site) -> usize {
    left.0.len() + left.1.len() + left.2.len() + right.0.len() + right.1.len() + right.2.len()
}
// Fallible in-place heapsort: no merge scratch, and refusal stops comparisons immediately.
fn sort(values: &mut [Site], work: &std::cell::Cell<usize>) -> Result<(), Vec<Diagnostic>> {
    fn sift(
        values: &mut [Site],
        mut root: usize,
        end: usize,
        work: &std::cell::Cell<usize>,
    ) -> Result<(), Vec<Diagnostic>> {
        loop {
            let left = root * 2 + 1;
            if left >= end {
                return Ok(());
            }
            let mut child = left;
            if left + 1 < end {
                debit(work, comparison_bytes(&values[left], &values[left + 1]))?;
                if values[left] < values[left + 1] {
                    child = left + 1;
                }
            }
            debit(work, comparison_bytes(&values[root], &values[child]))?;
            if values[root] >= values[child] {
                return Ok(());
            }
            values.swap(root, child);
            root = child;
        }
    }
    for root in (0..values.len() / 2).rev() {
        sift(values, root, values.len(), work)?;
    }
    for end in (1..values.len()).rev() {
        values.swap(0, end);
        sift(values, 0, end, work)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn backing_growth_reserves_full_replacement_before_allocation() {
        let required = 4 * std::mem::size_of::<String>();
        let ((result, capacity), refused, _) =
            crate::bounded_output::with_limit_usage(required, || {
                let mut values = Vec::<String>::new();
                let result = super::grow(&mut values);
                (result, values.capacity())
            });
        assert!(result.is_ok());
        assert!(!refused);
        assert_eq!(capacity, 4);
        let ((result, capacity), refused, _) =
            crate::bounded_output::with_limit_usage(required - 1, || {
                let mut values = Vec::<String>::new();
                let result = super::grow(&mut values);
                (result, values.capacity())
            });
        assert!(result.is_err());
        assert!(refused);
        assert_eq!(capacity, 0);
        for owner in ["", "é:12", "1234567890"] {
            for path in ["", "body.arg.0", "é:3"] {
                let expected = crate::hir::workspace_expression_identity(
                    &crate::hir::DeclarationId::new(owner),
                    path,
                );
                assert_eq!(super::identity(owner, path).unwrap(), expected);
            }
        }
        let mut sites = vec![
            ("z".into(), "2".into(), "a".into()),
            ("a".into(), "3".into(), "b".into()),
            ("a".into(), "1".into(), "c".into()),
        ];
        super::sort(&mut sites, &std::cell::Cell::new(0)).unwrap();
        assert_eq!(
            sites
                .iter()
                .map(|site| (&*site.0, &*site.1))
                .collect::<Vec<_>>(),
            vec![("a", "1"), ("a", "3"), ("z", "2")]
        );
        let before = sites.clone();
        assert!(super::sort(&mut sites, &std::cell::Cell::new(1_000_000)).is_err());
        assert_eq!(sites, before);
    }
}
