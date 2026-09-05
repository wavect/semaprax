//! Version-matched, authority-free inventory and explanation of diagnostic
//! identifiers statically present in the installed compiler sources.

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;

pub const INSTALLED_DIAGNOSTIC_CATALOG_SCHEMA: &str = "semaprax.installed-diagnostic-catalog.v1";
pub const INSTALLED_DIAGNOSTIC_EXPLANATION_SCHEMA: &str =
    "semaprax.installed-diagnostic-explanation.v1";
pub const MAX_INSTALLED_DIAGNOSTIC_CATALOG_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_INSTALLED_DIAGNOSTIC_EXPLANATION_BYTES: usize = 1024 * 1024;

const CATALOG_DOMAIN: &[u8] = b"semaprax.installed-diagnostic-catalog.payload.digest.v1\0";
const EXPLANATION_DOMAIN: &[u8] = b"semaprax.installed-diagnostic-explanation.payload.digest.v1\0";

const GENERATED_CODES: &[(&str, &[(&str, u32)])] =
    include!(concat!(env!("OUT_DIR"), "/installed_diagnostic_codes.rs"));
const GENERATED_DYNAMIC_SITES: &[(&str, u32)] = include!(concat!(
    env!("OUT_DIR"),
    "/installed_dynamic_diagnostic_sites.rs"
));

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstalledDiagnosticCatalog {
    json: String,
    digest: String,
    code_count: usize,
}

impl InstalledDiagnosticCatalog {
    pub fn to_json(&self) -> &str {
        &self.json
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn code_count(&self) -> usize {
        self.code_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstalledDiagnosticExplanation {
    code: &'static str,
    text: String,
    json: String,
    digest: String,
}

impl InstalledDiagnosticExplanation {
    pub fn code(&self) -> &'static str {
        self.code
    }
    /// Concise compiler-owned human projection, terminated by one LF.
    pub fn to_text(&self) -> &str {
        &self.text
    }
    pub fn to_json(&self) -> &str {
        &self.json
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }

    pub fn replay(code: &str, expected_digest: &str, bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_INSTALLED_DIAGNOSTIC_EXPLANATION_BYTES {
            return Err(capacity(
                "installed diagnostic explanation exceeds its byte limit",
            ));
        }
        validate_code(code)?;
        validate_digest(expected_digest)?;
        let value: Value = serde_json::from_slice(bytes)
            .map_err(|_| invalid("installed diagnostic explanation is not valid JSON"))?;
        if value["schema"] != INSTALLED_DIAGNOSTIC_EXPLANATION_SCHEMA
            || canonical(&value)?.as_bytes() != bytes
        {
            return Err(invalid(
                "installed diagnostic explanation is not the canonical schema",
            ));
        }
        if hash(EXPLANATION_DOMAIN, payload_bytes(&value)?.as_bytes()) != expected_digest {
            return Err(stale("installed diagnostic explanation digest is stale"));
        }
        let derived = explain_installed_diagnostic(code)?;
        if derived.digest() != expected_digest || derived.to_json().as_bytes() != bytes {
            return Err(stale(
                "installed diagnostic explanation failed exact replay",
            ));
        }
        Ok(derived)
    }
}

/// Return the complete statically observed code-token inventory generated from
/// this exact package's `src` and workspace-member `crates` Rust sources.
pub fn installed_diagnostic_catalog() -> Result<InstalledDiagnosticCatalog> {
    let entries = GENERATED_CODES
        .iter()
        .map(|(code, occurrences)| entry(code, occurrences))
        .collect::<Vec<_>>();
    let dynamic_sites = GENERATED_DYNAMIC_SITES
        .iter()
        .map(|(path, line)| json!({"line":line,"path":path,"scope":scope(path)}))
        .collect::<Vec<_>>();
    let payload = json!({
        "authority": false,
        "compiler": compiler()?,
        "coverage": {
            "classification": "complete_static_code_token_inventory_with_unresolved_dynamic_constructor_sites",
            "dynamic_constructor_sites": dynamic_sites,
            "dynamic_constructor_site_count": GENERATED_DYNAMIC_SITES.len(),
            "static_code_count": GENERATED_CODES.len(),
            "source_roots": ["crates", "src"],
        },
        "diagnostics": entries,
        "limits": {"max_document_bytes":MAX_INSTALLED_DIAGNOSTIC_CATALOG_BYTES},
        "nonclaims": [
            "static_source_tokens_are_not_runtime_reachability_or_backend_support",
            "dynamic_code_selection_may_use_a_statically_cataloged_identifier",
            "messages_help_severity_and_locations_are_emission_site_specific",
            "source_lines_are_build_source_provenance_not_installed_source_availability",
            "no_filesystem_process_network_execution_or_publication_authority",
            "no_host_capability_grant",
        ],
    });
    let (json, digest) = document(
        INSTALLED_DIAGNOSTIC_CATALOG_SCHEMA,
        CATALOG_DOMAIN,
        payload,
        MAX_INSTALLED_DIAGNOSTIC_CATALOG_BYTES,
    )?;
    Ok(InstalledDiagnosticCatalog {
        json,
        digest,
        code_count: GENERATED_CODES.len(),
    })
}

/// Explain one exact installed code using the same generated inventory. The
/// explanation describes identity and provenance; runtime wording remains on
/// the emitted `Diagnostic` and is not guessed from source text.
pub fn explain_installed_diagnostic(code: &str) -> Result<InstalledDiagnosticExplanation> {
    validate_code(code)?;
    let index = GENERATED_CODES
        .binary_search_by_key(&code, |(candidate, _)| *candidate)
        .map_err(|_| unknown("diagnostic code is not in the installed static catalog"))?;
    let (installed, occurrences) = GENERATED_CODES[index];
    let occurrence_count = occurrences.len();
    let text = format!(
        "{installed}: installed {} diagnostic ({occurrence_count} static source occurrence{}); emitted message and help are site-specific.\n",
        namespace(installed),
        if occurrence_count == 1 { "" } else { "s" },
    );
    let payload = json!({
        "authority": false,
        "code": installed,
        "compiler": compiler()?,
        "concise": text,
        "explanation": {
            "classification": "installed_static_diagnostic_identifier",
            "message_contract": "emission_site_specific; inspect the emitted message and optional help",
            "namespace": namespace(installed),
            "occurrences": occurrences.iter().map(|(path,line)|json!({
                "line":line,"path":path,"scope":scope(path)
            })).collect::<Vec<_>>(),
        },
        "limits": {"max_document_bytes":MAX_INSTALLED_DIAGNOSTIC_EXPLANATION_BYTES},
        "nonclaims": [
            "not_a_claim_that_every_static_occurrence_is_runtime_reachable",
            "not_a_repair_or_success_guarantee",
            "not_a_stable_cross_version_code_registry",
            "no_filesystem_process_network_execution_or_publication_authority",
            "no_host_capability_grant",
        ],
    });
    let (json, digest) = document(
        INSTALLED_DIAGNOSTIC_EXPLANATION_SCHEMA,
        EXPLANATION_DOMAIN,
        payload,
        MAX_INSTALLED_DIAGNOSTIC_EXPLANATION_BYTES,
    )?;
    Ok(InstalledDiagnosticExplanation {
        code: installed,
        text,
        json,
        digest,
    })
}

fn entry(code: &str, occurrences: &[(&str, u32)]) -> Value {
    json!({
        "code": code,
        "namespace": namespace(code),
        "occurrences": occurrences.iter().map(|(path,line)|json!({
            "line":line,"path":path,"scope":scope(path)
        })).collect::<Vec<_>>(),
    })
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
            "installed diagnostic build commit is not 40 lowercase hexadecimal characters",
        )),
    }
}

fn scope(path: &str) -> &'static str {
    if path.starts_with("src/") {
        "compiler_package"
    } else {
        "workspace_member_source"
    }
}

fn namespace(code: &str) -> &str {
    let body = code.strip_prefix("SPX-").unwrap_or_default();
    body.trim_end_matches(|character: char| character.is_ascii_digit())
}

fn validate_code(code: &str) -> Result<()> {
    let Some(body) = code.strip_prefix("SPX-") else {
        return Err(invalid("diagnostic code is not canonical"));
    };
    let bytes = body.as_bytes();
    if bytes.len() < 4
        || bytes.len() > 16
        || !bytes[..bytes.len() - 3].iter().all(u8::is_ascii_uppercase)
        || !bytes[bytes.len() - 3..].iter().all(u8::is_ascii_digit)
    {
        return Err(invalid("diagnostic code is not canonical"));
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
        return Err(invalid("installed diagnostic digest is not canonical"));
    }
    Ok(())
}

fn document(
    schema: &'static str,
    domain: &[u8],
    payload: Value,
    limit: usize,
) -> Result<(String, String)> {
    let payload = canonical(&payload)?;
    let digest = hash(domain, payload.as_bytes());
    let payload_value: Value = serde_json::from_str(&payload)
        .map_err(|_| invalid("installed diagnostic payload is invalid JSON"))?;
    let json = canonical(&json!({"digest":digest,"payload":payload_value,"schema":schema}))?;
    if json.len() > limit {
        return Err(capacity(
            "installed diagnostic document exceeds its byte limit",
        ));
    }
    Ok((json, digest))
}

fn payload_bytes(document: &Value) -> Result<String> {
    document
        .get("payload")
        .ok_or_else(|| invalid("installed diagnostic document lacks a payload"))
        .and_then(canonical)
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
        .map_err(|_| invalid("installed diagnostic document cannot be serialized"))?;
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
    vec![Diagnostic::io("SPX-G540", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G541", message)]
}
fn unknown(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G542", message)]
}
fn stale(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G543", message)]
}
