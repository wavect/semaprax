//! Version-matched, authority-free guidance embedded in the installed compiler.
//!
//! These documents package existing compiler-checked and generated resources.
//! They are descriptive guidance, not executable input, a complete grammar or
//! graph ABI, or a grant of any host capability.

use std::collections::BTreeSet;

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;
use crate::project::{
    MAX_SEMANTIC_QUERY_BYTES, MAX_SEMANTIC_QUERY_CONSUMER_LIMIT,
    MAX_SEMANTIC_QUERY_CONSUMER_OFFSET, MAX_SEMANTIC_QUERY_DECLARATION_LIMIT,
    MAX_SEMANTIC_QUERY_DECLARATION_OFFSET, MAX_SEMANTIC_QUERY_RESULT_BYTES,
    PROJECT_SEMANTIC_CONTEXT_SCHEMA, PROJECT_SEMANTIC_GRAPH_SCHEMA, PROJECT_SEMANTIC_IMAGE_SCHEMA,
    PROJECT_SEMANTIC_IMAGE_SYMBOL_SCHEMA, PROJECT_SEMANTIC_IMPACT_SCHEMA,
    SEMANTIC_QUERY_AVAILABLE_OPERATIONS_SCHEMA, SEMANTIC_QUERY_DECLARATIONS_SCHEMA,
    SEMANTIC_QUERY_DECLARATION_CONSUMERS_SCHEMA, SEMANTIC_QUERY_OWNERSHIP_AT_EXPRESSION_SCHEMA,
    SEMANTIC_QUERY_RESULT_SCHEMA, SEMANTIC_QUERY_SCHEMA, SEMANTIC_SERVICE_INDEX_QUERY_SCHEMA,
    SEMANTIC_TRANSACTION_EVIDENCE_SCHEMA, SEMANTIC_TRANSACTION_RESULT_SCHEMA,
    SEMANTIC_TRANSACTION_SCHEMA, SEMANTIC_WORKSPACE_SERVICE_HISTORY_QUERY_SCHEMA,
};

pub const INSTALLED_SKILL_SCHEMA: &str = "semaprax.installed-skill.v1";
pub const INSTALLED_QUERY_CAPABILITIES_SCHEMA: &str = "semaprax.installed-query-capabilities.v1";
pub const MAX_INSTALLED_GUIDANCE_BYTES: usize = 1024 * 1024;

const SKILL_DOMAIN: &[u8] = b"semaprax.installed-skill.payload.digest.v1\0";
const CAPABILITIES_DOMAIN: &[u8] = b"semaprax.installed-query-capabilities.payload.digest.v1\0";
const SOURCE_DOMAIN: &[u8] = b"semaprax.installed-guidance.source.digest.v1\0";

const QUICK_REFERENCE: &str = include_str!("../docs/AGENT-QUICK-REFERENCE.md");
const SHAPES_CATALOG: &str = include_str!("../docs/LANGUAGE-SHAPES-CATALOG.md");
const STDLIB_GUIDE: &str = include_str!("../docs/STANDARD-LIBRARY-CATALOG.md");
const STDLIB_CATALOG: &str = include_str!("../std/catalog.json");
const PACKAGE_CATALOG: &str = include_str!("../std/packages.json");

const COMMON_NONCLAIMS: &[&str] = &[
    "descriptive_installed_guidance_not_compiler_input",
    "no_binary_identity_or_reproducible_build_attestation",
    "no_filesystem_process_network_execution_or_publication_authority",
    "no_host_capability_grant",
];

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

/// The closed installed guidance selector inventory.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum InstalledSkill {
    Agent,
    Language,
    Graph,
    Stdlib,
    Packages,
    Effects,
}

impl InstalledSkill {
    pub const ALL: [Self; 6] = [
        Self::Agent,
        Self::Language,
        Self::Graph,
        Self::Stdlib,
        Self::Packages,
        Self::Effects,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Language => "language",
            Self::Graph => "graph",
            Self::Stdlib => "stdlib",
            Self::Packages => "packages",
            Self::Effects => "effects",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.as_str() == value)
    }
}

/// One immutable canonical installed-guidance envelope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstalledGuidance {
    schema: &'static str,
    digest: String,
    json: String,
}

impl InstalledGuidance {
    pub fn schema(&self) -> &'static str {
        self.schema
    }

    /// Domain-separated digest of the exact canonical payload bytes.
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Exact recursively key-sorted canonical envelope, terminated by one LF.
    pub fn to_json(&self) -> &str {
        &self.json
    }
}

/// Package one of the six closed, version-matched installed skills.
pub fn installed_skill(skill: InstalledSkill) -> Result<InstalledGuidance> {
    let (content, sources, extra_nonclaims) = match skill {
        InstalledSkill::Agent => (
            json!({
                "quick_reference": QUICK_REFERENCE,
                "semantic_protocols": {
                    "query": SEMANTIC_QUERY_SCHEMA,
                    "query_result": SEMANTIC_QUERY_RESULT_SCHEMA,
                    "transaction": SEMANTIC_TRANSACTION_SCHEMA,
                    "transaction_evidence": SEMANTIC_TRANSACTION_EVIDENCE_SCHEMA,
                    "transaction_result": SEMANTIC_TRANSACTION_RESULT_SCHEMA,
                },
                "transaction_operations": transaction_operations(),
            }),
            vec![source(
                "agent-quick-reference",
                "markdown",
                None,
                QUICK_REFERENCE,
            )],
            vec!["quick_reference_narrative_is_compiler_checked_not_fully_generated"],
        ),
        InstalledSkill::Language => (
            json!({
                "declaration_shapes": SHAPES_CATALOG,
                "quick_reference": QUICK_REFERENCE,
            }),
            vec![
                source("agent-quick-reference", "markdown", None, QUICK_REFERENCE),
                source("language-shapes-catalog", "markdown", None, SHAPES_CATALOG),
            ],
            vec!["example_derived_shapes_are_not_a_complete_formal_grammar"],
        ),
        InstalledSkill::Graph => (
            json!({
                "declaration_shapes": SHAPES_CATALOG,
                "project_projection_schemas": [
                    PROJECT_SEMANTIC_GRAPH_SCHEMA,
                    PROJECT_SEMANTIC_IMAGE_SCHEMA,
                    PROJECT_SEMANTIC_IMAGE_SYMBOL_SCHEMA,
                    PROJECT_SEMANTIC_CONTEXT_SCHEMA,
                    PROJECT_SEMANTIC_IMPACT_SCHEMA,
                ],
                "query_payload_schemas": [
                    SEMANTIC_QUERY_DECLARATIONS_SCHEMA,
                    PROJECT_SEMANTIC_IMAGE_SYMBOL_SCHEMA,
                    PROJECT_SEMANTIC_CONTEXT_SCHEMA,
                    PROJECT_SEMANTIC_IMPACT_SCHEMA,
                    SEMANTIC_QUERY_AVAILABLE_OPERATIONS_SCHEMA,
                    SEMANTIC_QUERY_OWNERSHIP_AT_EXPRESSION_SCHEMA,
                    SEMANTIC_QUERY_DECLARATION_CONSUMERS_SCHEMA,
                ],
                "read_operations": [
                    "graph", "symbol", "context", "impact", "declarations",
                    "available_operations", "ownership_at_expression",
                    "declaration_consumers", "retained_index_query", "service_history_query",
                ],
                "service_query_schemas": [
                    SEMANTIC_SERVICE_INDEX_QUERY_SCHEMA,
                    SEMANTIC_WORKSPACE_SERVICE_HISTORY_QUERY_SCHEMA,
                ],
            }),
            vec![source(
                "language-shapes-catalog",
                "markdown",
                None,
                SHAPES_CATALOG,
            )],
            vec!["no_complete_or_stable_module_graph_schema_or_node_edge_catalog"],
        ),
        InstalledSkill::Stdlib => (
            json!({
                "catalog": parse_embedded_json(STDLIB_CATALOG, "standard-library catalog")?,
                "guide": STDLIB_GUIDE,
            }),
            vec![
                source(
                    "standard-library-catalog",
                    "json",
                    Some("semaprax.standard-library-catalog.v1"),
                    STDLIB_CATALOG,
                ),
                source("standard-library-guide", "markdown", None, STDLIB_GUIDE),
            ],
            vec!["catalog_status_and_target_rows_are_descriptive_not_release_promotion"],
        ),
        InstalledSkill::Packages => (
            json!({
                "standard_library_packages": parse_embedded_json(
                    PACKAGE_CATALOG,
                    "standard-library package catalog",
                )?,
                "ordinary_package_acquisition": "explicit_caller_supplied_authenticated_subject_closure_only",
            }),
            vec![source(
                "standard-library-packages",
                "json",
                Some("semaprax.standard-library-packages.v1"),
                PACKAGE_CATALOG,
            )],
            vec![
                "no_registry_network_fetch_or_installed_ordinary_package_inventory",
                "catalog_membership_does_not_claim_project_profile_or_dependency_admission",
            ],
        ),
        InstalledSkill::Effects => (
            effects_content()?,
            vec![source(
                "standard-library-catalog",
                "json",
                Some("semaprax.standard-library-catalog.v1"),
                STDLIB_CATALOG,
            )],
            vec!["no_complete_stable_user_defined_effect_or_capability_vocabulary"],
        ),
    };
    let mut nonclaims = COMMON_NONCLAIMS.to_vec();
    nonclaims.extend(extra_nonclaims);
    document(
        INSTALLED_SKILL_SCHEMA,
        SKILL_DOMAIN,
        json!({
            "authority": false,
            "compiler": compiler()?,
            "content": content,
            "limits": {"max_document_bytes": MAX_INSTALLED_GUIDANCE_BYTES},
            "nonclaims": nonclaims,
            "skill": skill.as_str(),
            "sources": sources,
        }),
    )
}

/// Describe the exact installed Universal Semantic Query v1 operation set.
///
/// This is not session discovery: `host_grants` is deliberately empty and the
/// artifact cannot enable an operation, acquire authority, or select a target.
pub fn installed_query_capabilities() -> Result<InstalledGuidance> {
    document(
        INSTALLED_QUERY_CAPABILITIES_SCHEMA,
        CAPABILITIES_DOMAIN,
        json!({
            "authority": false,
            "compiler": compiler()?,
            "host_grants": [],
            "limits": {
                "declaration_limit_max": MAX_SEMANTIC_QUERY_DECLARATION_LIMIT,
                "declaration_offset_max": MAX_SEMANTIC_QUERY_DECLARATION_OFFSET,
                "consumer_limit_max": MAX_SEMANTIC_QUERY_CONSUMER_LIMIT,
                "consumer_offset_max": MAX_SEMANTIC_QUERY_CONSUMER_OFFSET,
                "max_query_bytes": MAX_SEMANTIC_QUERY_BYTES,
                "max_result_bytes": MAX_SEMANTIC_QUERY_RESULT_BYTES,
            },
            "nonclaims": [
                "installed_support_not_live_service_or_transport_discovery",
                "no_host_capability_grant_or_request_capability_changes",
                "available_operations_requires_an_exact_workspace_revision_and_target",
                "availability_does_not_claim_an_arbitrary_operation_payload_validates",
            ],
            "operations": [
                {"name":"declarations", "payload_schema":SEMANTIC_QUERY_DECLARATIONS_SCHEMA},
                {"name":"symbol", "payload_schema":PROJECT_SEMANTIC_IMAGE_SYMBOL_SCHEMA},
                {"name":"context", "payload_schema":PROJECT_SEMANTIC_CONTEXT_SCHEMA},
                {"name":"impact", "payload_schema":PROJECT_SEMANTIC_IMPACT_SCHEMA},
                {"name":"available_operations", "payload_schema":SEMANTIC_QUERY_AVAILABLE_OPERATIONS_SCHEMA},
                {"name":"ownership_at_expression", "payload_schema":SEMANTIC_QUERY_OWNERSHIP_AT_EXPRESSION_SCHEMA},
                {"name":"declaration_consumers", "payload_schema":SEMANTIC_QUERY_DECLARATION_CONSUMERS_SCHEMA},
            ],
            "request_schema": SEMANTIC_QUERY_SCHEMA,
            "result_schema": SEMANTIC_QUERY_RESULT_SCHEMA,
            "transaction_operations": transaction_operations(),
        }),
    )
}

fn transaction_operations() -> Value {
    json!([
        {
            "kind": "rename_display_name",
            "operation_fields": ["expected_old_value", "kind", "new_value", "target"],
        },
        {
            "kind": "replace_block",
            "operation_fields": ["expected_old_block", "kind", "replacement", "target"],
        },
        {
            "kind": "add_contract",
            "operation_fields": ["expected_old_contract", "kind", "phase", "predicate", "target"],
            "phases": ["requires", "ensures"],
        },
        {
            "kind": "add_declaration",
            "operation_fields": ["declaration", "expected_old_module", "kind", "target"],
        },
    ])
}

fn compiler() -> Result<Value> {
    Ok(json!({
        "binary_identity_claimed": false,
        "build_commit": validated_commit(option_env!("SEMAPRAX_BUILD_COMMIT"))?,
        "package": env!("CARGO_PKG_NAME"),
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

fn validated_commit(commit: Option<&str>) -> Result<Option<&str>> {
    match commit {
        None => Ok(None),
        Some(value)
            if value.len() == 40
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) =>
        {
            Ok(Some(value))
        }
        Some(_) => Err(invalid(
            "installed guidance build commit is not 40 lowercase hexadecimal characters",
        )),
    }
}

fn effects_content() -> Result<Value> {
    let catalog = parse_embedded_json(STDLIB_CATALOG, "standard-library catalog")?;
    let mut library = BTreeSet::new();
    for module in catalog["modules"]
        .as_array()
        .ok_or_else(|| invalid("standard-library catalog module inventory is invalid"))?
    {
        for declaration in module["declarations"]
            .as_array()
            .ok_or_else(|| invalid("standard-library declaration inventory is invalid"))?
        {
            for effect in declaration["effects"]
                .as_array()
                .ok_or_else(|| invalid("standard-library effect inventory is invalid"))?
            {
                library.insert(
                    effect
                        .as_str()
                        .ok_or_else(|| invalid("standard-library effect token is invalid"))?
                        .to_owned(),
                );
            }
        }
    }
    let mut compiler_owned = BTreeSet::from([
        crate::command_io_ops::ARGS_READ_EFFECT,
        crate::command_io_ops::STDERR_WRITE_EFFECT,
        crate::command_io_ops::STDIN_READ_EFFECT,
        crate::command_io_ops::STDOUT_WRITE_EFFECT,
        crate::network_io_ops::NETWORK_CONNECT_EFFECT,
        crate::network_io_ops::NETWORK_READ_EFFECT,
        crate::network_io_ops::NETWORK_WRITE_EFFECT,
    ]);
    compiler_owned.insert(crate::host_io_ops::STDOUT_WRITE_EFFECT);
    Ok(json!({
        "compiler_owned_host_operation_effects": compiler_owned,
        "standard_library_declared_effects": library,
    }))
}

fn parse_embedded_json(source: &str, name: &'static str) -> Result<Value> {
    serde_json::from_str(source).map_err(|_| invalid(name))
}

fn source(id: &str, format: &str, schema: Option<&str>, bytes: &str) -> Value {
    json!({
        "bytes": bytes.len(),
        "digest": hash(SOURCE_DOMAIN, bytes.as_bytes()),
        "format": format,
        "id": id,
        "schema": schema,
    })
}

fn document(schema: &'static str, domain: &[u8], payload: Value) -> Result<InstalledGuidance> {
    let payload_bytes = canonical(&payload)?;
    let digest = hash(domain, payload_bytes.as_bytes());
    let json = canonical(&json!({
        "digest": digest,
        "payload": payload,
        "schema": schema,
    }))?;
    if json.len() > MAX_INSTALLED_GUIDANCE_BYTES {
        return Err(capacity("installed guidance exceeds its byte limit"));
    }
    Ok(InstalledGuidance {
        schema,
        digest,
        json,
    })
}

fn canonical(value: &Value) -> Result<String> {
    fn sorted(value: &Value) -> Value {
        match value {
            Value::Array(items) => Value::Array(items.iter().map(sorted).collect()),
            Value::Object(object) => {
                let mut keys = object.keys().collect::<Vec<_>>();
                keys.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
                let mut result = Map::new();
                for key in keys {
                    result.insert(key.clone(), sorted(&object[key]));
                }
                Value::Object(result)
            }
            other => other.clone(),
        }
    }
    let mut output = serde_json::to_string(&sorted(value))
        .map_err(|_| invalid("installed guidance cannot be serialized"))?;
    output.push('\n');
    Ok(output)
}

fn hash(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G534", message)]
}

fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G535", message)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selectors_are_closed_and_every_skill_is_canonical_bounded_and_deterministic() {
        assert_eq!(
            InstalledSkill::ALL.map(InstalledSkill::as_str),
            ["agent", "language", "graph", "stdlib", "packages", "effects"]
        );
        assert_eq!(InstalledSkill::parse("agent"), Some(InstalledSkill::Agent));
        assert_eq!(InstalledSkill::parse("Agent"), None);
        assert_eq!(InstalledSkill::parse("unknown"), None);
        for skill in InstalledSkill::ALL {
            let first = installed_skill(skill).unwrap();
            let second = installed_skill(skill).unwrap();
            assert_eq!(first, second);
            assert_eq!(first.schema(), INSTALLED_SKILL_SCHEMA);
            assert!(first.to_json().ends_with('\n'));
            assert!(first.to_json().len() <= MAX_INSTALLED_GUIDANCE_BYTES);
            let value: Value = serde_json::from_str(first.to_json()).unwrap();
            assert_eq!(value["digest"], first.digest());
            assert_eq!(value["payload"]["skill"], skill.as_str());
            assert_eq!(value["payload"]["authority"], false);
            assert_eq!(
                value["payload"]["compiler"]["binary_identity_claimed"],
                false
            );
            assert_eq!(canonical(&value).unwrap(), first.to_json());
        }
    }

    #[test]
    fn embedded_source_rows_bind_the_exact_installed_bytes() {
        let language = installed_skill(InstalledSkill::Language).unwrap();
        let value: Value = serde_json::from_str(language.to_json()).unwrap();
        let sources = value["payload"]["sources"].as_array().unwrap();
        assert_eq!(sources.len(), 2);
        for (row, bytes) in sources.iter().zip([QUICK_REFERENCE, SHAPES_CATALOG]) {
            assert_eq!(row["bytes"], bytes.len());
            assert_eq!(row["digest"], hash(SOURCE_DOMAIN, bytes.as_bytes()));
        }
        let stdlib = installed_skill(InstalledSkill::Stdlib).unwrap();
        assert!(stdlib
            .to_json()
            .contains("semaprax.standard-library-catalog.v1"));
        assert!(stdlib.to_json().contains("# Standard library catalog"));
    }

    #[test]
    fn installed_query_capabilities_are_closed_and_grant_nothing() {
        let document = installed_query_capabilities().unwrap();
        assert_eq!(document.schema(), INSTALLED_QUERY_CAPABILITIES_SCHEMA);
        let value: Value = serde_json::from_str(document.to_json()).unwrap();
        let payload = &value["payload"];
        assert_eq!(payload["authority"], false);
        assert_eq!(payload["host_grants"], json!([]));
        assert_eq!(payload["request_schema"], SEMANTIC_QUERY_SCHEMA);
        assert_eq!(payload["result_schema"], SEMANTIC_QUERY_RESULT_SCHEMA);
        assert_eq!(
            payload["operations"]
                .as_array()
                .unwrap()
                .iter()
                .map(|operation| operation["name"].as_str().unwrap())
                .collect::<Vec<_>>(),
            [
                "declarations",
                "symbol",
                "context",
                "impact",
                "available_operations",
                "ownership_at_expression",
                "declaration_consumers"
            ]
        );
        assert_eq!(
            payload["transaction_operations"]
                .as_array()
                .unwrap()
                .iter()
                .map(|operation| operation["kind"].as_str().unwrap())
                .collect::<Vec<_>>(),
            [
                "rename_display_name",
                "replace_block",
                "add_contract",
                "add_declaration"
            ]
        );
        assert_eq!(canonical(&value).unwrap(), document.to_json());
    }

    #[test]
    fn commit_validation_is_exact() {
        assert_eq!(validated_commit(None).unwrap(), None);
        assert_eq!(
            validated_commit(Some("0123456789abcdef0123456789abcdef01234567")).unwrap(),
            Some("0123456789abcdef0123456789abcdef01234567")
        );
        for invalid_commit in [
            "0123456789abcdef0123456789abcdef0123456",
            "0123456789abcdef0123456789abcdef012345678",
            "0123456789abcdef0123456789abcdef0123456A",
        ] {
            assert_eq!(
                validated_commit(Some(invalid_commit)).unwrap_err()[0].code,
                "SPX-G534"
            );
        }
    }
}
