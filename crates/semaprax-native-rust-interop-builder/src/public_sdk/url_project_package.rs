//! Pure, authenticated-project package facts for the RI-06 Url registry route.
//!
//! This module deliberately creates no files and spawns no processes. The host
//! must separately authorize and stage the returned exact files before a
//! locked, offline Cargo invocation. This route has no RI-11 CLI registration.

use super::{raw_digest, sdk_error, target_triple};
use semaprax::diagnostic::Diagnostic;
use semaprax::project::ProjectManifest;
use semaprax_rust_api_index::RustApiIndex;
use std::fmt::Write;

const URL_PACKAGE: &str = "url";
const URL_VERSION: &str = "=2.5.8";
const URL_SOURCE_SHA256: &str =
    "sha256:ff67a8a4397373c3ef660812acab3268222035010ab8680ec4215f38ba3d0eed";
const URL_CONSTRUCTOR: &str = "url_alias::Url::parse";
const URL_MATCH: &str = "url_alias::Url::as_str";
const URL_RESULT_SIGNATURE: &str =
    "fn parse(input: &str) -> core::result::Result<Self, url::ParseError>";
const URL_MATCH_SIGNATURE: &str = "fn as_str(&self) -> &str";

/// Builder-owned proof data. It has no directory, tool, process, or output
/// authority. The public toolchain must recheck every byte after staging.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedUrlProjectPackage {
    project_subject_digest: String,
    target: String,
    binding_plan: Vec<u8>,
    descriptor: Vec<u8>,
    cargo_toml: Vec<u8>,
    cargo_lock: Vec<u8>,
    lib_rs: Vec<u8>,
    c_source: Vec<u8>,
    header: Vec<u8>,
}

impl PreparedUrlProjectPackage {
    pub fn project_subject_digest(&self) -> &str {
        &self.project_subject_digest
    }
    pub fn target(&self) -> &str {
        &self.target
    }
    pub fn binding_plan(&self) -> &[u8] {
        &self.binding_plan
    }
    pub fn descriptor(&self) -> &[u8] {
        &self.descriptor
    }
    pub fn cargo_toml(&self) -> &[u8] {
        &self.cargo_toml
    }
    pub fn cargo_lock(&self) -> &[u8] {
        &self.cargo_lock
    }
    pub fn c_source(&self) -> &[u8] {
        &self.c_source
    }
    pub fn header(&self) -> &[u8] {
        &self.header
    }
    pub fn lib_rs(&self) -> &[u8] {
        &self.lib_rs
    }
}

/// All bytes were previously acquired under the caller's Project/index/lock
/// authority. This constructor only validates and canonically binds them.
pub(crate) fn prepare_url_project_package(
    program: &semaprax::hir::ResolvedProgram,
    export: &str,
    subject_canonical: &[u8],
    subject_digest: &str,
    manifest: &str,
    index_bytes: &[u8],
    cargo_alias: &str,
    source_sha256: &str,
    target: &str,
    stable_rustc_version: &str,
    cargo_lock: &[u8],
) -> Result<PreparedUrlProjectPackage, Vec<Diagnostic>> {
    if subject_canonical.is_empty()
        || !subject_digest.starts_with("sha256:")
        || cargo_alias != "url_alias"
        || source_sha256 != URL_SOURCE_SHA256
        || target != target_triple().unwrap_or("")
        || cargo_lock.len() > 1_048_576
        || !std::str::from_utf8(cargo_lock).is_ok()
    {
        return Err(vec![sdk_error(
            "selected Url Project package facts are invalid",
        )]);
    }
    let manifest = ProjectManifest::parse(manifest)?;
    if manifest.rust_dependencies().len() != 1
        || manifest.rust_dependencies()[0].name() != URL_PACKAGE
        || manifest.rust_dependencies()[0].version() != URL_VERSION
        || !manifest.rust_dependencies()[0].features().is_empty()
    {
        return Err(vec![sdk_error(
            "selected Url Project package requires exact url =2.5.8",
        )]);
    }
    let index = RustApiIndex::replay(index_bytes)
        .map_err(|_| vec![sdk_error("selected Url Project index replay failed")])?;
    index
        .require_package_identity(
            URL_PACKAGE,
            "2.5.8",
            source_sha256,
            target,
            index.feature_digest(),
        )
        .and_then(|_| index.require_cargo_alias_identity(cargo_alias))
        .and_then(|_| index.require_stable_compiler_identity(stable_rustc_version))
        .map_err(|_| vec![sdk_error("selected Url Project package identity drifted")])?;
    if index.feature_digest()
        != "sha256:af269ab39e76ec749dfa30da5ce5878b153b3b47b25fc5bd1a61084a929b17a1"
    {
        return Err(vec![sdk_error(
            "selected Url Project feature closure differs from default/std",
        )]);
    }
    let constructor = index
        .select_closed_url_method("url::Url::parse")
        .map_err(|_| vec![sdk_error("selected Url constructor is unavailable")])?;
    let matcher = index
        .select_closed_url_method("url::Url::as_str")
        .map_err(|_| vec![sdk_error("selected Url view is unavailable")])?;
    if constructor.signature != URL_RESULT_SIGNATURE || matcher.signature != URL_MATCH_SIGNATURE {
        return Err(vec![sdk_error(
            "selected Url signatures are outside the RI-06 profile",
        )]);
    }
    // This first registry route supports one closed, committed Cargo closure.
    // Substring membership is not lock authentication: refuse any byte drift.
    if cargo_lock
        != include_bytes!("../../../semaprax-toolchain/src/fixtures/ri06-url-2.5.8.Cargo.lock")
    {
        return Err(vec![sdk_error(
            "selected Url Cargo closure differs from its pinned lock",
        )]);
    }
    let cargo_toml = format!(
        "[package]\nname=\"ri06-url-owner\"\nversion=\"0.1.0\"\nedition=\"2021\"\npublish=false\n\n[lib]\npath=\"src/lib.rs\"\n\n[workspace]\n\n[dependencies]\nurl_alias={{package=\"url\",version=\"=2.5.8\"}}\n"
    );
    let native = super::url_project_native::render(program, export).map_err(|e| vec![e])?;
    let mut binding = String::new();
    write!(binding,"{{\"schema\":\"semaprax.ri06.url-project-plan.v1\",\"subject\":\"{}\",\"target\":\"{}\",\"constructor\":\"{}\",\"matcher\":\"{}\",\"index\":\"{}\"}}\n",subject_digest,target,URL_CONSTRUCTOR,URL_MATCH,index.digest()).expect("String write");
    let binding = format!(
        "{}{}",
        binding.trim_end_matches('\n').trim_end_matches('}'),
        format!(
            ",\"c_sha256\":\"{}\",\"header_sha256\":\"{}\",\"rust_sha256\":\"{}\"}}\n",
            raw_digest(native.c.as_bytes()),
            raw_digest(native.header.as_bytes()),
            raw_digest(native.rust.as_bytes())
        )
    );
    let mut descriptor = String::new();
    write!(descriptor,"{{\"schema\":\"semaprax.ri06.url-project-descriptor.v1\",\"subject\":\"{}\",\"source_sha256\":\"{}\",\"lock_sha256\":\"{}\"}}\n",subject_digest,source_sha256,raw_digest(cargo_lock)).expect("String write");
    Ok(PreparedUrlProjectPackage {
        project_subject_digest: subject_digest.into(),
        target: target.into(),
        binding_plan: binding.into_bytes(),
        descriptor: descriptor.into_bytes(),
        cargo_toml: cargo_toml.into_bytes(),
        cargo_lock: cargo_lock.to_vec(),
        lib_rs: native.rust.into_bytes(),
        c_source: native.c.into_bytes(),
        header: native.header.into_bytes(),
    })
}
