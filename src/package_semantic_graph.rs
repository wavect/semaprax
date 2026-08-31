//! Read-only coordinate-qualified package facts, derived only through exact
//! Offline Multi-Package Source Capsule replay. Serialized facts carry no HIR
//! or Project association and are never accepted as compilation authority.
use std::collections::BTreeMap;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;
use crate::package_lock_v2::Coordinate;
use crate::package_resolver::{ResolutionInput, ResolutionOptions};
use crate::package_source_capsule::{self, PackageSource, SourceCapsuleOptions};

pub const PACKAGE_SEMANTIC_GRAPH_SCHEMA: &str = "semaprax.package-semantic-graph.v1";
pub const PACKAGE_SEMANTIC_SUMMARY_SCHEMA: &str = "semaprax.package-semantic-summary.v1";
pub const PACKAGE_SEMANTIC_CONSUMERS_SCHEMA: &str = "semaprax.package-semantic-consumers.v1";
pub const MAX_PACKAGE_SEMANTIC_GRAPH_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_PACKAGE_SEMANTIC_REPORT_BYTES: usize = 1024 * 1024;
const MAX_CALLS: usize = 65_536;
const MAX_INTERFACE_FUNCTIONS: usize = 4096;

/// Immutable descriptive graph. Construction independently authenticates the
/// caller-supplied source, reports, resolution and exact capsule bytes. No raw
/// path, network, source publication, or executable HIR accessor is provided.
pub struct PackageSemanticGraph {
    digest: String,
    json: String,
    facts: Value,
    exports: BTreeMap<Coordinate, Vec<String>>,
}

impl PackageSemanticGraph {
    pub fn derive(
        capsule: &str,
        sources: &[PackageSource],
        resolution_evidence: &str,
        resolution_input: &ResolutionInput,
        resolution_options: &ResolutionOptions,
        options: &SourceCapsuleOptions,
    ) -> Result<Self, Vec<Diagnostic>> {
        let verified = package_source_capsule::verify_for_semantic_graph(
            capsule,
            sources,
            resolution_evidence,
            resolution_input,
            resolution_options,
            options,
        )
        .map_err(|error| vec![error])?;
        if verified.package_facts.len() > package_source_capsule::MAX_PACKAGES
            || verified.import_facts.len() > package_source_capsule::MAX_IMPORTS
            || verified.call_facts.len() > MAX_CALLS
        {
            return Err(limit("package graph retained inventory exceeds its bound"));
        }
        let mut exports = BTreeMap::new();
        let mut function_count = 0usize;
        for subject in &verified.selected_subjects {
            function_count = function_count.saturating_add(subject.interface.functions.len());
            if function_count > MAX_INTERFACE_FUNCTIONS {
                return Err(limit(
                    "package graph interface function inventory exceeds its bound",
                ));
            }
            let ids = subject
                .interface
                .functions
                .iter()
                .map(|function| function.stable_id.clone())
                .collect::<Vec<_>>();
            if exports.insert(subject.coordinate.clone(), ids).is_some() {
                return Err(binding("package graph selected coordinate is duplicated"));
            }
        }
        let source_facts = verified
            .package_facts
            .iter()
            .map(|fact| (&fact.coordinate, fact))
            .collect::<BTreeMap<_, _>>();
        if source_facts.len() != verified.package_facts.len() || source_facts.len() != exports.len()
        {
            return Err(binding(
                "package graph source and selected interface inventories disagree",
            ));
        }
        let root = source_facts
            .keys()
            .find(|coordinate| coordinate.package == verified.receipt.root_package())
            .ok_or_else(|| binding("package graph root coordinate is absent"))?;
        let mut packages = Vec::new();
        let mut budget = ConstructionBudget { bytes: 16_384 };
        for (coordinate, fact) in &source_facts {
            let selected_exports = exports
                .get(*coordinate)
                .ok_or_else(|| binding("package graph source has no selected interface"))?;
            budget.charge(
                coordinate.package.len()
                    + coordinate.version.len()
                    + fact.subject_digest.len()
                    + fact.report_digest.len()
                    + fact.interface_digest.len()
                    + fact.interface_source_revision.len()
                    + fact.source_revision.len()
                    + fact.source_digest.len()
                    + selected_exports.iter().map(String::len).sum::<usize>(),
                768 + selected_exports.len().saturating_mul(8),
            )?;
            packages.push(json!({"coordinate":coordinate_value(coordinate),
                "subject_digest":fact.subject_digest,"report_digest":fact.report_digest,
                "interface_digest":fact.interface_digest,"interface_source_revision":fact.interface_source_revision,
                "source_revision":fact.source_revision,"source_digest":fact.source_digest,
                "source_bytes":fact.source_bytes,"exports":selected_exports}));
        }
        let mut imports = Vec::new();
        for import in &verified.import_facts {
            if !source_facts.contains_key(&import.dependent)
                || !exports
                    .get(&import.dependency)
                    .is_some_and(|ids| ids.contains(&import.target))
            {
                return Err(binding(
                    "package graph import has no exact selected source/interface association",
                ));
            }
            budget.charge(
                import.dependent.package.len()
                    + import.dependent.version.len()
                    + import.dependency.package.len()
                    + import.dependency.version.len()
                    + import.target.len()
                    + import.alias.len(),
                384,
            )?;
            imports.push(json!({"dependent":coordinate_value(&import.dependent),"dependency":coordinate_value(&import.dependency),
                "target":import.target,"alias":import.alias,"ordinal":import.ordinal}));
        }
        let mut calls = Vec::new();
        for call in &verified.call_facts {
            let caller = source_facts
                .get(&call.caller_package)
                .ok_or_else(|| binding("package graph call caller source is absent"))?;
            let target = source_facts
                .get(&call.target_package)
                .ok_or_else(|| binding("package graph call target source is absent"))?;
            if !verified.import_facts.iter().any(|import| {
                import.dependent == call.caller_package
                    && import.dependency == call.target_package
                    && import.target == call.target
                    && import.alias == call.alias
            }) {
                return Err(binding(
                    "package graph actual call lacks its authenticated direct import",
                ));
            }
            budget.charge(
                call.caller_package.package.len()
                    + call.caller_package.version.len()
                    + call.target_package.package.len()
                    + call.target_package.version.len()
                    + call.caller.len()
                    + call.target.len()
                    + caller.source_revision.len()
                    + target.source_revision.len()
                    + call.site.len()
                    + call.expression.len()
                    + call.ast_path.len()
                    + call.alias.len(),
                640,
            )?;
            calls.push(json!({"caller_package":coordinate_value(&call.caller_package),"target_package":coordinate_value(&call.target_package),
                "caller":call.caller,"target":call.target,"caller_source_revision":caller.source_revision,
                "target_source_revision":target.source_revision,"site":call.site,"expression":call.expression,
                "ast_path":call.ast_path,"alias":call.alias,"ordinal":call.ordinal}));
        }
        let facts = json!({"schema":PACKAGE_SEMANTIC_GRAPH_SCHEMA,
            "source_capsule_digest":verified.receipt.digest(),"source_set_digest":verified.receipt.source_set_digest(),
            "link_digest":verified.receipt.link_digest(),"root_package":coordinate_value(root),
            "packages":packages,"imports":imports,"calls":calls,
            "counts":{"packages":source_facts.len(),"interface_functions":function_count,
                "imports":verified.import_facts.len(),"cross_package_calls":verified.call_facts.len()},
            "project_association":"none","evidence_owner":"verified_package_source_capsule_and_workspace_calls",
            "source_authority":false,"execution":false,"publication_authority":false,"nonclaims":nonclaims()});
        let json = render(facts.clone(), true, MAX_PACKAGE_SEMANTIC_GRAPH_BYTES)?;
        let digest = digest(
            b"semaprax.package-semantic-graph.digest.v1\0",
            json.as_bytes(),
        );
        Ok(Self {
            digest,
            json,
            facts,
            exports,
        })
    }

    pub fn graph_digest(&self) -> &str {
        &self.digest
    }
    /// Canonical compact graph JSON with one terminal LF. Digest framing is
    /// domain, little-endian u64 byte length, then these exact bytes.
    pub fn to_json(&self) -> &str {
        &self.json
    }

    /// Compact source/interface/import inventory; actual calls are counted here
    /// and selected explicitly through consumers. No ambient Project is joined.
    pub fn summary(&self, expected_graph: &str) -> Result<String, Vec<Diagnostic>> {
        self.require(expected_graph)?;
        let mut value = self
            .facts
            .as_object()
            .ok_or_else(|| binding("package graph retained facts are invalid"))?
            .iter()
            .filter(|(key, _)| key.as_str() != "calls")
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<serde_json::Map<_, _>>();
        value.insert("schema".to_owned(), json!(PACKAGE_SEMANTIC_SUMMARY_SCHEMA));
        value.insert("graph_revision".to_owned(), json!(self.digest));
        render(
            Value::Object(value),
            false,
            MAX_PACKAGE_SEMANTIC_REPORT_BYTES,
        )
    }

    /// Select a verified provider coordinate and exported stable ID. Import
    /// declarations are distinct from actual source call sites; an unused
    /// import has no fabricated caller. Same-package and external dynamic calls
    /// are outside this cross-package source-edge inventory.
    pub fn consumers(
        &self,
        expected_graph: &str,
        provider: &Coordinate,
        target: &str,
    ) -> Result<String, Vec<Diagnostic>> {
        self.require(expected_graph)?;
        if provider.package.len() > 255 || provider.version.len() > 128 || target.len() > 4096 {
            return Err(limit("package consumers selector exceeds its byte bound"));
        }
        if provider.package.is_empty()
            || provider.version.is_empty()
            || target.is_empty()
            || provider.package.contains('\0')
            || provider.version.contains('\0')
            || target.contains('\0')
        {
            return Err(grammar(
                "package consumers requires nonempty coordinate and target selectors",
            ));
        }
        if !self
            .exports
            .get(provider)
            .is_some_and(|ids| ids.iter().any(|id| id == target))
        {
            return Err(binding(
                "package consumers coordinate or interface export is not selected",
            ));
        }
        let coordinate = coordinate_value(provider);
        let source = self.facts["packages"]
            .as_array()
            .and_then(|rows| rows.iter().find(|row| row["coordinate"] == coordinate))
            .ok_or_else(|| binding("package consumers provider source is absent"))?;
        let imports = self.facts["imports"]
            .as_array()
            .ok_or_else(|| binding("package import facts are absent"))?
            .iter()
            .filter(|row| row["dependency"] == coordinate && row["target"] == target)
            .cloned()
            .collect::<Vec<_>>();
        let calls = self.facts["calls"]
            .as_array()
            .ok_or_else(|| binding("package call facts are absent"))?
            .iter()
            .filter(|row| row["target_package"] == coordinate && row["target"] == target)
            .cloned()
            .collect::<Vec<_>>();
        render(
            json!({"schema":PACKAGE_SEMANTIC_CONSUMERS_SCHEMA,"graph_revision":self.digest,
            "source_capsule_digest":self.facts["source_capsule_digest"],"provider":coordinate,"target":target,
            "provider_source_revision":source["source_revision"],"provider_source_digest":source["source_digest"],
            "imports":imports,"calls":calls,"project_association":"none","source_authority":false,
            "execution":false,"publication_authority":false,"nonclaims":nonclaims()}),
            false,
            MAX_PACKAGE_SEMANTIC_REPORT_BYTES,
        )
    }

    fn require(&self, expected: &str) -> Result<(), Vec<Diagnostic>> {
        if expected.len() > 71 {
            return Err(limit("package graph selector exceeds its byte bound"));
        }
        if expected.len() != 71
            || !expected.starts_with("sha256:")
            || !expected.as_bytes()[7..]
                .iter()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
        {
            return Err(grammar("package graph selector is not a canonical digest"));
        }
        if expected != self.digest {
            return Err(binding("package graph selector is stale"));
        }
        Ok(())
    }
}

fn coordinate_value(coordinate: &Coordinate) -> Value {
    json!({"package":coordinate.package,"version":coordinate.version})
}
fn nonclaims() -> Value {
    json!(["no_project_or_live_workspace_association","no_untrusted_graph_or_hir_admission",
    "imports_do_not_prove_calls","calls_are_all_authenticated_cross_package_source_sites_not_runtime_execution_or_linked_closure_coverage",
    "no_same_package_dynamic_or_external_consumer_completeness","no_dependency_resolution_network_or_filesystem_authority",
    "no_installed_package_artifact_or_deployment_conformance"])
}
fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}
struct ConstructionBudget {
    bytes: usize,
}
impl ConstructionBudget {
    fn charge(&mut self, text: usize, fixed: usize) -> Result<(), Vec<Diagnostic>> {
        self.bytes = self
            .bytes
            .saturating_add(text.saturating_mul(6))
            .saturating_add(fixed);
        if self.bytes > MAX_PACKAGE_SEMANTIC_GRAPH_BYTES {
            return Err(limit(
                "package graph exceeds its conservative construction bound",
            ));
        }
        Ok(())
    }
}
fn render(mut value: Value, lf: bool, max: usize) -> Result<String, Vec<Diagnostic>> {
    use std::io::Write;
    struct Writer {
        bytes: Vec<u8>,
        max: usize,
    }
    impl Write for Writer {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if bytes.len() > self.max.saturating_sub(self.bytes.len()) {
                return Err(std::io::Error::other("package semantic report bound"));
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    value.sort_all_objects();
    let mut writer = Writer {
        bytes: Vec::new(),
        max,
    };
    serde_json::to_writer(&mut writer, &value)
        .map_err(|_| limit("package semantic report exceeds its byte bound"))?;
    if lf {
        writer
            .write_all(b"\n")
            .map_err(|_| limit("package semantic graph exceeds its byte bound"))?;
    }
    String::from_utf8(writer.bytes).map_err(|_| grammar("package semantic report is not UTF-8"))
}
fn grammar(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-PS601", message)]
}
fn binding(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-PS602", message)]
}
fn limit(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-PS603", message)]
}
