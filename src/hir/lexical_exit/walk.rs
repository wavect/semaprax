//! Open, authored-order, fixed-depth traversal shared by lexical proof consumers.
use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedExpr, ResolvedExprKind as K, ResolvedFunction};

/// Borrowed authored-order child enumeration. The cursor has no heap backing.
/// Block statements and match arms are visited once rather than rescanned for
/// each flattened child index. Consumers retain their own depth/work bounds.
#[derive(Clone, Copy)]
pub(crate) struct ChildCursor<'a> {
    expression: &'a ResolvedExpr,
    position: Position,
    started: bool,
}
#[derive(Clone, Copy)]
enum Position {
    Indexed(usize),
    Block { statement: usize, child: usize },
    Match { arm: usize, guard: bool },
}
impl<'a> ChildCursor<'a> {
    pub(crate) fn new(expression: &'a ResolvedExpr) -> Self {
        let position = match &expression.kind {
            K::Block { .. } | K::VecConstructor { .. } => Position::Block {
                statement: 0,
                child: 0,
            },
            K::Match { .. } => Position::Match {
                arm: 0,
                guard: true,
            },
            _ => Position::Indexed(0),
        };
        Self {
            expression,
            position,
            started: false,
        }
    }
    pub(crate) fn expression(&self) -> &'a ResolvedExpr {
        self.expression
    }
    /// Statement edge most recently returned by `next`; tails have no statement.
    /// This uses the saved position and never rescans a block prefix.
    pub(crate) fn last_statement_child(
        &self,
    ) -> Option<(&'a crate::hir::ResolvedStatement, usize)> {
        match (&self.expression.kind, self.position) {
            (
                K::Block { statements, .. } | K::VecConstructor { statements, .. },
                Position::Block { statement, child },
            ) if self.started && child != 0 => {
                statements.get(statement).map(|item| (item, child - 1))
            }
            _ => None,
        }
    }
    pub(crate) fn is_started(&self) -> bool {
        self.started
    }
}
impl<'a> Iterator for ChildCursor<'a> {
    type Item = &'a ResolvedExpr;
    fn next(&mut self) -> Option<Self::Item> {
        let first = !self.started;
        self.started = true;
        match (&self.expression.kind, &mut self.position) {
            (
                K::Block { statements, tail }
                | K::VecConstructor {
                    statements, tail, ..
                },
                Position::Block {
                    statement,
                    child: index,
                },
            ) => {
                while let Some(item) = statements.get(*statement) {
                    if *index < item.child_count() {
                        let result = item.child(*index);
                        *index += 1;
                        return result;
                    }
                    *statement += 1;
                    *index = 0;
                }
                if *index == 0 {
                    *index = 1;
                    Some(tail)
                } else {
                    None
                }
            }
            (
                K::Match {
                    scrutinee, arms, ..
                },
                Position::Match { arm, guard },
            ) => {
                if first {
                    return Some(scrutinee);
                }
                if let Some(item) = arms.get(*arm) {
                    if *guard {
                        *guard = false;
                        if let Some(value) = &item.guard {
                            return Some(value);
                        }
                    }
                    *arm += 1;
                    *guard = true;
                    return Some(&item.value);
                }
                None
            }
            (_, Position::Indexed(index)) => {
                let result = child(self.expression, *index);
                if result.is_some() {
                    *index += 1;
                }
                result
            }
            _ => unreachable!("child cursor position matches its immutable expression"),
        }
    }
}
impl std::iter::FusedIterator for ChildCursor<'_> {}

pub(crate) fn child(expr: &ResolvedExpr, index: usize) -> Option<&ResolvedExpr> {
    match &expr.kind {
        K::LexicalExit { value, .. } => (index == 0).then_some(value.as_deref()).flatten(),
        K::Closure { captures, body, .. } => captures
            .get(index)
            .map(|capture| &capture.value)
            .or_else(|| (index == captures.len()).then_some(body.as_ref())),
        K::Block { statements, tail }
        | K::VecConstructor {
            statements, tail, ..
        } => {
            let mut offset = 0;
            for statement in statements {
                let count = statement.child_count();
                if index < offset + count {
                    return statement.child(index - offset);
                }
                offset += count;
            }
            (index == offset).then_some(tail.as_ref())
        }
        K::Invoke { callable, args } => {
            if index == 0 {
                Some(callable)
            } else {
                args.get(index - 1)
            }
        }
        K::Call { args, .. }
        | K::LiteralFormat { args, .. }
        | K::VecFieldRead { args, .. }
        | K::RowLoan { args, .. } => args.get(index),
        K::NativeRustImportCall(call) => call.args.get(index),
        K::HostCommandCall(call) => call.args.get(index),
        K::ByteRange {
            source, start, end, ..
        } => [source.as_ref(), start.as_ref(), end.as_ref()]
            .get(index)
            .copied(),
        K::Unary { value, .. }
        | K::Try { operand: value, .. }
        | K::TryOption { operand: value, .. }
        | K::Project { base: value, .. }
        | K::Upcast { source: value }
        | K::Yield { request: value } => (index == 0).then_some(value),
        K::Binary { left, right, .. } => [left.as_ref(), right.as_ref()].get(index).copied(),
        K::If {
            condition,
            then_branch,
            else_branch,
        } => [
            condition.as_ref(),
            then_branch.as_ref(),
            else_branch.as_ref(),
        ]
        .get(index)
        .copied(),
        K::ConstructRecord { fields, .. } | K::ConstructVariant { fields, .. } => {
            fields.get(index).map(|field| &field.value)
        }
        K::Match {
            scrutinee, arms, ..
        } => {
            if index == 0 {
                return Some(scrutinee);
            }
            let mut offset = 1;
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    if index == offset {
                        return Some(guard);
                    }
                    offset += 1;
                }
                if index == offset {
                    return Some(&arm.value);
                }
                offset += 1;
            }
            None
        }
        K::UpdateRecord { base, fields, .. } => {
            if index == 0 {
                Some(base)
            } else {
                fields.get(index - 1).map(|field| &field.value)
            }
        }
        K::FunctionReference { .. }
        | K::Int(_)
        | K::Int32(_)
        | K::Char(_)
        | K::Uint8(_)
        | K::Usize(_)
        | K::ArrayU8(_)
        | K::RepeatArrayU8 { .. }
        | K::Float32(_)
        | K::Float64(_)
        | K::Bool(_)
        | K::String(_)
        | K::BorrowedUtf8Literal(_)
        | K::Place(_)
        | K::BorrowPlace { .. } => None,
    }
}

// Ordinary expression admission has a 512-node boundary, plus its root cursor.
// Lexical contours retain their narrower cache-depth contract only when an
// actual exit selects it, matching the independent authored-source verifier.
pub(super) const PRESENCE_DEPTH: usize = 513;

/// Finite depth-first scan: no frontier allocation, identity clones or recursion.
/// Callers charge this one source-defined function walk in their existing work census.
pub(crate) fn requires(function: &ResolvedFunction) -> Result<bool, Diagnostic> {
    scan(function, false)
}

pub(super) fn requires_validation(function: &ResolvedFunction) -> Result<bool, Diagnostic> {
    scan(function, true)
}

fn scan(function: &ResolvedFunction, include_markers: bool) -> Result<bool, Diagnostic> {
    let mut stack = [None; PRESENCE_DEPTH];
    for root in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        stack[0] = Some(ChildCursor::new(root));
        let mut len = 1usize;
        while len != 0 {
            let frame = stack[len - 1]
                .as_mut()
                .expect("occupied lexical scan frame");
            let expr = frame.expression();
            if include_markers
                && !frame.is_started()
                && (expr.ty == crate::hir::ResolvedType::Never
                    || matches!(&expr.kind, K::Block { statements, .. } | K::VecConstructor { statements, .. }
                        if statements.iter().any(|s| matches!(s, crate::hir::ResolvedStatement::LexicalWhile { .. }))))
            {
                return Ok(true);
            }
            if !frame.is_started() && matches!(&expr.kind, K::LexicalExit { .. }) {
                if len > crate::cache_codec::MAX_DEPTH + 1 {
                    return Err(Diagnostic::io(
                        "SPX-H006",
                        "lexical control scan exceeds admitted expression depth",
                    ));
                }
                return Ok(true);
            }
            if let Some(next) = frame.next() {
                if len == stack.len() {
                    return Err(Diagnostic::io(
                        "SPX-H006",
                        "lexical control scan exceeds admitted expression depth",
                    ));
                }
                stack[len] = Some(ChildCursor::new(next));
                len += 1;
            } else {
                stack[len - 1] = None;
                len -= 1;
            }
        }
    }
    Ok(false)
}

/// Admitted resolver output already obeys the HIR depth bound.
pub(crate) fn contains(root: &ResolvedExpr) -> bool {
    let mut stack = [None; PRESENCE_DEPTH];
    stack[0] = Some(ChildCursor::new(root));
    let mut len = 1;
    while len != 0 {
        let frame = stack[len - 1].as_mut().expect("occupied lexical frame");
        let expr = frame.expression();
        if !frame.is_started() && matches!(expr.kind, K::LexicalExit { .. }) {
            return true;
        }
        if let Some(next) = frame.next() {
            if len == stack.len() {
                return true;
            } // fail closed: independent HIR depth replay rejects it
            stack[len] = Some(ChildCursor::new(next));
            len += 1;
        } else {
            len -= 1;
            stack[len] = None;
        }
    }
    false
}

#[cfg(test)]
mod tests;

impl<'a> ChildCursor<'a> {
    #[cfg(test)]
    pub(crate) fn next_with_work<E>(
        &mut self,
        work: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Option<&'a ResolvedExpr>, E> {
        work(1)?;
        let first = !self.started;
        self.started = true;
        Ok(match (&self.expression.kind, &mut self.position) {
            (
                K::Block { statements, tail }
                | K::VecConstructor {
                    statements, tail, ..
                },
                Position::Block {
                    statement,
                    child: index,
                },
            ) => {
                loop {
                    work(1)?;
                    let Some(item) = statements.get(*statement) else {
                        break;
                    };
                    if *index < item.child_count() {
                        let result = item.child(*index);
                        *index += 1;
                        return Ok(result);
                    }
                    *statement += 1;
                    *index = 0;
                }
                if *index == 0 {
                    *index = 1;
                    Some(tail)
                } else {
                    None
                }
            }
            (
                K::Match {
                    scrutinee, arms, ..
                },
                Position::Match { arm, guard },
            ) => {
                if first {
                    return Ok(Some(scrutinee));
                }
                if let Some(item) = arms.get(*arm) {
                    if *guard {
                        *guard = false;
                        if let Some(value) = &item.guard {
                            return Ok(Some(value));
                        }
                    }
                    *arm += 1;
                    *guard = true;
                    return Ok(Some(&item.value));
                }
                None
            }
            (_, Position::Indexed(index)) => {
                let result = child(self.expression, *index);
                if result.is_some() {
                    *index += 1;
                }
                result
            }
            _ => unreachable!("child cursor position matches its immutable expression"),
        })
    }
}
