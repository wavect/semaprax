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
    /// SPX-B121: Cargo metadata did not produce a bounded successful response.
    MetadataFailed,
    /// SPX-B122: selected policy cannot execute build scripts or proc macros.
    BuildCodeDenied,
    /// SPX-B122: a sandbox policy was selected but no enforcing runner exists.
    SandboxUnavailable,
    /// SPX-B122: prepared crate bytes differ from the admitted identity.
    BuildIdentityMismatch,
    /// SPX-B123: locked/offline Cargo build failed after trusted execution.
    BuildFailed,
    /// SPX-B124: Cargo output exceeded the bounded capture budget.
    OutputTooLarge,
    Preparation(CargoPreparationError),
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
    prepared: &PreparedCargoClosure,
    binding_plan: &[u8],
    tool_identity: &[u8],
) -> Result<NativeBuildAuthority, CargoExecutionError> {
    profile
        .authorize_build(binding_plan, prepared.bytes(), tool_identity)
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
    validate_invocation(invocation)?;
    if !authority.matches_crate_identity(prepared.bytes()) {
        return Err(CargoExecutionError::BuildIdentityMismatch);
    }
    let output = cargo_command(invocation)
        .arg("build")
        .arg("--locked")
        .arg("--offline")
        .arg("--manifest-path")
        .arg(&invocation.manifest)
        .arg("--target-dir")
        .arg(&invocation.target_dir)
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
    if !regular_file(&invocation.cargo)
        || !regular_file(&invocation.rustc)
        || !directory(&invocation.workspace)
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
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("src/lib.rs"),
            "pub const VALUE: i64 = probe_macro::probe!();\n",
        )
        .unwrap();
        fs::write(
            root.join("build.rs"),
            "fn main() { std::fs::write(\"build-script-entered\", b\"1\").unwrap(); let _ = std::net::TcpStream::connect(\"127.0.0.1:9\"); }\n",
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
            "#[proc_macro] pub fn probe(_: proc_macro::TokenStream) -> proc_macro::TokenStream { std::fs::write(\"proc-macro-entered\", b\"1\").unwrap(); let _ = std::net::TcpStream::connect(\"127.0.0.1:9\"); \"1\".parse().unwrap() }\n",
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
        prepared: &PreparedCargoClosure,
    ) -> Result<NativeBuildAuthority, CargoExecutionError> {
        authorize_prepared_build(
            profile,
            prepared,
            b"exact-binding-plan",
            b"exact-cargo-and-rustc",
        )
    }

    fn pure_prepared_fixture() -> PreparedCargoClosure {
        let package_id = "path+file:///workspace/fixture#fixture@0.1.0";
        prepare_cargo_closure(CargoPreparationInput {
            binding_plan: b"exact-binding-plan".to_vec(),
            descriptor: b"descriptor".to_vec(),
            cargo_metadata: format!("{{\"packages\":[{{\"id\":\"{package_id}\",\"source\":null}}],\"resolve\":{{\"nodes\":[{{\"id\":\"{package_id}\",\"features\":[]}}]}}}}" ).into_bytes(),
            cargo_lock: b"lock".to_vec(),
            cargo_config: b"offline".to_vec(),
            toolchain_identity: b"exact-cargo-and-rustc".to_vec(),
            target_spec_identity: b"x86_64-unknown-linux-gnu".to_vec(),
            generator_revision: "sha256:fixture".into(),
            target: "x86_64-unknown-linux-gnu".into(),
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
                &prepared
            ),
            Err(CargoExecutionError::BuildCodeDenied)
        );
        assert!(!marker.exists());
        assert!(!invocation.workspace.join("build-script-entered").exists());
        assert!(!invocation.workspace.join("proc-macro-entered").exists());
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
                &prepared
            ),
            Err(CargoExecutionError::SandboxUnavailable)
        );
        assert!(!marker.exists());
        assert!(!invocation.workspace.join("build-script-entered").exists());
        assert!(!invocation.workspace.join("proc-macro-entered").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn stale_build_authority_refuses_before_cargo_process_entry() {
        let (root, invocation, marker) = fixture();
        let prepared = pure_prepared_fixture();
        let stale_profile = profile(b"old-prepared-crate", NativeBuildPolicy::TrustedHost);
        assert_eq!(
            authorize(&stale_profile, &prepared),
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
        assert!(!invocation.workspace.join("proc-macro-entered").exists());
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
        let profile = profile(prepared.closure().bytes(), NativeBuildPolicy::TrustedHost);
        let authority = authorize(&profile, prepared.closure()).unwrap();
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
