//! Host-selected source-backed image persistence and retained image refresh.
//! Disk entries contain canonical Project inputs, never trusted serialized HIR.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;
use crate::project_revision_store as store;
use crate::semantic_workspace::SemanticWorkspaceSource;

use super::{
    image, ProjectRevision, ProjectSemanticImage, MAX_SEMANTIC_IMAGE_BYTES,
    PROJECT_SEMANTIC_IMAGE_COMPATIBILITY, PROJECT_SEMANTIC_IMAGE_SCHEMA,
};

pub const SEMANTIC_IMAGE_STORE_SCHEMA: &str = "semaprax.semantic-image-store.v1";
pub const SEMANTIC_IMAGE_REFRESH_SCHEMA: &str = "semaprax.semantic-image-refresh.v1";
pub const MAX_SEMANTIC_IMAGE_STORE_RECEIPT_BYTES: usize = 8192;
pub const MAX_IMAGE_REFRESH_REPORT_BYTES: usize = 65_536;
type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

/// A source-rebuild locator, not proof of authority or current disk existence.
pub struct ImageStoreReceipt {
    json: String,
    digest: String,
    entry: String,
    image: String,
    project: String,
}

impl ImageStoreReceipt {
    pub fn to_json(&self) -> &str {
        &self.json
    }
    pub fn receipt_digest(&self) -> &str {
        &self.digest
    }
    pub fn entry_digest(&self) -> &str {
        &self.entry
    }
    pub fn image_digest(&self) -> &str {
        &self.image
    }
    pub fn project_revision(&self) -> &str {
        &self.project
    }
}

/// Publication uses only the existing secure immutable revision-store route.
/// The host supplies an existing exclusive 0700 root; this never discovers one.
pub fn persist_semantic_image(
    root: &Path,
    image: &ProjectSemanticImage,
    expected_image: &str,
) -> Result<ImageStoreReceipt> {
    require_image(image, expected_image)?;
    let receipt = identify(image)?;
    let published = store::persist(root, image.revision(), image.revision().project_revision())?;
    if published.entry_digest() != receipt.entry_digest()
        || published.project_revision() != receipt.project_revision()
        || published.workspace_revision() != image.revision().workspace_revision()
        || published.project_graph_digest() != image.revision().semantic_graph_digest()
    {
        return Err(stale(
            "published source store differs from the expected image locator",
        ));
    }
    Ok(receipt)
}

/// Cold-load canonical inputs, rebuild through ordinary Project admission, then
/// rederive and authenticate exact image/locator identities. No HIR is loaded.
pub fn load_semantic_image(
    root: &Path,
    receipt_bytes: &[u8],
    expected_image: &str,
) -> Result<Arc<ProjectSemanticImage>> {
    validate_digest(expected_image)?;
    if receipt_bytes.len() > MAX_SEMANTIC_IMAGE_STORE_RECEIPT_BYTES {
        return Err(capacity("image store receipt exceeds its byte bound"));
    }
    let receipt: Value = serde_json::from_slice(receipt_bytes)
        .map_err(|_| invalid("image store receipt is not bounded valid JSON"))?;
    let object = receipt
        .as_object()
        .ok_or_else(|| invalid("image store receipt must be an object"))?;
    const KEYS: &[&str] = &[
        "schema",
        "compiler",
        "image_schema",
        "image_digest",
        "image_bytes",
        "project_revision",
        "workspace_revision",
        "project_graph_digest",
        "revision_store",
        "nonclaims",
    ];
    if object.len() != KEYS.len()
        || KEYS.iter().any(|key| !object.contains_key(*key))
        || receipt["schema"] != SEMANTIC_IMAGE_STORE_SCHEMA
        || receipt["image_schema"] != PROJECT_SEMANTIC_IMAGE_SCHEMA
        || receipt["compiler"] != compiler()
        || receipt["nonclaims"] != nonclaims()
    {
        return Err(invalid(
            "image store receipt schema or compiler compatibility differs",
        ));
    }
    let location = receipt["revision_store"]
        .as_object()
        .ok_or_else(|| invalid("image store locator is missing"))?;
    if location.len() != 2
        || !location.contains_key("entry_digest")
        || receipt["revision_store"]["schema"] != store::PROJECT_REVISION_STORE_ENTRY_SCHEMA
    {
        return Err(invalid("image store locator schema differs"));
    }
    let image_digest = digest_field(&receipt, "image_digest")?;
    let project = digest_field(&receipt, "project_revision")?;
    digest_field(&receipt, "workspace_revision")?;
    digest_field(&receipt, "project_graph_digest")?;
    let entry = digest_field(&receipt["revision_store"], "entry_digest")?;
    if image_digest != expected_image {
        return Err(stale(
            "image store receipt does not name the expected image",
        ));
    }
    let bytes = receipt["image_bytes"]
        .as_u64()
        .ok_or_else(|| invalid("image store receipt lacks its image byte count"))?;
    if bytes == 0 || bytes > MAX_SEMANTIC_IMAGE_BYTES as u64 {
        return Err(capacity("stored image byte count exceeds the image bound"));
    }
    if render(receipt.clone(), MAX_SEMANTIC_IMAGE_STORE_RECEIPT_BYTES)?.as_bytes() != receipt_bytes
    {
        return Err(invalid(
            "image store receipt must have exact canonical bytes",
        ));
    }
    let revision = Arc::new(store::load(root, entry, project)?);
    let image = Arc::new(ProjectSemanticImage::derive(revision, project)?);
    if image.image_digest() != expected_image || image.to_json().len() as u64 != bytes {
        return Err(stale(
            "cold source replay differs from the expected semantic image",
        ));
    }
    if identify(&image)?.to_json().as_bytes() != receipt_bytes {
        return Err(stale(
            "cold source replay differs from the exact image store receipt",
        ));
    }
    Ok(image)
}

/// One retained in-memory image. Refresh owns no store or filesystem authority.
pub struct ImageWorkspace {
    image: Arc<ProjectSemanticImage>,
}

pub struct ImageRefreshReport {
    json: String,
    digest: String,
    reused: bool,
}
impl ImageRefreshReport {
    pub fn to_json(&self) -> &str {
        &self.json
    }
    pub fn report_digest(&self) -> &str {
        &self.digest
    }
    pub fn image_reused(&self) -> bool {
        self.reused
    }
}

impl ImageWorkspace {
    pub fn new(image: Arc<ProjectSemanticImage>) -> Self {
        Self { image }
    }
    pub fn image(&self) -> &Arc<ProjectSemanticImage> {
        &self.image
    }

    /// Reuse the old image Arc only for identical admitted revision facts.
    /// Changed revisions undergo complete source rebuild before replacement.
    pub fn refresh(
        &mut self,
        revision: Arc<ProjectRevision>,
        expected_old_image: &str,
    ) -> Result<ImageRefreshReport> {
        require_image(&self.image, expected_old_image)?;
        let reused = revision.project_revision() == self.image.revision().project_revision();
        let next = if reused {
            if !same_revision(self.image.revision(), &revision) {
                return Err(stale(
                    "unchanged image revision has different retained source facts",
                ));
            }
            Arc::clone(&self.image)
        } else {
            let sources = revision
                .sources()
                .iter()
                .map(|source| SemanticWorkspaceSource {
                    path: source.path().to_owned(),
                    source: source.source().to_owned(),
                })
                .collect();
            let rebuilt = Arc::new(ProjectRevision::from_built(
                revision.manifest().clone(),
                super::build::build_owned(revision.manifest(), sources)?,
            ));
            if !same_revision(&revision, &rebuilt) {
                return Err(stale(
                    "image refresh source rebuild differs from the supplied revision",
                ));
            }
            Arc::new(ProjectSemanticImage::derive(
                rebuilt,
                revision.project_revision(),
            )?)
        };
        let before = self.image.revision();
        let after = next.revision();
        let old = before
            .sources()
            .iter()
            .map(|source| (source.path(), source))
            .collect::<BTreeMap<_, _>>();
        let new = after
            .sources()
            .iter()
            .map(|source| (source.path(), source))
            .collect::<BTreeMap<_, _>>();
        let paths = old
            .keys()
            .chain(new.keys())
            .copied()
            .collect::<BTreeSet<_>>();
        let changed = paths
            .iter()
            .filter(|path| match (old.get(**path), new.get(**path)) {
                (Some(left), Some(right)) => left.source() != right.source(),
                _ => true,
            })
            .map(|path| (*path).to_owned())
            .collect::<BTreeSet<_>>();
        let manifest_changed =
            before.manifest().to_canonical_toml() != after.manifest().to_canonical_toml();
        let inventory_changed = old.keys().ne(new.keys());
        let mut invalidated = if manifest_changed || inventory_changed {
            paths.iter().map(|path| (*path).to_owned()).collect()
        } else {
            changed.clone()
        };
        let mut reverse = BTreeMap::<String, BTreeSet<String>>::new();
        for subject in [before.as_ref(), after.as_ref()] {
            for edge in subject.semantic.image_edges() {
                if matches!(edge.kind(), "function_import" | "type_import") {
                    reverse
                        .entry(edge.target_path().to_owned())
                        .or_default()
                        .insert(edge.caller_path().to_owned());
                }
            }
        }
        let mut pending = invalidated.iter().cloned().collect::<Vec<_>>();
        while let Some(path) = pending.pop() {
            if let Some(consumers) = reverse.get(&path) {
                for consumer in consumers {
                    if invalidated.insert(consumer.clone()) {
                        pending.push(consumer.clone());
                    }
                }
            }
        }
        let unchanged = paths
            .iter()
            .filter(|path| !invalidated.contains(**path))
            .map(|path| (*path).to_owned())
            .collect::<Vec<_>>();
        let value = json!({"schema":SEMANTIC_IMAGE_REFRESH_SCHEMA,
            "old_image_digest":self.image.image_digest(),"image_digest":next.image_digest(),
            "old_project_revision":before.project_revision(),"project_revision":after.project_revision(),
            "changed_sources":changed,"invalidated_sources":invalidated,"unchanged_source_facts":unchanged,
            "manifest_changed":manifest_changed,"source_inventory_changed":inventory_changed,
            "invalidation_basis":"changed_sources_and_union_of_old_new_reverse_module_imports",
            "image_arc_reused":reused,"compiler_work":if reused {"retained_image_arc_reused"} else {"complete_source_rebuild_and_image_derivation"},
            "nonclaims":["not_incremental_compilation","no_unchanged_module_HIR_reuse_claim","no_filesystem_freshness_or_publication_authority","no_persistent_HIR_deserialization"]});
        let json = render(value, MAX_IMAGE_REFRESH_REPORT_BYTES)?;
        let report = ImageRefreshReport {
            digest: hash(
                b"semaprax.semantic-image-refresh.report.v1\0",
                json.as_bytes(),
            ),
            json,
            reused,
        };
        self.image = next;
        Ok(report)
    }
}

fn identify(image: &ProjectSemanticImage) -> Result<ImageStoreReceipt> {
    let location = store::identify(image.revision(), image.revision().project_revision())?;
    let value = json!({"schema":SEMANTIC_IMAGE_STORE_SCHEMA,"compiler":compiler(),"image_schema":PROJECT_SEMANTIC_IMAGE_SCHEMA,
        "image_digest":image.image_digest(),"image_bytes":image.to_json().len(),"project_revision":location.project_revision(),
        "workspace_revision":location.workspace_revision(),"project_graph_digest":location.project_graph_digest(),
        "revision_store":{"schema":store::PROJECT_REVISION_STORE_ENTRY_SCHEMA,"entry_digest":location.entry_digest()},"nonclaims":nonclaims()});
    let json = render(value, MAX_SEMANTIC_IMAGE_STORE_RECEIPT_BYTES)?;
    Ok(ImageStoreReceipt {
        digest: hash(
            b"semaprax.semantic-image-store.receipt.v1\0",
            json.as_bytes(),
        ),
        json,
        entry: location.entry_digest().to_owned(),
        image: image.image_digest().to_owned(),
        project: location.project_revision().to_owned(),
    })
}

fn same_revision(left: &ProjectRevision, right: &ProjectRevision) -> bool {
    left.project_revision() == right.project_revision()
        && left.workspace_revision() == right.workspace_revision()
        && left.manifest().to_canonical_toml() == right.manifest().to_canonical_toml()
        && left.workspace_manifest() == right.workspace_manifest()
        && left.semantic_graph() == right.semantic_graph()
        && left.sources().len() == right.sources().len()
        && left.sources().iter().zip(right.sources()).all(|(a, b)| {
            a.path() == b.path()
                && a.source() == b.source()
                && a.source_digest() == b.source_digest()
                && a.source_revision() == b.source_revision()
                && a.source_graph_schema() == b.source_graph_schema()
        })
}
fn compiler() -> Value {
    json!({"package":env!("CARGO_PKG_NAME"),"version":env!("CARGO_PKG_VERSION"),"image_compatibility":PROJECT_SEMANTIC_IMAGE_COMPATIBILITY})
}
fn nonclaims() -> Value {
    json!([
        "source_backed_store_not_serialized_HIR_cache",
        "no_warm_cross_process_compilation_claim",
        "no_compiler_binary_identity",
        "no_default_or_ambient_store_root",
        "no_source_publication_or_reusable_authority",
        "receipt_not_proof_of_current_entry_existence"
    ])
}
fn hash(domain: &[u8], bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(digest.finalize())
    )
}
fn require_image(image: &ProjectSemanticImage, expected: &str) -> Result<()> {
    validate_digest(expected)?;
    if image.image_digest() != expected {
        return Err(stale("semantic image store or refresh selector is stale"));
    }
    Ok(())
}
fn validate_digest(value: &str) -> Result<()> {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(invalid(
            "image store selector must be a canonical bounded SHA-256 digest",
        ));
    }
    Ok(())
}
fn digest_field<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    let text = value[key]
        .as_str()
        .ok_or_else(|| invalid("image store receipt lacks a digest"))?;
    validate_digest(text)?;
    Ok(text)
}
fn render(value: Value, max: usize) -> Result<String> {
    image::render(value, true, max)
        .map_err(|_| capacity("image store or refresh report exceeds its byte bound"))
}
fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G249", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G250", message)]
}
fn stale(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G251", message)]
}
