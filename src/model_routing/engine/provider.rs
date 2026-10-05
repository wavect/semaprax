//! External decision-provider abstraction: the invoker trait, per-adapter
//! calibration profile and the experimental-mode enablement gate.

use super::call::{CallMetadata, IdentityKind, ScoreKind};
use super::json;
use super::model_profile::{AdapterIdentity, InstanceConfig, ModelProfile};
use super::render::RENDERER_V2;
use super::request::DecisionRequest;
use super::route::{TaskFamily, TaskFeatures};
use serde_json::{json, Value};
use std::collections::BTreeSet;

/// What one `decision.evaluate` invocation produced. A returned payload is
/// untrusted data; the router validates it before any use.
#[derive(Clone, Debug, PartialEq)]
pub enum DecisionCall {
    /// Result payload and injected elapsed time. `call` is the typed v2 call
    /// metadata (MR-03) taken from the payload's `call` member by the host
    /// invoker; `None` for v1 results (and fixtures that leave it in `result`).
    Answered {
        result: Value,
        elapsed_ms: u64,
        call: Option<CallMetadata>,
    },
    Unavailable,
    Timeout,
}

/// The MR-15 decision-adapter boundary: implemented by a host over its
/// approved adapters (and by fixtures in tests). The core never performs an
/// environment, credential or network lookup itself; whatever transport a
/// host uses lives behind this trait, and the answer is untrusted data.
pub trait DecisionInvoker {
    fn evaluate(&mut self, request: &DecisionRequest) -> DecisionCall;

    /// `decision.evaluate` contract versions this adapter negotiated. The
    /// router sends `model-route/v2` only when 2 is listed.
    fn decision_versions(&self) -> Vec<u32> {
        vec![1]
    }
}

/// The wire version a provider is consulted with: v2 only when the adapter
/// negotiated it and the model profile (if any) names the v2 renderer.
pub fn wire_version<I: ?Sized + DecisionInvoker>(profile: &ProviderProfile, invoker: &I) -> u32 {
    let v2 = invoker.decision_versions().contains(&2)
        && profile
            .model_profile
            .as_ref()
            .is_none_or(|m| m.renderer == RENDERER_V2);
    if v2 {
        2
    } else {
        1
    }
}

/// Adapter/task calibration. The confidence threshold belongs here, never in
/// a universal constant; a score is evidence for fallback decisions only and
/// never grants a destination.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProviderProfile {
    /// Adapter implementation id (descriptor `provider.id`).
    pub provider_id: String,
    pub model_id: String,
    pub checkpoint: String,
    /// Deprecated alias (MR-02), documented old behavior kept: minimum score
    /// of the chosen option whatever the score kind; a missing score fails
    /// closed. `None` means scores are ignored. Prefer `min_option_mass`.
    pub min_confidence: Option<f64>,
    /// Minimum chosen-option mass. Applies only to `option_distribution`
    /// scores; it is never a probability of task success.
    pub min_option_mass: Option<f64>,
    /// Declared feature ranges; outside them the provider is not consulted.
    pub max_context_tokens: Option<u64>,
    pub supported_families: Option<BTreeSet<TaskFamily>>,
    /// MR-15: adapter implementation version (descriptor `adapter.version`).
    pub adapter_version: Option<String>,
    /// MR-15: declared model profile; capabilities are checked before inference.
    pub model_profile: Option<ModelProfile>,
    /// MR-15: configured instance (endpoint/worker and secret references).
    pub instance: Option<InstanceConfig>,
}

impl ProviderProfile {
    /// Compose the three MR-15 identities into one routing profile.
    pub fn configured(
        adapter: AdapterIdentity,
        profile: ModelProfile,
        instance: InstanceConfig,
    ) -> Self {
        Self {
            provider_id: adapter.provider_id,
            model_id: profile.model.clone(),
            checkpoint: profile.checkpoint_label(),
            adapter_version: Some(adapter.adapter_version),
            model_profile: Some(profile),
            instance: Some(instance),
            ..Self::default()
        }
    }

    /// The adapter/profile/instance scope bound into cache and evidence keys;
    /// `Null` for a legacy profile, so v1 keys keep their bytes.
    pub fn scope_json(&self) -> Value {
        if self.adapter_version.is_none() && self.model_profile.is_none() && self.instance.is_none()
        {
            return Value::Null;
        }
        json!({"adapter": self.adapter_version.as_ref().map(|v| format!("{}@{v}", self.provider_id)),
               "profile": self.model_profile.as_ref().map(ModelProfile::digest),
               "instance": self.instance.as_ref().map(InstanceConfig::digest)})
    }

    pub fn scope_digest(&self) -> String {
        match self.scope_json() {
            Value::Null => String::new(),
            v => json::digest("semaprax.decision.provider-scope.v1", &v),
        }
    }

    /// The adapter may answer `score_kind: none`.
    pub fn scoreless(&self) -> bool {
        self.model_profile.as_ref().is_some_and(|m| m.scoreless)
    }

    /// Score kinds a result may carry under this profile.
    pub fn admits_score_kind(&self, kind: ScoreKind) -> bool {
        match &self.model_profile {
            Some(m) => m.admits_score_kind(kind),
            None => kind != ScoreKind::None,
        }
    }

    /// Capability preflight before any inference (MR-15): option count,
    /// rendered state size and input modalities against the declared profile.
    pub fn admits_request(
        &self,
        options: usize,
        state_bytes: usize,
        modalities: &BTreeSet<super::route_v2::Modality>,
    ) -> Result<(), String> {
        let Some(m) = &self.model_profile else {
            return Ok(());
        };
        if options > m.max_options as usize {
            return Err(format!(
                "{options} options exceed the profile's max_options {}",
                m.max_options
            ));
        }
        if state_bytes > m.max_state_bytes as usize {
            return Err(format!(
                "{state_bytes} state bytes exceed the profile's max_state_bytes {}",
                m.max_state_bytes
            ));
        }
        if let Some(x) = modalities.iter().find(|x| !m.modalities.contains(x)) {
            return Err(format!("input modality `{}` is not supported", x.as_str()));
        }
        Ok(())
    }

    /// Verify a call's answering identity against the qualified profile
    /// (MR-03). Unknown identity never verifies.
    pub fn verify_identity(&self, call: &CallMetadata) -> Result<(), String> {
        let Some(m) = &self.model_profile else {
            return Err(
                "no declared model profile to verify the answering identity against".into(),
            );
        };
        let model_ok = call.answering_model.as_deref() == Some(m.model.as_str());
        let ok = match m.identity_kind {
            IdentityKind::ImmutableCheckpoint => {
                call.identity_kind == IdentityKind::ImmutableCheckpoint
                    && call.checkpoint.is_some()
                    && call.checkpoint == m.checkpoint
                    && (call.answering_model.is_none() || model_ok)
            }
            IdentityKind::MutableService => {
                model_ok
                    && matches!(
                        call.identity_kind,
                        IdentityKind::MutableService | IdentityKind::ImmutableCheckpoint
                    )
            }
            IdentityKind::LocalDeclared => {
                call.identity_kind == IdentityKind::LocalDeclared
                    && (model_ok
                        || (call.answering_model.is_none()
                            && call.requested_model.as_deref() == Some(m.model.as_str())))
            }
            IdentityKind::Unknown => false,
        };
        if ok {
            Ok(())
        } else {
            Err(format!(
                "answering identity {}/{:?}/{:?} does not match the qualified {} profile `{}`",
                call.identity_kind.as_str(),
                call.answering_model,
                call.checkpoint,
                m.identity_kind.as_str(),
                m.model
            ))
        }
    }

    pub fn covers(&self, f: &TaskFeatures) -> bool {
        self.max_context_tokens
            .is_none_or(|m| f.estimated_context_tokens <= m)
            && self
                .supported_families
                .as_ref()
                .is_none_or(|s| s.contains(&f.task_family))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GateStatus {
    NotEvaluated,
    Passed { evidence: String },
    Failed,
}

/// Evaluation gate for automatic selection of a learned provider for one
/// registered task and profile. Default is `NotEvaluated` (rules decide).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnablementGate {
    pub task: String,
    pub profile: String,
    pub status: GateStatus,
}

impl EnablementGate {
    pub fn not_evaluated(task: &str, profile: &str) -> Self {
        Self {
            task: task.into(),
            profile: profile.into(),
            status: GateStatus::NotEvaluated,
        }
    }

    fn passed_for(&self, task: &str, profile: &str) -> bool {
        self.task == task
            && self.profile == profile
            && matches!(&self.status, GateStatus::Passed { evidence } if !evidence.is_empty())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderMode {
    /// User configuration explicitly selected the provider (experimental).
    Explicit,
    /// Automatic selection; requires a passed evaluation gate.
    Auto,
}

/// A provider the host attached for one decision. `I` is the host's invoker
/// type; the default is the core trait object. A host whose own invoker trait
/// carries its transport envelope implements [`DecisionInvoker`] for its trait
/// object and names `ConfiguredProvider<'a, dyn HostInvoker + 'a>`.
pub struct ConfiguredProvider<'a, I: ?Sized + 'a = dyn DecisionInvoker + 'a> {
    pub profile: ProviderProfile,
    pub invoker: &'a mut I,
    pub mode: ProviderMode,
    pub gate: EnablementGate,
}

impl<I: ?Sized + DecisionInvoker> ConfiguredProvider<'_, I> {
    /// Negotiated wire version for this provider (see [`wire_version`]).
    pub fn wire_version(&self) -> u32 {
        wire_version(&self.profile, &*self.invoker)
    }
}

impl<I: ?Sized> ConfiguredProvider<'_, I> {
    pub fn enabled(&self, task: &str) -> bool {
        match self.mode {
            ProviderMode::Explicit => true,
            ProviderMode::Auto => self.gate.passed_for(task, &self.profile.provider_id),
        }
    }

    /// Visible status label.
    pub fn status(&self, task: &str) -> &'static str {
        if self.gate.passed_for(task, &self.profile.provider_id) {
            "evaluated"
        } else if self.mode == ProviderMode::Explicit {
            "experimental"
        } else {
            "rules (learned provider not evaluated)"
        }
    }
}
