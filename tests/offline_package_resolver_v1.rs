use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::package_compatibility::{self, CompatibilityInput, CompatibilityOptions};
use semaprax::package_lock_v2::{self, Coordinate, LockOptions};
use semaprax::package_report::{self, PackageReportOptions};
use semaprax::package_report_v2::{self, PackageReportV2Options};
use semaprax::package_resolver::{
    self, Requirement, ResolutionInput, ResolutionOptions, MAX_ALLOWED_CAPABILITIES, MAX_DECISIONS,
    MAX_DEPTH, MAX_EDGES, MAX_JSON_DEPTH, MAX_OUTPUT_BYTES, MAX_RENDER_BYTES, MAX_REQUIREMENTS,
    MAX_SELECTED_PACKAGES, MAX_SUBJECTS, MAX_SUBJECT_BYTES, MAX_TOTAL_SUBJECT_BYTES,
    MAX_VERSIONS_PER_PACKAGE, MAX_WORK_UNITS,
};
use sha2::{Digest as _, Sha256};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

fn report(package: &str) -> String {
    report_from_source(
        package,
        &format!("module {package};\n@id(\"{package}.main\")\nfn main() -> i64 {{ 42 }}\n"),
    )
}

fn unavailable_report(package: &str) -> String {
    report_from_source(
        package,
        &format!("module {package};\nfn main() -> i64 {{ 42 }}\n"),
    )
}

fn unproven_native_report(package: &str) -> String {
    report_from_source(
        package,
        &format!(
            "module {package};\n\
             @id(\"{package}.token\")\n\
             resource Token {{ @id(\"{package}.token.drop\") drop trivial; }}\n\
             @id(\"{package}.identity\")\n\
             fn identity(value: own Token) -> Token {{ value }}\n\
             @id(\"{package}.main\")\n\
             fn main() -> i64 {{ 0 }}\n"
        ),
    )
}

fn report_from_source(package: &str, source: &str) -> String {
    let path = fixture_path(package);
    std::fs::write(&path, source).expect("write report fixture");
    let result = package_report_v2::generate(&path, &PackageReportV2Options::default());
    std::fs::remove_file(path).expect("remove report fixture");
    result.expect("source-replayed report")
}

fn fixture_path(tag: &str) -> PathBuf {
    let safe = tag.replace('.', "-");
    std::env::temp_dir().join(format!(
        "semaprax-offline-resolver-{}-{}-{safe}.spx",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
    ))
}

fn coordinate(package: &str, version: &str) -> Coordinate {
    Coordinate {
        package: package.to_owned(),
        version: version.to_owned(),
    }
}

fn subject(
    report: &str,
    package: &str,
    version: &str,
    dependencies: &[Coordinate],
    capabilities: &[&str],
) -> String {
    let capabilities = capabilities
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    package_lock_v2::create_subject(
        &coordinate(package, version),
        report,
        dependencies,
        &capabilities,
    )
    .expect("canonical subject")
}

fn input(
    requirements: &[(&str, &str)],
    subjects: Vec<String>,
    target: &str,
    allowed_capabilities: &[&str],
) -> ResolutionInput {
    ResolutionInput {
        requirements: requirements
            .iter()
            .map(|(package, range)| Requirement {
                package: (*package).to_owned(),
                range: (*range).to_owned(),
            })
            .collect(),
        subjects,
        target: target.to_owned(),
        allowed_capabilities: allowed_capabilities
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
    }
}

fn generate(input: &ResolutionInput) -> String {
    package_resolver::generate(input, &ResolutionOptions::default()).expect("resolution evidence")
}

fn error_code(input: &ResolutionInput) -> String {
    package_resolver::generate(input, &ResolutionOptions::default())
        .expect_err("resolution must reject")[0]
        .code
        .clone()
}

fn remint(schema: &str, domain: &[u8], payload: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((payload.len() as u64).to_le_bytes());
    hasher.update(payload.as_bytes());
    format!(
        "{{\"schema\":\"{schema}\",\"digest\":\"sha256:{:x}\",\"bytes\":{},\"payload\":{payload}}}",
        semaprax::digest_hex::LowerHex(hasher.finalize()),
        payload.len()
    )
}

fn payload(envelope: &str) -> &str {
    let start = envelope.find("\"payload\":").expect("payload marker") + "\"payload\":".len();
    &envelope[start..envelope.len() - 1]
}

#[test]
fn public_limits_and_input_grammar_are_closed() {
    assert!(ResolutionOptions::new(4_096).is_ok());
    assert_eq!(ResolutionOptions::new(4_095).unwrap_err().code, "SPX-PR501");
    assert!(ResolutionOptions::new(MAX_OUTPUT_BYTES).is_ok());
    assert_eq!(
        ResolutionOptions::new(MAX_OUTPUT_BYTES + 1)
            .unwrap_err()
            .code,
        "SPX-PR501"
    );
    assert_eq!(ResolutionOptions::default().max_bytes, 16 * 1024 * 1024);
    assert_eq!(MAX_REQUIREMENTS, 4);
    assert_eq!(MAX_SUBJECTS, 64);
    assert_eq!(MAX_VERSIONS_PER_PACKAGE, 32);
    assert_eq!(MAX_SELECTED_PACKAGES, 4);
    assert_eq!(MAX_ALLOWED_CAPABILITIES, 256);
    assert_eq!(MAX_SUBJECT_BYTES, 17 * 1024 * 1024);
    assert_eq!(MAX_TOTAL_SUBJECT_BYTES, 128 * 1024 * 1024);
    assert_eq!(MAX_EDGES, 256);
    assert_eq!(MAX_DEPTH, 32);
    assert_eq!(MAX_DECISIONS, 4_096);
    assert_eq!(MAX_WORK_UNITS, 8 * 1024 * 1024);
    assert_eq!(MAX_JSON_DEPTH, 128);
    assert_eq!(MAX_RENDER_BYTES, 64 * 1024 * 1024);

    let package = "resolver.grammar";
    let one = subject(&report(package), package, "1.2.3", &[], &[]);
    for range in [
        "1.2.3",
        ">=1.2.3",
        "=01.2.3",
        "=1.02.3",
        "=1.2.03",
        "=1.2",
        "=1.2.3-alpha",
        "=1.2.3+build",
        "=1.*.3",
        "=1.2.3 ",
        "^4294967295.0.0",
        "~1.4294967295.0",
        "^0.0.4294967295",
    ] {
        assert_eq!(
            error_code(&input(
                &[(package, range)],
                vec![one.clone()],
                "native64",
                &[]
            )),
            "SPX-PR501",
            "range {range} must reject"
        );
    }
    assert_eq!(
        error_code(&input(
            &[("resolver.z", "=1.0.0"), (package, "=1.2.3")],
            vec![one.clone()],
            "native64",
            &[]
        )),
        "SPX-PR501"
    );
    assert_eq!(
        error_code(&ResolutionInput {
            requirements: vec![Requirement {
                package: package.to_owned(),
                range: "=1.2.3".to_owned(),
            }],
            subjects: vec![one.clone()],
            target: "linux".to_owned(),
            allowed_capabilities: vec![],
        }),
        "SPX-PR501"
    );
    assert_eq!(
        error_code(&input(
            &[(package, "=1.2.3")],
            vec![one],
            "native64",
            &["z", "a"]
        )),
        "SPX-PR501"
    );
}

#[test]
fn exact_caret_and_tilde_boundaries_select_numeric_versions() {
    let package = "resolver.semver";
    let report = report(package);
    let versions = [
        "0.0.3", "0.0.4", "0.2.3", "0.2.9", "0.3.0", "1.2.3", "1.2.9", "1.3.0", "1.9.9", "2.0.0",
    ];
    let subjects = versions
        .iter()
        .map(|version| subject(&report, package, version, &[], &[]))
        .collect::<Vec<_>>();
    let selected = |range: &str| {
        let request = input(&[(package, range)], subjects.clone(), "native64", &[]);
        package_resolver::verify(&generate(&request), &request, &ResolutionOptions::default())
            .unwrap()
            .packages[0]
            .version
            .clone()
    };
    assert_eq!(selected("=1.2.3"), "1.2.3");
    assert_eq!(selected("~1.2.3"), "1.2.9");
    assert_eq!(selected("^1.2.3"), "1.9.9");
    assert_eq!(selected("^0.2.3"), "0.2.9");
    assert_eq!(selected("^0.0.3"), "0.0.3");
}

#[test]
fn permutation_and_first_feasible_backtracking_are_deterministic() {
    let a = "resolver.backtrack.a";
    let b = "resolver.backtrack.b";
    let a_report = report(a);
    let b_report = report(b);
    let b1 = coordinate(b, "1.0.0");
    let b2 = coordinate(b, "2.0.0");
    let mut subjects = vec![
        subject(&a_report, a, "1.1.0", std::slice::from_ref(&b2), &[]),
        subject(&a_report, a, "1.0.0", std::slice::from_ref(&b1), &[]),
        subject(&b_report, b, "2.0.0", &[], &["denied"]),
        subject(&b_report, b, "1.0.0", &[], &[]),
    ];
    let forward = input(&[(a, "^1.0.0")], subjects.clone(), "native64", &[]);
    let forward_bytes = generate(&forward);
    subjects.reverse();
    let reverse = input(&[(a, "^1.0.0")], subjects, "native64", &[]);
    let reverse_bytes = generate(&reverse);
    assert_eq!(forward_bytes, reverse_bytes);
    assert_eq!(
        package_resolver::verify(&forward_bytes, &forward, &ResolutionOptions::default())
            .unwrap()
            .packages,
        vec![coordinate(a, "1.0.0"), b1]
    );
}

#[test]
fn multi_root_convergence_and_exact_transitive_closure_are_selected_once() {
    let names = [
        "resolver.graph.a",
        "resolver.graph.b",
        "resolver.graph.c",
        "resolver.graph.d",
    ];
    let reports = names.iter().map(|name| report(name)).collect::<Vec<_>>();
    let b = coordinate(names[1], "1.0.0");
    let d = coordinate(names[3], "1.0.0");
    let subjects = vec![
        subject(
            &reports[0],
            names[0],
            "1.0.0",
            std::slice::from_ref(&b),
            &[],
        ),
        subject(
            &reports[1],
            names[1],
            "1.0.0",
            std::slice::from_ref(&d),
            &[],
        ),
        subject(
            &reports[2],
            names[2],
            "1.0.0",
            std::slice::from_ref(&b),
            &[],
        ),
        subject(&reports[3], names[3], "1.0.0", &[], &[]),
    ];
    let request = input(
        &[(names[0], "=1.0.0"), (names[2], "=1.0.0")],
        subjects,
        "wasm32",
        &[],
    );
    let receipt =
        package_resolver::verify(&generate(&request), &request, &ResolutionOptions::default())
            .unwrap();
    assert_eq!(
        receipt.packages,
        names
            .iter()
            .map(|name| coordinate(name, "1.0.0"))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        package_lock_v2::verify(&receipt.lock, &request.subjects, &LockOptions::default())
            .unwrap()
            .packages,
        vec![
            d,
            b,
            coordinate(names[0], "1.0.0"),
            coordinate(names[2], "1.0.0")
        ]
    );
}

#[test]
fn missing_conflicting_duplicate_and_cyclic_catalogs_fail_closed() {
    let a = "resolver.reject.a";
    let b = "resolver.reject.b";
    let c = "resolver.reject.c";
    let reports = [report(a), report(b), report(c)];
    let a1 = coordinate(a, "1.0.0");
    let b1 = coordinate(b, "1.0.0");
    let b2 = coordinate(b, "2.0.0");
    let c1 = coordinate(c, "1.0.0");

    let missing = subject(&reports[0], a, "1.0.0", std::slice::from_ref(&b1), &[]);
    assert_eq!(
        error_code(&input(&[(a, "=1.0.0")], vec![missing], "native64", &[])),
        "SPX-PR503"
    );

    let conflict = vec![
        subject(&reports[0], a, "1.0.0", std::slice::from_ref(&b1), &[]),
        subject(&reports[1], b, "1.0.0", &[], &[]),
        subject(&reports[1], b, "2.0.0", &[], &[]),
        subject(&reports[2], c, "1.0.0", std::slice::from_ref(&b2), &[]),
    ];
    assert_eq!(
        error_code(&input(
            &[(a, "=1.0.0"), (c, "=1.0.0")],
            conflict,
            "native64",
            &[]
        )),
        "SPX-PR503"
    );

    let duplicate = subject(&reports[0], a, "1.0.0", &[], &[]);
    assert_eq!(
        error_code(&input(
            &[(a, "=1.0.0")],
            vec![duplicate.clone(), duplicate],
            "native64",
            &[]
        )),
        "SPX-PR503"
    );

    let cycle = vec![
        subject(&reports[0], a, "1.0.0", std::slice::from_ref(&b1), &[]),
        subject(&reports[1], b, "1.0.0", std::slice::from_ref(&c1), &[]),
        subject(&reports[2], c, "1.0.0", std::slice::from_ref(&a1), &[]),
    ];
    assert_eq!(
        error_code(&input(&[(a, "=1.0.0")], cycle, "native64", &[])),
        "SPX-PR503"
    );
}

#[test]
fn catalog_and_selected_graph_count_overflows_are_global_bounds_failures() {
    let package = "resolver.bounds";
    let report = report(package);
    let one = subject(&report, package, "1.0.0", &[], &[]);
    assert_eq!(
        error_code(&input(&[(package, "=1.0.0")], vec![], "native64", &[])),
        "SPX-PR505"
    );
    assert_eq!(
        error_code(&input(
            &[(package, "=1.0.0")],
            vec![one.clone(); MAX_SUBJECTS + 1],
            "native64",
            &[]
        )),
        "SPX-PR505"
    );
    let versions = (0..=MAX_VERSIONS_PER_PACKAGE)
        .map(|patch| subject(&report, package, &format!("1.0.{patch}"), &[], &[]))
        .collect::<Vec<_>>();
    assert_eq!(
        error_code(&input(&[(package, "^1.0.0")], versions, "native64", &[])),
        "SPX-PR505"
    );

    let names = [
        "resolver.depth.a",
        "resolver.depth.b",
        "resolver.depth.c",
        "resolver.depth.d",
        "resolver.depth.e",
    ];
    let reports = names.iter().map(|name| report(name)).collect::<Vec<_>>();
    let chain = names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let dependencies = names
                .get(index + 1)
                .map(|next| vec![coordinate(next, "1.0.0")])
                .unwrap_or_default();
            subject(&reports[index], name, "1.0.0", &dependencies, &[])
        })
        .collect::<Vec<_>>();
    assert_eq!(
        error_code(&input(&[(names[0], "=1.0.0")], chain, "native64", &[])),
        "SPX-PR505"
    );
}

#[test]
fn decision_and_work_exhaustion_abort_the_whole_search() {
    let names = [
        "resolver.decisions.a",
        "resolver.decisions.b",
        "resolver.decisions.c",
    ];
    let reports = names.iter().map(|name| report(name)).collect::<Vec<_>>();
    let missing = coordinate("resolver.decisions.missing", "1.0.0");
    let mut subjects = Vec::new();
    for (index, name) in names.iter().enumerate() {
        for patch in 0..21 {
            let dependencies = if index == 2 {
                std::slice::from_ref(&missing)
            } else {
                &[]
            };
            subjects.push(subject(
                &reports[index],
                name,
                &format!("1.0.{patch}"),
                dependencies,
                &[],
            ));
        }
    }
    assert_eq!(subjects.len(), 63);
    assert_eq!(
        error_code(&input(
            &[
                (names[0], "^1.0.0"),
                (names[1], "^1.0.0"),
                (names[2], "^1.0.0"),
            ],
            subjects,
            "native64",
            &[]
        )),
        "SPX-PR505"
    );

    let package = "resolver.work";
    let literal = "a".repeat(940_000);
    let large_report = report_from_source(
        package,
        &format!("module {package};\nfn main() -> string {{ \"{literal}\" }}\n"),
    );
    let subjects = (0..9)
        .map(|patch| subject(&large_report, package, &format!("1.0.{patch}"), &[], &[]))
        .collect::<Vec<_>>();
    assert_eq!(
        error_code(&input(&[(package, "^1.0.0")], subjects, "native64", &[])),
        "SPX-PR505"
    );
}

#[test]
fn capability_allowlist_exact_count_and_plus_one_are_distinct() {
    let package = "resolver.cap.bound";
    let subject = subject(&report(package), package, "1.0.0", &[], &[]);
    let allowed = (0..MAX_ALLOWED_CAPABILITIES)
        .map(|index| format!("cap.{index:03}"))
        .collect::<Vec<_>>();
    let exact = ResolutionInput {
        requirements: vec![Requirement {
            package: package.to_owned(),
            range: "=1.0.0".to_owned(),
        }],
        subjects: vec![subject.clone()],
        target: "native64".to_owned(),
        allowed_capabilities: allowed.clone(),
    };
    assert!(package_resolver::generate(&exact, &ResolutionOptions::default()).is_ok());
    let mut overflow = exact;
    overflow
        .allowed_capabilities
        .push(format!("cap.{MAX_ALLOWED_CAPABILITIES:03}"));
    assert_eq!(error_code(&overflow), "SPX-PR501");
}

#[test]
fn target_and_direct_or_transitive_capability_policy_are_fail_closed() {
    let unavailable = "resolver.target.unavailable";
    let unavailable_subject = subject(
        &unavailable_report(unavailable),
        unavailable,
        "1.0.0",
        &[],
        &[],
    );
    assert_eq!(
        error_code(&input(
            &[(unavailable, "=1.0.0")],
            vec![unavailable_subject],
            "native64",
            &[]
        )),
        "SPX-PR504"
    );

    let unproven = "resolver.target.unproven";
    let unproven_report = unproven_native_report(unproven);
    assert!(unproven_report.contains("\"target\":\"native64\",\"status\":\"unproven\""));
    let unproven_subject = subject(&unproven_report, unproven, "1.0.0", &[], &[]);
    assert_eq!(
        error_code(&input(
            &[(unproven, "=1.0.0")],
            vec![unproven_subject],
            "native64",
            &[]
        )),
        "SPX-PR504"
    );

    let a = "resolver.cap.a";
    let b = "resolver.cap.b";
    let a_report = report(a);
    let b_report = report(b);
    let b1 = coordinate(b, "1.0.0");
    let subjects = vec![
        subject(
            &a_report,
            a,
            "1.0.0",
            std::slice::from_ref(&b1),
            &["root.execute"],
        ),
        subject(&b_report, b, "1.0.0", &[], &["dependency.read"]),
    ];
    assert_eq!(
        error_code(&input(&[(a, "=1.0.0")], subjects.clone(), "native64", &[])),
        "SPX-PR504"
    );
    assert_eq!(
        error_code(&input(
            &[(a, "=1.0.0")],
            subjects.clone(),
            "native64",
            &["root.execute"]
        )),
        "SPX-PR504"
    );
    let allowed = input(
        &[(a, "=1.0.0")],
        subjects,
        "native64",
        &["dependency.read", "root.execute"],
    );
    let evidence = generate(&allowed);
    assert!(evidence.contains("\"capability_closure\":[\"dependency.read\",\"root.execute\"]"));
}

#[test]
fn subject_and_nested_report_remints_do_not_bypass_source_replay() {
    const SUBJECT_SCHEMA: &str = "semaprax.offline-semantic-package-subject.v2";
    const SUBJECT_DOMAIN: &[u8] = b"semaprax.offline-semantic-package-subject.v2\0";
    const REPORT_SCHEMA: &str = "semaprax.semantic-package-report.v2";
    const REPORT_PAYLOAD_DOMAIN: &[u8] = b"semaprax.package-report-v2.payload.v1\0";
    const SUBJECT_REPORT_DOMAIN: &[u8] = b"semaprax.offline-semantic-package-report.v2\0";
    let package = "resolver.auth";
    let report = report(package);
    let subject = subject(&report, package, "1.0.0", &[], &[]);

    let changed_payload = payload(&subject).replacen(package, "resolver.authx", 1);
    let forged_subject = remint(SUBJECT_SCHEMA, SUBJECT_DOMAIN, &changed_payload);
    assert_eq!(
        error_code(&input(
            &[(package, "=1.0.0")],
            vec![forged_subject],
            "native64",
            &[]
        )),
        "SPX-PR502"
    );

    let changed_report_payload =
        payload(&report).replacen("\"ownership\":\"value\"", "\"ownership\":\"own\"", 1);
    let forged_report = remint(
        REPORT_SCHEMA,
        REPORT_PAYLOAD_DOMAIN,
        &changed_report_payload,
    );
    let old_report_digest = domain_digest(SUBJECT_REPORT_DOMAIN, report.as_bytes());
    let new_report_digest = domain_digest(SUBJECT_REPORT_DOMAIN, forged_report.as_bytes());
    let changed_subject_payload = payload(&subject)
        .replacen(&old_report_digest, &new_report_digest, 1)
        .replacen(
            &format!("\"report_bytes\":{}", report.len()),
            &format!("\"report_bytes\":{}", forged_report.len()),
            1,
        )
        .replacen(&report, &forged_report, 1);
    let forged_nested = remint(SUBJECT_SCHEMA, SUBJECT_DOMAIN, &changed_subject_payload);
    assert_eq!(
        error_code(&input(
            &[(package, "=1.0.0")],
            vec![forged_nested],
            "native64",
            &[]
        )),
        "SPX-PR502"
    );
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    )
}

#[test]
fn wire_mutation_remint_truncation_insertion_and_input_drift_are_rejected() {
    const DOMAIN: &[u8] = b"semaprax.offline-package-resolution-evidence.v1\0";
    let package = "resolver.wire";
    let request = input(
        &[(package, "=1.0.0")],
        vec![subject(&report(package), package, "1.0.0", &[], &[])],
        "native64",
        &[],
    );
    let evidence = generate(&request);
    let options = ResolutionOptions::default();
    assert!(evidence.starts_with(
        "{\"schema\":\"semaprax.offline-package-resolution-evidence.v1\",\"digest\":\"sha256:"
    ));
    assert!(evidence.contains(
        "\"nonclaims\":[\"offline_deterministic_resolution_evidence\",\"no_registry_network_fetch_build_script_execution_cache_or_publication\",\"capability_allowlist_is_resolution_admission_not_runtime_enforcement\",\"target_availability_is_projection_not_execution\",\"evidence_is_not_authority\"]"
    ));
    let receipt = package_resolver::verify(&evidence, &request, &options).unwrap();
    assert!(evidence.contains(&format!("\"lock\":{}", receipt.lock)));
    assert_eq!(
        package_resolver::verify(&evidence[..evidence.len() - 1], &request, &options)
            .unwrap_err()
            .code,
        "SPX-PR506"
    );
    assert_eq!(
        package_resolver::verify(&(evidence.clone() + " "), &request, &options)
            .unwrap_err()
            .code,
        "SPX-PR506"
    );
    let mutated = evidence.replacen("\"target\":\"native64\"", "\"target\":\"wasm32\"", 1);
    assert_eq!(
        package_resolver::verify(&mutated, &request, &options)
            .unwrap_err()
            .code,
        "SPX-PR507"
    );
    let reminted_payload =
        payload(&evidence).replacen("\"target\":\"native64\"", "\"target\":\"wasm32\"", 1);
    let reminted = remint(package_resolver::SCHEMA, DOMAIN, &reminted_payload);
    assert_eq!(
        package_resolver::verify(&reminted, &request, &options)
            .unwrap_err()
            .code,
        "SPX-PR507"
    );
    let mut drifted = request.clone();
    drifted.allowed_capabilities.push("drift".to_owned());
    assert_eq!(
        package_resolver::verify(&evidence, &drifted, &options)
            .unwrap_err()
            .code,
        "SPX-PR507"
    );
}
#[test]
fn resolver_preserves_report_v1_v2_lock_v2_and_compatibility_v1_bytes() {
    let package = "resolver.preservation";
    let report = report(package);
    let report_path = fixture_from_report_source(package, &report);
    let legacy_report_before =
        package_report::generate(&report_path, &PackageReportOptions::default()).unwrap();
    let v1 = subject(&report, package, "1.0.0", &[], &[]);
    let v2 = subject(&report, package, "1.1.0", &[], &[]);
    let lock_before =
        package_lock_v2::generate(std::slice::from_ref(&v1), &LockOptions::default()).unwrap();
    let candidate_lock =
        package_lock_v2::generate(std::slice::from_ref(&v2), &LockOptions::default()).unwrap();
    let base = CompatibilityInput {
        coordinate: coordinate(package, "1.0.0"),
        report: report.clone(),
        lock: lock_before.clone(),
        lock_subjects: vec![v1.clone()],
    };
    let candidate = CompatibilityInput {
        coordinate: coordinate(package, "1.1.0"),
        report: report.clone(),
        lock: candidate_lock.clone(),
        lock_subjects: vec![v2.clone()],
    };
    let compatibility_before =
        package_compatibility::generate(&base, &candidate, &CompatibilityOptions::default())
            .unwrap();

    let request = input(
        &[(package, "^1.0.0")],
        vec![v1.clone(), v2.clone()],
        "native64",
        &[],
    );
    let resolution = generate(&request);
    let receipt =
        package_resolver::verify(&resolution, &request, &ResolutionOptions::default()).unwrap();
    assert_eq!(receipt.packages, vec![coordinate(package, "1.1.0")]);

    let report_after =
        package_report_v2::generate(&report_path, &PackageReportV2Options::default()).unwrap();
    let legacy_report_after =
        package_report::generate(&report_path, &PackageReportOptions::default()).unwrap();
    std::fs::remove_file(report_path).unwrap();
    assert_eq!(report_after, report);
    assert_eq!(legacy_report_after, legacy_report_before);
    assert_eq!(
        package_lock_v2::generate(std::slice::from_ref(&v1), &LockOptions::default()).unwrap(),
        lock_before
    );
    assert_eq!(
        package_lock_v2::generate(std::slice::from_ref(&v2), &LockOptions::default()).unwrap(),
        candidate_lock
    );
    assert_eq!(
        package_compatibility::generate(&base, &candidate, &CompatibilityOptions::default())
            .unwrap(),
        compatibility_before
    );
}

fn fixture_from_report_source(package: &str, report: &str) -> PathBuf {
    let value: serde_json::Value = serde_json::from_str(report).unwrap();
    let source = value["payload"]["source"]["text"].as_str().unwrap();
    let path = fixture_path(package);
    std::fs::write(&path, source).unwrap();
    path
}
