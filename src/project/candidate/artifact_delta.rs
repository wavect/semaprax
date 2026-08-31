//! Actual pathless carrier differences; no artifact or package publication.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use std::sync::Arc;

use serde_json::{json, Value};

use super::{wire, ProjectCandidate};
use crate::diagnostic::Diagnostic;
use crate::project::{
    ImageArtifactKind, ProjectRevision, ProjectSemanticImage, IMAGE_ARTIFACT_PROJECTION_SCHEMA,
    MAX_IMAGE_ARTIFACT_BUILD_BYTES, MAX_IMAGE_ARTIFACT_REPORT_BYTES,
};

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
pub const PROJECT_CANDIDATE_ARTIFACT_DELTA_SCHEMA: &str =
    "semaprax.project-candidate-artifact-delta.v1";
pub const PROJECT_CANDIDATE_ARTIFACT_DELTA_VERIFICATION_SCHEMA: &str =
    "semaprax.project-candidate-artifact-delta-verification.v1";
pub const MAX_PROJECT_CANDIDATE_ARTIFACT_DELTA_BYTES: usize = 8 * 1024 * 1024;
const MAX_FACT_BYTES: usize = 32 * 1024 * 1024;
const MAX_ITEMS: usize = 65_536;
const MAX_VISITS: usize = 1_048_576;
const MAX_DEPTH: usize = 128;
const FACT_DOMAIN: &[u8] = b"semaprax.candidate-artifact-delta.fact.v1\0";
const REPORT_DOMAIN: &[u8] = b"semaprax.candidate-artifact-delta.report.v1\0";

#[derive(Default)]
struct Budget {
    bytes: usize,
    visits: usize,
}
impl Budget {
    fn fact(&mut self, value: &Value) -> Result<()> {
        struct Count(usize);
        impl Write for Count {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                if bytes.len() > MAX_PROJECT_CANDIDATE_ARTIFACT_DELTA_BYTES.saturating_sub(self.0) {
                    return Err(io::Error::other("artifact fact limit"));
                }
                self.0 += bytes.len();
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut count = Count(1);
        serde_json::to_writer(&mut count, value).map_err(|_| capacity())?;
        self.bytes = self.bytes.checked_add(count.0).ok_or_else(capacity)?;
        if self.bytes > MAX_FACT_BYTES {
            return Err(capacity());
        }
        Ok(())
    }
    fn copy(&mut self, value: Option<&Value>) -> Result<Value> {
        let value = value.unwrap_or(&Value::Null);
        self.fact(value)?;
        Ok(value.clone())
    }
    fn preflight(&mut self, text: &str) -> Result<()> {
        if text.len() > MAX_IMAGE_ARTIFACT_REPORT_BYTES {
            return Err(capacity());
        }
        let mut quoted = false;
        let mut escaped = false;
        let mut depth = 0usize;
        for byte in text.bytes() {
            if quoted {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == b'"' {
                    quoted = false;
                }
                continue;
            }
            match byte {
                b'"' => {
                    quoted = true;
                    self.visits += 1;
                }
                b'{' | b'[' => {
                    depth += 1;
                    self.visits += 1;
                }
                b'}' | b']' => {
                    depth = depth.checked_sub(1).ok_or_else(invalid)?;
                }
                b',' | b':' => self.visits += 1,
                _ => {}
            }
            if depth > MAX_DEPTH || self.visits > MAX_VISITS {
                return Err(capacity());
            }
        }
        if quoted || depth != 0 {
            return Err(invalid());
        }
        Ok(())
    }
}

struct Projection {
    value: Value,
    digest: String,
}

impl ProjectCandidate {
    /// Independently replay the complete candidate and both existing pathless
    /// carriers. Unsupported profile diagnostics remain those of the builder.
    pub fn artifact_delta(
        &self,
        expected_candidate: &str,
        kind: ImageArtifactKind,
    ) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        let replay = Self::replay(
            Arc::clone(&self.base),
            self.base.project_revision(),
            &self.changes,
            self.to_json().as_bytes(),
        )?;
        let mut budget = Budget::default();
        let before = projection(&replay.base, kind, &mut budget)?;
        let after = projection(&replay.revision, kind, &mut budget)?;
        let base_files = index(&before.value, "artifacts", "path", 64)?;
        let candidate_files = index(&after.value, "artifacts", "path", 64)?;
        let base_exports = index(&before.value, "exports", "id", MAX_ITEMS)?;
        let candidate_exports = index(&after.value, "exports", "id", MAX_ITEMS)?;
        let file_ids = base_files
            .keys()
            .chain(candidate_files.keys())
            .copied()
            .collect::<BTreeSet<_>>();
        let export_ids = base_exports
            .keys()
            .chain(candidate_exports.keys())
            .copied()
            .collect::<BTreeSet<_>>();
        if file_ids.len() + export_ids.len() > MAX_ITEMS {
            return Err(capacity());
        }
        let mut files = Vec::new();
        let mut unchanged_files = 0usize;
        for path in file_ids {
            let base = base_files.get(path).copied();
            let candidate = candidate_files.get(path).copied();
            let bytes_equal = match (base, candidate) {
                (Some(left), Some(right)) => {
                    left["sha256"] == right["sha256"] && left["bytes"] == right["bytes"]
                }
                _ => false,
            };
            unchanged_files += usize::from(bytes_equal);
            let row = json!({"path":path,"change":change(base,candidate,bytes_equal),"bytes_equal":bytes_equal,
                "base":budget.copy(base)?,"candidate":budget.copy(candidate)?});
            budget.fact(&row)?;
            files.push(row);
        }
        let mut exports = Vec::new();
        let mut unchanged_exports = 0usize;
        for id in export_ids {
            let base = base_exports.get(id).copied();
            let candidate = candidate_exports.get(id).copied();
            let exact_equal = base == candidate;
            unchanged_exports += usize::from(exact_equal);
            let row = json!({"id":id,"change":change(base,candidate,exact_equal),"exact_equal":exact_equal,
                "base":budget.copy(base)?,"candidate":budget.copy(candidate)?});
            budget.fact(&row)?;
            exports.push(row);
        }
        let carrier_equal = [
            "carrier_schema",
            "carrier_payload_digest",
            "carrier_envelope_sha256",
            "carrier_envelope_bytes",
        ]
        .into_iter()
        .all(|field| before.value[field] == after.value[field]);
        let artifact_bytes_equal = unchanged_files == files.len();
        let exports_equal = unchanged_exports == exports.len();
        let source_bindings_equal = before.value["sources"] == after.value["sources"];
        let inventory = json!({"base_files":base_files.len(),"candidate_files":candidate_files.len(),
            "changed_files":files.len()-unchanged_files,"unchanged_files":unchanged_files,
            "base_exports":base_exports.len(),"candidate_exports":candidate_exports.len(),
            "changed_exports":exports.len()-unchanged_exports,"unchanged_exports":unchanged_exports});
        // Preserve earlier kind-specific report bytes while describing the
        // new native-source carrier without a compiled-library claim.
        let outside_projection = match kind {
            ImageArtifactKind::C => json!(["rust", "compiled_c_library"]),
            ImageArtifactKind::OpenApi => json!(["rust", "c"]),
            _ => json!(["rust", "c", "openapi"]),
        };
        let export_relationship_scope = match kind {
            ImageArtifactKind::C => {
                "manifest_selected_exports_with_exact_header_native_prototype_or_exclusion"
            }
            ImageArtifactKind::OpenApi => {
                "manifest_selected_exports_with_exact_source_document_and_operation_mapping"
            }
            _ => "manifest_selected_exports_of_whole_carrier_not_per_file_edges",
        };
        let mapping_nonclaim = match kind {
            ImageArtifactKind::C => "native_prototype_mapping_is_not_a_public_linkable_abi",
            ImageArtifactKind::OpenApi => "document_operation_mapping_is_not_live_http_routing",
            _ => "not_per_file_export_mapping",
        };
        render(
            json!({"schema":PROJECT_CANDIDATE_ARTIFACT_DELTA_SCHEMA,"kind":kind.name(),
            "candidate_digest":expected_candidate,"base_project_revision":replay.base.project_revision(),
            "project_revision":replay.revision.project_revision(),"base":before.value,"candidate":after.value,
            "comparison":{"carrier_equal":carrier_equal,"artifact_bytes_equal":artifact_bytes_equal,
                "exports_equal":exports_equal,"source_bindings_equal":source_bindings_equal,
                "base_digest":before.digest,"candidate_digest":after.digest},
            "files":files,"exports":exports,"inventory":inventory,
            "max_build_bytes":MAX_IMAGE_ARTIFACT_BUILD_BYTES,
            "evidence_class":"exact_candidate_replay_and_independently_replayed_pathless_carrier_delta",
            "export_relationship_scope":export_relationship_scope,
            "outside_projection":outside_projection,
            "artifact_materialization":false,"target_execution":false,"source_authority":false,
            "limits":{"max_report_bytes":MAX_PROJECT_CANDIDATE_ARTIFACT_DELTA_BYTES,"max_projection_bytes":MAX_IMAGE_ARTIFACT_REPORT_BYTES,
                "max_fact_work_bytes":MAX_FACT_BYTES,"max_items":MAX_ITEMS,"max_json_syntax_visits":MAX_VISITS,"max_json_depth":MAX_DEPTH},
            "nonclaims":["not_runtime_or_test_coverage","not_external_consumer_usage_or_compatibility",
                "not_package_installation_or_publication",mapping_nonclaim,"no_native_compiler_or_package_manager_execution",
                "no_filesystem_artifact_materialization","no_source_or_publication_authority",
                "outside_projection_does_not_assert_platform_absence","not_allocator_or_RSS_accounting"]}),
        )
    }

    pub fn verify_artifact_delta(
        &self,
        expected_candidate: &str,
        kind: ImageArtifactKind,
        bytes: &[u8],
    ) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        if bytes.len() > MAX_PROJECT_CANDIDATE_ARTIFACT_DELTA_BYTES {
            return Err(capacity());
        }
        // artifact_delta itself owns complete candidate replay and both carrier
        // replays. Submitted bytes are compared, never deserialized as proof.
        if self.artifact_delta(expected_candidate, kind)?.as_bytes() != bytes {
            return Err(vec![Diagnostic::io(
                "SPX-G333",
                "artifact delta failed exact independent candidate and carrier replay",
            )]);
        }
        render(
            json!({"schema":PROJECT_CANDIDATE_ARTIFACT_DELTA_VERIFICATION_SCHEMA,
            "result":"exact_recomputation","kind":kind.name(),"candidate_digest":expected_candidate,
            "base_project_revision":self.base.project_revision(),"project_revision":self.revision.project_revision(),
            "delta_digest":wire::digest(REPORT_DOMAIN,bytes),"artifact_materialization":false,"target_execution":false,"source_authority":false}),
        )
    }
}

fn projection(
    revision: &Arc<ProjectRevision>,
    kind: ImageArtifactKind,
    budget: &mut Budget,
) -> Result<Projection> {
    let image = ProjectSemanticImage::derive(Arc::clone(revision), revision.project_revision())?;
    let text =
        image.artifact_projection(image.image_digest(), kind, MAX_IMAGE_ARTIFACT_BUILD_BYTES)?;
    budget.preflight(&text)?;
    // This is compiler-created bounded projection JSON, not an external HIR or
    // artifact carrier. The image API has already independently replayed it.
    let value: Value = serde_json::from_str(&text).map_err(|_| invalid())?;
    if value["schema"] != IMAGE_ARTIFACT_PROJECTION_SCHEMA
        || value["kind"] != kind.name()
        || value["project_revision"] != revision.project_revision()
        || value["image_revision"] != image.image_digest()
        || value["max_build_bytes"] != MAX_IMAGE_ARTIFACT_BUILD_BYTES
    {
        return Err(invalid());
    }
    budget.fact(&value)?;
    // Canonicalize independently of the existing no-LF projection wire so this
    // additive fact domain always hashes recursively sorted JSON plus one LF.
    let canonical = render(budget.copy(Some(&value))?)?;
    Ok(Projection {
        value,
        digest: wire::digest(FACT_DOMAIN, canonical.as_bytes()),
    })
}

fn index<'a>(
    projection: &'a Value,
    field: &str,
    key: &str,
    max: usize,
) -> Result<BTreeMap<&'a str, &'a Value>> {
    let rows = projection[field].as_array().ok_or_else(invalid)?;
    if rows.len() > max {
        return Err(capacity());
    }
    let mut indexed = BTreeMap::new();
    for row in rows {
        let id = row[key]
            .as_str()
            .filter(|id| !id.is_empty() && id.len() <= 4096)
            .ok_or_else(invalid)?;
        if key == "path" && (row["bytes"].as_u64().is_none() || row["sha256"].as_str().is_none()) {
            return Err(invalid());
        }
        if indexed.insert(id, row).is_some() {
            return Err(invalid());
        }
    }
    Ok(indexed)
}
fn change(base: Option<&Value>, candidate: Option<&Value>, equal: bool) -> &'static str {
    if base.is_none() {
        "added"
    } else if candidate.is_none() {
        "removed"
    } else if equal {
        "unchanged"
    } else {
        "modified"
    }
}
fn render(value: Value) -> Result<String> {
    wire::render(value, MAX_PROJECT_CANDIDATE_ARTIFACT_DELTA_BYTES).map_err(|_| capacity())
}
fn invalid() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G331",
        "source-backed artifact delta inventory is inconsistent",
    )]
}
fn capacity() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G332",
        "source-backed artifact delta exceeds its bounded inventory or output",
    )]
}
