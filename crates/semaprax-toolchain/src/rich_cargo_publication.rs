//! Atomic no-clobber publication for successful rich Cargo build outputs.
//!
//! Cargo writes into its ordinary target directory. This route copies the
//! selected outputs into one immutable bundle only after replaying the complete
//! prepared build identity and each output digest. The bundle is published
//! through the held-directory primitives used by the native package publisher.

use crate::rich_cargo_execution::{CargoExecutionError, ExplicitCargoInvocation};
use crate::rich_cargo_preparation::{PreparedCargoArtifact, PreparedCargoClosure};
use semaprax_native_rust_interop::NativeBuildAuthority;
use semaprax_native_rust_interop_platform as platform;
use sha2::{Digest, Sha256};
use std::ffi::{OsStr, OsString};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_PUBLISH_ARTIFACTS: usize = 1024;
const MAX_BUILD_RECEIPT_BYTES: usize = 1_048_576;
/// M1 publication is deliberately narrower than Cargo's receipt bound to cap
/// the one in-memory bundle buffer used by this route.
const MAX_PUBLISH_BYTES: usize = 32 * 1024 * 1024;
const RECEIPT_NAME: &str = "receipt.json";
const ARTIFACTS_NAME: &str = "artifacts.bin";
const BUNDLE_SCHEMA: &str = "semaprax.native-rust-cargo-artifact-bundle.v1";
const STAGE_ATTEMPTS: usize = 8;
static STAGE_NONCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishedCargoArtifactBundle {
    output_directory: PathBuf,
    closure_digest: String,
    receipt_digest: String,
}

impl PublishedCargoArtifactBundle {
    pub fn output_directory(&self) -> &Path {
        &self.output_directory
    }

    pub fn closure_digest(&self) -> &str {
        &self.closure_digest
    }

    pub fn receipt_digest(&self) -> &str {
        &self.receipt_digest
    }
}

struct BuildReceipt {
    schema: String,
    artifacts: Vec<BuildReceiptEntry>,
}

struct BuildReceiptEntry {
    path: String,
    bytes: u64,
    digest: String,
}

/// Replay the admitted closure, authenticate every Cargo output by held file
/// handle, then publish a two-file bundle with an atomic no-clobber directory
/// rename. The 32 MiB M1 bundle bound is enforced before staging begins.
pub fn publish_locked_cargo_artifacts(
    invocation: &ExplicitCargoInvocation,
    prepared: &PreparedCargoClosure,
    authority: &NativeBuildAuthority,
    artifact: &PreparedCargoArtifact,
    output: &Path,
) -> Result<PublishedCargoArtifactBundle, CargoExecutionError> {
    if artifact.closure_digest() != prepared.digest() {
        return Err(CargoExecutionError::BuildIdentityMismatch);
    }
    ensure_output_parent_outside_workspace(&invocation.workspace, output)?;
    validate_identity(invocation, prepared, authority)?;
    let receipt_digest = format!("sha256:{}", hex(&Sha256::digest(artifact.bytes())));
    if artifact.digest() != receipt_digest {
        return Err(CargoExecutionError::BuildFailed);
    }
    if artifact.bytes().len() > MAX_BUILD_RECEIPT_BYTES {
        return Err(CargoExecutionError::OutputTooLarge);
    }
    let build_receipt = parse_build_receipt(artifact.bytes())?;
    if build_receipt.schema != "semaprax.native-rust-cargo-artifact-receipt.v1"
        || build_receipt.artifacts.is_empty()
        || build_receipt.artifacts.len() > MAX_PUBLISH_ARTIFACTS
    {
        return Err(CargoExecutionError::BuildFailed);
    }

    let target = platform::hold_directory(&invocation.target_dir)
        .map_err(|_| CargoExecutionError::BuildInputsChanged)?;
    if !platform::same_directory_path(&target, &invocation.target_dir)
        .map_err(|_| CargoExecutionError::BuildInputsChanged)?
    {
        return Err(CargoExecutionError::BuildInputsChanged);
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut previous_path = None::<String>;
    let mut payload = Vec::new();
    let mut published_entries = Vec::with_capacity(build_receipt.artifacts.len());
    for entry in build_receipt.artifacts {
        if !valid_digest(&entry.digest)
            || !seen.insert(entry.path.clone())
            || previous_path
                .as_ref()
                .is_some_and(|path| path >= &entry.path)
        {
            return Err(CargoExecutionError::BuildFailed);
        }
        previous_path = Some(entry.path.clone());
        let components = validated_components(&entry.path)?;
        let mut parents = Vec::with_capacity(components.len().saturating_sub(1));
        for component in &components[..components.len() - 1] {
            let held = {
                let current = parents.last().unwrap_or(&target);
                platform::hold_child_directory(current, component)
                    .map_err(|_| CargoExecutionError::BuildInputsChanged)?
            };
            parents.push(held);
        }
        let current = parents.last().unwrap_or(&target);
        let final_name = components.last().ok_or(CargoExecutionError::BuildFailed)?;
        let size = usize::try_from(entry.bytes).map_err(|_| CargoExecutionError::OutputTooLarge)?;
        let end = payload
            .len()
            .checked_add(size)
            .ok_or(CargoExecutionError::OutputTooLarge)?;
        if end > MAX_PUBLISH_BYTES {
            return Err(CargoExecutionError::OutputTooLarge);
        }
        let file = platform::hold_regular_file_bounded(current, final_name, size)
            .map_err(|_| CargoExecutionError::BuildInputsChanged)?;
        let bytes = platform::read_exact(&file, size)
            .map_err(|_| CargoExecutionError::BuildInputsChanged)?;
        if bytes.len() != size || format!("sha256:{}", hex(&Sha256::digest(&bytes))) != entry.digest
        {
            return Err(CargoExecutionError::BuildInputsChanged);
        }
        let offset = payload.len();
        payload.extend_from_slice(&bytes);
        published_entries.push(serde_json::json!({
            "path": entry.path,
            "offset": offset,
            "bytes": size,
            "digest": entry.digest,
        }));
    }
    validate_identity(invocation, prepared, authority)?;
    let published_receipt = serde_json::to_vec(&serde_json::json!({
        "schema": BUNDLE_SCHEMA,
        "closure_digest": prepared.digest(),
        "build_receipt_digest": receipt_digest,
        "artifacts": published_entries,
    }))
    .map_err(|_| CargoExecutionError::BuildFailed)?;
    let output_directory = output.to_path_buf();
    publish_files(output, &published_receipt, &payload, || {
        validate_identity(invocation, prepared, authority)
    })?;
    Ok(PublishedCargoArtifactBundle {
        output_directory,
        closure_digest: prepared.digest().to_owned(),
        receipt_digest,
    })
}

fn parse_build_receipt(bytes: &[u8]) -> Result<BuildReceipt, CargoExecutionError> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| CargoExecutionError::BuildFailed)?;
    let object = value.as_object().ok_or(CargoExecutionError::BuildFailed)?;
    if object.len() != 2 {
        return Err(CargoExecutionError::BuildFailed);
    }
    let schema = object
        .get("schema")
        .and_then(serde_json::Value::as_str)
        .ok_or(CargoExecutionError::BuildFailed)?
        .to_owned();
    let entries = object
        .get("artifacts")
        .and_then(serde_json::Value::as_array)
        .ok_or(CargoExecutionError::BuildFailed)?;
    let mut artifacts = Vec::with_capacity(entries.len());
    for entry in entries {
        let object = entry.as_object().ok_or(CargoExecutionError::BuildFailed)?;
        if object.len() != 3 {
            return Err(CargoExecutionError::BuildFailed);
        }
        artifacts.push(BuildReceiptEntry {
            path: object
                .get("path")
                .and_then(serde_json::Value::as_str)
                .ok_or(CargoExecutionError::BuildFailed)?
                .to_owned(),
            bytes: object
                .get("bytes")
                .and_then(serde_json::Value::as_u64)
                .ok_or(CargoExecutionError::BuildFailed)?,
            digest: object
                .get("digest")
                .and_then(serde_json::Value::as_str)
                .ok_or(CargoExecutionError::BuildFailed)?
                .to_owned(),
        });
    }
    let canonical = serde_json::to_vec(&serde_json::json!({
        "schema": schema,
        "artifacts": artifacts.iter().map(|entry| serde_json::json!({
            "path": entry.path,
            "bytes": entry.bytes,
            "digest": entry.digest,
        })).collect::<Vec<_>>(),
    }))
    .map_err(|_| CargoExecutionError::BuildFailed)?;
    if canonical != bytes {
        return Err(CargoExecutionError::BuildFailed);
    }
    Ok(BuildReceipt { schema, artifacts })
}

fn validate_identity(
    invocation: &ExplicitCargoInvocation,
    prepared: &PreparedCargoClosure,
    authority: &NativeBuildAuthority,
) -> Result<(), CargoExecutionError> {
    let identity = crate::rich_cargo_snapshot::prepared_build_identity(invocation, prepared)?;
    if !authority.matches_crate_identity(&identity) {
        return Err(CargoExecutionError::BuildIdentityMismatch);
    }
    Ok(())
}

fn ensure_output_parent_outside_workspace(
    workspace: &Path,
    output: &Path,
) -> Result<(), CargoExecutionError> {
    let workspace = workspace
        .canonicalize()
        .map_err(|_| CargoExecutionError::InvalidInput)?;
    let parent = output
        .parent()
        .ok_or(CargoExecutionError::InvalidInput)?
        .canonicalize()
        .map_err(|_| CargoExecutionError::InvalidInput)?;
    if !output.is_absolute() || parent.starts_with(workspace) {
        return Err(CargoExecutionError::InvalidInput);
    }
    Ok(())
}

fn validated_components(path: &str) -> Result<Vec<OsString>, CargoExecutionError> {
    if path.is_empty() || path.contains('\\') {
        return Err(CargoExecutionError::BuildFailed);
    }
    let components = Path::new(path)
        .components()
        .map(|component| match component {
            Component::Normal(name) => Ok(name.to_os_string()),
            _ => Err(CargoExecutionError::BuildFailed),
        })
        .collect::<Result<Vec<_>, _>>()?;
    if components.is_empty() {
        return Err(CargoExecutionError::BuildFailed);
    }
    if components.len() > 64 {
        return Err(CargoExecutionError::OutputTooLarge);
    }
    Ok(components)
}

fn valid_digest(digest: &str) -> bool {
    digest
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn publish_files(
    output: &Path,
    receipt: &[u8],
    artifacts: &[u8],
    mut before_commit: impl FnMut() -> Result<(), CargoExecutionError>,
) -> Result<(), CargoExecutionError> {
    if !output.is_absolute() || receipt.len() > 1_048_576 || artifacts.len() > MAX_PUBLISH_BYTES {
        return Err(CargoExecutionError::InvalidInput);
    }
    let parent_path = output.parent().ok_or(CargoExecutionError::InvalidInput)?;
    let output_name = output
        .file_name()
        .ok_or(CargoExecutionError::InvalidInput)?
        .to_os_string();
    let parent = platform::hold_directory(parent_path)
        .map_err(|_| CargoExecutionError::PublicationFailed)?;
    if !platform::same_directory_path(&parent, parent_path)
        .map_err(|_| CargoExecutionError::PublicationFailed)?
    {
        return Err(CargoExecutionError::PublicationFailed);
    }
    let child_probe = platform::prepare_child_name(&output_name)
        .map_err(|_| CargoExecutionError::InvalidInput)?;
    if !platform::child_absent_prepared(&parent, &child_probe)
        .map_err(|_| CargoExecutionError::PublicationFailed)?
    {
        return Err(CargoExecutionError::PublicationFailed);
    }
    let mut inventory =
        platform::prepare_discard_inventory([OsStr::new(RECEIPT_NAME), OsStr::new(ARTIFACTS_NAME)])
            .map_err(|_| CargoExecutionError::PublicationFailed)?;
    let prepared_scan = platform::prepare_inventory_exact(&inventory)
        .map_err(|_| CargoExecutionError::PublicationFailed)?;
    let mut selected = None;
    for _ in 0..STAGE_ATTEMPTS {
        let text = format!(
            ".semaprax-cargo-artifact-{}-{}",
            std::process::id(),
            STAGE_NONCE.fetch_add(1, Ordering::Relaxed)
        );
        let stage_name = platform::prepare_stage_name(OsStr::new(&text))
            .map_err(|_| CargoExecutionError::PublicationFailed)?;
        match platform::create_directory_new_prepared(&parent, &stage_name, 0o700) {
            Ok(stage) => {
                selected = Some((text, stage_name, stage));
                break;
            }
            Err(platform::Error::Exists) => continue,
            Err(_) => return Err(CargoExecutionError::PublicationFailed),
        }
    }
    let (stage_text, stage_name, stage) = selected.ok_or(CargoExecutionError::PublicationFailed)?;
    let stage_path = parent_path.join(&stage_text);
    let preparation = (|| {
        if !platform::same_directory_path(&stage, &stage_path)
            .map_err(|_| CargoExecutionError::PublicationFailed)?
        {
            return Err(CargoExecutionError::PublicationFailed);
        }
        platform::write_file_new_prepared(&stage, &mut inventory, RECEIPT_NAME, receipt, 0o600)
            .map_err(|_| CargoExecutionError::PublicationFailed)?;
        platform::write_file_new_prepared(&stage, &mut inventory, ARTIFACTS_NAME, artifacts, 0o600)
            .map_err(|_| CargoExecutionError::PublicationFailed)?;
        let mut scan = prepared_scan;
        platform::inventory_exact_prepared(&mut scan, &stage, &inventory)
            .map_err(|_| CargoExecutionError::PublicationFailed)?;
        platform::recheck_directory(&parent).map_err(|_| CargoExecutionError::PublicationFailed)?;
        platform::recheck_directory(&stage).map_err(|_| CargoExecutionError::PublicationFailed)?;
        let receipt_file =
            platform::hold_regular_file_bounded(&stage, OsStr::new(RECEIPT_NAME), receipt.len())
                .map_err(|_| CargoExecutionError::PublicationFailed)?;
        let artifacts_file = platform::hold_regular_file_bounded(
            &stage,
            OsStr::new(ARTIFACTS_NAME),
            artifacts.len(),
        )
        .map_err(|_| CargoExecutionError::PublicationFailed)?;
        if platform::read_exact(&receipt_file, receipt.len())
            .map_err(|_| CargoExecutionError::PublicationFailed)?
            != receipt
            || platform::read_exact(&artifacts_file, artifacts.len())
                .map_err(|_| CargoExecutionError::PublicationFailed)?
                != artifacts
        {
            return Err(CargoExecutionError::PublicationFailed);
        }
        before_commit()?;
        Ok(())
    })();
    if let Err(error) = preparation {
        let _ = platform::discard_owned_stage_prepared(&parent, &stage, &stage_name, &inventory);
        return Err(error);
    }
    inventory
        .settle_for_publish()
        .map_err(|_| CargoExecutionError::PublicationFailed)?;
    let mut publish = platform::prepare_publish_directory(&output_name)
        .map_err(|_| CargoExecutionError::PublicationFailed)?;
    platform::publish_directory_new_prepared(
        &mut publish,
        &parent,
        &stage,
        &stage_name,
        &output_name,
    )
    .map_err(|_| CargoExecutionError::PublicationFailed)?;
    if !platform::same_directory_path(&stage, output)
        .map_err(|_| CargoExecutionError::PublicationFailed)?
    {
        return Err(CargoExecutionError::PublicationFailed);
    }
    platform::recheck_directory(&parent).map_err(|_| CargoExecutionError::PublicationFailed)?;
    let receipt_file =
        platform::hold_regular_file_bounded(&stage, OsStr::new(RECEIPT_NAME), receipt.len())
            .map_err(|_| CargoExecutionError::PublicationFailed)?;
    let artifacts_file =
        platform::hold_regular_file_bounded(&stage, OsStr::new(ARTIFACTS_NAME), artifacts.len())
            .map_err(|_| CargoExecutionError::PublicationFailed)?;
    if platform::read_exact(&receipt_file, receipt.len())
        .map_err(|_| CargoExecutionError::PublicationFailed)?
        != receipt
        || platform::read_exact(&artifacts_file, artifacts.len())
            .map_err(|_| CargoExecutionError::PublicationFailed)?
            != artifacts
    {
        return Err(CargoExecutionError::PublicationFailed);
    }
    let mut published_inventory = platform::prepare_inventory_entries_exact(
        [OsStr::new(RECEIPT_NAME), OsStr::new(ARTIFACTS_NAME)],
        2,
    )
    .map_err(|_| CargoExecutionError::PublicationFailed)?;
    platform::inventory_entries_exact_prepared(
        &mut published_inventory,
        &stage,
        [&receipt_file, &artifacts_file],
        [],
    )
    .map_err(|_| CargoExecutionError::PublicationFailed)?;
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(HEX[(byte >> 4) as usize] as char);
        result.push(HEX[(byte & 15) as usize] as char);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rich_cargo_execution::{authorize_prepared_build, CargoExecutionError};
    use crate::rich_cargo_preparation::{
        prepare_cargo_closure, CargoPreparationInput, LockedCargoSource, PreparedCargoArtifactCache,
    };
    use semaprax_native_rust_interop::{
        NativeBuildPolicy, NativeEffectContract, TrustedNativeProfile,
    };
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SERIAL: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        root: PathBuf,
        invocation: ExplicitCargoInvocation,
        prepared: PreparedCargoClosure,
        authority: NativeBuildAuthority,
        artifact: PreparedCargoArtifact,
    }

    use semaprax_native_rust_interop::NativeBuildAuthority;

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "semaprax-ri02-cargo-publish-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(root.join("workspace/src")).unwrap();
            fs::create_dir_all(root.join("target/debug/deps")).unwrap();
            fs::create_dir(root.join("cargo-home")).unwrap();
            let root = root.canonicalize().unwrap();
            let workspace = root.join("workspace");
            let target_dir = root.join("target");
            let cargo_home = root.join("cargo-home");
            fs::write(
                workspace.join("Cargo.toml"),
                "[package]\nname=\"fixture\"\nversion=\"0.1.0\"\nedition=\"2021\"\n",
            )
            .unwrap();
            fs::write(workspace.join("Cargo.lock"), "version = 4\n").unwrap();
            fs::write(workspace.join("src/lib.rs"), "pub fn fixture() {}\n").unwrap();
            let cargo = root.join("cargo");
            let rustc = root.join("rustc");
            fs::write(&cargo, b"cargo tool image").unwrap();
            fs::write(&rustc, b"rustc tool image").unwrap();
            let invocation = ExplicitCargoInvocation {
                cargo,
                rustc,
                workspace: workspace.clone(),
                manifest: workspace.join("Cargo.toml"),
                cargo_home,
                execution_path: vec![root.clone()],
                target_dir: target_dir.clone(),
            };
            let target = crate::rich_cargo_execution::current_host_target().unwrap();
            let package_id = "path+file:///workspace/fixture#fixture@0.1.0";
            let prepared = prepare_cargo_closure(CargoPreparationInput {
                binding_plan: b"exact-binding-plan".to_vec(),
                descriptor: b"descriptor".to_vec(),
                cargo_metadata: format!(
                    "{{\"packages\":[{{\"id\":\"{package_id}\",\"source\":null}}],\"resolve\":{{\"nodes\":[{{\"id\":\"{package_id}\",\"features\":[]}}]}}}}"
                )
                .into_bytes(),
                cargo_lock: b"lock".to_vec(),
                cargo_config: b"offline".to_vec(),
                toolchain_identity: b"exact-cargo-and-rustc".to_vec(),
                target_spec_identity: target.as_bytes().to_vec(),
                host_target_identity: format!("host={target};target={target}").into_bytes(),
                build_script_inputs: b"none".to_vec(),
                proc_macro_inputs: b"none".to_vec(),
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
            })
            .unwrap();
            let identity =
                crate::rich_cargo_snapshot::prepared_build_identity(&invocation, &prepared)
                    .unwrap();
            let profile = TrustedNativeProfile::admit(
                b"exact-binding-plan",
                &identity,
                b"exact-cargo-and-rustc",
                NativeEffectContract::Opaque,
                NativeBuildPolicy::TrustedHost,
            )
            .unwrap();
            let authority = authorize_prepared_build(
                &profile,
                &invocation,
                &prepared,
                b"exact-binding-plan",
                b"exact-cargo-and-rustc",
            )
            .unwrap();
            let artifact_path = target_dir.join("debug/deps/libfixture.rlib");
            fs::write(&artifact_path, b"fixture artifact bytes").unwrap();
            let build_receipt = serde_json::to_vec(&serde_json::json!({
                "schema": "semaprax.native-rust-cargo-artifact-receipt.v1",
                "artifacts": [{
                    "path": "debug/deps/libfixture.rlib",
                    "bytes": 22,
                    "digest": format!("sha256:{}", hex(&Sha256::digest(b"fixture artifact bytes"))),
                }]
            }))
            .unwrap();
            let (artifact, reused) = PreparedCargoArtifactCache::default()
                .reuse_or_publish(&prepared, build_receipt)
                .unwrap();
            assert!(!reused);
            Self {
                root,
                invocation,
                prepared,
                authority,
                artifact,
            }
        }
    }

    #[test]
    fn publishes_exact_bundle_atomically_and_preserves_conflicting_output() {
        let fixture = Fixture::new();
        let output = fixture.root.join("published");
        let bundle = publish_locked_cargo_artifacts(
            &fixture.invocation,
            &fixture.prepared,
            &fixture.authority,
            &fixture.artifact,
            &output,
        )
        .unwrap();
        assert_eq!(bundle.closure_digest(), fixture.prepared.digest());
        assert_eq!(
            fs::read(output.join(ARTIFACTS_NAME)).unwrap(),
            b"fixture artifact bytes"
        );
        let receipt: serde_json::Value =
            serde_json::from_slice(&fs::read(output.join(RECEIPT_NAME)).unwrap()).unwrap();
        assert_eq!(receipt["closure_digest"], fixture.prepared.digest());
        assert_eq!(
            fs::read_dir(&output).unwrap().count(),
            2,
            "published bundle inventory must be exact"
        );

        let collision = fixture.root.join("collision");
        fs::create_dir(&collision).unwrap();
        fs::write(collision.join("foreign"), b"leave unchanged").unwrap();
        assert_eq!(
            publish_locked_cargo_artifacts(
                &fixture.invocation,
                &fixture.prepared,
                &fixture.authority,
                &fixture.artifact,
                &collision,
            ),
            Err(CargoExecutionError::PublicationFailed)
        );
        assert_eq!(
            fs::read(collision.join("foreign")).unwrap(),
            b"leave unchanged"
        );
        fs::remove_dir_all(&fixture.root).unwrap();
    }

    #[test]
    fn stale_inputs_or_output_bytes_refuse_before_publication() {
        let fixture = Fixture::new();
        let output = fixture.root.join("stale-output");
        fs::write(
            fixture.invocation.workspace.join("src/lib.rs"),
            b"changed after build admission",
        )
        .unwrap();
        assert!(matches!(
            publish_locked_cargo_artifacts(
                &fixture.invocation,
                &fixture.prepared,
                &fixture.authority,
                &fixture.artifact,
                &output,
            ),
            Err(CargoExecutionError::BuildIdentityMismatch)
                | Err(CargoExecutionError::BuildInputsChanged)
        ));
        assert!(!output.exists());
        assert!(fs::read_dir(&fixture.root).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".semaprax-cargo-artifact-")));
        fs::remove_dir_all(&fixture.root).unwrap();

        let fixture = Fixture::new();
        fs::write(
            fixture
                .invocation
                .target_dir
                .join("debug/deps/libfixture.rlib"),
            b"different artifact bytes",
        )
        .unwrap();
        let output = fixture.root.join("stale-artifact");
        assert_eq!(
            publish_locked_cargo_artifacts(
                &fixture.invocation,
                &fixture.prepared,
                &fixture.authority,
                &fixture.artifact,
                &output,
            ),
            Err(CargoExecutionError::BuildInputsChanged)
        );
        assert!(!output.exists());
        fs::remove_dir_all(&fixture.root).unwrap();
    }

    #[test]
    fn refuses_receipt_path_escape_before_creating_output() {
        let fixture = Fixture::new();
        let forged = serde_json::json!({
            "schema": "semaprax.native-rust-cargo-artifact-receipt.v1",
            "artifacts": [{
                "path": "../escape.rlib",
                "bytes": 1,
                "digest": format!("sha256:{}", hex(&Sha256::digest(b"x"))),
            }]
        });
        let (artifact, _) = PreparedCargoArtifactCache::default()
            .reuse_or_publish(&fixture.prepared, serde_json::to_vec(&forged).unwrap())
            .unwrap();
        let output = fixture.root.join("forged-output");
        assert_eq!(
            publish_locked_cargo_artifacts(
                &fixture.invocation,
                &fixture.prepared,
                &fixture.authority,
                &artifact,
                &output,
            ),
            Err(CargoExecutionError::BuildFailed)
        );
        assert!(!output.exists());
        fs::remove_dir_all(&fixture.root).unwrap();
    }
}
