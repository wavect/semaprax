//! Additive experimental packaging for a compiler-checked opaque-owner bridge.
//! HIR and cleanup replay belong to the caller; this layer authenticates the
//! generated payload, type-checks it with held rustc, and publishes once.
use super::*;
use serde_json::{json, Value};

pub const OPAQUE_OWNER_PACKAGE_SCHEMA: &str =
    "semaprax.native-rust-opaque-owner-sdk.experimental.v1";
const CRATE: &str = "semaprax-generated-native-rust-opaque-owner-sdk";
const DOMAIN: &[u8] = b"semaprax.native-rust-opaque-owner-sdk.manifest.experimental.v1\0";

/// Facts for the additive experimental package; existing owned-data facts keep
/// their original crate-name contract.
pub struct OpaqueOwnerPackageBundle {
    output_directory: PathBuf,
    manifest_path: PathBuf,
    manifest_digest: String,
    descriptor_digest: String,
    target_triple: String,
}
impl OpaqueOwnerPackageBundle {
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
    pub fn target_triple(&self) -> &str {
        &self.target_triple
    }
    pub fn crate_name(&self) -> &'static str {
        CRATE
    }
}

pub struct OpaqueOwnerPackagePlan {
    pub descriptor: Vec<u8>,
    pub provider_c: Vec<u8>,
    pub header: Vec<u8>,
    pub rust_source: Vec<u8>,
}

pub fn build_opaque_owner_package(
    plan: OpaqueOwnerPackagePlan,
    output: &Path,
) -> Result<OpaqueOwnerPackageBundle, PackageError> {
    let target = HostTarget::current().ok_or_else(PackageError::tool)?;
    if plan.descriptor.len() > MAX_DESCRIPTOR_BYTES
        || plan.provider_c.len() > MAX_PROVIDER_BYTES
        || plan.rust_source.len() > MAX_PROVIDER_BYTES
        || plan.header.len() > 65_536
    {
        return Err(PackageError::descriptor());
    }
    let descriptor: Value =
        serde_json::from_slice(&plan.descriptor).map_err(|_| PackageError::descriptor())?;
    let object = descriptor
        .as_object()
        .ok_or_else(PackageError::descriptor)?;
    if object.len() != 8
        || descriptor["schema"] != "semaprax.native-rust-opaque-owner-descriptor.experimental.v1"
        || descriptor["target"] != target.triple()
        || descriptor["provider_sha256"] != raw_sha256(&plan.provider_c)
        || descriptor["header_sha256"] != raw_sha256(&plan.header)
        || descriptor["rust_sha256"] != raw_sha256(&plan.rust_source)
        || descriptor["subject"].as_object().is_none()
        || descriptor["binding"].as_object().is_none()
    {
        return Err(PackageError::descriptor());
    }
    let rustc = descriptor["stable_rustc_version"]
        .as_str()
        .filter(|v| v.len() <= 512)
        .ok_or_else(PackageError::descriptor)?;
    if canonical(&descriptor) != plan.descriptor {
        return Err(PackageError::descriptor());
    }
    let authority = publication::PublicationAuthority::new(output)?;
    let tools = publication::HeldTools::from_environment().map_err(|mut error| {
        error.detail = Some("hold configured C compiler and archiver");
        error
    })?;
    // This held compiler is acquired before any stage. No PATH fallback.
    let compiler = publication::opaque_rust::Compiler::hold(&authority, rustc)?;
    compiler.check(&authority, target, &plan.rust_source)?;
    let archive = publication::build_archive(&plan.provider_c, target, &authority, &tools)
        .map_err(|mut error| {
            error.detail = error.detail.or(Some("compile and archive owner provider"));
            error
        })?;
    let cargo = format!("[package]\nname = {CRATE:?}\nversion = \"0.1.0\"\nedition = \"2021\"\nbuild = \"build.rs\"\n[lib]\npath = \"lib.rs\"\n[workspace]\n");
    let build = build_script::render(target, false);
    let archive_name = target.archive_name();
    let descriptor_digest = raw_sha256(&plan.descriptor);
    let files = [
        ("Cargo.toml", cargo.as_bytes()),
        ("build.rs", build.as_bytes()),
        ("lib.rs", plan.rust_source.as_slice()),
        ("owner.h", plan.header.as_slice()),
        (archive_name, archive.as_slice()),
        ("descriptor.json", plan.descriptor.as_slice()),
    ];
    let manifest = canonical(&json!({
        "schema": OPAQUE_OWNER_PACKAGE_SCHEMA, "target": target.triple(),
        "descriptor_sha256": descriptor_digest,
        "files": files.iter().map(|(path,bytes)| json!({"path":path,"bytes":bytes.len(),"sha256":raw_sha256(bytes)})).collect::<Vec<_>>(),
        "nonclaims": ["experimental_bounded_owner_profile", "no_general_rust_abi", "no_borrowed_or_cross_thread_owners", "no_untrusted_native_sandbox", "no_cross_target_reuse"]
    }));
    let files = [
        files[0],
        files[1],
        files[2],
        files[3],
        files[4],
        files[5],
        ("semaprax.native-rust-sdk.json", manifest.as_slice()),
    ];
    let published = publication::publish_package(&authority, files)?;
    publication::verify_published(&authority, &published, files)?;
    Ok(OpaqueOwnerPackageBundle {
        output_directory: output.to_path_buf(),
        manifest_path: output.join("semaprax.native-rust-sdk.json"),
        manifest_digest: domain_digest(DOMAIN, &manifest),
        descriptor_digest,
        target_triple: target.triple().into(),
    })
}

fn canonical(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(value).expect("JSON value serializes");
    bytes.push(b'\n');
    bytes
}
