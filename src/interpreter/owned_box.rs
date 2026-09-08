//! Reference-interpreter carrier and operations for compiler-owned bounded Box.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::conformance::{NormalizedStatus, Retryability, StatusClass};
use crate::hir::{FunctionInstanceId, ResolvedExpr, ResolvedExprKind, ResolvedType};

use super::{Environment, Evaluator, Flow, Value};

pub(super) fn instance_is_admitted(
    program: &crate::hir::ResolvedProgram,
    instance: &crate::hir::ResolvedFunctionInstance,
) -> bool {
    instance.id == FunctionInstanceId::derive(&instance.template, &instance.type_arguments)
        && instance.function.id == instance.template
        && program
            .function_templates
            .iter()
            .find(|template| template.id == instance.template)
            .is_some_and(|template| {
                (crate::hir::generic_collection::profile(template)
                    && crate::hir::generic_collection::arguments(&instance.type_arguments)
                    || crate::box_ops::hir_wrapper_in_program(program, template).is_some()
                        && crate::box_ops::hir_arguments_are_admitted(
                            program,
                            template,
                            &instance.type_arguments,
                        ))
                    && crate::hir::is_exact_materialized_function_instance(template, instance)
            })
}

#[derive(Debug)]
pub(super) struct OwnedBoxValue {
    pub(super) element: ResolvedType,
    pub(super) value: Option<Value>,
    pub(super) generation: u64,
    allocations: Arc<AtomicUsize>,
}

impl PartialEq for OwnedBoxValue {
    fn eq(&self, other: &Self) -> bool {
        self.element == other.element
            && self.value == other.value
            && self.generation == other.generation
    }
}

impl Drop for OwnedBoxValue {
    fn drop(&mut self) {
        let previous = self.allocations.fetch_sub(1, Ordering::AcqRel);
        assert!(previous != 0, "bounded Box allocation accounting underflow");
    }
}

fn scalar_matches(value: &Value, ty: &ResolvedType) -> bool {
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
    )
}

fn allocation_failure() -> Flow {
    Flow::Failure(
        NormalizedStatus::try_new(
            crate::box_ops::STATUS_DOMAIN,
            crate::box_ops::ALLOCATION_FAILURE_CODE,
            StatusClass::Adapter,
            Retryability::Known(false),
        )
        .expect("compiler-owned bounded Box status table is valid"),
    )
}

pub(super) fn is_intrinsic_call(
    callee: &crate::hir::DeclarationId,
    instance: &Option<FunctionInstanceId>,
    type_arguments: &[ResolvedType],
) -> bool {
    instance.is_none()
        && crate::box_ops::by_id(callee.as_str()).is_some()
        && type_arguments.len() == 1
}

impl Evaluator<'_> {
    pub(super) fn evaluate_box_op(
        &mut self,
        op: crate::box_ops::BoxOp,
        type_arguments: &[ResolvedType],
        args: &[ResolvedExpr],
        environment: &mut Environment,
        depth: usize,
    ) -> Result<Value, Flow> {
        self.charge()?;
        let [element] = type_arguments else {
            return Err(Flow::Guard("invalid compiler-owned bounded Box type"));
        };
        if !crate::box_ops::resolved_element_is_admitted(element) || args.len() != 1 {
            return Err(Flow::Guard("invalid compiler-owned bounded Box call"));
        }
        let value = if op == crate::box_ops::BoxOp::Get {
            self.charge()?;
            let ResolvedExprKind::Place(place) = &args[0].kind else {
                return Err(Flow::Guard(
                    "borrowed bounded Box argument is not a named place",
                ));
            };
            if !place.projections.is_empty() {
                return Err(Flow::Guard("borrowed bounded Box argument is projected"));
            }
            self.lookup(environment, &place.root)?
                .ok_or(Flow::Guard("borrowed bounded Box owner is unavailable"))?
        } else {
            self.evaluate(&args[0], environment, depth)?
        };
        match op {
            crate::box_ops::BoxOp::New => {
                if !scalar_matches(&value, element) {
                    return Err(Flow::Guard("forged bounded Box element type"));
                }
                self.box_live_allocations
                    .fetch_update(Ordering::AcqRel, Ordering::Acquire, |live| {
                        (live < crate::box_ops::MAX_LIVE_ALLOCATIONS).then_some(live + 1)
                    })
                    .map_err(|_| allocation_failure())?;
                Ok(Value::Box(Arc::new(OwnedBoxValue {
                    element: element.clone(),
                    value: Some(value),
                    generation: 1,
                    allocations: Arc::clone(&self.box_live_allocations),
                })))
            }
            crate::box_ops::BoxOp::Get => {
                let Value::Box(carrier) = value else {
                    return Err(Flow::Guard("ill-typed bounded Box get"));
                };
                if carrier.element != *element || carrier.generation == 0 {
                    return Err(Flow::Guard("forged bounded Box carrier"));
                }
                self.clone_value(
                    carrier
                        .value
                        .as_ref()
                        .ok_or(Flow::Guard("empty bounded Box carrier"))?,
                )
            }
            crate::box_ops::BoxOp::IntoInner => {
                let Value::Box(carrier) = value else {
                    return Err(Flow::Guard("ill-typed bounded Box into_inner"));
                };
                if carrier.element != *element || carrier.generation == 0 {
                    return Err(Flow::Guard("forged bounded Box carrier"));
                }
                let mut carrier = Arc::try_unwrap(carrier)
                    .map_err(|_| Flow::Guard("aliased owned bounded Box carrier"))?;
                carrier
                    .value
                    .take()
                    .ok_or(Flow::Guard("empty bounded Box carrier"))
            }
        }
    }
}

/// Reconstruct only the existing finite Box/Vec tables. The envelope verifier
/// independently compares schema, class, and retryability to this rendering.
pub(super) fn rebuild_collection_status(
    domain: Option<&str>,
    code: u64,
) -> Result<String, crate::diagnostic::Diagnostic> {
    let admitted = match domain {
        Some(crate::box_ops::STATUS_DOMAIN) => {
            code == u64::from(crate::box_ops::ALLOCATION_FAILURE_CODE)
        }
        Some(crate::vec_ops::STATUS_DOMAIN) => [
            crate::vec_ops::PUSH_FULL_CODE,
            crate::vec_ops::GET_OUT_OF_BOUNDS_CODE,
            crate::vec_ops::ALLOCATION_FAILURE_CODE,
        ]
        .into_iter()
        .any(|known| code == u64::from(known)),
        _ => false,
    };
    if !admitted {
        return Err(super::consistency_error(
            "interpreted failures only ever carry compiler-owned status domains".to_owned(),
        ));
    }
    Ok(NormalizedStatus::try_new(
        domain.expect("admitted collection domain"),
        u32::try_from(code).expect("finite collection status code"),
        StatusClass::Adapter,
        Retryability::Known(false),
    )
    .expect("compiler collection status table is valid")
    .to_json())
}

#[cfg(test)]
mod status_tests {
    #[test]
    fn collection_status_reconstruction_is_closed() {
        for (domain, codes) in [
            (crate::box_ops::STATUS_DOMAIN, vec![1]),
            (crate::vec_ops::STATUS_DOMAIN, vec![1, 2, 3]),
        ] {
            for code in codes {
                let rendered = super::rebuild_collection_status(Some(domain), code).unwrap();
                let status: serde_json::Value = serde_json::from_str(&rendered).unwrap();
                super::super::verify_status(&status).unwrap();
                for (field, value) in [
                    ("class", serde_json::json!("contract")),
                    ("retryable", serde_json::json!(true)),
                    ("code", serde_json::json!(0)),
                    ("code", serde_json::json!(4)),
                    ("code", serde_json::json!(4294967297u64)),
                    ("domain_id", serde_json::json!("foreign.box")),
                ] {
                    let mut forged = status.clone();
                    forged[field] = value;
                    assert!(super::super::verify_status(&forged).is_err(), "{forged}");
                }
            }
        }
        assert!(super::rebuild_collection_status(Some(crate::box_ops::STATUS_DOMAIN), 2).is_err());
    }
}
