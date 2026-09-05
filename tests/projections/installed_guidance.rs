use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::installed_guidance::{
    installed_query_capabilities, installed_skill, InstalledSkill,
    INSTALLED_QUERY_CAPABILITIES_SCHEMA, INSTALLED_SKILL_SCHEMA, MAX_INSTALLED_GUIDANCE_BYTES,
};
use semaprax::query::QueryFilters;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct EmptyRoot(PathBuf);

impl EmptyRoot {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "spx-installed-guidance-v1-{}-{}",
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

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "status={:?}, stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "{:?}", output.stderr);
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

fn assert_envelope(bytes: &str, schema: &str, domain: &[u8]) -> Value {
    assert!(bytes.ends_with('\n'));
    assert!(bytes.len() <= MAX_INSTALLED_GUIDANCE_BYTES);
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
    let commit = &value["payload"]["compiler"]["build_commit"];
    assert!(
        commit.is_null()
            || commit.as_str().is_some_and(|value| {
                value.len() == 40
                    && value
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            })
    );
    value
}

#[test]
fn all_six_installed_skills_equal_the_exact_core_artifacts() {
    let root = EmptyRoot::new();
    assert_eq!(
        InstalledSkill::ALL.map(InstalledSkill::as_str),
        ["agent", "language", "graph", "stdlib", "packages", "effects"]
    );
    for skill in InstalledSkill::ALL {
        assert_eq!(InstalledSkill::parse(skill.as_str()), Some(skill));
        let core = installed_skill(skill).unwrap();
        let repeated = installed_skill(skill).unwrap();
        assert_eq!(core, repeated);
        assert_eq!(core.schema(), INSTALLED_SKILL_SCHEMA);
        assert_eq!(core.digest(), repeated.digest());
        let value = assert_envelope(
            core.to_json(),
            INSTALLED_SKILL_SCHEMA,
            b"semaprax.installed-skill.payload.digest.v1\0",
        );
        assert_eq!(value["payload"]["skill"], skill.as_str());
        assert_eq!(
            value["payload"]["limits"]["max_document_bytes"],
            MAX_INSTALLED_GUIDANCE_BYTES
        );

        let output = root.invoke(&["skills", "get", skill.as_str()]);
        assert_success(&output);
        assert_eq!(output.stdout, core.to_json().as_bytes());
        root.assert_empty();
    }
}

#[test]
fn query_capabilities_are_exact_inert_installed_support() {
    let root = EmptyRoot::new();
    let core = installed_query_capabilities().unwrap();
    assert_eq!(core.schema(), INSTALLED_QUERY_CAPABILITIES_SCHEMA);
    let value = assert_envelope(
        core.to_json(),
        INSTALLED_QUERY_CAPABILITIES_SCHEMA,
        b"semaprax.installed-query-capabilities.payload.digest.v1\0",
    );
    assert_eq!(
        value["payload"]["operations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|operation| operation["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "declarations",
            "symbol",
            "context",
            "impact",
            "available_operations",
            "ownership_at_expression",
            "declaration_consumers"
        ]
    );
    assert_eq!(
        value["payload"]["transaction_operations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|operation| operation["kind"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "rename_display_name",
            "replace_block",
            "add_contract",
            "add_declaration"
        ]
    );
    assert_eq!(value["payload"]["host_grants"], json!([]));
    assert_eq!(value["payload"]["authority"], false);

    let output = root.invoke(&["query", "--capabilities"]);
    assert_success(&output);
    assert_eq!(output.stdout, core.to_json().as_bytes());
    root.assert_empty();
}

#[test]
fn malformed_installed_guidance_grammar_is_status_two_and_inert() {
    let root = EmptyRoot::new();
    for arguments in [
        &["skills"][..],
        &["skills", "get"][..],
        &["skills", "list", "agent"][..],
        &["skills", "get", "Agent"][..],
        &["skills", "get", "unknown"][..],
        &["skills", "get", "agent", "extra"][..],
        &["query", "--capabilities", "extra"][..],
        &["query", "--capabilities", "--json"][..],
    ] {
        let output = root.invoke(arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(output.stdout.is_empty(), "{arguments:?}");
        assert!(output.stderr.len() <= 4096, "{arguments:?}");
        root.assert_empty();
    }
}

#[test]
fn legacy_source_query_output_is_preserved() {
    let root = EmptyRoot::new();
    let source_path = root.0.join("sample.spx");
    let source =
        "module guidance.sample;\n\n@id(\"guidance.main\")\nfn main() -> i64\n{\n    0\n}\n";
    std::fs::write(&source_path, source).unwrap();
    let (program, comments) = semaprax::parse_with_comments(source, &source_path).unwrap();
    let filters = QueryFilters {
        kinds: vec!["function".to_owned()],
        name: Some("main".to_owned()),
        ..QueryFilters::default()
    };
    let direct = semaprax::query::run(&program, &comments, &filters).unwrap();
    let expected = semaprax::query::json(&direct);
    let output = root.invoke(&[
        "query",
        source_path.to_str().unwrap(),
        "--kind",
        "function",
        "--name",
        "main",
        "--json",
    ]);
    assert_success(&output);
    assert_eq!(output.stdout, expected.as_bytes());
    assert_eq!(std::fs::read_to_string(source_path).unwrap(), source);
}

#[test]
fn installed_documents_reference_only_existing_embedded_sources() {
    let language = installed_skill(InstalledSkill::Language).unwrap();
    let value: Value = serde_json::from_str(language.to_json()).unwrap();
    let rows = value["payload"]["sources"].as_array().unwrap();
    for (id, path) in [
        ("agent-quick-reference", "docs/AGENT-QUICK-REFERENCE.md"),
        ("language-shapes-catalog", "docs/LANGUAGE-SHAPES-CATALOG.md"),
    ] {
        let row = rows.iter().find(|row| row["id"] == id).unwrap();
        let bytes = std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap();
        assert_eq!(row["bytes"], bytes.len());
        assert_eq!(
            row["digest"],
            digest(b"semaprax.installed-guidance.source.digest.v1\0", &bytes)
        );
    }
}
