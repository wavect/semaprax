//! Reuse one funded authored-order stack across the three presence selectors.
use super::paid_type_facts::Payer;
use super::{Diagnostic, ResolvedExpr, ResolvedExprKind, ResolvedFunction};
use crate::hir::{self, DeclarationIndex, ResolvedStatement, ResolvedType, TypeFactsPayer};
use std::mem::size_of;

enum Test<'a> {
    Owned,
    Record(&'a DeclarationIndex),
    Loop,
}

impl Test<'_> {
    fn ty(&self, ty: &ResolvedType) -> bool {
        match self {
            Self::Owned => crate::iterator_ops::resolved_type_uses_owned_iterator(ty),
            Self::Record(declarations) => crate::iterator_ops::element(ty).is_some_and(|element| {
                hir::owned_record_collection::is_admitted_owned_record_collection_element(
                    declarations,
                    element,
                )
            }),
            Self::Loop => false,
        }
    }

    fn expression(&self, expression: &ResolvedExpr) -> bool {
        if !matches!(self, Self::Loop) {
            return self.ty(&expression.ty);
        }
        let (ResolvedExprKind::Block { statements, .. }
        | ResolvedExprKind::VecConstructor { statements, .. }) = &expression.kind
        else {
            return false;
        };
        statements.iter().any(|statement| {
            matches!(statement,
                ResolvedStatement::While { condition, body, .. }
                | ResolvedStatement::LexicalWhile { condition, body, .. }
                if hir::iterator_loop::recognize(condition, body).is_some())
        })
    }
}

pub(super) struct Walk<'a, 'p, 'scope> {
    function: &'a ResolvedFunction,
    pending: Vec<&'a ResolvedExpr>,
    pub(super) payer: &'p mut Payer<'scope>,
}

impl<'a, 'p, 'scope> Walk<'a, 'p, 'scope> {
    pub(super) fn new(
        function: &'a ResolvedFunction,
        payer: &'p mut Payer<'scope>,
    ) -> Result<Self, Diagnostic> {
        // Caller/formal inputs, local vector header, return and caller receipt.
        payer.for_subject(function)?;
        payer.bytes(size_of::<(
            [(&ResolvedFunction, &mut Payer<'_>); 2],
            Vec<&ResolvedExpr>,
            [Result<Self, Diagnostic>; 2],
        )>())?;
        Ok(Self {
            function,
            pending: Vec::new(),
            payer,
        })
    }

    pub(super) fn uses_owned_iterator(&mut self) -> Result<bool, Diagnostic> {
        self.scan(Test::Owned)
    }

    pub(super) fn uses_record_iterator(
        &mut self,
        declarations: &DeclarationIndex,
    ) -> Result<bool, Diagnostic> {
        self.scan(Test::Record(declarations))
    }

    pub(super) fn contains_iterator_loop(&mut self) -> Result<bool, Diagnostic> {
        self.scan(Test::Loop)
    }

    fn scan(&mut self, test: Test<'_>) -> Result<bool, Diagnostic> {
        #[cfg(test)]
        let _phase = crate::test_allocation_observer::function_phase_guard(match test {
            Test::Owned => "schema-owned-iterator",
            Test::Record(_) => "schema-record-iterator",
            Test::Loop => "schema-iterator-loop",
        });
        let payer = &mut *self.payer;
        let function = self.function;
        // Pay the selector handoff, signature iterator, root extent, and both
        // returning receivers before reading the roots or moving the stack.
        payer.for_subject(function)?;
        payer.bytes(size_of::<(
            [(&mut Self, Test<'_>); 2],
            std::slice::Iter<'_, hir::ResolvedParam>,
            [usize; 4],
            [Result<bool, Diagnostic>; 4],
        )>())?;
        self.pending.clear();
        if matches!(test, Test::Loop) {
            payer.grow(&mut self.pending, 1)?;
            self.pending.push(&function.body);
        } else {
            if test.ty(&function.return_type) || function.params.iter().any(|p| test.ty(&p.ty)) {
                return Ok(true);
            }
            let roots = function
                .requires
                .len()
                .checked_add(function.ensures.len())
                .and_then(|count| count.checked_add(1))
                .ok_or_else(|| payer.construction_refusal("iterator selector roots overflowed"))?;
            payer.grow(&mut self.pending, roots)?;
            self.pending.extend(&function.requires);
            self.pending.push(&function.body);
            self.pending.extend(&function.ensures);
        }

        // Reserve before taking the next owned stack slot, including exhaustion.
        loop {
            payer.bytes(size_of::<(
                [&mut Vec<&ResolvedExpr>; 2],
                [Option<&ResolvedExpr>; 2],
                [&ResolvedExpr; 3],
                &Test<'_>,
                std::slice::Iter<'_, ResolvedStatement>,
                [bool; 3],
                [usize; 2],
                [Result<(), Diagnostic>; 2],
            )>())?;
            let Some(expression) = self.pending.pop() else {
                return Ok(false);
            };
            if test.expression(expression) {
                return Ok(true);
            }
            payer.grow(&mut self.pending, hir::funded::child_count(expression))?;
            hir::push_resolved_expression_children_in_authored_order(expression, &mut self.pending);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bounded_output::{set_active_floor, with_limit_usage_refusal};
    use crate::cleanup_plan::replay::ReplayBudget;

    #[test]
    fn reuses_the_exact_authored_walk_and_preserves_each_presence_result() {
        for program in crate::cleanup_plan::replay::paid_type_facts::tests::fixtures() {
            for original in &program.functions {
                for owned_result in [false, true] {
                    let mut function = original.clone();
                    if owned_result {
                        function.return_type =
                            crate::iterator_ops::resolved_iter(ResolvedType::Bytes);
                    }
                    let expected = [
                        crate::iterator_ops::function_uses_owned_iterator(&function),
                        crate::iterator_ops::function_uses_record_iterator_in(
                            &program.declarations,
                            &function,
                        ),
                        hir::iterator_loop::function_contains(&function),
                    ];
                    let invoke = |limit, floor| {
                        with_limit_usage_refusal(limit, || -> Result<_, Diagnostic> {
                            let mut budget = ReplayBudget::new();
                            assert!(set_active_floor(floor));
                            let mut payer = Payer::capture(&function, &mut budget)?;
                            let mut walk = Walk::new(&function, &mut payer)?;
                            Ok([
                                walk.uses_owned_iterator()?,
                                walk.uses_record_iterator(&program.declarations)?,
                                walk.contains_iterator_loop()?,
                            ])
                        })
                    };
                    let (actual, refused, paid, first) = invoke(usize::MAX, 0);
                    assert_eq!(actual.unwrap(), expected);
                    assert!(!refused && first.is_none());
                    let (exact, refused, exact_paid, first) = invoke(paid + 17, 17);
                    assert_eq!(exact.unwrap(), expected);
                    assert_eq!(exact_paid, paid);
                    assert!(!refused && first.is_none());
                    let (short, refused, _, first) = invoke(paid + 16, 17);
                    assert!(short.is_err() && refused && first.is_some());
                }
            }
        }
    }
}
