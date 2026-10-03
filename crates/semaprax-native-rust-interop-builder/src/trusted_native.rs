//! Explicit acknowledgement and dispatch gates for rich native Rust bindings.
//!
//! A [`TrustedNativeProfile`] binds exact opaque inputs and a maintainer-supplied
//! effect contract. It does not prove anything about the Rust implementation.
//! In particular, a digest of Cargo metadata or a binding plan is identity
//! evidence, never permission to run Cargo or to enter native code.

use semaprax::diagnostic::Diagnostic;
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
    /// A sandbox was requested. No enforcing runner is currently available,
    /// so authorization refuses before any Cargo build process starts.
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
        matches!(self, Self::TrustedHost)
    }

    pub const fn confinement_is_enforced(self) -> bool {
        false
    }

    pub const fn disclosure(self) -> &'static str {
        match self {
            Self::StrictDenyExecution => "native build scripts and proc macros are denied",
            Self::EnforcedSandbox => {
                "native build scripts and proc macros require an enforced sandbox; no runner is available"
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
    BuildIdentityMismatch,
    BuildCodeDenied,
    SandboxUnavailable,
}

impl NativeTrustError {
    pub const fn diagnostic_code(&self) -> &'static str {
        match self {
            Self::OpaqueNativeBehavior | Self::BuildCodeDenied => "SPX-B126",
            Self::SandboxUnavailable => "SPX-B127",
            Self::BuildIdentityMismatch => "SPX-B128",
            Self::CapabilityNotDeclared => "SPX-B129",
            Self::EmptyIdentity
            | Self::IdentityTooLarge
            | Self::TooManyEffects
            | Self::InvalidEffect
            | Self::EffectsNotCanonical => "SPX-B121",
        }
    }

    /// Host-facing, path-free diagnostic for the existing SEMAPRAX renderer.
    pub fn diagnostic(&self) -> Diagnostic {
        let message = match self {
            Self::OpaqueNativeBehavior => "Native Rust behavior is opaque; an audited adapter or stronger execution boundary is required",
            Self::BuildCodeDenied => "Native Rust build scripts and proc macros are untrusted under the strict profile",
            Self::SandboxUnavailable => "Native Rust sandbox execution was requested but no enforcing runner is available",
            Self::BuildIdentityMismatch => "Native Rust prepared code or tool identity changed; re-admission is required",
            Self::CapabilityNotDeclared => "Native Rust capability is not in the audited effect contract",
            Self::EmptyIdentity | Self::IdentityTooLarge | Self::TooManyEffects
            | Self::InvalidEffect | Self::EffectsNotCanonical => "Native Rust trust profile input is invalid",
        };
        Diagnostic::io(self.diagnostic_code(), message)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeDispatchError {
    MissingCapability,
    ProfileMismatch,
}

impl NativeDispatchError {
    pub const fn diagnostic_code(&self) -> &'static str {
        match self {
            Self::MissingCapability => "SPX-B129",
            Self::ProfileMismatch => "SPX-B128",
        }
    }

    /// Host-facing refusal that preserves the missing-capability distinction.
    pub fn diagnostic(&self) -> Diagnostic {
        let message = match self {
            Self::MissingCapability => {
                "Native Rust callback requires a capability that was not granted"
            }
            Self::ProfileMismatch => {
                "Native Rust callback grant belongs to a different admitted profile"
            }
        };
        Diagnostic::io(self.diagnostic_code(), message)
    }
}

/// An acknowledgement that exact native input bytes may be dispatched.
///
/// Construction is intentionally explicit. The caller supplies the plan,
/// crate/index, and tool identity bytes separately so any changed component
/// produces a new profile digest and requires re-admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrustedNativeProfile {
    digest: [u8; 32],
    input_digests: [[u8; 32]; 3],
    effects: Box<[String]>,
    effects_are_audited: bool,
    build_policy: NativeBuildPolicy,
}

/// An admission for one exact prepared crate identity. Only the trusted-host
/// policy currently yields this value; there is no enforcing sandbox runner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeBuildAuthority {
    crate_identity_digest: [u8; 32],
}

impl NativeBuildAuthority {
    pub fn matches_crate_identity(&self, bytes: &[u8]) -> bool {
        let actual: [u8; 32] = Sha256::digest(bytes).into();
        self.crate_identity_digest == actual
    }

    pub const fn disclosure(&self) -> &'static str {
        NativeBuildPolicy::TrustedHost.disclosure()
    }
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
        hasher.update([u8::from(effects_are_audited)]);
        for identity in [binding_plan, crate_identity, tool_identity] {
            frame(&mut hasher, identity);
        }
        hasher.update([build_policy as u8]);
        for effect in &effects {
            frame(&mut hasher, effect.as_bytes());
        }
        Ok(Self {
            digest: hasher.finalize().into(),
            input_digests: [
                Sha256::digest(binding_plan).into(),
                Sha256::digest(crate_identity).into(),
                Sha256::digest(tool_identity).into(),
            ],
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

    /// Recheck the exact acknowledged inputs before allowing Cargo build code.
    /// The caller must separately authenticate the prepared source and tool
    /// bytes against its held files immediately before this call.
    pub fn authorize_build(
        &self,
        binding_plan: &[u8],
        crate_identity: &[u8],
        tool_identity: &[u8],
    ) -> Result<NativeBuildAuthority, NativeTrustError> {
        if [binding_plan, crate_identity, tool_identity]
            .iter()
            .zip(self.input_digests)
            .any(|(bytes, expected)| <[u8; 32]>::from(Sha256::digest(bytes)) != expected)
        {
            return Err(NativeTrustError::BuildIdentityMismatch);
        }
        match self.build_policy {
            NativeBuildPolicy::StrictDenyExecution => Err(NativeTrustError::BuildCodeDenied),
            NativeBuildPolicy::EnforcedSandbox => Err(NativeTrustError::SandboxUnavailable),
            NativeBuildPolicy::TrustedHost => Ok(NativeBuildAuthority {
                crate_identity_digest: self.input_digests[1],
            }),
        }
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
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static CALLBACK_ENTRIES: AtomicUsize = AtomicUsize::new(0);
    static NETWORK_ATTEMPTS: AtomicUsize = AtomicUsize::new(0);
    static FIXTURE_SERIAL: AtomicUsize = AtomicUsize::new(0);

    // Its Rust signature looks pure. The body proves why index metadata cannot
    // classify a native callback as pure without an audited adapter.
    fn safe_looking_native_callback(value: i64, marker: &Path) -> i64 {
        CALLBACK_ENTRIES.fetch_add(1, Ordering::SeqCst);
        std::fs::write(marker, b"entered").unwrap();
        NETWORK_ATTEMPTS.fetch_add(1, Ordering::SeqCst);
        let _ = std::net::TcpStream::connect("127.0.0.1:9");
        value + 1
    }

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
        assert_eq!(
            NativeDispatchError::MissingCapability.diagnostic().code,
            "SPX-B129"
        );
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
        assert!(!NativeBuildPolicy::EnforcedSandbox.permits_build_code());
        assert!(!NativeBuildPolicy::EnforcedSandbox.confinement_is_enforced());
        assert!(NativeBuildPolicy::TrustedHost.permits_build_code());
        assert!(!NativeBuildPolicy::TrustedHost.confinement_is_enforced());
        assert_eq!(
            NativeBuildPolicy::TrustedHost.disclosure(),
            "native build scripts and proc macros run with trusted host authority"
        );
        assert_eq!(
            NativeBuildPolicy::EnforcedSandbox.disclosure(),
            "native build scripts and proc macros require an enforced sandbox; no runner is available"
        );
        assert_eq!(
            NativeDispatchError::MissingCapability.diagnostic_code(),
            "SPX-B129"
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

    #[test]
    fn safe_looking_callback_with_real_side_effects_stays_opaque() {
        let root = std::env::temp_dir().join(format!(
            "semaprax-ri11-opaque-{}-{}",
            std::process::id(),
            FIXTURE_SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let marker = root.join("filesystem-effect");
        let entries_before = CALLBACK_ENTRIES.load(Ordering::SeqCst);
        let network_before = NETWORK_ATTEMPTS.load(Ordering::SeqCst);
        let profile = TrustedNativeProfile::admit(
            b"safe-looking-fn-i64-to-i64",
            b"index-metadata-claims-pure",
            b"exact-rust-tool",
            NativeEffectContract::Opaque,
            NativeBuildPolicy::StrictDenyExecution,
        )
        .unwrap();
        let refusal = profile.grant(&[]).unwrap_err();
        assert_eq!(refusal, NativeTrustError::OpaqueNativeBehavior);
        assert_eq!(refusal.diagnostic().code, "SPX-B126");
        assert_eq!(CALLBACK_ENTRIES.load(Ordering::SeqCst), entries_before);
        assert_eq!(NETWORK_ATTEMPTS.load(Ordering::SeqCst), network_before);
        assert!(!marker.exists());

        // Positive control: the exact callback has all three side effects if
        // invoked directly outside the denied SEMAPRAX dispatch path.
        assert_eq!(safe_looking_native_callback(41, &marker), 42);
        assert_eq!(CALLBACK_ENTRIES.load(Ordering::SeqCst), entries_before + 1);
        assert_eq!(NETWORK_ATTEMPTS.load(Ordering::SeqCst), network_before + 1);
        assert_eq!(std::fs::read(&marker).unwrap(), b"entered");
        std::fs::remove_file(marker).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn build_authority_rechecks_all_three_identities_before_execution() {
        let profile = TrustedNativeProfile::admit(
            b"plan",
            b"prepared-crate-and-index",
            b"cargo-rustc-images",
            NativeEffectContract::Opaque,
            NativeBuildPolicy::TrustedHost,
        )
        .unwrap();
        for (plan, crate_identity, tools) in [
            (
                b"changed-plan".as_slice(),
                b"prepared-crate-and-index".as_slice(),
                b"cargo-rustc-images".as_slice(),
            ),
            (
                b"plan".as_slice(),
                b"changed-crate-and-index".as_slice(),
                b"cargo-rustc-images".as_slice(),
            ),
            (
                b"plan".as_slice(),
                b"prepared-crate-and-index".as_slice(),
                b"changed-cargo-rustc-images".as_slice(),
            ),
        ] {
            assert_eq!(
                profile.authorize_build(plan, crate_identity, tools),
                Err(NativeTrustError::BuildIdentityMismatch)
            );
        }
        let authority = profile
            .authorize_build(b"plan", b"prepared-crate-and-index", b"cargo-rustc-images")
            .unwrap();
        assert!(authority.matches_crate_identity(b"prepared-crate-and-index"));
        assert!(!authority.matches_crate_identity(b"changed-crate-and-index"));
        assert_eq!(
            authority.disclosure(),
            NativeBuildPolicy::TrustedHost.disclosure()
        );
    }

    #[test]
    fn audited_and_opaque_profiles_have_distinct_identities() {
        let opaque = TrustedNativeProfile::admit(
            b"plan",
            b"crate",
            b"tool",
            NativeEffectContract::Opaque,
            NativeBuildPolicy::TrustedHost,
        )
        .unwrap();
        let audited = TrustedNativeProfile::admit(
            b"plan",
            b"crate",
            b"tool",
            NativeEffectContract::Audited(&[]),
            NativeBuildPolicy::TrustedHost,
        )
        .unwrap();
        assert_ne!(opaque.digest(), audited.digest());
        assert_eq!(
            opaque.grant(&[]),
            Err(NativeTrustError::OpaqueNativeBehavior)
        );
        assert!(audited.grant(&[]).is_ok());
    }
}
