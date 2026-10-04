//! One held Project source with independently authenticated Regex and Url
//! registry closures. The two native carriers remain separate generated crates:
//! their owner symbols deliberately cannot be merged by string concatenation.
use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedRegexUrlProjectPackages {
    pub subject_digest: String,
    pub regex: PreparedRegexProjectPackage,
    pub url: PreparedUrlProjectPackage,
}

/// Admit the closed RI-13 Project profile through one held source snapshot and
/// four authenticated registry signatures. This performs no native package
/// preparation; callers that prepare a carrier must still use
/// [`prepare_indexed_regex_url_project_packages`], which checks the current
/// native target and pinned locks before rendering.
pub fn with_authenticated_indexed_regex_url_project<T>(
    manifest_path: &Path,
    regex_selections: &[IndexedProjectRegexRegistrySelection<'_>],
    url_selections: &[IndexedProjectUrlRegistrySelection<'_>],
    operation: impl FnOnce(&mut semaprax::project::ProjectSnapshot) -> Result<T, Vec<Diagnostic>>,
) -> Result<T, Vec<Diagnostic>> {
    let (regex, mut bindings, regex_ids) =
        indexed_project::indexed_regex_project_bindings(regex_selections)?;
    let (url, url_bindings, url_ids) =
        indexed_url_project::indexed_url_project_bindings(url_selections)?;
    if regex.source_path != url.source_path
        || regex.source != url.source
        || regex.package.target != url.package.target
        || regex.package.stable_rustc_version != url.package.stable_rustc_version
    {
        return Err(vec![sdk_error(
            "mixed Regex/Url Project requires one exact source and one target/toolchain identity",
        )]);
    }
    bindings.extend(url_bindings);
    let mut expected_imports = regex_ids;
    expected_imports.extend(url_ids);
    expected_imports.sort();
    semaprax::project::with_authenticated_indexed_rust_project(
        manifest_path,
        &bindings,
        |snapshot| {
            if snapshot.retain_revision().manifest().project_profile()
                != semaprax::project::ProjectProfile::SourceLocalFutureIndexedRustV1
            {
                return Err(vec![sdk_error(
                    "mixed Regex/Url indexed admission requires source-local-future-indexed-rust.v1",
                )]);
            }
            snapshot.with_authenticated_native_rust_sdk_subject(|input| {
                let subject = project::ProjectSdkSubject::from_authenticated(&input)?;
                project::verify_project_subject(subject.canonical.as_bytes(), &subject)
                    .map_err(|error| vec![error])?;
                if subject.imports != expected_imports {
                    return Err(vec![sdk_error(
                        "mixed Regex/Url indexed admission requires the exact four linked imports",
                    )]);
                }
                Ok(())
            })?;
            operation(snapshot)
        },
    )
}

/// Authenticate all four selected imports and both exports under one held
/// Project snapshot. Every generated package binds the same Project subject,
/// while its own exact index and pinned offline lock remain independent.
#[expect(
    clippy::too_many_arguments,
    reason = "four selected APIs and two exact Cargo closures"
)]
pub fn prepare_indexed_regex_url_project_packages(
    manifest_path: &Path,
    regex_selections: &[IndexedProjectRegexRegistrySelection<'_>],
    url_selections: &[IndexedProjectUrlRegistrySelection<'_>],
    regex_export_id: &str,
    url_export_id: &str,
    regex_lock: &[u8],
    url_lock: &[u8],
) -> Result<PreparedRegexUrlProjectPackages, Vec<Diagnostic>> {
    let (regex, mut regex_bindings, regex_ids) =
        indexed_project::indexed_regex_project_bindings(regex_selections)?;
    let (url, url_bindings, url_ids) =
        indexed_url_project::indexed_url_project_bindings(url_selections)?;
    if regex.source_path != url.source_path
        || regex.source != url.source
        || regex.package.target != url.package.target
        || regex.package.stable_rustc_version != url.package.stable_rustc_version
        || regex_export_id == url_export_id
        || regex_export_id.is_empty()
        || url_export_id.is_empty()
    {
        return Err(vec![sdk_error(
            "mixed Regex/Url Project requires one exact source and two distinct exports",
        )]);
    }
    regex_bindings.extend(url_bindings);
    let mut expected_imports = regex_ids;
    expected_imports.extend(url_ids);
    expected_imports.sort();
    let mut expected_exports = [regex_export_id, url_export_id];
    expected_exports.sort();
    semaprax::project::with_authenticated_indexed_rust_project(
        manifest_path,
        &regex_bindings,
        |snapshot| {
            snapshot.with_authenticated_native_rust_sdk_subject(|input| {
                let subject = project::ProjectSdkSubject::from_authenticated(&input)?;
                project::verify_project_subject(subject.canonical.as_bytes(), &subject)
                    .map_err(|error| vec![error])?;
                if subject.imports != expected_imports
                    || subject.exports.len() != 2
                    || subject.exports[0].id != expected_exports[0]
                    || subject.exports[1].id != expected_exports[1]
                {
                    return Err(vec![sdk_error(
                        "mixed Regex/Url Project selection does not cover exact imports and exports",
                    )]);
                }
                let regex_package = regex_project_package::prepare_regex_project_package(
                    input.program(),
                    regex_export_id,
                    subject.canonical.as_bytes(),
                    &subject.digest,
                    &subject.manifest,
                    regex.index_bytes,
                    regex.package.cargo_alias,
                    regex.package.source_sha256,
                    regex.package.target,
                    regex.package.stable_rustc_version,
                    regex_lock,
                    true,
                )?;
                let url_package = url_project_package::prepare_url_project_package(
                    input.program(),
                    url_export_id,
                    subject.canonical.as_bytes(),
                    &subject.digest,
                    &subject.manifest,
                    url.index_bytes,
                    url.package.cargo_alias,
                    url.package.source_sha256,
                    url.package.target,
                    url.package.stable_rustc_version,
                    url_lock,
                    true,
                )?;
                Ok(PreparedRegexUrlProjectPackages {
                    subject_digest: subject.digest,
                    regex: regex_package,
                    url: url_package,
                })
            })
        },
    )
}

#[cfg(test)]
#[path = "indexed_regex_url_project_tests.rs"]
mod tests;
