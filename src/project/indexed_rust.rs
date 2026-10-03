//! Explicit source-bound Rust index facts for Project frontend admission.
//! These inputs provide types and graph facts, never foreign-call authority.

use super::*;
use crate::ast::Program;
use crate::native_rust_binding::bind_selected_scalar_signature;
use sha2::{Digest, Sha256};

/// One prepared index signature tied to exact canonical Semaprax source bytes.
/// Native publishers must separately replay the index/package and type-check
/// the selected Rust implementation; this compiler input grants no authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectIndexedRustImport {
    pub source_path: String,
    pub source_sha256: String,
    pub import_id: String,
    pub rust_path: String,
    pub signature: String,
    pub index_digest: String,
    pub receiver: String,
}

fn error(message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-B142", message)]
}

pub(super) fn validate(
    selections: &[ProjectIndexedRustImport],
    sources: &[ProjectFrontendSource],
) -> Result<String, Vec<Diagnostic>> {
    if selections.len() > 32 {
        return Err(error("indexed Project import count exceeds 32"));
    }
    let mut ids = BTreeSet::new();
    let mut hash = Sha256::new();
    hash.update(b"semaprax.project-indexed-rust-input.v1\0");
    let mut sorted = selections.iter().collect::<Vec<_>>();
    sorted.sort_by(|a, b| a.import_id.cmp(&b.import_id));
    for selection in sorted {
        if !ids.insert(&selection.import_id)
            || selection.signature.len() > 8192
            || selection.rust_path.len() > 1024
            || selection.import_id.len() > 128
            || selection.source_path.len() > MAX_PATH_BYTES
            || selection.index_digest.len() != 71
            || !selection.index_digest.starts_with("sha256:")
            || !selection.index_digest[7..]
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            || !matches!(selection.receiver.as_str(), "none" | "shared")
        {
            return Err(error(
                "indexed Project import identity is invalid or duplicated",
            ));
        }
        let source = sources
            .iter()
            .find(|source| source.path() == selection.source_path)
            .ok_or_else(|| error("indexed Project import source is absent"))?;
        let digest = format!(
            "sha256:{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(source.source().as_bytes()))
        );
        if digest != selection.source_sha256 {
            return Err(error("indexed Project import source bytes have drifted"));
        }
        for field in [
            &selection.source_path,
            &selection.source_sha256,
            &selection.import_id,
            &selection.rust_path,
            &selection.signature,
            &selection.index_digest,
            &selection.receiver,
        ] {
            hash.update((field.len() as u64).to_le_bytes());
            hash.update(field.as_bytes());
        }
    }
    Ok(format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hash.finalize())
    ))
}

pub(crate) fn bind_program(
    program: &mut Program,
    selections: &[ProjectIndexedRustImport],
) -> Result<(), Vec<Diagnostic>> {
    for selection in selections
        .iter()
        .filter(|row| row.source_path == program.path)
    {
        let import = program
            .interfaces
            .iter_mut()
            .flat_map(|interface| &mut interface.imports)
            .find(|import| import.stable_id == selection.import_id)
            .ok_or_else(|| error("indexed Project import declaration is absent"))?;
        if !import.index_selected
            || import.rust_path.as_deref() != Some(selection.rust_path.as_str())
        {
            return Err(vec![Diagnostic::error(
                "SPX-B143",
                "indexed Project selection disagrees with source Rust path",
                import.span,
            )
            .at_path(&program.path)]);
        }
        bind_selected_scalar_signature(
            import,
            &selection.signature,
            &selection.index_digest,
            &selection.receiver,
        )
        .map_err(|error| vec![error.at_path(&program.path)])?;
    }
    Ok(())
}

/// Authenticate source files first, then bind the caller's exact prepared
/// index facts before normal Project linking, HIR validation, and graphing.
/// Final held-source rechecks run on both success and refusal.
pub fn with_authenticated_indexed_rust_project<T>(
    manifest_path: &Path,
    selections: &[ProjectIndexedRustImport],
    operation: impl FnOnce(&mut ProjectSnapshot) -> Result<T, Vec<Diagnostic>>,
) -> Result<T, Vec<Diagnostic>> {
    let (snapshot, ()) = load_snapshot_building(manifest_path, |manifest, sources| {
        let sources = sources
            .iter()
            .map(|source| ProjectFrontendSource::new(&source.path, &source.source))
            .collect::<Result<Vec<_>, _>>()?;
        let mut cache = ProjectFrontendCache::new();
        let build = cache.build_indexed_rust(&manifest, &sources, selections)?;
        Ok((build.into_revision(), ()))
    })?;
    with_snapshot_operation(snapshot, operation)
}
