//! Reference-interpreter carrier and evaluation for compiler-owned bounded Vec.

use std::sync::Arc;

use crate::conformance::{NormalizedStatus, Retryability, StatusClass};
use crate::hir::{FunctionInstanceId, ResolvedExpr, ResolvedExprKind, ResolvedType};

use super::{Environment, Evaluator, Flow, Value};

pub(super) fn is_collection_type(ty: &ResolvedType) -> bool {
    crate::iterator_ops::is_iter(ty)
        || crate::cleanup::is_owned_bounded_vec_type(ty)
        || crate::cleanup::is_owned_bounded_box_type(ty)
}

pub(super) fn instance_is_admitted(
    program: &crate::hir::ResolvedProgram,
    instance: &crate::hir::ResolvedFunctionInstance,
) -> bool {
    super::resolved_signature_is_admitted(&instance.function, &program.declarations)
        || (instance.id == FunctionInstanceId::derive(&instance.template, &instance.type_arguments)
            && instance.function.id == instance.template
            && program
                .function_templates
                .iter()
                .find(|template| template.id == instance.template)
                .is_some_and(|template| {
                    crate::vec_ops::hir_wrapper_in_program(program, template).is_some()
                        && crate::vec_ops::hir_arguments_are_admitted(
                            program,
                            template,
                            &instance.type_arguments,
                        )
                        && crate::hir::is_exact_materialized_function_instance(template, instance)
                }))
}

#[derive(Debug, PartialEq)]
pub(super) struct OwnedVecValue {
    pub(super) element: ResolvedType,
    pub(super) values: Vec<Value>,
    pub(super) capacity: usize,
    pub(super) generation: u64,
}

fn scalar_value_matches_type(value: &Value, ty: &ResolvedType) -> bool {
    matches!(
        (value, ty),
        (Value::Int(_), ResolvedType::I64)
            | (Value::Int32(_), ResolvedType::I32)
            | (Value::Uint8(_), ResolvedType::U8)
            | (Value::Usize(_), ResolvedType::Usize)
            | (Value::Char(_), ResolvedType::Char)
            | (Value::Float32(_), ResolvedType::F32)
            | (Value::Float64(_), ResolvedType::F64)
            | (Value::Bool(_), ResolvedType::Bool)
            | (Value::Bytes(_), ResolvedType::Bytes)
    )
}

fn normalize_vec(code: u32) -> NormalizedStatus {
    NormalizedStatus::try_new(
        crate::vec_ops::STATUS_DOMAIN,
        code,
        StatusClass::Adapter,
        Retryability::Known(false),
    )
    .expect("compiler-owned bounded Vec status table is valid")
}

pub(super) fn is_intrinsic_call(
    callee: &crate::hir::DeclarationId,
    instance: &Option<FunctionInstanceId>,
    type_arguments: &[ResolvedType],
) -> bool {
    instance.is_none()
        && crate::vec_ops::by_id(callee.as_str()).is_some()
        && type_arguments.len() == 1
}

impl Evaluator<'_> {
    pub(super) fn evaluate_vec_op(
        &mut self,
        op: crate::vec_ops::VecOp,
        type_arguments: &[ResolvedType],
        args: &[ResolvedExpr],
        environment: &mut Environment,
        depth: usize,
    ) -> Result<Value, Flow> {
        self.charge()?;
        let element = type_arguments
            .first()
            .filter(|element| {
                type_arguments.len() == 1
                    && crate::vec_ops::resolved_operation_element_is_admitted(op, element)
            })
            .ok_or(Flow::Guard("invalid compiler-owned bounded Vec type"))?
            .clone();
        let mut values = Vec::with_capacity(args.len());
        for (index, argument) in args.iter().enumerate() {
            if index == 0
                && matches!(
                    op,
                    crate::vec_ops::VecOp::Len
                        | crate::vec_ops::VecOp::Capacity
                        | crate::vec_ops::VecOp::Get
                )
            {
                self.charge()?;
                let ResolvedExprKind::Place(place) = &argument.kind else {
                    return Err(Flow::Guard(
                        "borrowed bounded Vec argument is not a named place",
                    ));
                };
                if !place.projections.is_empty() {
                    return Err(Flow::Guard("borrowed bounded Vec argument is projected"));
                }
                values.push(
                    self.lookup(environment, &place.root)?
                        .ok_or(Flow::Guard("borrowed bounded Vec owner is unavailable"))?,
                );
            } else {
                values.push(self.evaluate(argument, environment, depth)?);
            }
        }
        match op {
            crate::vec_ops::VecOp::WithCapacity => {
                let [Value::Usize(capacity)] = values.as_slice() else {
                    return Err(Flow::Guard(
                        "ill-typed compiler-owned bounded Vec operation",
                    ));
                };
                let capacity = usize::try_from(*capacity).map_err(|_| {
                    Flow::Failure(normalize_vec(crate::vec_ops::ALLOCATION_FAILURE_CODE))
                })?;
                if capacity > crate::vec_ops::MAX_CAPACITY as usize {
                    return Err(Flow::Failure(normalize_vec(
                        crate::vec_ops::ALLOCATION_FAILURE_CODE,
                    )));
                }
                if element == ResolvedType::Bytes
                    && u64::try_from(capacity)
                        .ok()
                        .and_then(|capacity| {
                            capacity.checked_mul(crate::vec_ops::OWNED_PAYLOAD_BYTES_PER_ELEMENT)
                        })
                        .is_none_or(|charge| charge > crate::vec_ops::MAX_OWNED_PAYLOAD_BYTES)
                {
                    return Err(Flow::Failure(normalize_vec(
                        crate::vec_ops::ALLOCATION_FAILURE_CODE,
                    )));
                }
                let mut elements = Vec::new();
                if elements.try_reserve_exact(capacity).is_err() {
                    return Err(Flow::Failure(normalize_vec(
                        crate::vec_ops::ALLOCATION_FAILURE_CODE,
                    )));
                }
                Ok(Value::Vec(Arc::new(OwnedVecValue {
                    element,
                    values: elements,
                    capacity,
                    generation: 1,
                })))
            }
            crate::vec_ops::VecOp::Push => {
                let mut values = values.into_iter();
                let (Some(Value::Vec(vector)), Some(value), None) =
                    (values.next(), values.next(), values.next())
                else {
                    return Err(Flow::Guard(
                        "ill-typed compiler-owned bounded Vec operation",
                    ));
                };
                if vector.element != element || !scalar_value_matches_type(&value, &element) {
                    return Err(Flow::Guard("forged bounded Vec element type"));
                }
                let mut vector = Arc::try_unwrap(vector)
                    .map_err(|_| Flow::Guard("aliased owned bounded Vec carrier"))?;
                if vector.values.len() == vector.capacity {
                    return Err(Flow::Failure(normalize_vec(crate::vec_ops::PUSH_FULL_CODE)));
                }
                vector.values.push(value);
                vector.generation = vector
                    .generation
                    .checked_add(1)
                    .ok_or(Flow::Guard("bounded Vec generation overflowed"))?;
                Ok(Value::Vec(Arc::new(vector)))
            }
            crate::vec_ops::VecOp::ReserveExact => {
                let mut values = values.into_iter();
                let (Some(Value::Vec(vector)), Some(Value::Usize(additional)), None) =
                    (values.next(), values.next(), values.next())
                else {
                    return Err(Flow::Guard(
                        "ill-typed compiler-owned bounded Vec operation",
                    ));
                };
                if vector.element != element {
                    return Err(Flow::Guard("forged bounded Vec element type"));
                }
                let target = u64::try_from(vector.values.len())
                    .ok()
                    .and_then(|len| len.checked_add(additional))
                    .map(|required| required.max(vector.capacity as u64))
                    .filter(|target| *target <= crate::vec_ops::MAX_CAPACITY)
                    .and_then(|target| usize::try_from(target).ok())
                    .ok_or_else(|| {
                        Flow::Failure(normalize_vec(crate::vec_ops::ALLOCATION_FAILURE_CODE))
                    })?;
                if element == ResolvedType::Bytes
                    && u64::try_from(target)
                        .ok()
                        .and_then(|capacity| {
                            capacity.checked_mul(crate::vec_ops::OWNED_PAYLOAD_BYTES_PER_ELEMENT)
                        })
                        .is_none_or(|charge| charge > crate::vec_ops::MAX_OWNED_PAYLOAD_BYTES)
                {
                    return Err(Flow::Failure(normalize_vec(
                        crate::vec_ops::ALLOCATION_FAILURE_CODE,
                    )));
                }
                let mut vector = Arc::try_unwrap(vector)
                    .map_err(|_| Flow::Guard("aliased owned bounded Vec carrier"))?;
                if target > vector.values.capacity()
                    && vector
                        .values
                        .try_reserve_exact(target - vector.values.len())
                        .is_err()
                {
                    return Err(Flow::Failure(normalize_vec(
                        crate::vec_ops::ALLOCATION_FAILURE_CODE,
                    )));
                }
                vector.capacity = target;
                vector.generation = vector
                    .generation
                    .checked_add(1)
                    .ok_or(Flow::Guard("bounded Vec generation overflowed"))?;
                Ok(Value::Vec(Arc::new(vector)))
            }
            crate::vec_ops::VecOp::Set => {
                let mut values = values.into_iter();
                let (Some(Value::Vec(vector)), Some(Value::Usize(index)), Some(value), None) =
                    (values.next(), values.next(), values.next(), values.next())
                else {
                    return Err(Flow::Guard(
                        "ill-typed compiler-owned bounded Vec operation",
                    ));
                };
                if vector.element != element || !scalar_value_matches_type(&value, &element) {
                    return Err(Flow::Guard("forged bounded Vec element type"));
                }
                let index = usize::try_from(index)
                    .ok()
                    .filter(|index| *index < vector.values.len())
                    .ok_or_else(|| {
                        Flow::Failure(normalize_vec(crate::vec_ops::GET_OUT_OF_BOUNDS_CODE))
                    })?;
                let mut vector = Arc::try_unwrap(vector)
                    .map_err(|_| Flow::Guard("aliased owned bounded Vec carrier"))?;
                vector.values[index] = value;
                vector.generation = vector
                    .generation
                    .checked_add(1)
                    .ok_or(Flow::Guard("bounded Vec generation overflowed"))?;
                Ok(Value::Vec(Arc::new(vector)))
            }
            crate::vec_ops::VecOp::Clear => {
                let [Value::Vec(vector)] = values.as_slice() else {
                    return Err(Flow::Guard(
                        "ill-typed compiler-owned bounded Vec operation",
                    ));
                };
                if vector.element != element {
                    return Err(Flow::Guard("forged bounded Vec element type"));
                }
                // The `as_slice()` match above already proved the sole value
                // is `Value::Vec`; this re-destructures the owned `Value` (an
                // owned `Vec::into_iter().next()` cannot itself change
                // variant). Guarded rather than `unreachable!()` per
                // `docs/OWNED-RECORD-COLLECTION-ELEMENT-V1.md` (backend
                // hazard): a clean diagnostic, never a panic, if a future
                // admission widening or refactor ever invalidates that proof.
                let Value::Vec(vector) = values.into_iter().next().unwrap() else {
                    return Err(Flow::Guard("validated Vec clear carrier changed variant"));
                };
                let mut vector = Arc::try_unwrap(vector)
                    .map_err(|_| Flow::Guard("aliased owned bounded Vec carrier"))?;
                vector.values.clear();
                vector.generation = vector
                    .generation
                    .checked_add(1)
                    .ok_or(Flow::Guard("bounded Vec generation overflowed"))?;
                Ok(Value::Vec(Arc::new(vector)))
            }
            crate::vec_ops::VecOp::Len => match values.as_slice() {
                [Value::Vec(vector)] if vector.element == element => {
                    Ok(Value::Usize(vector.values.len() as u64))
                }
                _ => Err(Flow::Guard(
                    "ill-typed compiler-owned bounded Vec operation",
                )),
            },
            crate::vec_ops::VecOp::Capacity => match values.as_slice() {
                [Value::Vec(vector)] if vector.element == element => {
                    Ok(Value::Usize(vector.capacity as u64))
                }
                _ => Err(Flow::Guard(
                    "ill-typed compiler-owned bounded Vec operation",
                )),
            },
            crate::vec_ops::VecOp::Get => match values.as_slice() {
                [Value::Vec(vector), Value::Usize(index)] if vector.element == element => {
                    let value = usize::try_from(*index)
                        .ok()
                        .and_then(|index| vector.values.get(index))
                        .ok_or_else(|| {
                            Flow::Failure(normalize_vec(crate::vec_ops::GET_OUT_OF_BOUNDS_CODE))
                        })?;
                    self.clone_value(value)
                }
                _ => Err(Flow::Guard(
                    "ill-typed compiler-owned bounded Vec operation",
                )),
            },
        }
    }
}
