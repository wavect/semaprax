//! MR-15: the decision provider's routing profile, built from the adopted
//! descriptor and host configuration only (no vendor/model-name branching),
//! and an opener that starts the selected `decision.evaluate` adapter through
//! the normal resolve/trust/lock/negotiate path.

use super::cli::{start, RunOptions};
use super::snapshot::Snapshot;
use crate::cli::Environment;
use crate::contract::{CapabilityKind, Descriptor};
use crate::decision::model_profile::{MODEL_PROFILE_FIELD, MODEL_PROFILE_VAR};
use crate::decision::{
    AdapterIdentity, HostDecisionInvoker, InstanceConfig, ModelProfile, ProviderProfile,
};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::host::{AdapterManager, HostConfig};
use crate::profile::config::CapabilityConfig;
use crate::profile::{self, lock, BindingState, HarnessConfig};
use std::path::Path;

/// Host threshold for chosen-option mass (`option_distribution` only).
pub const MIN_OPTION_MASS_VAR: &str = "SEMAPRAX_HARNESS_MIN_OPTION_MASS";

fn cfg_str<'a>(cfg: &'a CapabilityConfig, k: &str) -> Option<&'a str> {
    cfg.config.get(k).and_then(|v| v.as_str())
}

/// Build the routing profile. Without a configured model profile this is the
/// legacy profile (unchanged keys); with one, the three MR-15 identities are
/// composed and a malformed profile is refused (`SPX-HPJ020`) before any
/// inference.
pub fn decision_profile(
    descriptor: &Descriptor,
    cfg: &CapabilityConfig,
    env: &Environment,
) -> HarnessResult<ProviderProfile> {
    let raw = cfg_str(cfg, MODEL_PROFILE_FIELD)
        .map(str::to_string)
        .or_else(|| env.vars.get(MODEL_PROFILE_VAR).cloned());
    let mut p = match raw {
        None => ProviderProfile {
            provider_id: descriptor.provider_id.clone(),
            model_id: descriptor.provider_id.clone(),
            checkpoint: descriptor.adapter_version.clone(),
            ..ProviderProfile::default()
        },
        Some(text) => {
            let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
                HarnessDiagnostic::new("SPX-HPJ020", format!("model profile is not JSON: {e}"))
            })?;
            let model = ModelProfile::from_json(&v)?;
            let instance = InstanceConfig {
                instance_id: cfg_str(cfg, "instance_id").unwrap_or("default").to_string(),
                endpoint: cfg_str(cfg, "endpoint").map(str::to_string),
                secret_refs: descriptor.permissions.secrets.clone(),
            };
            ProviderProfile::configured(
                AdapterIdentity {
                    provider_id: descriptor.provider_id.clone(),
                    adapter_version: descriptor.adapter_version.clone(),
                },
                model,
                instance,
            )
        }
    };
    if let Some(m) = env.vars.get(MIN_OPTION_MASS_VAR) {
        match m.parse::<f64>() {
            Ok(x) if x.is_finite() && (0.0..=1.0).contains(&x) => p.min_option_mass = Some(x),
            _ => {
                return Err(HarnessDiagnostic::new(
                    "SPX-HPJ020",
                    format!("{MIN_OPTION_MASS_VAR} must be a number in [0,1]"),
                ))
            }
        }
    }
    Ok(p)
}

/// A started decision adapter and its routing profile.
pub struct OpenedDecision {
    pub invoker: HostDecisionInvoker,
    pub profile: ProviderProfile,
    pub binding: crate::contract::ProjectBinding,
    pub lock_digest: String,
    /// Keeps the adapter manager (and so the process) alive.
    pub _manager: AdapterManager,
}

/// Resolve the project's profile (same adopt/trust/lock flow as `run`) and
/// start its selected `decision.evaluate` provider.
pub fn open_decision(
    env: &Environment,
    project: &Path,
    python: Option<&Path>,
    cache: &Path,
) -> HarnessResult<OpenedDecision> {
    let snapshot = Snapshot::capture(project)?;
    let config = HarnessConfig::load(&snapshot.root)?;
    let res = profile::resolve_project(env, &snapshot.root)?;
    if let Some(e) = res.unmet.first() {
        return Err(e.clone());
    }
    match lock::load(&snapshot.root)? {
        Some(l) => lock::verify_frozen(&l, &res.profile)?,
        None => lock::write(&snapshot.root, &res.profile)?,
    }
    let kind = CapabilityKind::DecisionEvaluate;
    let l = res
        .launches
        .get(&kind)
        .filter(|_| {
            res.profile
                .binding(kind)
                .is_some_and(|b| b.state == BindingState::Selected)
        })
        .ok_or_else(|| {
            HarnessDiagnostic::new(
                "SPX-HPD090",
                "the project has no selected decision.evaluate provider",
            )
        })?;
    let profile = decision_profile(&l.descriptor, &config.capability(kind), env)?;
    let o = RunOptions {
        task: None,
        proposal: None,
        apply_policy: None,
        python: python.map(Path::to_path_buf),
        node: None,
        compiler: None,
        observations: None,
        tokenizer_python: None,
        tokenizer_script: None,
        tokenizer_cache: None,
        tokenizers: vec![],
        cancel: None,
        cancel_file: None,
        frozen: false,
        offline: true,
        updates_fixture: None,
        updates_gh: None,
        updates_now: None,
        disable: true,
        json: false,
    };
    let manager = AdapterManager::new(HostConfig::default());
    let handle = start(&manager, l, &snapshot, cache, &o, env, &config)?;
    Ok(OpenedDecision {
        invoker: HostDecisionInvoker::new(handle),
        profile,
        binding: snapshot.binding(),
        lock_digest: res.profile.lock_digest(),
        _manager: manager,
    })
}
