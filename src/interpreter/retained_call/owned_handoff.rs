//! Sealed, synchronous owner handoff, not a general small-stack interpreter.
//!
//! Only `own Bytes -> Bytes { let next = input; next }` is executable here.
//! Its checked cleanup plan remains authoritative. The weak observation below
//! cannot drop anything; it independently observes the last owner disappearing.

use super::*;
use crate::hir::{OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedStatement};

pub(crate) const MAX_BYTES: usize = 20;
pub(crate) const MAX_FUEL: usize = 8;

pub(crate) struct PreparedOwnedHandoff {
    program: hir::ResolvedProgram,
    prepared: PreparedRetainedCall,
}

/// Constructors are private: a result cannot mint its own settlement evidence.
pub(crate) struct SettledHandoff {
    evaluation: RetainedCallEvaluation,
    released: bool,
    input_length: usize,
}

impl SettledHandoff {
    pub(crate) fn into_bytes(self) -> Result<Vec<u8>, ()> {
        if !self.released
            || self.evaluation.cleanup_events != [OwnedDataCleanupEvent::CopyOutAndSettleBytes]
        {
            return Err(());
        }
        match self.evaluation.outcome {
            RetainedCallOutcome::Returned(RetainedValue::Bytes(bytes))
                if bytes.len() == self.input_length && bytes.len() <= MAX_BYTES =>
            {
                Ok(bytes)
            }
            _ => Err(()),
        }
    }
}

impl PreparedOwnedHandoff {
    pub(crate) fn new(
        program: hir::ResolvedProgram,
        entry_id: &str,
    ) -> Result<Self, Vec<Diagnostic>> {
        // Bound even the validation input before the recursive verifier runs.
        // The production owner derives this tiny program from exact static
        // source; it never hands us caller-authored arbitrary retained HIR.
        if program.functions.len() > 3
            || !program.function_instances.is_empty()
            || !program.function_templates.is_empty()
        {
            return Err(refusal_at("function inventory"));
        }
        // Resolution always inserts the frozen Option/Result prelude, even
        // when neither is referenced. They are not authored aggregate support.
        if program.types.len() != 2
            || program
                .types
                .iter()
                .zip([crate::prelude::OPTION_ID, crate::prelude::RESULT_ID])
                .any(|(declaration, expected)| declaration.id.as_str() != expected)
        {
            return Err(refusal_at(
                "only the implicit Option/Result prelude is permitted",
            ));
        }
        if program.functions.iter().any(|function| {
            !function.requires.is_empty()
                || !function.ensures.is_empty()
                || !function.effects.is_empty()
                || function.yields.is_some()
                || !shallow(&function.body)
        }) {
            return Err(refusal_at("body depth, node budget, contracts, or effects"));
        }
        let entry = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == entry_id)
            .ok_or_else(|| refusal_at("selected entry absent"))?;
        let [parameter] = entry.params.as_slice() else {
            return Err(refusal_at("parameter count"));
        };
        if parameter.ty != ResolvedType::Bytes
            || parameter.ownership != OwnershipMode::Own
            || entry.return_type != ResolvedType::Bytes
        {
            return Err(refusal_at("own Bytes parameter and Bytes result required"));
        }
        let ResolvedExprKind::Block { statements, tail } = &entry.body.kind else {
            return Err(refusal_at("selected block required"));
        };
        let [ResolvedStatement::Let {
            binding,
            mutable: false,
            value,
            ..
        }] = statements.as_slice()
        else {
            return Err(refusal_at("one immutable move binding required"));
        };
        if binding.ty != ResolvedType::Bytes
            || binding.ownership != OwnershipMode::Own
            || !owned_place(value, &parameter.id)
            || !owned_place(tail, &binding.id)
        {
            return Err(refusal_at(
                "owned input-to-binding-to-result places required",
            ));
        }
        let prepared = prepare_retained_call(&program, entry_id)?;
        if prepared.function_ids().count() != 1 {
            return Err(refusal_at("selected closure must contain only the handoff"));
        }
        Ok(Self { program, prepared })
    }

    pub(crate) fn execute(
        &self,
        input: &[u8],
        max_steps: usize,
    ) -> Result<SettledHandoff, Vec<Diagnostic>> {
        if input.len() > MAX_BYTES || !(1..=MAX_FUEL).contains(&max_steps) {
            return Err(refusal());
        }
        let arguments = [RetainedValue::Bytes(input.to_vec())];
        let invocation = execution::prepare(&self.program, &self.prepared, &arguments, max_steps)?;
        let [(_, Value::Bytes(owner))] = invocation.values.as_slice() else {
            return Err(refusal());
        };
        if Arc::strong_count(&owner.bytes) != 1 || owner.allocation != 1 {
            return Err(refusal());
        }
        let observer = Arc::downgrade(&owner.bytes);
        #[cfg(test)]
        tests::after_staging(&observer);
        // No worker, stack allocation request, process, or second evaluator.
        // The only expression recursion is the block and its two plain places.
        let evaluation = invocation.run(max_steps, None);
        let released = observer.upgrade().is_none();
        if !released {
            return Err(refusal());
        }
        Ok(SettledHandoff {
            evaluation,
            released,
            input_length: input.len(),
        })
    }
}

fn owned_place(expression: &ResolvedExpr, id: &hir::ValueId) -> bool {
    expression.ty == ResolvedType::Bytes
        && expression.ownership == OwnershipMode::Own
        && matches!(&expression.kind, ResolvedExprKind::Place(place)
            if place.root == *id && place.projections.is_empty())
}

fn shallow(expression: &ResolvedExpr) -> bool {
    let mut pending = vec![(expression, 0usize)];
    let mut nodes = 0;
    while let Some((expression, depth)) = pending.pop() {
        nodes += 1;
        if nodes > 32 || depth > 4 {
            return false;
        }
        match &expression.kind {
            ResolvedExprKind::Int(_) | ResolvedExprKind::Place(_) => {}
            ResolvedExprKind::Block { statements, tail } => {
                if statements.len() > 2 {
                    return false;
                }
                pending.push((tail, depth + 1));
                for statement in statements {
                    let ResolvedStatement::Let {
                        value,
                        mutable: false,
                        ..
                    } = statement
                    else {
                        return false;
                    };
                    pending.push((value, depth + 1));
                }
            }
            // A target-only borrowed-input shim calls the checked owner. It
            // is never selected by this seam; its closure is verified normally.
            ResolvedExprKind::Call {
                args,
                type_arguments,
                instance,
                ..
            } if type_arguments.is_empty() && instance.is_none() && args.len() == 1 => {
                pending.push((&args[0], depth + 1));
            }
            _ => return false,
        }
    }
    true
}

fn refusal() -> Vec<Diagnostic> {
    refusal_at("byte/fuel bound or owner settlement")
}

fn refusal_at(reason: &'static str) -> Vec<Diagnostic> {
    vec![guard_error(&format!("owned handoff refused: {reason}"))]
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) fn staged_count() -> usize {
    tests::staged_count()
}

#[cfg(test)]
pub(crate) fn panic_on_next_staging() {
    tests::panic_on_next_staging();
}
