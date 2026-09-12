use std::path::Path;

use sha2::Sha256;

use super::*;

// `create_subject` cross-checks its `coordinate.package` against the
// package name embedded in the replayed Report-v2 (`module examples.meaning;`
// / `module examples.calculator;` -- see the two fixture files), so every
// *genuinely* valid Subject-v3 fixture in this file must use one of these
// two exact package identities. Where a test needs an ordinary, non-`std.*`
// package name at all (the reserved-namespace and coordinate-mismatch
// tests), it starts from one of these valid entries and overrides only the
// registry-level `package`/`version` fields the check under test actually
// looks at -- never conjuring a self-consistent-but-differently-named
// Subject-v3 fixture, which `create_subject` would refuse to build.
const MEANING: &str = "examples.meaning";
const CALCULATOR: &str = "examples.calculator";

fn report(path: &str) -> String {
    crate::package_report_v2::generate(
        Path::new(path),
        &crate::package_report_v2::PackageReportV2Options::default(),
    )
    .expect("v2 report fixture")
}

fn subject_with_capabilities(
    package: &str,
    version: &str,
    report: &str,
    capabilities: &[&str],
) -> String {
    package_lock_v3::create_subject(
        &package_lock_v3::Coordinate {
            package: package.to_owned(),
            version: version.to_owned(),
        },
        report,
        &[],
        &capabilities
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>(),
    )
    .expect("v2 subject fixture")
}

fn fake_digest(seed: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(seed.as_bytes());
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

fn entry_from(package: &str, version: &str, subject_bytes: String, seed: &str) -> PublishedEntry {
    let content_digest = crate::audit_capsule::sha256_digest(subject_bytes.as_bytes());
    PublishedEntry {
        package: package.to_owned(),
        version: version.to_owned(),
        content_digest,
        api_digest: fake_digest(&format!("api-surface:{seed}")),
        license: "Apache-2.0".to_owned(),
        provenance_digest: None,
        signature: RegistrySignature {
            algorithm: "ed25519".to_owned(),
            identity: format!("signer.{seed}"),
            signature: format!("opaque-unverified-signature-bytes-{seed}"),
        },
        status: PublicationStatus::Active,
        subject_bytes,
    }
}

fn yanked(entry: PublishedEntry, reason: &str) -> PublishedEntry {
    PublishedEntry {
        status: PublicationStatus::Yanked {
            reason: reason.to_owned(),
        },
        ..entry
    }
}

/// A fully valid, self-consistent `examples.meaning@version` entry.
fn meaning_entry(version: &str, seed: &str) -> PublishedEntry {
    let subject_bytes =
        subject_with_capabilities(MEANING, version, &report("examples/meaning.spx"), &[]);
    entry_from(MEANING, version, subject_bytes, seed)
}

/// A fully valid, self-consistent `examples.calculator@version` entry.
fn calculator_entry(version: &str, seed: &str) -> PublishedEntry {
    let subject_bytes =
        subject_with_capabilities(CALCULATOR, version, &report("examples/calculator.spx"), &[]);
    entry_from(CALCULATOR, version, subject_bytes, seed)
}

// --- Determinism -------------------------------------------------------------

#[test]
fn same_entries_produce_byte_identical_snapshots_every_time() {
    let entries = vec![meaning_entry("1.0.0", "alpha")];
    let first = build_snapshot(&entries).expect("first build");
    let second = build_snapshot(&entries).expect("second build");
    assert_eq!(first.envelope(), second.envelope());
    assert_eq!(first.digest(), second.digest());
}

#[test]
fn entry_order_does_not_affect_canonical_bytes() {
    let meaning = meaning_entry("1.0.0", "alpha");
    let calculator = calculator_entry("2.0.0", "beta");
    let forward = build_snapshot(&[meaning.clone(), calculator.clone()]).expect("forward build");
    let reversed = build_snapshot(&[calculator, meaning]).expect("reversed build");
    assert_eq!(forward.envelope(), reversed.envelope());
    assert_eq!(forward.digest(), reversed.digest());
}

#[test]
fn one_changed_byte_changes_the_digest() {
    let baseline = meaning_entry("1.0.0", "alpha");
    let mut changed = baseline.clone();
    changed.api_digest = fake_digest("a completely different api surface");
    let baseline_snapshot = build_snapshot(&[baseline]).expect("baseline build");
    let changed_snapshot = build_snapshot(&[changed]).expect("changed build");
    assert_ne!(baseline_snapshot.digest(), changed_snapshot.digest());
    assert_ne!(baseline_snapshot.envelope(), changed_snapshot.envelope());
}

#[test]
fn verify_snapshot_round_trips_exactly() {
    let entries = vec![meaning_entry("1.0.0", "alpha")];
    let snapshot = build_snapshot(&entries).expect("build");
    let verified = verify_snapshot(snapshot.envelope(), &entries).expect("verify");
    assert_eq!(verified.digest, snapshot.digest());
    assert_eq!(
        verified.coordinates,
        vec![(MEANING.to_owned(), "1.0.0".to_owned())]
    );
}

#[test]
fn verify_snapshot_refuses_a_single_tampered_byte() {
    let entries = vec![meaning_entry("1.0.0", "alpha")];
    let snapshot = build_snapshot(&entries).expect("build");
    // Flip one ASCII hex digit inside the rendered digest field -- still
    // valid JSON shape and valid UTF-8, just one wrong byte.
    let position =
        snapshot.envelope().find("\"digest\":\"sha256:").unwrap() + "\"digest\":\"sha256:".len();
    let mut bytes = snapshot.envelope().as_bytes().to_vec();
    bytes[position] = if bytes[position] == b'0' { b'1' } else { b'0' };
    let tampered = String::from_utf8(bytes).expect("ASCII hex digit swap stays valid UTF-8");
    assert_eq!(
        verify_snapshot(&tampered, &entries).unwrap_err().code,
        "SPX-PKR608"
    );
}

#[test]
fn verify_snapshot_refuses_stale_evidence_after_entries_change() {
    let entries_v1 = vec![meaning_entry("1.0.0", "alpha")];
    let snapshot_v1 = build_snapshot(&entries_v1).expect("build v1");
    // Confirm the v1 evidence is genuinely valid against its own entries
    // first, so the refusal below is about staleness, not a malformed
    // baseline.
    assert!(verify_snapshot(snapshot_v1.envelope(), &entries_v1).is_ok());
    let entries_v2 = vec![
        meaning_entry("1.0.0", "alpha"),
        calculator_entry("2.0.0", "beta"),
    ];
    assert_eq!(
        verify_snapshot(snapshot_v1.envelope(), &entries_v2)
            .unwrap_err()
            .code,
        "SPX-PKR608"
    );
}

// --- Immutability ------------------------------------------------------------

#[test]
fn a_single_publish_of_one_coordinate_succeeds() {
    let entries = vec![meaning_entry("1.0.0", "alpha")];
    assert!(build_snapshot(&entries).is_ok());
}

#[test]
fn republishing_the_identical_digest_is_refused_as_duplicate() {
    let one = meaning_entry("1.0.0", "alpha");
    // Control: the single copy alone is admitted, so the failure below is
    // caused by the second occurrence, not by this entry's own shape.
    assert!(build_snapshot(&[one.clone()]).is_ok());
    let entries = vec![one.clone(), one];
    assert_eq!(build_snapshot(&entries).unwrap_err().code, "SPX-PKR605");
}

#[test]
fn republishing_a_different_digest_under_the_same_coordinate_is_refused_as_immutable_conflict() {
    let original = meaning_entry("1.0.0", "alpha");
    assert!(build_snapshot(&[original.clone()]).is_ok());
    // Same declared coordinate (`examples.meaning@1.0.0`), but a genuinely
    // different, still self-consistent, correctly digest-bound package body
    // (a nonempty capability list changes the Subject-v3 bytes).
    let conflicting_subject = subject_with_capabilities(
        MEANING,
        "1.0.0",
        &report("examples/meaning.spx"),
        &["process.stdout.write"],
    );
    let conflicting = entry_from(MEANING, "1.0.0", conflicting_subject, "beta-body");
    assert_ne!(conflicting.content_digest, original.content_digest);
    let entries = vec![original, conflicting];
    assert_eq!(build_snapshot(&entries).unwrap_err().code, "SPX-PKR604");
}

// --- Dependency confusion / reserved namespace --------------------------------

#[test]
fn an_ordinary_namespace_is_admitted() {
    // `examples.meaning` is an ordinary, non-`std.*` package name, and a
    // fully valid entry under it is admitted -- the control every
    // reserved-namespace refusal below is compared against.
    let entries = vec![meaning_entry("1.0.0", "control")];
    assert!(build_snapshot(&entries).is_ok());
}

#[test]
fn the_std_dot_star_namespace_is_reserved_even_with_otherwise_valid_shape() {
    // Start from a fully valid entry and override only `package`: the
    // returned code (602, not 601) itself proves the identity-grammar check
    // ran and passed first, since a grammar failure could only ever produce
    // 601.
    let mut reserved = meaning_entry("1.0.0", "reserved");
    reserved.package = "std.registrytest".to_owned();
    assert_eq!(build_snapshot(&[reserved]).unwrap_err().code, "SPX-PKR602");
}

#[test]
fn an_exact_bundled_std_name_is_also_reserved() {
    // Ties this registry's reservation directly to the compiler's existing
    // closed bundled-dependency registry (src/project/standard_dependencies.rs),
    // which shipped `std.auth` as a bundled package (issue #189-192).
    assert!(crate::project::standard_dependencies::is_bundled(
        "std.auth"
    ));
    let mut reserved = meaning_entry("1.0.0", "confusion");
    reserved.package = "std.auth".to_owned();
    assert_eq!(build_snapshot(&[reserved]).unwrap_err().code, "SPX-PKR602");
}

// --- Digest binding ------------------------------------------------------------

#[test]
fn content_digest_must_match_the_plain_sha256_of_subject_bytes() {
    let mut tampered = meaning_entry("1.0.0", "alpha");
    tampered.content_digest = fake_digest("not the real subject bytes at all");
    assert_eq!(build_snapshot(&[tampered]).unwrap_err().code, "SPX-PKR603");
}

#[test]
fn declared_coordinate_must_match_the_embedded_subject_coordinate() {
    // The subject bytes are genuinely bound (content_digest is recomputed
    // from the real bytes, so that earlier check passes), but the declared
    // package no longer matches the coordinate embedded in those bytes.
    let mut mismatched = meaning_entry("1.0.0", "coord-mismatch");
    mismatched.package = CALCULATOR.to_owned();
    mismatched.content_digest =
        crate::audit_capsule::sha256_digest(mismatched.subject_bytes.as_bytes());
    assert_eq!(
        build_snapshot(&[mismatched]).unwrap_err().code,
        "SPX-PKR603"
    );
}

// --- Shape / grammar ------------------------------------------------------------

#[test]
fn a_non_canonical_version_is_refused_before_any_binding_check() {
    let mut malformed = meaning_entry("1.0.0", "alpha");
    malformed.version = "1.00.0".to_owned();
    assert_eq!(build_snapshot(&[malformed]).unwrap_err().code, "SPX-PKR601");
}

#[test]
fn a_malformed_digest_shape_is_refused() {
    let mut malformed = meaning_entry("1.0.0", "alpha");
    malformed.api_digest = "sha1:deadbeef".to_owned();
    assert_eq!(build_snapshot(&[malformed]).unwrap_err().code, "SPX-PKR601");
}

// --- Capacity --------------------------------------------------------------------

#[test]
fn an_empty_entry_list_is_refused() {
    assert_eq!(build_snapshot(&[]).unwrap_err().code, "SPX-PKR606");
}

// --- Revocation / yank policy ------------------------------------------------------

#[test]
fn yank_policy_excludes_yanked_entries_by_default() {
    let active = meaning_entry("1.0.0", "alpha");
    let revoked = yanked(
        calculator_entry("2.0.0", "beta"),
        "known security advisory SPX-ADV-0001",
    );
    let snapshot = build_snapshot(&[active.clone(), revoked]).expect("build");
    let projected = project_subjects(&snapshot, YankPolicy::ExcludeYanked).expect("project");
    assert_eq!(projected.subjects, vec![active.subject_bytes]);
    assert!(projected.warnings.is_empty());
}

#[test]
fn yank_policy_can_refuse_outright() {
    let active = meaning_entry("1.0.0", "alpha");
    let revoked = yanked(
        calculator_entry("2.0.0", "beta"),
        "known security advisory SPX-ADV-0001",
    );
    let snapshot = build_snapshot(&[active, revoked]).expect("build");
    // Control: the default policy on this exact snapshot succeeds, so the
    // refusal below is attributable to the stricter policy, not the data.
    assert!(project_subjects(&snapshot, YankPolicy::ExcludeYanked).is_ok());
    assert_eq!(
        project_subjects(&snapshot, YankPolicy::RefuseIfYanked)
            .unwrap_err()
            .code,
        "SPX-PKR610"
    );
}

#[test]
fn yank_policy_can_allow_with_an_explicit_warning() {
    let active = meaning_entry("1.0.0", "alpha");
    let revoked = yanked(
        calculator_entry("2.0.0", "beta"),
        "known security advisory SPX-ADV-0001",
    );
    let snapshot = build_snapshot(&[active, revoked]).expect("build");
    let projected =
        project_subjects(&snapshot, YankPolicy::AllowYankedWithWarning).expect("project");
    assert_eq!(projected.subjects.len(), 2);
    assert_eq!(projected.warnings.len(), 1);
    assert_eq!(projected.warnings[0].code, "SPX-PKR609");
    assert_eq!(projected.warnings[0].severity, Severity::Warning);
}

// --- Signature/provenance are opaque, structurally checked, never verified --------

#[test]
fn signature_is_opaque_and_never_cryptographically_checked() {
    // A signature naming an approved-looking identity but carrying obviously
    // fabricated bytes is accepted exactly like a real one would be: this
    // module runs no cryptography over `signature`, exactly as
    // `crate::audit_capsule::SignatureEntry` does not either (issue #168,
    // still open, HUMAN_BLOCKED for both).
    let mut forged = meaning_entry("1.0.0", "alpha");
    forged.signature = RegistrySignature {
        algorithm: "ed25519".to_owned(),
        identity: "trusted-maintainer@example.invalid".to_owned(),
        signature: "this-is-not-a-real-signature-and-never-gets-checked".to_owned(),
    };
    assert!(build_snapshot(&[forged]).is_ok());
}

#[test]
fn provenance_digest_is_an_opaque_unverified_reference() {
    let mut published = meaning_entry("1.0.0", "alpha");
    published.provenance_digest = Some(fake_digest("a provenance object nobody checked"));
    assert!(build_snapshot(&[published]).is_ok());
}

// --- Composition with the existing reproducible resolver (package_resolver_v2) ---

#[test]
fn projected_subjects_resolve_deterministically_through_package_resolver_v2() {
    let entries = vec![meaning_entry("1.0.0", "meaning")];
    let snapshot = build_snapshot(&entries).expect("build");
    let projected = project_subjects(&snapshot, YankPolicy::ExcludeYanked).expect("project");
    let input = crate::package_resolver_v2::ResolutionInput {
        requirements: vec![crate::package_resolver_v2::Requirement {
            package: MEANING.to_owned(),
            range: "^1.0.0".to_owned(),
        }],
        subjects: projected.subjects,
        target: "native64".to_owned(),
        allowed_capabilities: vec![],
    };
    let options = crate::package_resolver_v2::ResolutionOptions::default();
    let first = crate::package_resolver_v2::generate(&input, &options).expect("first resolve");
    let second = crate::package_resolver_v2::generate(&input, &options).expect("second resolve");
    assert_eq!(
        first, second,
        "resolver-v2 evidence must be byte-identical for identical inputs"
    );
    let verified = crate::package_resolver_v2::verify(&first, &input, &options)
        .expect("resolver-v2 must independently replay the registry-projected catalog");
    assert_eq!(
        verified.packages,
        vec![crate::package_lock_v3::Coordinate {
            package: MEANING.to_owned(),
            version: "1.0.0".to_owned(),
        }]
    );
}

// --- Structural determinism argument ---------------------------------------------

#[test]
fn determinism_argument_is_structural_not_just_repeated_runs() {
    // This is the argument the module docstring makes, checked mechanically:
    // no hash-iterated map/set (whose iteration order is not a pure function
    // of content), no wall clock, and no environment or filesystem read
    // anywhere on the resolution path in this file or its helpers. Matched
    // as real Rust syntax (`<`/`::`), not the bare word, so this does not
    // trip over the module docstring's own prose describing this property.
    let source = include_str!("../package_registry.rs");
    for forbidden in [
        "HashMap<",
        "HashMap::",
        "HashSet<",
        "HashSet::",
        "SystemTime::now",
        "Instant::now",
        "std::env::",
        "std::fs::",
        "read_dir(",
    ] {
        assert!(
            !source.contains(forbidden),
            "package_registry.rs must not contain `{forbidden}`, which would make canonical \
             bytes depend on something other than entry content"
        );
    }
}
