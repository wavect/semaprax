//! Explicit analysis boundaries over retained inputs, without discovery I/O.
//! A missing external declaration is never evidence that no external system
//! exists; source admission cannot establish deployment or generated provenance.
use serde_json::{json, Value};

use super::ProjectSemanticImage;
use crate::diagnostic::Diagnostic;

pub const IMAGE_ANALYSIS_COVERAGE_SCHEMA: &str = "semaprax.image-analysis-coverage.v1";
pub const MAX_IMAGE_ANALYSIS_COVERAGE_BYTES: usize = 1024 * 1024;
const MAX_FACTS: usize = 65_536;
const MAX_SOURCES: usize = 16;

impl ProjectSemanticImage {
    /// Describe what this exact retained image knows and does not inspect.
    /// This inventories admitted source provenance and declared foreign
    /// boundaries; it does not scan directories, generate artifacts, contact
    /// external services, run programs, or authenticate the current checkout.
    pub fn analysis_coverage(&self, expected_image: &str) -> Result<String, Vec<Diagnostic>> {
        self.require_digest(expected_image)?;
        let revision = self.revision();
        let modules = revision.semantic.image_modules();
        let manifest = revision.manifest();
        if modules.len() > MAX_SOURCES
            || modules.len() != revision.sources().len()
            || manifest.sources().len() != modules.len()
        {
            return Err(invalid(
                "analysis coverage source inventory disagrees with retained Project inputs",
            ));
        }
        let mut budget = Budget {
            facts: 0,
            bytes: 32_768,
        };
        let manifest_bytes = manifest.schema().len()
            + manifest.profile().map_or(0, str::len)
            + manifest.entry().len()
            + manifest.test_module().len()
            + manifest.sources().iter().map(String::len).sum::<usize>()
            + manifest
                .web_exports()
                .iter()
                .map(String::len)
                .sum::<usize>()
            + manifest
                .capabilities()
                .iter()
                .map(String::len)
                .sum::<usize>();
        budget.reserve(manifest_bytes, 1024)?;
        let mut sources = Vec::new();
        let mut external_contracts = Vec::new();
        let mut functions = 0usize;
        let mut templates = 0usize;
        let mut instances = 0usize;
        let mut types = 0usize;
        let mut interfaces = 0usize;
        for source in revision.sources() {
            let matching = modules
                .iter()
                .filter(|module| module.path() == source.path())
                .collect::<Vec<_>>();
            if matching.len() != 1 || !manifest.sources().iter().any(|path| path == source.path()) {
                return Err(invalid(
                    "analysis coverage source lacks an exact retained module join",
                ));
            }
            let module = matching[0];
            if module.source_revision() != source.source_revision()
                || module.source_digest() != source.source_digest()
                || module.source_graph_schema() != source.source_graph_schema()
            {
                return Err(invalid(
                    "analysis coverage retained source and graph bindings disagree",
                ));
            }
            budget.reserve(
                source.path().len()
                    + module.module().len()
                    + source.source_revision().len()
                    + source.source_digest().len()
                    + source.source_graph_schema().len(),
                256,
            )?;
            sources.push(json!({"path":source.path(),"module":module.module(),
                "source_revision":source.source_revision(),"source_digest":source.source_digest(),
                "source_graph_schema":source.source_graph_schema()}));
            for count in [
                module.functions().len(),
                module.function_templates().len(),
                module.function_instances().len(),
                module.types().len(),
                module.interfaces().len(),
            ] {
                budget.count(count)?;
            }
            functions += module.functions().len();
            templates += module.function_templates().len();
            instances += module.function_instances().len();
            types += module.types().len();
            interfaces += module.interfaces().len();
            for interface in module.interfaces() {
                for import in &interface.imports {
                    if import.interface != interface.id {
                        return Err(invalid("analysis coverage foreign import owner disagrees with retained interface"));
                    }
                    budget.reserve(
                        source.path().len()
                            + module.module().len()
                            + interface.id.as_str().len()
                            + import.id.as_str().len()
                            + import.name.len()
                            + import.import_key.len()
                            + import.effects.iter().map(String::len).sum::<usize>()
                            + import
                                .required_authority
                                .iter()
                                .map(String::len)
                                .sum::<usize>(),
                        512 + import
                            .effects
                            .len()
                            .saturating_add(import.required_authority.len())
                            .saturating_mul(8),
                    )?;
                    external_contracts.push(json!({"path":source.path(),"module":module.module(),
                        "interface_id":interface.id.as_str(),"import_id":import.id.as_str(),
                        "name":import.name,"import_key":import.import_key,"native_rust":import.native_rust,
                        "effects":import.effects,"required_authority":import.required_authority}));
                }
            }
        }
        // Ordering is a display-only inventory, not an execution/cleanup plan.
        sources.sort_by(|left, right| left["path"].as_str().cmp(&right["path"].as_str()));
        external_contracts.sort_by(|left, right| {
            (
                left["path"].as_str(),
                left["interface_id"].as_str(),
                left["import_id"].as_str(),
            )
                .cmp(&(
                    right["path"].as_str(),
                    right["interface_id"].as_str(),
                    right["import_id"].as_str(),
                ))
        });
        let interface_imports = external_contracts.len();
        let areas = areas(interface_imports > 0);
        super::image::render(
            json!({
                "schema":IMAGE_ANALYSIS_COVERAGE_SCHEMA,
                "image_revision":self.image_digest(),"project_revision":revision.project_revision(),
                "workspace_revision":revision.workspace_revision(),"project_graph_digest":revision.semantic_graph_digest(),
                "manifest":{"schema":manifest.schema(),"profile":manifest.profile(),"entry":manifest.entry(),
                    "test_module":manifest.test_module(),"source_paths":manifest.sources(),
                    "web_exports":manifest.web_exports(),"capabilities":manifest.capabilities()},
                "sources":sources,
                "inventory":{"source_modules":modules.len(),"functions":functions,
                    "function_templates":templates,"function_instances":instances,"nominal_types":types,
                    "interfaces":interfaces,"interface_imports":interface_imports},
                "external_contracts":external_contracts,"areas":areas,
                "source_authority":false,"external_io":false,"execution":false,
                "evidence_class":"retained_source_analysis_boundary_inventory",
                "nonclaims":["not_a_completeness_or_coverage_percentage",
                    "not_current_filesystem_or_deployment_authentication",
                    "no_absence_proof_for_undeclared_files_services_or_external_callers",
                    "declared_source_may_be_generated_without_known_producer_or_origin",
                    "no_external_implementation_contract_or_runtime_conformance_proof",
                    "no_new_host_capabilities_or_source_publication_authority"]
            }),
            false,
            MAX_IMAGE_ANALYSIS_COVERAGE_BYTES,
        )
    }
}

fn areas(has_imports: bool) -> Vec<Value> {
    vec![
        area(
            "declared_source_inputs",
            "known",
            "exact_retained_manifest_source_and_graph_bindings",
            &[
                "only_manifest_listed_sources",
                "admission_is_profile_bounded",
                "retention_does_not_authenticate_current_disk",
            ],
            &["fresh_host_source_authentication_for_current_checkout_claims"],
        ),
        area(
            "declared_external_contracts",
            if has_imports {
                "partial"
            } else {
                "not_inspected"
            },
            if has_imports {
                "retained_checked_interface_import_declarations"
            } else {
                "no_retained_interface_import_declarations"
            },
            &[
                "declarations_are_not_external_implementation_evidence",
                "native_rust_imports_are_rejected_by_current_semantic_graph_schemas",
                "zero_imports_does_not_prove_no_external_or_network_dependencies",
            ],
            &["independently_authenticated_provider_contracts_and_implementations"],
        ),
        area(
            "deployment_configuration",
            "not_inspected",
            "no_deployment_configuration_inputs_or_io",
            &[
                "environment_variables_secrets_routing_and_infrastructure_are_not_discovered",
                "manifest_capabilities_are_not_deployment_state",
            ],
            &["explicit_authenticated_deployment_inputs_with_separate_analysis_and_authority"],
        ),
        area(
            "generated_file_provenance",
            "not_inspected",
            "manifest_source_membership_does_not_record_a_generator",
            &[
                "listed_generated_spx_is_checked_as_source",
                "generator_identity_inputs_freshness_and_unlisted_outputs_are_unknown",
            ],
            &["authenticated_generator_manifest_and_output_provenance"],
        ),
        area(
            "generated_artifacts",
            "not_inspected",
            "this_query_does_not_generate_or_replay_target_artifacts",
            &[
                "existing_projection_apis_require_separate_invocation",
                "deployed_artifact_identity_and_consumers_are_unknown",
            ],
            &["source_bound_artifact_projection_and_independent_deployment_binding"],
        ),
        area(
            "external_api_behavior",
            "not_inspected",
            "no_external_service_or_native_provider_execution",
            &[
                "availability_versions_authentication_and_remote_side_effects_are_unknown",
                "source_effect_declarations_do_not_inspect_remote_systems",
            ],
            &["explicit_external_contract_version_and_conformance_evidence"],
        ),
        area(
            "runtime_environment",
            "not_inspected",
            "no_runtime_or_host_environment_observation",
            &["runtime_paths_test_coverage_liveness_and_environment_drift_are_not_measured"],
            &["authorized_execution_and_environment_evidence_bound_to_this_revision"],
        ),
        area(
            "external_consumers",
            "not_inspected",
            "retained_project_graph_has_no_external_consumer_inventory",
            &[
                "manifest_exports_do_not_enumerate_actual_clients",
                "absence_of_graph_edges_is_not_absence_of_external_callers",
            ],
            &["explicit_authenticated_consumer_inventory_and_compatibility_evidence"],
        ),
    ]
}

fn area(
    area: &str,
    status: &str,
    basis: &str,
    limitations: &[&str],
    required_evidence: &[&str],
) -> Value {
    json!({"area":area,"status":status,"basis":basis,"limitations":limitations,"required_evidence":required_evidence})
}

struct Budget {
    facts: usize,
    bytes: usize,
}
impl Budget {
    fn count(&mut self, count: usize) -> Result<(), Vec<Diagnostic>> {
        self.facts = self.facts.saturating_add(count);
        if self.facts > MAX_FACTS {
            return Err(limit("analysis coverage fact inventory exceeds its bound"));
        }
        Ok(())
    }
    fn reserve(&mut self, text_bytes: usize, fixed_bytes: usize) -> Result<(), Vec<Diagnostic>> {
        self.count(1)?;
        self.bytes = self
            .bytes
            .saturating_add(text_bytes.saturating_mul(6))
            .saturating_add(fixed_bytes);
        if self.bytes > MAX_IMAGE_ANALYSIS_COVERAGE_BYTES {
            return Err(limit(
                "analysis coverage report exceeds its conservative construction bound",
            ));
        }
        Ok(())
    }
}
fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G219", message)]
}
fn limit(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G220", message)]
}
