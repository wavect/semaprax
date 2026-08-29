//! Deterministic, offline, read-only Package Lock v1.
//!
//! This module consumes only an explicit finite set of already-owned package
//! subject envelopes. It does not accept paths, discover packages, resolve
//! versions, fetch data, run scripts, compile targets, publish a lockfile, or
//! carry authority. Every output fact is independently rebuilt from exact
//! subject bytes and an independently verified Interface Package Report v1.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use sha2::{Digest as _, Sha256};

use crate::diagnostic::{quote_json, Diagnostic};
use crate::package_report;

pub const SCHEMA: &str = "semaprax.offline-package-lock.v1";
pub const SUBJECT_SCHEMA: &str = "semaprax.offline-package-subject.v1";

const SUBJECT_DIGEST_DOMAIN: &[u8] = b"semaprax.offline-package-subject.payload.v1\0";
const LOCK_DIGEST_DOMAIN: &[u8] = b"semaprax.offline-package-lock.payload.v1\0";
const REPORT_ENVELOPE_DIGEST_DOMAIN: &[u8] = b"semaprax.offline-package-lock.report-envelope.v1\0";

pub const MAX_PACKAGES: usize = 64;
pub const MAX_SUBJECT_BYTES: usize = 262_144;
pub const MAX_TOTAL_SUBJECT_BYTES: usize = 4_194_304;
pub const MAX_DEPENDENCIES_PER_PACKAGE: usize = 64;
pub const MAX_EDGES: usize = 256;
pub const MAX_DEPENDENCY_DEPTH: usize = 32;
pub const MAX_CAPABILITIES: usize = 256;
pub const MAX_LICENSES: usize = 64;
pub const MAX_PROVENANCE: usize = 64;
pub const MAX_JSON_DEPTH: usize = 8;
pub const MAX_WORK_UNITS: usize = 16_384;
pub const MAX_BUILDER_BYTES: usize = 16_777_216;
pub const MAX_OUTPUT_BYTES: usize = 8_388_608;
const MIN_OUTPUT_BYTES: usize = 4_096;

const TARGET_KEYS: [&str; 2] = ["available", "target"];
const CAPABILITY_DOMAINS: [&str; 5] = ["filesystem", "home", "network", "process", "secrets"];
const PROVENANCE_KINDS: [&str; 4] = ["repository", "revision", "source", "vendor"];
const PACKAGE_REPORT_NONCLAIMS: [&str; 9] = [
    "report_descriptor_only",
    "no_resolver",
    "no_lockfile_or_dependency_model",
    "no_package_registry_or_hosting",
    "no_version_compatibility_engine",
    "no_conformance_tests",
    "no_provenance_signatures_licenses_or_sbom",
    "no_target_execution",
    "read_only_no_source_changes",
];

const NONCLAIMS_JSON: &str = "\"offline_read_only_lock_evidence\",\
\"not_resolver_version_negotiation_or_compatibility_engine\",\
\"no_registry_network_fetch_or_filesystem_discovery\",\
\"no_dependency_source_archive_or_artifact_acquisition\",\
\"no_build_script_compilation_linking_or_target_execution\",\
\"no_sandbox_or_capability_enforcement\",\
\"capabilities_are_integrity_bound_declared_facts_only\",\
\"licenses_and_provenance_are_optional_integrity_bound_claims_only\",\
\"not_signature_trusted_provenance_sbom_approval_or_policy\",\
\"no_source_mutation_lockfile_publication_or_commit_authority\",\
\"no_path_facts_raw_tree_git_editor_or_workspace_authority\",\
\"no_reusable_authorization_token\",\
\"no_incremental_cache_persistence_recovery_cleanup_or_gc\",\
\"no_external_consumer_compatibility_or_conformance_claim\",\
\"no_new_language_graph_cleanup_backend_or_runtime_semantics\"";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackageLockOptions {
    max_bytes: usize,
}

impl PackageLockOptions {
    pub fn new(max_bytes: usize) -> Result<Self, Diagnostic> {
        if !(MIN_OUTPUT_BYTES..=MAX_OUTPUT_BYTES).contains(&max_bytes) {
            return Err(option_error(format!(
                "package-lock max_bytes must be between {MIN_OUTPUT_BYTES} and {MAX_OUTPUT_BYTES}"
            )));
        }
        Ok(Self { max_bytes })
    }

    pub fn max_bytes(self) -> usize {
        self.max_bytes
    }
}

impl Default for PackageLockOptions {
    fn default() -> Self {
        Self {
            max_bytes: MAX_OUTPUT_BYTES,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Coordinate {
    package: String,
    version: String,
}

impl Coordinate {
    fn render(&self) -> String {
        format!(
            "{{\"package\":{},\"version\":{}}}",
            quote_json(&self.package),
            quote_json(&self.version)
        )
    }

    fn label(&self) -> String {
        format!("{}@{}", self.package, self.version)
    }
}

#[derive(Clone, Debug)]
struct TargetFact {
    target: String,
    available: bool,
}

impl TargetFact {
    fn render(&self) -> String {
        format!(
            "{{\"target\":{},\"available\":{}}}",
            quote_json(&self.target),
            self.available
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct ProvenanceFact {
    kind: String,
    value: String,
}

impl ProvenanceFact {
    fn render(&self) -> String {
        format!(
            "{{\"kind\":{},\"value\":{}}}",
            quote_json(&self.kind),
            quote_json(&self.value)
        )
    }
}

#[derive(Clone, Debug)]
struct PackageSubject {
    coordinate: Coordinate,
    subject_digest: String,
    subject_bytes: usize,
    report_digest: String,
    report_bytes: usize,
    report_envelope_digest: String,
    targets: Vec<TargetFact>,
    dependencies: Vec<Coordinate>,
    capabilities: Vec<String>,
    licenses: Vec<String>,
    provenance: Vec<ProvenanceFact>,
}

#[derive(Clone, Copy, Debug, Default)]
struct Budget {
    packages: usize,
    total_subject_bytes: usize,
    edges: usize,
    dependency_depth: usize,
    capability_facts: usize,
    capability_closure_facts: usize,
    license_facts: usize,
    provenance_facts: usize,
    work_units: usize,
    builder_bytes: usize,
    output_bytes: usize,
}

/// Independently replayed package coordinates in dependency-first order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedPackageLock {
    packages: Vec<String>,
}

impl VerifiedPackageLock {
    pub fn packages(&self) -> &[String] {
        &self.packages
    }
}

/// Generate one canonical offline lock from exact owned subject envelopes.
pub fn generate(
    subjects: &[String],
    options: &PackageLockOptions,
) -> Result<String, Vec<Diagnostic>> {
    build(subjects, options).map_err(|error| vec![error])
}

/// Independently rebuild and exact-byte compare one submitted lock.
pub fn verify(
    lock: &str,
    subjects: &[String],
    options: &PackageLockOptions,
) -> Result<VerifiedPackageLock, Diagnostic> {
    if lock.len() > options.max_bytes || lock.len() > MAX_OUTPUT_BYTES {
        return Err(limit_error(format!(
            "output_bytes exceeds {}",
            options.max_bytes.min(MAX_OUTPUT_BYTES)
        )));
    }
    validate_json_wire(lock, "lock")?;
    let (schema, _, _, _) = parse_wrapper(lock, LOCK_DIGEST_DOMAIN, "lock")?;
    if schema != SCHEMA {
        return Err(grammar_error(format!("lock schema must be {SCHEMA}")));
    }
    let rebuilt = build(subjects, options)?;
    if rebuilt != lock {
        return Err(replay_error(
            "submitted lock does not exactly replay the supplied subjects".to_owned(),
        ));
    }
    let value: Value = serde_json::from_str(lock)
        .map_err(|error| grammar_error(format!("lock is not valid JSON: {error}")))?;
    let packages = value["payload"]["packages"]
        .as_array()
        .ok_or_else(|| grammar_error("lock packages must be an array".to_owned()))?
        .iter()
        .map(|row| {
            let package = required_str(row, "package", "lock package")?;
            let version = required_str(row, "version", "lock package")?;
            Ok(format!("{package}@{version}"))
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    Ok(VerifiedPackageLock { packages })
}

fn build(subjects: &[String], options: &PackageLockOptions) -> Result<String, Diagnostic> {
    validate_package_count(subjects.len())?;

    let mut budget = Budget {
        packages: subjects.len(),
        ..Budget::default()
    };
    let mut packages = BTreeMap::<Coordinate, PackageSubject>::new();
    let mut identities = BTreeMap::<String, String>::new();
    for subject in subjects {
        checked_add(
            &mut budget.total_subject_bytes,
            subject.len(),
            MAX_TOTAL_SUBJECT_BYTES,
            "total_subject_bytes",
        )?;
        ensure_at_most(subject.len(), MAX_SUBJECT_BYTES, "subject_bytes")?;
        debit_work(&mut budget, 1)?;
        let parsed = parse_subject(subject, &mut budget)?;
        if let Some(previous) = identities.insert(
            parsed.coordinate.package.clone(),
            parsed.coordinate.version.clone(),
        ) {
            return Err(confusion_error(format!(
                "package identity `{}` is submitted more than once (versions `{previous}` and `{}`)",
                parsed.coordinate.package, parsed.coordinate.version
            )));
        }
        if packages.insert(parsed.coordinate.clone(), parsed).is_some() {
            return Err(confusion_error(
                "duplicate package coordinate is not allowed".to_owned(),
            ));
        }
    }

    for subject in packages.values() {
        for dependency in &subject.dependencies {
            debit_work(&mut budget, 1)?;
            if dependency == &subject.coordinate {
                return Err(confusion_error(format!(
                    "package `{}` cannot depend on itself",
                    subject.coordinate.label()
                )));
            }
            match identities.get(&dependency.package) {
                None => {
                    return Err(confusion_error(format!(
                        "dependency `{}` is not present in the explicit subject set",
                        dependency.label()
                    )))
                }
                Some(version) if version != &dependency.version => {
                    return Err(confusion_error(format!(
                        "dependency `{}` disagrees with submitted version `{version}`",
                        dependency.label()
                    )))
                }
                Some(_) => {}
            }
            budget.edges = budget
                .edges
                .checked_add(1)
                .ok_or_else(|| limit_error("dependency_edges overflow".to_owned()))?;
            ensure_at_most(budget.edges, MAX_EDGES, "dependency_edges")?;
        }
    }

    let (order, _) = topological_order(&packages, &mut budget)?;
    let mut depth = BTreeMap::<Coordinate, usize>::new();
    let mut closures = BTreeMap::<Coordinate, BTreeSet<String>>::new();
    for coordinate in &order {
        let subject = packages.get(coordinate).ok_or_else(|| {
            integrity_error("topological order references an absent package".to_owned())
        })?;
        let mut package_depth = 0usize;
        let mut closure = subject
            .capabilities
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        for dependency in &subject.dependencies {
            debit_work(&mut budget, 1)?;
            package_depth = package_depth.max(
                depth
                    .get(dependency)
                    .ok_or_else(|| {
                        integrity_error(
                            "dependency depth was unavailable in topological order".to_owned(),
                        )
                    })?
                    .checked_add(1)
                    .ok_or_else(|| limit_error("dependency_depth overflow".to_owned()))?,
            );
            closure.extend(
                closures
                    .get(dependency)
                    .ok_or_else(|| {
                        integrity_error(
                            "dependency closure was unavailable in topological order".to_owned(),
                        )
                    })?
                    .iter()
                    .cloned(),
            );
        }
        ensure_at_most(package_depth, MAX_DEPENDENCY_DEPTH, "dependency_depth")?;
        ensure_at_most(closure.len(), MAX_CAPABILITIES, "capability_closure")?;
        debit_work(&mut budget, closure.len())?;
        checked_add(
            &mut budget.capability_closure_facts,
            closure.len(),
            MAX_WORK_UNITS,
            "capability_closure_facts",
        )?;
        budget.dependency_depth = budget.dependency_depth.max(package_depth);
        depth.insert(coordinate.clone(), package_depth);
        closures.insert(coordinate.clone(), closure);
    }

    let mut all_capabilities = BTreeSet::new();
    for closure in closures.values() {
        all_capabilities.extend(closure.iter().cloned());
    }
    ensure_at_most(
        all_capabilities.len(),
        MAX_CAPABILITIES,
        "top-level capability_closure",
    )?;

    let target_matrix = intersect_targets(packages.values())?;
    let depended_on = packages
        .values()
        .flat_map(|subject| subject.dependencies.iter().cloned())
        .collect::<BTreeSet<_>>();
    let roots = packages
        .keys()
        .filter(|coordinate| !depended_on.contains(*coordinate))
        .cloned()
        .collect::<Vec<_>>();

    let mut edges = Vec::with_capacity(budget.edges);
    for dependent in packages.keys() {
        let subject = packages.get(dependent).ok_or_else(|| {
            integrity_error("edge inventory references an absent dependent".to_owned())
        })?;
        for dependency in &subject.dependencies {
            edges.push((dependency.clone(), dependent.clone()));
        }
    }
    edges.sort();

    // Every retained string is debited once in addition to the exact owned
    // subject bytes. This is a conservative deterministic builder bound.
    budget.builder_bytes = budget.total_subject_bytes;
    for subject in packages.values() {
        debit_builder_string(&mut budget, &subject.coordinate.package)?;
        debit_builder_string(&mut budget, &subject.coordinate.version)?;
        for dependency in &subject.dependencies {
            debit_builder_string(&mut budget, &dependency.package)?;
            debit_builder_string(&mut budget, &dependency.version)?;
        }
        for fact in &subject.capabilities {
            debit_builder_string(&mut budget, fact)?;
        }
        for fact in &subject.licenses {
            debit_builder_string(&mut budget, fact)?;
        }
        for fact in &subject.provenance {
            debit_builder_string(&mut budget, &fact.kind)?;
            debit_builder_string(&mut budget, &fact.value)?;
        }
    }

    converge_output_budget(budget, options.max_bytes, 32, |fixed_budget| {
        render_lock(
            &roots,
            &order,
            &packages,
            &closures,
            &edges,
            &target_matrix,
            &all_capabilities,
            options,
            fixed_budget,
        )
    })
}

fn converge_output_budget(
    mut budget: Budget,
    requested_max_bytes: usize,
    max_iterations: usize,
    mut render: impl FnMut(Budget) -> String,
) -> Result<String, Diagnostic> {
    let mut previous_output = usize::MAX;
    for _ in 0..max_iterations {
        budget.output_bytes = if previous_output == usize::MAX {
            0
        } else {
            previous_output
        };
        let output = render(budget);
        let output_bytes = output.len();
        if output_bytes > requested_max_bytes || output_bytes > MAX_OUTPUT_BYTES {
            return Err(limit_error(format!(
                "output_bytes exceeds {}",
                requested_max_bytes.min(MAX_OUTPUT_BYTES)
            )));
        }
        if output_bytes == previous_output {
            return Ok(output);
        }
        previous_output = output_bytes;
    }
    Err(replay_error(
        "output byte accounting did not reach a fixed point".to_owned(),
    ))
}

fn parse_subject(subject: &str, budget: &mut Budget) -> Result<PackageSubject, Diagnostic> {
    validate_json_wire(subject, "subject")?;
    let (schema, subject_digest, declared_bytes, payload) =
        parse_wrapper(subject, SUBJECT_DIGEST_DOMAIN, "subject")?;
    if schema != SUBJECT_SCHEMA {
        return Err(grammar_error(format!(
            "subject schema must be {SUBJECT_SCHEMA}"
        )));
    }
    let value: Value = serde_json::from_str(payload)
        .map_err(|error| grammar_error(format!("subject payload is not valid JSON: {error}")))?;
    expect_object_keys(
        &value,
        &[
            "capabilities",
            "dependencies",
            "licenses",
            "package",
            "provenance",
            "report",
            "schema",
            "version",
        ],
        "subject payload",
    )?;
    if value["schema"].as_str() != Some(SUBJECT_SCHEMA) {
        return Err(grammar_error(format!(
            "subject payload schema must be {SUBJECT_SCHEMA}"
        )));
    }

    let package = required_str(&value, "package", "subject")?.to_owned();
    validate_package_identity(&package)?;
    let version = required_str(&value, "version", "subject")?.to_owned();
    validate_version(&version)?;
    let coordinate = Coordinate { package, version };

    let report = value["report"]
        .as_object()
        .ok_or_else(|| grammar_error("subject report must be an object".to_owned()))?;
    expect_map_keys(
        report,
        &["bytes", "digest", "envelope", "schema"],
        "subject report",
    )?;
    let report_schema = required_str(&value["report"], "schema", "subject report")?;
    if report_schema != package_report::SCHEMA {
        return Err(confusion_error(format!(
            "subject report schema must be {}",
            package_report::SCHEMA
        )));
    }
    let report_digest = required_str(&value["report"], "digest", "subject report")?.to_owned();
    let report_bytes = required_usize(&value["report"], "bytes", "subject report")?;
    let report_envelope = required_str(&value["report"], "envelope", "subject report")?;
    if report_envelope.len() != report_bytes {
        return Err(integrity_error(
            "subject report byte count does not match the exact embedded envelope".to_owned(),
        ));
    }
    validate_package_report_wire(report_envelope)?;
    package_report::verify_envelope(report_envelope).map_err(|_| {
        integrity_error("embedded Interface Package Report v1 failed independent replay".to_owned())
    })?;
    let report_value: Value = serde_json::from_str(report_envelope)
        .map_err(|_| integrity_error("embedded report is not JSON".to_owned()))?;
    if report_value["digest"].as_str() != Some(report_digest.as_str()) {
        return Err(integrity_error(
            "subject report digest disagrees with the exact embedded report".to_owned(),
        ));
    }
    let report_package = report_value["payload"]["package"]["name"]
        .as_str()
        .ok_or_else(|| integrity_error("embedded report package name is missing".to_owned()))?;
    if report_package != coordinate.package {
        return Err(confusion_error(
            "subject package identity must exactly equal the report module identity".to_owned(),
        ));
    }
    let targets = parse_targets(&report_value["payload"]["targets"])?;

    let dependencies = parse_dependencies(&value["dependencies"])?;
    ensure_at_most(
        dependencies.len(),
        MAX_DEPENDENCIES_PER_PACKAGE,
        "dependencies_per_package",
    )?;
    let capabilities = parse_sorted_strings(
        &value["capabilities"],
        MAX_CAPABILITIES,
        "capabilities",
        validate_capability,
    )?;
    let licenses = parse_sorted_strings(
        &value["licenses"],
        MAX_LICENSES,
        "licenses",
        validate_license,
    )?;
    let provenance = parse_provenance(&value["provenance"])?;

    debit_work(
        budget,
        dependencies
            .len()
            .checked_add(capabilities.len())
            .and_then(|value| value.checked_add(licenses.len()))
            .and_then(|value| value.checked_add(provenance.len()))
            .ok_or_else(|| limit_error("subject fact work overflow".to_owned()))?,
    )?;

    checked_add(
        &mut budget.capability_facts,
        capabilities.len(),
        MAX_WORK_UNITS,
        "capability_facts",
    )?;
    checked_add(
        &mut budget.license_facts,
        licenses.len(),
        MAX_WORK_UNITS,
        "license_facts",
    )?;
    checked_add(
        &mut budget.provenance_facts,
        provenance.len(),
        MAX_WORK_UNITS,
        "provenance_facts",
    )?;

    let canonical_payload = render_subject_payload(
        &coordinate,
        report_schema,
        &report_digest,
        report_bytes,
        report_envelope,
        &dependencies,
        &capabilities,
        &licenses,
        &provenance,
    );
    if canonical_payload != payload {
        return Err(grammar_error(
            "subject payload is not in exact canonical form".to_owned(),
        ));
    }
    if declared_bytes != payload.len() {
        return Err(integrity_error(
            "subject payload byte count mismatch".to_owned(),
        ));
    }

    Ok(PackageSubject {
        coordinate,
        subject_digest,
        subject_bytes: subject.len(),
        report_digest,
        report_bytes,
        report_envelope_digest: domain_digest(
            REPORT_ENVELOPE_DIGEST_DOMAIN,
            report_envelope.as_bytes(),
        ),
        targets,
        dependencies,
        capabilities,
        licenses,
        provenance,
    })
}

fn parse_wrapper<'a>(
    envelope: &'a str,
    domain: &[u8],
    label: &str,
) -> Result<(String, String, usize, &'a str), Diagnostic> {
    let value: Value = serde_json::from_str(envelope)
        .map_err(|error| grammar_error(format!("{label} is not valid JSON: {error}")))?;
    expect_object_keys(&value, &["bytes", "digest", "payload", "schema"], label)?;
    let schema = required_str(&value, "schema", label)?.to_owned();
    let digest = required_str(&value, "digest", label)?.to_owned();
    let bytes = required_usize(&value, "bytes", label)?;
    const MARKER: &str = "\"payload\":";
    let offset = envelope
        .find(MARKER)
        .ok_or_else(|| grammar_error(format!("{label} is missing payload")))?
        + MARKER.len();
    if !envelope.ends_with('}') {
        return Err(grammar_error(format!("{label} must end with `}}`")));
    }
    let payload = &envelope[offset..envelope.len() - 1];
    if !payload.starts_with('{') || !payload.ends_with('}') {
        return Err(grammar_error(format!("{label} payload must be an object")));
    }
    if bytes != payload.len() {
        return Err(integrity_error(format!(
            "{label} payload byte count mismatch"
        )));
    }
    if digest != domain_digest(domain, payload.as_bytes()) {
        return Err(integrity_error(format!("{label} payload digest mismatch")));
    }
    let expected = format!(
        "{{\"schema\":{},\"digest\":{},\"bytes\":{},\"payload\":{}}}",
        quote_json(&schema),
        quote_json(&digest),
        bytes,
        payload
    );
    if expected != envelope {
        return Err(grammar_error(format!(
            "{label} wrapper is not in exact canonical form"
        )));
    }
    Ok((schema, digest, bytes, payload))
}

fn validate_package_report_wire(report: &str) -> Result<(), Diagnostic> {
    validate_json_wire(report, "embedded report")?;
    let value: Value = serde_json::from_str(report)
        .map_err(|error| integrity_error(format!("embedded report is not JSON: {error}")))?;
    expect_object_keys(
        &value,
        &["bytes", "digest", "payload", "schema"],
        "embedded report",
    )?;
    if top_level_object_keys(report)? != ["schema", "digest", "bytes", "payload"] {
        return Err(integrity_error(
            "embedded report wrapper key order is not canonical".to_owned(),
        ));
    }
    const PAYLOAD_MARKER: &str = "\"payload\":";
    let payload_offset = report
        .find(PAYLOAD_MARKER)
        .ok_or_else(|| integrity_error("embedded report payload is missing".to_owned()))?
        + PAYLOAD_MARKER.len();
    let raw_payload = &report[payload_offset..report.len() - 1];
    if top_level_object_keys(raw_payload)?
        != [
            "schema",
            "source",
            "limits",
            "package",
            "targets",
            "exports",
            "exclusions",
            "unavailable_capabilities",
            "nonclaims",
        ]
    {
        return Err(integrity_error(
            "embedded report payload key order is not canonical".to_owned(),
        ));
    }
    let payload = &value["payload"];
    expect_object_keys(
        payload,
        &[
            "exclusions",
            "exports",
            "limits",
            "nonclaims",
            "package",
            "schema",
            "source",
            "targets",
            "unavailable_capabilities",
        ],
        "embedded report payload",
    )?;
    expect_object_keys(
        &payload["source"],
        &["path", "revision", "sha256"],
        "report source",
    )?;
    expect_object_keys(&payload["limits"], &["max_bytes"], "report limits")?;
    let report_max_bytes = required_usize(&payload["limits"], "max_bytes", "report limits")?;
    package_report::PackageReportOptions::new(report_max_bytes).map_err(|_| {
        integrity_error("report max_bytes is outside the Package Report v1 bounds".to_owned())
    })?;
    expect_object_keys(
        &payload["package"],
        &[
            "exports_admitted",
            "exports_excluded",
            "functions_total",
            "name",
        ],
        "report package",
    )?;
    for target in payload["targets"]
        .as_array()
        .ok_or_else(|| integrity_error("report targets must be an array".to_owned()))?
    {
        expect_object_keys(target, &TARGET_KEYS, "report target")?;
    }
    for export in payload["exports"]
        .as_array()
        .ok_or_else(|| integrity_error("report exports must be an array".to_owned()))?
    {
        expect_object_keys(
            export,
            &[
                "effects",
                "ensures",
                "name",
                "native64",
                "parameters",
                "requires",
                "result",
                "stable_id",
            ],
            "report export",
        )?;
        expect_object_keys(
            &export["native64"],
            &["signature", "signature_sha256", "symbol"],
            "report native signature",
        )?;
    }
    for exclusion in payload["exclusions"]
        .as_array()
        .ok_or_else(|| integrity_error("report exclusions must be an array".to_owned()))?
    {
        expect_object_keys(
            exclusion,
            &["name", "reason", "stable_id"],
            "report exclusion",
        )?;
    }
    let nonclaims = payload["nonclaims"]
        .as_array()
        .ok_or_else(|| integrity_error("report nonclaims must be an array".to_owned()))?;
    if nonclaims.len() != PACKAGE_REPORT_NONCLAIMS.len()
        || nonclaims
            .iter()
            .zip(PACKAGE_REPORT_NONCLAIMS)
            .any(|(actual, expected)| actual.as_str() != Some(expected))
    {
        return Err(integrity_error(
            "report nonclaims must be the exact closed Package Report v1 inventory".to_owned(),
        ));
    }
    Ok(())
}

fn parse_dependencies(value: &Value) -> Result<Vec<Coordinate>, Diagnostic> {
    let rows = value
        .as_array()
        .ok_or_else(|| grammar_error("dependencies must be an array".to_owned()))?;
    ensure_at_most(
        rows.len(),
        MAX_DEPENDENCIES_PER_PACKAGE,
        "dependencies_per_package",
    )?;
    let mut output = Vec::with_capacity(rows.len());
    for row in rows {
        expect_object_keys(row, &["package", "version"], "dependency")?;
        let package = required_str(row, "package", "dependency")?.to_owned();
        validate_package_identity(&package)?;
        let version = required_str(row, "version", "dependency")?.to_owned();
        validate_version(&version)?;
        output.push(Coordinate { package, version });
    }
    if !strictly_sorted(&output) {
        return Err(confusion_error(
            "dependencies must be strictly coordinate-sorted and unique".to_owned(),
        ));
    }
    Ok(output)
}

fn parse_targets(value: &Value) -> Result<Vec<TargetFact>, Diagnostic> {
    let rows = value
        .as_array()
        .ok_or_else(|| integrity_error("report targets must be an array".to_owned()))?;
    let mut targets = Vec::with_capacity(rows.len());
    for row in rows {
        let target = required_str(row, "target", "report target")?.to_owned();
        let available = row["available"].as_bool().ok_or_else(|| {
            integrity_error("report target availability must be boolean".to_owned())
        })?;
        targets.push(TargetFact { target, available });
    }
    if targets.is_empty() {
        return Err(confusion_error(
            "report target matrix cannot be empty".to_owned(),
        ));
    }
    Ok(targets)
}

fn parse_sorted_strings(
    value: &Value,
    maximum: usize,
    label: &str,
    validator: fn(&str) -> Result<(), Diagnostic>,
) -> Result<Vec<String>, Diagnostic> {
    let rows = value
        .as_array()
        .ok_or_else(|| grammar_error(format!("{label} must be an array")))?;
    ensure_at_most(rows.len(), maximum, label)?;
    let mut output = Vec::with_capacity(rows.len());
    for row in rows {
        let fact = row
            .as_str()
            .ok_or_else(|| grammar_error(format!("{label} entries must be strings")))?;
        validator(fact)?;
        output.push(fact.to_owned());
    }
    if !strictly_sorted(&output) {
        return Err(confusion_error(format!(
            "{label} must be strictly byte-sorted and unique"
        )));
    }
    Ok(output)
}

fn parse_provenance(value: &Value) -> Result<Vec<ProvenanceFact>, Diagnostic> {
    let rows = value
        .as_array()
        .ok_or_else(|| grammar_error("provenance must be an array".to_owned()))?;
    ensure_at_most(rows.len(), MAX_PROVENANCE, "provenance")?;
    let mut output = Vec::with_capacity(rows.len());
    for row in rows {
        expect_object_keys(row, &["kind", "value"], "provenance fact")?;
        let kind = required_str(row, "kind", "provenance fact")?.to_owned();
        if !PROVENANCE_KINDS.contains(&kind.as_str()) {
            return Err(confusion_error(
                "provenance kind is outside the closed vocabulary".to_owned(),
            ));
        }
        let value = required_str(row, "value", "provenance fact")?.to_owned();
        if value.is_empty() || value.len() > 1_024 || value.contains('\0') {
            return Err(confusion_error(
                "provenance value must be 1..1024 UTF-8 bytes without NUL".to_owned(),
            ));
        }
        output.push(ProvenanceFact { kind, value });
    }
    if !strictly_sorted(&output) {
        return Err(confusion_error(
            "provenance must be strictly (kind,value)-sorted and unique".to_owned(),
        ));
    }
    Ok(output)
}

fn topological_order(
    packages: &BTreeMap<Coordinate, PackageSubject>,
    budget: &mut Budget,
) -> Result<(Vec<Coordinate>, BTreeMap<Coordinate, Vec<Coordinate>>), Diagnostic> {
    let mut remaining = packages
        .iter()
        .map(|(coordinate, subject)| (coordinate.clone(), subject.dependencies.len()))
        .collect::<BTreeMap<_, _>>();
    let mut dependents = packages
        .keys()
        .cloned()
        .map(|coordinate| (coordinate, Vec::new()))
        .collect::<BTreeMap<_, _>>();
    for (dependent, subject) in packages {
        for dependency in &subject.dependencies {
            dependents
                .get_mut(dependency)
                .ok_or_else(|| {
                    integrity_error("dependency graph references an absent package".to_owned())
                })?
                .push(dependent.clone());
        }
    }
    for rows in dependents.values_mut() {
        rows.sort();
    }
    let mut ready = remaining
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(coordinate, _)| coordinate.clone())
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(packages.len());
    while let Some(coordinate) = ready.pop_first() {
        debit_work(budget, 1)?;
        order.push(coordinate.clone());
        let coordinate_dependents = dependents.get(&coordinate).ok_or_else(|| {
            integrity_error("topological frontier references an absent package".to_owned())
        })?;
        for dependent in coordinate_dependents {
            let count = remaining
                .get_mut(dependent)
                .ok_or_else(|| integrity_error("topological dependent is absent".to_owned()))?;
            *count = count.checked_sub(1).ok_or_else(|| {
                integrity_error("topological dependency count underflow".to_owned())
            })?;
            if *count == 0 {
                ready.insert(dependent.clone());
            }
        }
    }
    if order.len() != packages.len() {
        return Err(cycle_error(
            "package dependency graph contains a cycle".to_owned(),
        ));
    }
    Ok((order, dependents))
}

fn intersect_targets<'a>(
    mut subjects: impl Iterator<Item = &'a PackageSubject>,
) -> Result<Vec<TargetFact>, Diagnostic> {
    let first = subjects
        .next()
        .ok_or_else(|| limit_error("package set is empty".to_owned()))?;
    let mut targets = first.targets.clone();
    for subject in subjects {
        if subject.targets.len() != targets.len()
            || subject
                .targets
                .iter()
                .zip(targets.iter())
                .any(|(left, right)| left.target != right.target)
        {
            return Err(confusion_error(
                "package report target matrices use incompatible target identities or order"
                    .to_owned(),
            ));
        }
        for (aggregate, item) in targets.iter_mut().zip(subject.targets.iter()) {
            aggregate.available &= item.available;
        }
    }
    Ok(targets)
}

#[allow(clippy::too_many_arguments)]
fn render_subject_payload(
    coordinate: &Coordinate,
    report_schema: &str,
    report_digest: &str,
    report_bytes: usize,
    report_envelope: &str,
    dependencies: &[Coordinate],
    capabilities: &[String],
    licenses: &[String],
    provenance: &[ProvenanceFact],
) -> String {
    format!(
        "{{\"schema\":{},\"package\":{},\"version\":{},\"report\":{{\"schema\":{},\"digest\":{},\"bytes\":{},\"envelope\":{}}},\"dependencies\":[{}],\"capabilities\":[{}],\"licenses\":[{}],\"provenance\":[{}]}}",
        quote_json(SUBJECT_SCHEMA),
        quote_json(&coordinate.package),
        quote_json(&coordinate.version),
        quote_json(report_schema),
        quote_json(report_digest),
        report_bytes,
        quote_json(report_envelope),
        join(dependencies.iter().map(Coordinate::render)),
        join(capabilities.iter().map(|value| quote_json(value))),
        join(licenses.iter().map(|value| quote_json(value))),
        join(provenance.iter().map(ProvenanceFact::render)),
    )
}

#[allow(clippy::too_many_arguments)]
fn render_lock(
    roots: &[Coordinate],
    order: &[Coordinate],
    packages: &BTreeMap<Coordinate, PackageSubject>,
    closures: &BTreeMap<Coordinate, BTreeSet<String>>,
    edges: &[(Coordinate, Coordinate)],
    target_matrix: &[TargetFact],
    all_capabilities: &BTreeSet<String>,
    options: &PackageLockOptions,
    budget: Budget,
) -> String {
    let package_rows = order.iter().map(|coordinate| {
        let subject = &packages[coordinate];
        format!(
            "{{\"package\":{},\"version\":{},\"subject_digest\":{},\"subject_bytes\":{},\"report\":{{\"schema\":{},\"digest\":{},\"envelope_digest\":{},\"bytes\":{}}},\"targets\":[{}],\"dependencies\":[{}],\"capabilities\":{{\"direct\":[{}],\"closure\":[{}]}},\"licenses\":[{}],\"provenance\":[{}]}}",
            quote_json(&coordinate.package),
            quote_json(&coordinate.version),
            quote_json(&subject.subject_digest),
            subject.subject_bytes,
            quote_json(package_report::SCHEMA),
            quote_json(&subject.report_digest),
            quote_json(&subject.report_envelope_digest),
            subject.report_bytes,
            join(subject.targets.iter().map(TargetFact::render)),
            join(subject.dependencies.iter().map(Coordinate::render)),
            join(subject.capabilities.iter().map(|value| quote_json(value))),
            join(closures[coordinate].iter().map(|value| quote_json(value))),
            join(subject.licenses.iter().map(|value| quote_json(value))),
            join(subject.provenance.iter().map(ProvenanceFact::render)),
        )
    });
    let edge_rows = edges.iter().map(|(dependency, dependent)| {
        format!(
            "{{\"dependency\":{},\"dependent\":{}}}",
            dependency.render(),
            dependent.render()
        )
    });
    let payload = format!(
        "{{\"schema\":{},\"roots\":[{}],\"packages\":[{}],\"edges\":[{}],\"target_matrix\":[{}],\"capability_closure\":[{}],\"limits\":{{\"max_packages\":{},\"max_subject_bytes\":{},\"max_total_subject_bytes\":{},\"max_dependencies_per_package\":{},\"max_dependency_edges\":{},\"max_dependency_depth\":{},\"max_capabilities\":{},\"max_licenses\":{},\"max_provenance\":{},\"max_json_depth\":{},\"max_builder_work_units\":{},\"max_builder_bytes\":{},\"max_output_bytes\":{},\"requested_max_bytes\":{}}},\"budget\":{{\"used_packages\":{},\"used_total_subject_bytes\":{},\"used_dependency_edges\":{},\"used_dependency_depth\":{},\"used_capability_facts\":{},\"used_capability_closure_facts\":{},\"used_license_facts\":{},\"used_provenance_facts\":{},\"used_builder_work_units\":{},\"used_builder_bytes\":{},\"used_output_bytes\":{}}},\"nonclaims\":[{}]}}",
        quote_json(SCHEMA),
        join(roots.iter().map(Coordinate::render)),
        join(package_rows),
        join(edge_rows),
        join(target_matrix.iter().map(TargetFact::render)),
        join(all_capabilities.iter().map(|value| quote_json(value))),
        MAX_PACKAGES,
        MAX_SUBJECT_BYTES,
        MAX_TOTAL_SUBJECT_BYTES,
        MAX_DEPENDENCIES_PER_PACKAGE,
        MAX_EDGES,
        MAX_DEPENDENCY_DEPTH,
        MAX_CAPABILITIES,
        MAX_LICENSES,
        MAX_PROVENANCE,
        MAX_JSON_DEPTH,
        MAX_WORK_UNITS,
        MAX_BUILDER_BYTES,
        MAX_OUTPUT_BYTES,
        options.max_bytes,
        budget.packages,
        budget.total_subject_bytes,
        budget.edges,
        budget.dependency_depth,
        budget.capability_facts,
        budget.capability_closure_facts,
        budget.license_facts,
        budget.provenance_facts,
        budget.work_units,
        budget.builder_bytes,
        budget.output_bytes,
        NONCLAIMS_JSON,
    );
    format!(
        "{{\"schema\":{},\"digest\":{},\"bytes\":{},\"payload\":{}}}",
        quote_json(SCHEMA),
        quote_json(&domain_digest(LOCK_DIGEST_DOMAIN, payload.as_bytes())),
        payload.len(),
        payload
    )
}

fn validate_json_wire(value: &str, label: &str) -> Result<(), Diagnostic> {
    if value.is_empty()
        || value.starts_with('\u{feff}')
        || value.ends_with('\n')
        || value.contains('\r')
    {
        return Err(grammar_error(format!(
            "{label} must be compact UTF-8 without BOM, CRLF, or terminal LF"
        )));
    }
    let mut depth = 0usize;
    let mut maximum = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for byte in value.bytes() {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => {
                depth = depth
                    .checked_add(1)
                    .ok_or_else(|| limit_error("json_depth overflow".to_owned()))?;
                maximum = maximum.max(depth);
            }
            b'}' | b']' => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| grammar_error(format!("{label} has unbalanced JSON")))?;
            }
            b' ' | b'\t' | b'\n' => {
                return Err(grammar_error(format!(
                    "{label} contains insignificant whitespace"
                )))
            }
            _ => {}
        }
    }
    if in_string || depth != 0 {
        return Err(grammar_error(format!("{label} has incomplete JSON")));
    }
    if maximum > MAX_JSON_DEPTH {
        return Err(limit_error(format!("json_depth exceeds {MAX_JSON_DEPTH}")));
    }
    Ok(())
}

fn top_level_object_keys(value: &str) -> Result<Vec<String>, Diagnostic> {
    if !value.starts_with('{') || !value.ends_with('}') {
        return Err(grammar_error(
            "canonical value must be an object".to_owned(),
        ));
    }
    let bytes = value.as_bytes();
    let mut keys = Vec::new();
    let mut depth = 0usize;
    let mut index = 0usize;
    let mut expecting_key = false;
    while index < bytes.len() {
        match bytes[index] {
            b'{' | b'[' => {
                depth += 1;
                if depth == 1 {
                    expecting_key = true;
                }
                index += 1;
            }
            b'}' | b']' => {
                depth = depth.saturating_sub(1);
                index += 1;
            }
            b',' if depth == 1 => {
                expecting_key = true;
                index += 1;
            }
            b'"' => {
                let start = index;
                index += 1;
                let mut escaped = false;
                while index < bytes.len() {
                    let byte = bytes[index];
                    index += 1;
                    if escaped {
                        escaped = false;
                    } else if byte == b'\\' {
                        escaped = true;
                    } else if byte == b'"' {
                        break;
                    }
                }
                if index > bytes.len() || bytes.get(index - 1) != Some(&b'"') {
                    return Err(grammar_error("unterminated JSON string".to_owned()));
                }
                if expecting_key && depth == 1 {
                    let key: String = serde_json::from_str(&value[start..index])
                        .map_err(|_| grammar_error("invalid object key".to_owned()))?;
                    if bytes.get(index) != Some(&b':') {
                        return Err(grammar_error(
                            "object key must be followed by colon".to_owned(),
                        ));
                    }
                    keys.push(key);
                    expecting_key = false;
                }
            }
            _ => index += 1,
        }
    }
    Ok(keys)
}

fn validate_package_identity(value: &str) -> Result<(), Diagnostic> {
    if value.is_empty() || value.len() > 255 {
        return Err(confusion_error(
            "package identity must contain 1..255 bytes".to_owned(),
        ));
    }
    for segment in value.split('.') {
        let mut bytes = segment.bytes();
        let Some(first) = bytes.next() else {
            return Err(confusion_error(
                "package identity contains an empty segment".to_owned(),
            ));
        };
        if !(first.is_ascii_alphabetic() || first == b'_')
            || !bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err(confusion_error(
                "package identity is not canonical dotted module syntax".to_owned(),
            ));
        }
    }
    Ok(())
}

fn validate_version(value: &str) -> Result<(), Diagnostic> {
    let parts = value.split('.').collect::<Vec<_>>();
    if parts.len() != 3
        || parts.iter().any(|part| {
            part.is_empty()
                || !part.bytes().all(|byte| byte.is_ascii_digit())
                || (part.len() > 1 && part.starts_with('0'))
                || part.parse::<u32>().is_err()
        })
    {
        return Err(confusion_error(
            "version must be canonical numeric MAJOR.MINOR.PATCH".to_owned(),
        ));
    }
    Ok(())
}

fn validate_capability(value: &str) -> Result<(), Diagnostic> {
    let Some(domain) = CAPABILITY_DOMAINS.iter().find(|domain| {
        value == **domain
            || value
                .strip_prefix(**domain)
                .is_some_and(|suffix| suffix.starts_with('.'))
    }) else {
        return Err(confusion_error(
            "capability is outside the closed capability vocabulary".to_owned(),
        ));
    };
    let suffix = value
        .strip_prefix(*domain)
        .ok_or_else(|| confusion_error("capability domain prefix replay disagreed".to_owned()))?;
    if suffix.is_empty() {
        return Ok(());
    }
    if suffix.len() > 128
        || suffix[1..].split('.').any(|part| {
            part.is_empty()
                || !part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        })
    {
        return Err(confusion_error(
            "capability suffix is not canonical dotted ASCII".to_owned(),
        ));
    }
    Ok(())
}

fn validate_license(value: &str) -> Result<(), Diagnostic> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_graphic() && !matches!(byte, b'"' | b'\\'))
    {
        return Err(confusion_error(
            "license fact must be 1..128 printable ASCII bytes".to_owned(),
        ));
    }
    Ok(())
}

fn debit_work(budget: &mut Budget, amount: usize) -> Result<(), Diagnostic> {
    checked_add(
        &mut budget.work_units,
        amount,
        MAX_WORK_UNITS,
        "builder_work_units",
    )
}

fn debit_builder_string(budget: &mut Budget, value: &str) -> Result<(), Diagnostic> {
    checked_add(
        &mut budget.builder_bytes,
        value.len(),
        MAX_BUILDER_BYTES,
        "builder_bytes",
    )
}

fn checked_add(
    target: &mut usize,
    amount: usize,
    maximum: usize,
    label: &str,
) -> Result<(), Diagnostic> {
    *target = target
        .checked_add(amount)
        .ok_or_else(|| limit_error(format!("{label} overflow")))?;
    if *target > maximum {
        return Err(limit_error(format!("{label} exceeds {maximum}")));
    }
    Ok(())
}

fn validate_package_count(count: usize) -> Result<(), Diagnostic> {
    if !(1..=MAX_PACKAGES).contains(&count) {
        Err(limit_error(format!(
            "package count must be between 1 and {MAX_PACKAGES}"
        )))
    } else {
        Ok(())
    }
}

fn ensure_at_most(value: usize, maximum: usize, label: &str) -> Result<(), Diagnostic> {
    if value > maximum {
        Err(limit_error(format!("{label} exceeds {maximum}")))
    } else {
        Ok(())
    }
}

fn required_str<'a>(value: &'a Value, key: &str, label: &str) -> Result<&'a str, Diagnostic> {
    value[key]
        .as_str()
        .ok_or_else(|| grammar_error(format!("{label} {key} must be a string")))
}

fn required_usize(value: &Value, key: &str, label: &str) -> Result<usize, Diagnostic> {
    let number = value[key]
        .as_u64()
        .ok_or_else(|| grammar_error(format!("{label} {key} must be an unsigned integer")))?;
    usize::try_from(number)
        .map_err(|_| limit_error(format!("{label} {key} does not fit the host size")))
}

fn expect_object_keys(value: &Value, expected: &[&str], label: &str) -> Result<(), Diagnostic> {
    let object = value
        .as_object()
        .ok_or_else(|| grammar_error(format!("{label} must be an object")))?;
    expect_map_keys(object, expected, label)
}

fn expect_map_keys(
    object: &serde_json::Map<String, Value>,
    expected: &[&str],
    label: &str,
) -> Result<(), Diagnostic> {
    let actual = object.keys().map(String::as_str).collect::<Vec<_>>();
    if actual != expected {
        return Err(grammar_error(format!(
            "{label} has a foreign, missing, or duplicate member"
        )));
    }
    Ok(())
}

fn strictly_sorted<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn join(values: impl Iterator<Item = String>) -> String {
    values.collect::<Vec<_>>().join(",")
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

fn option_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-L401", message)
}

fn grammar_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-L402", message)
}

fn integrity_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-L403", message)
}

fn confusion_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-L404", message)
}

fn cycle_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-L405", message)
}

fn limit_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-L406", message)
}

fn replay_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-L407", message)
}

#[cfg(test)]
mod tests;
