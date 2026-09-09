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
        if *element == ResolvedType::Bytes {
            // A Place staging clone adds exactly one alias to the caller's
            // owner. Reject all other aliases before removing that owner.
            let expected = if matches!(argument.kind, ResolvedExprKind::Place(_)) {
                2
            } else {
                1
            };
            let unique = match &staged {
                Value::Vec(vector) => Arc::strong_count(vector) == expected,
                Value::Iter(iterator) => {
                    Arc::strong_count(iterator) == expected
                        && Arc::strong_count(&iterator.vector) == 1
                }
                _ => false,
            };
            if !unique {
                return Err(Flow::Guard("aliased owning iterator staging"));
            }
        }
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
                if *element == ResolvedType::Bytes {
                    if !iterator.vector.values[..iterator.cursor]
                        .iter()
                        .all(|value| matches!(value, Value::Bool(false)))
                        || !iterator.vector.values[iterator.cursor..]
                            .iter()
                            .all(|value| matches!(value, Value::Bytes(_)))
                    {
                        return Err(Flow::Guard("invalid owned iterator initialized window"));
                    }
                    None
                } else {
                    iterator
                        .vector
                        .values
                        .get(iterator.cursor)
                        .map(|value| self.clone_value(value))
                        .transpose()?
                }
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
            (IteratorOp::Next, Value::Iter(iterator)) if *element == ResolvedType::Bytes => {
                self.finish_owned_iterator_next(iterator)
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

impl Evaluator<'_> {
    fn finish_owned_iterator_next(&mut self, iterator: Arc<IteratorValue>) -> Result<Value, Flow> {
        let iterator =
            Arc::try_unwrap(iterator).map_err(|_| Flow::Guard("aliased owned iterator"))?;
        let mut vector = Arc::try_unwrap(iterator.vector)
            .map_err(|_| Flow::Guard("aliased owned iterator backing"))?;
        let mut fields = BTreeMap::new();
        let case = if iterator.cursor < vector.values.len() {
            let item = std::mem::replace(&mut vector.values[iterator.cursor], Value::Bool(false));
            fields.insert(hir::DeclarationId::new(crate::iterator_ops::ITEM_ID), item);
            fields.insert(
                hir::DeclarationId::new(crate::iterator_ops::REST_ID),
                Value::Iter(Arc::new(IteratorValue {
                    vector: Arc::new(vector),
                    cursor: iterator.cursor + 1,
                })),
            );
            crate::iterator_ops::YIELD_ID
        } else {
            crate::iterator_ops::DONE_ID
        };
        Ok(Value::Variant(Arc::new(OwnedVariantValue {
            ty: crate::iterator_ops::resolved_iter_step(ResolvedType::Bytes),
            variant: hir::DeclarationId::new(crate::iterator_ops::STEP_ID),
            case: hir::DeclarationId::new(case),
            fields,
        })))
    }
}
