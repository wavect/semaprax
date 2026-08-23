//! Deterministic, read-only Protocol Projection v1.
//!
//! [`generate`] projects one verified single-file SEMAPRAX module into one
//! canonical compact JSON envelope (`semaprax.protocol.v1`): an inventory of
//! every `protocol` declaration and its body-less method signatures with
//! stable identities, ownership modes, and rendered parameter/result types,
//! sorted bytewise by stable identity. Every method signature is validated to
//! resolve against the closed v1 rules: a required receiver typed `Self` or
//! the protocol's own name, primitive or module-declared named types
//! everywhere else, and no generic arguments. Conformance is reported as an
//! explicitly empty listing under a closed reason vocabulary — the language
//! admits no `impl` declarations yet, so nothing conforms to anything.
//!
//! [`verify_envelope`] independently replays one envelope: exact envelope and
//! payload shape, declared byte count, domain-separated payload digest,
//! counts, both closed vocabularies, the fixed signature-rule table, the
//! explicitly empty conformance section, canonical ordering,
//! identity-origin consistency, and parameter shape, so forged-but-re-signed
//! mutations still fail closed.
//!
//! Diagnostics use the previously unused `SPX-Q1xx` family:
//! - `SPX-Q101`: invalid options (bounds, malformed values).
//! - `SPX-Q102`: output byte-budget exhaustion (fail-closed, no truncation).
//! - `SPX-Q103`: envelope or projection consistency failure.
//! - `SPX-Q104`: protocol signature does not resolve under the closed rules.
//! - `SPX-Q105`: stable-id collision involving a protocol or protocol method.
//!
//! Parser-level structural gates live in the parser (`SPX-P120`-`SPX-P123`):
//! duplicate protocol names, duplicate method names, duplicate protocol
//! identities, and empty protocols are rejected fail-closed at parse time.
//! There is no `impl` syntax in v1, so implements cycles cannot be expressed;
//! the parser therefore cannot admit one and the conformance listing stays
//! structurally empty.
//!
//! This tranche emits no dispatch code, lowers nothing to any backend,
//! executes no target, and changes no source.

use std::path::Path;

use sha2::{Digest as _, Sha256};

use crate::ast::{Program, ProtocolDeclaration, ProtocolMethod, Type, TypeDeclarationKind};
use crate::bounded_output::{with_limit, BudgetedJoin as _};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::hir::IdentityOrigin;
use crate::{graph, parse, patch, verify};

macro_rules! bformat {
    ($($argument:tt)*) => {
        crate::bounded_output::budgeted_format(format_args!($($argument)*))
    };
}

pub const SCHEMA: &str = "semaprax.protocol.v1";

const DEFAULT_MAX_BYTES: usize = 64 * 1024;

const SOURCE_DIGEST_DOMAIN: &[u8] = b"semaprax.protocol.source.v1\0";
const PAYLOAD_DIGEST_DOMAIN: &[u8] = b"semaprax.protocol.payload.v1\0";

/// Closed v1 signature-resolution rule table, in canonical bytewise order by
/// rule name. Every method of every protocol is validated against exactly
/// these rules; nothing else about signatures is checked or assumed.
const SIGNATURE_RULES_JSON: &str = "[\
{\"rule\":\"generic_arguments_closed\"},\
{\"rule\":\"named_types_must_resolve_in_module\"},\
{\"rule\":\"receiver_required\"},\
{\"rule\":\"receiver_type_self_or_protocol_name\"},\
{\"rule\":\"self_only_in_receiver_position\"}]";

/// Closed conformance reason vocabulary, in canonical bytewise order. v1
/// admits no `impl` declarations, so every projection reports the same single
/// closed reason and an explicitly empty conformance listing.
const REASON_NO_IMPL_DECLARATIONS_IN_V1: &str = "no_impl_declarations_in_v1";
const CONFORMANCE_REASONS: [&str; 1] = [REASON_NO_IMPL_DECLARATIONS_IN_V1];

/// Closed inventory of capabilities this projection explicitly does not
/// provide, in canonical bytewise order.
#[allow(dead_code, reason = "test-side twin of the rendered JSON constant")]
const UNAVAILABLE_CAPABILITIES: [&str; 4] = [
    "class_carriers",
    "dispatch_codegen",
    "impl_declarations",
    "runtime_witness_tables",
];
const UNAVAILABLE_CAPABILITIES_JSON: &str = "[\"class_carriers\",\
\"dispatch_codegen\",\
\"impl_declarations\",\
\"runtime_witness_tables\"]";

/// The fixed honest-boundary statement, in canonical bytewise order.
const NONCLAIMS_JSON: &str = "[\"no_backend_or_codegen_changes\",\
\"no_conformance_admission\",\
\"no_dispatch_lowering\",\
\"no_target_execution\",\
\"read_only_no_source_changes\",\
\"static_signature_projection_only\"]";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtocolCheckOptions {
    pub max_bytes: usize,
}

impl ProtocolCheckOptions {
    pub fn new(max_bytes: usize) -> Result<Self, Diagnostic> {
        if !(graph::MIN_AGENT_CONTEXT_BYTES..=graph::MAX_AGENT_CONTEXT_BYTES).contains(&max_bytes) {
            return Err(option_error(format!(
                "protocol-check max_bytes must be between {} and {}",
                graph::MIN_AGENT_CONTEXT_BYTES,
                graph::MAX_AGENT_CONTEXT_BYTES
            )));
        }
        Ok(Self { max_bytes })
    }
}

impl Default for ProtocolCheckOptions {
    fn default() -> Self {
        Self {
            max_bytes: DEFAULT_MAX_BYTES,
        }
    }
}

fn option_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-Q101", message)
}

fn consistency_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-Q103", message)
}

fn signature_error(protocol: &str, method: &str, message: String) -> Diagnostic {
    Diagnostic::io(
        "SPX-Q104",
        format!("protocol `{protocol}` method `{method}`: {message}"),
    )
}

fn collision_error(stable_id: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-Q105",
        format!(
            "protocol identity `{stable_id}` collides with another declaration identity"
        ),
    )
}

struct MethodEntry {
    stable_id: String,
    name: String,
    origin: IdentityOrigin,
    params: Vec<ParamEntry>,
    return_type: String,
}

struct ParamEntry {
    name: String,
    mode: &'static str,
    ty: String,
}

struct ProtocolEntry {
    stable_id: String,
    name: String,
    origin: IdentityOrigin,
    methods: Vec<MethodEntry>,
}

/// One independently replayed protocol returned by [`verify_envelope`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedMethod {
    pub stable_id: String,
    pub name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedProtocol {
    pub stable_id: String,
    pub name: String,
    pub methods: Vec<VerifiedMethod>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VerifiedProtocols {
    pub protocols: Vec<VerifiedProtocol>,
}

/// Generate the canonical `semaprax.protocol.v1` envelope JSON for one
/// verified source file.
///
/// Read-only: source bytes must remain unchanged between the snapshot and the
/// final check or generation fails closed.
pub fn generate(
    source_path: &Path,
    options: &ProtocolCheckOptions,
) -> Result<String, Vec<Diagnostic>> {
    let canonical_source_path = patch::canonical_source_path(source_path)?;
    let snapshot = patch::read_source_snapshot(&canonical_source_path)?;
    let program = parse(snapshot.source(), source_path).map_err(|error| vec![error])?;
    let diagnostics = verify::verify(&program);
    if diagnostics.iter().any(|item| item.severity.is_error()) {
        return Err(diagnostics);
    }
    validate_program(&program).map_err(|diagnostic| vec![diagnostic])?;
    let revision = graph::revision(&program);

    let mut sorted = program.protocols.iter().collect::<Vec<_>>();
    sorted.sort_by(|left, right| left.stable_id.as_bytes().cmp(right.stable_id.as_bytes()));
    let mut entries = Vec::with_capacity(sorted.len());
    let mut methods_total = 0usize;
    for protocol in sorted {
        let mut methods = protocol.methods.iter().collect::<Vec<_>>();
        methods.sort_by(|left, right| left.stable_id.as_bytes().cmp(right.stable_id.as_bytes()));
        methods_total += methods.len();
        let mut method_entries = Vec::with_capacity(methods.len());
        for method in methods {
            method_entries.push(MethodEntry {
                stable_id: method.stable_id.clone(),
                name: method.name.clone(),
                origin: method_identity_origin(method),
                params: method
                    .params
                    .iter()
                    .map(|param| ParamEntry {
                        name: param.name.clone(),
                        mode: param.mode.text(),
                        ty: param.ty.to_string(),
                    })
                    .collect(),
                return_type: method.return_type.to_string(),
            });
        }
        entries.push(ProtocolEntry {
            stable_id: protocol.stable_id.clone(),
            name: protocol.name.clone(),
            origin: protocol_identity_origin(protocol),
            methods: method_entries,
        });
    }

    // Records stand in as the future class receivers; counting them keeps the
    // empty conformance listing honest about what it considered.
    let candidates_considered = program
        .types
        .iter()
        .filter(|declaration| matches!(declaration.kind, TypeDeclarationKind::Record { .. }))
        .count();

    let digest = source_digest(snapshot.source());
    let path_text = source_path.display().to_string();
    let (envelope, overflowed) = with_limit(options.max_bytes, || {
        render(
            &path_text,
            &revision,
            &digest,
            &program.module,
            options.max_bytes,
            entries.len(),
            methods_total,
            candidates_considered,
            &entries,
        )
    });
    if overflowed {
        return Err(vec![Diagnostic::io(
            "SPX-Q102",
            "protocol-check output exceeds the max-bytes budget; refusing to truncate".to_owned(),
        )]);
    }
    patch::validate_source_unchanged(&canonical_source_path, source_path, &snapshot, &revision)?;
    Ok(envelope)
}

/// Validate every protocol signature against the closed v1 resolution rules
/// and reject any stable-id collision involving a protocol or method.
///
/// Deterministic and fail-closed; used by the CLI projection and by semantic
/// Graph emission before protocol nodes may appear.
pub fn validate_program(program: &Program) -> Result<(), Diagnostic> {
    if program.protocols.is_empty() {
        return Ok(());
    }
    // Existing declaration identities are collected without duplicate
    // reporting (the verifier already owns those diagnostics); only
    // collisions introduced from the protocol side fail here.
    let mut ids = std::collections::BTreeSet::new();
    for function in &program.functions {
        ids.insert(function.stable_id.as_str());
    }
    for declaration in &program.types {
        ids.insert(declaration.stable_id.as_str());
        match &declaration.kind {
            TypeDeclarationKind::Variant { cases } => {
                for case in cases {
                    ids.insert(case.stable_id.as_str());
                    for field in &case.fields {
                        ids.insert(field.stable_id.as_str());
                    }
                }
            }
            TypeDeclarationKind::Record { fields } | TypeDeclarationKind::Class { fields, .. } => {
                for field in fields {
                    ids.insert(field.stable_id.as_str());
                }
            }
            TypeDeclarationKind::Resource { .. } => {}
        }
    }
    for interface in &program.interfaces {
        ids.insert(interface.stable_id.as_str());
        for import in &interface.imports {
            ids.insert(import.stable_id.as_str());
        }
    }
    let declared_types = program
        .types
        .iter()
        .map(|declaration| declaration.name.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    for protocol in &program.protocols {
        if !ids.insert(protocol.stable_id.as_str()) {
            return Err(collision_error(&protocol.stable_id));
        }
        for method in &protocol.methods {
            if !ids.insert(method.stable_id.as_str()) {
                return Err(collision_error(&method.stable_id));
            }
            check_method(protocol, method, &declared_types)?;
        }
    }
    Ok(())
}

fn check_method(
    protocol: &ProtocolDeclaration,
    method: &ProtocolMethod,
    declared_types: &std::collections::BTreeSet<&str>,
) -> Result<(), Diagnostic> {
    let Some(receiver) = method.params.first() else {
        return Err(signature_error(
            &protocol.name,
            &method.name,
            "must declare a receiver parameter".to_owned(),
        ));
    };
    if !is_self(&receiver.ty) && !is_protocol_name(&receiver.ty, &protocol.name) {
        return Err(signature_error(
            &protocol.name,
            &method.name,
            format!(
                "receiver must be typed `Self` or `{}`, found `{}`",
                protocol.name, receiver.ty
            ),
        ));
    }
    for param in method.params.iter().skip(1) {
        if is_self(&param.ty) {
            return Err(signature_error(
                &protocol.name,
                &method.name,
                format!(
                    "`Self` is only allowed in the receiver position, found on `{}`",
                    param.name
                ),
            ));
        }
        check_type(protocol, method, &param.ty, declared_types)?;
    }
    if is_self(&method.return_type) {
        return Err(signature_error(
            &protocol.name,
            &method.name,
            "`Self` is only allowed in the receiver position, found as the return type".to_owned(),
        ));
    }
    check_type(protocol, method, &method.return_type, declared_types)
}

fn check_type(
    protocol: &ProtocolDeclaration,
    method: &ProtocolMethod,
    ty: &Type,
    declared_types: &std::collections::BTreeSet<&str>,
) -> Result<(), Diagnostic> {
    match ty {
        Type::I64 | Type::I32 | Type::Char | Type::U8 | Type::F32 | Type::F64 | Type::Bool => {
            Ok(())
        }
        Type::String => Err(signature_error(
            &protocol.name,
            &method.name,
            "`string` parameters and results are outside Protocol Projection v1".to_owned(),
        )),
        Type::Named { name, arguments } => {
            if !arguments.is_empty() {
                return Err(signature_error(
                    &protocol.name,
                    &method.name,
                    format!("generic arguments on `{name}` are outside Protocol Projection v1"),
                ));
            }
            if declared_types.contains(name.as_str()) || name == &protocol.name {
                Ok(())
            } else {
                Err(signature_error(
                    &protocol.name,
                    &method.name,
                    format!(
                        "unknown type `{name}`; signatures must reference primitives or types declared in this module"
                    ),
                ))
            }
        }
    }
}

fn is_self(ty: &Type) -> bool {
    matches!(ty, Type::Named { name, arguments } if arguments.is_empty() && name == "Self")
}

fn is_protocol_name(ty: &Type, protocol_name: &str) -> bool {
    matches!(ty, Type::Named { name, arguments } if arguments.is_empty() && name == protocol_name)
}

fn protocol_identity_origin(protocol: &ProtocolDeclaration) -> IdentityOrigin {
    if protocol.explicit_id {
        IdentityOrigin::Explicit
    } else {
        IdentityOrigin::Automatic
    }
}

fn method_identity_origin(method: &ProtocolMethod) -> IdentityOrigin {
    if method.explicit_id {
        IdentityOrigin::Explicit
    } else {
        IdentityOrigin::Automatic
    }
}

fn source_digest(source: &str) -> String {
    domain_digest(SOURCE_DIGEST_DOMAIN, source.as_bytes())
}

pub(crate) fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

#[allow(clippy::too_many_arguments)]
fn render(
    path_text: &str,
    revision: &str,
    digest: &str,
    module_name: &str,
    max_bytes: usize,
    protocols_total: usize,
    methods_total: usize,
    candidates_considered: usize,
    protocols: &[ProtocolEntry],
) -> String {
    let protocol_entries = protocols
        .iter()
        .map(|entry| {
            let methods = entry
                .methods
                .iter()
                .map(|method| {
                    let params = method
                        .params
                        .iter()
                        .map(|param| {
                            bformat!(
                                "{{\"mode\":{},\"name\":{},\"type\":{}}}",
                                quote_json(param.mode),
                                quote_json(&param.name),
                                quote_json(&param.ty),
                            )
                        })
                        .collect::<Vec<_>>();
                    bformat!(
                        "{{\"stable_id\":{},\"name\":{},\"identity_origin\":{},\"persistent\":{},\
\"params\":[{}],\"return_type\":{}}}",
                        quote_json(&method.stable_id),
                        quote_json(&method.name),
                        quote_json(method.origin.text()),
                        method.origin.is_persistent(),
                        params.budgeted_join(","),
                        quote_json(&method.return_type),
                    )
                })
                .collect::<Vec<_>>();
            bformat!(
                "{{\"stable_id\":{},\"name\":{},\"identity_origin\":{},\"persistent\":{},\"methods\":[{}]}}",
                quote_json(&entry.stable_id),
                quote_json(&entry.name),
                quote_json(entry.origin.text()),
                entry.origin.is_persistent(),
                methods.budgeted_join(","),
            )
        })
        .collect::<Vec<_>>();

    let payload = bformat!(
        "{{\"schema\":\"{}\",\"source\":{{\"path\":{},\"revision\":{},\"sha256\":{}}},\
\"limits\":{{\"max_bytes\":{}}},\
\"module\":{},\"protocols_total\":{},\"methods_total\":{},\
\"signature_rules\":{},\
\"protocols\":[{}],\
\"conformance\":{{\"admitted\":0,\"candidates_considered\":{},\"closed_reason\":\"{}\"}},\
\"conformances\":[],\
\"unavailable_capabilities\":{},\"nonclaims\":{}}}",
        SCHEMA,
        quote_json(path_text),
        quote_json(revision),
        quote_json(digest),
        max_bytes,
        quote_json(module_name),
        protocols_total,
        methods_total,
        SIGNATURE_RULES_JSON,
        protocol_entries.budgeted_join(","),
        candidates_considered,
        REASON_NO_IMPL_DECLARATIONS_IN_V1,
        UNAVAILABLE_CAPABILITIES_JSON,
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

/// Independently verify one envelope produced by [`generate`].
///
/// Recomputes the outer payload digest over the exact serialized payload
/// bytes, re-checks the declared byte count and payload key order, replays
/// counts, both closed vocabularies, the fixed signature-rule table, the
/// explicitly empty conformance section, canonical ordering, identity-origin
/// consistency, and parameter shape before returning the protocol summaries.
pub fn verify_envelope(envelope: &str) -> Result<VerifiedProtocols, Diagnostic> {
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
        return Err(consistency_error(format!("envelope schema must be {SCHEMA}")));
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
        return Err(consistency_error(format!("payload schema must be {SCHEMA}")));
    }
    const PAYLOAD_KEYS: [&str; 12] = [
        "conformance",
        "conformances",
        "limits",
        "methods_total",
        "module",
        "nonclaims",
        "protocols",
        "protocols_total",
        "schema",
        "signature_rules",
        "source",
        "unavailable_capabilities",
    ];
    let payload_keys: Vec<&str> = payload_value
        .as_object()
        .map(|object| object.keys().map(String::as_str).collect())
        .unwrap_or_default();
    if payload_keys != PAYLOAD_KEYS {
        return Err(consistency_error(format!(
            "payload keys must be exactly {PAYLOAD_KEYS:?}, found {payload_keys:?}"
        )));
    }

    // Fixed closed sections.
    let signature_rules: serde_json::Value =
        serde_json::from_str(SIGNATURE_RULES_JSON).expect("signature rule constant is valid JSON");
    if payload_value["signature_rules"] != signature_rules {
        return Err(consistency_error(
            "signature_rules must be exactly the closed v1 rule table".to_owned(),
        ));
    }
    let nonclaims: serde_json::Value =
        serde_json::from_str(NONCLAIMS_JSON).expect("nonclaims constant is valid JSON");
    if payload_value["nonclaims"] != nonclaims {
        return Err(consistency_error(
            "nonclaims must be exactly the fixed honest-boundary statement".to_owned(),
        ));
    }
    let unavailable: serde_json::Value =
        serde_json::from_str(UNAVAILABLE_CAPABILITIES_JSON).expect("unavailable constant");
    if payload_value["unavailable_capabilities"] != unavailable {
        return Err(consistency_error(
            "unavailable_capabilities must be exactly the closed canonical inventory".to_owned(),
        ));
    }
    let conformances = payload_value["conformances"]
        .as_array()
        .ok_or_else(|| consistency_error("payload conformances must be an array".to_owned()))?;
    if !conformances.is_empty() {
        return Err(consistency_error(
            "v1 conformances must be explicitly empty".to_owned(),
        ));
    }

    // Conformance section replays against its closed vocabulary.
    let Some(conformance) = payload_value["conformance"].as_object() else {
        return Err(consistency_error(
            "payload conformance must be an object".to_owned(),
        ));
    };
    if conformance["admitted"].as_u64() != Some(0) {
        return Err(consistency_error(
            "conformance admitted must be exactly zero in v1".to_owned(),
        ));
    }
    let Some(reason) = conformance["closed_reason"].as_str() else {
        return Err(consistency_error(
            "conformance closed_reason must be a string".to_owned(),
        ));
    };
    if !CONFORMANCE_REASONS.contains(&reason) {
        return Err(consistency_error(format!(
            "conformance closed_reason `{reason}` is outside the closed vocabulary"
        )));
    }
    if conformance["candidates_considered"].as_u64().is_none() {
        return Err(consistency_error(
            "conformance candidates_considered must be an unsigned integer".to_owned(),
        ));
    }

    // Counts agree with the listings.
    let protocols = payload_value["protocols"]
        .as_array()
        .ok_or_else(|| consistency_error("payload protocols must be an array".to_owned()))?;
    let protocols_total = payload_value["protocols_total"].as_u64().ok_or_else(|| {
        consistency_error("payload protocols_total must be an unsigned integer".to_owned())
    })?;
    let methods_total = payload_value["methods_total"].as_u64().ok_or_else(|| {
        consistency_error("payload methods_total must be an unsigned integer".to_owned())
    })?;
    if protocols.len() as u64 != protocols_total {
        return Err(consistency_error(
            "protocols_total disagrees with the listed protocols".to_owned(),
        ));
    }
    let listed_method_counts = protocols
        .iter()
        .map(|protocol| {
            protocol["methods"]
                .as_array()
                .map(Vec::len)
                .ok_or_else(|| consistency_error("protocol methods must be an array".to_owned()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if listed_method_counts.iter().sum::<usize>() as u64 != methods_total {
        return Err(consistency_error(
            "methods_total disagrees with the listed methods".to_owned(),
        ));
    }

    // Canonical ordering, identity-origin consistency, parameter shape.
    let mut verified = Vec::<VerifiedProtocol>::with_capacity(protocols.len());
    let mut previous_protocol: Option<&str> = None;
    for protocol in protocols {
        let Some(stable_id) = protocol["stable_id"].as_str() else {
            return Err(consistency_error(
                "protocol stable_id must be a string".to_owned(),
            ));
        };
        if let Some(previous) = previous_protocol {
            if previous.as_bytes() >= stable_id.as_bytes() {
                return Err(consistency_error(format!(
                    "protocol `{stable_id}` breaks the strict stable-id ordering"
                )));
            }
        }
        previous_protocol = Some(stable_id);
        let Some(name) = protocol["name"].as_str() else {
            return Err(consistency_error("protocol name must be a string".to_owned()));
        };
        check_identity(protocol, stable_id)?;
        let methods = protocol["methods"]
            .as_array()
            .ok_or_else(|| consistency_error("protocol methods must be an array".to_owned()))?;
        let mut verified_methods = Vec::<VerifiedMethod>::with_capacity(methods.len());
        let mut previous_method: Option<&str> = None;
        for method in methods {
            let Some(method_id) = method["stable_id"].as_str() else {
                return Err(consistency_error(
                    "method stable_id must be a string".to_owned(),
                ));
            };
            if let Some(previous) = previous_method {
                if previous.as_bytes() >= method_id.as_bytes() {
                    return Err(consistency_error(format!(
                        "method `{method_id}` breaks the strict stable-id ordering"
                    )));
                }
            }
            previous_method = Some(method_id);
            let Some(method_name) = method["name"].as_str() else {
                return Err(consistency_error("method name must be a string".to_owned()));
            };
            check_identity(method, method_id)?;
            let params = method["params"]
                .as_array()
                .ok_or_else(|| consistency_error("method params must be an array".to_owned()))?;
            for param in params {
                let mode = param["mode"].as_str().ok_or_else(|| {
                    consistency_error("parameter mode must be a string".to_owned())
                })?;
                if !matches!(mode, "value" | "own" | "borrow" | "shared") {
                    return Err(consistency_error(format!(
                        "parameter mode `{mode}` is outside the closed ownership vocabulary"
                    )));
                }
                if param["name"].as_str().is_none() || param["type"].as_str().is_none() {
                    return Err(consistency_error(
                        "parameter name and type must be strings".to_owned(),
                    ));
                }
            }
            if method["return_type"].as_str().is_none() {
                return Err(consistency_error(
                    "method return_type must be a string".to_owned(),
                ));
            }
            verified_methods.push(VerifiedMethod {
                stable_id: method_id.to_owned(),
                name: method_name.to_owned(),
            });
        }
        verified.push(VerifiedProtocol {
            stable_id: stable_id.to_owned(),
            name: name.to_owned(),
            methods: verified_methods,
        });
    }
    Ok(VerifiedProtocols { protocols: verified })
}

fn check_identity(entry: &serde_json::Value, stable_id: &str) -> Result<(), Diagnostic> {
    let origin = entry["identity_origin"].as_str().ok_or_else(|| {
        consistency_error(format!("identity of `{stable_id}` must be a string"))
    })?;
    let persistent = entry["persistent"].as_bool().ok_or_else(|| {
        consistency_error(format!("persistence of `{stable_id}` must be a boolean"))
    })?;
    let expected_persistent = match origin {
        "explicit" => true,
        "automatic" => false,
        _ => {
            return Err(consistency_error(format!(
                "identity origin `{origin}` of `{stable_id}` is outside the closed vocabulary"
            )))
        }
    };
    if persistent != expected_persistent {
        return Err(consistency_error(format!(
            "persistence flag of `{stable_id}` contradicts its identity origin"
        )));
    }
    Ok(())
}
