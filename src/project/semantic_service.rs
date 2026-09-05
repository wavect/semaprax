//! Authority-free persistent-process semantic workspace service core.
//!
//! The service retains one indivisible immutable generation and stages all
//! incremental compiler work before replacing it. Filesystem admission and
//! persistence remain explicit host responsibilities outside this module.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;
use crate::workspace_analysis::{
    WorkspaceAnalysisTargetKind, WorkspaceContextOptions, WorkspaceImpactOptions,
};

use super::semantic_service_indexes::SemanticServiceIndexes;
use super::{
    ProjectFrontendCache, ProjectFrontendSource, ProjectManifest, ProjectRevision,
    ProjectSemanticImage, SemanticQuery, SemanticQueryResult, SemanticServiceIndexQuery,
    SemanticServiceIndexResult, SemanticTransaction, SemanticTransactionArtifacts,
    SemanticWorkspaceRevision,
};

mod history;

use history::SemanticWorkspaceServiceHistory;
pub use history::{
    SemanticWorkspaceServiceHistoryEntry, SemanticWorkspaceServiceHistoryQuery,
    SemanticWorkspaceServiceHistoryResult, SemanticWorkspaceServiceHistorySnapshot,
    MAX_SEMANTIC_WORKSPACE_SERVICE_HISTORY_ENTRIES,
    MAX_SEMANTIC_WORKSPACE_SERVICE_HISTORY_QUERY_BYTES,
    MAX_SEMANTIC_WORKSPACE_SERVICE_HISTORY_QUERY_LIMIT,
    MAX_SEMANTIC_WORKSPACE_SERVICE_HISTORY_RESULT_BYTES,
    SEMANTIC_WORKSPACE_SERVICE_HISTORY_ENTRY_SCHEMA,
    SEMANTIC_WORKSPACE_SERVICE_HISTORY_QUERY_SCHEMA,
    SEMANTIC_WORKSPACE_SERVICE_HISTORY_RESULT_SCHEMA,
};

pub const SEMANTIC_WORKSPACE_SERVICE_WORK_SCHEMA: &str =
    "semaprax.semantic-workspace-service-work.v1";
pub const SEMANTIC_WORKSPACE_SERVICE_REFRESH_SCHEMA: &str =
    "semaprax.semantic-workspace-service-refresh.v1";
pub const MAX_SEMANTIC_WORKSPACE_SERVICE_RECEIPT_BYTES: usize = 65_536;

const WORK_DOMAIN: &[u8] = b"semaprax.semantic-workspace-service.work.digest.v1\0";
const REFRESH_DOMAIN: &[u8] = b"semaprax.semantic-workspace-service.refresh.digest.v1\0";
type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

/// One source-backed immutable semantic generation. Its three views can never
/// be replaced independently.
pub struct SemanticWorkspaceGeneration {
    revision: Arc<ProjectRevision>,
    canonical: SemanticWorkspaceRevision,
    image: Arc<ProjectSemanticImage>,
    indexes: SemanticServiceIndexes,
}

impl SemanticWorkspaceGeneration {
    pub fn revision(&self) -> &Arc<ProjectRevision> {
        &self.revision
    }

    pub fn canonical(&self) -> &SemanticWorkspaceRevision {
        &self.canonical
    }

    pub fn image(&self) -> &Arc<ProjectSemanticImage> {
        &self.image
    }

    pub fn workspace_revision(&self) -> &str {
        self.canonical.workspace_revision()
    }

    pub(crate) fn indexes(&self) -> &SemanticServiceIndexes {
        &self.indexes
    }
}

/// An immutable exact-revision handle suitable for concurrent read-only use.
#[derive(Clone)]
pub struct SemanticWorkspaceSnapshot {
    generation: Arc<SemanticWorkspaceGeneration>,
}

impl SemanticWorkspaceSnapshot {
    pub fn generation(&self) -> &Arc<SemanticWorkspaceGeneration> {
        &self.generation
    }

    pub fn workspace_revision(&self) -> &str {
        self.generation.workspace_revision()
    }

    pub fn symbol(&self, id: &str) -> Result<String> {
        self.generation
            .image
            .symbol(self.generation.image.image_digest(), id)
    }

    pub fn context(
        &self,
        target_kind: WorkspaceAnalysisTargetKind,
        target: &str,
        options: WorkspaceContextOptions,
    ) -> Result<String> {
        self.generation.image.context(
            self.generation.image.image_digest(),
            target_kind,
            target,
            options,
        )
    }

    pub fn impact(
        &self,
        target_kind: WorkspaceAnalysisTargetKind,
        target: &str,
        options: WorkspaceImpactOptions,
    ) -> Result<String> {
        self.generation.image.impact(
            self.generation.image.image_digest(),
            target_kind,
            target,
            options,
        )
    }

    pub fn query(&self, query: &SemanticQuery) -> Result<SemanticQueryResult> {
        query.execute(self)
    }

    /// Execute one typed, exact-revision retained-index query.
    pub fn index_query(
        &self,
        query: &SemanticServiceIndexQuery,
    ) -> Result<SemanticServiceIndexResult> {
        query.execute(self)
    }
}

/// Deterministic account of the complete source-derived service open.
pub struct SemanticWorkspaceServiceWork {
    json: String,
    digest: String,
}

impl SemanticWorkspaceServiceWork {
    pub fn to_json(&self) -> &str {
        &self.json
    }

    pub fn receipt_digest(&self) -> &str {
        &self.digest
    }
}

/// Deterministic account of one successfully adopted refresh.
pub struct SemanticWorkspaceServiceRefresh {
    json: String,
    digest: String,
    old_workspace_revision: String,
    workspace_revision: String,
    generation_reused: bool,
}

impl SemanticWorkspaceServiceRefresh {
    pub fn to_json(&self) -> &str {
        &self.json
    }

    pub fn receipt_digest(&self) -> &str {
        &self.digest
    }

    pub fn old_workspace_revision(&self) -> &str {
        &self.old_workspace_revision
    }

    pub fn workspace_revision(&self) -> &str {
        &self.workspace_revision
    }

    pub fn generation_reused(&self) -> bool {
        self.generation_reused
    }
}

/// Long-lived, caller-owned incremental service state with no ambient I/O or
/// publication capability.
pub struct SemanticWorkspaceService {
    active: Arc<SemanticWorkspaceGeneration>,
    frontend: ProjectFrontendCache,
    open_work: SemanticWorkspaceServiceWork,
    history: Mutex<SemanticWorkspaceServiceHistory>,
}

impl SemanticWorkspaceService {
    /// Open from an already admitted immutable revision and prime exact-input
    /// checked-module reuse from its retained source bytes.
    pub fn open(revision: Arc<ProjectRevision>) -> Result<Self> {
        Self::open_with_semantic_cache(revision, ProjectFrontendCache::new_with_semantic_cache())
    }

    /// Open with compiler-created cache state. A persistent cache must already
    /// have been authenticated by the explicit host adapter that supplied it.
    pub fn open_with_semantic_cache(
        revision: Arc<ProjectRevision>,
        mut frontend: ProjectFrontendCache,
    ) -> Result<Self> {
        if !frontend.is_semantic_cache_enabled() {
            return Err(invalid(
                "semantic workspace service requires a checked-module semantic cache",
            ));
        }
        let restored_work = frontend.restored_work().map(parse_value).transpose()?;
        let sources = super::incremental::sources_from_revision(&revision)?;
        let build = frontend.build(revision.manifest(), &sources)?;
        if !same_revision(&revision, build.revision()) {
            return Err(stale(
                "semantic workspace service cache priming disagrees with its admitted revision",
            ));
        }
        let frontend_work = parse_value(build.to_json())?;
        let active = Arc::new(derive_generation(revision)?);
        let json = render(json!({
            "authority": false,
            "frontend_work": frontend_work,
            "image_digest": active.image.image_digest(),
            "limits": {"max_receipt_bytes": MAX_SEMANTIC_WORKSPACE_SERVICE_RECEIPT_BYTES},
            "nonclaims": [
                "no_filesystem_network_process_or_publication_authority",
                "not_full_incremental_semantic_verification",
                "not_peak_heap_or_latency_accounting"
            ],
            "project_revision": active.revision.project_revision(),
            "restored_work": restored_work,
            "schema": SEMANTIC_WORKSPACE_SERVICE_WORK_SCHEMA,
            "workspace_revision": active.workspace_revision(),
        }))?;
        let open_work = SemanticWorkspaceServiceWork {
            digest: hash(WORK_DOMAIN, json.as_bytes()),
            json,
        };
        Ok(Self {
            active,
            frontend,
            open_work,
            history: Mutex::new(SemanticWorkspaceServiceHistory::new()),
        })
    }

    pub fn open_work(&self) -> &SemanticWorkspaceServiceWork {
        &self.open_work
    }

    pub fn active_generation(&self) -> &Arc<SemanticWorkspaceGeneration> {
        &self.active
    }

    pub fn semantic_cache(&self) -> &ProjectFrontendCache {
        &self.frontend
    }

    /// Capture an immutable bounded history view at the exact active revision.
    /// Later successful appends do not change the returned snapshot.
    pub fn history_snapshot(
        &self,
        expected_workspace_revision: &str,
    ) -> Result<SemanticWorkspaceServiceHistorySnapshot> {
        validate_digest(expected_workspace_revision)?;
        if expected_workspace_revision != self.active.workspace_revision() {
            return Err(stale(
                "semantic workspace service history revision is stale",
            ));
        }
        self.history
            .lock()
            .map_err(|_| invalid("semantic workspace service history lock is poisoned"))?
            .snapshot(
                self.active.workspace_revision(),
                self.active.revision.project_revision(),
            )
    }

    /// Execute one exact canonical query against the current immutable history
    /// snapshot. This reads no external state and grants no authority.
    pub fn history_query(
        &self,
        query_bytes: &[u8],
    ) -> Result<SemanticWorkspaceServiceHistoryResult> {
        let query = SemanticWorkspaceServiceHistoryQuery::from_json(query_bytes)?;
        self.history_snapshot(query.expected_workspace_revision())?
            .query(&query)
    }

    /// Execute one exact canonical query against the active immutable generation.
    pub fn query(&self, query_bytes: &[u8]) -> Result<SemanticQueryResult> {
        let query = SemanticQuery::from_json(query_bytes)?;
        let snapshot = SemanticWorkspaceSnapshot {
            generation: Arc::clone(&self.active),
        };
        query.execute(&snapshot)
    }

    /// Admit and execute one exact canonical retained-index query against the
    /// active generation. This reads no filesystem state and grants no authority.
    pub fn index_query(&self, query_bytes: &[u8]) -> Result<SemanticServiceIndexResult> {
        let query = SemanticServiceIndexQuery::from_json(query_bytes)?;
        let snapshot = SemanticWorkspaceSnapshot {
            generation: Arc::clone(&self.active),
        };
        query.execute(&snapshot)
    }

    /// Select only the exact active canonical revision. A returned snapshot
    /// remains internally consistent even after a later service refresh.
    pub fn snapshot(&self, expected_workspace_revision: &str) -> Result<SemanticWorkspaceSnapshot> {
        validate_digest(expected_workspace_revision)?;
        if expected_workspace_revision != self.active.workspace_revision() {
            return Err(stale("semantic workspace service revision is stale"));
        }
        Ok(SemanticWorkspaceSnapshot {
            generation: Arc::clone(&self.active),
        })
    }

    /// Stage a complete cached Project admission, canonical revision, image,
    /// invalidation report, and bounded receipt before adopting either state.
    pub fn refresh_owned_sources(
        &mut self,
        manifest: &ProjectManifest,
        sources: &[ProjectFrontendSource],
        expected_old_workspace_revision: &str,
    ) -> Result<SemanticWorkspaceServiceRefresh> {
        validate_digest(expected_old_workspace_revision)?;
        if expected_old_workspace_revision != self.active.workspace_revision() {
            return Err(stale(
                "semantic workspace service refresh expected revision is stale",
            ));
        }
        let mut history = self
            .history
            .lock()
            .map_err(|_| invalid("semantic workspace service history lock is poisoned"))?;
        history.require_capacity()?;

        let mut frontend = self.frontend.fork();
        let build = frontend.build(manifest, sources)?;
        let frontend_work = parse_value(build.to_json())?;
        let candidate_revision = build.into_revision();
        let candidate = Arc::new(derive_generation(candidate_revision)?);

        let before = &self.active.revision;
        let after = &candidate.revision;
        let (changed, invalidated, manifest_changed, inventory_changed) =
            invalidation(before, after);
        let generation_reused = candidate.workspace_revision() == self.active.workspace_revision();
        if generation_reused && !same_revision(before, after) {
            return Err(stale(
                "unchanged canonical workspace revision has different retained Project facts",
            ));
        }
        let adopted = if generation_reused {
            Arc::clone(&self.active)
        } else {
            candidate
        };
        let old_workspace_revision = self.active.workspace_revision().to_owned();
        let workspace_revision = adopted.workspace_revision().to_owned();
        let json = render(json!({
            "authority": false,
            "changed_sources": changed,
            "frontend_work": frontend_work,
            "generation_arc_reused": generation_reused,
            "image_digest": adopted.image.image_digest(),
            "invalidated_sources": invalidated,
            "invalidation_basis": "changed_sources_and_union_of_old_new_reverse_module_imports",
            "limits": {"max_receipt_bytes": MAX_SEMANTIC_WORKSPACE_SERVICE_RECEIPT_BYTES},
            "manifest_changed": manifest_changed,
            "nonclaims": [
                "no_filesystem_freshness_or_publication_authority",
                "not_function_level_incremental_verification",
                "not_peak_heap_or_latency_accounting"
            ],
            "old_image_digest": self.active.image.image_digest(),
            "old_project_revision": before.project_revision(),
            "old_workspace_revision": old_workspace_revision,
            "project_revision": adopted.revision.project_revision(),
            "schema": SEMANTIC_WORKSPACE_SERVICE_REFRESH_SCHEMA,
            "source_inventory_changed": inventory_changed,
            "workspace_revision": workspace_revision,
        }))?;
        let receipt = SemanticWorkspaceServiceRefresh {
            digest: hash(REFRESH_DOMAIN, json.as_bytes()),
            json,
            old_workspace_revision,
            workspace_revision,
            generation_reused,
        };

        let history_entry = history.refresh_entry(
            before.project_revision(),
            receipt.old_workspace_revision(),
            adopted.revision.project_revision(),
            receipt.workspace_revision(),
            receipt.receipt_digest(),
        )?;

        self.active = adopted;
        self.frontend = frontend;
        history.append(history_entry);
        Ok(receipt)
    }

    /// Validate exact canonical transaction bytes against the selected active
    /// generation. Candidate evidence is returned without changing service state.
    pub fn validate_transaction(
        &self,
        transaction_bytes: &[u8],
    ) -> Result<SemanticTransactionArtifacts> {
        let mut history = self
            .history
            .lock()
            .map_err(|_| invalid("semantic workspace service history lock is poisoned"))?;
        history.require_capacity()?;
        let transaction = SemanticTransaction::from_json(transaction_bytes)?;
        if transaction.expected_workspace_revision() != self.active.workspace_revision() {
            return Err(stale(
                "semantic workspace service transaction revision is stale",
            ));
        }
        let artifacts = transaction.validate(Arc::clone(&self.active.revision))?;
        let candidate_workspace = artifacts
            .candidate()
            .revision()
            .canonical_workspace_revision()?;
        let history_entry = history.transaction_entry(
            self.active.revision.project_revision(),
            self.active.workspace_revision(),
            artifacts.candidate().revision().project_revision(),
            candidate_workspace.workspace_revision(),
            transaction.digest(),
            artifacts.result_digest(),
        )?;
        history.append(history_entry);
        Ok(artifacts)
    }
}

fn derive_generation(revision: Arc<ProjectRevision>) -> Result<SemanticWorkspaceGeneration> {
    let canonical = revision.canonical_workspace_revision()?;
    let image = Arc::new(ProjectSemanticImage::derive(
        Arc::clone(&revision),
        revision.project_revision(),
    )?);
    let indexes = SemanticServiceIndexes::derive(&revision)?;
    Ok(SemanticWorkspaceGeneration {
        revision,
        canonical,
        image,
        indexes,
    })
}

fn invalidation(
    before: &ProjectRevision,
    after: &ProjectRevision,
) -> (BTreeSet<String>, BTreeSet<String>, bool, bool) {
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
    for revision in [before, after] {
        for edge in revision.semantic.image_edges() {
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
    (changed, invalidated, manifest_changed, inventory_changed)
}

fn same_revision(left: &ProjectRevision, right: &ProjectRevision) -> bool {
    left.project_revision() == right.project_revision()
        && left.workspace_revision() == right.workspace_revision()
        && left.manifest().to_canonical_toml() == right.manifest().to_canonical_toml()
        && left.workspace_manifest() == right.workspace_manifest()
        && left.semantic_graph() == right.semantic_graph()
        && left.sources().len() == right.sources().len()
        && left
            .sources()
            .iter()
            .zip(right.sources())
            .all(|(left, right)| {
                left.path() == right.path()
                    && left.source() == right.source()
                    && left.source_revision() == right.source_revision()
                    && left.source_digest() == right.source_digest()
            })
}

fn parse_value(source: &str) -> Result<Value> {
    serde_json::from_str(source)
        .map_err(|_| invalid("semantic workspace service retained work is not valid JSON"))
}

fn render(mut value: Value) -> Result<String> {
    value.sort_all_objects();
    let mut json = serde_json::to_string(&value)
        .map_err(|_| invalid("semantic workspace service receipt cannot be rendered"))?;
    json.push('\n');
    if json.len() > MAX_SEMANTIC_WORKSPACE_SERVICE_RECEIPT_BYTES {
        return Err(capacity(
            "semantic workspace service receipt exceeds its byte limit",
        ));
    }
    Ok(json)
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

fn validate_digest(value: &str) -> Result<()> {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(invalid(
            "semantic workspace service revision digest is invalid",
        ));
    }
    Ok(())
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G528", message)]
}

fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G529", message)]
}

fn stale(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G530", message)]
}
