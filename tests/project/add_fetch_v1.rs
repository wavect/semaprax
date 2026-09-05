//! `semaprax add` and `semaprax fetch`: the explicit, network-free dependency
//! steps around `resolve`. `add` rewrites a Package Manifest v1 table manifest
//! canonically with one more `[dependencies]` row and leaves it untouched on
//! any rejection; `fetch` replays Subject-v3 envelopes and files them into the
//! content-addressed cache by digest, so `resolve` can select them.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::package_lock_v3::{self, Coordinate, DependencyRequirement};
use semaprax::package_report_v2::{self, PackageReportV2Options};
use semaprax::project::ProjectManifest;
use serde_json::Value;

static SERIAL: AtomicU64 = AtomicU64::new(0);

const MANIFEST: &str = "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"consumer\"\nversion = \"0.1.0\"\n\n[modules]\nentry = \"consumer.app\"\nsources = [\"src/app.spx\", \"src/core.spx\", \"src/tests.spx\"]\ntests = [\"consumer.tests\"]\n\n[exports]\nweb = [\"calculator.add\"]\n";

fn report(spx: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(spx);
    package_report_v2::generate(&path, &PackageReportV2Options::default())
        .unwrap_or_else(|error| panic!("report for {spx}: {error:?}"))
}

fn subject(package: &str, version: &str, spx: &str, dependencies: &[(&str, &str)]) -> String {
    package_lock_v3::create_subject(
        &Coordinate {
            package: package.to_owned(),
            version: version.to_owned(),
        },
        &report(spx),
        &dependencies
            .iter()
            .map(|(package, range)| DependencyRequirement {
                package: (*package).to_owned(),
                range: (*range).to_owned(),
            })
            .collect::<Vec<_>>(),
        &[],
    )
    .unwrap_or_else(|error| panic!("subject {package}@{version}: {error:?}"))
}

fn digest_hex(subject: &str) -> String {
    let envelope: Value = serde_json::from_str(subject).unwrap();
    envelope["digest"]
        .as_str()
        .unwrap()
        .strip_prefix("sha256:")
        .unwrap()
        .to_owned()
}

struct Fixture {
    root: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn fixture(label: &str, manifest: &str) -> Fixture {
    let root = std::env::temp_dir().join(format!(
        "semaprax-add-fetch-v1-{label}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    for file in ["app.spx", "core.spx", "tests.spx"] {
        std::fs::write(root.join("src").join(file), "module consumer.stub;\n").unwrap();
    }
    std::fs::write(root.join("semaprax.toml"), manifest).unwrap();
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

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap()
}

fn manifest_text(root: &Path) -> String {
    std::fs::read_to_string(root.join("semaprax.toml")).unwrap()
}

#[test]
fn add_appends_sorted_rows_canonically_and_rejects_without_writing() {
    let fixture = fixture("add", MANIFEST);
    let root = &fixture.root;
    let first = cli(root, &["add", ".", "examples.meaning", "^1.0.0"]);
    assert!(first.status.success(), "{}", text(&first.stderr));
    assert_eq!(
        text(&first.stdout),
        format!(
            "added examples.meaning = \"^1.0.0\" to {}\n",
            root.join("semaprax.toml").display()
        )
    );
    let after_first = manifest_text(root);
    assert_eq!(
        after_first,
        format!("{MANIFEST}\n[dependencies]\nexamples.meaning = \"^1.0.0\"\n")
    );
    ProjectManifest::parse(&after_first).unwrap();

    // A second row is byte-sorted before the first; the manifest path form works too.
    let second = cli(
        root,
        &["add", "semaprax.toml", "examples.calculator", "=2.0.0"],
    );
    assert!(second.status.success(), "{}", text(&second.stderr));
    let after_second = manifest_text(root);
    assert!(after_second.ends_with(
        "[dependencies]\nexamples.calculator = \"=2.0.0\"\nexamples.meaning = \"^1.0.0\"\n"
    ));
    ProjectManifest::parse(&after_second).unwrap();

    for (arguments, code) in [
        (&["add", ".", "examples.meaning", "~1.0.0"][..], "SPX-J127"),
        (&["add", ".", "examples.other", "1.0"][..], "SPX-J100"),
        (&["add", ".", "Examples.Other", "^1.0.0"][..], "SPX-J100"),
        (
            &[
                "add",
                root.join("missing").to_str().unwrap(),
                "a.b",
                "^1.0.0",
            ][..],
            "SPX-I001",
        ),
    ] {
        let rejected = cli(root, arguments);
        assert_eq!(rejected.status.code(), Some(1), "{arguments:?}");
        assert!(rejected.stdout.is_empty(), "{arguments:?}");
        assert!(
            text(&rejected.stderr).contains(code),
            "{arguments:?}: {}",
            text(&rejected.stderr)
        );
        assert_eq!(manifest_text(root), after_second, "{arguments:?}");
    }
    for arguments in [
        &["add"][..],
        &["add", "."][..],
        &["add", ".", "a.b"][..],
        &["add", ".", "a.b", "^1.0.0", "extra"][..],
        &["add", ".", "--name", "^1.0.0"][..],
    ] {
        let output = cli(root, arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn add_requires_the_table_layout() {
    let calculator = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
    let fixture = fixture(
        "frozen",
        &std::fs::read_to_string(calculator.join("semaprax.toml")).unwrap(),
    );
    let before = manifest_text(&fixture.root);
    let output = cli(&fixture.root, &["add", ".", "examples.meaning", "^1.0.0"]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("SPX-J127") && stderr.contains("--layout tables"),
        "{stderr}"
    );
    assert_eq!(manifest_text(&fixture.root), before);
}

#[test]
fn fetch_files_replayed_subjects_by_digest_and_resolve_selects_them() {
    let fixture = fixture(
        "fetch",
        &format!("{MANIFEST}\n[dependencies]\nexamples.meaning = \"^1.0.0\"\n\n[targets]\nmatrix = [\"native64\"]\n"),
    );
    let root = &fixture.root;
    let meaning = subject("examples.meaning", "1.0.0", "examples/meaning.spx", &[]);
    let calculator = subject(
        "examples.calculator",
        "2.0.0",
        "examples/calculator.spx",
        &[],
    );
    std::fs::create_dir_all(root.join("inbox")).unwrap();
    std::fs::write(root.join("inbox/meaning.json"), &meaning).unwrap();
    std::fs::write(root.join("inbox/calculator.json"), &calculator).unwrap();

    let fetched = cli(
        root,
        &[
            "fetch",
            "cache",
            "inbox/meaning.json",
            "inbox/calculator.json",
        ],
    );
    assert!(fetched.status.success(), "{}", text(&fetched.stderr));
    let receipt: Value = serde_json::from_str(&text(&fetched.stdout)).unwrap();
    assert_eq!(receipt["schema"], "semaprax.fetch-receipt.v1");
    assert_eq!(receipt["cache"], "cache");
    assert_eq!(receipt["subjects"][0]["package"], "examples.meaning");
    assert_eq!(receipt["subjects"][0]["version"], "1.0.0");
    assert_eq!(receipt["subjects"][0]["state"], "added");
    assert_eq!(receipt["subjects"][1]["package"], "examples.calculator");
    assert_eq!(receipt["subjects"][1]["state"], "added");
    let meaning_hex = digest_hex(&meaning);
    assert_eq!(
        receipt["subjects"][0]["digest"],
        format!("sha256:{meaning_hex}")
    );
    assert_eq!(
        std::fs::read_to_string(root.join("cache").join(format!("{meaning_hex}.json"))).unwrap(),
        meaning
    );
    let mut names: Vec<_> = std::fs::read_dir(root.join("cache"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&format!("{meaning_hex}.json")));

    // Refetching is idempotent and says so; the cache is unchanged.
    let again = cli(root, &["fetch", "cache", "inbox/meaning.json"]);
    assert!(again.status.success());
    let receipt: Value = serde_json::from_str(&text(&again.stdout)).unwrap();
    assert_eq!(receipt["subjects"][0]["state"], "present");

    // The fetched cache is exactly what resolve reads.
    let resolved = cli(
        root,
        &["resolve", ".", "--target", "native64", "--cache", "cache"],
    );
    assert!(resolved.status.success(), "{}", text(&resolved.stderr));
    let evidence: Value = serde_json::from_str(&text(&resolved.stdout)).unwrap();
    assert!(text(&resolved.stdout).contains("examples.meaning"));
    assert!(evidence["schema"]
        .as_str()
        .unwrap()
        .starts_with("semaprax."));
}

#[test]
fn fetch_rejects_tampered_subjects_and_content_address_collisions_before_writing() {
    let fixture = fixture("closed", MANIFEST);
    let root = &fixture.root;
    let meaning = subject("examples.meaning", "1.0.0", "examples/meaning.spx", &[]);
    std::fs::create_dir_all(root.join("inbox")).unwrap();
    std::fs::write(root.join("inbox/good.json"), &meaning).unwrap();
    std::fs::write(
        root.join("inbox/tampered.json"),
        meaning.replacen("\"version\":\"1.0.0\"", "\"version\":\"1.0.1\"", 1),
    )
    .unwrap();
    std::fs::write(root.join("inbox/not.json"), "{\"schema\":\"other\"}\n").unwrap();

    // A rejected operand rejects the run: the good subject beside it is not filed.
    for bad in [
        "inbox/tampered.json",
        "inbox/not.json",
        "inbox/missing.json",
    ] {
        let output = cli(root, &["fetch", "cache", "inbox/good.json", bad]);
        assert_eq!(output.status.code(), Some(1), "{bad}");
        assert!(output.stdout.is_empty(), "{bad}");
        assert!(!text(&output.stderr).is_empty(), "{bad}");
        assert!(!root.join("cache").exists(), "{bad}");
    }

    // An existing entry with different bytes at the same address is a collision.
    let hex = digest_hex(&meaning);
    std::fs::create_dir_all(root.join("cache")).unwrap();
    std::fs::write(root.join("cache").join(format!("{hex}.json")), "{}\n").unwrap();
    let collision = cli(root, &["fetch", "cache", "inbox/good.json"]);
    assert_eq!(collision.status.code(), Some(1));
    assert!(text(&collision.stderr).contains("SPX-J128"));
    assert_eq!(
        std::fs::read_to_string(root.join("cache").join(format!("{hex}.json"))).unwrap(),
        "{}\n"
    );
    for arguments in [
        &["fetch"][..],
        &["fetch", "cache"][..],
        &["fetch", "cache", "--json"][..],
    ] {
        let output = cli(root, arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
    }
}
