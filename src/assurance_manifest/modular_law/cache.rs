//! Bounded logical-query reuse for the installed modular scalar profile.
//!
//! Entries are produced only after registered Z3 success. Each lookup derives
//! the complete current query script and transitive checked-summary key from
//! retained Project HIR; a prior Project certificate is never retargeted.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::assurance_manifest::smt_discharge::{self as smt, DischargeOutcome};
use crate::project::ProjectRevision;
use crate::proof_export::installed::InstalledProofTool;

use super::{
    installed,
    summary::{self, ModularFailure, ModularProof, Prepared, Query, QueryKind},
    BOUNDS_V1,
};

const KEY_DOMAIN: &[u8] = b"semaprax.modular-proof-task.v1\0";
const MAX_ENTRIES: usize = 1024;
const MAX_SNAPSHOT_BYTES: usize = 1024 * 1024;
const SNAPSHOT_SCHEMA: &str = "semaprax.modular-proof-task-cache.v1";
const NUMERIC_MODEL: &str = "semaprax.checked-scalar-qf-lia.v1";
const AXIOMS: &str = "none";

fn digest(parts: &[&str]) -> String {
    let mut hash = Sha256::new();
    hash.update(KEY_DOMAIN);
    for part in parts {
        hash.update((part.len() as u64).to_le_bytes());
        hash.update(part.as_bytes());
    }
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

#[derive(Clone)]
struct TaskKeyFields<'a> {
    role: &'a str,
    owner: &'a str,
    summary: &'a str,
    script: &'a str,
    compiler: &'a str,
    numeric_model: &'a str,
    profile: &'a str,
    axioms: &'a str,
    solver_version: &'a str,
    solver_executable: &'a str,
    version_timeout_ms: u64,
    proof_timeout_ms: u64,
    stream_max: usize,
}

impl TaskKeyFields<'_> {
    fn digest(&self) -> String {
        let version_timeout = self.version_timeout_ms.to_string();
        let proof_timeout = self.proof_timeout_ms.to_string();
        let stream_max = self.stream_max.to_string();
        digest(&[
            "task",
            self.role,
            self.owner,
            self.summary,
            self.script,
            self.compiler,
            self.numeric_model,
            self.profile,
            self.axioms,
            self.solver_version,
            self.solver_executable,
            &version_timeout,
            &proof_timeout,
            &stream_max,
        ])
    }
}

fn script(query: &Query, tool: &InstalledProofTool) -> Result<String, String> {
    let encoded = smt::translate_function(&query.function).map_err(|error| error.detail())?;
    if query.index >= encoded.ensures.len() {
        return Err("cached proof query postcondition is absent".into());
    }
    Ok(smt::render_postcondition_script(
        &encoded,
        query.index,
        tool.proof_timeout_ms(),
    ))
}

fn scripts_for<'a>(prepared: &'a Prepared, id: &'a str) -> impl Iterator<Item = &'a Query> {
    let leaf = prepared
        .callees
        .iter()
        .filter(move |query| query.declaration_id == id);
    let target = prepared.plan.target == id;
    leaf.chain(prepared.preconditions.iter().filter(move |_| target))
        .chain(prepared.caller.iter().filter(move |_| target))
}

fn dependency_keys(
    dependencies: &[super::SummaryDependency],
    keys: &BTreeMap<String, String>,
) -> Result<Vec<String>, String> {
    let mut parts = Vec::with_capacity(dependencies.len() * 2);
    for dependency in dependencies {
        parts.push(dependency.callee.clone());
        parts.push(
            keys.get(&dependency.callee)
                .cloned()
                .ok_or_else(|| "modular dependency order is incomplete".to_owned())?,
        );
    }
    Ok(parts)
}

fn summary_keys(
    prepared: &Prepared,
    tool: &InstalledProofTool,
) -> Result<BTreeMap<String, String>, String> {
    let mut keys = BTreeMap::<String, String>::new();
    for row in &prepared.plan.summaries {
        let mut parts = vec!["summary".to_owned(), row.declaration_id.clone()];
        for query in scripts_for(prepared, &row.declaration_id) {
            parts.push(smt::script_digest(&script(query, tool)?));
        }
        parts.extend(dependency_keys(&row.dependencies, &keys)?);
        let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
        keys.insert(row.declaration_id.clone(), digest(&refs));
    }
    Ok(keys)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckedEntry {
    key: String,
    script_digest: String,
    solver_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    kernel_axioms: Option<Vec<(String, Vec<String>)>>,
}

/// Compiler-owned, process-local checked successes. No public entry writer or
/// serialized certificate constructor exists. The host's authenticated store
/// can persist this opaque state in a separate versioned extension.
pub struct ProofTaskCache {
    project_scope: String,
    entries: BTreeMap<String, CheckedEntry>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    schema: String,
    project_scope: String,
    entries: BTreeMap<String, CheckedEntry>,
}

fn invalid(reason: &str) -> Vec<crate::diagnostic::Diagnostic> {
    vec![crate::diagnostic::Diagnostic::io(
        "SPX-G306",
        format!("modular proof cache refused: {reason}"),
    )]
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn entry_matches(entry: &CheckedEntry, key: &str, script_digest: &str, version: &str) -> bool {
    entry.key == key && entry.script_digest == script_digest && entry.solver_version == version
}

impl ProofTaskCache {
    /// Bind one cache to a caller-held Project root. The original Project
    /// evidence remains exact-revision-bound; this scope limits logical reuse.
    pub fn for_project(root: &std::path::Path) -> Result<Self, Vec<crate::diagnostic::Diagnostic>> {
        let canonical = root
            .canonicalize()
            .map_err(|_| invalid("Project root is unavailable"))?;
        let path = canonical
            .to_str()
            .ok_or_else(|| invalid("Project root is not UTF-8"))?;
        Ok(Self {
            project_scope: digest(&["project-root", path]),
            entries: BTreeMap::new(),
        })
    }

    fn require_project(&self, root: &std::path::Path) -> Result<(), String> {
        let canonical = root
            .canonicalize()
            .map_err(|_| "Project root is unavailable".to_owned())?;
        let path = canonical
            .to_str()
            .ok_or_else(|| "Project root is not UTF-8".to_owned())?;
        if self.project_scope != digest(&["project-root", path]) {
            return Err("proof cache belongs to a different Project root".into());
        }
        Ok(())
    }

    /// Opaque checked-success snapshot for the host-selected authenticated
    /// semantic cache store. No source text, counterexamples or solver output.
    pub(crate) fn encode_snapshot(&self) -> Result<Vec<u8>, Vec<crate::diagnostic::Diagnostic>> {
        let mut bytes = serde_json::to_vec(&Snapshot {
            schema: SNAPSHOT_SCHEMA.into(),
            project_scope: self.project_scope.clone(),
            entries: self.entries.clone(),
        })
        .map_err(|_| invalid("snapshot encoding failed"))?;
        bytes.push(b'\n');
        if bytes.len() > MAX_SNAPSHOT_BYTES {
            return Err(invalid("snapshot exceeds 1 MiB"));
        }
        Ok(bytes)
    }

    /// Called only after the cache store authenticated the full envelope.
    /// Each later lookup still rederives the current checked query and key.
    pub(crate) fn decode_snapshot(
        bytes: &[u8],
    ) -> Result<Self, Vec<crate::diagnostic::Diagnostic>> {
        if bytes.len() > MAX_SNAPSHOT_BYTES {
            return Err(invalid("snapshot exceeds 1 MiB"));
        }
        let snapshot: Snapshot =
            serde_json::from_slice(bytes).map_err(|_| invalid("snapshot schema is malformed"))?;
        if snapshot.schema != SNAPSHOT_SCHEMA
            || !valid_digest(&snapshot.project_scope)
            || snapshot.entries.len() > MAX_ENTRIES
        {
            return Err(invalid("snapshot schema or task inventory is invalid"));
        }
        for (slot, entry) in &snapshot.entries {
            if slot.is_empty()
                || slot.len() > 512
                || !valid_digest(&entry.key)
                || !valid_digest(&entry.script_digest)
                || entry.solver_version.is_empty()
                || entry.solver_version.len() > 256
            {
                return Err(invalid("snapshot task entry is malformed"));
            }
            if let Some(report) = &entry.kernel_axioms {
                if report.is_empty()
                    || report.len() > 1024
                    || report.iter().any(|(theorem, axioms)| {
                        theorem.is_empty()
                            || theorem.len() > 512
                            || axioms.len()
                                > crate::proof_export::kernel_report::STANDARD_AXIOMS.len()
                            || !axioms.windows(2).all(|pair| pair[0] < pair[1])
                            || axioms.iter().any(|axiom| {
                                !crate::proof_export::kernel_report::STANDARD_AXIOMS
                                    .contains(&axiom.as_str())
                            })
                    })
                {
                    return Err(invalid("snapshot kernel axiom report is malformed"));
                }
            }
        }
        let candidate = Self {
            project_scope: snapshot.project_scope,
            entries: snapshot.entries,
        };
        if candidate.encode_snapshot()? != bytes {
            return Err(invalid("snapshot is noncanonical"));
        }
        Ok(candidate)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WorkMetrics {
    pub fresh: usize,
    pub reused: usize,
    pub stale: usize,
}

pub struct CachedProof {
    pub proof: ModularProof,
    pub work: WorkMetrics,
}

/// A cold run and warm run produce the same current Project `ModularProof`.
/// Only `WorkMetrics` may differ. Failure never inserts a proved entry.
pub fn prove_straight_line_installed_cached(
    project_root: &std::path::Path,
    revision: &ProjectRevision,
    target: &str,
    tool: &InstalledProofTool,
    cache: &mut ProofTaskCache,
) -> Result<CachedProof, ModularFailure> {
    cache
        .require_project(project_root)
        .map_err(ModularFailure::CalleeProof)?;
    if !tool.is_modular_scalar() {
        return Err(ModularFailure::CalleeProof(
            "explicit modular scalar process budget required".into(),
        ));
    }
    if !tool.proof_cache_active() {
        return Err(ModularFailure::CalleeProof(
            "proof task was cancelled".into(),
        ));
    }
    let prepared = summary::prepare(revision, target).map_err(ModularFailure::Refused)?;
    let keys = summary_keys(&prepared, tool).map_err(ModularFailure::CalleeProof)?;
    let mut work = WorkMetrics::default();
    // A selected law is one cache transaction. Even a proved early query is
    // not published when a later dependency, budget, or cancellation fails.
    let mut pending = BTreeMap::<String, CheckedEntry>::new();
    let mut next_callee = 0usize;
    let mut next_precondition = 0usize;
    let mut next_caller = 0usize;
    let proof = summary::prove_prepared(prepared, |kind, query| {
        if !tool.proof_cache_active() {
            return Err("proof task was cancelled".into());
        }
        let rendered = script(query, tool)?;
        let script_digest = smt::script_digest(&rendered);
        let owner = if kind == QueryKind::Callee {
            query.declaration_id.as_str()
        } else {
            target
        };
        let summary_key = keys
            .get(owner)
            .ok_or_else(|| "modular task has no dependency-complete summary key".to_owned())?;
        let (kind, ordinal) = match kind {
            QueryKind::Callee => {
                let ordinal = next_callee;
                next_callee += 1;
                ("callee", ordinal)
            }
            QueryKind::Precondition => {
                let ordinal = next_precondition;
                next_precondition += 1;
                ("precondition", ordinal)
            }
            QueryKind::Caller => {
                let ordinal = next_caller;
                next_caller += 1;
                ("caller", ordinal)
            }
        };
        let (version_timeout, proof_timeout, stream_max) = tool.proof_cache_options();
        let task_key = TaskKeyFields {
            role: kind,
            owner,
            summary: summary_key,
            script: &script_digest,
            compiler: env!("CARGO_PKG_VERSION"),
            numeric_model: NUMERIC_MODEL,
            profile: BOUNDS_V1,
            axioms: AXIOMS,
            solver_version: tool.expected_version(),
            solver_executable: tool.proof_cache_executable_digest(),
            version_timeout_ms: version_timeout,
            proof_timeout_ms: proof_timeout,
            stream_max,
        }
        .digest();
        // Logical work excludes revision-scoped expression IDs. Current
        // obligation IDs remain in the newly constructed proof record.
        let slot = format!("{target}:{kind}:{ordinal}");
        if let Some(entry) = pending.get(&slot).or_else(|| cache.entries.get(&slot)) {
            if entry_matches(entry, &task_key, &script_digest, tool.expected_version()) {
                work.reused += 1;
                return Ok(DischargeOutcome::Proved {
                    script_digest,
                    solver_identity: "z3",
                    solver_version: tool.expected_version().into(),
                });
            }
            work.stale += 1;
        }
        if let Some(existing) = pending
            .values()
            .chain(cache.entries.values())
            .find(|entry| entry_matches(entry, &task_key, &script_digest, tool.expected_version()))
            .cloned()
        {
            if pending_capacity_exceeded(&cache.entries, &pending, &slot) {
                return Err("proof task cache capacity exceeded".into());
            }
            pending.insert(slot, existing);
            work.reused += 1;
            return Ok(DischargeOutcome::Proved {
                script_digest,
                solver_identity: "z3",
                solver_version: tool.expected_version().into(),
            });
        }
        // The installed tool independently checks a satisfiable domain and
        // confirms UNSAT; no timeout, cancellation, SAT or unknown is stored.
        let result = installed::discharge(&query.function, query.index, tool)?;
        if matches!(result, DischargeOutcome::Proved { .. }) {
            if !tool.proof_cache_active() {
                return Err("proof task was cancelled".into());
            }
            if pending_capacity_exceeded(&cache.entries, &pending, &slot) {
                return Err("proof task cache capacity exceeded".into());
            }
            pending.insert(
                slot,
                CheckedEntry {
                    key: task_key,
                    script_digest,
                    solver_version: tool.expected_version().into(),
                    kernel_axioms: None,
                },
            );
            work.fresh += 1;
        }
        Ok(result)
    })?;
    if !tool.proof_cache_active() {
        return Err(ModularFailure::CalleeProof(
            "proof task was cancelled".into(),
        ));
    }
    cache.entries.extend(pending);
    Ok(CachedProof { proof, work })
}

fn pending_capacity_exceeded(
    entries: &BTreeMap<String, CheckedEntry>,
    pending: &BTreeMap<String, CheckedEntry>,
    slot: &str,
) -> bool {
    if entries.contains_key(slot) || pending.contains_key(slot) {
        return false;
    }
    let additions = pending
        .keys()
        .filter(|key| !entries.contains_key(*key))
        .count();
    entries.len() + additions >= MAX_ENTRIES
}

/// Share the authenticated checked-success store with another *compiler-owned*
/// installed law route. The caller must derive `logical_subject` from current
/// checked HIR and include every admitted implementation and proof dependency
/// in it. `confirm` is the ordinary complete domain-and-proof checker; a
/// process failure or cancellation never inserts a task.
#[allow(clippy::too_many_arguments)]
pub(crate) fn check_bound_task(
    cache: &mut ProofTaskCache,
    project_root: &std::path::Path,
    tool: &InstalledProofTool,
    role: &str,
    owner: &str,
    logical_subject: &str,
    query_digest: &str,
    profile: &str,
    axioms: &str,
    confirm: impl FnOnce() -> Result<(), Vec<crate::diagnostic::Diagnostic>>,
) -> Result<WorkMetrics, Vec<crate::diagnostic::Diagnostic>> {
    cache
        .require_project(project_root)
        .map_err(|reason| invalid(&reason))?;
    if !tool.proof_cache_active() {
        return Err(invalid("proof task was cancelled"));
    }
    if role.is_empty()
        || owner.is_empty()
        || !valid_digest(logical_subject)
        || !valid_digest(query_digest)
        || profile.is_empty()
        || axioms.is_empty()
    {
        return Err(invalid("checked subject dependency identity is malformed"));
    }
    let (version_timeout_ms, proof_timeout_ms, stream_max) = tool.proof_cache_options();
    let key = TaskKeyFields {
        role,
        owner,
        summary: logical_subject,
        script: query_digest,
        compiler: env!("CARGO_PKG_VERSION"),
        numeric_model: NUMERIC_MODEL,
        profile,
        axioms,
        solver_version: tool.expected_version(),
        solver_executable: tool.proof_cache_executable_digest(),
        version_timeout_ms,
        proof_timeout_ms,
        stream_max,
    }
    .digest();
    let slot = digest(&["slot", role, owner]);
    let mut work = WorkMetrics::default();
    if let Some(entry) = cache.entries.get(&slot) {
        if entry_matches(entry, &key, query_digest, tool.expected_version()) {
            if !tool.proof_cache_active() {
                return Err(invalid("proof task was cancelled"));
            }
            work.reused = 1;
            return Ok(work);
        }
        work.stale = 1;
    }
    if let Some(existing) = cache
        .entries
        .values()
        .find(|entry| entry_matches(entry, &key, query_digest, tool.expected_version()))
        .cloned()
    {
        if cache.entries.len() >= MAX_ENTRIES && !cache.entries.contains_key(&slot) {
            return Err(invalid("proof task cache capacity exceeded"));
        }
        if !tool.proof_cache_active() {
            return Err(invalid("proof task was cancelled"));
        }
        cache.entries.insert(slot, existing);
        work.reused = 1;
        return Ok(work);
    }
    confirm()?;
    if !tool.proof_cache_active() {
        return Err(invalid("proof task was cancelled"));
    }
    if cache.entries.len() >= MAX_ENTRIES && !cache.entries.contains_key(&slot) {
        return Err(invalid("proof task cache capacity exceeded"));
    }
    cache.entries.insert(
        slot,
        CheckedEntry {
            key,
            script_digest: query_digest.into(),
            solver_version: tool.expected_version().into(),
            kernel_axioms: None,
        },
    );
    work.fresh = 1;
    Ok(work)
}

pub(crate) fn logical_subject_digest(parts: &[&str]) -> String {
    digest(parts)
}

fn valid_kernel_report(report: &[(String, Vec<String>)], expected: &[String]) -> bool {
    !expected.is_empty()
        && report.len() == expected.len()
        && report
            .iter()
            .zip(expected)
            .all(|((theorem, axioms), name)| {
                theorem == name
                    && axioms.len() <= crate::proof_export::kernel_report::STANDARD_AXIOMS.len()
                    && axioms.windows(2).all(|pair| pair[0] < pair[1])
                    && axioms.iter().all(|axiom| {
                        crate::proof_export::kernel_report::STANDARD_AXIOMS
                            .contains(&axiom.as_str())
                    })
            })
}

/// A pinned Lean acceptance includes the kernel's exact per-theorem axiom
/// report. Persist only the bounded, closed report, never arbitrary output.
/// A current export must reproduce both the complete Lean source digest and
/// ordered theorem inventory before the checked report can be reused.
#[allow(clippy::too_many_arguments)]
pub(crate) fn check_bound_kernel_task(
    cache: &mut ProofTaskCache,
    project_root: &std::path::Path,
    tool: &InstalledProofTool,
    role: &str,
    owner: &str,
    logical_subject: &str,
    lean_digest: &str,
    profile: &str,
    axioms_policy: &str,
    expected: &[String],
    confirm: impl FnOnce() -> Result<Vec<(String, Vec<String>)>, Vec<crate::diagnostic::Diagnostic>>,
) -> Result<(Vec<(String, Vec<String>)>, WorkMetrics), Vec<crate::diagnostic::Diagnostic>> {
    cache
        .require_project(project_root)
        .map_err(|reason| invalid(&reason))?;
    if !tool.proof_cache_active() {
        return Err(invalid("proof task was cancelled"));
    }
    if role.is_empty()
        || owner.is_empty()
        || !valid_digest(logical_subject)
        || !valid_digest(lean_digest)
        || profile.is_empty()
        || axioms_policy.is_empty()
        || expected.is_empty()
    {
        return Err(invalid("checked kernel subject identity is malformed"));
    }
    let (version_timeout_ms, proof_timeout_ms, stream_max) = tool.proof_cache_options();
    let key = TaskKeyFields {
        role,
        owner,
        summary: logical_subject,
        script: lean_digest,
        compiler: env!("CARGO_PKG_VERSION"),
        numeric_model: NUMERIC_MODEL,
        profile,
        axioms: axioms_policy,
        solver_version: tool.expected_version(),
        solver_executable: tool.proof_cache_executable_digest(),
        version_timeout_ms,
        proof_timeout_ms,
        stream_max,
    }
    .digest();
    let slot = digest(&["slot", role, owner]);
    let mut work = WorkMetrics::default();
    if let Some(entry) = cache.entries.get(&slot) {
        if entry_matches(entry, &key, lean_digest, tool.expected_version()) {
            let report = entry
                .kernel_axioms
                .as_ref()
                .filter(|report| valid_kernel_report(report, expected))
                .ok_or_else(|| invalid("checked kernel axiom report is missing or malformed"))?;
            if !tool.proof_cache_active() {
                return Err(invalid("proof task was cancelled"));
            }
            work.reused = 1;
            return Ok((report.clone(), work));
        }
        work.stale = 1;
    }
    if let Some(existing) = cache
        .entries
        .values()
        .find(|entry| entry_matches(entry, &key, lean_digest, tool.expected_version()))
        .cloned()
    {
        let report = existing
            .kernel_axioms
            .as_ref()
            .filter(|report| valid_kernel_report(report, expected))
            .ok_or_else(|| invalid("checked kernel axiom report is missing or malformed"))?;
        if cache.entries.len() >= MAX_ENTRIES && !cache.entries.contains_key(&slot) {
            return Err(invalid("proof task cache capacity exceeded"));
        }
        if !tool.proof_cache_active() {
            return Err(invalid("proof task was cancelled"));
        }
        let report = report.clone();
        cache.entries.insert(slot, existing);
        work.reused = 1;
        return Ok((report, work));
    }
    let report = confirm()?;
    if !valid_kernel_report(&report, expected) {
        return Err(invalid("installed kernel returned an invalid axiom report"));
    }
    if !tool.proof_cache_active() {
        return Err(invalid("proof task was cancelled"));
    }
    if cache.entries.len() >= MAX_ENTRIES && !cache.entries.contains_key(&slot) {
        return Err(invalid("proof task cache capacity exceeded"));
    }
    cache.entries.insert(
        slot,
        CheckedEntry {
            key,
            script_digest: lean_digest.into(),
            solver_version: tool.expected_version().into(),
            kernel_axioms: Some(report.clone()),
        },
    );
    work.fresh = 1;
    Ok((report, work))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_kernel_report_rejects_wrong_theorem_extra_axiom_and_poisoned_snapshot() {
        let expected = vec!["Semaprax.Proof.ok".to_owned()];
        let valid = vec![(
            expected[0].clone(),
            vec!["Classical.choice".to_owned(), "propext".to_owned()],
        )];
        assert!(valid_kernel_report(&valid, &expected));
        assert!(!valid_kernel_report(
            &valid,
            &["Semaprax.Proof.other".into()]
        ));
        assert!(!valid_kernel_report(
            &[(expected[0].clone(), vec!["sorryAx".into()])],
            &expected
        ));
        assert!(!valid_kernel_report(
            &[(
                expected[0].clone(),
                vec!["propext".into(), "propext".into()]
            )],
            &expected
        ));

        let snapshot = Snapshot {
            schema: SNAPSHOT_SCHEMA.into(),
            project_scope: digest(&["project-root", "fixture"]),
            entries: BTreeMap::from([(
                "slot".into(),
                CheckedEntry {
                    key: digest(&["task"]),
                    script_digest: digest(&["lean"]),
                    solver_version: "leanprover/lean4:v4.34.0".into(),
                    kernel_axioms: Some(vec![(expected[0].clone(), vec!["sorryAx".into()])]),
                },
            )]),
        };
        let mut bytes = serde_json::to_vec(&snapshot).unwrap();
        bytes.push(b'\n');
        assert_eq!(
            ProofTaskCache::decode_snapshot(&bytes).err().unwrap()[0].code,
            "SPX-G306"
        );
    }

    #[test]
    fn missing_dependency_and_forced_key_collision_fail_closed() {
        let dependency = super::super::SummaryDependency {
            callee: "missing.callee".into(),
            digest: digest(&["callee"]),
        };
        assert!(dependency_keys(&[dependency], &BTreeMap::new()).is_err());

        let key = digest(&["task"]);
        let script = digest(&["script"]);
        let poisoned = CheckedEntry {
            key: key.clone(),
            script_digest: digest(&["different-script"]),
            solver_version: "Z3 version 4.12.5".into(),
            kernel_axioms: None,
        };
        assert!(
            !entry_matches(&poisoned, &key, &script, "Z3 version 4.12.5"),
            "a colliding task key without the current script commitment is stale"
        );
        let mut entries = BTreeMap::new();
        entries.insert("slot".into(), poisoned);
        assert!(!pending_capacity_exceeded(
            &entries,
            &BTreeMap::new(),
            "slot"
        ));
    }

    #[test]
    fn task_key_changes_for_every_selected_dependency_and_budget() {
        let base = TaskKeyFields {
            role: "caller",
            owner: "law.total",
            summary: "summary-a",
            script: "script-a",
            compiler: "compiler-a",
            numeric_model: "checked-i64-a",
            profile: "profile-a",
            axioms: "none",
            solver_version: "z3-a",
            solver_executable: "binary-a",
            version_timeout_ms: 2_000,
            proof_timeout_ms: 10_000,
            stream_max: 32_752,
        };
        let expected = base.digest();
        let changes = [
            (
                "role",
                TaskKeyFields {
                    role: "callee",
                    ..base.clone()
                },
            ),
            (
                "owner",
                TaskKeyFields {
                    owner: "law.other",
                    ..base.clone()
                },
            ),
            (
                "summary",
                TaskKeyFields {
                    summary: "summary-b",
                    ..base.clone()
                },
            ),
            (
                "script",
                TaskKeyFields {
                    script: "script-b",
                    ..base.clone()
                },
            ),
            (
                "compiler",
                TaskKeyFields {
                    compiler: "compiler-b",
                    ..base.clone()
                },
            ),
            (
                "model",
                TaskKeyFields {
                    numeric_model: "checked-i64-b",
                    ..base.clone()
                },
            ),
            (
                "profile",
                TaskKeyFields {
                    profile: "profile-b",
                    ..base.clone()
                },
            ),
            (
                "axioms",
                TaskKeyFields {
                    axioms: "axiom-b",
                    ..base.clone()
                },
            ),
            (
                "solver version",
                TaskKeyFields {
                    solver_version: "z3-b",
                    ..base.clone()
                },
            ),
            (
                "solver binary",
                TaskKeyFields {
                    solver_executable: "binary-b",
                    ..base.clone()
                },
            ),
            (
                "version budget",
                TaskKeyFields {
                    version_timeout_ms: 2_001,
                    ..base.clone()
                },
            ),
            (
                "proof budget",
                TaskKeyFields {
                    proof_timeout_ms: 10_001,
                    ..base.clone()
                },
            ),
            (
                "stream bound",
                TaskKeyFields {
                    stream_max: 32_751,
                    ..base.clone()
                },
            ),
        ];
        for (dependency, changed) in changes {
            assert_ne!(expected, changed.digest(), "{dependency} must invalidate");
        }
    }
}
