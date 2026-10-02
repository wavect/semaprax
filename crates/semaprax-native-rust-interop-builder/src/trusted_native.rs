//! Explicit acknowledgement and dispatch gates for rich native Rust bindings.
//!
//! A [`TrustedNativeProfile`] binds exact opaque inputs and a maintainer-supplied
//! effect contract. It does not prove anything about the Rust implementation.
//! In particular, a digest of Cargo metadata or a binding plan is identity
//! evidence, never permission to run Cargo or to enter native code.

use sha2::{Digest, Sha256};

pub const TRUSTED_NATIVE_PROFILE_SCHEMA: &str = "semaprax.trusted-native-profile.v1";

const PROFILE_DOMAIN: &[u8] = b"semaprax.trusted-native-profile.v1\0";
const MAX_IDENTITY_BYTES: usize = 1_048_576;
const MAX_EFFECTS: usize = 64;
const MAX_EFFECT_BYTES: usize = 128;

/// Build-script and proc-macro execution policy selected by the embedding host.
///
/// `TrustedHost` is an acknowledgement that Cargo code receives ordinary host
/// authority. It is deliberately not presented as confinement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeBuildPolicy {
    /// Build scripts and proc macros are denied before Cargo can execute them.
    StrictDenyExecution,
    /// A separately implemented sandbox was selected and is known to enforce
    /// the named isolation policy.
    EnforcedSandbox,
    /// Cargo build code is allowed with full authority of the invoking host.
    TrustedHost,
}

/// The source of an effect contract for native code.
///
/// Metadata alone must select `Opaque`. Only an audited adapter or a stronger
/// execution boundary may select `Audited`, including when the audited effect
/// set is empty.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeEffectContract<'a> {
    Opaque,
    Audited(&'a [&'a str]),
}

impl NativeBuildPolicy {
    pub const fn permits_build_code(self) -> bool {
        !matches!(self, Self::StrictDenyExecution)
    }

    pub const fn confinement_is_enforced(self) -> bool {
        matches!(self, Self::EnforcedSandbox)
    }

    pub const fn disclosure(self) -> &'static str {
        match self {
            Self::StrictDenyExecution => "native build scripts and proc macros are denied",
            Self::EnforcedSandbox => {
                "native build scripts and proc macros run in an enforced sandbox"
            }
            Self::TrustedHost => {
                "native build scripts and proc macros run with trusted host authority"
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeTrustError {
    EmptyIdentity,
    IdentityTooLarge,
    TooManyEffects,
    InvalidEffect,
    EffectsNotCanonical,
    CapabilityNotDeclared,
    OpaqueNativeBehavior,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeDispatchError {
    MissingCapability,
    ProfileMismatch,
}

/// An acknowledgement that exact native input bytes may be dispatched.
///
/// Construction is intentionally explicit. The caller supplies the plan,
/// crate/index, and tool identity bytes separately so any changed component
/// produces a new profile digest and requires re-admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrustedNativeProfile {
    digest: [u8; 32],
    effects: Box<[String]>,
    effects_are_audited: bool,
    build_policy: NativeBuildPolicy,
}

/// Per-call capability authority derived from one admitted profile.
///
/// This value carries no build authority and cannot be used for a different
/// admitted identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeExecutionGrant {
    profile_digest: [u8; 32],
    capabilities: Box<[String]>,
}

impl TrustedNativeProfile {
    pub fn admit(
        binding_plan: &[u8],
        crate_identity: &[u8],
        tool_identity: &[u8],
        effect_contract: NativeEffectContract<'_>,
        build_policy: NativeBuildPolicy,
    ) -> Result<Self, NativeTrustError> {
        for identity in [binding_plan, crate_identity, tool_identity] {
            if identity.is_empty() {
                return Err(NativeTrustError::EmptyIdentity);
            }
            if identity.len() > MAX_IDENTITY_BYTES {
                return Err(NativeTrustError::IdentityTooLarge);
            }
        }
        let (effects, effects_are_audited) = match effect_contract {
            NativeEffectContract::Opaque => (Vec::new(), false),
            NativeEffectContract::Audited(effects) => (canonical_effects(effects)?, true),
        };
        let mut hasher = Sha256::new();
        hasher.update(PROFILE_DOMAIN);
        hasher.update(TRUSTED_NATIVE_PROFILE_SCHEMA.as_bytes());
        for identity in [binding_plan, crate_identity, tool_identity] {
            frame(&mut hasher, identity);
        }
        hasher.update([build_policy as u8]);
        for effect in &effects {
            frame(&mut hasher, effect.as_bytes());
        }
        Ok(Self {
            digest: hasher.finalize().into(),
            effects: effects.into_boxed_slice(),
            effects_are_audited,
            build_policy,
        })
    }

    pub fn digest(&self) -> [u8; 32] {
        self.digest
    }

    pub fn declared_effects(&self) -> &[String] {
        &self.effects
    }

    pub const fn build_policy(&self) -> NativeBuildPolicy {
        self.build_policy
    }

    pub const fn effects_are_audited(&self) -> bool {
        self.effects_are_audited
    }

    /// Grants only capabilities listed in this admitted conservative contract.
    pub fn grant(&self, capabilities: &[&str]) -> Result<NativeExecutionGrant, NativeTrustError> {
        if !self.effects_are_audited {
            return Err(NativeTrustError::OpaqueNativeBehavior);
        }
        let capabilities = canonical_effects(capabilities)?;
        if capabilities
            .iter()
            .any(|capability| self.effects.binary_search(capability).is_err())
        {
            return Err(NativeTrustError::CapabilityNotDeclared);
        }
        Ok(NativeExecutionGrant {
            profile_digest: self.digest,
            capabilities: capabilities.into_boxed_slice(),
        })
    }

    /// Enters the native adapter only after profile and capability checks.
    ///
    /// The closure stands for generated adapter dispatch. This gate protects
    /// Semaprax dispatch; it makes no claim to intercept arbitrary syscalls by
    /// already trusted code in the process.
    pub fn dispatch<T>(
        &self,
        grant: &NativeExecutionGrant,
        required_effects: &[&str],
        adapter: impl FnOnce() -> T,
    ) -> Result<T, NativeDispatchError> {
        if grant.profile_digest != self.digest {
            return Err(NativeDispatchError::ProfileMismatch);
        }
        if required_effects.iter().any(|effect| {
            grant
                .capabilities
                .binary_search_by(|candidate| candidate.as_str().cmp(effect))
                .is_err()
        }) {
            return Err(NativeDispatchError::MissingCapability);
        }
        Ok(adapter())
    }
}

fn canonical_effects(values: &[&str]) -> Result<Vec<String>, NativeTrustError> {
    if values.len() > MAX_EFFECTS {
        return Err(NativeTrustError::TooManyEffects);
    }
    let mut effects = Vec::with_capacity(values.len());
    for value in values {
        if value.is_empty()
            || value.len() > MAX_EFFECT_BYTES
            || value.bytes().any(|byte| byte.is_ascii_control())
        {
            return Err(NativeTrustError::InvalidEffect);
        }
        effects.push((*value).to_owned());
    }
    if effects.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(NativeTrustError::EffectsNotCanonical);
    }
    Ok(effects)
}

fn frame(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update(
        u64::try_from(bytes.len())
            .expect("identity length is bounded")
            .to_be_bytes(),
    );
    hasher.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> TrustedNativeProfile {
        TrustedNativeProfile::admit(
            b"binding-plan-v1",
            b"crate-index-v1",
            b"cargo-and-rustc-v1",
            NativeEffectContract::Audited(&["host.filesystem", "host.network"]),
            NativeBuildPolicy::StrictDenyExecution,
        )
        .unwrap()
    }

    #[test]
    fn changed_bound_input_requires_readmission() {
        let baseline = profile();
        for (plan, crate_identity, tool) in [
            (
                b"binding-plan-v2".as_slice(),
                b"crate-index-v1".as_slice(),
                b"cargo-and-rustc-v1".as_slice(),
            ),
            (
                b"binding-plan-v1".as_slice(),
                b"crate-index-v2".as_slice(),
                b"cargo-and-rustc-v1".as_slice(),
            ),
            (
                b"binding-plan-v1".as_slice(),
                b"crate-index-v1".as_slice(),
                b"cargo-and-rustc-v2".as_slice(),
            ),
        ] {
            let changed = TrustedNativeProfile::admit(
                plan,
                crate_identity,
                tool,
                NativeEffectContract::Audited(&["host.filesystem", "host.network"]),
                NativeBuildPolicy::StrictDenyExecution,
            )
            .unwrap();
            assert_ne!(baseline.digest(), changed.digest());
        }
    }

    #[test]
    fn missing_capability_rejects_before_adapter_entry() {
        let profile = profile();
        let grant = profile.grant(&["host.filesystem"]).unwrap();
        let mut callback_entries = 0;
        let result = profile.dispatch(&grant, &["host.network"], || {
            callback_entries += 1;
        });
        assert_eq!(result, Err(NativeDispatchError::MissingCapability));
        assert_eq!(callback_entries, 0);
    }

    #[test]
    fn capability_grant_cannot_cross_an_admission_boundary() {
        let first = profile();
        let second = TrustedNativeProfile::admit(
            b"binding-plan-v2",
            b"crate-index-v1",
            b"cargo-and-rustc-v1",
            NativeEffectContract::Audited(&["host.filesystem", "host.network"]),
            NativeBuildPolicy::StrictDenyExecution,
        )
        .unwrap();
        let grant = first.grant(&["host.filesystem"]).unwrap();
        assert_eq!(
            second.dispatch(&grant, &["host.filesystem"], || 1),
            Err(NativeDispatchError::ProfileMismatch)
        );
    }

    #[test]
    fn build_policy_discloses_host_trust_and_never_labels_it_confinement() {
        assert!(!NativeBuildPolicy::StrictDenyExecution.permits_build_code());
        assert!(!NativeBuildPolicy::StrictDenyExecution.confinement_is_enforced());
        assert!(NativeBuildPolicy::EnforcedSandbox.permits_build_code());
        assert!(NativeBuildPolicy::EnforcedSandbox.confinement_is_enforced());
        assert!(NativeBuildPolicy::TrustedHost.permits_build_code());
        assert!(!NativeBuildPolicy::TrustedHost.confinement_is_enforced());
        assert_eq!(
            NativeBuildPolicy::TrustedHost.disclosure(),
            "native build scripts and proc macros run with trusted host authority"
        );
    }

    #[test]
    fn effects_must_be_canonical_and_conservative() {
        assert_eq!(
            TrustedNativeProfile::admit(
                b"plan",
                b"crate",
                b"tool",
                NativeEffectContract::Audited(&["host.network", "host.filesystem"]),
                NativeBuildPolicy::StrictDenyExecution,
            ),
            Err(NativeTrustError::EffectsNotCanonical)
        );
        assert_eq!(
            profile().grant(&["host.process"]),
            Err(NativeTrustError::CapabilityNotDeclared)
        );
    }

    #[test]
    fn metadata_only_native_code_is_opaque_and_never_enters_dispatch() {
        let profile = TrustedNativeProfile::admit(
            b"safe-looking-plan",
            b"metadata-claims-no-effects",
            b"cargo-and-rustc-v1",
            NativeEffectContract::Opaque,
            NativeBuildPolicy::StrictDenyExecution,
        )
        .unwrap();
        assert!(!profile.effects_are_audited());
        assert!(profile.declared_effects().is_empty());
        assert_eq!(
            profile.grant(&[]),
            Err(NativeTrustError::OpaqueNativeBehavior)
        );
    }
}
