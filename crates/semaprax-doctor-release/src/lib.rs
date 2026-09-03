//! Deterministic, authority-explicit release capsule construction.
//!
//! The caller must quiesce same-principal mutation of admitted inputs and the
//! output directory. Held-file identity and exact-length reads exclude pathname
//! substitution, but cannot prove the absence of concurrent same-size writes to
//! an already open file.

mod directory;
mod elf;

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use ed25519_dalek::{Signature, Signer as _, SigningKey, VerifyingKey};
use semaprax_doctor_capsule::{encode_body, parse_signed, Artifact, CapsuleSpec};
use sha2::{Digest as _, Sha256};

use elf::{verify_static_elf64, ExpectedArchitecture};

pub use directory::{verify_release_directory, ReleaseExpectation};

pub const CAPSULE_FILE: &str = "semaprax-doctor-release.capsule";
pub const MANIFEST_FILE: &str = "semaprax-doctor-release-manifest.json";
pub const MANIFEST_SIGNATURE_FILE: &str = "semaprax-doctor-release-manifest.sig";
pub const REQUEST_FILE: &str = "semaprax-doctor-request.bin";
pub const BUNDLE_FILE: &str = "semaprax-doctor-bundle.bin";
pub const LAUNCHER_FILE: &str = "semaprax-doctor-launcher";
pub const WORKER_FILE: &str = "semaprax-doctor-worker";
pub const COLLECTOR_FILE: &str = "semaprax-doctor-collector";
pub const PROVISIONER_FILE: &str = "semaprax-doctor-provisioner";
const MAX_ARTIFACT_BYTES: u64 = 512 * 1024 * 1024;
const MAX_KEY_BYTES: u64 = 65;
const MAX_MANIFEST_BYTES: u64 = 32 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseInputs {
    pub request: PathBuf,
    pub bundle: PathBuf,
    pub launcher: PathBuf,
    pub worker: PathBuf,
    pub collector: PathBuf,
    pub provisioner: PathBuf,
    pub selector: String,
    pub architecture: u8,
    pub target: u8,
    pub release_version: String,
    pub release_commit: String,
    pub target_triple: String,
    pub signing_key: PathBuf,
    pub output_directory: PathBuf,
}

#[derive(Debug)]
struct Secret(Vec<u8>);

impl Drop for Secret {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

struct InputArtifact {
    role: &'static str,
    bytes: Vec<u8>,
    artifact: Artifact,
}

pub fn key_information(path: &Path) -> Result<String, String> {
    let signing = load_signing_key(path)?;
    let public = signing.verifying_key();
    if public.is_weak() {
        return Err("signing key has a weak public key".into());
    }
    let public_hex = hex(&public.to_bytes());
    let fingerprint = digest(&public.to_bytes());
    Ok(format!(
        "{{\"schema\":\"semaprax.doctor-release-key-info.v1\",\"public_key_hex\":\"{public_hex}\",\"public_key_fingerprint\":\"{fingerprint}\"}}\n"
    ))
}

pub fn create_release(inputs: &ReleaseInputs) -> Result<(), String> {
    validate_selector(&inputs.selector)?;
    validate_release_identity(inputs)?;
    if !matches!(inputs.architecture, 1 | 2) || inputs.target > 3 {
        return Err("architecture or target is unsupported".into());
    }
    validate_output_directory(&inputs.output_directory)?;
    let request = load_artifact("request", &inputs.request, false, inputs.architecture)?;
    let bundle = load_artifact("bundle", &inputs.bundle, false, inputs.architecture)?;
    let launcher = load_artifact("launcher", &inputs.launcher, true, inputs.architecture)?;
    let worker = load_artifact("worker", &inputs.worker, true, inputs.architecture)?;
    let collector = load_artifact("collector", &inputs.collector, true, inputs.architecture)?;
    let provisioner = load_artifact(
        "provisioner",
        &inputs.provisioner,
        true,
        inputs.architecture,
    )?;
    let signing = load_signing_key(&inputs.signing_key)?;
    let public = signing.verifying_key();
    if public.is_weak() {
        return Err("signing key has a weak public key".into());
    }
    let body = encode_body(&CapsuleSpec {
        architecture: inputs.architecture,
        target: inputs.target,
        selector: inputs.selector.clone(),
        artifacts: [
            copy_artifact(&request.artifact),
            copy_artifact(&bundle.artifact),
            copy_artifact(&launcher.artifact),
            copy_artifact(&worker.artifact),
            copy_artifact(&collector.artifact),
        ],
    })
    .map_err(|_| "capsule body admission failed".to_owned())?;
    let mut capsule = body;
    capsule.extend_from_slice(&signing.sign(&capsule).to_bytes());

    let artifacts = [request, bundle, launcher, worker, collector, provisioner];
    let public_hex = hex(&public.to_bytes());
    let manifest = render_manifest(
        inputs,
        &artifacts,
        capsule.len(),
        &digest(&capsule),
        &digest(&public.to_bytes()),
        &public_hex,
    );
    let manifest_signature = signing.sign(manifest.as_bytes()).to_bytes();
    verify_outputs(
        &capsule,
        manifest.as_bytes(),
        &manifest_signature,
        &public_hex,
    )?;
    publish_outputs(
        &inputs.output_directory,
        &capsule,
        manifest.as_bytes(),
        &manifest_signature,
    )
}

pub fn verify_outputs(
    capsule: &[u8],
    manifest: &[u8],
    signature: &[u8],
    public_key_hex: &str,
) -> Result<(), String> {
    let public = parse_public_key(public_key_hex)?;
    let parsed =
        parse_signed(capsule, &public).map_err(|_| "signed capsule replay failed".to_owned())?;
    let signature: [u8; 64] = signature
        .try_into()
        .map_err(|_| "manifest signature length is invalid")?;
    public
        .verify_strict(manifest, &Signature::from_bytes(&signature))
        .map_err(|_| "manifest signature verification failed".to_owned())?;
    verify_manifest_binding(manifest, capsule, public_key_hex, &parsed)
}

fn verify_manifest_binding(
    manifest: &[u8],
    capsule_bytes: &[u8],
    public_hex: &str,
    capsule: &semaprax_doctor_capsule::Capsule,
) -> Result<(), String> {
    let value: serde_json::Value =
        serde_json::from_slice(manifest).map_err(|_| "release manifest is not JSON")?;
    let object = value
        .as_object()
        .ok_or("release manifest is not an object")?;
    let expected = [
        "architecture",
        "artifacts",
        "capsule",
        "key_fingerprint",
        "manifest_signature",
        "public_key_hex",
        "release_commit",
        "release_version",
        "roles",
        "schema",
        "selector",
        "target",
        "target_triple",
    ];
    if object.len() != expected.len() || !expected.iter().all(|key| object.contains_key(*key)) {
        return Err("release manifest keys are not closed".into());
    }
    if object["schema"] != "semaprax.doctor-release-manifest.v1"
        || object["public_key_hex"] != public_hex
        || object["key_fingerprint"] != digest(&parse_public_key(public_hex)?.to_bytes())
        || object["architecture"] != capsule.architecture
        || object["target"] != capsule.target
        || object["roles"] != capsule.roles
        || object["selector"] != capsule.selector
    {
        return Err("release manifest identity disagrees with capsule".into());
    }
    let carrier = object["capsule"]
        .as_object()
        .ok_or("manifest capsule row is invalid")?;
    if carrier.len() != 3
        || carrier["file"] != CAPSULE_FILE
        || carrier["length"] != capsule_bytes.len()
        || carrier["sha256"] != digest(capsule_bytes)
    {
        return Err("release manifest capsule binding disagrees".into());
    }
    let signature = object["manifest_signature"]
        .as_object()
        .ok_or("manifest signature row is invalid")?;
    if signature.len() != 2
        || signature["file"] != MANIFEST_SIGNATURE_FILE
        || signature["algorithm"] != "ed25519"
    {
        return Err("release manifest signature descriptor disagrees".into());
    }
    let rows = object["artifacts"]
        .as_array()
        .ok_or("manifest artifact inventory is invalid")?;
    if rows.len() != 6 {
        return Err("manifest artifact inventory is not exact".into());
    }
    for (index, role) in ["request", "bundle", "launcher", "worker", "collector"]
        .iter()
        .enumerate()
    {
        let row = rows[index]
            .as_object()
            .ok_or("manifest artifact row is invalid")?;
        let artifact = capsule.artifacts[index];
        if row.len() != 4
            || row["role"] != *role
            || row["file"] != artifact_file(role).ok_or("manifest role is invalid")?
            || row["length"] != artifact.length
            || row["sha256"] != format!("sha256:{}", hex(&artifact.digest))
        {
            return Err("manifest artifact binding disagrees with capsule".into());
        }
    }
    let provisioner = rows[5]
        .as_object()
        .ok_or("manifest provisioner row is invalid")?;
    if provisioner.len() != 4
        || provisioner["role"] != "provisioner"
        || provisioner["file"] != PROVISIONER_FILE
    {
        return Err("manifest provisioner row is invalid".into());
    }
    Ok(())
}

fn copy_artifact(value: &Artifact) -> Artifact {
    Artifact {
        length: value.length,
        digest: value.digest,
    }
}

fn load_artifact(
    role: &'static str,
    path: &Path,
    executable: bool,
    architecture: u8,
) -> Result<InputArtifact, String> {
    let bytes = read_once(path, MAX_ARTIFACT_BYTES, executable, false)?;
    if executable {
        validate_static_elf(&bytes, architecture)?;
    }
    let length = u64::try_from(bytes.len()).map_err(|_| "artifact length overflow".to_owned())?;
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    Ok(InputArtifact {
        role,
        bytes,
        artifact: Artifact { length, digest },
    })
}

fn validate_static_elf(bytes: &[u8], architecture: u8) -> Result<(), String> {
    let expected = match architecture {
        1 => ExpectedArchitecture::X86_64,
        2 => ExpectedArchitecture::Aarch64,
        _ => return Err("release executable architecture is unsupported".into()),
    };
    verify_static_elf64(bytes, expected)
        .map(|_| ())
        .map_err(|_| "release executable is not an admitted static ELF64 image".into())
}

fn load_signing_key(path: &Path) -> Result<SigningKey, String> {
    let encoded = Secret(read_once(path, MAX_KEY_BYTES, false, true)?);
    let text = std::str::from_utf8(&encoded.0).map_err(|_| "signing key is not canonical UTF-8")?;
    let text = text
        .strip_suffix('\n')
        .ok_or("signing key lacks its canonical LF terminator")?;
    if text.len() != 64
        || !text
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err("signing key must be exactly 64 lowercase hexadecimal characters".into());
    }
    let mut secret = [0u8; 32];
    for (index, byte) in secret.iter_mut().enumerate() {
        *byte = (hex_nibble(text.as_bytes()[index * 2])? << 4)
            | hex_nibble(text.as_bytes()[index * 2 + 1])?;
    }
    let signing = SigningKey::from_bytes(&secret);
    secret.fill(0);
    Ok(signing)
}

fn read_once(path: &Path, limit: u64, executable: bool, secret: bool) -> Result<Vec<u8>, String> {
    let before = std::fs::symlink_metadata(path).map_err(|_| "input metadata is unavailable")?;
    if !before.file_type().is_file() || before.len() == 0 || before.len() > limit {
        return Err("input must be one nonempty bounded regular file".into());
    }
    check_permissions(&before, executable, secret)?;
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let file = options.open(path).map_err(|_| "input cannot be opened")?;
    let opened = file
        .metadata()
        .map_err(|_| "opened input metadata is unavailable")?;
    same_file(&before, &opened)?;
    let capacity = usize::try_from(before.len()).map_err(|_| "input length is unsupported")?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(capacity)
        .map_err(|_| "input allocation failed")?;
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "input read failed")?;
    if bytes.len() != capacity {
        return Err("input changed while being read".into());
    }
    Ok(bytes)
}

#[cfg(unix)]
fn check_permissions(
    metadata: &std::fs::Metadata,
    executable: bool,
    secret: bool,
) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt as _;
    let mode = metadata.mode() & 0o777;
    if metadata.nlink() != 1 || mode & 0o6022 != 0 || (secret && mode != 0o600) {
        return Err("input permissions or link count are unsafe".into());
    }
    if executable && mode & 0o100 == 0 {
        return Err("release executable is not owner-executable".into());
    }
    Ok(())
}

#[cfg(not(unix))]
fn check_permissions(_: &std::fs::Metadata, _: bool, _: bool) -> Result<(), String> {
    Ok(())
}

#[cfg(unix)]
fn same_file(left: &std::fs::Metadata, right: &std::fs::Metadata) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt as _;
    if left.dev() == right.dev() && left.ino() == right.ino() && right.nlink() == 1 {
        Ok(())
    } else {
        Err("input identity changed before read".into())
    }
}

#[cfg(not(unix))]
fn same_file(left: &std::fs::Metadata, right: &std::fs::Metadata) -> Result<(), String> {
    if left.len() == right.len() && right.file_type().is_file() {
        Ok(())
    } else {
        Err("input identity changed before read".into())
    }
}

fn validate_output_directory(path: &Path) -> Result<(), String> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| "output directory is unavailable")?;
    if !metadata.file_type().is_dir() {
        return Err("output path is not one directory".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        if metadata.mode() & 0o022 != 0 {
            return Err("output directory permissions are unsafe".into());
        }
    }
    Ok(())
}

fn publish_outputs(
    directory: &Path,
    capsule: &[u8],
    manifest: &[u8],
    signature: &[u8],
) -> Result<(), String> {
    let capsule_path = directory.join(CAPSULE_FILE);
    let manifest_path = directory.join(MANIFEST_FILE);
    let signature_path = directory.join(MANIFEST_SIGNATURE_FILE);
    if capsule_path.exists() || manifest_path.exists() || signature_path.exists() {
        return Err("release output already exists".into());
    }
    let mut capsule_file = create_new(&capsule_path)?;
    capsule_file
        .write_all(capsule)
        .and_then(|_| capsule_file.sync_all())
        .map_err(|error| format!("capsule output failed: {error}"))?;
    // A failure after create-new leaves an inert partial release for explicit
    // operator removal. Pathname cleanup could race and delete foreign bytes.
    let mut manifest_file = create_new(&manifest_path)
        .map_err(|error| format!("partial release contains capsule only: {error}"))?;
    if let Err(error) = manifest_file
        .write_all(manifest)
        .and_then(|_| manifest_file.sync_all())
    {
        return Err(format!(
            "partial release contains capsule and incomplete manifest: {error}"
        ));
    }
    let mut signature_file = create_new(&signature_path)
        .map_err(|error| format!("partial release lacks manifest signature: {error}"))?;
    signature_file
        .write_all(signature)
        .and_then(|_| signature_file.sync_all())
        .map_err(|error| format!("partial release has incomplete manifest signature: {error}"))?;
    Ok(())
}

fn create_new(path: &Path) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    options
        .open(path)
        .map_err(|_| "release output cannot be created without clobbering".into())
}

fn render_manifest(
    inputs: &ReleaseInputs,
    artifacts: &[InputArtifact; 6],
    capsule_len: usize,
    capsule_digest: &str,
    fingerprint: &str,
    public_key_hex: &str,
) -> String {
    render_manifest_exact(
        &inputs.release_version,
        &inputs.release_commit,
        &inputs.target_triple,
        inputs.architecture,
        inputs.target,
        &inputs.selector,
        artifacts,
        capsule_len,
        capsule_digest,
        fingerprint,
        public_key_hex,
    )
}

#[allow(clippy::too_many_arguments)]
fn render_manifest_exact(
    release_version: &str,
    release_commit: &str,
    target_triple: &str,
    architecture: u8,
    target: u8,
    selector: &str,
    artifacts: &[InputArtifact; 6],
    capsule_len: usize,
    capsule_digest: &str,
    fingerprint: &str,
    public_key_hex: &str,
) -> String {
    let rows = artifacts
        .iter()
        .map(|item| {
            format!(
                "{{\"role\":\"{}\",\"file\":\"{}\",\"length\":{},\"sha256\":\"{}\"}}",
                item.role,
                artifact_file(item.role).expect("closed release artifact role"),
                item.bytes.len(),
                digest(&item.bytes)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let roles = [4, 1, 2, 7][usize::from(target)];
    format!("{{\"schema\":\"semaprax.doctor-release-manifest.v1\",\"release_version\":\"{}\",\"release_commit\":\"{}\",\"target_triple\":\"{}\",\"architecture\":{},\"target\":{},\"roles\":{},\"selector\":\"{}\",\"public_key_hex\":\"{}\",\"key_fingerprint\":\"{}\",\"artifacts\":[{}],\"capsule\":{{\"file\":\"{}\",\"length\":{},\"sha256\":\"{}\"}},\"manifest_signature\":{{\"file\":\"{}\",\"algorithm\":\"ed25519\"}}}}\n", release_version, release_commit, target_triple, architecture, target, roles, selector, public_key_hex, fingerprint, rows, CAPSULE_FILE, capsule_len, capsule_digest, MANIFEST_SIGNATURE_FILE)
}

fn artifact_file(role: &str) -> Option<&'static str> {
    match role {
        "request" => Some(REQUEST_FILE),
        "bundle" => Some(BUNDLE_FILE),
        "launcher" => Some(LAUNCHER_FILE),
        "worker" => Some(WORKER_FILE),
        "collector" => Some(COLLECTOR_FILE),
        "provisioner" => Some(PROVISIONER_FILE),
        _ => None,
    }
}

fn validate_release_identity(inputs: &ReleaseInputs) -> Result<(), String> {
    validate_release_identity_values(
        &inputs.release_version,
        &inputs.release_commit,
        &inputs.target_triple,
        inputs.architecture,
    )
}

fn validate_release_identity_values(
    release_version: &str,
    release_commit: &str,
    target_triple: &str,
    architecture: u8,
) -> Result<(), String> {
    if release_version.is_empty()
        || release_version.len() > 64
        || !release_version.as_bytes()[0].is_ascii_digit()
        || !release_version.as_bytes()[release_version.len() - 1].is_ascii_alphanumeric()
        || !release_version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
    {
        return Err("release version is not canonical".into());
    }
    if release_commit.len() != 40
        || !release_commit
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err("release commit must be 40 lowercase hexadecimal characters".into());
    }
    if target_triple.is_empty()
        || target_triple.len() > 128
        || !target_triple.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
    {
        return Err("target triple is not canonical".into());
    }
    let expected_prefix = if architecture == 1 {
        "x86_64-"
    } else {
        "aarch64-"
    };
    if !target_triple.starts_with(expected_prefix) || !target_triple.contains("-linux-") {
        return Err("target triple disagrees with the Linux capsule architecture".into());
    }
    Ok(())
}

fn parse_public_key(encoded: &str) -> Result<VerifyingKey, String> {
    if encoded.len() != 64 {
        return Err("public key is not canonical".into());
    }
    let mut bytes = [0u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = (hex_nibble(encoded.as_bytes()[index * 2])? << 4)
            | hex_nibble(encoded.as_bytes()[index * 2 + 1])?;
    }
    let key = VerifyingKey::from_bytes(&bytes).map_err(|_| "public key is invalid")?;
    if key.is_weak() {
        Err("public key is weak".into())
    } else {
        Ok(key)
    }
}

fn validate_selector(value: &str) -> Result<(), String> {
    let bytes = value.as_bytes();
    if bytes.is_empty()
        || bytes.len() > 64
        || !bytes[0].is_ascii_lowercase()
        || !bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
    {
        Err("selector is not canonical".into())
    } else {
        Ok(())
    }
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{}", hex(&Sha256::digest(bytes)))
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn hex_nibble(byte: u8) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err("signing key is not lowercase hexadecimal".into()),
    }
}
