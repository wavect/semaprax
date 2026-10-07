//! Closed typed collection semantics, with copied scalar/String entries.
use super::*;
use crate::map_ops::MapOp;
#[derive(Debug, PartialEq)]
pub(super) struct MapValue {
    pub(super) ty: ResolvedType,
    capacity: usize,
    entries: Vec<(Value, Value)>,
}
fn compare(a: &Value, b: &Value) -> Result<std::cmp::Ordering, Flow> {
    Ok(match (a, b) {
        (Value::String(a), Value::String(b)) => a.as_bytes().cmp(b.as_bytes()),
        (Value::Int(a), Value::Int(b)) => a.cmp(b),
        (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
        _ => return Err(Flow::Guard("invalid typed collection key")),
    })
}
impl MapValue {
    fn find(&self, key: &Value) -> Result<Result<usize, usize>, Flow> {
        let mut lo = 0;
        let mut hi = self.entries.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            match compare(&self.entries[mid].0, key)? {
                std::cmp::Ordering::Equal => return Ok(Ok(mid)),
                std::cmp::Ordering::Less => lo = mid + 1,
                std::cmp::Ordering::Greater => hi = mid,
            }
        }
        Ok(Err(lo))
    }
}
fn failure(code: u32) -> Flow {
    Flow::Failure(
        NormalizedStatus::try_new(
            crate::map_ops::STATUS_DOMAIN,
            code,
            StatusClass::Adapter,
            Retryability::Known(false),
        )
        .expect("compiler-owned map v2 status is valid"),
    )
}
impl Evaluator<'_> {
    fn collection_atom_copy(&mut self, atom: &Value) -> Result<Value, Flow> {
        Ok(match atom {
            Value::String(s) => Value::String(self.materialize_utf8_copy(s)?),
            Value::Int(v) => Value::Int(*v),
            Value::Int32(v) => Value::Int32(*v),
            Value::Uint8(v) => Value::Uint8(*v),
            Value::Usize(v) => Value::Usize(*v),
            Value::Char(v) => Value::Char(*v),
            Value::Float32(v) => Value::Float32(*v),
            Value::Float64(v) => Value::Float64(*v),
            Value::Bool(v) => Value::Bool(*v),
            _ => return Err(Flow::Guard("invalid typed collection atom")),
        })
    }
    pub(super) fn evaluate_typed_map(
        &mut self,
        op: MapOp,
        types: &[ResolvedType],
        args: &[ResolvedExpr],
        environment: &mut Environment,
        depth: usize,
    ) -> Result<Value, Flow> {
        if let Some(legacy) = op.legacy(types) {
            return self.evaluate_string_op(legacy, args, environment, depth);
        }
        self.charge()?;
        let (params, result) = op
            .resolved_signature(types)
            .ok_or(Flow::Guard("invalid typed collection signature"))?;
        let mut values = Vec::new();
        for (index, arg) in args.iter().enumerate() {
            if crate::map_ops::is_typed_collection(&arg.ty)
                && params[index].ownership == hir::OwnershipMode::Borrow
            {
                if let ResolvedExprKind::Place(place) = &arg.kind {
                    values.push(
                        self.lookup_place(environment, place)?
                            .ok_or(Flow::Guard("borrowed collection unavailable"))?,
                    );
                    continue;
                }
            }
            values.push(self.evaluate(arg, environment, depth)?);
        }
        if matches!(op, MapOp::New | MapOp::SetNew) {
            let [Value::Usize(capacity)] = values.as_slice() else {
                return Err(Flow::Guard("invalid collection capacity"));
            };
            if *capacity > 65536 {
                return Err(failure(3));
            }
            return Ok(Value::Collection(Arc::new(MapValue {
                ty: result,
                capacity: *capacity as usize,
                entries: Vec::new(),
            })));
        }
        if op.reopens() {
            let mut values = values.into_iter();
            let Some(Value::Collection(map)) = values.next() else {
                return Err(Flow::Guard("missing owned collection"));
            };
            let key = values.next().ok_or(Flow::Guard("missing collection key"))?;
            let mut map =
                Arc::try_unwrap(map).map_err(|_| Flow::Guard("aliased owned collection"))?;
            let position = map.find(&key)?;
            if matches!(op, MapOp::Remove | MapOp::SetRemove) {
                if let Ok(i) = position {
                    map.entries.remove(i);
                }
                return Ok(Value::Collection(Arc::new(map)));
            }
            let mut value = if op == MapOp::SetInsert {
                Value::Bool(true)
            } else {
                values
                    .next()
                    .ok_or(Flow::Guard("missing collection value"))?
            };
            if let Ok(i) = position {
                if op == MapOp::Add {
                    let (Value::Int(old), Value::Int(increment)) = (&map.entries[i].1, &value)
                    else {
                        return Err(Flow::Guard("invalid collection addition"));
                    };
                    value = Value::Int(old.checked_add(*increment).ok_or_else(|| failure(4))?);
                }
                map.entries[i].1 = self.collection_atom_copy(&value)?;
            } else if let Err(i) = position {
                if map.entries.len() == map.capacity {
                    return Err(failure(1));
                }
                let key = self.collection_atom_copy(&key)?;
                let value = self.collection_atom_copy(&value)?;
                map.entries.insert(i, (key, value));
            }
            return Ok(Value::Collection(Arc::new(map)));
        }
        let Some(Value::Collection(map)) = values.first() else {
            return Err(Flow::Guard("missing borrowed collection"));
        };
        match op {
            MapOp::Len | MapOp::SetLen => Ok(Value::Usize(map.entries.len() as u64)),
            MapOp::Has | MapOp::SetHas => Ok(Value::Bool(map.find(&values[1])?.is_ok())),
            MapOp::GetOr => {
                let value = map
                    .find(&values[1])?
                    .ok()
                    .map(|i| &map.entries[i].1)
                    .unwrap_or(&values[2]);
                self.collection_atom_copy(value)
            }
            MapOp::KeyAt | MapOp::ValueAt | MapOp::SetKeyAt => {
                let Value::Usize(index) = values[1] else {
                    return Err(Flow::Guard("invalid collection index"));
                };
                let entry = map.entries.get(index as usize).ok_or_else(|| failure(2))?;
                self.collection_atom_copy(if op == MapOp::ValueAt {
                    &entry.1
                } else {
                    &entry.0
                })
            }
            _ => Err(Flow::Guard("invalid collection read")),
        }
    }
}
