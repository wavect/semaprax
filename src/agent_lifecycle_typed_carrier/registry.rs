//! An ordered, selector-addressed rich effect operation registry.
//!
//! Mirrors [Direct Agent Runtime v2](../../docs/AGENT-RUNTIME-V2.md)'s own
//! operation registry discipline — "ordered and selected by an exact
//! checked `usize` Proposal field... its operation, effect, argument and
//! result identities must match the deployed source contracts" — for the
//! rich profile: [`TypedCarrierRegistry::resolve`] requires the selected
//! slot's deployed operation identity to equal exactly what the caller
//! declares it expects, refusing an incorrect deployed operation (a
//! reordered registry, or a caller naming the wrong operation) before the
//! handler is ever called. [`call_typed_operation`] then validates the
//! argument shape before dispatch and the result shape after, so a
//! malformed or wrongly-typed result is refused the same way a malformed
//! argument is, and the staged argument's ownership settles
//! ([`OwnershipLedger`]) regardless of what the handler returns.

use crate::agent_interaction_schema::{CompiledInteractionSchema, DecodedInteractionValue};
use crate::agent_runtime::AgentCancellation;
use crate::diagnostic::Diagnostic;
use crate::interpreter::retained_call::RetainedValue;

use super::binding::StageBinding;
use super::ownership::OwnershipLedger;
use super::projection::{to_retained, InteractionTypeGraph};
use super::refusal;

/// One rich effect operation's exact source-owned contract: the deployed
/// operation identity it must be selected under, and the exact argument and
/// result bindings its handler is contracted to.
pub struct TypedCarrierOperation {
    pub deployed_operation_id: String,
    pub argument: StageBinding,
    pub result: StageBinding,
}

/// An ordered registry of [`TypedCarrierOperation`]s, addressed by
/// position. Reordering this vector changes which operation a given
/// selector resolves to — the same "registry reordering changes the
/// deployment association" property Direct Runtime v2's own registry has.
pub struct TypedCarrierRegistry {
    operations: Vec<TypedCarrierOperation>,
}

impl TypedCarrierRegistry {
    #[must_use]
    pub fn new(operations: Vec<TypedCarrierOperation>) -> Self {
        Self { operations }
    }

    /// Resolves the operation at `selector`, requiring its deployed
    /// operation identity to equal `expected_deployed_operation_id`.
    ///
    /// Refuses before any dispatch (`SPX-Z211`):
    /// - `operation.selector_out_of_range` — no operation at `selector`.
    /// - `operation.wrong_deployed_operation` — the resolved slot's
    ///   deployed operation identity disagrees with what the caller
    ///   declares it expects.
    pub fn resolve(
        &self,
        selector: usize,
        expected_deployed_operation_id: &str,
    ) -> Result<&TypedCarrierOperation, Diagnostic> {
        let operation = self
            .operations
            .get(selector)
            .ok_or_else(|| refusal("SPX-Z211", "operation.selector_out_of_range"))?;
        if operation.deployed_operation_id != expected_deployed_operation_id {
            return Err(refusal("SPX-Z211", "operation.wrong_deployed_operation"));
        }
        Ok(operation)
    }
}

/// The injected rich effect handler boundary. A real implementation binds a
/// deployed tool contract; this module ships no implementation of its own.
/// Returns untrusted response bytes shaped like one
/// `semaprax.agent-interaction-value.v1` document for the operation's
/// declared result binding — never a value this module already trusts.
pub trait TypedCarrierHandler {
    fn execute(&mut self, operation_id: &str, argument: &RetainedValue) -> Vec<u8>;
}

/// Resolves `selector`, admits and projects the argument, stages exactly
/// one owned temporary for the handler call, and validates the handler's
/// response against the operation's result binding.
///
/// Refuses before any dispatch, with no [`OwnedToken`](super::OwnedToken)
/// ever opened:
/// - Cancellation already requested (`SPX-Z213`, `operation.cancelled`).
/// - Operation resolution failure (`SPX-Z211`, from
///   [`TypedCarrierRegistry::resolve`]).
/// - Argument admission or projection failure (`SPX-Z210`).
///
/// Once the argument is staged, the [`OwnedToken`](super::OwnedToken)
/// covers exactly the handler call and settles when it returns,
/// independent of what the handler returned. The handler's response is
/// then decoded against `result_schema` and admitted against the
/// operation's result binding; a decode failure (partial/malformed
/// response) or a result-binding refusal (wrong nominal type/case, stale
/// schema) is returned as `SPX-Z212`/`SPX-Z210` respectively, always after
/// the argument's ownership has already settled.
#[allow(clippy::too_many_arguments)]
pub fn call_typed_operation(
    ledger: &OwnershipLedger,
    cancellation: &AgentCancellation,
    registry: &TypedCarrierRegistry,
    selector: usize,
    expected_deployed_operation_id: &str,
    argument_graph: &InteractionTypeGraph,
    argument: DecodedInteractionValue,
    result_schema: &CompiledInteractionSchema,
    handler: &mut dyn TypedCarrierHandler,
) -> Result<DecodedInteractionValue, Diagnostic> {
    if cancellation.is_cancelled() {
        return Err(refusal("SPX-Z213", "operation.cancelled"));
    }
    let operation = registry.resolve(selector, expected_deployed_operation_id)?;
    let admitted_argument = operation.argument.admit(argument)?;
    let retained_argument = to_retained(argument_graph, &admitted_argument)?;

    let result_bytes = {
        let _token = ledger.open();
        handler.execute(&operation.deployed_operation_id, &retained_argument)
    };

    let decoded_result = result_schema
        .decode(&result_bytes)
        .map_err(|_| refusal("SPX-Z212", "operation.malformed_result"))?;
    operation.result.admit(decoded_result)
}
