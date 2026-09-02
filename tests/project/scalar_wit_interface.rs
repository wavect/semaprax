use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use semaprax::project::{
    with_authenticated_project, ProjectRevision, ScalarWitTypeV1, MAX_SCALAR_WIT_DESCRIPTOR_BYTES,
    SCALAR_WIT_INTERFACE_SCHEMA,
};
use sha2::{Digest, Sha256};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const DIGEST_DOMAIN: &[u8] = b"semaprax.project.scalar-wit-interface.digest.v1\0";
const WIT_DIGEST_DOMAIN: &[u8] = b"semaprax.project.scalar-wit-interface.wit-digest.v1\0";

const EXPECTED_WIT: &str = r#"package semaprax:project-scalar@1.0.0;

interface exports {
  record status { domain: string, code: u32, class: u8, retryable: option<bool> }
  spx-7769742e612d62: func(arg-0: s64) -> result<s64, status>;
  spx-7769742e612e62: func(arg-0: s64) -> result<s64, status>;
  spx-7769742e615f62: func(arg-0: s64) -> result<s64, status>;
  spx-7769742e626f6f6c: func(arg-0: bool) -> result<bool, status>;
  spx-7769742e6569676874: func(arg-0: s64, arg-1: s64, arg-2: s64, arg-3: s64, arg-4: s64, arg-5: s64, arg-6: s64, arg-7: s64) -> result<s64, status>;
  spx-7769742e7a65726f: func() -> result<s64, status>;
}

world project-scalar-v1 {
  export exports;
}
"#;

struct TemporaryProject(PathBuf);

impl Drop for TemporaryProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/project_scalar_wit_interface_v1")
}

fn temporary(label: &str) -> TemporaryProject {
    let root = std::env::temp_dir().join(format!(
        "semaprax-project-scalar-wit-{label}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    TemporaryProject(root.canonicalize().unwrap())
}

fn copy_fixture(label: &str) -> TemporaryProject {
    let project = temporary(label);
    for name in ["app.spx", "semaprax.toml", "source.spx", "tests.spx"] {
        std::fs::copy(fixture_root().join(name), project.0.join(name)).unwrap();
    }
    project
}

fn retain(manifest: &Path) -> Arc<ProjectRevision> {
    with_authenticated_project(manifest, |snapshot| Ok(snapshot.retain_revision())).unwrap()
}

fn digest(bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(DIGEST_DOMAIN);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hash.finalize())
    )
}

fn wit_digest(bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(WIT_DIGEST_DOMAIN);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hash.finalize())
    )
}

fn diagnostic_code(error: &[semaprax::diagnostic::Diagnostic]) -> &str {
    error.first().map_or("", |diagnostic| diagnostic.code)
}

fn exact_files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(root)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            assert!(entry.file_type().unwrap().is_file());
            (
                entry.file_name().into_string().unwrap(),
                std::fs::read(entry.path()).unwrap(),
            )
        })
        .collect()
}

#[test]
fn six_export_interface_is_an_exact_ordered_stable_id_projection() {
    let revision = retain(&fixture_root().join("semaprax.toml"));
    let artifact = revision.scalar_wit_interface_v1().unwrap();

    assert_eq!(artifact.schema(), SCALAR_WIT_INTERFACE_SCHEMA);
    assert_eq!(artifact.project_schema(), "semaprax.project.v1");
    assert_eq!(artifact.project_name(), "scalar-wit-interface");
    assert_eq!(artifact.wit(), EXPECTED_WIT);
    assert_eq!(
        artifact
            .exports()
            .iter()
            .map(|export| export.stable_id().as_str())
            .collect::<Vec<_>>(),
        [
            "wit.a-b",
            "wit.a.b",
            "wit.a_b",
            "wit.bool",
            "wit.eight",
            "wit.zero",
        ]
    );
    assert_eq!(artifact.exports()[3].parameters(), [ScalarWitTypeV1::Bool]);
    assert_eq!(artifact.exports()[3].result(), ScalarWitTypeV1::Bool);
    assert_eq!(artifact.exports()[4].parameters().len(), 8);
    assert!(artifact.exports()[4]
        .parameters()
        .iter()
        .all(|parameter| *parameter == ScalarWitTypeV1::I64));
    assert!(artifact.exports()[5].parameters().is_empty());
    assert_eq!(artifact.digest(), digest(&artifact.canonical_bytes()));
    assert_eq!(artifact.wit_digest(), wit_digest(artifact.wit().as_bytes()));

    let descriptor: serde_json::Value =
        serde_json::from_slice(&artifact.canonical_bytes()).unwrap();
    assert_eq!(descriptor["schema"], SCALAR_WIT_INTERFACE_SCHEMA);
    assert_eq!(descriptor["wit"], EXPECTED_WIT);
    assert_eq!(descriptor["wit_digest"], artifact.wit_digest());
    assert_eq!(descriptor["mapping"]["i64"], "s64");
    assert_eq!(descriptor["mapping"]["bool"], "bool");
    assert_eq!(descriptor["mapping"]["function_result"], "result<T,status>");
    assert_eq!(
        descriptor["mapping"]["status"]["schema"],
        "semaprax.status.v1"
    );
    assert_eq!(
        descriptor["mapping"]["status"]["domain"]["semantic"],
        "domain_id"
    );
    assert_eq!(
        descriptor["mapping"]["status"]["domain"]["max_utf8_bytes"],
        255
    );
    assert_eq!(
        descriptor["mapping"]["status"]["domain"]["forbid_nul"],
        true
    );
    assert_eq!(
        descriptor["mapping"]["status"]["class"]["ordinals"]["adapter"],
        5
    );
    assert_eq!(
        descriptor["mapping"]["status"]["retryable"]["unknown"],
        serde_json::Value::Null
    );
    assert_eq!(descriptor["exports"].as_array().unwrap().len(), 6);
    assert_eq!(descriptor["nonclaims"][0], "no_component_binary_or_runtime");
    assert!(
        artifact
            .exports()
            .iter()
            .map(|export| export.wit_name())
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == 6
    );
}

#[test]
fn display_rename_preserves_wit_but_changes_the_bound_descriptor_subject() {
    let project = copy_fixture("rename");
    let manifest = project.0.join("semaprax.toml");
    let baseline = retain(&manifest);
    let baseline_artifact = baseline.scalar_wit_interface_v1().unwrap();

    let source_path = project.0.join("source.spx");
    let source = std::fs::read_to_string(&source_path).unwrap();
    let renamed = source
        .replace("fn invert(value: bool)", "fn negate(flag: bool)")
        .replace("!value", "!flag");
    assert_ne!(source, renamed);
    std::fs::write(&source_path, renamed).unwrap();

    let renamed_revision = retain(&manifest);
    let renamed_artifact = renamed_revision.scalar_wit_interface_v1().unwrap();
    assert_eq!(baseline_artifact.wit(), renamed_artifact.wit());
    assert_eq!(baseline_artifact.exports(), renamed_artifact.exports());
    assert_ne!(
        baseline_artifact.project_revision(),
        renamed_artifact.project_revision()
    );
    assert_ne!(
        baseline_artifact.canonical_bytes(),
        renamed_artifact.canonical_bytes()
    );
    assert_ne!(baseline_artifact.digest(), renamed_artifact.digest());

    let error = renamed_revision
        .replay_scalar_wit_interface_v1(
            &baseline_artifact.canonical_bytes(),
            &baseline_artifact.digest(),
        )
        .unwrap_err();
    assert_eq!(diagnostic_code(&error), "SPX-WIT111");
}

#[test]
fn selected_signature_change_updates_wit_facts_and_both_digests() {
    let project = copy_fixture("signature");
    let manifest = project.0.join("semaprax.toml");
    let baseline = retain(&manifest).scalar_wit_interface_v1().unwrap();

    let source_path = project.0.join("source.spx");
    let source = std::fs::read_to_string(&source_path).unwrap();
    let changed = source.replace(
        "fn dot(value: i64) -> i64\n{\n    value\n}",
        "fn dot(value: bool) -> bool\n{\n    value\n}",
    );
    assert_ne!(source, changed);
    std::fs::write(&source_path, changed).unwrap();

    let changed = retain(&manifest).scalar_wit_interface_v1().unwrap();
    let dot = changed
        .exports()
        .iter()
        .find(|export| export.stable_id().as_str() == "wit.a.b")
        .unwrap();
    assert_eq!(dot.parameters(), [ScalarWitTypeV1::Bool]);
    assert_eq!(dot.result(), ScalarWitTypeV1::Bool);
    assert_ne!(baseline.wit(), changed.wit());
    assert_ne!(baseline.wit_digest(), changed.wit_digest());
    assert_ne!(baseline.digest(), changed.digest());
}

#[test]
fn descriptor_replay_rejects_mutation_truncation_trailing_bytes_and_reminting() {
    let revision = retain(&fixture_root().join("semaprax.toml"));
    let artifact = revision.scalar_wit_interface_v1().unwrap();
    let bytes = artifact.canonical_bytes();
    let artifact_digest = artifact.digest();

    let mut mutated = bytes.clone();
    let position = mutated
        .windows(b"scalar-wit-interface".len())
        .position(|window| window == b"scalar-wit-interface")
        .unwrap();
    mutated[position] = b't';
    for hostile in [mutated, bytes[..bytes.len() - 1].to_vec(), {
        let mut trailing = bytes.clone();
        trailing.push(b' ');
        trailing
    }] {
        let error = revision
            .replay_scalar_wit_interface_v1(&hostile, &artifact_digest)
            .unwrap_err();
        assert_eq!(diagnostic_code(&error), "SPX-WIT111");
    }

    let mut reminted: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    reminted["project_name"] = serde_json::Value::String("forged-project".to_owned());
    let reminted = serde_json::to_vec(&reminted).unwrap();
    let reminted_digest = digest(&reminted);
    let error = revision
        .replay_scalar_wit_interface_v1(&reminted, &reminted_digest)
        .unwrap_err();
    assert_eq!(diagnostic_code(&error), "SPX-WIT111");

    assert_eq!(
        revision
            .replay_scalar_wit_interface_v1(&bytes, &artifact_digest)
            .unwrap(),
        artifact
    );
}

#[test]
fn descriptor_replay_has_an_exact_outer_byte_limit() {
    let revision = retain(&fixture_root().join("semaprax.toml"));
    let at_limit = vec![b' '; MAX_SCALAR_WIT_DESCRIPTOR_BYTES];
    let error = revision
        .replay_scalar_wit_interface_v1(&at_limit, &digest(&at_limit))
        .unwrap_err();
    assert_eq!(diagnostic_code(&error), "SPX-WIT111");

    let over_limit = vec![b' '; MAX_SCALAR_WIT_DESCRIPTOR_BYTES + 1];
    let error = revision
        .replay_scalar_wit_interface_v1(&over_limit, &digest(&over_limit))
        .unwrap_err();
    assert_eq!(diagnostic_code(&error), "SPX-WIT112");
}

#[test]
fn retained_interface_read_performs_no_write_or_artifact_publication() {
    let root = fixture_root();
    let before = exact_files(&root);
    let revision = retain(&root.join("semaprax.toml"));
    let artifact = revision.scalar_wit_interface_v1().unwrap();
    assert_eq!(artifact.wit(), EXPECTED_WIT);
    assert_eq!(before, exact_files(&root));
    for forbidden in [
        "app.wasm",
        "component.wasm",
        "semaprax.wit",
        "package.json",
        "semaprax.scalar-exports.json",
    ] {
        assert!(
            !root.join(forbidden).exists(),
            "unexpected emitted {forbidden}"
        );
    }
}

fn owned_fixture(label: &str, manifest: &str, source: &str) -> TemporaryProject {
    let project = temporary(label);
    std::fs::write(project.0.join("semaprax.toml"), manifest).unwrap();
    let source =
        semaprax::format::canonical(&semaprax::parse(source, Path::new("source.spx")).unwrap());
    let tests = semaprax::format::canonical(
        &semaprax::parse(
            &format!(
                "module {label}.tests;\n\n@id(\"{label}.tests.main\")\nfn main() -> i64 {{ 0 }}\n"
            ),
            Path::new("tests.spx"),
        )
        .unwrap(),
    );
    std::fs::write(project.0.join("source.spx"), source).unwrap();
    std::fs::write(project.0.join("tests.spx"), tests).unwrap();
    project
}

#[test]
fn project_v2_through_v10_reject_before_any_interface_artifact_is_returned() {
    let examples = [
        "config-validator-project",
        "binary-frame-project",
        "spxgrep-project",
        "spxgrep-native-command-project",
        "spxgrep-language-command-project",
        "spxgrep-lines-project",
        "frame-payload-project",
    ];
    for example in examples {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples")
            .join(example)
            .join("semaprax.toml");
        let revision = retain(&manifest);
        let error = revision.scalar_wit_interface_v1().unwrap_err();
        assert_eq!(diagnostic_code(&error), "SPX-J105", "{example}");
    }

    let v9 = owned_fixture(
        "flatwit",
        "schema = \"semaprax.project.v9\"\nname = \"flatwit\"\nversion = \"1.0.0\"\nprofile = \"flat-owned-record-api.v1\"\nentry = \"flatwit.app\"\nsources = [\"source.spx\", \"tests.spx\"]\nweb_exports = [\"flatwit.make\"]\ntests = [\"flatwit.tests\"]\n",
        "module flatwit.app;\n@id(\"flatwit.packet\") record Packet { @id(\"flatwit.packet.bytes\") bytes: Bytes, @id(\"flatwit.packet.kind\") kind: i64, }\n@id(\"flatwit.make\") fn make(input: borrow Slice<u8>) -> Packet { Packet { bytes: bytes_copy(input), kind: 7 } }\n@id(\"flatwit.app.main\") fn main() -> i64 { 0 }\n",
    );
    let v10 = owned_fixture(
        "utf8wit",
        "schema = \"semaprax.project.v10\"\nname = \"utf8wit\"\nversion = \"1.0.0\"\nprofile = \"owned-utf8-api.v1\"\nentry = \"utf8wit.app\"\nsources = [\"source.spx\", \"tests.spx\"]\nweb_exports = [\"utf8wit.greeting\"]\ntests = [\"utf8wit.tests\"]\n",
        "module utf8wit.app;\n@id(\"utf8wit.greeting\") fn greeting() -> string { \"hello\" }\n@id(\"utf8wit.app.main\") fn main() -> i64 { 0 }\n",
    );
    for project in [&v9, &v10] {
        let before = exact_files(&project.0);
        let revision = retain(&project.0.join("semaprax.toml"));
        let error = revision.scalar_wit_interface_v1().unwrap_err();
        assert_eq!(diagnostic_code(&error), "SPX-J105");
        assert_eq!(before, exact_files(&project.0));
    }
}
