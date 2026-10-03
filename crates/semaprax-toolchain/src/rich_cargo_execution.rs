//! Explicit effectful Cargo stages for rich Native Rust interop.
//!
//! This module is intentionally separate from `rich_cargo_preparation`: pure
//! compiler routes cannot reach either function here. Cargo metadata is an
//! explicitly requested subprocess. A later locked/offline build is refused
//! before spawning unless the selected native profile acknowledges trusted host
//! build-code execution. `--offline` limits Cargo networking; it is not a
//! sandbox for build scripts or proc macros.

use crate::rich_cargo_preparation::{
    prepare_cargo_closure, CargoPreparationError, CargoPreparationInput, PreparedCargoClosure,
};
use semaprax::diagnostic::Diagnostic;
use semaprax_native_rust_interop::{NativeBuildAuthority, NativeTrustError, TrustedNativeProfile};
use std::path::{Path, PathBuf};
use std::process::Command;

const MAX_CARGO_OUTPUT_BYTES: usize = 1_048_576;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CargoExecutionError {
    /// SPX-B121: explicit executable, workspace, manifest, Rust toolchain,
    /// Cargo home, executable path, or output directory input is malformed or
    /// changed before invocation.
    InvalidInput,
    /// SPX-B125: the explicitly selected Cargo or rustc image is unavailable.
    MissingTool,
    /// SPX-B122: the prepared API or target has no executable native profile.
    UnsupportedApi,
    /// SPX-B121: Cargo metadata did not produce a bounded successful response.
    MetadataFailed,
    /// SPX-B122: selected policy cannot execute build scripts or proc macros.
    BuildCodeDenied,
    /// SPX-B122: a sandbox policy was selected but no enforcing runner exists.
    SandboxUnavailable,
    /// SPX-B122: prepared crate bytes differ from the admitted identity.
    BuildIdentityMismatch,
    /// SPX-B128: a source, lock, config, Cargo home, or direct tool image
    /// changed after build admission or exceeds the bounded snapshot profile.
    BuildInputsChanged,
    /// SPX-B123: locked/offline Cargo build failed after trusted execution.
    BuildFailed,
    /// SPX-B124: Cargo output exceeded the bounded capture budget.
    OutputTooLarge,
    Preparation(CargoPreparationError),
}

impl CargoExecutionError {
    /// Stable public distinction for CLI and host adapters; no raw path or
    /// tool stderr is needed to explain a refused operation.
    pub const fn diagnostic_code(&self) -> &'static str {
        match self {
            Self::InvalidInput | Self::MetadataFailed => "SPX-B121",
            Self::MissingTool => "SPX-B125",
            Self::UnsupportedApi => "SPX-B122",
            Self::BuildCodeDenied => "SPX-B126",
            Self::SandboxUnavailable => "SPX-B127",
            Self::BuildIdentityMismatch | Self::BuildInputsChanged => "SPX-B128",
            Self::BuildFailed => "SPX-B123",
            Self::OutputTooLarge => "SPX-B124",
            Self::Preparation(CargoPreparationError::Unsupported) => "SPX-B122",
            Self::Preparation(CargoPreparationError::Malformed) => "SPX-B121",
            Self::Preparation(CargoPreparationError::Disagreement) => "SPX-B123",
            Self::Preparation(CargoPreparationError::Capacity) => "SPX-B124",
        }
    }

    /// Render through the existing SEMAPRAX host diagnostic surface without
    /// exposing source paths, tool output, environment values, or secrets.
    pub fn diagnostic(&self) -> Diagnostic {
        let message = match self {
            Self::MissingTool => "Native Rust Cargo or rustc tool is missing",
            Self::UnsupportedApi => "Native Rust API or target is unsupported by this host",
            Self::BuildCodeDenied => "Native Rust build code is untrusted under the strict profile",
            Self::SandboxUnavailable => "Native Rust sandbox profile has no enforcing runner",
            Self::BuildIdentityMismatch | Self::BuildInputsChanged => {
                "Native Rust build inputs changed; re-admission is required"
            }
            Self::InvalidInput | Self::MetadataFailed => {
                "Native Rust Cargo invocation or metadata is invalid"
            }
            Self::BuildFailed => "Native Rust trusted Cargo build failed",
            Self::OutputTooLarge => "Native Rust Cargo output exceeds its bound",
            Self::Preparation(_) => "Native Rust Cargo preparation was refused",
        };
        Diagnostic::io(self.diagnostic_code(), message)
    }
}

impl From<CargoPreparationError> for CargoExecutionError {
    fn from(error: CargoPreparationError) -> Self {
        Self::Preparation(error)
    }
}

/// Every process-relevant path is explicit and absolute. The caller creates
/// these directories under its own held-directory authority before invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExplicitCargoInvocation {
    pub cargo: PathBuf,
    pub rustc: PathBuf,
    pub workspace: PathBuf,
    pub manifest: PathBuf,
    /// A caller-owned Cargo home. It is also the child process's HOME, so
    /// neither Cargo nor build code inherit the caller's home directory.
    pub cargo_home: PathBuf,
    /// Absolute directories available to Cargo, rustc, and approved build
    /// code. This is a supplied capability, never the caller's ambient PATH.
    pub execution_path: Vec<PathBuf>,
    pub target_dir: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CargoMetadataPreparation {
    closure: PreparedCargoClosure,
    metadata: Vec<u8>,
}

/// Collect Cargo's exact metadata bytes under an explicit, offline invocation.
/// This stage has no build-script or proc-macro execution authority.
pub fn collect_cargo_metadata(
    invocation: &ExplicitCargoInvocation,
) -> Result<Vec<u8>, CargoExecutionError> {
    validate_invocation(invocation)?;
    let output = cargo_command(invocation)
        .arg("metadata")
        .arg("--format-version=1")
        .arg("--locked")
        .arg("--offline")
        .arg("--manifest-path")
        .arg(&invocation.manifest)
        .output()
        .map_err(|_| CargoExecutionError::MetadataFailed)?;
    bounded(&output.stdout)?;
    bounded(&output.stderr)?;
    if !output.status.success() {
        return Err(cargo_failed(
            CargoExecutionError::MetadataFailed,
            &output.stderr,
        ));
    }
    Ok(output.stdout)
}

impl CargoMetadataPreparation {
    pub fn closure(&self) -> &PreparedCargoClosure {
        &self.closure
    }

    pub fn metadata(&self) -> &[u8] {
        &self.metadata
    }
}

/// Convert an exact prepared closure and explicit host acknowledgement into
/// build authority before any Cargo build process is constructed.
pub fn authorize_prepared_build(
    profile: &TrustedNativeProfile,
    invocation: &ExplicitCargoInvocation,
    prepared: &PreparedCargoClosure,
    binding_plan: &[u8],
    tool_identity: &[u8],
) -> Result<NativeBuildAuthority, CargoExecutionError> {
    validate_invocation(invocation)?;
    let identity =
        if profile.build_policy() == semaprax_native_rust_interop::NativeBuildPolicy::TrustedHost {
            validate_native_target(prepared)?;
            crate::rich_cargo_snapshot::prepared_build_identity(invocation, prepared)?.to_vec()
        } else {
            prepared.bytes().to_vec()
        };
    profile
        .authorize_build(binding_plan, &identity, tool_identity)
        .map_err(|error| match error {
            NativeTrustError::BuildCodeDenied => CargoExecutionError::BuildCodeDenied,
            NativeTrustError::SandboxUnavailable => CargoExecutionError::SandboxUnavailable,
            NativeTrustError::BuildIdentityMismatch => CargoExecutionError::BuildIdentityMismatch,
            _ => CargoExecutionError::InvalidInput,
        })
}

/// Invoke `cargo metadata` only after the caller explicitly selects all tool
/// and directory paths. The supplied metadata bytes are discarded; they cannot
/// substitute for the output from this invocation.
pub fn prepare_with_cargo_metadata(
    invocation: &ExplicitCargoInvocation,
    mut input: CargoPreparationInput,
) -> Result<CargoMetadataPreparation, CargoExecutionError> {
    let metadata = collect_cargo_metadata(invocation)?;
    input.cargo_metadata = metadata.clone();
    let closure = prepare_cargo_closure(input)?;
    Ok(CargoMetadataPreparation { closure, metadata })
}

/// Run the selected Cargo package only with an exact trusted-host admission.
/// Strict and unenforced-sandbox profiles cannot create this authority.
pub fn build_locked_offline(
    invocation: &ExplicitCargoInvocation,
    prepared: &PreparedCargoClosure,
    authority: &NativeBuildAuthority,
) -> Result<(), CargoExecutionError> {
    build_locked_offline_with_hook(invocation, prepared, authority, || {})
}

fn build_locked_offline_with_hook(
    invocation: &ExplicitCargoInvocation,
    prepared: &PreparedCargoClosure,
    authority: &NativeBuildAuthority,
    mut before_final_replay: impl FnMut(),
) -> Result<(), CargoExecutionError> {
    validate_invocation(invocation)?;
    validate_native_target(prepared)?;
    let identity = crate::rich_cargo_snapshot::prepared_build_identity(invocation, prepared)?;
    if !authority.matches_crate_identity(&identity) {
        return Err(CargoExecutionError::BuildIdentityMismatch);
    }
    let mut command = cargo_command(invocation);
    command
        .arg("build")
        .arg("--locked")
        .arg("--offline")
        .arg("--manifest-path")
        .arg(&invocation.manifest)
        .arg("--target-dir")
        .arg(&invocation.target_dir);
    before_final_replay();
    if crate::rich_cargo_snapshot::prepared_build_identity(invocation, prepared)? != identity {
        return Err(CargoExecutionError::BuildIdentityMismatch);
    }
    let output = command
        .output()
        .map_err(|_| CargoExecutionError::BuildFailed)?;
    bounded(&output.stdout)?;
    bounded(&output.stderr)?;
    if !output.status.success() {
        return Err(cargo_failed(
            CargoExecutionError::BuildFailed,
            &output.stderr,
        ));
    }
    Ok(())
}

fn validate_invocation(invocation: &ExplicitCargoInvocation) -> Result<(), CargoExecutionError> {
    validate_selected_tools(&invocation.cargo, &invocation.rustc)?;
    if !directory(&invocation.workspace)
        || !regular_file(&invocation.manifest)
        || !directory(&invocation.cargo_home)
        || invocation.execution_path.is_empty()
        || invocation
            .execution_path
            .iter()
            .any(|path| !directory(path))
        || std::env::join_paths(&invocation.execution_path).is_err()
        || !directory(&invocation.target_dir)
        || invocation.manifest.parent() != Some(invocation.workspace.as_path())
    {
        return Err(CargoExecutionError::InvalidInput);
    }
    Ok(())
}

fn validate_native_target(prepared: &PreparedCargoClosure) -> Result<(), CargoExecutionError> {
    let record: serde_json::Value =
        serde_json::from_slice(prepared.bytes()).map_err(|_| CargoExecutionError::InvalidInput)?;
    let target = record
        .get("target")
        .and_then(serde_json::Value::as_str)
        .ok_or(CargoExecutionError::InvalidInput)?;
    validate_selected_native_target(target)
}

/// Read-only validation shared by the explicit build and private trust CLI.
pub fn validate_selected_native_target(target: &str) -> Result<(), CargoExecutionError> {
    if Some(target) != current_host_target() {
        return Err(CargoExecutionError::UnsupportedApi);
    }
    Ok(())
}

/// Reject a missing selected tool before any Cargo process is constructed.
pub fn validate_selected_tools(cargo: &Path, rustc: &Path) -> Result<(), CargoExecutionError> {
    if !cargo.is_absolute() || !rustc.is_absolute() {
        return Err(CargoExecutionError::InvalidInput);
    }
    if !regular_file(cargo) || !regular_file(rustc) {
        return Err(CargoExecutionError::MissingTool);
    }
    Ok(())
}

fn current_host_target() -> Option<&'static str> {
    if cfg!(all(target_arch = "aarch64", target_os = "macos")) {
        Some("aarch64-apple-darwin")
    } else if cfg!(all(target_arch = "x86_64", target_os = "macos")) {
        Some("x86_64-apple-darwin")
    } else if cfg!(all(target_arch = "x86_64", target_os = "linux")) {
        Some("x86_64-unknown-linux-gnu")
    } else if cfg!(all(target_arch = "aarch64", target_os = "linux")) {
        Some("aarch64-unknown-linux-gnu")
    } else if cfg!(all(target_arch = "x86_64", target_os = "windows")) {
        Some("x86_64-pc-windows-msvc")
    } else {
        None
    }
}

fn regular_file(path: &Path) -> bool {
    path.is_absolute() && path.metadata().is_ok_and(|metadata| metadata.is_file())
}

fn directory(path: &Path) -> bool {
    path.is_absolute() && path.metadata().is_ok_and(|metadata| metadata.is_dir())
}

fn cargo_command(invocation: &ExplicitCargoInvocation) -> Command {
    let mut command = Command::new(&invocation.cargo);
    command
        .env_clear()
        .current_dir(&invocation.workspace)
        .env("RUSTC", &invocation.rustc)
        .env("HOME", &invocation.cargo_home)
        .env("CARGO_HOME", &invocation.cargo_home)
        .env(
            "PATH",
            std::env::join_paths(&invocation.execution_path).unwrap(),
        )
        .env("CARGO_NET_OFFLINE", "true")
        .env("CARGO_TARGET_DIR", &invocation.target_dir);
    command
}

fn cargo_failed(error: CargoExecutionError, stderr: &[u8]) -> CargoExecutionError {
    #[cfg(test)]
    eprintln!(
        "rich Cargo fixture stderr: {}",
        String::from_utf8_lossy(stderr)
    );
    let _ = stderr;
    error
}

fn bounded(bytes: &[u8]) -> Result<(), CargoExecutionError> {
    if bytes.len() > MAX_CARGO_OUTPUT_BYTES {
        Err(CargoExecutionError::OutputTooLarge)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rich_cargo_preparation::{prepare_cargo_closure, LockedCargoSource};
    use semaprax_native_rust_interop::{
        NativeBuildPolicy, NativeEffectContract, TrustedNativeProfile,
    };
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SERIAL: AtomicU64 = AtomicU64::new(0);

    #[cfg(unix)]
    fn fixture() -> (PathBuf, ExplicitCargoInvocation, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "semaprax-ri02-cargo-execution-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let manifest = root.join("Cargo.toml");
        fs::write(
            &manifest,
            "[package]\nname=\"fixture\"\nversion=\"0.1.0\"\nedition=\"2021\"\nbuild=\"build.rs\"\n[dependencies]\nprobe_macro={path=\"probe_macro\"}\n",
        )
        .unwrap();
        fs::write(root.join("Cargo.lock"), "version = 4\n\n[[package]]\nname = \"fixture\"\nversion = \"0.1.0\"\ndependencies = [\"probe_macro\"]\n\n[[package]]\nname = \"probe_macro\"\nversion = \"0.1.0\"\n").unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("src/lib.rs"),
            "pub const VALUE: i64 = probe_macro::probe!();\n",
        )
        .unwrap();
        fs::write(
            root.join("build.rs"),
            "fn main() { std::fs::write(std::path::Path::new(env!(\"CARGO_MANIFEST_DIR\")).join(\"build-script-entered\"), b\"1\").unwrap(); let _ = std::net::TcpStream::connect(\"127.0.0.1:9\"); }\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("probe_macro/src")).unwrap();
        fs::write(
            root.join("probe_macro/Cargo.toml"),
            "[package]\nname=\"probe_macro\"\nversion=\"0.1.0\"\nedition=\"2021\"\n[lib]\nproc-macro=true\n",
        )
        .unwrap();
        fs::write(
            root.join("probe_macro/src/lib.rs"),
            "#[proc_macro] pub fn probe(_: proc_macro::TokenStream) -> proc_macro::TokenStream { std::fs::write(std::path::Path::new(env!(\"CARGO_MANIFEST_DIR\")).join(\"proc-macro-entered\"), b\"1\").unwrap(); let _ = std::net::TcpStream::connect(\"127.0.0.1:9\"); \"1\".parse().unwrap() }\n",
        )
        .unwrap();
        let target = root.join("target");
        fs::create_dir(&target).unwrap();
        let cargo_home = root.join("cargo-home");
        fs::create_dir(&cargo_home).unwrap();
        let marker = root.join("marker");
        let cargo = root.join("cargo");
        fs::write(
            &cargo,
            format!("#!/bin/sh\ntouch {}\nexit 0\n", marker.display()),
        )
        .unwrap();
        let mut permissions = fs::metadata(&cargo).unwrap().permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&cargo, permissions).unwrap();
        let rustc = root.join("rustc");
        fs::write(&rustc, "#!/bin/sh\nexit 0\n").unwrap();
        let mut permissions = fs::metadata(&rustc).unwrap().permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&rustc, permissions).unwrap();
        (
            root.clone(),
            ExplicitCargoInvocation {
                execution_path: test_execution_path(&cargo, &rustc),
                cargo,
                rustc,
                workspace: root,
                manifest,
                cargo_home,
                target_dir: target,
            },
            marker,
        )
    }

    fn profile(crate_identity: &[u8], policy: NativeBuildPolicy) -> TrustedNativeProfile {
        TrustedNativeProfile::admit(
            b"exact-binding-plan",
            crate_identity,
            b"exact-cargo-and-rustc",
            NativeEffectContract::Opaque,
            policy,
        )
        .unwrap()
    }

    fn authorize(
        profile: &TrustedNativeProfile,
        invocation: &ExplicitCargoInvocation,
        prepared: &PreparedCargoClosure,
    ) -> Result<NativeBuildAuthority, CargoExecutionError> {
        authorize_prepared_build(
            profile,
            invocation,
            prepared,
            b"exact-binding-plan",
            b"exact-cargo-and-rustc",
        )
    }

    fn pure_prepared_fixture() -> PreparedCargoClosure {
        pure_prepared_fixture_for_target(native_target())
    }

    fn pure_prepared_fixture_for_target(target: &str) -> PreparedCargoClosure {
        let package_id = "path+file:///workspace/fixture#fixture@0.1.0";
        prepare_cargo_closure(CargoPreparationInput {
            binding_plan: b"exact-binding-plan".to_vec(),
            descriptor: b"descriptor".to_vec(),
            cargo_metadata: format!("{{\"packages\":[{{\"id\":\"{package_id}\",\"source\":null}}],\"resolve\":{{\"nodes\":[{{\"id\":\"{package_id}\",\"features\":[]}}]}}}}" ).into_bytes(),
            cargo_lock: b"lock".to_vec(),
            cargo_config: b"offline".to_vec(),
            toolchain_identity: b"exact-cargo-and-rustc".to_vec(),
            target_spec_identity: target.as_bytes().to_vec(),
            host_target_identity: format!("host={};target={target}", native_target())
                .into_bytes(),
            build_script_inputs: b"no-build-script-inputs".to_vec(),
            proc_macro_inputs: b"no-proc-macro-inputs".to_vec(),
            native_toolchain_inputs: b"rustc".to_vec(),
            generator_revision: "sha256:fixture".into(),
            target: target.into(),
            panic_strategy: "unwind".into(),
            profile: "dev".into(),
            selected_features: Vec::new(),
            sources: vec![LockedCargoSource::Local {
                package_id: package_id.into(),
                tree_digest: format!("sha256:{}", "0".repeat(64)),
            }],
        }).unwrap()
    }

    #[cfg(unix)]
    #[test]
    fn strict_policy_refuses_before_cargo_can_run_a_build_script() {
        let (root, invocation, marker) = fixture();
        let prepared = pure_prepared_fixture();
        assert_eq!(
            authorize(
                &profile(prepared.bytes(), NativeBuildPolicy::StrictDenyExecution),
                &invocation,
                &prepared
            ),
            Err(CargoExecutionError::BuildCodeDenied)
        );
        assert!(!marker.exists());
        assert!(!invocation.workspace.join("build-script-entered").exists());
        assert!(!invocation
            .workspace
            .join("probe_macro/proc-macro-entered")
            .exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn sandbox_policy_refuses_without_claiming_local_confinement() {
        let (root, invocation, marker) = fixture();
        let prepared = pure_prepared_fixture();
        assert_eq!(
            authorize(
                &profile(prepared.bytes(), NativeBuildPolicy::EnforcedSandbox),
                &invocation,
                &prepared
            ),
            Err(CargoExecutionError::SandboxUnavailable)
        );
        assert!(!marker.exists());
        assert!(!invocation.workspace.join("build-script-entered").exists());
        assert!(!invocation
            .workspace
            .join("probe_macro/proc-macro-entered")
            .exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn stale_build_authority_refuses_before_cargo_process_entry() {
        let (root, invocation, marker) = fixture();
        let prepared = pure_prepared_fixture();
        let stale_profile = profile(b"old-prepared-crate", NativeBuildPolicy::TrustedHost);
        assert_eq!(
            authorize(&stale_profile, &invocation, &prepared),
            Err(CargoExecutionError::BuildIdentityMismatch)
        );
        let stale_authority = stale_profile
            .authorize_build(
                b"exact-binding-plan",
                b"old-prepared-crate",
                b"exact-cargo-and-rustc",
            )
            .unwrap();
        assert_eq!(
            build_locked_offline(&invocation, &prepared, &stale_authority),
            Err(CargoExecutionError::BuildIdentityMismatch)
        );
        assert!(!marker.exists());
        assert!(!invocation.workspace.join("build-script-entered").exists());
        assert!(!invocation
            .workspace
            .join("probe_macro/proc-macro-entered")
            .exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn physical_source_lock_config_and_tool_drift_refuse_before_cargo_entry() {
        for changed in [
            "build.rs",
            "Cargo.lock",
            ".cargo/config.toml",
            "cargo",
            "rustc",
        ] {
            let (root, invocation, marker) = fixture();
            fs::create_dir(root.join(".cargo")).unwrap();
            fs::write(root.join(".cargo/config.toml"), b"[net]\noffline=true\n").unwrap();
            let prepared = pure_prepared_fixture();
            let identity =
                crate::rich_cargo_snapshot::prepared_build_identity(&invocation, &prepared)
                    .unwrap();
            let profile = profile(&identity, NativeBuildPolicy::TrustedHost);
            let authority = authorize(&profile, &invocation, &prepared).unwrap();
            fs::write(
                root.join(changed),
                b"changed after native build admission\n",
            )
            .unwrap();
            let refusal = build_locked_offline(&invocation, &prepared, &authority);
            assert!(
                matches!(
                    &refusal,
                    Err(CargoExecutionError::BuildIdentityMismatch)
                        | Err(CargoExecutionError::BuildInputsChanged)
                ),
                "{changed}: {refusal:?}"
            );
            assert!(!marker.exists(), "Cargo entered after {changed} changed");
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[cfg(unix)]
    #[test]
    fn change_during_command_preparation_refuses_at_final_replay() {
        let (root, invocation, marker) = fixture();
        let prepared = pure_prepared_fixture();
        let identity =
            crate::rich_cargo_snapshot::prepared_build_identity(&invocation, &prepared).unwrap();
        let profile = profile(&identity, NativeBuildPolicy::TrustedHost);
        let authority = authorize(&profile, &invocation, &prepared).unwrap();
        let result = build_locked_offline_with_hook(&invocation, &prepared, &authority, || {
            fs::write(
                root.join("build.rs"),
                b"changed while Cargo command was prepared\n",
            )
            .unwrap();
        });
        assert_eq!(result, Err(CargoExecutionError::BuildIdentityMismatch));
        assert!(!marker.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn missing_tool_has_distinct_diagnostic_before_process_entry() {
        let (root, mut invocation, marker) = fixture();
        invocation.cargo = root.join("missing-cargo");
        assert_eq!(
            collect_cargo_metadata(&invocation),
            Err(CargoExecutionError::MissingTool)
        );
        assert_eq!(
            CargoExecutionError::MissingTool.diagnostic_code(),
            "SPX-B125"
        );
        let missing_tool = CargoExecutionError::MissingTool.diagnostic();
        assert_eq!(missing_tool.code, "SPX-B125");
        assert_eq!(
            missing_tool.message,
            "Native Rust Cargo or rustc tool is missing"
        );
        assert!(missing_tool.path.is_none());
        assert_eq!(
            CargoExecutionError::BuildCodeDenied.diagnostic_code(),
            "SPX-B126"
        );
        assert_eq!(
            CargoExecutionError::SandboxUnavailable.diagnostic_code(),
            "SPX-B127"
        );
        assert_eq!(
            CargoExecutionError::BuildInputsChanged.diagnostic_code(),
            "SPX-B128"
        );
        assert_eq!(
            CargoExecutionError::Preparation(CargoPreparationError::Unsupported).diagnostic_code(),
            "SPX-B122"
        );
        assert_eq!(
            CargoExecutionError::UnsupportedApi.diagnostic().code,
            "SPX-B122"
        );
        assert_eq!(
            CargoExecutionError::BuildCodeDenied.diagnostic().code,
            "SPX-B126"
        );
        assert_eq!(
            CargoExecutionError::SandboxUnavailable.diagnostic().code,
            "SPX-B127"
        );
        assert!(!marker.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn cross_target_build_is_unsupported_before_cargo_entry() {
        let (root, invocation, marker) = fixture();
        let wrong_target = if native_target() == "x86_64-unknown-linux-gnu" {
            "aarch64-apple-darwin"
        } else {
            "x86_64-unknown-linux-gnu"
        };
        let prepared = pure_prepared_fixture_for_target(wrong_target);
        let profile = profile(prepared.bytes(), NativeBuildPolicy::TrustedHost);
        assert_eq!(
            authorize(&profile, &invocation, &prepared),
            Err(CargoExecutionError::UnsupportedApi)
        );
        assert_eq!(
            CargoExecutionError::UnsupportedApi.diagnostic_code(),
            "SPX-B122"
        );
        assert!(!marker.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn trusted_host_executes_build_script_and_proc_macro_with_disclosure() {
        let (root, mut invocation, _) = fixture();
        invocation.cargo = configured_tool("CARGO");
        invocation.rustc = configured_tool("RUSTC");
        invocation.execution_path = test_execution_path(&invocation.cargo, &invocation.rustc);
        let lock = cargo_command(&invocation)
            .arg("generate-lockfile")
            .arg("--offline")
            .arg("--manifest-path")
            .arg(&invocation.manifest)
            .output()
            .unwrap();
        assert!(
            lock.status.success(),
            "{}",
            String::from_utf8_lossy(&lock.stderr)
        );
        let prepared = pure_prepared_fixture();
        let identity =
            crate::rich_cargo_snapshot::prepared_build_identity(&invocation, &prepared).unwrap();
        let profile = profile(&identity, NativeBuildPolicy::TrustedHost);
        let authority = authorize(&profile, &invocation, &prepared).unwrap();
        assert_eq!(
            authority.disclosure(),
            "native build scripts and proc macros run with trusted host authority"
        );
        build_locked_offline(&invocation, &prepared, &authority).unwrap();
        assert!(root.join("build-script-entered").exists());
        assert!(root.join("probe_macro/proc-macro-entered").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn explicit_offline_metadata_prepares_the_local_rich_fixture() {
        let prepared = prepare_fixture("rich-rust-fixture");
        assert!(prepared.closure().bytes().ends_with(b"\n"));
    }

    #[test]
    fn explicit_offline_metadata_prepares_the_vendored_registry_fixture() {
        let prepared = prepare_fixture("rich-rust-vendored-fixture");
        assert!(prepared
            .closure()
            .bytes()
            .windows(b"\"kind\":\"registry\"".len())
            .any(|window| window == b"\"kind\":\"registry\""));
    }

    #[cfg(unix)]
    #[test]
    fn explicit_offline_metadata_preserves_dependency_shape_facts() {
        let prepared = prepare_fixture("rich-rust-shape-fixture");
        let metadata: serde_json::Value = serde_json::from_slice(prepared.metadata()).unwrap();
        let packages = metadata
            .get("packages")
            .and_then(serde_json::Value::as_array)
            .unwrap();
        let renamed = packages
            .iter()
            .find(|package| {
                package.get("name").and_then(serde_json::Value::as_str) == Some("shape-renamed")
            })
            .unwrap();
        assert_eq!(
            renamed
                .get("targets")
                .and_then(serde_json::Value::as_array)
                .and_then(|targets| targets.first())
                .and_then(|target| target.get("name"))
                .and_then(serde_json::Value::as_str),
            Some("different_lib_target")
        );
        assert_eq!(
            packages
                .iter()
                .filter(
                    |package| package.get("name").and_then(serde_json::Value::as_str)
                        == Some("shape-dual")
                )
                .count(),
            2
        );
        let resolve = metadata
            .get("resolve")
            .and_then(|resolve| resolve.get("nodes"))
            .and_then(serde_json::Value::as_array)
            .unwrap();
        let root = resolve
            .iter()
            .find(|node| {
                node.get("id")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|id| id.contains("semaprax-ri02-shape-fixture"))
            })
            .unwrap();
        assert!(root
            .get("deps")
            .and_then(serde_json::Value::as_array)
            .unwrap()
            .iter()
            .any(
                |dependency| dependency.get("name").and_then(serde_json::Value::as_str)
                    == Some("renamed_shape")
            ));
        let renamed_node = resolve
            .iter()
            .find(|node| {
                node.get("id")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|id| id.contains("shape-renamed@0.1.0"))
            })
            .unwrap();
        let features = renamed_node
            .get("features")
            .and_then(serde_json::Value::as_array)
            .unwrap();
        assert!(features.iter().any(|feature| feature == "target-feature"));
        assert!(!features.iter().any(|feature| feature == "default-on"));
        assert!(prepared
            .closure()
            .bytes()
            .windows(b"shape-target@0.1.0".len())
            .any(|window| window == b"shape-target@0.1.0"));
    }

    #[cfg(unix)]
    #[test]
    fn missing_vendored_dependency_reports_the_exact_required_package() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(
                "../semaprax-native-rust-interop-builder/tests/fixtures/rich-rust-missing-vendor-fixture",
            )
            .canonicalize()
            .unwrap();
        let target = std::env::temp_dir().join(format!(
            "semaprax-ri02-missing-vendor-target-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&target).unwrap();
        let cargo_home = target.join("cargo-home");
        fs::create_dir(&cargo_home).unwrap();
        let cargo = configured_tool("CARGO");
        let rustc = configured_tool("RUSTC");
        let invocation = ExplicitCargoInvocation {
            execution_path: test_execution_path(&cargo, &rustc),
            cargo,
            rustc,
            workspace: fixture.clone(),
            manifest: fixture.join("Cargo.toml"),
            cargo_home,
            target_dir: target.clone(),
        };
        let output = cargo_command(&invocation)
            .arg("metadata")
            .arg("--format-version=1")
            .arg("--locked")
            .arg("--offline")
            .arg("--manifest-path")
            .arg(&invocation.manifest)
            .output()
            .unwrap();
        assert!(!output.status.success());
        bounded(&output.stderr).unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("`missing-vendor`"), "{stderr}");
        assert!(
            stderr.contains("semaprax-ri02-missing-vendor-fixture"),
            "{stderr}"
        );
        fs::remove_dir_all(target).unwrap();
    }

    #[test]
    fn trusted_host_runs_the_vendored_fixture_locked_and_offline() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(
                "../semaprax-native-rust-interop-builder/tests/fixtures/rich-rust-vendored-fixture",
            )
            .canonicalize()
            .unwrap();
        let target = std::env::temp_dir().join(format!(
            "semaprax-ri02-vendored-build-target-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&target).unwrap();
        let cargo_home = target.join("cargo-home");
        fs::create_dir(&cargo_home).unwrap();
        let cargo = configured_tool("CARGO");
        let rustc = configured_tool("RUSTC");
        let invocation = ExplicitCargoInvocation {
            execution_path: test_execution_path(&cargo, &rustc),
            cargo,
            rustc,
            workspace: fixture.clone(),
            manifest: fixture.join("Cargo.toml"),
            cargo_home,
            target_dir: target.clone(),
        };
        let prepared = prepare_fixture("rich-rust-vendored-fixture");
        let identity =
            crate::rich_cargo_snapshot::prepared_build_identity(&invocation, prepared.closure())
                .unwrap();
        let profile = profile(&identity, NativeBuildPolicy::TrustedHost);
        let authority = authorize(&profile, &invocation, prepared.closure()).unwrap();
        assert_eq!(
            authority.disclosure(),
            NativeBuildPolicy::TrustedHost.disclosure()
        );
        build_locked_offline(&invocation, prepared.closure(), &authority).unwrap();
        assert!(target.join("debug").is_dir());
        fs::remove_dir_all(target).unwrap();
    }

    fn prepare_fixture(name: &str) -> CargoMetadataPreparation {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../semaprax-native-rust-interop-builder/tests/fixtures")
            .join(name)
            .canonicalize()
            .unwrap();
        let target = std::env::temp_dir().join(format!(
            "semaprax-ri02-rich-fixture-target-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&target).unwrap();
        let cargo_home = target.join("cargo-home");
        fs::create_dir(&cargo_home).unwrap();
        let cargo = configured_tool("CARGO");
        let rustc = configured_tool("RUSTC");
        let invocation = ExplicitCargoInvocation {
            execution_path: test_execution_path(&cargo, &rustc),
            cargo,
            rustc,
            workspace: fixture.clone(),
            manifest: fixture.join("Cargo.toml"),
            cargo_home,
            target_dir: target.clone(),
        };
        let metadata = collect_cargo_metadata(&invocation).unwrap();
        let sources = serde_json::from_slice::<serde_json::Value>(&metadata)
            .unwrap()
            .get("packages")
            .and_then(serde_json::Value::as_array)
            .unwrap()
            .iter()
            .map(|package| {
                let package_id = package
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap()
                    .to_owned();
                if package
                    .get("source")
                    .and_then(serde_json::Value::as_str)
                    .is_some()
                {
                    LockedCargoSource::Registry {
                        package_id,
                        checksum: format!("sha256:{}", "0".repeat(64)),
                    }
                } else {
                    LockedCargoSource::Local {
                        package_id,
                        tree_digest: format!("sha256:{}", "0".repeat(64)),
                    }
                }
            })
            .collect::<Vec<_>>();
        let mut sources = sources;
        sources.sort_by(|left, right| left.package_id().cmp(right.package_id()));
        let target_name = native_target();
        let prepared = prepare_with_cargo_metadata(
            &invocation,
            CargoPreparationInput {
                binding_plan: b"rich-binding-plan".to_vec(),
                descriptor: b"native-rust-descriptor".to_vec(),
                cargo_metadata: Vec::new(),
                cargo_lock: fs::read(fixture.join("Cargo.lock")).unwrap(),
                cargo_config: b"[net]\noffline=true\n".to_vec(),
                toolchain_identity: b"explicit-cargo-and-rustc".to_vec(),
                target_spec_identity: target_name.as_bytes().to_vec(),
                host_target_identity: format!("host={target_name};target={target_name}")
                    .into_bytes(),
                build_script_inputs: b"fixture-build-script-inputs".to_vec(),
                proc_macro_inputs: b"fixture-proc-macro-inputs".to_vec(),
                native_toolchain_inputs: b"fixture-rustc-and-linker".to_vec(),
                generator_revision: "sha256:rich-fixture".into(),
                target: target_name.into(),
                panic_strategy: "unwind".into(),
                profile: "dev".into(),
                selected_features: Vec::new(),
                sources,
            },
        )
        .unwrap();
        assert_eq!(prepared.metadata(), metadata);
        fs::remove_dir_all(target).unwrap();
        prepared
    }

    fn configured_tool(name: &str) -> PathBuf {
        std::env::var_os(name)
            .map(PathBuf::from)
            .and_then(|path| path.canonicalize().ok())
            .or_else(|| {
                // Test-only discovery constructs the explicit absolute input
                // required by the production invocation API. The API itself
                // never resolves a tool from PATH.
                std::env::var_os("PATH").and_then(|paths| {
                    std::env::split_paths(&paths)
                        .map(|directory| {
                            directory.join(format!(
                                "{}{}",
                                name.to_ascii_lowercase(),
                                std::env::consts::EXE_SUFFIX
                            ))
                        })
                        .find_map(|path| path.canonicalize().ok())
                })
            })
            .filter(|path| path.is_absolute() && path.is_file())
            .expect("Cargo test harness must provide an absolute tool")
    }

    fn test_execution_path(cargo: &Path, rustc: &Path) -> Vec<PathBuf> {
        let mut paths = vec![
            cargo.parent().unwrap().to_owned(),
            rustc.parent().unwrap().to_owned(),
        ];
        #[cfg(unix)]
        paths.push(PathBuf::from("/usr/bin"));
        paths.sort();
        paths.dedup();
        paths
    }

    fn native_target() -> &'static str {
        if cfg!(all(target_arch = "aarch64", target_os = "macos")) {
            "aarch64-apple-darwin"
        } else if cfg!(all(target_arch = "x86_64", target_os = "macos")) {
            "x86_64-apple-darwin"
        } else if cfg!(all(target_arch = "x86_64", target_os = "linux")) {
            "x86_64-unknown-linux-gnu"
        } else if cfg!(all(target_arch = "aarch64", target_os = "linux")) {
            "aarch64-unknown-linux-gnu"
        } else if cfg!(all(target_arch = "x86_64", target_os = "windows")) {
            "x86_64-pc-windows-msvc"
        } else {
            panic!("focused rich Cargo fixture has no admitted native target")
        }
    }
}
