//! Consuming iterator evaluation with read/validation before the owner commit.
use super::*;
#[derive(Debug, PartialEq)]
pub(super) struct IteratorValue {
    pub(super) vector: Arc<owned_vec::OwnedVecValue>,
    pub(super) cursor: usize,
}
impl Evaluator<'_> {
    pub(super) fn evaluate_iterator_op(
        &mut self,
        op: crate::iterator_ops::IteratorOp,
        type_arguments: &[ResolvedType],
        args: &[ResolvedExpr],
        environment: &mut Environment,
        depth: usize,
    ) -> Result<Value, Flow> {
        use crate::iterator_ops::{self, IteratorOp};
        self.charge()?;
        let ([element], [argument]) = (type_arguments, args) else {
            return Err(Flow::Guard("invalid iterator operation shape"));
        };
        if !iterator_ops::resolved_element_is_admitted(element) {
            return Err(Flow::Guard("invalid iterator scalar element"));
        }
        // A place remains owned by its caller until all fallible reads finish.
        let staged = if let ResolvedExprKind::Place(place) = &argument.kind {
            self.charge()?;
            self.lookup_place(environment, place)?
                .ok_or(Flow::Guard("iterator caller owner unavailable"))?
        } else {
            self.evaluate(argument, environment, depth)?
        };
        let item = match (op, &staged) {
            (IteratorOp::VecIntoIter, Value::Vec(vector))
                if &vector.element == element && vector.values.len() <= vector.capacity =>
            {
                None
            }
            (IteratorOp::Next, Value::Iter(iterator))
                if &iterator.vector.element == element
                    && iterator.cursor <= iterator.vector.values.len() =>
            {
                self.charge()?;
                iterator
                    .vector
                    .values
                    .get(iterator.cursor)
                    .map(|value| self.clone_value(value))
                    .transpose()?
            }
            _ => return Err(Flow::Guard("iterator carrier type or cursor is invalid")),
        };
        let owned = if let ResolvedExprKind::Place(place) = &argument.kind {
            drop(staged);
            take_owned_place(environment, place)
                .ok_or(Flow::Guard("iterator commit owner unavailable"))?
        } else {
            staged
        };
        match (op, owned) {
            (IteratorOp::VecIntoIter, Value::Vec(vector)) => {
                Ok(Value::Iter(Arc::new(IteratorValue { vector, cursor: 0 })))
            }
            (IteratorOp::Next, Value::Iter(iterator)) => {
                let mut fields = BTreeMap::new();
                let case = if let Some(item) = item {
                    fields.insert(hir::DeclarationId::new(iterator_ops::ITEM_ID), item);
                    fields.insert(
                        hir::DeclarationId::new(iterator_ops::REST_ID),
                        Value::Iter(Arc::new(IteratorValue {
                            vector: Arc::clone(&iterator.vector),
                            cursor: iterator.cursor + 1,
                        })),
                    );
                    iterator_ops::YIELD_ID
                } else {
                    iterator_ops::DONE_ID
                };
                drop(iterator);
                Ok(Value::Variant(Arc::new(OwnedVariantValue {
                    ty: iterator_ops::resolved_iter_step(element.clone()),
                    variant: hir::DeclarationId::new(iterator_ops::STEP_ID),
                    case: hir::DeclarationId::new(case),
                    fields,
                })))
            }
            _ => Err(Flow::Guard("iterator commit carrier changed")),
        }
    }
}
