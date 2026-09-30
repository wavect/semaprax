//! Cases for `semaprax registry`. The front is thin, so these check the
//! three things a thin front can still get wrong: the argument grammar, that
//! every refusal keeps the *owning* module's code rather than a code minted
//! here, and that the front holds none of the authorities its verbs are
//! named after -- no process, no network, no write, and no publication.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::package_registry::wire::render_registry_document;
use semaprax::package_registry::{PublicationStatus, PublishedEntry, RegistrySignature};

use super::*;

static NEXT: AtomicU64 = AtomicU64::new(0);

fn scratch_dir(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-cli-registry-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&path).expect("scratch directory must be creatable");
    path
}

fn write(dir: &std::path::Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, bytes).expect("fixture must be writable");
    path
}

const MEANING: &str = "examples.meaning";

/// One genuinely valid entry: the digest binds the exact Subject-v3 bytes of
/// a real committed example, which is what `SPX-PKR603` checks.
fn entry(version: &str, seed: &str) -> PublishedEntry {
    // This shared driver is tested from both the root and toolchain crates;
    // Cargo's per-package working directory is not the repository root.
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .map(|root| root.join("examples/meaning.spx"))
        .find(|source| source.is_file())
        .expect("committed meaning fixture");
    let report = semaprax::package_report_v2::generate(
        &source,
        &semaprax::package_report_v2::PackageReportV2Options::default(),
    )
    .expect("v2 report fixture");
    let subject_bytes = semaprax::package_lock_v3::create_subject(
        &semaprax::package_lock_v3::Coordinate {
            package: MEANING.to_owned(),
            version: version.to_owned(),
        },
        &report,
        &[],
        &[],
    )
    .expect("v3 subject fixture");
    PublishedEntry {
        package: MEANING.to_owned(),
        version: version.to_owned(),
        content_digest: semaprax::audit_capsule::sha256_digest(subject_bytes.as_bytes()),
        api_digest: semaprax::audit_capsule::sha256_digest(seed.as_bytes()),
        license: "Apache-2.0".to_owned(),
        provenance_digest: None,
        signature: RegistrySignature {
            algorithm: "ed25519".to_owned(),
            identity: "signer.alpha".to_owned(),
            signature: format!("opaque-unverified-{seed}"),
        },
        status: PublicationStatus::Active,
        subject_bytes,
    }
}

fn registry_document(entries: &[PublishedEntry]) -> String {
    render_registry_document(entries)
}

const TEMPLATE: &str = r#"{
    "schema": "semaprax.registry-resolution-template.v1",
    "requirements": [{"package": "examples.meaning", "range": "^1.0.0"}],
    "target": "native64",
    "allowed_capabilities": [],
    "yank_policy": "exclude_yanked",
    "max_bytes": 65536
}"#;

/// A scratch directory holding a two-version registry document and a
/// template, the fixture most cases below start from.
fn fixture(label: &str) -> (PathBuf, PathBuf, PathBuf) {
    let dir = scratch_dir(label);
    let document = registry_document(&[entry("1.0.0", "alpha"), entry("1.2.0", "beta")]);
    let registry = write(&dir, "registry.json", document.as_bytes());
    let template = write(&dir, "template.json", TEMPLATE.as_bytes());
    (dir, registry, template)
}

// ---------------------------------------------------------------------
// `parse`: closed argument grammar.
// ---------------------------------------------------------------------

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn parse_admits_every_documented_shape() {
    assert!(matches!(
        parse(&strings(&["search", "r.json", "examples"])),
        Ok(RegistryCommand::Search(..))
    ));
    assert!(matches!(
        parse(&strings(&["add", "r.json", "examples.meaning", "^1.0.0"])),
        Ok(RegistryCommand::Add(..))
    ));
    assert!(matches!(
        parse(&strings(&["lock", "r.json", "t.json"])),
        Ok(RegistryCommand::Lock(..))
    ));
    assert!(matches!(
        parse(&strings(&["lock", "r.json", "t.json", "--raw"])),
        Ok(RegistryCommand::Lock(..))
    ));
    assert!(matches!(
        parse(&strings(&["fetch", "r.json", "examples.meaning", "1.0.0"])),
        Ok(RegistryCommand::Fetch(..))
    ));
    assert!(matches!(
        parse(&strings(&[
            "fetch",
            "r.json",
            "examples.meaning",
            "1.0.0",
            "--raw"
        ])),
        Ok(RegistryCommand::Fetch(..))
    ));
    assert!(matches!(
        parse(&strings(&["verify", "r.json", "e.json"])),
        Ok(RegistryCommand::VerifySnapshot(..))
    ));
    assert!(matches!(
        parse(&strings(&["verify", "r.json", "t.json", "e.json"])),
        Ok(RegistryCommand::VerifyLock(..))
    ));
    assert!(matches!(
        parse(&strings(&["publish", "r.json", "entry.json"])),
        Ok(RegistryCommand::Publish(..))
    ));
}

#[test]
fn parse_refuses_everything_else_with_exit_code_two() {
    for arguments in [
        vec![],
        strings(&["install", "r.json"]),
        strings(&["search"]),
        strings(&["search", "r.json"]),
        strings(&["search", "r.json", "q", "extra"]),
        strings(&["add", "r.json", "examples.meaning"]),
        strings(&["lock", "r.json"]),
        strings(&["lock", "r.json", "t.json", "--json"]),
        strings(&["lock", "r.json", "--raw", "t.json"]),
        strings(&["lock", "r.json", "t.json", "--raw", "extra"]),
        strings(&["fetch", "r.json", "examples.meaning"]),
        strings(&["fetch", "r.json", "examples.meaning", "1.0.0", "--json"]),
        strings(&["fetch", "r.json", "examples.meaning", "--raw", "1.0.0"]),
        strings(&[
            "fetch",
            "r.json",
            "examples.meaning",
            "1.0.0",
            "--raw",
            "extra",
        ]),
        strings(&["publish", "r.json"]),
        strings(&["publish", "r.json", "e.json", "--force"]),
        strings(&["search", "--registry", "q"]),
        strings(&["fetch", "r.json", "", "1.0.0"]),
    ] {
        assert_eq!(parse(&arguments).err(), Some(2), "{arguments:?}");
    }
}

// ---------------------------------------------------------------------
// The six verbs, on a valid registry.
// ---------------------------------------------------------------------

#[test]
fn search_lists_matching_coordinates_and_says_there_is_no_network() {
    let (_dir, registry, _) = fixture("search");
    let report = run_search(&registry, "meaning").expect("search");
    assert!(report.contains("matches: 2"), "{report}");
    assert!(report.contains("examples.meaning@1.0.0"), "{report}");
    assert!(report.contains("examples.meaning@1.2.0"), "{report}");
    assert!(report.contains("no default registry"), "{report}");
}

#[test]
fn search_reports_no_matches_rather_than_failing() {
    let (_dir, registry, _) = fixture("search-miss");
    let report = run_search(&registry, "nothing-like-this").expect("search");
    assert!(report.contains("matches: 0"), "{report}");
}

/// A query is data, never a path: a traversal-shaped query is compared as an
/// opaque string against package names and matches nothing, rather than
/// being joined with anything or reaching the filesystem.
#[test]
fn a_path_traversal_shaped_query_is_an_opaque_string() {
    let (_dir, registry, _) = fixture("search-traversal");
    let report = run_search(&registry, "../../../../etc/passwd").expect("search");
    assert!(report.contains("matches: 0"), "{report}");
}

#[test]
fn add_selects_the_highest_satisfying_version_and_writes_nothing() {
    let (dir, registry, _) = fixture("add");
    let before = fs::read_dir(&dir).expect("dir").count();
    let report = run_add(&registry, MEANING, "^1.0.0").expect("add");
    assert!(
        report.contains("selected: examples.meaning@1.2.0"),
        "{report}"
    );
    assert!(report.contains("status: SELECTED"), "{report}");
    assert!(report.contains("UNVERIFIED"), "{report}");
    assert_eq!(fs::read_dir(&dir).expect("dir").count(), before);
}

#[test]
fn add_refuses_an_unsatisfiable_range_under_the_registrys_own_code() {
    let (_dir, registry, _) = fixture("add-miss");
    let error = run_add(&registry, MEANING, "^9.0.0").expect_err("refused");
    assert_eq!(error.code, "SPX-PKR601");
}

#[test]
fn lock_emits_a_bound_resolution_that_verify_then_replays() {
    let (dir, registry, template) = fixture("lock");
    let report = run_lock(&registry, &template).expect("lock");
    assert!(report.contains("status: LOCKED"), "{report}");
    let document = report
        .split_once("lock document:\n")
        .expect("lock document")
        .1
        .lines()
        .next()
        .expect("one line")
        .to_owned();
    let evidence = write(&dir, "lock.json", document.as_bytes());
    let verified = run_verify_lock(&registry, &template, &evidence).expect("verify");
    assert!(verified.contains("status: REPLAYED"), "{verified}");
    assert!(verified.contains("examples.meaning@1.2.0"), "{verified}");
}

#[test]
fn raw_lock_is_the_exact_replayable_bound_resolution_bytes() {
    let (dir, registry, template) = fixture("lock-raw");
    let command = parse(&[
        "lock".to_owned(),
        registry.display().to_string(),
        template.display().to_string(),
        "--raw".to_owned(),
    ])
    .expect("raw lock grammar");
    let raw = run(&command).expect("raw lock");
    let (_, snapshot) = read_snapshot(&registry).expect("snapshot");
    let template_document = read_template(&template).expect("template");
    let expected = binding::bind_to_snapshot(
        &snapshot,
        template_document.policy,
        &template_document.template,
        &template_document.options,
    )
    .expect("bound resolution");
    assert_eq!(raw, expected.envelope());
    assert!(
        !raw.ends_with('\n'),
        "raw output adds no redirect-corrupting newline"
    );

    let evidence = write(&dir, "lock.json", raw.as_bytes());
    assert!(run_verify_lock(&registry, &template, &evidence)
        .expect("raw evidence independently replays")
        .contains("status: REPLAYED"));
    let tampered = write(
        &dir,
        "tampered-lock.json",
        raw.replacen("1.2.0", "1.0.0", 1).as_bytes(),
    );
    assert_eq!(
        run_verify_lock(&registry, &template, &tampered)
            .expect_err("tampered raw lock refused")
            .code,
        "SPX-PKR608"
    );
}

#[test]
fn a_tampered_lock_document_is_refused_under_the_binding_layers_own_code() {
    let (dir, registry, template) = fixture("lock-tamper");
    let report = run_lock(&registry, &template).expect("lock");
    let document = report
        .split_once("lock document:\n")
        .expect("lock document")
        .1
        .lines()
        .next()
        .expect("one line")
        .replace("1.2.0", "1.0.0");
    let evidence = write(&dir, "lock.json", document.as_bytes());
    let error = run_verify_lock(&registry, &template, &evidence).expect_err("refused");
    assert_eq!(error.code, "SPX-PKR608");
}

#[test]
fn verify_replays_snapshot_evidence_and_refuses_a_tampered_byte() {
    let (dir, registry, _) = fixture("verify");
    let entries = vec![entry("1.0.0", "alpha"), entry("1.2.0", "beta")];
    let snapshot = semaprax::package_registry::build_snapshot(&entries).expect("snapshot");
    let good = write(&dir, "evidence.json", snapshot.envelope().as_bytes());
    let report = run_verify_snapshot(&registry, &good).expect("verify");
    assert!(report.contains("status: REPLAYED"), "{report}");
    let bad = write(
        &dir,
        "tampered.json",
        snapshot.envelope().replace("Apache-2.0", "MIT").as_bytes(),
    );
    let error = run_verify_snapshot(&registry, &bad).expect_err("refused");
    assert_eq!(error.code, "SPX-PKR608");
}

#[test]
fn fetch_serves_the_exact_published_subject_bytes_offline() {
    let (_dir, registry, _) = fixture("fetch");
    let report = run_fetch(&registry, MEANING, "1.0.0").expect("fetch");
    assert!(report.contains("status: FETCHED"), "{report}");
    assert!(report.contains("status-field: active"), "{report}");
    assert!(
        report.contains(&entry("1.0.0", "alpha").subject_bytes),
        "fetch must serve the published bytes verbatim"
    );
}

#[test]
fn raw_fetch_is_the_exact_replayable_subject_bytes() {
    let (dir, registry, _) = fixture("fetch-raw");
    let command = parse(&[
        "fetch".to_owned(),
        registry.display().to_string(),
        MEANING.to_owned(),
        "1.0.0".to_owned(),
        "--raw".to_owned(),
    ])
    .expect("raw fetch grammar");
    let raw = run(&command).expect("raw fetch");
    let expected = entry("1.0.0", "alpha").subject_bytes;
    assert_eq!(raw, expected);
    assert!(!raw.starts_with("registry fetch:"));

    let subject = write(&dir, "meaning.subject.json", raw.as_bytes());
    let verified = semaprax::package_lock_v3::verify_dependency_subject(
        &fs::read_to_string(&subject).expect("raw subject bytes"),
    )
    .expect("raw fetched bytes independently replay");
    assert_eq!(verified.coordinate.package, MEANING);
    assert_eq!(verified.coordinate.version, "1.0.0");
    let tampered = write(
        &dir,
        "tampered.subject.json",
        raw.replacen("1.0.0", "1.0.1", 1).as_bytes(),
    );
    assert!(semaprax::package_lock_v3::verify_dependency_subject(
        &fs::read_to_string(tampered).expect("tampered raw subject bytes"),
    )
    .is_err());
}

#[test]
fn fetch_refuses_an_unpublished_coordinate_under_this_fronts_own_code() {
    let (_dir, registry, _) = fixture("fetch-miss");
    let error = run_fetch(&registry, MEANING, "9.9.9").expect_err("refused");
    assert_eq!(error.code, "SPX-Z927");
}

// ---------------------------------------------------------------------
// `publish`: decide-and-record, never publication.
// ---------------------------------------------------------------------

#[test]
fn publish_reports_an_admissible_candidate_without_publishing_it() {
    let (dir, registry, _) = fixture("publish");
    let candidate = semaprax::package_registry::wire::render_entry(&entry("1.3.0", "gamma"));
    let path = write(&dir, "entry.json", candidate.as_bytes());
    let before = fs::read(&registry).expect("registry bytes");
    let report = run_publish(&registry, &path).expect("publish");
    assert!(
        report.contains("status: ADMISSIBLE, NOT PUBLISHED"),
        "{report}"
    );
    assert!(report.contains("entries: 2 -> 3"), "{report}");
    assert_eq!(
        fs::read(&registry).expect("registry bytes"),
        before,
        "publish must not rewrite the registry document"
    );
}

#[test]
fn publish_refuses_a_duplicate_under_the_registrys_own_code() {
    let (dir, registry, _) = fixture("publish-duplicate");
    let candidate = semaprax::package_registry::wire::render_entry(&entry("1.0.0", "alpha"));
    let path = write(&dir, "entry.json", candidate.as_bytes());
    let error = run_publish(&registry, &path).expect_err("refused");
    assert_eq!(error.code, "SPX-PKR605");
}

#[test]
fn publish_refuses_a_mutable_replacement_of_a_published_version() {
    let (dir, registry, _) = fixture("publish-conflict");
    let replacement = PublishedEntry {
        api_digest: semaprax::audit_capsule::sha256_digest(b"different-surface"),
        content_digest: semaprax::audit_capsule::sha256_digest(b"different-content"),
        ..entry("1.0.0", "alpha")
    };
    let path = write(
        &dir,
        "entry.json",
        semaprax::package_registry::wire::render_entry(&replacement).as_bytes(),
    );
    let error = run_publish(&registry, &path).expect_err("refused");
    // The digest no longer binds the subject bytes, which is the first rule
    // the registry checks; either way the refusal is the registry's, never
    // this front's.
    assert!(
        error.code == "SPX-PKR603" || error.code == "SPX-PKR604",
        "{}",
        error.code
    );
}

#[test]
fn publish_refuses_the_reserved_namespace_under_the_registrys_own_code() {
    let (dir, registry, _) = fixture("publish-reserved");
    let reserved = PublishedEntry {
        package: "std.core".to_owned(),
        ..entry("1.0.0", "alpha")
    };
    let path = write(
        &dir,
        "entry.json",
        semaprax::package_registry::wire::render_entry(&reserved).as_bytes(),
    );
    let error = run_publish(&registry, &path).expect_err("refused");
    assert_eq!(error.code, "SPX-PKR602");
}

// ---------------------------------------------------------------------
// Document-boundary failures keep this front's own code.
// ---------------------------------------------------------------------

#[test]
fn a_missing_document_is_this_fronts_own_read_failure() {
    let dir = scratch_dir("missing");
    let error = run_search(&dir.join("absent.json"), "x").expect_err("refused");
    assert_eq!(error.code, "SPX-Z926");
}

#[test]
fn a_non_utf8_document_is_refused_before_any_decode() {
    let dir = scratch_dir("non-utf8");
    let path = write(&dir, "registry.json", &[0xff, 0xfe, 0xfd]);
    let error = run_search(&path, "x").expect_err("refused");
    assert_eq!(error.code, "SPX-Z926");
}

#[test]
fn a_malformed_registry_document_keeps_the_decoders_own_code() {
    let dir = scratch_dir("malformed");
    let path = write(&dir, "registry.json", b"{\"schema\":\"other\"}");
    let error = run_search(&path, "x").expect_err("refused");
    assert_eq!(error.code, "SPX-PKR613");
}

// ---------------------------------------------------------------------
// Determinism and authority.
// ---------------------------------------------------------------------

#[test]
fn the_same_registry_produces_byte_identical_reports() {
    let (_dir, registry, template) = fixture("determinism");
    assert_eq!(
        run_search(&registry, "examples").expect("search"),
        run_search(&registry, "examples").expect("search")
    );
    assert_eq!(
        run_lock(&registry, &template).expect("lock"),
        run_lock(&registry, &template).expect("lock")
    );
}

/// This front is named after six verbs that in every other package manager
/// touch the network, the filesystem and a publication endpoint. It touches
/// none of them, and that is checked mechanically rather than asserted in
/// prose, so it cannot rot silently.
#[test]
fn this_front_spawns_no_process_reaches_no_network_and_writes_nothing() {
    let source = include_str!("../registry.rs");
    assert!(!source.contains("std::process::Command"));
    assert!(!source.contains("TcpStream"));
    assert!(!source.contains("std::net::"));
    assert!(!source.contains("reqwest"));
    assert!(!source.contains("fs::write"));
    assert!(!source.contains("fs::remove"));
    assert!(!source.contains("fs::create_dir"));
    assert!(!source.contains("OpenOptions"));
}
