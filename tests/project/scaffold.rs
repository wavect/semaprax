use semaprax::project::{derive_project_scaffold, replay_project_scaffold};
use sha2::{Digest, Sha256};

const NAME: &str = "demo-project";
const DIGEST_DOMAIN: &[u8] = b"semaprax.project-scaffold.digest.v2\0";
const ARTIFACT_DIGEST: &str =
    "sha256:1dc3a351d11baa83a8d2c82cbc7eab528798b58386d977ea5f7e56b830d476f3";
const FILE_DIGESTS: [&str; 5] = [
    "sha256:abf54a4e33e0f6fa1a6be76dc0d324b713de081bc2529b7cc2b2598477e643ec",
    "sha256:84d80abf5e7327a753c1f48e0a3f48a8a94f8217863cdd5f08c487338215447a",
    "sha256:158830289b7204499bd5ab0854ecf57caaa3e2654e6de36aab387ba94f869db3",
    "sha256:f5508160f8d4bd6a406b9a9a51e7765146a79185f6e042cb7b9b8def7765e975",
    "sha256:e570c4d28171cd826038c3f881a51d87693ef1071e14a3c6323d0c69815d8a00",
];
const PATHS: [&str; 5] = [
    "README.md",
    "AGENTS.md",
    "semaprax.toml",
    "src/app.spx",
    "src/tests.spx",
];

fn expected_files() -> [Vec<u8>; 5] {
    [
        b"# demo-project\n\nA small calculator project created by SEMAPRAX.\n\n```sh\nsemaprax check .\nsemaprax test .\nsemaprax run .\nsemaprax build . --target web -o web\n```\n\nRead `AGENTS.md` before editing the source, whether you are a person or a\ncoding agent: it lists the commands and the rules that differ from other\nlanguages.\n".to_vec(),
        b"# Agent guide for demo-project\n\nThis is a SEMAPRAX project. `semaprax.toml` lists its modules; the compiler\nis the authority on what the language admits. Read `semaprax help language`\nbefore writing source.\n\n## Commands\n\n- `semaprax check .` parses, resolves, type-checks, and verifies every module.\n- `semaprax test .` runs `demo_project.tests`; `semaprax run .` runs the entry and prints its `i64`.\n- `semaprax fmt <file>` rewrites one file in canonical form.\n- `semaprax build . --target web -o dist/web` emits a browser package.\n- `semaprax help <command>` prints one command's exact grammar.\n\n## Rules that differ from other languages\n\n- Every file starts with `module dotted.name;`, and every declaration carries\n  `@id(\"...\")`. The id is the stable identity: rename freely, never change an id.\n- A function body is statements followed by exactly one tail expression. There\n  is no `return`, `for`, `else if`, tuple, or unit value.\n- `if` always has `else`; a `while` body ends with the bool that decides\n  whether to loop again.\n- Contracts are `requires` and `ensures` lines; effects are `permit` at module\n  level plus `uses` on every function that performs or calls into one.\n- Check the whole project, not one file: modules import each other, so a\n  single file reports `SPX-G172` or `SPX-T105`.\n- A new module must be listed in `sources` in `semaprax.toml`, and a test\n  module in `tests`.\n- Diagnostics carry stable `SPX-` codes and, where the compiler knows the fix,\n  a `help:` line. `semaprax check . --json` prints one diagnostic per line.\n".to_vec(),
        b"schema = \"semaprax.project.v1\"\nname = \"demo-project\"\nentry = \"demo_project.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"demo-project.add\"]\ntests = [\"demo_project.tests\"]\n".to_vec(),
        b"module demo_project.app;\n\n@id(\"demo-project.add\")\nfn add(left: i64, right: i64) -> i64\n{\n    left + right\n}\n\n@id(\"demo-project.app.main\")\nfn main() -> i64\n{\n    add(19, 23)\n}\n".to_vec(),
        b"module demo_project.tests;\n\n@id(\"demo-project.tests.main\")\nfn main() -> i64\n{\n    if 19 + 23 == 42 { 0 } else { 1 }\n}\n".to_vec(),
    ]
}

fn assert_digest(value: &str) {
    assert!(value.starts_with("sha256:"), "{value}");
    assert_eq!(value.len(), "sha256:".len() + 64, "{value}");
    assert!(value["sha256:".len()..]
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
}

fn sha256(bytes: &[u8]) -> String {
    let mut output = String::from("sha256:");
    for byte in Sha256::digest(bytes) {
        use std::fmt::Write as _;
        write!(output, "{byte:02x}").unwrap();
    }
    output
}

fn artifact_digest(bytes_without_digest: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(DIGEST_DOMAIN);
    hasher.update((bytes_without_digest.len() as u64).to_le_bytes());
    hasher.update(bytes_without_digest);
    let mut output = String::from("sha256:");
    for byte in hasher.finalize() {
        use std::fmt::Write as _;
        write!(output, "{byte:02x}").unwrap();
    }
    output
}

#[test]
fn derivation_is_literal_ordered_deterministic_and_self_replaying() {
    let derived = derive_project_scaffold(NAME, "calculator").unwrap();
    assert_eq!(derived.schema(), "semaprax.project-scaffold.v2");
    assert_eq!(derived.template(), "calculator");
    assert_eq!(derived.project_schema(), "semaprax.project.v1");
    assert_eq!(derived.project_name(), NAME);
    assert_digest(derived.digest());
    assert_eq!(derived.digest(), ARTIFACT_DIGEST);
    assert_eq!(derived.canonical_bytes().len(), 3511);

    let expected = expected_files();
    assert_eq!(derived.files().len(), PATHS.len());
    for (index, ((file, path), bytes)) in
        derived.files().iter().zip(PATHS).zip(expected).enumerate()
    {
        assert_eq!(file.path(), path);
        assert_eq!(file.bytes(), bytes);
        assert_digest(file.sha256());
        assert_eq!(file.sha256(), sha256(file.bytes()));
        assert_eq!(file.sha256(), FILE_DIGESTS[index]);
    }

    let again = derive_project_scaffold(NAME, "calculator").unwrap();
    assert_eq!(again.canonical_bytes(), derived.canonical_bytes());
    assert_eq!(again.digest(), derived.digest());

    let canonical = derived.canonical_bytes();
    let replayed =
        replay_project_scaffold(NAME, "calculator", &canonical, derived.digest()).unwrap();
    assert_eq!(replayed.schema(), derived.schema());
    assert_eq!(replayed.template(), derived.template());
    assert_eq!(replayed.project_schema(), derived.project_schema());
    assert_eq!(replayed.project_name(), derived.project_name());
    assert_eq!(replayed.canonical_bytes(), derived.canonical_bytes());
    assert_eq!(replayed.digest(), derived.digest());
    for (left, right) in replayed.files().iter().zip(derived.files()) {
        assert_eq!(left.path(), right.path());
        assert_eq!(left.bytes(), right.bytes());
        assert_eq!(left.sha256(), right.sha256());
    }
}

#[test]
fn names_templates_and_expected_replay_subject_are_bounded() {
    for name in ["", "Bad_Name", "-leading", "a..b"] {
        let error = derive_project_scaffold(name, "calculator").unwrap_err();
        assert_eq!(error[0].code, "SPX-J115", "{name:?}");
    }
    let oversized = "a".repeat(65);
    let error = derive_project_scaffold(&oversized, "calculator").unwrap_err();
    assert_eq!(error[0].code, "SPX-J116");

    let first = derive_project_scaffold("first-project", "calculator").unwrap();
    let second = derive_project_scaffold("second-project", "calculator").unwrap();
    assert_ne!(first.canonical_bytes(), second.canonical_bytes());
    assert_ne!(first.digest(), second.digest());
    assert_eq!(
        replay_project_scaffold(
            "second-project",
            "calculator",
            &second.canonical_bytes(),
            second.digest(),
        )
        .unwrap()
        .project_name(),
        "second-project"
    );
    let error = replay_project_scaffold(
        "first-project",
        "calculator",
        &second.canonical_bytes(),
        second.digest(),
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-J115");
    let error = derive_project_scaffold(NAME, "remote").unwrap_err();
    assert_eq!(error[0].code, "SPX-J115");
}

fn replay_error(bytes: &[u8], digest: &str) {
    let error = replay_project_scaffold(NAME, "calculator", bytes, digest).unwrap_err();
    assert_eq!(error[0].code, "SPX-J115");
}

#[test]
fn replay_rejects_noncanonical_and_semantically_reminted_capsules() {
    let artifact = derive_project_scaffold(NAME, "calculator").unwrap();
    let canonical = artifact.canonical_bytes();
    replay_error(&[], artifact.digest());

    let mut trailing = canonical.to_vec();
    trailing.push(b' ');
    replay_error(&trailing, artifact.digest());

    let mut wrong_schema = canonical.to_vec();
    let schema = b"semaprax.project-scaffold.v2";
    let at = wrong_schema
        .windows(schema.len())
        .position(|window| window == schema)
        .unwrap();
    wrong_schema[at + schema.len() - 1] = b'3';
    replay_error(&wrong_schema, artifact.digest());

    let mut wrong_name = canonical.to_vec();
    let at = wrong_name
        .windows(NAME.len())
        .position(|window| window == NAME.as_bytes())
        .unwrap();
    wrong_name[at] = b'x';
    replay_error(&wrong_name, artifact.digest());

    let mut wrong_file = canonical.to_vec();
    let needle = b"calculator project";
    let at = wrong_file
        .windows(needle.len())
        .position(|window| window == needle)
        .unwrap();
    wrong_file[at] = b'C';
    replay_error(&wrong_file, artifact.digest());

    // Recompute both the changed file hash and the top-level capsule digest.
    // Replay must still reject a correctly reminted document because the
    // expected subject is the freshly derived built-in template, not merely a
    // self-consistent attacker-supplied capsule.
    let canonical_text = String::from_utf8(canonical.to_vec()).unwrap();
    let changed_readme = expected_files()[0]
        .windows(b"A small".len())
        .position(|window| window == b"A small")
        .map(|at| {
            let mut bytes = expected_files()[0].clone();
            bytes[at + 2] = b'S';
            bytes
        })
        .unwrap();
    let reminted_file_digest = sha256(&changed_readme);
    let mut reminted = canonical_text
        .replacen("A small calculator", "A Small calculator", 1)
        .replacen(FILE_DIGESTS[0], &reminted_file_digest, 1);
    let digest_member = format!(",\"digest\":\"{}\"", artifact.digest());
    let without_digest = reminted.replacen(&digest_member, "", 1);
    let reminted_digest = artifact_digest(without_digest.as_bytes());
    reminted = reminted.replacen(artifact.digest(), &reminted_digest, 1);
    replay_error(reminted.as_bytes(), &reminted_digest);

    let mut value: serde_json::Value = serde_json::from_slice(&canonical).unwrap();
    let files = value["files"].as_array_mut().unwrap();
    files.swap(0, 1);
    replay_error(&serde_json::to_vec(&value).unwrap(), artifact.digest());

    let mut value: serde_json::Value = serde_json::from_slice(&canonical).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .insert("unknown".to_owned(), serde_json::Value::Bool(true));
    replay_error(&serde_json::to_vec(&value).unwrap(), artifact.digest());

    let oversized = vec![b'x'; 65_536 + 1];
    let error =
        replay_project_scaffold(NAME, "calculator", &oversized, artifact.digest()).unwrap_err();
    assert_eq!(error[0].code, "SPX-J116");

    let exact = vec![b'x'; 65_536];
    let error = replay_project_scaffold(NAME, "calculator", &exact, artifact.digest()).unwrap_err();
    assert_eq!(error[0].code, "SPX-J115");
}
