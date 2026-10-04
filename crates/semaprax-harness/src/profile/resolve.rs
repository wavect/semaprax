//! Pure provider resolution. Reads only the project configuration and the
//! machine-local state (plus re-hashing files the user adopted); it executes
//! nothing and never consults `PATH`, `$HOME` or the workspace for providers.
//!
//! Precedence per capability: disabled, explicit project pin, user preference,
//! the single compatible trusted installation, builtin fallback. `required`
//! never falls back: an unmet requirement is an error naming the identity.

use super::builtin;
use super::config::{HarnessConfig, Mode};
use super::installations::{Inspected, Installation, LocalState};
use super::trust::grant_for;
use crate::contract::{CapabilityKind, Descriptor};
use crate::diag::HarnessDiagnostic;
use crate::host::grant::Grant;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindingState {
    Selected,
    Fallback,
    Disabled,
    Unavailable,
}

impl BindingState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Selected => "selected",
            Self::Fallback => "fallback",
            Self::Disabled => "disabled",
            Self::Unavailable => "unavailable",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        [
            Self::Selected,
            Self::Fallback,
            Self::Disabled,
            Self::Unavailable,
        ]
        .into_iter()
        .find(|b| b.as_str() == s)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpstreamBinding {
    pub name: String,
    /// Probed version recorded at adoption.
    pub version: String,
    pub digest: String,
}

/// One capability's resolved provider. Identity fields are empty strings for
/// `Disabled` and `Unavailable`. `reason` is explanatory and is not locked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub kind: CapabilityKind,
    pub provider_id: String,
    pub provider_version: String,
    pub adapter_version: String,
    pub descriptor_digest: String,
    pub upstream: Option<UpstreamBinding>,
    pub state: BindingState,
    pub reason: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Eligible,
    Untrusted,
    Unsupported,
    Unavailable,
}

impl Verdict {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Eligible => "trusted",
            Self::Untrusted => "untrusted",
            Self::Unsupported => "unsupported",
            Self::Unavailable => "unavailable",
        }
    }
}

/// An adopted installation considered for a capability (status and explain only).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub kind: CapabilityKind,
    pub provider_id: String,
    pub verdict: Verdict,
    pub detail: String,
}

/// Portable result of resolution; the lock is a rendering of this.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedProfile {
    pub bindings: Vec<Binding>,
    pub config_digest: String,
    pub candidates: Vec<Candidate>,
    /// Kinds whose resolution found two or more eligible providers.
    pub ambiguous: BTreeMap<CapabilityKind, Vec<String>>,
    pub inactive: Vec<String>,
}

impl ResolvedProfile {
    pub fn binding(&self, kind: CapabilityKind) -> Option<&Binding> {
        self.bindings.iter().find(|b| b.kind == kind)
    }
    /// Digest of the lock rendering (the request envelope's `lock_digest`).
    pub fn lock_digest(&self) -> String {
        super::lock::digest(self)
    }
}

/// Machine-local launch view of one selected external provider. Never
/// serialized into the lock.
#[derive(Clone, Debug)]
pub struct ResolvedLaunch {
    pub kind: CapabilityKind,
    pub provider_id: String,
    pub descriptor: Descriptor,
    pub descriptor_path: PathBuf,
    pub entry_path: Option<PathBuf>,
    pub upstream_path: Option<PathBuf>,
    /// Adopted adapter runtime executable, when one was recorded.
    pub runtime: Option<PathBuf>,
    pub grant: Grant,
}

#[derive(Debug)]
pub struct Resolution {
    pub profile: ResolvedProfile,
    pub launches: BTreeMap<CapabilityKind, ResolvedLaunch>,
    /// Unsatisfied `required` capabilities.
    pub unmet: Vec<HarnessDiagnostic>,
}

struct Eval<'a> {
    inst: &'a Installation,
    verdict: Verdict,
    detail: String,
    inspected: Option<Inspected>,
    grant: Option<Grant>,
}

pub fn current_platform() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

fn evaluate<'a>(inst: &'a Installation, state: &LocalState) -> Eval<'a> {
    let fail = |verdict, detail: String, inspected| Eval {
        inst,
        verdict,
        detail,
        inspected,
        grant: None,
    };
    let insp = match inst.inspect() {
        Ok(i) => i,
        Err(e) => return fail(Verdict::Unavailable, e.message, None),
    };
    let d = &insp.descriptor;
    let platform = current_platform();
    if !d.platforms.iter().any(|p| *p == platform) {
        return fail(
            Verdict::Unsupported,
            format!("platform `{platform}` is not listed by the descriptor"),
            Some(insp),
        );
    }
    // Bundled code (a `local:` package with no probe) has no executable to adopt.
    if let Some(up) = d
        .upstream
        .as_ref()
        .filter(|u| !super::installations::is_bundled(u))
    {
        let Some(rec) = &inst.upstream else {
            let detail = format!(
                "upstream `{}` is not adopted; install it yourself ({} from {}) and run `semaprax harness adopt <descriptor> --upstream <absolute path>`",
                up.name, up.package, up.repository
            );
            return fail(Verdict::Unavailable, detail, Some(insp));
        };
        if insp.current.upstream_digest.is_none() {
            return fail(
                Verdict::Unavailable,
                format!(
                    "upstream executable `{}` is missing at its adopted path",
                    up.name
                ),
                Some(insp),
            );
        }
        if !rec.compatible {
            let seen = rec.version.as_deref().unwrap_or("unidentified");
            let detail = format!(
                "detected upstream `{}` version {seen} is incompatible (supported: {})",
                up.name,
                up.versions.join(", ")
            );
            return fail(Verdict::Unsupported, detail, Some(insp));
        }
        if insp.current.upstream_digest.as_deref() != Some(rec.digest.as_str()) {
            return fail(
                Verdict::Untrusted,
                "the upstream executable changed since adoption; run adopt again".into(),
                Some(insp),
            );
        }
    }
    if insp.current.descriptor_digest != inst.descriptor_digest
        || insp.current.entry_digest != inst.entry_digest
    {
        return fail(
            Verdict::Untrusted,
            "the descriptor or adapter entry changed since adoption; run adopt again".into(),
            Some(insp),
        );
    }
    match grant_for(state, &inst.provider_id, &insp.current) {
        Ok(g) => Eval {
            inst,
            verdict: Verdict::Eligible,
            detail: "compatible and trusted".into(),
            inspected: Some(insp),
            grant: Some(g),
        },
        Err(e) => fail(Verdict::Untrusted, e.message, Some(insp)),
    }
}

/// Compatibility and trust verdict of one adopted installation.
pub fn verdict_of(inst: &Installation, state: &LocalState) -> (Verdict, String) {
    let e = evaluate(inst, state);
    (e.verdict, e.detail)
}

fn declares(i: &Inspected, kind: CapabilityKind) -> bool {
    i.descriptor
        .capabilities
        .iter()
        .any(|c| c.kind == Some(kind) && kind.supported_versions().contains(&c.version))
}

fn empty_binding(kind: CapabilityKind, state: BindingState, reason: String) -> Binding {
    Binding {
        kind,
        provider_id: String::new(),
        provider_version: String::new(),
        adapter_version: String::new(),
        descriptor_digest: String::new(),
        upstream: None,
        state,
        reason,
    }
}

fn builtin_binding(kind: CapabilityKind, state: BindingState, reason: String) -> Binding {
    let Some(d) = builtin::provider_for(kind).and_then(builtin::descriptor) else {
        return empty_binding(
            kind,
            BindingState::Unavailable,
            format!(
                "{reason}; no builtin provider exists for `{}`",
                kind.as_str()
            ),
        );
    };
    Binding {
        kind,
        provider_id: d.provider_id.clone(),
        provider_version: d.provider_version.clone(),
        adapter_version: d.adapter_version.clone(),
        descriptor_digest: d.digest().to_string(),
        upstream: None,
        state,
        reason,
    }
}

fn external_binding(kind: CapabilityKind, ev: &Eval, reason: &str) -> (Binding, ResolvedLaunch) {
    let insp = ev
        .inspected
        .as_ref()
        .expect("eligible evaluation is inspected");
    let d = &insp.descriptor;
    let upstream = d.upstream.as_ref().map(|u| UpstreamBinding {
        name: u.name.clone(),
        version: ev
            .inst
            .upstream
            .as_ref()
            .and_then(|r| r.version.clone())
            .unwrap_or_default(),
        digest: insp.current.upstream_digest.clone().unwrap_or_default(),
    });
    let binding = Binding {
        kind,
        provider_id: d.provider_id.clone(),
        provider_version: d.provider_version.clone(),
        adapter_version: d.adapter_version.clone(),
        descriptor_digest: insp.current.descriptor_digest.clone(),
        upstream,
        state: BindingState::Selected,
        reason: reason.to_string(),
    };
    let launch = ResolvedLaunch {
        kind,
        provider_id: d.provider_id.clone(),
        descriptor: d.clone(),
        descriptor_path: ev.inst.descriptor_path.clone(),
        entry_path: insp.entry_path.clone(),
        upstream_path: insp.upstream_path.clone(),
        runtime: ev.inst.runtime.clone(),
        grant: ev
            .grant
            .clone()
            .expect("eligible evaluation carries a grant"),
    };
    (binding, launch)
}

/// Resolve every capability; unmet `required` bindings are reported in
/// [`Resolution::unmet`] and bound `Unavailable`.
pub fn resolve_report(config: &HarnessConfig, state: &LocalState) -> Resolution {
    let evals: Vec<Eval> = state
        .installations
        .values()
        .map(|i| evaluate(i, state))
        .collect();
    let mut out = Resolution {
        profile: ResolvedProfile {
            bindings: Vec::new(),
            config_digest: config.digest(),
            candidates: Vec::new(),
            ambiguous: BTreeMap::new(),
            inactive: config.inactive.clone(),
        },
        launches: BTreeMap::new(),
        unmet: Vec::new(),
    };
    let mut kinds = CapabilityKind::ALL;
    kinds.sort_by_key(CapabilityKind::as_str);
    for kind in kinds {
        let cc = config.capability(kind);
        let name = kind.as_str();
        if !config.profile_enabled || cc.mode == Mode::Disabled {
            let why = if config.profile_enabled {
                "disabled by project configuration"
            } else {
                "profile disabled by project configuration"
            };
            out.profile
                .bindings
                .push(empty_binding(kind, BindingState::Disabled, why.into()));
            continue;
        }
        let cands: Vec<&Eval> = evals
            .iter()
            .filter(|e| e.inspected.as_ref().is_some_and(|i| declares(i, kind)))
            .collect();
        for e in &cands {
            out.profile.candidates.push(Candidate {
                kind,
                provider_id: e.inst.provider_id.clone(),
                verdict: e.verdict,
                detail: e.detail.clone(),
            });
        }
        let required = cc.mode == Mode::Required;
        let eligible: Vec<&&Eval> = cands
            .iter()
            .filter(|e| e.verdict == Verdict::Eligible)
            .collect();

        // A required capability that cannot be met is an error, never a fallback.
        let unmet = |code: &'static str, msg: String, out: &mut Resolution| {
            let d = HarnessDiagnostic::new(code, msg);
            out.profile.bindings.push(empty_binding(
                kind,
                BindingState::Unavailable,
                d.message.clone(),
            ));
            out.unmet.push(d);
        };
        let select = |ev: &Eval, why: &str, out: &mut Resolution| {
            let (b, l) = external_binding(kind, ev, why);
            out.profile.bindings.push(b);
            out.launches.insert(kind, l);
        };

        // 1. explicit project pin
        if let Some(pin) = &cc.provider {
            if builtin::kind_of(pin) == Some(kind) {
                out.profile.bindings.push(builtin_binding(
                    kind,
                    BindingState::Selected,
                    "project pin (builtin)".into(),
                ));
                continue;
            }
            match cands.iter().find(|e| e.inst.provider_id == *pin) {
                Some(ev) if ev.verdict == Verdict::Eligible => select(ev, "project pin", &mut out),
                other => {
                    let why = other.map_or_else(
                        || format!("not adopted on this machine, or it does not declare `{name}`"),
                        |e| format!("{}: {}", e.verdict.as_str(), e.detail),
                    );
                    if required {
                        unmet("SPX-HPB040", format!("required capability `{name}` is pinned to `{pin}`, which is unusable: {why}"), &mut out);
                    } else {
                        let reason = format!(
                            "pinned provider `{pin}` is unusable ({why}); builtin fallback"
                        );
                        out.profile.bindings.push(builtin_binding(
                            kind,
                            BindingState::Fallback,
                            reason,
                        ));
                    }
                }
            }
            continue;
        }
        // 2. approved user preference
        if let Some(pref) = state.preferences.get(&kind) {
            if builtin::kind_of(pref) == Some(kind) {
                out.profile.bindings.push(builtin_binding(
                    kind,
                    BindingState::Selected,
                    "user preference (builtin)".into(),
                ));
                continue;
            }
            if let Some(ev) = eligible.iter().find(|e| e.inst.provider_id == *pref) {
                select(ev, "user preference", &mut out);
                continue;
            }
        }
        // 3. the single compatible trusted installation
        match eligible.len() {
            1 => select(
                eligible[0],
                "single compatible trusted installation",
                &mut out,
            ),
            0 if required => {
                let seen: Vec<String> = cands
                    .iter()
                    .map(|e| {
                        format!(
                            "{} ({}: {})",
                            e.inst.provider_id,
                            e.verdict.as_str(),
                            e.detail
                        )
                    })
                    .collect();
                let tail = if seen.is_empty() {
                    "no installed provider declares it".to_string()
                } else {
                    seen.join("; ")
                };
                unmet(
                    "SPX-HPB041",
                    format!("required capability `{name}` has no compatible trusted provider: {tail}. Adopt and trust one, or pin a provider in semaprax.harness.toml"),
                    &mut out,
                );
            }
            0 => {
                let reason = if cands.is_empty() {
                    "no external provider installed; builtin fallback".to_string()
                } else {
                    "no installed provider is compatible and trusted; builtin fallback".to_string()
                };
                out.profile
                    .bindings
                    .push(builtin_binding(kind, BindingState::Fallback, reason));
            }
            _ => {
                let ids: Vec<String> = eligible
                    .iter()
                    .map(|e| e.inst.provider_id.clone())
                    .collect();
                out.profile.ambiguous.insert(kind, ids.clone());
                let why = format!("ambiguous: {} are all compatible and trusted; pin one with `provider = ...` or set a preference", ids.join(", "));
                if required {
                    unmet(
                        "SPX-HPB042",
                        format!("required capability `{name}` is {why}"),
                        &mut out,
                    );
                } else {
                    out.profile.bindings.push(builtin_binding(
                        kind,
                        BindingState::Fallback,
                        format!("{why}; builtin fallback"),
                    ));
                }
            }
        }
    }
    out
}

/// Strict resolution: the first unmet `required` capability is an error.
pub fn resolve(
    config: &HarnessConfig,
    state: &LocalState,
) -> Result<Resolution, HarnessDiagnostic> {
    let r = resolve_report(config, state);
    match r.unmet.first() {
        Some(d) => Err(d.clone()),
        None => Ok(r),
    }
}
