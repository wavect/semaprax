//! Project Lock v1: `semaprax lock`, the `semaprax.lock` it writes, and the
//! explicit `--verify` that fails closed on drift. `check` is never involved.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;
use sha2::{Digest, Sha256};

static SERIAL: AtomicU64 = AtomicU64::new(0);

const CALCULATOR_TABLES: &str = "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"calculator\"\nversion = \"0.1.0\"\n\n[modules]\nentry = \"calculator.app\"\nsources = [\"src/app.spx\", \"src/core.spx\", \"src/tests.spx\"]\ntests = [\"calculator.tests\"]\n\n[exports]\nweb = [\"calculator.add\", \"calculator.divide\", \"calculator.is-negative\", \"calculator.multiply\", \"calculator.not\", \"calculator.subtract\"]\n";

const SPXGREP_TABLES: &str = "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"spxgrep\"\nversion = \"0.1.0\"\nprofile = \"useful-data-command.v1\"\n\n[modules]\nentry = \"spxgrep.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\ntests = [\"spxgrep.tests\"]\n\n[exports]\nweb = [\"spxgrep.contains\"]\n\n[command]\nfunction = \"spxgrep.contains\"\n\n[capabilities]\nrequired = [\"process.stdout.write\"]\n";

fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    let mut hex = String::from("sha256:");
    for byte in digest.finalize() {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn lock_is_deterministic_authenticated_and_binds_the_program_root() {
    let fixture = example_fixture("calculator-frozen", "examples/calculator-project", None);
    let first = cli(&fixture.root, &["lock", "semaprax.toml"]);
    assert!(first.status.success(), "{}", stderr(&first));
    let second = cli(&fixture.root, &["lock", "semaprax.toml"]);
    assert_eq!(
        first.stdout, second.stdout,
        "two renders are byte-identical"
    );
    assert!(
        !fixture.root.join("semaprax.lock").exists(),
        "no --write, no file"
    );

    let text = stdout(&first);
    assert!(text.ends_with("}\n"));
    let lock: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(lock["schema"], "semaprax.project-lock.v1");
    let payload = serde_json::to_string(&lock["payload"]).unwrap();
    assert_eq!(lock["bytes"].as_u64().unwrap() as usize, payload.len());
    assert_eq!(
        lock["digest"].as_str().unwrap(),
        digest(b"semaprax.project-lock.v1\0", payload.as_bytes()),
        "the digest is independently recomputable from the payload bytes"
    );
    let payload = &lock["payload"];
    assert_eq!(payload["schema"], "semaprax.project-lock.v1");
    assert_eq!(payload["package"]["name"], "calculator");
    assert_eq!(payload["package"]["version"], Value::Null);
    assert_eq!(payload["package"]["manifest_schema"], "semaprax.project.v1");
    assert_eq!(payload["package"]["contract"], "semaprax.project.v1");
    assert_eq!(payload["package"]["profile"], "scalar");
    assert!(payload["package"]["manifest_digest"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));

    // The program root equals the revision `check` prints; the lock never
    // affects `check`, which passes with or without a lock file present.
    let check = cli(&fixture.root, &["check", "semaprax.toml"]);
    assert!(check.status.success(), "{}", stderr(&check));
    let verified = stdout(&check);
    let revision = verified
        .trim_end()
        .strip_prefix("verified project calculator (")
        .and_then(|rest| rest.strip_suffix(')'))
        .unwrap_or_else(|| panic!("{verified}"));
    assert_eq!(payload["program_root"].as_str().unwrap(), revision);

    let files = payload["source"]["files"].as_array().unwrap();
    assert_eq!(
        files
            .iter()
            .map(|file| file["path"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["src/app.spx", "src/core.spx", "src/tests.spx"]
    );
    for file in files {
        assert!(file["source_digest"]
            .as_str()
            .unwrap()
            .starts_with("sha256:"));
        assert!(!file["source_revision"].as_str().unwrap().is_empty());
        assert!(
            file.get("source").is_none(),
            "the lock carries digests, never source"
        );
    }
    assert!(payload["source"]["workspace_revision"].as_str().is_some());

    assert_eq!(payload["interface"]["kind"], "scalar-wit.v1");
    assert!(payload["interface"]["digest"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
    assert_eq!(payload["interface"]["exports"].as_array().unwrap().len(), 6);
    assert_eq!(payload["dependencies"], Value::Array(Vec::new()));
    assert_eq!(
        payload["targets"],
        serde_json::json!([
            {"state": "default", "target": "native64"},
            {"state": "default", "target": "wasm32"},
        ])
    );
    assert_eq!(payload["capabilities"], Value::Array(Vec::new()));
    assert_eq!(payload["compiler"]["package"], "semaprax");
    assert_eq!(payload["compiler"]["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(payload["resolution_policy"]["dependencies"], "none");
    assert_eq!(payload["resolution_policy"]["registry"], "none");
    assert!(payload["nonclaims"]
        .as_array()
        .unwrap()
        .iter()
        .any(|claim| claim == "no_target_execution_or_availability_proof"));
}

#[test]
fn write_persists_the_lock_and_verify_fails_closed_on_drift() {
    let fixture = example_fixture(
        "calculator-tables",
        "examples/calculator-project",
        Some(CALCULATOR_TABLES),
    );
    let lock_path = fixture.root.join("semaprax.lock");

    // Verifying before any lock exists reports the missing file, and check is
    // unaffected either way.
    assert!(cli(&fixture.root, &["check", "semaprax.toml"])
        .status
        .success());
    let missing = cli(&fixture.root, &["lock", "semaprax.toml", "--verify"]);
    assert!(!missing.status.success());
    assert!(
        stderr(&missing).contains("SPX-J124"),
        "{}",
        stderr(&missing)
    );

    let written = cli(&fixture.root, &["lock", "semaprax.toml", "--write"]);
    assert!(written.status.success(), "{}", stderr(&written));
    let lock = std::fs::read_to_string(&lock_path).unwrap();
    let rendered = stdout(&cli(&fixture.root, &["lock", "semaprax.toml"]));
    assert_eq!(
        lock, rendered,
        "--write persists exactly the rendered bytes"
    );
    let parsed: Value = serde_json::from_str(&lock).unwrap();
    assert_eq!(
        stdout(&written),
        format!(
            "wrote semaprax.lock for calculator ({})\n",
            parsed["digest"].as_str().unwrap()
        )
    );
    assert_eq!(parsed["payload"]["package"]["version"], "0.1.0");
    assert_eq!(
        parsed["payload"]["package"]["manifest_schema"],
        "semaprax.manifest.v1"
    );
    assert_eq!(
        parsed["payload"]["package"]["contract"],
        "semaprax.project.v1"
    );
    assert!(std::fs::read_dir(&fixture.root).unwrap().all(|entry| !entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .contains("staging")));

    let verified = cli(&fixture.root, &["lock", "semaprax.toml", "--verify"]);
    assert!(verified.status.success(), "{}", stderr(&verified));
    assert_eq!(
        stdout(&verified),
        format!(
            "verified semaprax.lock for calculator ({})\n",
            parsed["digest"].as_str().unwrap()
        )
    );
    let again = cli(&fixture.root, &["lock", "semaprax.toml", "--write"]);
    assert!(again.status.success());
    assert_eq!(
        std::fs::read_to_string(&lock_path).unwrap(),
        lock,
        "idempotent"
    );

    // Source drift changes the program root and the source rows, not the
    // interface. `check` still passes; only `--verify` reports the drift.
    let core = fixture.root.join("src/core.spx");
    let original = std::fs::read_to_string(&core).unwrap();
    std::fs::write(&core, original.replace("left + right", "right + left")).unwrap();
    assert!(cli(&fixture.root, &["check", "semaprax.toml"])
        .status
        .success());
    let drifted = cli(&fixture.root, &["lock", "semaprax.toml", "--verify"]);
    assert!(!drifted.status.success());
    let message = stderr(&drifted);
    assert!(message.contains("SPX-J123"), "{message}");
    assert!(
        message.contains(
            "semaprax.lock is stale: program_root, source differ from the checked project"
        ),
        "{message}"
    );
    assert!(
        message.contains("semaprax lock semaprax.toml --write"),
        "{message}"
    );
    assert_eq!(
        std::fs::read_to_string(&lock_path).unwrap(),
        lock,
        "--verify never rewrites"
    );
    std::fs::write(&core, original).unwrap();
    assert!(cli(&fixture.root, &["lock", "semaprax.toml", "--verify"])
        .status
        .success());

    // Manifest drift changes the manifest digest, the program root, and the
    // targets rows.
    std::fs::write(
        fixture.root.join("semaprax.toml"),
        format!("{CALCULATOR_TABLES}\n[targets]\nmatrix = [\"wasm32\"]\n"),
    )
    .unwrap();
    let message = stderr(&cli(&fixture.root, &["lock", "semaprax.toml", "--verify"]));
    assert!(
        message.contains("semaprax.lock is stale: package, program_root, targets differ"),
        "{message}"
    );
    let relocked = cli(&fixture.root, &["lock", "semaprax.toml", "--write"]);
    assert!(relocked.status.success());
    let relocked: Value =
        serde_json::from_str(&std::fs::read_to_string(&lock_path).unwrap()).unwrap();
    assert_eq!(
        relocked["payload"]["targets"],
        serde_json::json!([{"state": "declared", "target": "wasm32"}])
    );
    assert!(cli(&fixture.root, &["lock", "semaprax.toml", "--verify"])
        .status
        .success());

    // Foreign or unreadable lock bytes reject before any rendering claim.
    for (bytes, fragment) in [
        (&b"not json"[..], "is not a JSON object"),
        (
            &br#"{"schema":"semaprax.offline-semantic-package-lock.v3"}"#[..],
            "does not carry schema semaprax.project-lock.v1",
        ),
    ] {
        std::fs::write(&lock_path, bytes).unwrap();
        let foreign = cli(&fixture.root, &["lock", "semaprax.toml", "--verify"]);
        assert!(!foreign.status.success());
        let message = stderr(&foreign);
        assert!(message.contains("SPX-J124"), "{message}");
        assert!(message.contains(fragment), "{message}");
    }
    std::fs::remove_file(&lock_path).unwrap();
    std::fs::create_dir(&lock_path).unwrap();
    assert!(
        stderr(&cli(&fixture.root, &["lock", "semaprax.toml", "--verify"])).contains("SPX-J124")
    );
    std::fs::remove_dir(&lock_path).unwrap();
}

#[test]
fn lock_reports_the_interface_kind_per_profile_and_rejects_bad_usage() {
    let spxgrep = example_fixture("spxgrep", "examples/spxgrep-project", Some(SPXGREP_TABLES));
    let lock = cli(&spxgrep.root, &["lock", "semaprax.toml"]);
    assert!(lock.status.success(), "{}", stderr(&lock));
    let lock: Value = serde_json::from_str(&stdout(&lock)).unwrap();
    assert_eq!(lock["payload"]["interface"]["kind"], "unproven");
    assert_eq!(lock["payload"]["interface"]["digest"], Value::Null);
    assert_eq!(
        lock["payload"]["package"]["profile"],
        "useful-data-command.v1"
    );
    assert_eq!(
        lock["payload"]["package"]["contract"],
        "semaprax.project.v4"
    );
    assert_eq!(
        lock["payload"]["capabilities"],
        serde_json::json!(["process.stdout.write"])
    );

    let frame = example_fixture("frame-payload", "examples/frame-payload-project", None);
    let lock = cli(&frame.root, &["lock", "semaprax.toml"]);
    assert!(lock.status.success(), "{}", stderr(&lock));
    let lock: Value = serde_json::from_str(&stdout(&lock)).unwrap();
    assert_eq!(
        lock["payload"]["interface"]["kind"],
        "public-owned-data-api.v1"
    );
    assert!(lock["payload"]["interface"]["digest"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
    assert_eq!(
        lock["payload"]["package"]["contract"],
        "semaprax.project.v8"
    );

    for (arguments, fragment) in [
        (&["lock"][..], "lock requires a manifest path"),
        (
            &["lock", "semaprax.toml", "extra.toml"],
            "exactly one manifest path",
        ),
        (
            &["lock", "semaprax.toml", "--write", "--verify"],
            "at most one of `--write`, `--verify`, `--compare`, `--emit-interface`, or `--compare-interface`",
        ),
        (
            &["lock", "semaprax.toml", "--force"],
            "unknown lock option `--force`",
        ),
    ] {
        let output = cli(&frame.root, arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(output.stdout.is_empty());
        assert!(
            stderr(&output).contains(fragment),
            "{arguments:?}: {}",
            stderr(&output)
        );
    }
    assert!(!frame.root.join("semaprax.lock").exists());

    let help = cli(&frame.root, &["lock", "--help"]);
    assert!(help.status.success());
    assert_eq!(
        stdout(&help),
        "Usage:\n  semaprax lock <manifest> [--write|--verify|--compare <baseline.lock>|--emit-interface|--compare-interface <baseline.json>]\n"
    );
}

struct Fixture {
    root: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn example_fixture(label: &str, example: &str, manifest: Option<&str>) -> Fixture {
    let root = std::env::temp_dir().join(format!(
        "semaprax-project-lock-v1-{label}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(example);
    for entry in std::fs::read_dir(source.join("src")).unwrap() {
        let entry = entry.unwrap();
        std::fs::copy(entry.path(), root.join("src").join(entry.file_name())).unwrap();
    }
    match manifest {
        Some(manifest) => std::fs::write(root.join("semaprax.toml"), manifest).unwrap(),
        None => {
            std::fs::copy(source.join("semaprax.toml"), root.join("semaprax.toml")).unwrap();
        }
    }
    Fixture {
        root: root.canonicalize().unwrap(),
    }
}

fn cli(root: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args(arguments)
        .current_dir(root)
        .output()
        .unwrap()
}

// A minimal Project Lock v1 envelope carrying only the fields the compatibility
// classifier reads, for exact branch coverage.
fn synthetic_lock(
    name: &str,
    contract: &str,
    version: &str,
    exports: &[&str],
    interface_digest: &str,
    capabilities: &[&str],
    targets: &[&str],
) -> String {
    serde_json::json!({
        "schema": "semaprax.project-lock.v1",
        "payload": {
            "package": {"name": name, "contract": contract, "version": version},
            "interface": {"exports": exports, "digest": interface_digest},
            "capabilities": capabilities,
            "targets": targets.iter().map(|target| serde_json::json!({"target": target, "state": "declared"})).collect::<Vec<_>>(),
        }
    })
    .to_string()
}

fn classify(base: &str, candidate: &str) -> (bool, Value) {
    let compatibility = semaprax::project::classify_lock_change(base, candidate).unwrap();
    let report: Value = serde_json::from_str(compatibility.report()).unwrap();
    assert_eq!(
        report["verdict"],
        if compatibility.breaking() {
            "breaking"
        } else {
            "compatible"
        }
    );
    (compatibility.breaking(), report)
}

#[test]
fn compare_classifies_the_project_interface_against_a_baseline_lock() {
    let fixture = example_fixture("compare", "examples/calculator-project", None);
    let baseline = fixture.root.join("base.lock");
    std::fs::write(
        &baseline,
        stdout(&cli(&fixture.root, &["lock", "semaprax.toml"])),
    )
    .unwrap();

    // Identical: compatible, exit 0, no changes.
    let same = cli(
        &fixture.root,
        &["lock", "semaprax.toml", "--compare", "base.lock"],
    );
    assert!(same.status.success(), "{}", stderr(&same));
    let report: Value = serde_json::from_str(&stdout(&same)).unwrap();
    assert_eq!(report["schema"], "semaprax.project-lock-compatibility.v1");
    assert_eq!(report["verdict"], "compatible");
    assert!(report["changes"].as_array().unwrap().is_empty());

    // Dropping an export is breaking: the report is still printed to stdout, but
    // the command exits nonzero so a CI gate fails.
    std::fs::write(
        fixture.root.join("semaprax.toml"),
        CALCULATOR_TABLES.replace(", \"calculator.subtract\"", ""),
    )
    .unwrap();
    let broken = cli(
        &fixture.root,
        &["lock", "semaprax.toml", "--compare", "base.lock"],
    );
    assert_eq!(broken.status.code(), Some(1));
    let report: Value = serde_json::from_str(&stdout(&broken)).unwrap();
    assert_eq!(report["verdict"], "breaking");
    let change = &report["changes"][0];
    assert_eq!(change["kind"], "exports-removed");
    assert_eq!(change["classification"], "breaking");
    assert_eq!(change["detail"], "calculator.subtract");

    // Restoring the manifest makes the comparison compatible again; the
    // additive-export and other classifier branches are pinned exactly in
    // `classifier_branches_are_exact`.
    std::fs::write(fixture.root.join("semaprax.toml"), CALCULATOR_TABLES).unwrap();
    assert!(cli(
        &fixture.root,
        &["lock", "semaprax.toml", "--compare", "base.lock"]
    )
    .status
    .success());

    // A missing or foreign baseline fails closed before any verdict.
    let missing = cli(
        &fixture.root,
        &["lock", "semaprax.toml", "--compare", "absent.lock"],
    );
    assert!(!missing.status.success());
    assert!(
        stderr(&missing).contains("SPX-J124"),
        "{}",
        stderr(&missing)
    );
    std::fs::write(baseline.with_file_name("foreign.lock"), "{}").unwrap();
    let foreign = cli(
        &fixture.root,
        &["lock", "semaprax.toml", "--compare", "foreign.lock"],
    );
    assert!(
        stderr(&foreign).contains("does not carry schema"),
        "{}",
        stderr(&foreign)
    );

    let help = cli(&fixture.root, &["lock", "--help"]);
    assert_eq!(
        stdout(&help),
        "Usage:\n  semaprax lock <manifest> [--write|--verify|--compare <baseline.lock>|--emit-interface|--compare-interface <baseline.json>]\n"
    );
    let both = cli(
        &fixture.root,
        &[
            "lock",
            "semaprax.toml",
            "--verify",
            "--compare",
            "base.lock",
        ],
    );
    assert_eq!(both.status.code(), Some(2));
    assert!(stderr(&both).contains("at most one of `--write`, `--verify`, `--compare`, `--emit-interface`, or `--compare-interface`"));
}

#[test]
fn classifier_branches_are_exact() {
    let base = synthetic_lock(
        "pkg",
        "semaprax.project.v8",
        "1.0.0",
        &["pkg.a", "pkg.b"],
        "sha256:aaa",
        &["process.stdout.write"],
        &["native64", "wasm32"],
    );

    // Identical is compatible.
    let (breaking, report) = classify(&base, &base);
    assert!(!breaking);
    assert!(report["changes"].as_array().unwrap().is_empty());

    // Name and contract changes are breaking.
    let (breaking, report) = classify(
        &base,
        &synthetic_lock(
            "other",
            "semaprax.project.v8",
            "1.0.0",
            &["pkg.a", "pkg.b"],
            "sha256:aaa",
            &["process.stdout.write"],
            &["native64", "wasm32"],
        ),
    );
    assert!(breaking);
    assert_eq!(report["changes"][0]["kind"], "package-name");
    let (breaking, _) = classify(
        &base,
        &synthetic_lock(
            "pkg",
            "semaprax.project.v9",
            "1.0.0",
            &["pkg.a", "pkg.b"],
            "sha256:aaa",
            &["process.stdout.write"],
            &["native64", "wasm32"],
        ),
    );
    assert!(breaking);

    // A changed interface digest with the same export set is breaking; a new
    // export is not.
    let (breaking, report) = classify(
        &base,
        &synthetic_lock(
            "pkg",
            "semaprax.project.v8",
            "1.0.0",
            &["pkg.a", "pkg.b"],
            "sha256:bbb",
            &["process.stdout.write"],
            &["native64", "wasm32"],
        ),
    );
    assert!(breaking);
    assert_eq!(report["changes"][0]["kind"], "interface-digest");
    let (breaking, report) = classify(
        &base,
        &synthetic_lock(
            "pkg",
            "semaprax.project.v8",
            "1.0.0",
            &["pkg.a", "pkg.b", "pkg.c"],
            "sha256:aaa",
            &["process.stdout.write"],
            &["native64", "wasm32"],
        ),
    );
    assert!(!breaking);
    assert_eq!(report["changes"][0]["kind"], "exports-added");

    // Widening required capabilities is breaking; narrowing is not.
    let (breaking, report) = classify(
        &base,
        &synthetic_lock(
            "pkg",
            "semaprax.project.v8",
            "1.0.0",
            &["pkg.a", "pkg.b"],
            "sha256:aaa",
            &["process.stderr.write", "process.stdout.write"],
            &["native64", "wasm32"],
        ),
    );
    assert!(breaking);
    assert_eq!(report["changes"][0]["kind"], "capabilities-widened");
    let (breaking, _) = classify(
        &base,
        &synthetic_lock(
            "pkg",
            "semaprax.project.v8",
            "1.0.0",
            &["pkg.a", "pkg.b"],
            "sha256:aaa",
            &[],
            &["native64", "wasm32"],
        ),
    );
    assert!(!breaking);

    // Removing a target is breaking; adding one is not.
    let (breaking, report) = classify(
        &base,
        &synthetic_lock(
            "pkg",
            "semaprax.project.v8",
            "1.0.0",
            &["pkg.a", "pkg.b"],
            "sha256:aaa",
            &["process.stdout.write"],
            &["native64"],
        ),
    );
    assert!(breaking);
    assert_eq!(report["changes"][0]["kind"], "targets-removed");
    let narrower = synthetic_lock(
        "pkg",
        "semaprax.project.v8",
        "1.0.0",
        &["pkg.a", "pkg.b"],
        "sha256:aaa",
        &["process.stdout.write"],
        &["native64"],
    );
    let (breaking, report) = classify(&narrower, &base);
    assert!(!breaking);
    assert_eq!(report["changes"][0]["kind"], "targets-added");

    // A version-only change is informational, not breaking.
    let (breaking, report) = classify(
        &base,
        &synthetic_lock(
            "pkg",
            "semaprax.project.v8",
            "2.0.0",
            &["pkg.a", "pkg.b"],
            "sha256:aaa",
            &["process.stdout.write"],
            &["native64", "wasm32"],
        ),
    );
    assert!(!breaking);
    assert_eq!(report["changes"][0]["kind"], "version");
    assert_eq!(report["changes"][0]["classification"], "informational");

    // A foreign baseline is rejected.
    assert!(semaprax::project::classify_lock_change("{}", &base).is_err());
}

#[test]
fn emit_and_compare_interface_classify_exports_fine_grained() {
    let fixture = example_fixture("wit", "examples/calculator-project", None);

    // Emit the scalar interface descriptor as a baseline.
    let emitted = cli(
        &fixture.root,
        &["lock", "semaprax.toml", "--emit-interface"],
    );
    assert!(emitted.status.success(), "{}", stderr(&emitted));
    let descriptor: Value = serde_json::from_str(&stdout(&emitted)).unwrap();
    assert_eq!(
        descriptor["schema"],
        "semaprax.project.scalar-wit-interface.v1"
    );
    assert_eq!(descriptor["exports"].as_array().unwrap().len(), 6);
    std::fs::write(fixture.root.join("base.wit.json"), stdout(&emitted)).unwrap();

    // Identical: compatible, exit 0, no changes.
    let same = cli(
        &fixture.root,
        &[
            "lock",
            "semaprax.toml",
            "--compare-interface",
            "base.wit.json",
        ],
    );
    assert!(same.status.success(), "{}", stderr(&same));
    let report: Value = serde_json::from_str(&stdout(&same)).unwrap();
    assert_eq!(
        report["schema"],
        "semaprax.project-scalar-wit-compatibility.v1"
    );
    assert_eq!(report["verdict"], "compatible");
    assert!(report["changes"].as_array().unwrap().is_empty());

    // Removing an export is breaking and names the export; exit 1.
    std::fs::write(
        fixture.root.join("semaprax.toml"),
        std::fs::read_to_string(fixture.root.join("semaprax.toml"))
            .unwrap()
            .replace(", \"calculator.subtract\"", ""),
    )
    .unwrap();
    let removed = cli(
        &fixture.root,
        &[
            "lock",
            "semaprax.toml",
            "--compare-interface",
            "base.wit.json",
        ],
    );
    assert_eq!(removed.status.code(), Some(1));
    let report: Value = serde_json::from_str(&stdout(&removed)).unwrap();
    assert_eq!(report["verdict"], "breaking");
    assert_eq!(report["changes"][0]["kind"], "export-removed");
    assert_eq!(report["changes"][0]["export"], "calculator.subtract");

    // Symmetrically, a baseline with fewer exports sees the export as added,
    // which is not breaking (exit 0).
    let base_five = cli(
        &fixture.root,
        &["lock", "semaprax.toml", "--emit-interface"],
    );
    std::fs::write(fixture.root.join("five.wit.json"), stdout(&base_five)).unwrap();
    std::fs::write(
        fixture.root.join("semaprax.toml"),
        std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project/semaprax.toml"),
        )
        .unwrap(),
    )
    .unwrap();
    let added = cli(
        &fixture.root,
        &[
            "lock",
            "semaprax.toml",
            "--compare-interface",
            "five.wit.json",
        ],
    );
    assert!(added.status.success(), "{}", stderr(&added));
    let report: Value = serde_json::from_str(&stdout(&added)).unwrap();
    assert_eq!(report["verdict"], "compatible");
    assert_eq!(report["changes"][0]["kind"], "export-added");
    assert_eq!(report["changes"][0]["export"], "calculator.subtract");

    // A missing or foreign baseline fails closed.
    let missing = cli(
        &fixture.root,
        &[
            "lock",
            "semaprax.toml",
            "--compare-interface",
            "absent.json",
        ],
    );
    assert!(!missing.status.success());
    assert!(
        stderr(&missing).contains("SPX-J124"),
        "{}",
        stderr(&missing)
    );
    std::fs::write(fixture.root.join("foreign.json"), "{}").unwrap();
    let foreign = cli(
        &fixture.root,
        &[
            "lock",
            "semaprax.toml",
            "--compare-interface",
            "foreign.json",
        ],
    );
    assert!(
        stderr(&foreign).contains("does not carry schema"),
        "{}",
        stderr(&foreign)
    );

    // A command project has no scalar WIT interface.
    let spxgrep = example_fixture("wit-cmd", "examples/spxgrep-project", Some(SPXGREP_TABLES));
    let no_interface = cli(
        &spxgrep.root,
        &["lock", "semaprax.toml", "--emit-interface"],
    );
    assert!(!no_interface.status.success());
}
