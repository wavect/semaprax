//! Exact-revision, authority-free references to retained image functions.
//!
//! A reference carries only selection and provenance facts. Resolution checks
//! every fact against the selected current image and freshly derives the
//! function summary and optional facet handle from retained validated HIR.

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::{ImageFacet, ProjectSemanticImage};
use crate::diagnostic::Diagnostic;
use crate::workspace_graph::WorkspaceGraphProjectionModule;

pub const IMAGE_FUNCTION_REFERENCE_SCHEMA: &str = "semaprax.image-function-reference.v1";
pub const IMAGE_FUNCTION_REFERENCE_RESOLUTION_SCHEMA: &str =
    "semaprax.image-function-reference-resolution.v1";
pub const IMAGE_FUNCTION_REFERENCE_REBIND_SCHEMA: &str =
    "semaprax.image-function-reference-rebind.v1";
pub const MAX_IMAGE_FUNCTION_REFERENCE_BYTES: usize = 16 * 1024;
pub const MAX_IMAGE_FUNCTION_REFERENCE_RESOLUTION_BYTES: usize = 128 * 1024;
pub const MAX_IMAGE_FUNCTION_REFERENCE_REBIND_BYTES: usize = 256 * 1024;

const TARGET_KIND: &str = "function";
const MAX_TARGET_BYTES: usize = 4096;
const REFERENCE_DOMAIN: &[u8] = b"semaprax.image-function-reference.payload.v1\0";
const REFERENCE_KEYS: &[&str] = &[
    "schema",
    "reference_revision",
    "image_revision",
    "project_revision",
    "workspace_revision",
    "project_graph_digest",
    "target_kind",
    "target",
    "facet",
    "source",
    "source_authority",
    "execution",
    "publication_authority",
    "nonclaims",
];
const SOURCE_KEYS: &[&str] = &["path", "module", "source_revision", "source_digest"];
const REFERENCE_NONCLAIMS: &[&str] = &[
    "integrity_and_staleness_binding_not_capability_or_secret",
    "exact_revision_only_no_automatic_migration",
    "no_hir_graph_source_or_handle_facts_trusted_from_reference",
    "no_source_execution_candidate_retention_or_publication_authority",
    "no_persistent_server_state_or_general_session_recovery",
];
const RESOLUTION_NONCLAIMS: &[&str] = &[
    "resolved_only_against_exact_current_image_and_source_provenance",
    "function_summary_and_facet_handle_freshly_derived_not_trusted_from_reference",
    "no_cursor_persistence_or_automatic_migration",
    "no_source_execution_candidate_retention_or_publication_authority",
    "no_ranking_or_general_session_recovery",
];
const REBIND_NONCLAIMS: &[&str] = &[
    "no_revision_ancestry_or_semantic_equivalence_inference",
    "stable_identity_survival_does_not_prove_unchanged_signature_contract_body_or_behavior",
    "source_change_classification_is_exact_provenance_not_source_compatibility",
    "rebound_reference_requires_normal_exact_destination_image_resolution",
    "no_source_execution_candidate_migration_retention_or_publication_authority",
    "no_filesystem_refresh_persistent_server_state_or_general_session_recovery",
];

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

impl ProjectSemanticImage {
    /// Export a canonical exact-revision selector, never facts or authority
    /// that a later resolver may adopt as compiler state.
    pub fn export_function_reference(
        &self,
        expected_image: &str,
        target: &str,
        facet: Option<ImageFacet>,
    ) -> Result<String> {
        require_image(self, expected_image)?;
        validate_target(target)?;

        // Use the same membership contract as function-summary navigation.
        self.function_summary(expected_image, target)?;
        let source = function_source(self, target)?;
        let revision = self.revision();
        let mut payload = json!({
            "schema": IMAGE_FUNCTION_REFERENCE_SCHEMA,
            "image_revision": self.image_digest(),
            "project_revision": revision.project_revision(),
            "workspace_revision": revision.workspace_revision(),
            "project_graph_digest": revision.semantic_graph_digest(),
            "target_kind": TARGET_KIND,
            "target": target,
            "facet": facet.map(ImageFacet::name),
            "source": {
                "path": source.path(),
                "module": source.module(),
                "source_revision": source.source_revision(),
                "source_digest": source.source_digest(),
            },
            "source_authority": false,
            "execution": false,
            "publication_authority": false,
            "nonclaims": REFERENCE_NONCLAIMS,
        });
        let canonical_payload = render_reference(payload.clone())?;
        payload["reference_revision"] =
            json!(digest(REFERENCE_DOMAIN, canonical_payload.as_bytes()));
        render_reference(payload)
    }

    /// Resolve only canonical bytes bound to this exact current image and its
    /// exact source provenance. Returned summary and handles are freshly
    /// derived; no fact from the reference becomes trusted compiler state.
    pub fn resolve_function_reference(
        &self,
        expected_image: &str,
        reference_bytes: &[u8],
    ) -> Result<String> {
        require_image(self, expected_image)?;
        if reference_bytes.len() > MAX_IMAGE_FUNCTION_REFERENCE_BYTES {
            return Err(bound("image function reference exceeds its byte bound"));
        }
        if reference_bytes.is_empty() {
            return Err(invalid("image function reference is empty"));
        }

        let mut reference: Value = serde_json::from_slice(reference_bytes)
            .map_err(|_| invalid("image function reference is not valid bounded JSON"))?;
        validate_shape(&reference)?;
        if render_reference(reference.clone())?.as_bytes() != reference_bytes {
            return Err(invalid(
                "image function reference must have exact canonical bytes",
            ));
        }

        let reference_revision = digest_field(&reference, "reference_revision")?.to_owned();
        reference
            .as_object_mut()
            .expect("closed reference object checked")
            .remove("reference_revision");
        let canonical_payload = render_reference(reference.clone())?;
        if digest(REFERENCE_DOMAIN, canonical_payload.as_bytes()) != reference_revision {
            return Err(invalid(
                "image function reference content digest does not match",
            ));
        }

        let image_revision = digest_field(&reference, "image_revision")?;
        let project_revision = digest_field(&reference, "project_revision")?;
        let workspace_revision = digest_field(&reference, "workspace_revision")?;
        let project_graph_digest = digest_field(&reference, "project_graph_digest")?;
        let target = string_field(&reference, "target")?;
        validate_target(target)?;
        let facet = match &reference["facet"] {
            Value::Null => None,
            Value::String(name) => Some(
                ImageFacet::parse(name)
                    .map_err(|_| invalid("image function reference names an unknown facet"))?,
            ),
            _ => {
                return Err(invalid(
                    "image function reference facet must be null or a known name",
                ));
            }
        };
        let source = reference["source"]
            .as_object()
            .expect("closed source object checked");
        let source_path = source["path"].as_str().expect("closed source path checked");
        let source_module = source["module"]
            .as_str()
            .expect("closed source module checked");
        let source_revision = source["source_revision"]
            .as_str()
            .expect("closed source revision checked");
        let source_digest = source["source_digest"]
            .as_str()
            .expect("closed source digest checked");

        let current_source = function_source(self, target)?;
        let current = self.revision();
        if image_revision != self.image_digest()
            || project_revision != current.project_revision()
            || workspace_revision != current.workspace_revision()
            || project_graph_digest != current.semantic_graph_digest()
            || source_path != current_source.path()
            || source_module != current_source.module()
            || source_revision != current_source.source_revision()
            || source_digest != current_source.source_digest()
        {
            return Err(invalid(
                "image function reference is stale or has mismatched provenance",
            ));
        }

        let summary_text = self.function_summary(expected_image, target)?;
        let summary: Value = serde_json::from_str(&summary_text)
            .map_err(|_| invalid("fresh image function summary is invalid"))?;
        let facet_handle = match facet {
            None => Value::Null,
            Some(selected) => Value::String(fresh_facet_handle(&summary, selected)?),
        };
        super::image::render(
            json!({
                "schema": IMAGE_FUNCTION_REFERENCE_RESOLUTION_SCHEMA,
                "reference_revision": reference_revision,
                "image_revision": self.image_digest(),
                "project_revision": current.project_revision(),
                "workspace_revision": current.workspace_revision(),
                "project_graph_digest": current.semantic_graph_digest(),
                "target": target,
                "facet": facet.map(ImageFacet::name),
                "function_summary": summary,
                "facet_handle": facet_handle,
                "source_authority": false,
                "execution": false,
                "publication_authority": false,
                "nonclaims": RESOLUTION_NONCLAIMS,
            }),
            false,
            MAX_IMAGE_FUNCTION_REFERENCE_RESOLUTION_BYTES,
        )
        .map_err(|_| bound("image function reference resolution exceeds its byte bound"))
    }

    /// Rebind one exact reference from a separately admitted source image to
    /// this exact destination image. Only a unique explicit stable function
    /// identity under the same canonical Project/workspace configuration is
    /// eligible. The returned reference is still an ordinary exact-image
    /// selector and must pass normal destination replay before use.
    pub fn rebind_function_reference(
        &self,
        expected_image: &str,
        source_image: &ProjectSemanticImage,
        expected_source_image: &str,
        reference_bytes: &[u8],
    ) -> Result<String> {
        require_image(self, expected_image)?;
        require_image(source_image, expected_source_image)?;
        // This authenticates canonical bytes, content digest, exact source
        // image bindings, exact source provenance, and the original target.
        source_image.resolve_function_reference(expected_source_image, reference_bytes)?;
        let reference: Value = serde_json::from_slice(reference_bytes)
            .map_err(|_| invalid("authenticated image function reference is not JSON"))?;
        let target = string_field(&reference, "target")?;
        let facet = match &reference["facet"] {
            Value::Null => None,
            Value::String(name) => Some(
                ImageFacet::parse(name)
                    .map_err(|_| invalid("image function reference names an unknown facet"))?,
            ),
            _ => {
                return Err(invalid(
                    "image function reference facet must be null or a known name",
                ));
            }
        };
        let reference_revision = digest_field(&reference, "reference_revision")?;
        let old_source = reference["source"]
            .as_object()
            .expect("exact source reference already resolved");
        let source_selection = explicit_function(source_image, target)?;
        let destination_selection = explicit_function(self, target)?;
        let source_revision = source_image.revision();
        let destination_revision = self.revision();
        let same_configuration = source_revision.manifest().to_canonical_toml()
            == destination_revision.manifest().to_canonical_toml()
            && source_revision.workspace_manifest() == destination_revision.workspace_manifest();

        let rejection = if source_image.image_digest() == self.image_digest() {
            Some((
                "cross_revision",
                "source_and_destination_images_are_identical",
            ))
        } else if !same_configuration {
            Some((
                "cross_revision",
                "project_or_workspace_configuration_changed",
            ))
        } else if let FunctionSelection::Rejected(reason) = &source_selection {
            Some(("source", *reason))
        } else if let FunctionSelection::Rejected(reason) = &destination_selection {
            Some(("destination", *reason))
        } else {
            None
        };

        let (accepted, status, destination_source, rebound_reference) = match rejection {
            Some(_) => (false, "rejected", Value::Null, Value::Null),
            None => {
                let FunctionSelection::Accepted(destination) = destination_selection else {
                    unreachable!("rejected destination handled above")
                };
                let path_changed = old_source["path"] != destination.module.path();
                let module_changed = old_source["module"] != destination.module.module();
                let revision_changed =
                    old_source["source_revision"] != destination.module.source_revision();
                let digest_changed =
                    old_source["source_digest"] != destination.module.source_digest();
                let status = if path_changed || module_changed {
                    "rebound_to_moved_explicit_function"
                } else if revision_changed || digest_changed {
                    "rebound_to_changed_source_explicit_function"
                } else {
                    "rebound_to_unchanged_source_explicit_function"
                };
                let rebound = self.export_function_reference(expected_image, target, facet)?;
                (
                    true,
                    status,
                    json!({
                        "path": destination.module.path(),
                        "module": destination.module.module(),
                        "source_revision": destination.module.source_revision(),
                        "source_digest": destination.module.source_digest(),
                    }),
                    json!(rebound),
                )
            }
        };
        let changes = match &destination_source {
            Value::Object(_) => json!({
                "image_revision": source_image.image_digest() != self.image_digest(),
                "project_revision": source_revision.project_revision() != destination_revision.project_revision(),
                "workspace_revision": source_revision.workspace_revision() != destination_revision.workspace_revision(),
                "project_graph_digest": source_revision.semantic_graph_digest() != destination_revision.semantic_graph_digest(),
                "source_path": old_source["path"] != destination_source["path"],
                "source_module": old_source["module"] != destination_source["module"],
                "source_revision": old_source["source_revision"] != destination_source["source_revision"],
                "source_digest": old_source["source_digest"] != destination_source["source_digest"],
            }),
            _ => Value::Null,
        };
        super::image::render(
            json!({
                "schema": IMAGE_FUNCTION_REFERENCE_REBIND_SCHEMA,
                "source_reference_revision": reference_revision,
                "source_image": {
                    "image_revision": source_image.image_digest(),
                    "project_revision": source_revision.project_revision(),
                    "workspace_revision": source_revision.workspace_revision(),
                    "project_graph_digest": source_revision.semantic_graph_digest(),
                    "project_name": source_revision.manifest().name(),
                    "source": reference["source"],
                },
                "destination_image": {
                    "image_revision": self.image_digest(),
                    "project_revision": destination_revision.project_revision(),
                    "workspace_revision": destination_revision.workspace_revision(),
                    "project_graph_digest": destination_revision.semantic_graph_digest(),
                    "project_name": destination_revision.manifest().name(),
                    "source": destination_source,
                },
                "target": target,
                "facet": facet.map(ImageFacet::name),
                "accepted": accepted,
                "status": status,
                "rejection": rejection.map(|(stage,reason)| json!({"stage":stage,"reason":reason})),
                "changes": changes,
                "rebound_reference": rebound_reference,
                "validation": "exact_source_reference_resolution_and_unique_explicit_destination_identity",
                "normal_destination_replay_required": true,
                "source_authority": false,
                "execution": false,
                "publication_authority": false,
                "nonclaims": REBIND_NONCLAIMS,
            }),
            false,
            MAX_IMAGE_FUNCTION_REFERENCE_REBIND_BYTES,
        )
        .map_err(|_| bound("image function reference rebind report exceeds its byte bound"))
    }
}

enum FunctionSelection<'a> {
    Accepted(FunctionIdentity<'a>),
    Rejected(&'static str),
}

struct FunctionIdentity<'a> {
    module: &'a WorkspaceGraphProjectionModule,
}

fn explicit_function<'a>(
    image: &'a ProjectSemanticImage,
    target: &str,
) -> Result<FunctionSelection<'a>> {
    let graph: Value = serde_json::from_str(image.revision().semantic_graph())
        .map_err(|_| invalid("retained Project semantic graph is invalid"))?;
    let declarations = graph["declarations"]
        .as_array()
        .ok_or_else(|| invalid("retained Project declaration inventory is absent"))?;
    let selected = declarations
        .iter()
        .filter(|declaration| declaration["id"] == target)
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Ok(FunctionSelection::Rejected("destination_target_is_absent"));
    }
    if selected.len() != 1 {
        return Ok(FunctionSelection::Rejected("stable_identity_is_not_unique"));
    }
    let declaration = selected[0];
    if declaration["kind"] != "function" {
        return Ok(FunctionSelection::Rejected(
            "stable_identity_is_not_a_function",
        ));
    }
    if declaration["identity_origin"] != "explicit" {
        return Ok(FunctionSelection::Rejected(
            "function_identity_is_not_explicit",
        ));
    }
    let modules = image
        .revision()
        .semantic
        .image_modules()
        .iter()
        .filter(|module| {
            module
                .functions()
                .iter()
                .any(|function| function.id.as_str() == target)
        })
        .collect::<Vec<_>>();
    if modules.len() != 1 {
        return Ok(FunctionSelection::Rejected(
            "retained_function_source_provenance_is_not_unique",
        ));
    }
    let module = modules[0];
    if declaration["path"] != module.path() || declaration["module"] != module.module() {
        return Ok(FunctionSelection::Rejected(
            "retained_function_source_provenance_disagrees",
        ));
    }
    Ok(FunctionSelection::Accepted(FunctionIdentity { module }))
}

fn validate_shape(reference: &Value) -> Result<()> {
    let object = reference
        .as_object()
        .ok_or_else(|| invalid("image function reference must be an object"))?;
    if object.len() != REFERENCE_KEYS.len()
        || REFERENCE_KEYS.iter().any(|key| !object.contains_key(*key))
        || reference["schema"] != IMAGE_FUNCTION_REFERENCE_SCHEMA
        || reference["target_kind"] != TARGET_KIND
        || reference["source_authority"] != false
        || reference["execution"] != false
        || reference["publication_authority"] != false
        || reference["nonclaims"] != json!(REFERENCE_NONCLAIMS)
    {
        return Err(invalid(
            "image function reference has missing, unknown, or changed fields",
        ));
    }
    let source = reference["source"]
        .as_object()
        .ok_or_else(|| invalid("image function reference source must be an object"))?;
    if source.len() != SOURCE_KEYS.len()
        || SOURCE_KEYS.iter().any(|key| !source.contains_key(*key))
        || SOURCE_KEYS
            .iter()
            .any(|key| source.get(*key).and_then(Value::as_str).is_none())
    {
        return Err(invalid(
            "image function reference source provenance is not closed",
        ));
    }
    for key in [
        "reference_revision",
        "image_revision",
        "project_revision",
        "workspace_revision",
        "project_graph_digest",
    ] {
        digest_field(reference, key)?;
    }
    if reference["target"].as_str().is_none() {
        return Err(invalid("image function reference target must be a string"));
    }
    Ok(())
}

fn function_source<'a>(
    image: &'a ProjectSemanticImage,
    target: &str,
) -> Result<&'a WorkspaceGraphProjectionModule> {
    image
        .revision()
        .semantic
        .image_modules()
        .iter()
        .find(|module| {
            module
                .functions()
                .iter()
                .any(|function| function.id.as_str() == target)
        })
        .ok_or_else(|| invalid("image function reference target is unavailable"))
}

fn fresh_facet_handle(summary: &Value, facet: ImageFacet) -> Result<String> {
    summary["facets"]
        .as_array()
        .and_then(|facets| {
            facets.iter().find_map(|row| {
                (row["facet"].as_str() == Some(facet.name()))
                    .then(|| row["handle"].as_str().map(str::to_owned))
                    .flatten()
            })
        })
        .ok_or_else(|| invalid("fresh image function summary lacks its facet handle"))
}

fn validate_target(target: &str) -> Result<()> {
    if target.len() > MAX_TARGET_BYTES {
        return Err(bound(
            "image function reference target exceeds its byte bound",
        ));
    }
    if target.is_empty() || target.contains('\0') {
        return Err(invalid("image function reference target is invalid"));
    }
    Ok(())
}

fn require_image(image: &ProjectSemanticImage, expected: &str) -> Result<()> {
    image.require_digest(expected)
}

fn string_field<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key]
        .as_str()
        .ok_or_else(|| invalid("image function reference lacks a string field"))
}

fn digest_field<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    let value = string_field(value, key)?;
    if !valid_digest(value) {
        return Err(invalid("image function reference lacks a canonical digest"));
    }
    Ok(value)
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

fn render_reference(value: Value) -> Result<String> {
    super::image::render(value, false, MAX_IMAGE_FUNCTION_REFERENCE_BYTES)
        .map_err(|_| bound("image function reference exceeds its byte bound"))
}

fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(digest.finalize())
    )
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G363", message)]
}

fn bound(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G364", message)]
}
