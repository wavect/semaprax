//! Canonical composition of the three explicit candidate blind-spot
//! declarations. Child declarations are independently authenticated by their
//! owning attachment APIs; this layer grants no additional authority.

use std::collections::BTreeSet;

use serde_json::{json, Map, Value};

use crate::diagnostic::Diagnostic;

use super::{
    wire, ProjectCandidate, PROJECT_CANDIDATE_ANALYSIS_COVERAGE_SCHEMA,
    PROJECT_CANDIDATE_DEPLOYMENT_CONTRACT_EVIDENCE_SCHEMA,
    PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_EVIDENCE_SCHEMA,
    PROJECT_CANDIDATE_GENERATED_FILE_PROVENANCE_EVIDENCE_SCHEMA,
};

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

pub const PROJECT_CANDIDATE_ANALYSIS_BOUNDARY_BUNDLE_SCHEMA: &str =
    "semaprax.project-candidate-analysis-boundary-bundle.v1";
pub const PROJECT_CANDIDATE_ANALYSIS_BOUNDARY_BUNDLE_REPORT_SCHEMA: &str =
    "semaprax.project-candidate-analysis-boundary-bundle-report.v1";
/// The twice-escaped declaration remains below the ordinary 64 KiB protocol
/// frame with room for the request envelope and selectors.
pub const MAX_PROJECT_CANDIDATE_ANALYSIS_BOUNDARY_BUNDLE_BYTES: usize = 24 * 1024;
pub const MAX_PROJECT_CANDIDATE_ANALYSIS_BOUNDARY_BUNDLE_REPORT_BYTES: usize = 2 * 1024 * 1024;

const BUNDLE_DOMAIN: &[u8] = b"semaprax.project-candidate-analysis-boundary-bundle.v1\0";
const CHILD_KEYS: [&str; 3] = [
    "deployment_contract",
    "generated_file_provenance",
    "external_api_contract",
];
const PRESERVED_KEYS: [&str; 15] = [
    "image_revision",
    "candidate_revision",
    "base_project_revision",
    "project_revision",
    "workspace_revision",
    "project_graph_digest",
    "manifest",
    "sources",
    "inventory",
    "external_contracts",
    "source_authority",
    "external_io",
    "execution",
    "candidate_retained",
    "publication_authority",
];

struct Child<'a> {
    declaration: &'a str,
    digest: &'a str,
}

struct Merge<'a> {
    report_schema: &'static str,
    area: usize,
    blind_spot: usize,
    attachment: &'static str,
    report: &'a Value,
}

impl ProjectCandidate {
    /// Compose all three caller declarations against one exact candidate.
    /// Every child API repeats its own canonical bytes, domain digest, source
    /// and export checks before any facts are selected for this report.
    pub fn analysis_boundary_bundle(
        &self,
        expected_candidate: &str,
        bundle_bytes: &[u8],
        expected_bundle_digest: &str,
    ) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        let bundle = authenticate_bundle(self, bundle_bytes, expected_bundle_digest)?;
        let deployment = child(&bundle, "deployment_contract")?;
        let generated = child(&bundle, "generated_file_provenance")?;
        let external = child(&bundle, "external_api_contract")?;

        let mut coverage = parse_report(
            &self.analysis_coverage(expected_candidate)?,
            PROJECT_CANDIDATE_ANALYSIS_COVERAGE_SCHEMA,
        )?;
        let deployment_report = parse_report(
            &self.analysis_deployment_contract_evidence(
                expected_candidate,
                deployment.declaration.as_bytes(),
                deployment.digest,
            )?,
            PROJECT_CANDIDATE_DEPLOYMENT_CONTRACT_EVIDENCE_SCHEMA,
        )?;
        let generated_report = parse_report(
            &self.analysis_generated_file_provenance_evidence(
                expected_candidate,
                generated.declaration.as_bytes(),
                generated.digest,
            )?,
            PROJECT_CANDIDATE_GENERATED_FILE_PROVENANCE_EVIDENCE_SCHEMA,
        )?;
        let external_report = parse_report(
            &self.analysis_external_api_contract_evidence(
                expected_candidate,
                external.declaration.as_bytes(),
                external.digest,
            )?,
            PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_EVIDENCE_SCHEMA,
        )?;

        validate_base(self, &coverage)?;
        let merges = [
            Merge {
                report_schema: PROJECT_CANDIDATE_DEPLOYMENT_CONTRACT_EVIDENCE_SCHEMA,
                area: 2,
                blind_spot: 0,
                attachment: "deployment_contract_declaration",
                report: &deployment_report,
            },
            Merge {
                report_schema: PROJECT_CANDIDATE_GENERATED_FILE_PROVENANCE_EVIDENCE_SCHEMA,
                area: 3,
                blind_spot: 1,
                attachment: "generated_file_provenance_declaration",
                report: &generated_report,
            },
            Merge {
                report_schema: PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_EVIDENCE_SCHEMA,
                area: 5,
                blind_spot: 2,
                attachment: "external_api_contract_declaration",
                report: &external_report,
            },
        ];
        for merge in &merges {
            validate_child(&coverage, merge)?;
        }

        let object = coverage
            .as_object_mut()
            .ok_or_else(|| invalid("candidate analysis coverage is not an object"))?;
        object.insert(
            "schema".into(),
            json!(PROJECT_CANDIDATE_ANALYSIS_BOUNDARY_BUNDLE_REPORT_SCHEMA),
        );
        object.insert(
            "evidence_class".into(),
            json!("retained_source_and_three_explicit_analysis_boundary_declarations"),
        );
        let mut areas = object["areas"]
            .as_array()
            .expect("validated coverage areas")
            .clone();
        let mut blind_spots = object["blind_spots"]
            .as_array()
            .expect("validated coverage blind spots")
            .clone();
        let mut nonclaims = object["nonclaims"]
            .as_array()
            .expect("validated coverage nonclaims")
            .clone();
        let base_nonclaim_count = nonclaims.len();
        let mut seen = nonclaims
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        for merge in &merges {
            areas[merge.area] = merge.report["areas"][merge.area].clone();
            blind_spots[merge.blind_spot] = merge.report["blind_spots"][merge.blind_spot].clone();
            object.insert(
                merge.attachment.into(),
                merge.report[merge.attachment].clone(),
            );
            let child_nonclaims = merge.report["nonclaims"]
                .as_array()
                .expect("validated child nonclaims");
            for value in &child_nonclaims[base_nonclaim_count..] {
                let text = value
                    .as_str()
                    .ok_or_else(|| invalid("candidate attachment nonclaim is not text"))?;
                if !seen.insert(text.to_owned()) {
                    return Err(invalid(
                        "candidate analysis-boundary bundle nonclaim is duplicated",
                    ));
                }
                nonclaims.push(value.clone());
            }
        }
        object.insert("areas".into(), Value::Array(areas.clone()));
        object.insert("blind_spots".into(), Value::Array(blind_spots));
        object.insert("nonclaims".into(), Value::Array(nonclaims));
        object.insert(
            "analysis_boundary_bundle".into(),
            json!({
                "schema":PROJECT_CANDIDATE_ANALYSIS_BOUNDARY_BUNDLE_SCHEMA,
                "digest":expected_bundle_digest,
                "bytes":bundle_bytes.len(),
                "canonical_json":std::str::from_utf8(bundle_bytes)
                    .map_err(|_| invalid("candidate analysis-boundary bundle is not UTF-8"))?,
                "candidate_revision":self.candidate_digest(),
                "declaration_digests":{
                    "deployment_contract":deployment.digest,
                    "generated_file_provenance":generated.digest,
                    "external_api_contract":external.digest,
                },
                "owned_partial_areas":[
                    "deployment_configuration",
                    "generated_file_provenance",
                    "external_api_behavior",
                ],
                "authentication":"exact_canonical_bundle_and_three_independently_authenticated_child_declarations",
                "source_authority":false,
                "filesystem_scan":false,
                "generator_execution":false,
                "artifact_materialization":false,
                "network_observation":false,
                "provider_observation":false,
                "runtime_observation":false,
                "conformance_evidence":false,
                "ambient_authority":false,
                "publication_authority":false,
                "deployment_authority":false,
            }),
        );
        if areas[2]["status"] != "partial"
            || areas[3]["status"] != "partial"
            || areas[5]["status"] != "partial"
        {
            return Err(binding(
                "candidate analysis-boundary bundle did not mark every owned area partial",
            ));
        }
        // Every nested candidate report is rendered through `wire`, and the
        // readers that authenticate them compare against `wire::render`, which
        // terminates with a line feed. Rendering this one without it made the
        // bundle report fail its own exact-canonical check.
        wire::render(
            coverage,
            MAX_PROJECT_CANDIDATE_ANALYSIS_BOUNDARY_BUNDLE_REPORT_BYTES,
        )
        .map_err(|_| capacity("candidate analysis-boundary bundle report exceeds its byte bound"))
    }
}

fn authenticate_bundle(
    candidate: &ProjectCandidate,
    bytes: &[u8],
    expected_digest: &str,
) -> Result<Value> {
    if bytes.is_empty() || bytes.len() > MAX_PROJECT_CANDIDATE_ANALYSIS_BOUNDARY_BUNDLE_BYTES {
        return Err(capacity(
            "candidate analysis-boundary bundle is empty or exceeds its transport-safe bound",
        ));
    }
    validate_digest(expected_digest)?;
    if wire::digest(BUNDLE_DOMAIN, bytes) != expected_digest {
        return Err(binding(
            "candidate analysis-boundary bundle digest disagrees",
        ));
    }
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| invalid("candidate analysis-boundary bundle is not bounded valid JSON"))?;
    if wire::render(
        value.clone(),
        MAX_PROJECT_CANDIDATE_ANALYSIS_BOUNDARY_BUNDLE_BYTES,
    )?
    .as_bytes()
        != bytes
    {
        return Err(invalid(
            "candidate analysis-boundary bundle requires exact canonical JSON bytes",
        ));
    }
    let object = value
        .as_object()
        .ok_or_else(|| invalid("candidate analysis-boundary bundle must be an object"))?;
    require_keys(
        object,
        &[
            "schema",
            "candidate_revision",
            "deployment_contract",
            "generated_file_provenance",
            "external_api_contract",
        ],
        "candidate analysis-boundary bundle has unknown or missing fields",
    )?;
    if value["schema"] != PROJECT_CANDIDATE_ANALYSIS_BOUNDARY_BUNDLE_SCHEMA
        || value["candidate_revision"] != candidate.candidate_digest()
    {
        return Err(binding(
            "candidate analysis-boundary bundle schema or candidate binding disagrees",
        ));
    }
    for key in CHILD_KEYS {
        child(&value, key)?;
    }
    Ok(value)
}

fn child<'a>(bundle: &'a Value, key: &str) -> Result<Child<'a>> {
    let object = bundle[key]
        .as_object()
        .ok_or_else(|| invalid("candidate analysis-boundary child must be an object"))?;
    require_keys(
        object,
        &["declaration", "declaration_digest"],
        "candidate analysis-boundary child has unknown or missing fields",
    )?;
    let declaration = object["declaration"]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid("candidate analysis-boundary child declaration must be text"))?;
    let digest = object["declaration_digest"]
        .as_str()
        .ok_or_else(|| invalid("candidate analysis-boundary child digest must be text"))?;
    validate_digest(digest)?;
    Ok(Child {
        declaration,
        digest,
    })
}

fn parse_report(bytes: &str, schema: &str) -> Result<Value> {
    let value: Value = serde_json::from_str(bytes)
        .map_err(|_| invalid("nested candidate analysis-boundary report is not compiler JSON"))?;
    if value.as_object().is_none() || value["schema"] != schema {
        return Err(invalid(
            "nested candidate analysis-boundary report has an unexpected compiler schema",
        ));
    }
    Ok(value)
}

fn validate_base(candidate: &ProjectCandidate, coverage: &Value) -> Result<()> {
    if coverage["schema"] != PROJECT_CANDIDATE_ANALYSIS_COVERAGE_SCHEMA
        || coverage["candidate_revision"] != candidate.candidate_digest()
        || coverage["base_project_revision"] != candidate.base.project_revision()
        || coverage["project_revision"] != candidate.revision.project_revision()
        || coverage["workspace_revision"] != candidate.revision.workspace_revision()
        || coverage["project_graph_digest"] != candidate.revision.semantic_graph_digest()
        || coverage["evidence_class"] != "retained_source_analysis_boundary_inventory"
        || coverage["source_authority"] != false
        || coverage["external_io"] != false
        || coverage["execution"] != false
        || coverage["candidate_retained"] != false
        || coverage["publication_authority"] != false
        || coverage["areas"]
            .as_array()
            .is_none_or(|rows| rows.len() != 8)
        || coverage["blind_spots"]
            .as_array()
            .is_none_or(|rows| rows.len() != 3)
        || coverage["nonclaims"].as_array().is_none()
    {
        return Err(binding(
            "candidate analysis-boundary base coverage binding disagrees",
        ));
    }
    Ok(())
}

fn validate_child(base: &Value, merge: &Merge<'_>) -> Result<()> {
    if merge.report["schema"] != merge.report_schema {
        return Err(invalid(
            "candidate analysis-boundary child report schema disagrees",
        ));
    }
    for key in PRESERVED_KEYS {
        if merge.report[key] != base[key] {
            return Err(binding(
                "candidate analysis-boundary child changed a preserved coverage fact",
            ));
        }
    }
    let base_areas = base["areas"]
        .as_array()
        .ok_or_else(|| invalid("candidate analysis coverage areas are absent"))?;
    let areas = merge.report["areas"]
        .as_array()
        .ok_or_else(|| invalid("candidate attachment coverage areas are absent"))?;
    let base_blind = base["blind_spots"]
        .as_array()
        .ok_or_else(|| invalid("candidate analysis blind spots are absent"))?;
    let blind = merge.report["blind_spots"]
        .as_array()
        .ok_or_else(|| invalid("candidate attachment blind spots are absent"))?;
    if areas.len() != base_areas.len()
        || blind.len() != base_blind.len()
        || areas[merge.area]["status"] != "partial"
        || blind[merge.blind_spot]["evidence_status"] != "partial"
        || areas
            .iter()
            .zip(base_areas)
            .enumerate()
            .any(|(index, (child, parent))| index != merge.area && child != parent)
        || blind
            .iter()
            .zip(base_blind)
            .enumerate()
            .any(|(index, (child, parent))| index != merge.blind_spot && child != parent)
        || merge.report[merge.attachment].as_object().is_none()
    {
        return Err(binding(
            "candidate analysis-boundary child changed facts outside its owned boundary",
        ));
    }
    let base_nonclaims = base["nonclaims"]
        .as_array()
        .ok_or_else(|| invalid("candidate analysis coverage nonclaims are absent"))?;
    let child_nonclaims = merge.report["nonclaims"]
        .as_array()
        .ok_or_else(|| invalid("candidate attachment nonclaims are absent"))?;
    if child_nonclaims.len() <= base_nonclaims.len()
        || child_nonclaims[..base_nonclaims.len()] != base_nonclaims[..]
    {
        return Err(binding(
            "candidate analysis-boundary child did not preserve base nonclaims",
        ));
    }
    Ok(())
}

fn require_keys(object: &Map<String, Value>, keys: &[&str], message: &'static str) -> Result<()> {
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(invalid(message));
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
            "candidate analysis-boundary digest is not canonical SHA-256",
        ));
    }
    Ok(())
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G440", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G441", message)]
}
fn binding(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G442", message)]
}
