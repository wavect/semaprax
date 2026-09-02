//! Deterministic target projections for one already checked semantic patch.

use std::collections::BTreeSet;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::diagnostic::{quote_json, Diagnostic};
use crate::hir::ResolvedProgram;
use crate::{codegen, graph, patch, review, wasm};

macro_rules! format {
    ($($argument:tt)*) => {
        crate::bounded_output::budgeted_format(format_args!($($argument)*))
    };
}

const SCHEMA: &str = "semaprax.semantic-target-evidence.v1";
const CAPABILITY_SCHEMA: &str = "semaprax.capability-manifest.v1";
const GRAPH_DIGEST_DOMAIN: &[u8] = b"semaprax.semantic-target-evidence.graph-digest.v1\0";
const CAPABILITY_DIGEST_DOMAIN: &[u8] =
    b"semaprax.semantic-target-evidence.capability-manifest-digest.v1\0";
const NATIVE_DIGEST_DOMAIN: &[u8] =
    b"semaprax.semantic-target-evidence.native-c11-source-digest.v1\0";
const WASM_DIGEST_DOMAIN: &[u8] = b"semaprax.semantic-target-evidence.wasm-core-module-digest.v1\0";
const REPORT_DIGEST_DOMAIN: &[u8] = b"semaprax.semantic-target-evidence.report-digest.v1\0";
pub(crate) const MAX_GRAPH_BYTES: usize = 32 * 1024 * 1024;
pub(crate) const MAX_NATIVE_C11_BYTES: usize = 32 * 1024 * 1024;
pub(crate) const MAX_WASM_CORE_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const MAX_OUTPUT_BYTES: usize = 65_536;
const NONCLAIMS: [&str; 12] = [
    "not_native_machine_code_or_toolchain_attestation",
    "no_native_or_wasm_runtime_execution",
    "no_project_test_discovery_or_execution",
    "not_signature_or_authenticated_provenance",
    "not_human_approval_or_policy",
    "not_safe_compatible_or_abi_verified",
    "not_target_verified_or_runtime_conformant",
    "no_repository_or_multi_file_analysis",
    "no_external_consumer_compatibility",
    "no_general_capability_flow_proof",
    "no_commit_authority",
    "no_new_patch_graph_cleanup_or_runtime_semantics",
];

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct CapabilityFact<'a> {
    kind: &'static str,
    owner: &'a str,
    value: &'a str,
}

#[derive(Clone)]
pub(crate) struct TargetEvidenceFacts {
    report: String,
    digest: String,
    native_changed: bool,
    wasm_changed: bool,
    base_graph_bytes: usize,
    candidate_graph_bytes: usize,
    base_native_bytes: usize,
    candidate_native_bytes: usize,
    base_wasm_bytes: usize,
    candidate_wasm_bytes: usize,
}

impl TargetEvidenceFacts {
    pub(crate) fn report(&self) -> &str {
        &self.report
    }

    pub(crate) fn digest(&self) -> &str {
        &self.digest
    }

    pub(crate) fn target_changed(&self) -> bool {
        self.native_changed || self.wasm_changed
    }

    pub(crate) fn capability_unchanged(&self) -> bool {
        true
    }

    pub(crate) fn usage(&self) -> [usize; 6] {
        [
            self.base_graph_bytes,
            self.candidate_graph_bytes,
            self.base_native_bytes,
            self.candidate_native_bytes,
            self.base_wasm_bytes,
            self.candidate_wasm_bytes,
        ]
    }
}

pub fn preview(source_path: &Path, patch_path: &Path) -> Result<String, Vec<Diagnostic>> {
    preview_with_hook(source_path, patch_path, |_| Ok(()))
}

fn preview_with_hook(
    source_path: &Path,
    patch_path: &Path,
    mut hook: impl FnMut(&Path) -> std::io::Result<()>,
) -> Result<String, Vec<Diagnostic>> {
    let canonical_source_path = patch::canonical_source_path(source_path)?;
    let snapshot = patch::read_source_snapshot_bounded(
        &canonical_source_path,
        review::MAX_SOURCE_BYTES,
        "SPX-G140",
    )?;
    let patch_source = review::read_patch_bounded(patch_path).map_err(map_review_diagnostics)?;
    let build = review::build_target_owned(
        snapshot.source().to_owned(),
        patch_source,
        source_path.to_path_buf(),
        MAX_NATIVE_C11_BYTES,
    )
    .map_err(map_review_diagnostics)?;
    let evidence = build_from_review(&build)?;
    hook(&canonical_source_path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I208",
            format!("Semantic Target Evidence final-check hook failed: {error}"),
        )]
    })?;
    patch::validate_source_unchanged_bounded(
        &canonical_source_path,
        source_path,
        &snapshot,
        build.base_revision(),
        review::MAX_SOURCE_BYTES,
    )?;
    Ok(evidence.report)
}

pub(crate) fn build_from_review(
    build: &review::ReviewBuild,
) -> Result<TargetEvidenceFacts, Vec<Diagnostic>> {
    graph::reject_native_rust_imports(build.before_resolved()).map_err(|error| vec![error])?;
    graph::reject_native_rust_imports(build.candidate_resolved()).map_err(|error| vec![error])?;
    let base_graph = bounded_graph(build.before_resolved(), build.base_revision())?;
    let base_graph_bytes = base_graph.len();
    let base_graph_digest = domain_digest(GRAPH_DIGEST_DOMAIN, base_graph.as_bytes());
    bound_artifact("base Graph", base_graph_bytes, MAX_GRAPH_BYTES)?;
    drop(base_graph);
    let candidate_graph = bounded_graph(build.candidate_resolved(), build.candidate_revision())?;
    let candidate_graph_bytes = candidate_graph.len();
    let candidate_graph_digest = domain_digest(GRAPH_DIGEST_DOMAIN, candidate_graph.as_bytes());
    bound_artifact("candidate Graph", candidate_graph_bytes, MAX_GRAPH_BYTES)?;
    drop(candidate_graph);

    let base_capabilities = capability_manifest(build.before_resolved())?;
    let candidate_capabilities = capability_manifest(build.candidate_resolved())?;
    if base_capabilities != candidate_capabilities {
        return Err(vec![invariant_error(
            "semantic patch changes compiler-derived capability authority",
        )]);
    }
    let (capability_json, capability_overflowed) =
        crate::bounded_output::with_limit(MAX_GRAPH_BYTES, || {
            capability_manifest_json(&base_capabilities)
        });
    if capability_overflowed {
        return Err(vec![bound_error(format!(
            "Semantic Target Evidence capability manifest emission exceeds {MAX_GRAPH_BYTES} bytes"
        ))]);
    }
    let capability_digest = domain_digest(CAPABILITY_DIGEST_DOMAIN, capability_json.as_bytes());
    drop(capability_json);
    drop(base_capabilities);
    drop(candidate_capabilities);

    let base_native = bounded_native(build.preflight().before(), build.before_resolved())?;
    bound_artifact(
        "base native C11 source",
        base_native.len(),
        MAX_NATIVE_C11_BYTES,
    )?;
    let base_native_digest = domain_digest(NATIVE_DIGEST_DOMAIN, base_native.as_bytes());
    let base_native_bytes = base_native.len();
    drop(base_native);
    let candidate_native =
        bounded_native(build.preflight().candidate(), build.candidate_resolved())?;
    bound_artifact(
        "candidate native C11 source",
        candidate_native.len(),
        MAX_NATIVE_C11_BYTES,
    )?;
    let candidate_native_digest = domain_digest(NATIVE_DIGEST_DOMAIN, candidate_native.as_bytes());
    let candidate_native_bytes = candidate_native.len();
    drop(candidate_native);

    let base_wasm = bounded_wasm(build.before_resolved())?;
    let base_wasm_digest = domain_digest(WASM_DIGEST_DOMAIN, &base_wasm);
    let base_wasm_bytes = base_wasm.len();
    drop(base_wasm);
    let candidate_wasm = bounded_wasm(build.candidate_resolved())?;
    let candidate_wasm_digest = domain_digest(WASM_DIGEST_DOMAIN, &candidate_wasm);
    let candidate_wasm_bytes = candidate_wasm.len();
    drop(candidate_wasm);

    let native_changed = base_native_digest != candidate_native_digest;
    let wasm_changed = base_wasm_digest != candidate_wasm_digest;
    let usage = build.usage();
    let input = RenderInput {
        build,
        base_graph_digest: &base_graph_digest,
        candidate_graph_digest: &candidate_graph_digest,
        base_graph_bytes,
        candidate_graph_bytes,
        capability_digest: &capability_digest,
        base_native_digest: &base_native_digest,
        candidate_native_digest: &candidate_native_digest,
        base_native_bytes,
        candidate_native_bytes,
        base_wasm_digest: &base_wasm_digest,
        candidate_wasm_digest: &candidate_wasm_digest,
        base_wasm_bytes,
        candidate_wasm_bytes,
        native_changed,
        wasm_changed,
        source_bytes: usage.source_bytes(),
        patch_bytes: usage.patch_bytes(),
        operations: usage.operations(),
        declarations: usage.declarations(),
        callables: usage.callables(),
        call_sites: usage.call_sites(),
    };
    let report = render_bounded(&input)?;
    let digest = domain_digest(REPORT_DIGEST_DOMAIN, report.as_bytes());
    Ok(TargetEvidenceFacts {
        report,
        digest,
        native_changed,
        wasm_changed,
        base_graph_bytes,
        candidate_graph_bytes,
        base_native_bytes,
        candidate_native_bytes,
        base_wasm_bytes,
        candidate_wasm_bytes,
    })
}

fn capability_manifest(
    program: &ResolvedProgram,
) -> Result<BTreeSet<CapabilityFact<'_>>, Vec<Diagnostic>> {
    let mut facts = BTreeSet::new();
    for value in &program.permits {
        facts.insert(CapabilityFact {
            kind: "module_permit",
            owner: &program.module,
            value,
        });
    }
    for function in &program.functions {
        for value in &function.effects {
            facts.insert(CapabilityFact {
                kind: "function_effect",
                owner: function.id.as_str(),
                value,
            });
        }
    }
    for template in &program.function_templates {
        for value in &template.effects {
            facts.insert(CapabilityFact {
                kind: "function_template_effect",
                owner: template.id.as_str(),
                value,
            });
        }
    }
    // Execution instances carry no independent authority. Validate that each
    // materialization has exactly its persistent template's effect set before
    // intentionally keying authority by the template declaration.
    if !program.function_instances.iter().all(|instance| {
        program.function_templates.iter().any(|template| {
            template.id == instance.template && template.effects == instance.function.effects
        })
    }) {
        return Err(vec![invariant_error(
            "materialized function effects diverge from persistent template authority",
        )]);
    }
    for interface in &program.interfaces {
        for value in &interface.permits {
            facts.insert(CapabilityFact {
                kind: "interface_permit",
                owner: interface.id.as_str(),
                value,
            });
        }
        for import in &interface.imports {
            for value in &import.effects {
                facts.insert(CapabilityFact {
                    kind: "import_effect",
                    owner: import.id.as_str(),
                    value,
                });
            }
            for value in &import.required_authority {
                facts.insert(CapabilityFact {
                    kind: "import_required_authority",
                    owner: import.id.as_str(),
                    value,
                });
            }
        }
    }
    Ok(facts)
}

fn capability_manifest_json(facts: &BTreeSet<CapabilityFact<'_>>) -> String {
    let facts = crate::bounded_output::budgeted_join(
        facts.iter().map(|fact| {
            format!(
                "{{\"kind\":{},\"owner\":{},\"value\":{}}}",
                quote_json(fact.kind),
                quote_json(fact.owner),
                quote_json(fact.value)
            )
        }),
        ",",
    );
    format!("{{\"schema\":\"{CAPABILITY_SCHEMA}\",\"facts\":[{facts}]}}")
}

fn bounded_graph(program: &ResolvedProgram, revision: &str) -> Result<String, Vec<Diagnostic>> {
    bounded_graph_with_limit(program, revision, MAX_GRAPH_BYTES)
}

fn bounded_graph_with_limit(
    program: &ResolvedProgram,
    revision: &str,
    limit: usize,
) -> Result<String, Vec<Diagnostic>> {
    let (result, overflowed) =
        crate::bounded_output::with_limit(limit, || graph::to_hir_json(program, revision));
    if overflowed {
        return Err(vec![bound_error(format!(
            "Semantic Target Evidence Graph emission exceeds {MAX_GRAPH_BYTES} bytes"
        ))]);
    }
    result.map_err(|error| vec![error])
}

fn bounded_native(
    source: &crate::ast::Program,
    resolved: &ResolvedProgram,
) -> Result<String, Vec<Diagnostic>> {
    bounded_native_with_limit(source, resolved, MAX_NATIVE_C11_BYTES)
}

fn bounded_native_with_limit(
    source: &crate::ast::Program,
    resolved: &ResolvedProgram,
    limit: usize,
) -> Result<String, Vec<Diagnostic>> {
    let (result, overflowed) = crate::bounded_output::with_limit(limit, || {
        codegen::emit_resolved_c_with_source(source, resolved)
    });
    if overflowed {
        return Err(vec![bound_error(format!(
            "Semantic Target Evidence native C11 emission exceeds {MAX_NATIVE_C11_BYTES} bytes"
        ))]);
    }
    result.map_err(|error| vec![error])
}

fn bounded_wasm(program: &ResolvedProgram) -> Result<Vec<u8>, Vec<Diagnostic>> {
    bounded_wasm_with_limit(program, MAX_WASM_CORE_BYTES)
}

fn bounded_wasm_with_limit(
    program: &ResolvedProgram,
    limit: usize,
) -> Result<Vec<u8>, Vec<Diagnostic>> {
    let (result, overflowed) =
        crate::bounded_output::with_limit(limit, || emit_validated_wasm(program));
    if overflowed {
        return Err(vec![bound_error(format!(
            "Semantic Target Evidence Wasm emission exceeds {MAX_WASM_CORE_BYTES} bytes"
        ))]);
    }
    result
}

fn emit_validated_wasm(program: &ResolvedProgram) -> Result<Vec<u8>, Vec<Diagnostic>> {
    let bytes = wasm::emit_resolved_module(program).map_err(|error| vec![error])?;
    bound_artifact("Wasm core module", bytes.len(), MAX_WASM_CORE_BYTES)?;
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(&bytes)
        .map_err(|error| {
            vec![invariant_error(format!(
                "compiler-emitted Wasm core module failed structural validation: {error}"
            ))]
        })?;
    Ok(bytes)
}

struct RenderInput<'a> {
    build: &'a review::ReviewBuild,
    base_graph_digest: &'a str,
    candidate_graph_digest: &'a str,
    base_graph_bytes: usize,
    candidate_graph_bytes: usize,
    capability_digest: &'a str,
    base_native_digest: &'a str,
    candidate_native_digest: &'a str,
    base_native_bytes: usize,
    candidate_native_bytes: usize,
    base_wasm_digest: &'a str,
    candidate_wasm_digest: &'a str,
    base_wasm_bytes: usize,
    candidate_wasm_bytes: usize,
    native_changed: bool,
    wasm_changed: bool,
    source_bytes: usize,
    patch_bytes: usize,
    operations: usize,
    declarations: usize,
    callables: usize,
    call_sites: usize,
}

fn render_bounded(input: &RenderInput<'_>) -> Result<String, Vec<Diagnostic>> {
    let mut used_output_bytes = 0;
    for _ in 0..4 {
        let (output, overflowed) = crate::bounded_output::with_limit(MAX_OUTPUT_BYTES, || {
            render(input, used_output_bytes)
        });
        if overflowed {
            return Err(vec![bound_error(format!(
                "Semantic Target Evidence exceeds {MAX_OUTPUT_BYTES} bytes"
            ))]);
        }
        if output.len() == used_output_bytes {
            if output.len() > MAX_OUTPUT_BYTES {
                return Err(vec![bound_error(format!(
                    "Semantic Target Evidence exceeds {MAX_OUTPUT_BYTES} bytes"
                ))]);
            }
            return Ok(output);
        }
        used_output_bytes = output.len();
    }
    Err(vec![invariant_error(
        "Semantic Target Evidence byte accounting did not converge",
    )])
}

fn render(input: &RenderInput<'_>, used_output_bytes: usize) -> String {
    let classification = |changed| if changed { "changed" } else { "unchanged" };
    format!(
        "{{\"schema\":\"{SCHEMA}\",\"source_graph_schema\":{},\"base_revision\":{},\"candidate_revision\":{},\"source\":{{\"digest\":{}}},\"patch\":{{\"schema\":{},\"digest\":{}}},\"graphs\":{{\"classification\":{},\"base\":{{\"digest\":{},\"bytes\":{}}},\"candidate\":{{\"digest\":{},\"bytes\":{}}}}},\"capabilities\":{{\"schema\":\"{CAPABILITY_SCHEMA}\",\"base_digest\":{},\"candidate_digest\":{},\"classification\":\"unchanged\",\"added\":[],\"removed\":[]}},\"targets\":[{{\"kind\":\"native_c11_source\",\"profile\":\"semaprax.native-c11.bootstrap.v1\",\"base_digest\":{},\"candidate_digest\":{},\"base_bytes\":{},\"candidate_bytes\":{},\"classification\":{}}},{{\"kind\":\"wasm_core_module\",\"profile\":\"semaprax.wasm-core.v1\",\"validation\":\"wasmparser_structural\",\"validator_version\":\"0.258.0\",\"validator_features\":\"all\",\"base_digest\":{},\"candidate_digest\":{},\"base_bytes\":{},\"candidate_bytes\":{},\"classification\":{}}}],\"limits\":{{\"max_source_bytes\":{},\"max_patch_bytes\":{},\"max_operations\":{},\"max_declarations\":{},\"max_callables\":{},\"max_call_sites\":{},\"max_graph_bytes\":{MAX_GRAPH_BYTES},\"max_native_c11_bytes\":{MAX_NATIVE_C11_BYTES},\"max_wasm_core_bytes\":{MAX_WASM_CORE_BYTES},\"max_output_bytes\":{MAX_OUTPUT_BYTES}}},\"budget\":{{\"used_source_bytes\":{},\"used_patch_bytes\":{},\"used_operations\":{},\"used_declarations\":{},\"used_callables\":{},\"used_call_sites\":{},\"used_base_graph_bytes\":{},\"used_candidate_graph_bytes\":{},\"used_base_native_c11_bytes\":{},\"used_candidate_native_c11_bytes\":{},\"used_base_wasm_core_bytes\":{},\"used_candidate_wasm_core_bytes\":{},\"used_output_bytes\":{used_output_bytes}}},\"nonclaims\":{}}}",
        quote_json(input.build.source_graph_schema()),
        quote_json(input.build.base_revision()),
        quote_json(input.build.candidate_revision()),
        quote_json(input.build.source_digest()),
        quote_json(input.build.patch_schema()),
        quote_json(input.build.patch_digest()),
        quote_json(classification(input.base_graph_digest != input.candidate_graph_digest)),
        quote_json(input.base_graph_digest), input.base_graph_bytes,
        quote_json(input.candidate_graph_digest), input.candidate_graph_bytes,
        quote_json(input.capability_digest), quote_json(input.capability_digest),
        quote_json(input.base_native_digest), quote_json(input.candidate_native_digest),
        input.base_native_bytes, input.candidate_native_bytes,
        quote_json(classification(input.native_changed)),
        quote_json(input.base_wasm_digest), quote_json(input.candidate_wasm_digest),
        input.base_wasm_bytes, input.candidate_wasm_bytes,
        quote_json(classification(input.wasm_changed)),
        review::MAX_SOURCE_BYTES, review::MAX_PATCH_BYTES, review::MAX_OPERATIONS,
        review::MAX_DECLARATIONS, review::MAX_CALLABLES, review::MAX_CALL_SITES,
        input.source_bytes, input.patch_bytes, input.operations, input.declarations,
        input.callables, input.call_sites, input.base_graph_bytes,
        input.candidate_graph_bytes, input.base_native_bytes, input.candidate_native_bytes,
        input.base_wasm_bytes, input.candidate_wasm_bytes,
        nonclaims_json(),
    )
}

fn nonclaims_json() -> String {
    format!(
        "[{}]",
        crate::bounded_output::budgeted_join(NONCLAIMS.iter().map(|value| quote_json(value)), ",",)
    )
}

fn bound_artifact(label: &str, actual: usize, limit: usize) -> Result<(), Vec<Diagnostic>> {
    if actual > limit {
        return Err(vec![bound_error(format!(
            "Semantic Target Evidence {label} exceeds {limit} bytes"
        ))]);
    }
    Ok(())
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

fn map_review_diagnostics(mut diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    for diagnostic in &mut diagnostics {
        diagnostic.code = match diagnostic.code {
            "SPX-G120" => "SPX-G140",
            "SPX-G121" => "SPX-G141",
            code => code,
        };
    }
    diagnostics
}

fn bound_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-G140", message)
}

fn invariant_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-G141", message)
}

#[cfg(test)]
#[path = "target_evidence/tests.rs"]
mod tests;
