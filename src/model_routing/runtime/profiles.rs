//! Host-owned runtime routing configuration: a finite set of approved,
//! pre-bound concrete deployment profiles for one semantic definition.

use std::collections::BTreeSet;

use serde_json::{json, Value};

use super::error::RuntimeRoutingError;
use crate::agent_deployment::{bind_agent_deployment, BoundAgentDeployment};
use crate::model_budget_policy::{
    DurablePolicyBinding, EffectiveModelBudget, ProviderPolicy, ProviderSlot,
};
use crate::model_routing::engine::json;
use crate::model_routing::engine::{Destination, Modality, ModelPlan, RoutePolicy};

/// At most this many profiles in one route.
pub const MAX_PROFILES: usize = 64;
const MAX_ID_BYTES: usize = 128;

/// Logical model metadata the router may compare. It is host-owned,
/// untrusted for authority and may drift (aliases, estimates) without
/// changing what an in-flight invocation is bound to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileModel {
    /// Logical model name or alias; never an endpoint or credential.
    pub alias: String,
    pub destination: Destination,
    pub structured_output: bool,
    pub tools: bool,
    pub modalities: BTreeSet<Modality>,
    pub max_context: u64,
    pub est_cost_micros: u64,
    pub est_latency_ms: u64,
    pub strength_rank: u32,
}

impl ProfileModel {
    fn to_json(&self) -> Value {
        let dest = match &self.destination {
            Destination::Local => json!({"kind": "local"}),
            Destination::Remote { origin } => json!({"kind": "remote", "origin": origin}),
        };
        json!({
            "alias": self.alias, "destination": dest,
            "structured_output": self.structured_output, "tools": self.tools,
            "modalities": self.modalities.iter().map(|m| m.as_str()).collect::<Vec<_>>(),
            "max_context": self.max_context, "est_cost_micros": self.est_cost_micros,
            "est_latency_ms": self.est_latency_ms, "strength_rank": self.strength_rank,
        })
    }
}

/// One host-approved profile before validation.
#[derive(Clone, Debug)]
pub struct ProfileSpec {
    /// The profile id the router may name.
    pub id: String,
    /// The concrete AgentDeployment v1 document for the route's definition.
    pub deployment_source: String,
    /// Host failover order. `None` authorizes the deployment's exact model
    /// order; a supplied policy must match it exactly.
    pub provider_policy: Option<ProviderPolicy>,
    /// Effective per-invocation limits; never above the retained ceilings.
    pub limits: EffectiveModelBudget,
    pub model: ProfileModel,
}

/// One validated profile: a bound deployment plus its pre-admitted policy.
pub struct ApprovedProfile {
    id: String,
    deployment_source: String,
    deployment: BoundAgentDeployment,
    policy: ProviderPolicy,
    limits: EffectiveModelBudget,
    model: ProfileModel,
}

impl ApprovedProfile {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn deployment_source(&self) -> &str {
        &self.deployment_source
    }
    pub fn deployment(&self) -> &BoundAgentDeployment {
        &self.deployment
    }
    /// The bound deployment digest: the profile's authority identity.
    pub fn deployment_digest(&self) -> &str {
        self.deployment.digest()
    }
    pub fn provider_policy(&self) -> &ProviderPolicy {
        &self.policy
    }
    pub fn limits(&self) -> EffectiveModelBudget {
        self.limits
    }
    pub fn model(&self) -> &ProfileModel {
        &self.model
    }

    /// The candidate the decision core sees: profile id plus logical
    /// metadata, nothing else.
    pub(crate) fn plan(&self) -> ModelPlan {
        ModelPlan {
            id: self.id.clone(),
            destination: self.model.destination.clone(),
            structured_output: self.model.structured_output,
            tools: self.model.tools,
            max_context: self.model.max_context,
            est_cost_micros: self.model.est_cost_micros,
            est_latency_ms: self.model.est_latency_ms,
            strength_rank: self.model.strength_rank,
            descriptor: Default::default(),
        }
    }

    fn to_json(&self) -> Value {
        let slots: Vec<Value> = (0..self.policy.len())
            .filter_map(|i| self.policy.slot(i))
            .map(|s| json!([s.id, s.authorized]))
            .collect();
        let l = self.limits.limits();
        json!({
            "id": self.id, "deployment": self.deployment.digest(), "providers": slots,
            "limits": {"calls": l.max_calls, "retries": l.max_retries, "providers": l.max_providers,
                       "context": l.max_context_tokens, "output": l.max_output_tokens,
                       "aggregate": l.max_aggregate_tokens, "cost": l.max_cost_micros,
                       "latency": l.max_latency_millis},
            "model": self.model.to_json(),
        })
    }
}

/// The approved profile catalog for one semantic definition and one routing
/// policy. Its digest binds every profile's deployment, order, limits and
/// metadata; a retained route records it.
pub struct ApprovedProfileSet {
    definition_digest: String,
    profiles: Vec<ApprovedProfile>,
    route_policy: RoutePolicy,
    digest: String,
}

fn bounded_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID_BYTES
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-:".contains(&b))
}

impl ApprovedProfileSet {
    /// Validates every profile with the current deployment checks before it
    /// can be offered to a router. Binding contacts no provider.
    pub fn approve(
        definition_source: &str,
        specs: Vec<ProfileSpec>,
        route_policy: RoutePolicy,
    ) -> Result<Self, RuntimeRoutingError> {
        if specs.is_empty() || specs.len() > MAX_PROFILES {
            return Err(RuntimeRoutingError::InvalidConfig(format!(
                "a route needs 1..={MAX_PROFILES} profiles"
            )));
        }
        let mut seen = BTreeSet::new();
        let mut profiles = Vec::with_capacity(specs.len());
        let mut definition_digest: Option<String> = None;
        for spec in specs {
            if !bounded_id(&spec.id) || !seen.insert(spec.id.clone()) {
                return Err(RuntimeRoutingError::InvalidConfig(format!(
                    "profile id `{}` is empty, unbounded, non-canonical or duplicated",
                    spec.id.chars().take(MAX_ID_BYTES).collect::<String>()
                )));
            }
            if spec.model.alias.is_empty()
                || spec.model.alias.len() > MAX_ID_BYTES
                || !spec.model.alias.is_ascii()
            {
                return Err(RuntimeRoutingError::InvalidConfig(format!(
                    "profile `{}` alias must be bounded ASCII",
                    spec.id
                )));
            }
            let deployment = bind_agent_deployment(definition_source, &spec.deployment_source)
                .map_err(|d| RuntimeRoutingError::from_diagnostics(&spec.id, &d))?;
            let semantic = deployment.semantic_definition().digest().to_owned();
            match &definition_digest {
                None => definition_digest = Some(semantic),
                Some(first) if *first != semantic => {
                    return Err(RuntimeRoutingError::SemanticMismatch { profile: spec.id })
                }
                Some(_) => {}
            }
            let selections = deployment.model_selections();
            let policy = spec.provider_policy.clone().unwrap_or_else(|| {
                ProviderPolicy::new(
                    selections
                        .iter()
                        .map(|row| ProviderSlot::authorized(row.provider_id()))
                        .collect(),
                )
            });
            DurablePolicyBinding::admit_profile(&deployment, &policy, spec.limits).map_err(
                |refusal| RuntimeRoutingError::PolicyBinding {
                    profile: spec.id.clone(),
                    refusal,
                },
            )?;
            check_metadata(&spec, &deployment)?;
            profiles.push(ApprovedProfile {
                id: spec.id,
                deployment_source: spec.deployment_source,
                deployment,
                policy,
                limits: spec.limits,
                model: spec.model,
            });
        }
        profiles.sort_by(|a, b| a.id.cmp(&b.id));
        let definition_digest = definition_digest.expect("at least one profile");
        let digest = json::digest(
            "semaprax.runtime-profile-set.v1",
            &json!({
                "definition": definition_digest,
                "route_policy": route_policy.digest(),
                "profiles": profiles.iter().map(ApprovedProfile::to_json).collect::<Vec<_>>(),
            }),
        );
        Ok(Self {
            definition_digest,
            profiles,
            route_policy,
            digest,
        })
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn definition_digest(&self) -> &str {
        &self.definition_digest
    }
    pub fn route_policy(&self) -> &RoutePolicy {
        &self.route_policy
    }
    /// Profiles sorted by id.
    pub fn profiles(&self) -> &[ApprovedProfile] {
        &self.profiles
    }
    pub fn profile(&self, id: &str) -> Option<&ApprovedProfile> {
        self.profiles.iter().find(|p| p.id == id)
    }
    /// Looks a retained route up by its authority identity, not by alias or id.
    pub fn by_deployment(&self, digest: &str) -> Option<&ApprovedProfile> {
        self.profiles
            .iter()
            .find(|p| p.deployment.digest() == digest)
    }
}

/// Logical metadata may never claim more than the deployment admits: a
/// larger context window than any selected model, or a local destination for
/// a deployment that selects a non-local model.
fn check_metadata(
    spec: &ProfileSpec,
    deployment: &BoundAgentDeployment,
) -> Result<(), RuntimeRoutingError> {
    let selections = deployment.model_selections();
    let context = selections
        .iter()
        .map(|row| row.max_context_tokens())
        .min()
        .unwrap_or(0);
    if spec.model.max_context > context {
        return Err(RuntimeRoutingError::InvalidConfig(format!(
            "profile `{}` claims a {}-token context; its deployment admits {context}",
            spec.id, spec.model.max_context
        )));
    }
    let document: Value = serde_json::from_str(deployment.deployment().canonical_json())
        .map_err(|_| RuntimeRoutingError::InvalidConfig("deployment is not JSON".into()))?;
    let remote = document["models"]
        .as_array()
        .is_some_and(|rows| rows.iter().any(|row| row["locality"] != "local"));
    if remote && spec.model.destination == Destination::Local {
        return Err(RuntimeRoutingError::InvalidConfig(format!(
            "profile `{}` declares a local destination for a non-local deployment model",
            spec.id
        )));
    }
    let tools = !deployment.granted_capabilities().is_empty();
    if spec.model.tools && !tools {
        return Err(RuntimeRoutingError::InvalidConfig(format!(
            "profile `{}` claims tools but its deployment grants none",
            spec.id
        )));
    }
    Ok(())
}
