//! Checked, durable binding facts for the generic retry/failover profile.
//!
//! This module intentionally carries no adapter factory or persistence
//! authority.  It derives a closed binding from retained execution roots and
//! names the exact ordered policy that a V2 policy journal may continue.

use crate::agent_deployment::{BoundAgentDeployment, DeploymentModelSelection};
use crate::agent_interaction_schema::{
    compile_agent_interaction_schema_from_retained_source, CompiledInteractionSchema,
};
use crate::diagnostic::quote_json;
use crate::execution_revision::ExecutionRevision;
use crate::live_invocation::{
    budget::InvocationClock, identity::digest, LiveInvocationId, LiveInvocationSeed,
};
use sha2::{Digest, Sha256};
use std::path::Path;

use super::{DurableByteBudget, EffectiveModelBudget, ProviderPolicy};
use crate::provider_adapter_sdk::AdapterRequest;

const BINDING_DOMAIN: &[u8] = b"semaprax.generic-model-policy.binding.v2\0";
const MAX_PROVIDER_ID_BYTES: usize = 256;

/// Why a generic durable policy could not be bound before any adapter is
/// constructed.  These are local association failures, never provider facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DurablePolicyBindingRefusal {
    EmptyProviderPolicy,
    ProviderOrderMismatch,
    UnauthorizedPrimary,
    ProviderIdTooLong { max: usize },
    DeadlineOverflow,
    ExecutionRootMismatch,
    DeploymentBindingMismatch,
    SchemaMismatch,
    RetainedSchemaMismatch,
    RetainedLimitMismatch { dimension: &'static str },
    ProviderNotDeploymentSelected { provider: String },
    ContextLimitExceedsDeployment { requested: u64, max: u64 },
    RetainedByteLimitMismatch { dimension: &'static str },
    AdapterRequestBytesExceeded { dimension: &'static str },
}

/// An invocation- and retained-execution-root-bound ordered provider policy.
///
/// There is deliberately no public constructor from arbitrary strings.  The
/// only constructor needs an [`ExecutionRevision`], whose roots are produced
/// by the checked project/deployment binding path; a request, response, or
/// checkpoint document cannot mint this type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurablePolicyBinding {
    invocation: LiveInvocationId,
    deployment_root: String,
    instance_root: String,
    deployment_policy: String,
    task_digest: String,
    interaction_schema_digest: String,
    task_budget: i64,
    policy: ProviderPolicy,
    model_selections: Option<Vec<DeploymentModelSelection>>,
    limits: EffectiveModelBudget,
    retained_byte_budget: Option<DurableByteBudget>,
    deadline_millis: Option<i64>,
    digest: String,
}

impl DurablePolicyBinding {
    /// Binds the exact invocation seed to the retained deployment and
    /// instance roots.  Provider order is checked against the order already
    /// committed into `LiveInvocationId`; neither the journal nor a host
    /// callback can substitute a fallback later.
    pub fn bind(
        execution: &ExecutionRevision,
        deployment: &BoundAgentDeployment,
        schema: &CompiledInteractionSchema,
        seed: &LiveInvocationSeed,
        policy: ProviderPolicy,
        limits: EffectiveModelBudget,
        started_at_millis: i64,
    ) -> Result<Self, DurablePolicyBindingRefusal> {
        let facts = root_facts(execution.deployment_root().canonical_json())?;
        if facts.get("definition").and_then(serde_json::Value::as_str)
            != Some(deployment.semantic_definition().digest())
            || facts.get("deployment").and_then(serde_json::Value::as_str)
                != Some(deployment.deployment().digest())
            || facts.get("binding").and_then(serde_json::Value::as_str) != Some(deployment.digest())
            || facts
                .get("program_root")
                .and_then(serde_json::Value::as_str)
                != Some(seed.program_root.as_str())
        {
            return Err(DurablePolicyBindingRefusal::ExecutionRootMismatch);
        }
        let source_path = facts
            .get("source_path")
            .and_then(serde_json::Value::as_str)
            .ok_or(DurablePolicyBindingRefusal::ExecutionRootMismatch)?;
        let source_revision = facts
            .get("source_revision")
            .and_then(serde_json::Value::as_str)
            .ok_or(DurablePolicyBindingRefusal::ExecutionRootMismatch)?;
        let retained_source = execution
            .project_revision()
            .sources()
            .iter()
            .find(|source| source.path() == source_path)
            .ok_or(DurablePolicyBindingRefusal::ExecutionRootMismatch)?;
        if retained_source.source_revision() != source_revision {
            return Err(DurablePolicyBindingRefusal::ExecutionRootMismatch);
        }
        let proposal_type_id = serde_json::from_str::<serde_json::Value>(
            deployment.semantic_definition().canonical_json(),
        )
        .ok()
        .and_then(|definition| {
            definition
                .get("types")?
                .as_array()?
                .iter()
                .find_map(|item| {
                    if item.get("role")?.as_str() == Some("proposal") {
                        item.get("stable_id")?.as_str().map(str::to_owned)
                    } else {
                        None
                    }
                })
        })
        .ok_or(DurablePolicyBindingRefusal::RetainedSchemaMismatch)?;
        let retained_schema = compile_agent_interaction_schema_from_retained_source(
            retained_source.source(),
            Path::new(retained_source.path()),
            &proposal_type_id,
        )
        .map_err(|_| DurablePolicyBindingRefusal::RetainedSchemaMismatch)?;
        if retained_schema.schema().canonical_json() != schema.schema().canonical_json()
            || retained_schema.source_revision() != schema.source_revision()
            || retained_schema.schema().digest() != schema.schema().digest()
        {
            return Err(DurablePolicyBindingRefusal::RetainedSchemaMismatch);
        }
        let instance = root_facts(execution.instance_root().canonical_json())?;
        let task_digest = input_digest(&seed.task);
        if instance
            .get("deployment_root")
            .and_then(serde_json::Value::as_str)
            != Some(execution.deployment_root().digest())
            || instance
                .get("task_digest")
                .and_then(serde_json::Value::as_str)
                != Some(task_digest.as_str())
            || instance
                .get("task_budget")
                .and_then(serde_json::Value::as_i64)
                != Some(seed.budget)
        {
            return Err(DurablePolicyBindingRefusal::ExecutionRootMismatch);
        }
        let revision = root_facts(execution.execution_revision().canonical_json())?;
        if revision
            .get("program_root")
            .and_then(serde_json::Value::as_str)
            != Some(seed.program_root.as_str())
            || revision
                .get("deployment_root")
                .and_then(serde_json::Value::as_str)
                != Some(execution.deployment_root().digest())
            || revision
                .get("instance_root")
                .and_then(serde_json::Value::as_str)
                != Some(execution.instance_root().digest())
            || revision
                .get("project_revision")
                .and_then(serde_json::Value::as_str)
                != Some(execution.project_revision().project_revision())
        {
            return Err(DurablePolicyBindingRefusal::ExecutionRootMismatch);
        }
        if seed.deployment_policy != deployment.digest() {
            return Err(DurablePolicyBindingRefusal::DeploymentBindingMismatch);
        }
        if seed.interaction_schema_digest != schema.schema().digest() {
            return Err(DurablePolicyBindingRefusal::SchemaMismatch);
        }
        let Some(primary) = policy.primary() else {
            return Err(DurablePolicyBindingRefusal::EmptyProviderPolicy);
        };
        if !primary.authorized {
            return Err(DurablePolicyBindingRefusal::UnauthorizedPrimary);
        }
        let selected = deployment.model_selections();
        if policy.len() != seed.approved_providers.len()
            || policy.len() != selected.len()
            || seed
                .approved_providers
                .iter()
                .enumerate()
                .any(|(index, id)| policy.slot(index).is_none_or(|slot| slot.id != *id))
            || selected.iter().enumerate().any(|(index, selected)| {
                policy
                    .slot(index)
                    .is_none_or(|slot| slot.id != selected.provider_id())
            })
        {
            return Err(DurablePolicyBindingRefusal::ProviderOrderMismatch);
        }
        if (0..policy.len()).any(|index| {
            policy
                .slot(index)
                .is_some_and(|slot| slot.id.is_empty() || slot.id.len() > MAX_PROVIDER_ID_BYTES)
        }) {
            return Err(DurablePolicyBindingRefusal::ProviderIdTooLong {
                max: MAX_PROVIDER_ID_BYTES,
            });
        }
        check_retained_limits(deployment, limits)?;
        let retained_byte_budget = retained_byte_budget(deployment)?;
        let deployment_context_limit = selected
            .iter()
            .map(|item| item.max_context_tokens())
            .min()
            .ok_or(DurablePolicyBindingRefusal::EmptyProviderPolicy)?;
        if limits.limits().max_context_tokens > deployment_context_limit {
            return Err(DurablePolicyBindingRefusal::ContextLimitExceedsDeployment {
                requested: limits.limits().max_context_tokens,
                max: deployment_context_limit,
            });
        }
        let latency = limits.limits().max_latency_millis;
        let deadline_millis = if latency == i64::MAX {
            None
        } else {
            Some(
                started_at_millis
                    .checked_add(latency)
                    .ok_or(DurablePolicyBindingRefusal::DeadlineOverflow)?,
            )
        };
        let invocation = LiveInvocationId::derive(seed);
        let deployment_root = execution.deployment_root().digest().to_owned();
        let instance_root = execution.instance_root().digest().to_owned();
        let providers = (0..policy.len())
            .map(|index| {
                let slot = policy.slot(index).expect("checked policy index");
                format!(
                    "{{\"id\":{},\"authorized\":{}}}",
                    quote_json(&slot.id),
                    slot.authorized
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let limit_values = limits.limits();
        let canonical = format!(
            "{{\"schema\":\"semaprax.generic-model-policy.binding.v2\",\"invocation\":{},\"deployment_root\":{},\"instance_root\":{},\"deployment_policy\":{},\"providers\":[{}],\"limits\":{{\"calls\":{},\"retries\":{},\"providers\":{},\"context\":{},\"output\":{},\"aggregate\":{},\"cost\":{},\"latency\":{}}},\"deadline_millis\":{}}}",
            quote_json(invocation.digest()),
            quote_json(&deployment_root),
            quote_json(&instance_root),
            quote_json(&seed.deployment_policy),
            providers,
            limit_values.max_calls,
            limit_values.max_retries,
            limit_values.max_providers,
            limit_values.max_context_tokens,
            limit_values.max_output_tokens,
            limit_values.max_aggregate_tokens,
            limit_values.max_cost_micros,
            limit_values.max_latency_millis,
            deadline_millis.map_or_else(|| "null".to_owned(), |value| value.to_string()),
        );
        Ok(Self {
            invocation,
            deployment_root,
            instance_root,
            deployment_policy: seed.deployment_policy.clone(),
            task_digest,
            interaction_schema_digest: seed.interaction_schema_digest.clone(),
            task_budget: seed.budget,
            policy,
            model_selections: Some(selected),
            limits,
            retained_byte_budget: Some(retained_byte_budget),
            deadline_millis,
            digest: digest(BINDING_DOMAIN, canonical.as_bytes()),
        })
    }

    /// Pre-admits one deployment profile's ordered provider policy and limits
    /// with the same order, primary, limit and context checks [`Self::bind`]
    /// applies, before any execution root or seed exists. Runtime routing
    /// (MR-09) uses it so an incompatible profile is never offered to a
    /// router; `bind` still rechecks every fact at dispatch.
    pub fn admit_profile(
        deployment: &BoundAgentDeployment,
        policy: &ProviderPolicy,
        limits: EffectiveModelBudget,
    ) -> Result<(), DurablePolicyBindingRefusal> {
        let Some(primary) = policy.primary() else {
            return Err(DurablePolicyBindingRefusal::EmptyProviderPolicy);
        };
        if !primary.authorized {
            return Err(DurablePolicyBindingRefusal::UnauthorizedPrimary);
        }
        let selected = deployment.model_selections();
        if policy.len() != selected.len()
            || selected.iter().enumerate().any(|(index, selected)| {
                policy
                    .slot(index)
                    .is_none_or(|slot| slot.id != selected.provider_id())
            })
        {
            return Err(DurablePolicyBindingRefusal::ProviderOrderMismatch);
        }
        check_retained_limits(deployment, limits)?;
        let context = selected
            .iter()
            .map(|item| item.max_context_tokens())
            .min()
            .ok_or(DurablePolicyBindingRefusal::EmptyProviderPolicy)?;
        if limits.limits().max_context_tokens > context {
            return Err(DurablePolicyBindingRefusal::ContextLimitExceedsDeployment {
                requested: limits.limits().max_context_tokens,
                max: context,
            });
        }
        Ok(())
    }

    #[must_use]
    pub fn invocation(&self) -> &LiveInvocationId {
        &self.invocation
    }
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
    #[must_use]
    pub fn deployment_root(&self) -> &str {
        &self.deployment_root
    }
    #[must_use]
    pub fn instance_root(&self) -> &str {
        &self.instance_root
    }
    #[must_use]
    pub fn deployment_policy(&self) -> &str {
        &self.deployment_policy
    }
    #[must_use]
    pub fn deadline_millis(&self) -> Option<i64> {
        self.deadline_millis
    }
    pub(crate) fn policy(&self) -> &ProviderPolicy {
        &self.policy
    }
    pub(crate) fn model_selections(&self) -> Option<&[DeploymentModelSelection]> {
        self.model_selections.as_deref()
    }
    pub(crate) fn limits(&self) -> EffectiveModelBudget {
        self.limits
    }

    /// Narrows the byte limits already retained in this binding. The caller
    /// supplies an invocation ceiling, never a replacement authority.
    pub fn narrow_byte_budget(
        &self,
        invocation: DurableByteBudget,
    ) -> Result<DurableByteBudget, DurablePolicyBindingRefusal> {
        let retained = self
            .retained_byte_budget
            .ok_or(DurablePolicyBindingRefusal::ExecutionRootMismatch)?;
        for (dimension, requested, maximum) in [
            (
                "max_provider_request_bytes",
                invocation.max_request_bytes,
                retained.max_request_bytes,
            ),
            (
                "max_provider_response_bytes",
                invocation.max_response_bytes,
                retained.max_response_bytes,
            ),
            (
                "max_total_provider_input_bytes",
                invocation.max_total_input_bytes,
                retained.max_total_input_bytes,
            ),
            (
                "max_total_provider_output_bytes",
                invocation.max_total_output_bytes,
                retained.max_total_output_bytes,
            ),
        ] {
            if requested > maximum {
                return Err(DurablePolicyBindingRefusal::RetainedByteLimitMismatch { dimension });
            }
        }
        Ok(invocation)
    }

    /// Checks the actual canonical SDK envelope, rather than the logical
    /// task/observation bytes or a host-supplied plan field. The returned
    /// budget is suitable for a V3 byte ledger to reserve this exact input
    /// and the adapter's declared response capacity before factory/start.
    pub fn byte_budget_for_adapter(
        &self,
        invocation: DurableByteBudget,
        adapter_request: &AdapterRequest,
    ) -> Result<DurableByteBudget, DurablePolicyBindingRefusal> {
        let budget = self.narrow_byte_budget(invocation)?;
        let request_bytes = u64::try_from(adapter_request.request_bytes.len()).map_err(|_| {
            DurablePolicyBindingRefusal::AdapterRequestBytesExceeded {
                dimension: "max_provider_request_bytes",
            }
        })?;
        let response_bytes = u64::try_from(adapter_request.max_response_bytes).map_err(|_| {
            DurablePolicyBindingRefusal::AdapterRequestBytesExceeded {
                dimension: "max_provider_response_bytes",
            }
        })?;
        if request_bytes > budget.max_request_bytes || request_bytes > budget.max_total_input_bytes
        {
            return Err(DurablePolicyBindingRefusal::AdapterRequestBytesExceeded {
                dimension: "max_provider_request_bytes",
            });
        }
        if response_bytes > budget.max_response_bytes
            || response_bytes > budget.max_total_output_bytes
        {
            return Err(DurablePolicyBindingRefusal::AdapterRequestBytesExceeded {
                dimension: "max_provider_response_bytes",
            });
        }
        Ok(budget)
    }

    pub(crate) fn request_matches(
        &self,
        schema: &CompiledInteractionSchema,
        request: &crate::live_invocation::model_invoke::ModelInvocationRequest,
    ) -> bool {
        request.proposal_grammar_digest == self.interaction_schema_digest
            && schema.schema().digest() == self.interaction_schema_digest
            && input_digest(&request.task) == self.task_digest
            && request.effective_budget >= 0
            && request.effective_budget <= self.task_budget
    }

    /// Refuses a continuation whose caller clock already crossed the original
    /// absolute deadline.  This does not rebase a duration on recovery.
    pub(crate) fn check_clock(&self, clock: &dyn InvocationClock) -> bool {
        self.deadline_millis
            .is_none_or(|deadline| clock.now_millis() < deadline)
    }

    #[cfg(test)]
    pub(crate) fn fixture(
        invocation: LiveInvocationId,
        policy: ProviderPolicy,
        limits: EffectiveModelBudget,
        task: &[u8],
        interaction_schema_digest: &str,
        task_budget: i64,
    ) -> Self {
        Self {
            invocation,
            deployment_root: "sha256:test-deployment-root".into(),
            instance_root: "sha256:test-instance-root".into(),
            deployment_policy: "sha256:test-policy".into(),
            task_digest: input_digest(task),
            interaction_schema_digest: interaction_schema_digest.into(),
            task_budget,
            policy,
            model_selections: None,
            limits,
            retained_byte_budget: Some(DurableByteBudget {
                max_request_bytes: u64::MAX,
                max_response_bytes: u64::MAX,
                max_total_input_bytes: u64::MAX,
                max_total_output_bytes: u64::MAX,
            }),
            deadline_millis: None,
            digest: "sha256:test-binding".into(),
        }
    }

    #[cfg(test)]
    pub(crate) fn fixture_with_deadline(mut binding: Self, deadline_millis: i64) -> Self {
        binding.deadline_millis = Some(deadline_millis);
        binding
    }
}

fn root_facts(
    source: &str,
) -> Result<serde_json::Map<String, serde_json::Value>, DurablePolicyBindingRefusal> {
    serde_json::from_str::<serde_json::Value>(source)
        .ok()
        .and_then(|root| root.get("facts")?.as_object().cloned())
        .ok_or(DurablePolicyBindingRefusal::ExecutionRootMismatch)
}

fn input_digest(bytes: &[u8]) -> String {
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(bytes))
    )
}

/// The AgentDefinition v2 and AgentDeployment v1 documents retain these
/// seven policy dimensions under their original source names. Aggregate token
/// exposure has no same-unit source field (the documents retain aggregate
/// provider *bytes*), so this function deliberately does not reinterpret
/// bytes as tokens.
fn check_retained_limits(
    deployment: &BoundAgentDeployment,
    effective: EffectiveModelBudget,
) -> Result<(), DurablePolicyBindingRefusal> {
    let source = retained_limits(
        deployment.semantic_definition().canonical_json(),
        "ceilings",
    )?;
    let bound = retained_limits(deployment.deployment().canonical_json(), "limits")?;
    let limits = effective.limits();
    let checks = [
        (
            "max_calls",
            u64::from(limits.max_calls),
            "max_provider_attempts",
        ),
        (
            "max_retries",
            u64::from(limits.max_retries),
            "max_retries_per_turn",
        ),
        (
            "max_providers",
            u64::from(limits.max_providers),
            "max_provider_attempts",
        ),
        (
            "max_context_tokens",
            limits.max_context_tokens,
            "max_reported_model_input_tokens",
        ),
        (
            "max_output_tokens",
            limits.max_output_tokens,
            "max_reported_model_output_tokens",
        ),
        (
            "max_cost_micros",
            limits.max_cost_micros as u64,
            "max_usd_microunits",
        ),
        (
            "max_latency_millis",
            limits.max_latency_millis as u64,
            "max_elapsed_ms",
        ),
    ];
    for (dimension, actual, retained_key) in checks {
        let source_limit = source
            .get(retained_key)
            .and_then(serde_json::Value::as_u64)
            .ok_or(DurablePolicyBindingRefusal::ExecutionRootMismatch)?;
        let bound_limit = bound
            .get(retained_key)
            .and_then(serde_json::Value::as_u64)
            .ok_or(DurablePolicyBindingRefusal::ExecutionRootMismatch)?;
        if actual > source_limit || actual > bound_limit {
            return Err(DurablePolicyBindingRefusal::RetainedLimitMismatch { dimension });
        }
    }
    Ok(())
}

/// The byte dimensions are exact shared units in both retained documents, so
/// V3 can take their minimum without treating bytes as token evidence.
fn retained_byte_budget(
    deployment: &BoundAgentDeployment,
) -> Result<DurableByteBudget, DurablePolicyBindingRefusal> {
    let source = retained_limits(
        deployment.semantic_definition().canonical_json(),
        "ceilings",
    )?;
    let bound = retained_limits(deployment.deployment().canonical_json(), "limits")?;
    let limit = |key: &'static str| {
        let source = source
            .get(key)
            .and_then(serde_json::Value::as_u64)
            .ok_or(DurablePolicyBindingRefusal::ExecutionRootMismatch)?;
        let bound = bound
            .get(key)
            .and_then(serde_json::Value::as_u64)
            .ok_or(DurablePolicyBindingRefusal::ExecutionRootMismatch)?;
        Ok(source.min(bound))
    };
    Ok(DurableByteBudget {
        max_request_bytes: limit("max_provider_request_bytes")?,
        max_response_bytes: limit("max_provider_response_bytes")?,
        max_total_input_bytes: limit("max_total_provider_input_bytes")?,
        max_total_output_bytes: limit("max_total_provider_output_bytes")?,
    })
}

fn retained_limits(
    source: &str,
    field: &str,
) -> Result<serde_json::Map<String, serde_json::Value>, DurablePolicyBindingRefusal> {
    serde_json::from_str::<serde_json::Value>(source)
        .ok()
        .and_then(|document| document.get(field)?.as_object().cloned())
        .ok_or(DurablePolicyBindingRefusal::ExecutionRootMismatch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live_invocation::{LiveInvocationId, LiveInvocationSeed};
    use crate::model_budget_policy::{intersect, ModelBudgetLimits, ProviderSlot};

    fn binding() -> DurablePolicyBinding {
        let mut limits = ModelBudgetLimits::unbounded();
        limits.max_calls = 1;
        let limits = intersect(limits, limits, limits).unwrap();
        DurablePolicyBinding::fixture(
            LiveInvocationId::derive(&LiveInvocationSeed {
                program_root: "sha256:program".into(),
                deployment_policy: "sha256:test-policy".into(),
                task: b"task".to_vec(),
                budget: 1,
                interaction_schema_digest: "sha256:schema".into(),
                approved_providers: vec!["primary".into()],
            }),
            ProviderPolicy::new(vec![ProviderSlot::authorized("primary")]),
            limits,
            b"task",
            "sha256:schema",
            1,
        )
    }

    #[test]
    fn fixture_byte_projection_accepts_narrowing_and_checks_exact_envelope() {
        let binding = binding();
        let budget = DurableByteBudget {
            max_request_bytes: 4,
            max_response_bytes: 5,
            max_total_input_bytes: 8,
            max_total_output_bytes: 10,
        };
        let request = AdapterRequest {
            request_bytes: b"four".to_vec(),
            max_response_bytes: 5,
        };
        assert_eq!(
            binding.byte_budget_for_adapter(budget, &request),
            Ok(budget)
        );
        let oversized = AdapterRequest {
            request_bytes: b"five!".to_vec(),
            max_response_bytes: 5,
        };
        assert!(matches!(
            binding.byte_budget_for_adapter(budget, &oversized),
            Err(DurablePolicyBindingRefusal::AdapterRequestBytesExceeded { .. })
        ));
    }
}
