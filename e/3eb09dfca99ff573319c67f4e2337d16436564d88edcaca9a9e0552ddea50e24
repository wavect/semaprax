//! Open, authored-order, fixed-depth traversal shared by lexical proof consumers.
use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedExpr, ResolvedExprKind as K, ResolvedFunction};

/// Borrowed authored-order child enumeration. The cursor has no heap backing.
/// The immutable expression kind supplies the state discriminator. `inner`
/// stores only a statement child (at most two) or a match guard phase, so its
/// maximum value can mark a fresh cursor without limiting any outer index.
#[derive(Clone, Copy)]
pub(crate) struct ChildCursor<'a> {
    expression: &'a ResolvedExpr,
    outer: usize,
    inner: usize,
}
#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Position {
    Indexed(usize),
    Block { statement: usize, child: usize },
    Match { arm: usize, guard: bool },
}
impl<'a> ChildCursor<'a> {
    pub(crate) fn new(expression: &'a ResolvedExpr) -> Self {
        Self {
            expression,
            outer: 0,
            inner: usize::MAX,
        }
    }
    pub(crate) fn expression(&self) -> &'a ResolvedExpr {
        self.expression
    }
    /// Statement edge most recently returned by `next`; tails have no statement.
    /// This uses saved progress and never rescans a block prefix.
    pub(crate) fn last_statement_child(
        &self,
    ) -> Option<(&'a crate::hir::ResolvedStatement, usize)> {
        match &self.expression.kind {
            K::Block { statements, .. } | K::VecConstructor { statements, .. }
                if self.is_started() && self.inner != 0 =>
            {
                statements
                    .get(self.outer)
                    .map(|item| (item, self.inner - 1))
            }
            _ => None,
        }
    }
    pub(crate) fn is_started(&self) -> bool {
        self.inner != usize::MAX
    }
    #[cfg(test)]
    fn position(&self) -> Position {
        match &self.expression.kind {
            K::Block { .. } | K::VecConstructor { .. } => Position::Block {
                statement: self.outer,
                child: if self.is_started() { self.inner } else { 0 },
            },
            K::Match { .. } => Position::Match {
                arm: self.outer,
                guard: !self.is_started() || self.inner == 0,
            },
            _ => Position::Indexed(self.outer),
        }
    }
}
impl<'a> Iterator for ChildCursor<'a> {
    type Item = &'a ResolvedExpr;
    fn next(&mut self) -> Option<Self::Item> {
        let first = !self.is_started();
        if first {
            self.inner = 0;
        }
        match &self.expression.kind {
            K::Block { statements, tail }
            | K::VecConstructor {
                statements, tail, ..
            } => {
                while let Some(item) = statements.get(self.outer) {
                    if self.inner < item.child_count() {
                        let result = item.child(self.inner);
                        self.inner += 1;
                        return result;
                    }
                    self.outer += 1;
                    self.inner = 0;
                }
                if self.inner == 0 {
                    self.inner = 1;
                    Some(tail)
                } else {
                    None
                }
            }
            K::Match {
                scrutinee, arms, ..
            } => {
                if first {
                    return Some(scrutinee);
                }
                if let Some(item) = arms.get(self.outer) {
                    if self.inner == 0 {
                        self.inner = 1;
                        if let Some(value) = &item.guard {
                            return Some(value);
                        }
                    }
                    self.outer += 1;
                    self.inner = 0;
                    return Some(&item.value);
                }
                None
            }
            _ => {
                let result = child(self.expression, self.outer);
                if result.is_some() {
                    self.outer += 1;
                }
                result
            }
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
#[inline(never)]
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
        let first = !self.is_started();
        if first {
            self.inner = 0;
        }
        Ok(match &self.expression.kind {
            K::Block { statements, tail }
            | K::VecConstructor {
                statements, tail, ..
            } => {
                loop {
                    work(1)?;
                    let Some(item) = statements.get(self.outer) else {
                        break;
                    };
                    if self.inner < item.child_count() {
                        let result = item.child(self.inner);
                        self.inner += 1;
                        return Ok(result);
                    }
                    self.outer += 1;
                    self.inner = 0;
                }
                if self.inner == 0 {
                    self.inner = 1;
                    Some(tail)
                } else {
                    None
                }
            }
            K::Match {
                scrutinee, arms, ..
            } => {
                if first {
                    return Ok(Some(scrutinee));
                }
                if let Some(item) = arms.get(self.outer) {
                    if self.inner == 0 {
                        self.inner = 1;
                        if let Some(value) = &item.guard {
                            return Ok(Some(value));
                        }
                    }
                    self.outer += 1;
                    self.inner = 0;
                    return Ok(Some(&item.value));
                }
                None
            }
            _ => {
                let result = child(self.expression, self.outer);
                if result.is_some() {
                    self.outer += 1;
                }
                result
            }
        })
    }
}
