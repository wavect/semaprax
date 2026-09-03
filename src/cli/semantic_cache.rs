//! Explicit derived-cache storage; never canonical source publication.
use semaprax::diagnostic::Diagnostic;
use semaprax::image_transport::{VNextPolicy, VNextSession};
use semaprax::semantic_cache_store;
use serde_json::json;
use std::path::{Path, PathBuf};

fn absolute(path: &Path) -> Result<PathBuf, Vec<Diagnostic>> {
    if path.is_absolute() {
        return Ok(path.to_owned());
    }
    std::env::current_dir()
        .map(|root| root.join(path))
        .map_err(|_| {
            vec![Diagnostic::io(
                "SPX-G306",
                "cannot resolve semantic cache manifest",
            )]
        })
}
pub(crate) fn initialize(root: &Path) -> Result<String, Vec<Diagnostic>> {
    // Constant output prepared before a filesystem pivot.
    let output =
        "{\"schema\":\"semaprax.semantic-cache-initialized.v1\",\"source_authority\":false}\n"
            .to_owned();
    semantic_cache_store::initialize(root)?;
    Ok(output)
}
pub(crate) fn persist(manifest: &Path, root: &Path) -> Result<String, Vec<Diagnostic>> {
    let mut session =
        VNextSession::open_with_semantic_cache(&absolute(manifest)?, VNextPolicy::default())?;
    let cache = session.retained_semantic_cache()?;
    session.finish()?;
    // Live input authentication is complete. This is an immutable historical
    // cache subject, not a claim that source remains unchanged during storage.
    let receipt = semantic_cache_store::persist(root, &cache)?;
    let mut value = json!({
        "schema":"semaprax.semantic-cache-receipt.v1",
        "entry_digest":receipt.entry_digest(),
        "compiler_digest":receipt.compiler_digest(),
        "payload_bytes":receipt.payload_bytes(),
        "source_authority":false,
        "current_source_admission":false,
        "commit_approval":false,
    });
    value.sort_all_objects();
    Ok(format!("{value}\n"))
}
pub(crate) fn load(root: &Path, expected: &str) -> Result<String, Vec<Diagnostic>> {
    let cache = semantic_cache_store::load(root, expected)?;
    cache.restored_work().map(str::to_owned).ok_or_else(|| {
        vec![Diagnostic::io(
            "SPX-G307",
            "semantic cache load did not retain warm replay work",
        )]
    })
}

pub(crate) fn evict(root: &Path, expected: &str) -> Result<String, Vec<Diagnostic>> {
    let receipt = semantic_cache_store::evict(root, expected)?;
    let mut value = json!({
        "schema":"semaprax.semantic-cache-eviction.v1",
        "entry_digest":receipt.entry_digest(),
        "envelope_bytes":receipt.envelope_bytes(),
        "entries_remaining":receipt.entries_remaining(),
        "source_authority":false,
        "canonical_source_mutation":false,
        "publication_authority":false,
        "cache_management_effect":"selected_entry_removed",
    });
    value.sort_all_objects();
    Ok(format!("{value}\n"))
}
