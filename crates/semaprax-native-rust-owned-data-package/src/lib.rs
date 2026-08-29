//! Dependency-inverted packaging authority for the owned-data Rust SDK.
//!
//! This crate deliberately knows neither SEMAPRAX HIR nor Project paths. Its
//! input is one bounded root-authenticated provider and the canonical public
//! descriptor. It independently replays the descriptor and provider integrity
//! binding, renders the exact safe package, holds explicit tools, and owns the
//! single no-clobber publish. Provider semantics remain the root compiler's
//! responsibility: this lower crate has neither HIR nor codegen authority.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

mod descriptor;
mod flat_descriptor;
mod flat_render;
mod publication;
mod render;

pub use descriptor::{Descriptor, ParameterKind, ResultKind};

pub const NATIVE_RUST_OWNED_DATA_SDK_SCHEMA: &str = "semaprax.native-rust-owned-data-sdk.v1";
pub const PUBLIC_OWNED_DATA_API_SCHEMA: &str = "semaprax.public-owned-data-api.v1";
pub const PUBLIC_OWNED_DATA_PROJECT_SCHEMA: &str = "semaprax.project.v8";
pub const OWNED_CRATE_NAME: &str = "semaprax-generated-native-rust-owned-data-sdk";
pub const OWNED_CRATE_VERSION: &str = "0.1.0";
pub const MAX_DESCRIPTOR_BYTES: usize = 1024 * 1024;
pub const MAX_PROVIDER_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_ARCHIVE_BYTES: usize = 8 * 1024 * 1024;

const DESCRIPTOR_DIGEST_DOMAIN: &[u8] = b"semaprax.public-owned-data-api.digest.v1\0";
const FLAT_DESCRIPTOR_DIGEST_DOMAIN: &[u8] = b"semaprax.public-flat-owned-record-api.digest.v1\0";
const MANIFEST_DIGEST_DOMAIN: &[u8] = b"semaprax.native-rust-owned-data-sdk.manifest.v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostTarget {
    X86_64LinuxGnu,
    Aarch64LinuxGnu,
    X86_64Darwin,
    Aarch64Darwin,
    X86_64WindowsMsvc,
}

impl HostTarget {
    pub const fn current() -> Option<Self> {
        if cfg!(all(target_arch = "x86_64", target_os = "linux")) {
            Some(Self::X86_64LinuxGnu)
        } else if cfg!(all(target_arch = "aarch64", target_os = "linux")) {
            Some(Self::Aarch64LinuxGnu)
        } else if cfg!(all(target_arch = "x86_64", target_os = "macos")) {
            Some(Self::X86_64Darwin)
        } else if cfg!(all(target_arch = "aarch64", target_os = "macos")) {
            Some(Self::Aarch64Darwin)
        } else if cfg!(all(target_arch = "x86_64", target_os = "windows")) {
            Some(Self::X86_64WindowsMsvc)
        } else {
            None
        }
    }

    pub const fn triple(self) -> &'static str {
        match self {
            Self::X86_64LinuxGnu => "x86_64-unknown-linux-gnu",
            Self::Aarch64LinuxGnu => "aarch64-unknown-linux-gnu",
            Self::X86_64Darwin => "x86_64-apple-darwin",
            Self::Aarch64Darwin => "aarch64-apple-darwin",
            Self::X86_64WindowsMsvc => "x86_64-pc-windows-msvc",
        }
    }

    pub const fn archive_name(self) -> &'static str {
        match self {
            Self::X86_64WindowsMsvc => "semaprax_native_rust_owned_data_sdk.lib",
            _ => "libsemaprax_native_rust_owned_data_sdk.a",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackagePlan {
    descriptor: Vec<u8>,
    descriptor_digest: String,
    selected_exports: Vec<String>,
    provider_c: Vec<u8>,
    provider_sha256: String,
    mode: PackageMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackageMode {
    StandaloneEvidence,
    ProjectV8,
    ProjectV9FlatRecord,
}

pub fn build_flat_record_and_publish(
    plan: PackagePlan,
    output: &Path,
) -> Result<PackageBundle, PackageError> {
    if plan.mode != PackageMode::ProjectV9FlatRecord {
        return Err(PackageError::descriptor());
    }
    let target = HostTarget::current().ok_or_else(PackageError::tool)?;
    if plan.provider_c.is_empty()
        || plan.provider_c.len() > MAX_PROVIDER_BYTES
        || raw_sha256(&plan.provider_c) != plan.provider_sha256
        || !provider_binds_descriptor(&plan.provider_c, &plan.descriptor_digest)
        || flat_descriptor_digest(&plan.descriptor) != plan.descriptor_digest
    {
        return Err(PackageError::provider());
    }
    let descriptor = flat_descriptor::replay(
        &plan.descriptor,
        &plan.descriptor_digest,
        &plan.selected_exports,
    )?;
    let sources = flat_render::render_sources(&descriptor, target);
    let publication = publication::PublicationAuthority::new(output)?;
    let tools = publication::HeldTools::from_environment()?;
    let archive = publication::build_archive(&plan.provider_c, target, &publication, &tools)?;
    let archive_name = target.archive_name();
    let manifest = flat_render::render_manifest(
        target,
        &plan.descriptor,
        &plan.descriptor_digest,
        archive_name,
        &plan.provider_sha256,
        [
            ("Cargo.toml", sources.cargo_toml.as_bytes()),
            ("build.rs", sources.build_rs.as_bytes()),
            ("lib.rs", sources.lib_rs.as_bytes()),
            ("owned_data_ffi.rs", sources.ffi_rs.as_bytes()),
            (archive_name, archive.as_slice()),
            ("descriptor.json", &plan.descriptor),
        ],
    );
    flat_render::verify_manifest(manifest.as_bytes(), &manifest)?;
    let files = [
        ("Cargo.toml", sources.cargo_toml.as_bytes()),
        ("build.rs", sources.build_rs.as_bytes()),
        ("lib.rs", sources.lib_rs.as_bytes()),
        ("owned_data_ffi.rs", sources.ffi_rs.as_bytes()),
        (archive_name, archive.as_slice()),
        ("descriptor.json", plan.descriptor.as_slice()),
        (
            "semaprax.native-rust-owned-data-sdk.json",
            manifest.as_bytes(),
        ),
    ];
    let published = publication::publish_package(&publication, files)?;
    publication::verify_published(&publication, &published, files)?;
    Ok(PackageBundle {
        output_directory: output.to_path_buf(),
        manifest_path: output.join("semaprax.native-rust-owned-data-sdk.json"),
        manifest_digest: domain_digest(MANIFEST_DIGEST_DOMAIN, manifest.as_bytes()),
        descriptor_digest: plan.descriptor_digest,
        target_triple: target.triple().to_owned(),
    })
}

impl PackagePlan {
    pub fn new(
        descriptor: Vec<u8>,
        descriptor_digest: String,
        selected_exports: Vec<String>,
        provider_c: Vec<u8>,
        provider_sha256: String,
        mode: PackageMode,
    ) -> Self {
        Self {
            descriptor,
            descriptor_digest,
            selected_exports,
            provider_c,
            provider_sha256,
            mode,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageBundle {
    output_directory: PathBuf,
    manifest_path: PathBuf,
    manifest_digest: String,
    descriptor_digest: String,
    target_triple: String,
}

impl PackageBundle {
    pub fn output_directory(&self) -> &Path {
        &self.output_directory
    }

    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    pub fn manifest_digest(&self) -> &str {
        &self.manifest_digest
    }

    pub fn descriptor_digest(&self) -> &str {
        &self.descriptor_digest
    }

    pub fn crate_name(&self) -> &'static str {
        OWNED_CRATE_NAME
    }

    pub fn target_triple(&self) -> &str {
        &self.target_triple
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackageErrorKind {
    Descriptor,
    Provider,
    ToolConfiguration,
    Publication,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackageError {
    kind: PackageErrorKind,
}

impl PackageError {
    pub const fn kind(self) -> PackageErrorKind {
        self.kind
    }

    const fn descriptor() -> Self {
        Self {
            kind: PackageErrorKind::Descriptor,
        }
    }

    const fn provider() -> Self {
        Self {
            kind: PackageErrorKind::Provider,
        }
    }

    const fn tool() -> Self {
        Self {
            kind: PackageErrorKind::ToolConfiguration,
        }
    }

    const fn publication() -> Self {
        Self {
            kind: PackageErrorKind::Publication,
        }
    }
}

pub fn provider_sha256(bytes: &[u8]) -> String {
    raw_sha256(bytes)
}

pub fn build_and_publish(plan: PackagePlan, output: &Path) -> Result<PackageBundle, PackageError> {
    let target = HostTarget::current().ok_or_else(PackageError::tool)?;
    if plan.provider_c.is_empty()
        || plan.provider_c.len() > MAX_PROVIDER_BYTES
        || raw_sha256(&plan.provider_c) != plan.provider_sha256
        || (plan.mode == PackageMode::ProjectV8
            && !provider_binds_descriptor(&plan.provider_c, &plan.descriptor_digest))
    {
        return Err(PackageError::provider());
    }
    if descriptor_digest(&plan.descriptor) != plan.descriptor_digest {
        return Err(PackageError::descriptor());
    }
    let descriptor = descriptor::replay(
        &plan.descriptor,
        &plan.descriptor_digest,
        &plan.selected_exports,
    )?;
    let sources = render::render_sources(&descriptor, target, plan.mode);
    let publication = publication::PublicationAuthority::new(output)?;

    // Tool/environment authority is frozen before any stage or artifact is
    // created. There is no PATH fallback and no post-effect environment read.
    let tools = publication::HeldTools::from_environment()?;
    let archive = publication::build_archive(&plan.provider_c, target, &publication, &tools)?;
    let archive_name = target.archive_name();
    let manifest = render::render_manifest(
        target,
        &plan.descriptor,
        &plan.descriptor_digest,
        archive_name,
        plan.mode,
        &plan.provider_sha256,
        [
            ("Cargo.toml", sources.cargo_toml.as_bytes()),
            ("build.rs", sources.build_rs.as_bytes()),
            ("lib.rs", sources.lib_rs.as_bytes()),
            ("owned_data_ffi.rs", sources.ffi_rs.as_bytes()),
            (archive_name, archive.as_slice()),
            ("descriptor.json", &plan.descriptor),
        ],
    );
    render::verify_manifest(
        manifest.as_bytes(),
        target,
        &plan.descriptor,
        &plan.descriptor_digest,
        archive_name,
        plan.mode,
        &plan.provider_sha256,
        [
            ("Cargo.toml", sources.cargo_toml.as_bytes()),
            ("build.rs", sources.build_rs.as_bytes()),
            ("lib.rs", sources.lib_rs.as_bytes()),
            ("owned_data_ffi.rs", sources.ffi_rs.as_bytes()),
            (archive_name, archive.as_slice()),
            ("descriptor.json", &plan.descriptor),
        ],
    )?;
    let files = [
        ("Cargo.toml", sources.cargo_toml.as_bytes()),
        ("build.rs", sources.build_rs.as_bytes()),
        ("lib.rs", sources.lib_rs.as_bytes()),
        ("owned_data_ffi.rs", sources.ffi_rs.as_bytes()),
        (archive_name, archive.as_slice()),
        ("descriptor.json", plan.descriptor.as_slice()),
        (
            "semaprax.native-rust-owned-data-sdk.json",
            manifest.as_bytes(),
        ),
    ];
    let published = publication::publish_package(&publication, files)?;
    publication::verify_published(&publication, &published, files)?;
    Ok(PackageBundle {
        output_directory: output.to_path_buf(),
        manifest_path: output.join("semaprax.native-rust-owned-data-sdk.json"),
        manifest_digest: domain_digest(MANIFEST_DIGEST_DOMAIN, manifest.as_bytes()),
        descriptor_digest: plan.descriptor_digest,
        target_triple: target.triple().to_owned(),
    })
}

fn raw_sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", LowerHex(Sha256::digest(bytes)))
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(bytes);
    format!("sha256:{:x}", LowerHex(hasher.finalize()))
}

fn descriptor_digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(DESCRIPTOR_DIGEST_DOMAIN);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!("sha256:{:x}", LowerHex(hasher.finalize()))
}

fn flat_descriptor_digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(FLAT_DESCRIPTOR_DIGEST_DOMAIN);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!("sha256:{:x}", LowerHex(hasher.finalize()))
}

fn provider_binds_descriptor(provider: &[u8], descriptor_digest: &str) -> bool {
    let Ok(provider) = std::str::from_utf8(provider) else {
        return false;
    };
    let binding = format!("#define SPX_OWNED_DATA_DESCRIPTOR_DIGEST_V1 \"{descriptor_digest}\"");
    provider
        .lines()
        .filter(|line| line.starts_with("#define SPX_OWNED_DATA_DESCRIPTOR_DIGEST_V1 "))
        .eq([binding.as_str()])
}

struct LowerHex<T>(T);

impl<T: AsRef<[u8]>> std::fmt::LowerHex for LowerHex<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.0.as_ref() {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;
    use sha2::{Digest as _, Sha256};

    use super::*;

    fn descriptor_bytes(result: &str) -> Vec<u8> {
        format!(
            "{{\"schema\":\"semaprax.public-owned-data-api.v1\",\"project_schema\":\"semaprax.project.v8\",\"project_revision\":\"sha256:{}\",\"workspace_revision\":\"sha256:{}\",\"project_graph_digest\":\"sha256:{}\",\"exports\":[{{\"stable_id\":\"fixture.value\",\"typescript_name\":\"fixture.value\",\"rust_method_name\":\"spx_fixture_dot_value\",\"parameters\":[],\"result\":\"{result}\"}}],\"limits\":{{\"max_exports\":32,\"max_parameters\":8,\"max_closure_functions\":256,\"max_borrowed_input_bytes\":65536,\"max_owned_output_bytes\":65536,\"max_descriptor_bytes\":1048576}}}}\n",
            "1".repeat(64),
            "2".repeat(64),
            "3".repeat(64),
        )
        .into_bytes()
    }

    #[test]
    fn descriptor_digest_has_the_frozen_length_prefix_and_rejects_scalar_row_drift() {
        let bytes = descriptor_bytes("usize");
        let digest = descriptor_digest(&bytes);
        let selected = vec!["fixture.value".to_owned()];
        let replayed = descriptor::replay(&bytes, &digest, &selected).unwrap();
        assert_eq!(replayed.exports_len(), 1);

        let mut without_length = Sha256::new();
        without_length.update(DESCRIPTOR_DIGEST_DOMAIN);
        without_length.update(&bytes);
        assert_ne!(
            digest,
            format!("sha256:{:x}", LowerHex(without_length.finalize()))
        );

        let mut missing = String::from_utf8(bytes.clone()).unwrap();
        missing = missing.replace("\"result\":\"usize\"", "\"result\":null");
        let missing = missing.into_bytes();
        assert!(descriptor::replay(&missing, &descriptor_digest(&missing), &selected).is_err());

        let mut surplus: Value = serde_json::from_slice(&bytes).unwrap();
        surplus["exports"][0]["surplus"] = Value::Bool(true);
        let mut surplus = serde_json::to_vec(&surplus).unwrap();
        surplus.push(b'\n');
        assert!(descriptor::replay(&surplus, &descriptor_digest(&surplus), &selected).is_err());
    }

    #[test]
    fn provider_binding_is_exact_unique_and_descriptor_specific() {
        let digest = descriptor_digest(&descriptor_bytes("owned-bytes"));
        let line = format!("#define SPX_OWNED_DATA_DESCRIPTOR_DIGEST_V1 \"{digest}\"");
        let provider = format!("#define SPX_NO_ENTRY_WRAPPER 1\n{line}\nint x;\n");
        assert!(provider_binds_descriptor(provider.as_bytes(), &digest));
        assert!(!provider_binds_descriptor(
            format!("{provider}{line}\n").as_bytes(),
            &digest
        ));
        assert!(!provider_binds_descriptor(
            provider.as_bytes(),
            &descriptor_digest(&descriptor_bytes("i64"))
        ));
    }

    #[test]
    fn standalone_source_and_manifest_shape_remain_frozen_while_project_mode_is_additive() {
        let bytes = descriptor_bytes("owned-bytes");
        let digest = descriptor_digest(&bytes);
        let descriptor =
            descriptor::replay(&bytes, &digest, &["fixture.value".to_owned()]).unwrap();
        let target = HostTarget::current().unwrap();
        let standalone =
            render::render_sources(&descriptor, target, PackageMode::StandaloneEvidence);
        let project = render::render_sources(&descriptor, target, PackageMode::ProjectV8);
        assert!(standalone
            .ffi_rs
            .contains("if bytes.capacity()!=length{return Err(Failure::Host)}"));
        assert!(!project
            .ffi_rs
            .contains("if bytes.capacity()!=length{return Err(Failure::Host)}"));
        let archive = target.archive_name();
        let files = [
            ("Cargo.toml", standalone.cargo_toml.as_bytes()),
            ("build.rs", standalone.build_rs.as_bytes()),
            ("lib.rs", standalone.lib_rs.as_bytes()),
            ("owned_data_ffi.rs", standalone.ffi_rs.as_bytes()),
            (archive, b"archive".as_slice()),
            ("descriptor.json", bytes.as_slice()),
        ];
        let standalone_manifest = render::render_manifest(
            target,
            &bytes,
            &digest,
            archive,
            PackageMode::StandaloneEvidence,
            "sha256:provider",
            files,
        );
        let value: Value = serde_json::from_str(&standalone_manifest).unwrap();
        assert_eq!(value["provider"].as_object().unwrap().len(), 3);
        assert!(value["nonclaims"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "no_project_v8_activation"));
    }
}
