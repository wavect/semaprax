use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::ast::Span;
use semaprax::diagnostic::Diagnostic;
use semaprax::installed_diagnostics::{
    explain_installed_diagnostic, installed_diagnostic_catalog, InstalledDiagnosticExplanation,
    INSTALLED_DIAGNOSTIC_CATALOG_SCHEMA, INSTALLED_DIAGNOSTIC_EXPLANATION_SCHEMA,
    MAX_INSTALLED_DIAGNOSTIC_CATALOG_BYTES, MAX_INSTALLED_DIAGNOSTIC_EXPLANATION_BYTES,
};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

static SERIAL: AtomicU64 = AtomicU64::new(0);

type CodeOccurrences = BTreeMap<String, BTreeSet<(String, u32)>>;
type DynamicSites = BTreeSet<(String, u32)>;

struct EmptyRoot(PathBuf);

impl EmptyRoot {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "spx-installed-diagnostics-v1-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn invoke(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .current_dir(&self.0)
            .args(arguments)
            .output()
            .unwrap()
    }

    fn assert_empty(&self) {
        assert_eq!(std::fs::read_dir(&self.0).unwrap().count(), 0);
    }
}

impl Drop for EmptyRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

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

fn canonical(value: &Value) -> String {
    let mut output = serde_json::to_string(&sorted(value)).unwrap();
    output.push('\n');
    output
}

fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hash.finalize())
    )
}

fn envelope(bytes: &str, schema: &str, domain: &[u8], limit: usize) -> Value {
    assert!(bytes.ends_with('\n'));
    assert!(bytes.len() <= limit);
    let value: Value = serde_json::from_str(bytes).unwrap();
    assert_eq!(value["schema"], schema);
    assert_eq!(canonical(&value), bytes);
    let payload = canonical(&value["payload"]);
    assert_eq!(value["digest"], digest(domain, payload.as_bytes()));
    assert_eq!(value["payload"]["authority"], false);
    assert_eq!(value["payload"]["compiler"]["package"], "semaprax");
    assert_eq!(
        value["payload"]["compiler"]["version"],
        env!("CARGO_PKG_VERSION")
    );
    assert_eq!(
        value["payload"]["compiler"]["binary_identity_claimed"],
        false
    );
    value
}

fn collect_rust(path: &Path, files: &mut Vec<PathBuf>) {
    if !path.exists() {
        return;
    }
    for entry in std::fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            collect_rust(&entry.path(), files);
        } else if entry.path().extension().is_some_and(|value| value == "rs") {
            files.push(entry.path());
        }
    }
}

fn valid_code(token: &str) -> bool {
    let body = token.strip_prefix("SPX-").unwrap_or_default().as_bytes();
    body.len() >= 4
        && body.len() <= 16
        && body[..body.len() - 3].iter().all(u8::is_ascii_uppercase)
        && body[body.len() - 3..].iter().all(u8::is_ascii_digit)
}

fn tokens(source: &str) -> Vec<(usize, &str)> {
    let bytes = source.as_bytes();
    let mut result = Vec::new();
    let mut cursor = 0;
    while cursor + 4 <= bytes.len() {
        let Some(relative) = source[cursor..].find("SPX-") else {
            break;
        };
        let start = cursor + relative;
        let mut end = start + 4;
        while end < bytes.len() && (bytes[end].is_ascii_uppercase() || bytes[end].is_ascii_digit())
        {
            end += 1;
        }
        let token = &source[start..end];
        let boundary = |byte: u8| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-');
        if valid_code(token)
            && (start == 0 || !boundary(bytes[start - 1]))
            && (end == bytes.len() || !boundary(bytes[end]))
        {
            result.push((start, token));
        }
        cursor = end.max(start + 4);
    }
    result
}

fn line(source: &str, offset: usize) -> u32 {
    (source.as_bytes()[..offset]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count()
        + 1) as u32
}

fn independently_scan() -> (CodeOccurrences, DynamicSites) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    collect_rust(&root.join("src"), &mut files);
    collect_rust(&root.join("crates"), &mut files);
    files.sort();
    let mut codes = BTreeMap::<String, BTreeSet<(String, u32)>>::new();
    let mut dynamic = BTreeSet::new();
    for path in files {
        let relative = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let source = std::fs::read_to_string(&path).unwrap();
        for (offset, code) in tokens(&source) {
            codes
                .entry(code.to_owned())
                .or_default()
                .insert((relative.clone(), line(&source, offset)));
        }
        for needle in [
            "Diagnostic::error(",
            "Diagnostic::warning(",
            "Diagnostic::io(",
        ] {
            let mut rest = source.as_str();
            let mut base = 0;
            while let Some(found) = rest.find(needle) {
                let offset = base + found;
                if !source[offset + needle.len()..]
                    .trim_start()
                    .starts_with("\"SPX-")
                {
                    dynamic.insert((relative.clone(), line(&source, offset)));
                }
                let advance = found + needle.len();
                base += advance;
                rest = &rest[advance..];
            }
        }
    }
    (codes, dynamic)
}

fn error_code(errors: Vec<Diagnostic>) -> &'static str {
    assert_eq!(errors.len(), 1);
    errors[0].code
}

#[test]
fn catalog_exactly_covers_static_tokens_and_reports_unresolved_dynamic_sites() {
    let catalog = installed_diagnostic_catalog().unwrap();
    let repeated = installed_diagnostic_catalog().unwrap();
    assert_eq!(catalog, repeated);
    let value = envelope(
        catalog.to_json(),
        INSTALLED_DIAGNOSTIC_CATALOG_SCHEMA,
        b"semaprax.installed-diagnostic-catalog.payload.digest.v1\0",
        MAX_INSTALLED_DIAGNOSTIC_CATALOG_BYTES,
    );
    let (codes, dynamic) = independently_scan();
    assert_eq!(catalog.code_count(), codes.len());
    assert_eq!(
        value["payload"]["coverage"]["static_code_count"],
        codes.len()
    );
    assert_eq!(
        value["payload"]["coverage"]["classification"],
        "complete_static_code_token_inventory_with_unresolved_dynamic_constructor_sites"
    );
    let expected_codes = codes
        .iter()
        .map(|(code, occurrences)| {
            let namespace = code[4..].trim_end_matches(|c: char| c.is_ascii_digit());
            json!({
                "code":code,
                "namespace":namespace,
                "occurrences":occurrences.iter().map(|(path,line)|json!({
                    "line":line,
                    "path":path,
                    "scope":if path.starts_with("src/") {"compiler_package"} else {"workspace_member_source"}
                })).collect::<Vec<_>>()
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        value["payload"]["diagnostics"],
        Value::Array(expected_codes)
    );
    let expected_dynamic = dynamic
        .iter()
        .map(|(path, line)| json!({
            "line":line,
            "path":path,
            "scope":if path.starts_with("src/") {"compiler_package"} else {"workspace_member_source"}
        }))
        .collect::<Vec<_>>();
    assert_eq!(
        value["payload"]["coverage"]["dynamic_constructor_sites"],
        Value::Array(expected_dynamic)
    );
    assert_eq!(
        value["payload"]["coverage"]["dynamic_constructor_site_count"],
        dynamic.len()
    );
}

#[test]
fn explanation_is_deterministic_digest_bound_and_exactly_replayable() {
    let explanation = explain_installed_diagnostic("SPX-T001").unwrap();
    let repeated = explain_installed_diagnostic("SPX-T001").unwrap();
    assert_eq!(explanation, repeated);
    let value = envelope(
        explanation.to_json(),
        INSTALLED_DIAGNOSTIC_EXPLANATION_SCHEMA,
        b"semaprax.installed-diagnostic-explanation.payload.digest.v1\0",
        MAX_INSTALLED_DIAGNOSTIC_EXPLANATION_BYTES,
    );
    assert_eq!(value["payload"]["code"], "SPX-T001");
    assert_eq!(value["payload"]["explanation"]["namespace"], "T");
    assert_eq!(value["payload"]["concise"], explanation.to_text());
    assert!(explanation.to_text().ends_with('\n'));
    assert_eq!(explanation.code(), "SPX-T001");
    assert_eq!(value["digest"], explanation.digest());
    assert_eq!(
        InstalledDiagnosticExplanation::replay(
            "SPX-T001",
            explanation.digest(),
            explanation.to_json().as_bytes(),
        )
        .unwrap(),
        explanation
    );
}

#[test]
fn explain_cli_is_exact_core_projection_and_has_no_working_directory_authority() {
    let root = EmptyRoot::new();
    let explanation = explain_installed_diagnostic("SPX-T001").unwrap();
    for (arguments, expected) in [
        (&["explain", "SPX-T001"][..], explanation.to_text()),
        (
            &["explain", "SPX-T001", "--json"][..],
            explanation.to_json(),
        ),
    ] {
        let output = root.invoke(arguments);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, expected.as_bytes());
        root.assert_empty();
    }
}

#[test]
fn malformed_unknown_noncanonical_tampered_and_oversized_inputs_fail_closed() {
    assert_eq!(
        error_code(explain_installed_diagnostic("spx-T001").unwrap_err()),
        "SPX-G540"
    );
    assert_eq!(
        error_code(explain_installed_diagnostic("SPX-Z999").unwrap_err()),
        "SPX-G542"
    );
    let explanation = explain_installed_diagnostic("SPX-T001").unwrap();
    assert_eq!(
        error_code(
            InstalledDiagnosticExplanation::replay(
                "SPX-T001",
                explanation.digest(),
                explanation.to_json().trim_end().as_bytes(),
            )
            .unwrap_err()
        ),
        "SPX-G540"
    );
    let mut tampered: Value = serde_json::from_str(explanation.to_json()).unwrap();
    tampered["payload"]["concise"] = json!("tampered\n");
    let tampered = canonical(&tampered);
    assert_eq!(
        error_code(
            InstalledDiagnosticExplanation::replay(
                "SPX-T001",
                explanation.digest(),
                tampered.as_bytes(),
            )
            .unwrap_err()
        ),
        "SPX-G543"
    );
    assert_eq!(
        error_code(
            InstalledDiagnosticExplanation::replay(
                "SPX-T001",
                "invalid",
                explanation.to_json().as_bytes(),
            )
            .unwrap_err()
        ),
        "SPX-G540"
    );
    let oversized = vec![b' '; MAX_INSTALLED_DIAGNOSTIC_EXPLANATION_BYTES + 1];
    assert_eq!(
        error_code(
            InstalledDiagnosticExplanation::replay("SPX-T001", explanation.digest(), &oversized,)
                .unwrap_err()
        ),
        "SPX-G541"
    );

    let root = EmptyRoot::new();
    for arguments in [
        &["explain"][..],
        &["explain", "SPX-T001", "--unknown"][..],
        &["explain", "SPX-T001", "--json", "extra"][..],
    ] {
        let output = root.invoke(arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(output.stdout.is_empty());
        root.assert_empty();
    }
    let unknown = root.invoke(&["explain", "SPX-Z999"]);
    assert!(!unknown.status.success());
    assert!(unknown.stdout.is_empty());
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("SPX-G542"));
    root.assert_empty();
}

#[test]
fn legacy_diagnostic_text_and_json_bytes_are_unchanged() {
    let diagnostic = Diagnostic::warning(
        "SPX-W001",
        "smell",
        Span {
            line: 7,
            column: 3,
            start: 42,
            end: 51,
        },
    )
    .at_path("src/a.spx")
    .with_help("try the other spelling");
    assert_eq!(
        diagnostic.to_string(),
        "warning[SPX-W001]: smell at src/a.spx:7:3\n  help: try the other spelling"
    );
    assert_eq!(
        diagnostic.json(),
        "{\"code\":\"SPX-W001\",\"severity\":\"warning\",\"message\":\"smell\",\"path\":\"src/a.spx\",\"location\":{\"line\":7,\"column\":3,\"start\":42,\"end\":51},\"help\":\"try the other spelling\"}"
    );
}
