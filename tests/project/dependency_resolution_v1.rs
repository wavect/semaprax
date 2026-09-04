//! Project Dependency Resolution v1: `semaprax resolve` selects a manifest's
//! `[dependencies]` against a local content-addressed cache of Subject-v3
//! envelopes, deterministically and with no registry or build. The cache
//! fixtures are built from real example sources through the same library API
//! the envelope resolver uses.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::package_lock_v3::{self, Coordinate, DependencyRequirement};
use semaprax::package_report_v2::{self, PackageReportV2Options};
use serde_json::Value;

static SERIAL: AtomicU64 = AtomicU64::new(0);

fn report(spx: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(spx);
    package_report_v2::generate(&path, &PackageReportV2Options::default())
        .unwrap_or_else(|error| panic!("report for {spx}: {error:?}"))
}

fn subject(package: &str, version: &str, report: &str, dependencies: &[(&str, &str)]) -> String {
    package_lock_v3::create_subject(
        &Coordinate {
            package: package.to_owned(),
            version: version.to_owned(),
        },
        report,
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

/// Write a subject into the cache content-addressed: file `<hex>.json` where
/// the envelope digest is `sha256:<hex>`.
fn cache_subject(cache: &Path, subject: &str) -> String {
    let envelope: Value = serde_json::from_str(subject).unwrap();
    let digest = envelope["digest"].as_str().unwrap();
    let hex = digest.strip_prefix("sha256:").unwrap();
    std::fs::write(cache.join(format!("{hex}.json")), subject).unwrap();
    hex.to_owned()
}

fn manifest(dependencies: &str, targets: Option<&str>) -> String {
    let mut text = String::from(
        "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"consumer\"\nversion = \"0.1.0\"\n\n[modules]\nentry = \"consumer.app\"\nsources = [\"src/app.spx\", \"src/core.spx\", \"src/tests.spx\"]\ntests = [\"consumer.tests\"]\n\n[exports]\nweb = [\"calculator.add\"]\n",
    );
    if !dependencies.is_empty() {
        text.push_str(&format!("\n[dependencies]\n{dependencies}"));
    }
    if let Some(matrix) = targets {
        text.push_str(&format!("\n[targets]\nmatrix = {matrix}\n"));
    }
    text
}

struct Fixture {
    root: PathBuf,
    cache: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn fixture(label: &str, manifest_text: &str) -> Fixture {
    let root = std::env::temp_dir().join(format!(
        "semaprax-dependency-resolution-v1-{label}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    // The consumer's own sources are never read by `resolve`; a bare `.spx`
    // keeps the fixture a real directory without needing a buildable project.
    for file in ["app.spx", "core.spx", "tests.spx"] {
        std::fs::write(root.join("src").join(file), "module consumer.stub;\n").unwrap();
    }
    std::fs::write(root.join("semaprax.toml"), manifest_text).unwrap();
    let cache = root.join("cache");
    std::fs::create_dir_all(&cache).unwrap();
    let root = root.canonicalize().unwrap();
    let cache = root.join("cache");
    Fixture { root, cache }
}

fn cli(root: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args(arguments)
        .current_dir(root)
        .output()
        .unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn resolve_selects_the_transitive_closure_from_the_cache_deterministically() {
    let meaning = report("examples/meaning.spx");
    let calculator = report("examples/calculator.spx");
    let fixture = fixture(
        "closure",
        &manifest("examples.meaning = \"^1.0.0\"\n", None),
    );
    // Two versions of the root dependency and one transitive dependency.
    cache_subject(
        &fixture.cache,
        &subject("examples.meaning", "1.0.0", &meaning, &[]),
    );
    cache_subject(
        &fixture.cache,
        &subject(
            "examples.meaning",
            "1.1.0",
            &meaning,
            &[("examples.calculator", "^2.0.0")],
        ),
    );
    cache_subject(
        &fixture.cache,
        &subject("examples.calculator", "2.0.0", &calculator, &[]),
    );

    let output = cli(
        &fixture.root,
        &[
            "resolve",
            "semaprax.toml",
            "--target",
            "native64",
            "--cache",
            "cache",
        ],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    let evidence: Value =
        serde_json::from_str(&String::from_utf8(output.stdout.clone()).unwrap()).unwrap();
    assert_eq!(
        evidence["schema"],
        "semaprax.offline-package-resolution-evidence.v2"
    );
    let selected = evidence["payload"]["selected"].as_array().unwrap();
    let mut coordinates = selected
        .iter()
        .map(|row| {
            (
                row["package"].as_str().unwrap().to_owned(),
                row["version"].as_str().unwrap().to_owned(),
            )
        })
        .collect::<Vec<_>>();
    coordinates.sort();
    assert_eq!(
        coordinates,
        [
            ("examples.calculator".to_owned(), "2.0.0".to_owned()),
            ("examples.meaning".to_owned(), "1.1.0".to_owned()),
        ],
        "the caret range picks 1.1.0, which pulls in the transitive calculator"
    );

    // Deterministic: a second run is byte-identical regardless of cache order.
    let again = cli(
        &fixture.root,
        &[
            "resolve",
            "semaprax.toml",
            "--target",
            "native64",
            "--cache",
            "cache",
        ],
    );
    assert_eq!(output.stdout, again.stdout);
}

#[test]
fn resolve_fails_closed_on_missing_deps_bad_content_address_and_target() {
    let meaning = report("examples/meaning.spx");

    // A manifest with no [dependencies] has nothing to resolve.
    let empty = fixture("empty", &manifest("", None));
    let output = cli(
        &empty.root,
        &[
            "resolve",
            "semaprax.toml",
            "--target",
            "native64",
            "--cache",
            "cache",
        ],
    );
    assert!(!output.status.success());
    assert!(stderr(&output).contains("SPX-J126"), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("declares none"),
        "{}",
        stderr(&output)
    );

    // A subject filed under the wrong content address is an integrity failure.
    let tampered = fixture(
        "tampered",
        &manifest("examples.meaning = \"^1.0.0\"\n", None),
    );
    std::fs::write(
        tampered.cache.join("deadbeef.json"),
        subject("examples.meaning", "1.0.0", &meaning, &[]),
    )
    .unwrap();
    let output = cli(
        &tampered.root,
        &[
            "resolve",
            "semaprax.toml",
            "--target",
            "native64",
            "--cache",
            "cache",
        ],
    );
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("is not content-addressed"),
        "{}",
        stderr(&output)
    );

    // A target outside the declared matrix is rejected before resolution.
    let bounded = fixture(
        "matrix",
        &manifest("examples.meaning = \"^1.0.0\"\n", Some("[\"wasm32\"]")),
    );
    cache_subject(
        &bounded.cache,
        &subject("examples.meaning", "1.0.0", &meaning, &[]),
    );
    let output = cli(
        &bounded.root,
        &[
            "resolve",
            "semaprax.toml",
            "--target",
            "native64",
            "--cache",
            "cache",
        ],
    );
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("outside the manifest `[targets] matrix`"),
        "{}",
        stderr(&output)
    );
    // The declared target resolves.
    let output = cli(
        &bounded.root,
        &[
            "resolve",
            "semaprax.toml",
            "--target",
            "wasm32",
            "--cache",
            "cache",
        ],
    );
    assert!(output.status.success(), "{}", stderr(&output));

    // A missing cache directory and a bad target value are reported.
    let missing = fixture(
        "missing",
        &manifest("examples.meaning = \"^1.0.0\"\n", None),
    );
    std::fs::remove_dir_all(&missing.cache).unwrap();
    let output = cli(
        &missing.root,
        &[
            "resolve",
            "semaprax.toml",
            "--target",
            "native64",
            "--cache",
            "cache",
        ],
    );
    assert!(!output.status.success());
    assert!(stderr(&output).contains("SPX-J126"), "{}", stderr(&output));

    let bad_target = cli(
        &missing.root,
        &[
            "resolve",
            "semaprax.toml",
            "--target",
            "x86",
            "--cache",
            "cache",
        ],
    );
    assert_eq!(bad_target.status.code(), Some(2));
    assert!(
        stderr(&bad_target).contains("admits only"),
        "{}",
        stderr(&bad_target)
    );
}

#[test]
fn resolve_usage_and_help_are_exact() {
    let fixture = fixture("usage", &manifest("examples.meaning = \"^1.0.0\"\n", None));
    for (arguments, fragment) in [
        (
            &["resolve", "semaprax.toml", "extra.toml"][..],
            "at most one manifest path",
        ),
        (
            &["resolve", "semaprax.toml", "--cache", "cache"],
            "requires `--target",
        ),
        (
            &["resolve", "semaprax.toml", "--target", "native64"],
            "requires `--cache",
        ),
        (
            &[
                "resolve",
                "semaprax.toml",
                "--target",
                "native64",
                "--cache",
                "cache",
                "--nope",
            ],
            "unknown resolve option `--nope`",
        ),
    ] {
        let output = cli(&fixture.root, arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(output.stdout.is_empty());
        assert!(
            stderr(&output).contains(fragment),
            "{arguments:?}: {}",
            stderr(&output)
        );
    }
    let help = cli(&fixture.root, &["resolve", "--help"]);
    assert!(help.status.success());
    assert_eq!(
        String::from_utf8(help.stdout).unwrap(),
        "Usage:\n  semaprax resolve [<dir>|semaprax.toml] --target <native64|wasm32> --cache <dir> [--write|--verify] [--max-bytes N]\n"
    );
}

#[test]
fn write_pins_the_resolution_and_verify_fails_closed_on_cache_drift() {
    let meaning = report("examples/meaning.spx");
    let primary = fixture("pin", &manifest("examples.meaning = \"^1.0.0\"\n", None));
    cache_subject(
        &primary.cache,
        &subject("examples.meaning", "1.0.0", &meaning, &[]),
    );
    let base = [
        "resolve",
        "semaprax.toml",
        "--target",
        "native64",
        "--cache",
        "cache",
    ];

    // Verifying before a pin exists reports the missing file.
    let missing = cli(&primary.root, &[base.as_slice(), &["--verify"]].concat());
    assert!(!missing.status.success());
    assert!(
        stderr(&missing).contains("SPX-J126"),
        "{}",
        stderr(&missing)
    );
    assert!(
        stderr(&missing).contains("is not present"),
        "{}",
        stderr(&missing)
    );

    // --write pins the exact resolver evidence beside the manifest.
    let written = cli(&primary.root, &[base.as_slice(), &["--write"]].concat());
    assert!(written.status.success(), "{}", stderr(&written));
    assert_eq!(
        String::from_utf8(written.stdout.clone()).unwrap(),
        "wrote semaprax.resolution-native64.json for consumer (native64)\n"
    );
    let pinned = primary.root.join("semaprax.resolution-native64.json");
    let stored = std::fs::read_to_string(&pinned).unwrap();
    let printed = String::from_utf8(cli(&primary.root, &base).stdout).unwrap();
    assert_eq!(
        stored,
        printed.strip_suffix('\n').unwrap(),
        "the pin is exactly the printed evidence without the trailing newline"
    );
    assert!(std::fs::read_dir(&primary.root).unwrap().all(|entry| !entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .contains("staging")));

    // --verify against the unchanged cache passes.
    let verified = cli(&primary.root, &[base.as_slice(), &["--verify"]].concat());
    assert!(verified.status.success(), "{}", stderr(&verified));
    assert_eq!(
        String::from_utf8(verified.stdout.clone()).unwrap(),
        "verified semaprax.resolution-native64.json for consumer (native64)\n"
    );

    // Adding a higher version to the cache changes the selection, so the pin is
    // now stale and --verify fails closed without rewriting it.
    cache_subject(
        &primary.cache,
        &subject("examples.meaning", "1.1.0", &meaning, &[]),
    );
    let stale = cli(&primary.root, &[base.as_slice(), &["--verify"]].concat());
    assert!(!stale.status.success());
    assert!(stderr(&stale).contains("is stale"), "{}", stderr(&stale));
    assert_eq!(
        std::fs::read_to_string(&pinned).unwrap(),
        stored,
        "--verify never rewrites"
    );

    // Re-pinning records the new selection, and --verify passes again.
    assert!(
        cli(&primary.root, &[base.as_slice(), &["--write"]].concat())
            .status
            .success()
    );
    assert_ne!(std::fs::read_to_string(&pinned).unwrap(), stored);
    assert!(
        cli(&primary.root, &[base.as_slice(), &["--verify"]].concat())
            .status
            .success()
    );

    // The pin is per target: a wasm32 pin is a distinct file.
    let bounded = fixture(
        "pin-wasm",
        &manifest("examples.meaning = \"^1.0.0\"\n", Some("[\"wasm32\"]")),
    );
    cache_subject(
        &bounded.cache,
        &subject("examples.meaning", "1.0.0", &meaning, &[]),
    );
    assert!(cli(
        &bounded.root,
        &[
            "resolve",
            "semaprax.toml",
            "--target",
            "wasm32",
            "--cache",
            "cache",
            "--write"
        ]
    )
    .status
    .success());
    assert!(bounded
        .root
        .join("semaprax.resolution-wasm32.json")
        .is_file());
    assert!(!bounded
        .root
        .join("semaprax.resolution-native64.json")
        .exists());
}
