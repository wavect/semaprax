//! Deterministic, read-only Build Capability Manifest v1.
//!
//! [`generate`] projects one verified single-file SEMAPRAX module into one
//! canonical compact JSON envelope (`semaprax.capability-manifest.v1`)
//! declaring the EXACT build capabilities the module requires: the module
//! permit inventory, every declared function effect set, every declared
//! interface-import effect set, and an explicit empty-by-default ambient
//! authority assertion over the five admitted domains (`filesystem`, `home`,
//! `network`, `process`, `secrets`). Every capability token anywhere in the
//! module — module permits, interface permits, function effects, and
//! interface-import effects — must fall inside that closed vocabulary;
//! anything else fails the whole command closed before any bytes are emitted.
//! The ambient authority section is derived only from the required
//! inventories (module permits plus declared function and import effects);
//! an interface permit that no import consumes is still fail-closed-checked
//! but does not by itself mark a domain as declared. Admission mirrors the
//! established scalar projection profile in spirit: signatures, generics,
//! aggregates, and resources are irrelevant here because the manifest speaks
//! only about capabilities, so there are no signature exclusions and no
//! partial manifests.
//!
//! [`verify_envelope`] independently replays one envelope: exact envelope
//! shape, declared byte count, domain-separated payload digest, closed
//! vocabulary over every listed token, and equality of the embedded ambient
//! authority section with its derivation from the listed inventories.
//! [`verify_envelope_against_source`] additionally rebinds the current source
//! bytes to the embedded source digest and fails closed on drift.
//!
//! Diagnostics use the previously unused `SPX-K2xx` family:
//! - `SPX-K201`: invalid options (bounds, malformed values).
//! - `SPX-K202`: a capability token outside the admitted closed vocabulary.
//! - `SPX-K203`: output byte-budget exhaustion (fail-closed, no truncation).
//! - `SPX-K204`: envelope consistency or replay failure.
//!
//! This tranche performs no sandbox enforcement at build time, no dependency
//! resolution, no lockfile or registry work, no network/home/secrets/
//! filesystem/process enforcement machinery, executes nothing, and changes
//! no source.

use std::collections::BTreeSet;
use std::path::Path;

use sha2::{Digest as _, Sha256};

use crate::bounded_output::{with_limit, BudgetedJoin as _};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::{graph, parse, patch, verify};

macro_rules! bformat {
    ($($argument:tt)*) => {
        crate::bounded_output::budgeted_format(format_args!($($argument)*))
    };
}

pub const SCHEMA: &str = "semaprax.capability-manifest.v1";

const DEFAULT_MAX_BYTES: usize = 64 * 1024;

const SOURCE_DIGEST_DOMAIN: &[u8] = b"semaprax.capability-manifest.source.v1\0";
const PAYLOAD_DIGEST_DOMAIN: &[u8] = b"semaprax.capability-manifest.payload.v1\0";

/// Closed vocabulary of admitted ambient-authority capability domains, in
/// canonical bytewise order. A token names a domain when it equals the domain
/// or starts with `<domain>.`.
pub const AMBIENT_DOMAINS: [&str; 5] = ["filesystem", "home", "network", "process", "secrets"];

const AMBIENT_NONE: &str = "none";
const AMBIENT_DECLARED: &str = "declared";

const NONCLAIMS_JSON: &str = "\"no_sandbox_enforcement\",\
\"no_dependency_resolution_or_lockfile\",\
\"no_package_registry_or_hosting\",\
\"no_network_home_secrets_filesystem_or_process_enforcement_machinery\",\
\"no_target_execution\",\
\"read_only_no_source_changes\"";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityManifestOptions {
    pub max_bytes: usize,
}

impl CapabilityManifestOptions {
    pub fn new(max_bytes: usize) -> Result<Self, Diagnostic> {
        if !(graph::MIN_AGENT_CONTEXT_BYTES..=graph::MAX_AGENT_CONTEXT_BYTES).contains(&max_bytes) {
            return Err(option_error(format!(
                "capability-manifest max_bytes must be between {} and {}",
                graph::MIN_AGENT_CONTEXT_BYTES,
                graph::MAX_AGENT_CONTEXT_BYTES
            )));
        }
        Ok(Self { max_bytes })
    }
}

impl Default for CapabilityManifestOptions {
    fn default() -> Self {
        Self {
            max_bytes: DEFAULT_MAX_BYTES,
        }
    }
}

fn option_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-K201", message)
}

fn vocabulary_error(token: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-K202",
        format!(
            "capability `{token}` is outside the admitted bounded vocabulary {}; refusing to emit a manifest",
            AMBIENT_DOMAINS.join(", ")
        ),
    )
}

fn consistency_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-K204", message)
}

struct ManifestInput {
    module_name: String,
    permits: Vec<String>,
    functions: Vec<Entry>,
    imports: Vec<ImportEntry>,
}

struct Entry {
    stable_id: String,
    name: String,
    effects: Vec<String>,
}

struct ImportEntry {
    stable_id: String,
    name: String,
    interface: String,
    effects: Vec<String>,
}

/// Generate the canonical `semaprax.capability-manifest.v1` envelope JSON for
/// one verified source file.
///
/// Read-only: source bytes must remain unchanged between the snapshot and the
/// final check or generation fails closed.
pub fn generate(
    source_path: &Path,
    options: &CapabilityManifestOptions,
) -> Result<String, Vec<Diagnostic>> {
    let canonical_source_path = patch::canonical_source_path(source_path)?;
    let snapshot = patch::read_source_snapshot(&canonical_source_path)?;
    let program = parse(snapshot.source(), source_path).map_err(|error| vec![error])?;
    let diagnostics = verify::verify(&program);
    if diagnostics.iter().any(|item| item.severity.is_error()) {
        return Err(diagnostics);
    }
    let revision = graph::revision(&program);

    // Every capability token anywhere in the module must sit inside the
    // closed vocabulary; only the required inventories below drive the
    // ambient authority assertion.
    let mut required = BTreeSet::new();
    for permit in &program.permits {
        require_vocabulary(permit)?;
        required.insert(permit.clone());
    }
    let mut permits = program.permits.clone();
    permits.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    permits.dedup();

    let mut functions = Vec::with_capacity(program.functions.len());
    for function in &program.functions {
        let mut effects = BTreeSet::new();
        for effect in &function.effects {
            require_vocabulary(effect)?;
            required.insert(effect.clone());
            effects.insert(effect.clone());
        }
        functions.push(Entry {
            stable_id: function.stable_id.clone(),
            name: function.name.clone(),
            effects: effects.into_iter().collect(),
        });
    }
    functions.sort_by(|left, right| left.stable_id.as_bytes().cmp(right.stable_id.as_bytes()));

    let mut imports = Vec::new();
    for interface in &program.interfaces {
        for permit in &interface.permits {
            require_vocabulary(permit)?;
        }
        for import in &interface.imports {
            let mut effects = BTreeSet::new();
            for effect in &import.effects {
                require_vocabulary(effect)?;
                required.insert(effect.clone());
                effects.insert(effect.clone());
            }
            imports.push(ImportEntry {
                stable_id: import.stable_id.clone(),
                name: import.name.clone(),
                interface: interface.name.clone(),
                effects: effects.into_iter().collect(),
            });
        }
    }
    imports.sort_by(|left, right| left.stable_id.as_bytes().cmp(right.stable_id.as_bytes()));

    let input = ManifestInput {
        module_name: program.module.clone(),
        permits,
        functions,
        imports,
    };

    let digest = source_digest(snapshot.source());
    let path_text = source_path.display().to_string();
    let (envelope, overflowed) = with_limit(options.max_bytes, || {
        render(&path_text, &revision, &digest, &input, options.max_bytes)
    });
    if overflowed {
        return Err(vec![Diagnostic::io(
            "SPX-K203",
            "capability-manifest output exceeds the max-bytes budget; refusing to truncate"
                .to_owned(),
        )]);
    }
    patch::validate_source_unchanged(&canonical_source_path, source_path, &snapshot, &revision)?;
    Ok(envelope)
}

/// Independently verify one envelope produced by [`generate`].
///
/// Recomputes the outer payload digest over the exact serialized payload
/// bytes, re-checks the declared byte count, replays the closed-vocabulary
/// admission over every listed capability token, and re-derives the embedded
/// ambient authority section from the listed inventories.
pub fn verify_envelope(envelope: &str) -> Result<(), Diagnostic> {
    let value: serde_json::Value = serde_json::from_str(envelope)
        .map_err(|error| consistency_error(format!("envelope is not valid JSON: {error}")))?;
    let Some(object) = value.as_object() else {
        return Err(consistency_error(
            "envelope must be a JSON object".to_owned(),
        ));
    };
    let keys: Vec<&str> = object.keys().map(String::as_str).collect();
    if keys != ["bytes", "digest", "payload", "schema"] {
        return Err(consistency_error(format!(
            "envelope keys must be exactly [bytes, digest, payload, schema], found {keys:?}"
        )));
    }
    if object["schema"].as_str() != Some(SCHEMA) {
        return Err(consistency_error(format!(
            "envelope schema must be {SCHEMA}"
        )));
    }
    let Some(envelope_digest) = object["digest"].as_str() else {
        return Err(consistency_error(
            "envelope digest must be a string".to_owned(),
        ));
    };
    let Some(declared_bytes) = object["bytes"].as_u64() else {
        return Err(consistency_error(
            "envelope bytes must be an unsigned integer".to_owned(),
        ));
    };
    const PAYLOAD_KEY: &str = "\"payload\":";
    let Some(offset) = envelope.find(PAYLOAD_KEY) else {
        return Err(consistency_error(
            "envelope is missing its payload member".to_owned(),
        ));
    };
    if !envelope.ends_with('}') {
        return Err(consistency_error("envelope must end with `}`".to_owned()));
    }
    let payload = &envelope[offset + PAYLOAD_KEY.len()..envelope.len() - 1];
    if !payload.starts_with('{') || !payload.ends_with('}') {
        return Err(consistency_error(
            "envelope payload must be a JSON object".to_owned(),
        ));
    }
    if declared_bytes != payload.len() as u64 {
        return Err(consistency_error(format!(
            "envelope declares {declared_bytes} payload bytes but {} are present",
            payload.len()
        )));
    }
    let recomputed = domain_digest(PAYLOAD_DIGEST_DOMAIN, payload.as_bytes());
    if envelope_digest != recomputed {
        return Err(consistency_error(
            "envelope digest does not match the exact payload bytes".to_owned(),
        ));
    }
    let payload_value: serde_json::Value = serde_json::from_str(payload)
        .map_err(|error| consistency_error(format!("payload is not valid JSON: {error}")))?;
    if payload_value["schema"].as_str() != Some(SCHEMA) {
        return Err(consistency_error(format!(
            "payload schema must be {SCHEMA}"
        )));
    }
    let mut tokens = BTreeSet::new();
    collect_tokens(payload_value["module_permits"].as_array(), &mut tokens)?;
    let Some(functions) = payload_value["functions"].as_array() else {
        return Err(consistency_error(
            "payload functions must be an array".to_owned(),
        ));
    };
    for function in functions {
        collect_tokens(function["effects"].as_array(), &mut tokens)?;
    }
    let Some(imports) = payload_value["imports"].as_array() else {
        return Err(consistency_error(
            "payload imports must be an array".to_owned(),
        ));
    };
    for import in imports {
        collect_tokens(import["effects"].as_array(), &mut tokens)?;
    }
    for token in &tokens {
        if !within_vocabulary(token) {
            return Err(consistency_error(format!(
                "capability `{token}` is outside the admitted bounded vocabulary"
            )));
        }
    }
    let expected_ambient: serde_json::Value =
        serde_json::from_str(&ambient_authority_json(&tokens))
            .expect("derived ambient authority is valid JSON");
    if payload_value["ambient_authority"] != expected_ambient {
        return Err(consistency_error(
            "embedded ambient authority disagrees with its derivation from the declared capabilities".to_owned(),
        ));
    }
    Ok(())
}

/// Verify one envelope and additionally bind the current bytes of
/// `source_path` to the embedded source digest, failing closed on drift.
pub fn verify_envelope_against_source(
    envelope: &str,
    source_path: &Path,
) -> Result<(), Diagnostic> {
    verify_envelope(envelope)?;
    let current = std::fs::read(source_path).map_err(|error| {
        consistency_error(format!("cannot read {}: {error}", source_path.display()))
    })?;
    let bound = bound_source_digest(envelope)?;
    if bound != domain_digest(SOURCE_DIGEST_DOMAIN, &current) {
        return Err(consistency_error(
            "capability manifest source digest does not match the current source bytes; \
             the source drifted after the manifest was generated"
                .to_owned(),
        ));
    }
    Ok(())
}

fn bound_source_digest(envelope: &str) -> Result<String, Diagnostic> {
    let value: serde_json::Value = serde_json::from_str(envelope)
        .map_err(|error| consistency_error(format!("envelope is not valid JSON: {error}")))?;
    let Some(digest) = value["payload"]["source"]["sha256"].as_str() else {
        return Err(consistency_error(
            "payload source sha256 must be a string".to_owned(),
        ));
    };
    Ok(digest.to_owned())
}

fn collect_tokens(
    values: Option<&Vec<serde_json::Value>>,
    tokens: &mut BTreeSet<String>,
) -> Result<(), Diagnostic> {
    let Some(values) = values else {
        return Err(consistency_error(
            "payload capability lists must be arrays of strings".to_owned(),
        ));
    };
    for value in values {
        let Some(token) = value.as_str() else {
            return Err(consistency_error(
                "payload capability lists must contain only strings".to_owned(),
            ));
        };
        tokens.insert(token.to_owned());
    }
    Ok(())
}

fn require_vocabulary(token: &str) -> Result<(), Vec<Diagnostic>> {
    if !within_vocabulary(token) {
        return Err(vec![vocabulary_error(token)]);
    }
    Ok(())
}

fn within_vocabulary(token: &str) -> bool {
    AMBIENT_DOMAINS.contains(&token)
        || AMBIENT_DOMAINS.iter().any(|domain| {
            token
                .strip_prefix(domain)
                .is_some_and(|rest| rest.starts_with('.'))
        })
}

fn names_domain(token: &str, domain: &str) -> bool {
    token == domain
        || token
            .strip_prefix(domain)
            .is_some_and(|rest| rest.starts_with('.'))
}

fn ambient_authority_json(tokens: &BTreeSet<String>) -> String {
    let members = AMBIENT_DOMAINS
        .iter()
        .map(|domain| {
            let state = if tokens.iter().any(|token| names_domain(token, domain)) {
                AMBIENT_DECLARED
            } else {
                AMBIENT_NONE
            };
            format!("\"{domain}\":\"{state}\"")
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{members}}}")
}

fn source_digest(source: &str) -> String {
    domain_digest(SOURCE_DIGEST_DOMAIN, source.as_bytes())
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

fn render(
    path_text: &str,
    revision: &str,
    digest: &str,
    input: &ManifestInput,
    max_bytes: usize,
) -> String {
    let function_entries = input
        .functions
        .iter()
        .map(|entry| {
            let effects = entry
                .effects
                .iter()
                .map(|effect| quote_json(effect))
                .collect::<Vec<_>>();
            bformat!(
                "{{\"stable_id\":{},\"name\":{},\"effects\":[{}]}}",
                quote_json(&entry.stable_id),
                quote_json(&entry.name),
                effects.budgeted_join(","),
            )
        })
        .collect::<Vec<_>>();
    let import_entries = input
        .imports
        .iter()
        .map(|entry| {
            let effects = entry
                .effects
                .iter()
                .map(|effect| quote_json(effect))
                .collect::<Vec<_>>();
            bformat!(
                "{{\"stable_id\":{},\"name\":{},\"interface\":{},\"effects\":[{}]}}",
                quote_json(&entry.stable_id),
                quote_json(&entry.name),
                quote_json(&entry.interface),
                effects.budgeted_join(","),
            )
        })
        .collect::<Vec<_>>();
    let permits = input
        .permits
        .iter()
        .map(|permit| quote_json(permit))
        .collect::<Vec<_>>();
    let mut tokens = BTreeSet::new();
    for permit in &input.permits {
        tokens.insert(permit.clone());
    }
    for entry in &input.functions {
        for effect in &entry.effects {
            tokens.insert(effect.clone());
        }
    }
    for entry in &input.imports {
        for effect in &entry.effects {
            tokens.insert(effect.clone());
        }
    }
    let ambient =
        crate::bounded_output::budgeted_format(format_args!("{}", ambient_authority_json(&tokens)));

    let payload = bformat!(
        "{{\"schema\":\"{}\",\"source\":{{\"path\":{},\"revision\":{},\"sha256\":{}}},\
\"limits\":{{\"max_bytes\":{}}},\
\"module\":{{\"name\":{},\"permits_total\":{},\"functions_total\":{},\"imports_total\":{}}},\
\"module_permits\":[{}],\
\"functions\":[{}],\"imports\":[{}],\
\"ambient_authority\":{},\"nonclaims\":[{}]}}",
        SCHEMA,
        quote_json(path_text),
        quote_json(revision),
        quote_json(digest),
        max_bytes,
        quote_json(&input.module_name),
        input.permits.len(),
        input.functions.len(),
        input.imports.len(),
        permits.budgeted_join(","),
        function_entries.budgeted_join(","),
        import_entries.budgeted_join(","),
        ambient,
        NONCLAIMS_JSON,
    );
    bformat!(
        "{{\"schema\":\"{}\",\"digest\":{},\"bytes\":{},\"payload\":{}}}",
        SCHEMA,
        quote_json(&domain_digest(PAYLOAD_DIGEST_DOMAIN, payload.as_bytes())),
        payload.len(),
        payload,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_reject_out_of_bounds_values() {
        assert!(CapabilityManifestOptions::new(512).is_err());
        assert!(CapabilityManifestOptions::new(graph::MAX_AGENT_CONTEXT_BYTES + 1).is_err());
        assert!(CapabilityManifestOptions::new(graph::MIN_AGENT_CONTEXT_BYTES).is_ok());
        assert_eq!(
            CapabilityManifestOptions::default().max_bytes,
            DEFAULT_MAX_BYTES
        );
    }

    #[test]
    fn vocabulary_membership_is_exact_and_prefix_scoped() {
        assert!(within_vocabulary("network"));
        assert!(within_vocabulary("network.read"));
        assert!(within_vocabulary("filesystem.write"));
        assert!(!within_vocabulary("audit.write"));
        assert!(!within_vocabulary("networking.read"));
        assert!(!within_vocabulary("networkx"));
        assert!(!within_vocabulary(""));
    }

    #[test]
    fn ambient_section_defaults_to_all_none_and_tracks_declarations() {
        let empty = ambient_authority_json(&BTreeSet::new());
        assert_eq!(
            empty,
            "{\"filesystem\":\"none\",\"home\":\"none\",\"network\":\"none\",\
\"process\":\"none\",\"secrets\":\"none\"}"
        );
        let mut tokens = BTreeSet::new();
        tokens.insert("network.read".to_owned());
        tokens.insert("process".to_owned());
        let declared = ambient_authority_json(&tokens);
        assert!(declared.contains("\"network\":\"declared\""));
        assert!(declared.contains("\"process\":\"declared\""));
        assert!(declared.contains("\"home\":\"none\""));
    }

    #[test]
    fn domain_digest_is_domain_separated() {
        let first = domain_digest(SOURCE_DIGEST_DOMAIN, b"abc");
        let second = domain_digest(PAYLOAD_DIGEST_DOMAIN, b"abc");
        assert_ne!(first, second);
        assert_eq!(first, domain_digest(SOURCE_DIGEST_DOMAIN, b"abc"));
    }
}
