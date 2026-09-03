//! Caller-declared generated-source provenance for one exact candidate.
//! Canonical declarations bind retained source bytes to opaque generator
//! identities without reading paths, running generators, or materializing files.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use crate::diagnostic::Diagnostic;

use super::{wire, ProjectCandidate, PROJECT_CANDIDATE_ANALYSIS_COVERAGE_SCHEMA};

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

pub const PROJECT_CANDIDATE_GENERATED_FILE_PROVENANCE_DECLARATION_SCHEMA: &str =
    "semaprax.project-candidate-generated-file-provenance-declaration.v1";
pub const PROJECT_CANDIDATE_GENERATED_FILE_PROVENANCE_EVIDENCE_SCHEMA: &str =
    "semaprax.project-candidate-generated-file-provenance-evidence.v1";
pub const MAX_PROJECT_CANDIDATE_GENERATED_FILE_PROVENANCE_DECLARATION_BYTES: usize = 65_536;
pub const MAX_PROJECT_CANDIDATE_GENERATED_FILE_PROVENANCE_EVIDENCE_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_PROJECT_CANDIDATE_GENERATED_FILE_PROVENANCE_FILES: usize = 64;

const MAX_GENERATOR_ID_BYTES: usize = 256;
const DECLARATION_DOMAIN: &[u8] =
    b"semaprax.project-candidate-generated-file-provenance-declaration.v1\0";
const AREA_ORDER: [&str; 8] = [
    "declared_source_inputs",
    "declared_external_contracts",
    "deployment_configuration",
    "generated_file_provenance",
    "generated_artifacts",
    "external_api_behavior",
    "runtime_environment",
    "external_consumers",
];

impl ProjectCandidate {
    /// Attach one canonical caller declaration that names retained source files
    /// as generated artifacts and binds each to an opaque generator identity.
    /// The generator digest is declared evidence, not an executable locator or
    /// proof that the generator produced, can reproduce, or owns the file.
    pub fn analysis_generated_file_provenance_evidence(
        &self,
        expected_candidate: &str,
        declaration_bytes: &[u8],
        expected_declaration_digest: &str,
    ) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        let declaration =
            authenticate_declaration(self, declaration_bytes, expected_declaration_digest)?;
        let mut coverage: Value =
            serde_json::from_str(&self.analysis_coverage(expected_candidate)?)
                .map_err(|_| invalid("candidate analysis coverage is not compiler JSON"))?;
        validate_coverage(self, &coverage)?;

        let object = coverage
            .as_object_mut()
            .ok_or_else(|| invalid("candidate analysis coverage is not an object"))?;
        object.insert(
            "schema".into(),
            json!(PROJECT_CANDIDATE_GENERATED_FILE_PROVENANCE_EVIDENCE_SCHEMA),
        );
        object.insert(
            "evidence_class".into(),
            json!("retained_source_and_explicit_generated_file_provenance_declaration"),
        );

        let areas = object
            .get_mut("areas")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| invalid("candidate analysis coverage areas are absent"))?;
        if areas.len() != AREA_ORDER.len()
            || areas
                .iter()
                .zip(AREA_ORDER)
                .any(|(row, name)| row["area"].as_str() != Some(name))
        {
            return Err(invalid(
                "candidate analysis coverage area inventory is not canonical",
            ));
        }
        let provenance = &mut areas[3];
        if *provenance
            != json!({
                "area":"generated_file_provenance",
                "status":"not_inspected",
                "basis":"manifest_source_membership_does_not_record_a_generator",
                "limitations":[
                    "listed_generated_spx_is_checked_as_source",
                    "generator_identity_inputs_freshness_and_unlisted_outputs_are_unknown"
                ],
                "required_evidence":["authenticated_generator_manifest_and_output_provenance"]
            })
        {
            return Err(invalid(
                "candidate analysis coverage generated-file boundary is unexpected",
            ));
        }
        *provenance = json!({
            "area":"generated_file_provenance",
            "status":"partial",
            "basis":"caller_supplied_canonical_provenance_declaration_bound_to_exact_retained_candidate_sources",
            "limitations":[
                "generator_identity_and_digest_are_declared_not_observed_or_executed",
                "declaration_authenticates_exact_retained_output_bytes_not_generator_inputs_or_reproducibility",
                "unlisted_sources_outputs_and_generators_are_not_inspected",
                "no_filesystem_freshness_materialization_runtime_or_deployment_observation"
            ],
            "required_evidence":[
                "independently_authenticated_generator_manifest_inputs_and_execution",
                "reproducible_output_and_current_filesystem_or_deployment_binding"
            ]
        });

        let blind_spots = object
            .get_mut("blind_spots")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| invalid("candidate analysis blind-spot inventory is absent"))?;
        let matching = blind_spots
            .iter_mut()
            .filter(|row| row["domain"] == "generated_file_provenance")
            .collect::<Vec<_>>();
        if matching.len() != 1
            || *matching[0]
                != json!({
                    "domain":"generated_file_provenance",
                    "evidence_status":"absent",
                    "absent_evidence":"no_authenticated_generator_or_generated_source_provenance_evidence",
                    "source_binding":{
                        "kind":"exact_retained_project_revision_and_manifest_source_inventory",
                        "project_revision":self.revision.project_revision()
                    },
                    "nonclaim":"not_evidence_that_retained_or_unlisted_sources_are_not_generated"
                })
        {
            return Err(invalid(
                "candidate generated-file provenance blind-spot boundary is unexpected",
            ));
        }
        *matching.into_iter().next().expect("one provenance row") = json!({
            "domain":"generated_file_provenance",
            "evidence_status":"partial",
            "basis":"authenticated_caller_declaration_with_exact_candidate_source_joins",
            "source_binding":{
                "kind":"exact_candidate_revision_canonical_declaration_and_retained_source_identity",
                "candidate_revision":self.candidate_digest(),
                "project_revision":self.revision.project_revision(),
                "declaration_digest":expected_declaration_digest
            },
            "limitations":[
                "generator_identity_is_opaque_declared_data",
                "no_generator_inputs_execution_reproduction_or_freshness_observation"
            ],
            "nonclaim":"not_evidence_that_a_generator_ran_or_that_any_current_or_deployed_file_matches"
        });

        let nonclaims = object
            .get_mut("nonclaims")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| invalid("candidate analysis coverage nonclaims are absent"))?;
        for value in [
            "generated_file_declaration_not_generator_execution_or_reproducibility_proof",
            "no_filesystem_scan_materialization_freshness_or_unlisted_output_discovery",
            "no_runtime_deployment_or_external_consumer_conformance",
            "no_source_generator_artifact_publication_or_deployment_authority",
        ] {
            if nonclaims.iter().any(|row| row.as_str() == Some(value)) {
                return Err(invalid(
                    "candidate generated-file nonclaim inventory is duplicated",
                ));
            }
            nonclaims.push(json!(value));
        }
        object.insert(
            "generated_file_provenance_declaration".into(),
            json!({
                "schema":PROJECT_CANDIDATE_GENERATED_FILE_PROVENANCE_DECLARATION_SCHEMA,
                "digest":expected_declaration_digest,
                "bytes":declaration_bytes.len(),
                "canonical_json":std::str::from_utf8(declaration_bytes)
                    .map_err(|_| invalid("generated-file provenance declaration is not UTF-8"))?,
                "candidate_revision":self.candidate_digest(),
                "files":declaration["files"],
                "authentication":"exact_canonical_bytes_digest_candidate_and_retained_source_identity_joins",
                "source_authority":false,
                "filesystem_scan":false,
                "generator_execution":false,
                "artifact_materialization":false,
                "runtime_observation":false,
                "deployment_authority":false
            }),
        );
        super::super::image::render(
            coverage,
            false,
            MAX_PROJECT_CANDIDATE_GENERATED_FILE_PROVENANCE_EVIDENCE_BYTES,
        )
        .map_err(|_| {
            capacity("candidate generated-file provenance evidence exceeds its byte bound")
        })
    }
}

fn authenticate_declaration(
    candidate: &ProjectCandidate,
    bytes: &[u8],
    expected_digest: &str,
) -> Result<Value> {
    if bytes.is_empty()
        || bytes.len() > MAX_PROJECT_CANDIDATE_GENERATED_FILE_PROVENANCE_DECLARATION_BYTES
    {
        return Err(capacity(
            "generated-file provenance declaration is empty or exceeds its byte bound",
        ));
    }
    validate_digest(
        expected_digest,
        "generated-file provenance declaration digest is not canonical SHA-256",
    )?;
    if wire::digest(DECLARATION_DOMAIN, bytes) != expected_digest {
        return Err(binding(
            "generated-file provenance declaration digest disagrees",
        ));
    }
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| invalid("generated-file provenance declaration is not bounded valid JSON"))?;
    if wire::render(
        value.clone(),
        MAX_PROJECT_CANDIDATE_GENERATED_FILE_PROVENANCE_DECLARATION_BYTES,
    )?
    .as_bytes()
        != bytes
    {
        return Err(invalid(
            "generated-file provenance declaration requires exact canonical JSON bytes",
        ));
    }
    let object = value
        .as_object()
        .ok_or_else(|| invalid("generated-file provenance declaration must be an object"))?;
    require_keys(
        object,
        &["schema", "candidate_revision", "files"],
        "generated-file provenance declaration has unknown or missing fields",
    )?;
    if value["schema"] != PROJECT_CANDIDATE_GENERATED_FILE_PROVENANCE_DECLARATION_SCHEMA
        || value["candidate_revision"] != candidate.candidate_digest()
    {
        return Err(binding(
            "generated-file provenance declaration schema or candidate binding disagrees",
        ));
    }

    let files = value["files"]
        .as_array()
        .ok_or_else(|| invalid("generated-file provenance files must be an array"))?;
    if files.is_empty() || files.len() > MAX_PROJECT_CANDIDATE_GENERATED_FILE_PROVENANCE_FILES {
        return Err(capacity(
            "generated-file provenance inventory is empty or exceeds its file bound",
        ));
    }
    let mut previous = None;
    let mut generators = BTreeMap::new();
    for row in files {
        let row = row
            .as_object()
            .ok_or_else(|| invalid("generated-file provenance row must be an object"))?;
        require_keys(
            row,
            &["artifact", "source", "generator"],
            "generated-file provenance row has unknown or missing fields",
        )?;
        let artifact = child(row, "artifact", &["path", "bytes", "sha256"])?;
        let source = child(row, "source", &["path", "source_revision", "source_digest"])?;
        let generator = child(row, "generator", &["id", "digest"])?;
        let path = text(artifact, "path", "generated artifact path must be text")?;
        if path.is_empty()
            || previous.is_some_and(|prior: &str| prior.as_bytes() >= path.as_bytes())
        {
            return Err(invalid(
                "generated artifact paths must be nonempty, unique, and canonically ordered",
            ));
        }
        previous = Some(path);
        let retained = candidate
            .revision
            .sources()
            .iter()
            .filter(|candidate_source| candidate_source.path() == path)
            .collect::<Vec<_>>();
        let artifact_bytes = artifact
            .get("bytes")
            .and_then(Value::as_u64)
            .and_then(|bytes| usize::try_from(bytes).ok())
            .ok_or_else(|| invalid("generated artifact byte count must be a host-sized integer"))?;
        let artifact_digest = text(artifact, "sha256", "generated artifact digest must be text")?;
        validate_digest(
            artifact_digest,
            "generated artifact digest is not canonical SHA-256",
        )?;
        if retained.len() != 1
            || text(source, "path", "generated source path must be text")? != path
            || text(
                source,
                "source_revision",
                "generated source revision must be text",
            )? != retained[0].source_revision()
            || text(
                source,
                "source_digest",
                "generated source digest must be text",
            )? != retained[0].source_digest()
            || artifact_digest != retained[0].source_digest()
            || artifact_bytes != retained[0].source().len()
        {
            return Err(binding(
                "generated artifact and source identities lack one exact retained candidate source join",
            ));
        }
        let generator_id = text(generator, "id", "generator identity must be text")?;
        if generator_id.is_empty()
            || generator_id.len() > MAX_GENERATOR_ID_BYTES
            || !generator_id.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':' | b'@')
            })
        {
            return Err(invalid(
                "generator identity must be a bounded opaque canonical token",
            ));
        }
        let generator_digest = text(generator, "digest", "generator digest must be text")?;
        validate_digest(
            generator_digest,
            "generator digest is not canonical SHA-256",
        )?;
        if generators
            .insert(generator_id, generator_digest)
            .is_some_and(|prior| prior != generator_digest)
        {
            return Err(invalid(
                "generated-file provenance generator identity has conflicting digests",
            ));
        }
    }
    Ok(value)
}

fn validate_coverage(candidate: &ProjectCandidate, coverage: &Value) -> Result<()> {
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
    {
        return Err(binding(
            "candidate analysis coverage and generated-file declaration bindings disagree",
        ));
    }
    Ok(())
}

fn child<'a>(
    parent: &'a Map<String, Value>,
    key: &str,
    keys: &[&str],
) -> Result<&'a Map<String, Value>> {
    let object = parent
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("generated-file provenance nested identity must be an object"))?;
    require_keys(
        object,
        keys,
        "generated-file provenance nested identity has unknown or missing fields",
    )?;
    Ok(object)
}

fn require_keys(object: &Map<String, Value>, keys: &[&str], message: &'static str) -> Result<()> {
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(invalid(message));
    }
    Ok(())
}

fn text<'a>(object: &'a Map<String, Value>, key: &str, message: &'static str) -> Result<&'a str> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(message))
}

fn validate_digest(value: &str, message: &'static str) -> Result<()> {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(invalid(message));
    }
    Ok(())
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G430", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G431", message)]
}
fn binding(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G432", message)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{with_authenticated_project, ProjectCandidate};

    fn candidate() -> ProjectCandidate {
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples/calculator-project/semaprax.toml");
        with_authenticated_project(&manifest, |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
        })
        .unwrap()
    }

    fn declaration(candidate: &ProjectCandidate) -> (String, String) {
        let source = &candidate.revision.sources()[0];
        let value = json!({
            "schema":PROJECT_CANDIDATE_GENERATED_FILE_PROVENANCE_DECLARATION_SCHEMA,
            "candidate_revision":candidate.candidate_digest(),
            "files":[{
                "artifact":{"path":source.path(),"bytes":source.source().len(),"sha256":source.source_digest()},
                "source":{"path":source.path(),"source_revision":source.source_revision(),"source_digest":source.source_digest()},
                "generator":{"id":"fixture.generator:v1","digest":"sha256:1111111111111111111111111111111111111111111111111111111111111111"}
            }]
        });
        let bytes = wire::render(
            value,
            MAX_PROJECT_CANDIDATE_GENERATED_FILE_PROVENANCE_DECLARATION_BYTES,
        )
        .unwrap();
        let digest = wire::digest(DECLARATION_DOMAIN, bytes.as_bytes());
        (bytes, digest)
    }

    #[test]
    fn exact_declaration_marks_only_generated_file_provenance_partial() {
        let candidate = candidate();
        let (bytes, digest) = declaration(&candidate);
        let report: Value = serde_json::from_str(
            &candidate
                .analysis_generated_file_provenance_evidence(
                    candidate.candidate_digest(),
                    bytes.as_bytes(),
                    &digest,
                )
                .unwrap(),
        )
        .unwrap();
        let partial = report["areas"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["status"] == "partial")
            .map(|row| row["area"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(partial, ["generated_file_provenance"]);
        assert_eq!(
            report["generated_file_provenance_declaration"]["filesystem_scan"],
            false
        );
        assert_eq!(
            report["generated_file_provenance_declaration"]["generator_execution"],
            false
        );
        assert_eq!(
            report["generated_file_provenance_declaration"]["deployment_authority"],
            false
        );
    }

    #[test]
    fn declaration_rejects_unknown_fields_stale_source_and_digest() {
        let candidate = candidate();
        let (bytes, digest) = declaration(&candidate);
        let mut value: Value = serde_json::from_str(&bytes).unwrap();
        value["files"][0]["generator"]["command"] = json!("forbidden");
        let unknown = wire::render(
            value,
            MAX_PROJECT_CANDIDATE_GENERATED_FILE_PROVENANCE_DECLARATION_BYTES,
        )
        .unwrap();
        let unknown_digest = wire::digest(DECLARATION_DOMAIN, unknown.as_bytes());
        assert_eq!(
            candidate
                .analysis_generated_file_provenance_evidence(
                    candidate.candidate_digest(),
                    unknown.as_bytes(),
                    &unknown_digest,
                )
                .unwrap_err()[0]
                .code,
            "SPX-G430"
        );

        let mut value: Value = serde_json::from_str(&bytes).unwrap();
        value["files"][0]["generator"]["id"] = json!("https://generator.example/tool");
        let locator = wire::render(
            value,
            MAX_PROJECT_CANDIDATE_GENERATED_FILE_PROVENANCE_DECLARATION_BYTES,
        )
        .unwrap();
        let locator_digest = wire::digest(DECLARATION_DOMAIN, locator.as_bytes());
        assert_eq!(
            candidate
                .analysis_generated_file_provenance_evidence(
                    candidate.candidate_digest(),
                    locator.as_bytes(),
                    &locator_digest,
                )
                .unwrap_err()[0]
                .code,
            "SPX-G430"
        );

        let mut value: Value = serde_json::from_str(&bytes).unwrap();
        value["files"][0]["artifact"]["sha256"] =
            json!("sha256:2222222222222222222222222222222222222222222222222222222222222222");
        let stale = wire::render(
            value,
            MAX_PROJECT_CANDIDATE_GENERATED_FILE_PROVENANCE_DECLARATION_BYTES,
        )
        .unwrap();
        let stale_digest = wire::digest(DECLARATION_DOMAIN, stale.as_bytes());
        assert_eq!(
            candidate
                .analysis_generated_file_provenance_evidence(
                    candidate.candidate_digest(),
                    stale.as_bytes(),
                    &stale_digest,
                )
                .unwrap_err()[0]
                .code,
            "SPX-G432"
        );
        assert_eq!(
            candidate
                .analysis_generated_file_provenance_evidence(
                    candidate.candidate_digest(),
                    bytes.as_bytes(),
                    "sha256:0000000000000000000000000000000000000000000000000000000000000000",
                )
                .unwrap_err()[0]
                .code,
            "SPX-G432"
        );
        assert_eq!(digest.len(), 71);
    }
}
