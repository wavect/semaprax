//! Exact Url registry bindings replayed through the held Project route.
use super::*;
/// Registry-backed selected import facts. Unlike `IndexedProjectScalarSelection`,
/// this intentionally carries no embedded crate source: the registry closure is
/// bound to the pinned lock and exact selected index/source identities.
#[derive(Clone, Copy)]
pub struct IndexedProjectUrlRegistrySelection<'a> {
    pub source_path: &'a str,
    pub source: &'a str,
    pub import_id: &'a str,
    pub index_bytes: &'a [u8],
    pub package: crate::indexed_binding::SelectedPackage<'a>,
}

pub fn prepare_indexed_url_project_package(
    manifest_path: &std::path::Path,
    selections: &[IndexedProjectUrlRegistrySelection<'_>],
    cargo_lock: &[u8],
) -> Result<PreparedUrlProjectPackage, Vec<semaprax::diagnostic::Diagnostic>> {
    use semaprax::project::ProjectIndexedRustImport;
    use semaprax_rust_api_index::RustApiIndex;
    if selections.len() != 2
        || selections
            .iter()
            .any(|selection| selection.source.len() > MAX_SOURCE_BYTES)
    {
        return Err(vec![sdk_error(
            "selected Url Project requires exactly constructor and matcher bindings",
        )]);
    }
    let first = selections[0];
    if selections.iter().any(|selection| {
        selection.package.cargo_alias != first.package.cargo_alias
            || selection.package.name != first.package.name
            || selection.package.version != first.package.version
            || selection.package.source_sha256 != first.package.source_sha256
            || selection.package.target != first.package.target
            || selection.package.feature_digest != first.package.feature_digest
            || selection.package.stable_rustc_version != first.package.stable_rustc_version
            || selection.index_bytes != first.index_bytes
    }) {
        return Err(vec![sdk_error(
            "selected Url Project bindings must share one exact registry package",
        )]);
    }
    let index = RustApiIndex::replay(first.index_bytes)
        .map_err(|_| vec![sdk_error("selected Url Project index replay failed")])?;
    index
        .require_package_identity(
            first.package.name,
            first.package.version,
            first.package.source_sha256,
            first.package.target,
            first.package.feature_digest,
        )
        .and_then(|_| index.require_cargo_alias_identity(first.package.cargo_alias))
        .and_then(|_| index.require_stable_compiler_identity(first.package.stable_rustc_version))
        .map_err(|_| vec![sdk_error("selected Url Project registry identity drifted")])?;
    let mut bindings = Vec::with_capacity(2);
    let mut ids = Vec::with_capacity(2);
    for selection in selections {
        let program = semaprax::parse(
            selection.source,
            std::path::Path::new(selection.source_path),
        )
        .map_err(|error| vec![error])?;
        let import = program
            .interfaces
            .iter()
            .flat_map(|interface| &interface.imports)
            .find(|import| import.stable_id == selection.import_id)
            .ok_or_else(|| {
                vec![sdk_error(
                    "selected Url Project import declaration is missing",
                )]
            })?;
        let located = |code, message| {
            vec![
                semaprax::diagnostic::Diagnostic::error(code, message, import.span)
                    .at_path(selection.source_path),
            ]
        };
        if !import.index_selected || import.rust_path.is_none() {
            return Err(located(
                "SPX-B140",
                "selected Url Project import must name an indexed Rust declaration",
            ));
        }
        let path = import.rust_path.as_deref().expect("checked");
        let item = match path {
            "url_alias::Url::parse" => index.select_closed_url_method("url::Url::parse"),
            "url_alias::Url::as_str" => index.select_closed_url_method("url::Url::as_str"),
            _ => {
                return Err(located(
                    "SPX-B141",
                    "selected Url Project API is outside the RI-06 profile",
                ))
            }
        }
        .map_err(|_| located("SPX-B141", "selected Url Project API is unavailable"))?;
        let expected = if path.ends_with("::parse") {
            "fn parse(input: &str) -> core::result::Result<Self, url::ParseError>"
        } else {
            "fn as_str(&self) -> &str"
        };
        if item.signature != expected {
            return Err(located(
                "SPX-B145",
                "selected Url Project signature is outside the RI-06 profile",
            ));
        }
        ids.push(selection.import_id.to_owned());
        bindings.push(ProjectIndexedRustImport {
            source_path: selection.source_path.into(),
            source_sha256: raw_digest(selection.source.as_bytes()),
            import_id: selection.import_id.into(),
            rust_path: path.into(),
            signature: item.signature.clone(),
            index_digest: index.digest().into(),
            receiver: if path.ends_with("::parse") {
                "none"
            } else {
                "shared"
            }
            .into(),
        });
    }
    ids.sort();
    semaprax::project::with_authenticated_indexed_rust_project(
        manifest_path,
        &bindings,
        |snapshot| {
            snapshot.with_authenticated_native_rust_sdk_subject(|input| {
                let subject = project::ProjectSdkSubject::from_authenticated(&input)?;
                project::verify_project_subject(subject.canonical.as_bytes(), &subject)
                    .map_err(|error| vec![error])?;
                if subject.imports != ids {
                    return Err(vec![sdk_error(
                        "selected Url Project bindings do not cover the authenticated import set",
                    )]);
                }
                if subject.exports.len() != 1 {
                    return Err(vec![sdk_error(
                        "selected Url Project requires one exact scalar export",
                    )]);
                }
                url_project_package::prepare_url_project_package(
                    input.program(),
                    &subject.exports[0].id,
                    subject.canonical.as_bytes(),
                    &subject.digest,
                    &subject.manifest,
                    first.index_bytes,
                    first.package.cargo_alias,
                    first.package.source_sha256,
                    first.package.target,
                    first.package.stable_rustc_version,
                    cargo_lock,
                )
            })
        },
    )
}
