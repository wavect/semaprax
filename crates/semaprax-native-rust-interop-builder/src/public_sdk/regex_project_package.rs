//! Pure, authenticated-project package facts for the RI-06 Regex registry route.
//!
//! This module deliberately creates no files and spawns no processes. The host
//! stages the returned exact files and RI-11 replays them immediately before
//! its locked, offline Cargo invocation.

use super::{raw_digest, sdk_error, target_triple};
use semaprax::diagnostic::Diagnostic;
use semaprax::project::ProjectManifest;
use semaprax_rust_api_index::RustApiIndex;
use std::fmt::Write;

const REGEX_PACKAGE: &str = "regex";
const REGEX_VERSION: &str = "=1.13.1";
const REGEX_SOURCE_SHA256: &str =
    "sha256:f020237b6c8eed93db2e2cb53c00c60a8e1bc73da7d073199a1180401450218d";
const REGEX_CONSTRUCTOR: &str = "regex_alias::Regex::new";
const REGEX_MATCH: &str = "regex_alias::Regex::is_match";
const REGEX_RESULT_SIGNATURE: &str =
    "fn new(re: &str) -> core::result::Result<regex::Regex, regex::Error>";
const REGEX_MATCH_SIGNATURE: &str = "fn is_match(&self, haystack: &str) -> bool";

/// Builder-owned proof data. It has no directory, tool, process, or output
/// authority. The public toolchain must recheck every byte after staging.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedRegexProjectPackage {
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

impl PreparedRegexProjectPackage {
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
#[expect(
    clippy::too_many_arguments,
    reason = "binds all exact Project package inputs at the authenticated construction boundary"
)]
pub(crate) fn prepare_regex_project_package(
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
) -> Result<PreparedRegexProjectPackage, Vec<Diagnostic>> {
    if subject_canonical.is_empty()
        || !subject_digest.starts_with("sha256:")
        || cargo_alias != "regex_alias"
        || source_sha256 != REGEX_SOURCE_SHA256
        || target != target_triple().unwrap_or("")
        || cargo_lock.len() > 1_048_576
        || !std::str::from_utf8(cargo_lock).is_ok()
    {
        return Err(vec![sdk_error(
            "selected Regex Project package facts are invalid",
        )]);
    }
    let manifest = ProjectManifest::parse(manifest)?;
    if manifest.rust_dependencies().len() != 1
        || manifest.rust_dependencies()[0].name() != REGEX_PACKAGE
        || manifest.rust_dependencies()[0].version() != REGEX_VERSION
        || !manifest.rust_dependencies()[0].features().is_empty()
    {
        return Err(vec![sdk_error(
            "selected Regex Project package requires exact regex =1.13.1",
        )]);
    }
    let index = RustApiIndex::replay(index_bytes)
        .map_err(|_| vec![sdk_error("selected Regex Project index replay failed")])?;
    index
        .require_package_identity(
            REGEX_PACKAGE,
            "1.13.1",
            source_sha256,
            target,
            index.feature_digest(),
        )
        .and_then(|_| index.require_cargo_alias_identity(cargo_alias))
        .and_then(|_| index.require_stable_compiler_identity(stable_rustc_version))
        .map_err(|_| vec![sdk_error("selected Regex Project package identity drifted")])?;
    let constructor = index
        .select_closed_owner_result("regex::Regex::new", "regex::Regex", "regex::Error")
        .map_err(|_| vec![sdk_error("selected Regex constructor is unavailable")])?;
    let matcher = index
        .select_supported(&["regex::Regex::is_match"])
        .map_err(|_| vec![sdk_error("selected Regex matcher is unavailable")])?[0];
    if constructor.signature != REGEX_RESULT_SIGNATURE || matcher.signature != REGEX_MATCH_SIGNATURE
    {
        return Err(vec![sdk_error(
            "selected Regex signatures are outside the RI-06 profile",
        )]);
    }
    // This first registry route supports one closed, committed Cargo closure.
    // Substring membership is not lock authentication: refuse any byte drift.
    if cargo_lock
        != include_bytes!("../../../semaprax-toolchain/src/fixtures/ri06-regex-1.13.1.Cargo.lock")
    {
        return Err(vec![sdk_error(
            "selected Regex Cargo closure differs from its pinned lock",
        )]);
    }
    let cargo_toml = "[package]\nname=\"ri06-regex-owner\"\nversion=\"0.1.0\"\nedition=\"2021\"\npublish=false\n\n[lib]\npath=\"src/lib.rs\"\n\n[workspace]\n\n[dependencies]\nregex_alias={package=\"regex\",version=\"=1.13.1\"}\n".to_string();
    let native = super::regex_project_native::render(program, export).map_err(|e| vec![e])?;
    let mut binding = String::new();
    write!(binding,"{{\"schema\":\"semaprax.ri06.regex-project-plan.v1\",\"subject\":\"{}\",\"target\":\"{}\",\"constructor\":\"{}\",\"matcher\":\"{}\",\"index\":\"{}\"}}\n",subject_digest,target,REGEX_CONSTRUCTOR,REGEX_MATCH,index.digest()).expect("String write");
    let binding = format!(
        "{},\"c_sha256\":\"{}\",\"header_sha256\":\"{}\",\"rust_sha256\":\"{}\"}}\n",
        binding.trim_end_matches('\n').trim_end_matches('}'),
        raw_digest(native.c.as_bytes()),
        raw_digest(native.header.as_bytes()),
        raw_digest(native.rust.as_bytes())
    );
    let mut descriptor = String::new();
    write!(descriptor,"{{\"schema\":\"semaprax.ri06.regex-project-descriptor.v1\",\"subject\":\"{}\",\"source_sha256\":\"{}\",\"lock_sha256\":\"{}\"}}\n",subject_digest,source_sha256,raw_digest(cargo_lock)).expect("String write");
    Ok(PreparedRegexProjectPackage {
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
