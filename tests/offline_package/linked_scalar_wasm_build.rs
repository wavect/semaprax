use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::package_build_v2::{
    self, LinkedOfflinePackageBuild, LinkedOfflinePackageBuildOptions, MAX_ARTIFACT_BYTES,
    MAX_EVIDENCE_BYTES,
};
use semaprax::package_lock_v2::{self, Coordinate};
use semaprax::package_report_v2::{self, PackageReportV2Options};
use semaprax::package_resolver::{self, Requirement, ResolutionInput, ResolutionOptions};
use semaprax::package_source_capsule::{self, PackageSource, SourceCapsuleOptions};
use sha2::{Digest as _, Sha256};

const ROOT: &str = "app.main";
const PROVIDER: &str = "lib.math";
static NEXT_PATH: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    sources: Vec<PackageSource>,
    resolution: String,
    input: ResolutionInput,
    capsule_options: SourceCapsuleOptions,
    capsule: String,
    build_options: LinkedOfflinePackageBuildOptions,
    build: LinkedOfflinePackageBuild,
}

fn fixture(provider_value: i64) -> Fixture {
    let provider_source = format!(
        "module {PROVIDER};\n\n@id(\"lib.math.answer\")\nfn answer() -> i64\n{{\n    {provider_value}\n}}\n"
    );
    let provider_interface_source = format!(
        "module {PROVIDER};\n\n@id(\"lib.math.answer\")\nfn main() -> i64\n{{\n    {provider_value}\n}}\n"
    );
    let mut root_source = format!(
        "module {ROOT};\nuse function @id(\"lib.math.answer\") from {PROVIDER} as answer;\n\n"
    );
    // Report v2 is interface evidence, not executable dependency source. Keep
    // its independently target-projectable subject free of workspace imports;
    // the capsule must authenticate the source-only implementation below by
    // exact typed interface equality before linking it.
    let mut root_interface_source = format!("module {ROOT};\n\n");
    let mut exports = Vec::new();
    for index in 0..31 {
        let id = format!("app.main.f{index:02}");
        root_source.push_str(&format!(
            "@id(\"{id}\")\nfn f{index:02}() -> i64\n{{\n    answer() + {index}\n}}\n\n"
        ));
        let interface_name = if index == 0 {
            "main".to_owned()
        } else {
            format!("f{index:02}")
        };
        root_interface_source.push_str(&format!(
            "@id(\"{id}\")\nfn {interface_name}() -> i64\n{{\n    {index}\n}}\n\n"
        ));
        exports.push(id);
    }
    root_source.push_str("@id(\"app.main.run\")\nfn run() -> i64\n{\n    answer()\n}\n");
    root_interface_source.push_str("@id(\"app.main.run\")\nfn run() -> i64\n{\n    0\n}\n");
    exports.push("app.main.run".to_owned());

    let provider_report = report(PROVIDER, &provider_interface_source);
    let root_report = report(ROOT, &root_interface_source);
    let provider_coordinate = coordinate(PROVIDER);
    let provider_subject =
        package_lock_v2::create_subject(&provider_coordinate, &provider_report, &[], &[])
            .expect("provider Subject v2");
    let root_subject = package_lock_v2::create_subject(
        &coordinate(ROOT),
        &root_report,
        std::slice::from_ref(&provider_coordinate),
        &[],
    )
    .expect("root Subject v2");
    let input = ResolutionInput {
        requirements: vec![Requirement {
            package: ROOT.to_owned(),
            range: "=1.0.0".to_owned(),
        }],
        subjects: vec![root_subject, provider_subject],
        target: "wasm32".to_owned(),
        allowed_capabilities: Vec::new(),
    };
    let resolution_options = ResolutionOptions::default();
    let resolution = package_resolver::generate(&input, &resolution_options)
        .expect("two-package resolver evidence");
    let selected = package_resolver::verify(&resolution, &input, &resolution_options)
        .expect("two-package resolver replay");
    let by_package = BTreeMap::from([
        (
            ROOT.to_owned(),
            PackageSource {
                package: ROOT.to_owned(),
                report: root_report,
                source: root_source,
            },
        ),
        (
            PROVIDER.to_owned(),
            PackageSource {
                package: PROVIDER.to_owned(),
                report: provider_report,
                source: provider_source,
            },
        ),
    ]);
    let sources = selected
        .packages
        .iter()
        .map(|coordinate| {
            by_package
                .get(&coordinate.package)
                .expect("selected package source")
                .clone()
        })
        .collect::<Vec<_>>();
    let capsule_options = SourceCapsuleOptions {
        root_package: ROOT.to_owned(),
        max_bytes: package_source_capsule::MAX_OUTPUT_BYTES,
    };
    let capsule = package_source_capsule::generate(
        &sources,
        &resolution,
        &input,
        &resolution_options,
        &capsule_options,
    )
    .expect("two-package source capsule");
    let build_options = LinkedOfflinePackageBuildOptions {
        root_package: ROOT.to_owned(),
        exports,
        max_artifact_bytes: MAX_ARTIFACT_BYTES,
        max_evidence_bytes: MAX_EVIDENCE_BYTES,
    };
    let build = package_build_v2::generate(
        &capsule,
        &sources,
        &resolution,
        &input,
        &resolution_options,
        &capsule_options,
        &build_options,
    )
    .expect("linked package build");
    Fixture {
        sources,
        resolution,
        input,
        capsule_options,
        capsule,
        build_options,
        build,
    }
}

#[test]
fn real_two_package_capsule_generates_and_independently_replays() {
    let fixture = fixture(41);
    assert!(!fixture.sources[0].source.contains("fn main("));
    let verified = verify(&fixture.build, &fixture).expect("exact linked build replay");
    assert_eq!(verified.root_package, ROOT);
    assert_eq!(
        verified.packages,
        vec![coordinate(ROOT), coordinate(PROVIDER)]
    );
    assert_eq!(verified.capsule_digest.len(), 71);
    assert_eq!(verified.artifact_bytes, artifact_bytes(&fixture.build));
    assert!(fixture
        .build
        .manifest_json
        .contains("\"stable_id\":\"app.main.f15\""));
}

#[test]
fn capsule_source_and_resolver_cross_pairs_fail_closed() {
    let first = fixture(41);
    let second = fixture(42);
    assert_eq!(verify(&second.build, &first).unwrap_err().code, "SPX-PB607");

    let mut crossed_sources = first.sources.clone();
    crossed_sources[0].source.push('\n');
    let error = package_build_v2::verify(
        &first.build,
        &first.capsule,
        &crossed_sources,
        &first.resolution,
        &first.input,
        &ResolutionOptions::default(),
        &first.capsule_options,
        &first.build_options,
    )
    .unwrap_err();
    assert!(matches!(
        error.code,
        "SPX-PB602" | "SPX-PB603" | "SPX-PB607"
    ));
}

#[test]
fn module_and_canonical_wire_mutations_are_rejected() {
    let fixture = fixture(41);
    let mut module = copied_build(&fixture.build);
    module.module_wasm.push(0);
    assert_eq!(verify(&module, &fixture).unwrap_err().code, "SPX-PB607");

    let mut manifest = copied_build(&fixture.build);
    manifest.manifest_json.insert(1, ' ');
    assert_eq!(verify(&manifest, &fixture).unwrap_err().code, "SPX-PB606");

    let mut evidence = copied_build(&fixture.build);
    evidence.evidence_json.push('\n');
    assert_eq!(verify(&evidence, &fixture).unwrap_err().code, "SPX-PB606");

    let mut reminted = copied_build(&fixture.build);
    let changed = mutate_decimal_member(evidence_payload(&reminted.evidence_json), "wasm_bytes");
    reminted.evidence_json = remint_evidence(&changed);
    assert_eq!(verify(&reminted, &fixture).unwrap_err().code, "SPX-PB607");
}

#[test]
fn artifact_and_evidence_limits_reach_exact_replayable_fixed_points() {
    let fixture = fixture(41);
    let (artifact_options, artifact_build) = converge(&fixture, true);
    assert_eq!(
        artifact_bytes(&artifact_build),
        artifact_options.max_artifact_bytes
    );
    verify_with_options(&artifact_build, &fixture, &artifact_options)
        .expect("exact artifact fixed point");
    let too_small = LinkedOfflinePackageBuildOptions {
        max_artifact_bytes: artifact_options.max_artifact_bytes - 1,
        ..artifact_options
    };
    assert_eq!(generate_error(&fixture, &too_small), "SPX-PB605");

    let (evidence_options, evidence_build) = converge(&fixture, false);
    assert_eq!(
        evidence_build.evidence_json.len(),
        evidence_options.max_evidence_bytes
    );
    verify_with_options(&evidence_build, &fixture, &evidence_options)
        .expect("exact evidence fixed point");
    let too_small = LinkedOfflinePackageBuildOptions {
        max_evidence_bytes: evidence_options.max_evidence_bytes - 1,
        ..evidence_options
    };
    assert_eq!(generate_error(&fixture, &too_small), "SPX-PB605");
}

#[test]
fn provider_exports_and_out_of_range_limits_are_rejected() {
    let fixture = fixture(41);
    let provider = LinkedOfflinePackageBuildOptions {
        exports: vec!["lib.math.answer".to_owned()],
        ..fixture.build_options.clone()
    };
    assert_eq!(generate_error(&fixture, &provider), "SPX-PB604");
    for options in [
        LinkedOfflinePackageBuildOptions {
            max_artifact_bytes: 4095,
            ..fixture.build_options.clone()
        },
        LinkedOfflinePackageBuildOptions {
            max_evidence_bytes: MAX_EVIDENCE_BYTES + 1,
            ..fixture.build_options.clone()
        },
    ] {
        assert_eq!(generate_error(&fixture, &options), "SPX-PB601");
    }
}

fn report(package: &str, source: &str) -> String {
    let path = fixture_path(package);
    std::fs::write(&path, source).expect("write Report v2 source fixture");
    let report = package_report_v2::generate(&path, &PackageReportV2Options::default())
        .expect("source-authenticated Report v2");
    std::fs::remove_file(path).expect("remove Report v2 source fixture");
    report
}

fn fixture_path(package: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "semaprax-linked-package-build-{}-{}-{}.spx",
        std::process::id(),
        NEXT_PATH.fetch_add(1, Ordering::Relaxed),
        package.replace('.', "_")
    ))
}

fn coordinate(package: &str) -> Coordinate {
    Coordinate {
        package: package.to_owned(),
        version: "1.0.0".to_owned(),
    }
}

fn copied_build(build: &LinkedOfflinePackageBuild) -> LinkedOfflinePackageBuild {
    LinkedOfflinePackageBuild {
        module_wasm: build.module_wasm.clone(),
        manifest_json: build.manifest_json.clone(),
        evidence_json: build.evidence_json.clone(),
    }
}

fn artifact_bytes(build: &LinkedOfflinePackageBuild) -> usize {
    build.module_wasm.len() + build.manifest_json.len() + build.evidence_json.len()
}

fn evidence_payload(evidence: &str) -> &str {
    let marker = "\"payload\":";
    let start = evidence.find(marker).expect("evidence payload") + marker.len();
    &evidence[start..evidence.len() - 1]
}

fn mutate_decimal_member(wire: &str, member: &str) -> String {
    let marker = format!("\"{member}\":");
    let start = wire.rfind(&marker).expect("numeric member") + marker.len();
    let end = wire[start..]
        .find(|character: char| !character.is_ascii_digit())
        .map(|offset| start + offset)
        .expect("numeric member terminator");
    let mut bytes = wire.as_bytes()[start..end].to_vec();
    let last = bytes.last_mut().expect("non-empty decimal");
    *last = if *last == b'9' { b'8' } else { *last + 1 };
    format!(
        "{}{}{}",
        &wire[..start],
        String::from_utf8(bytes).expect("ASCII decimal"),
        &wire[end..]
    )
}

fn remint_evidence(payload: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(b"semaprax.offline-linked-scalar-wasm-package-build-evidence.v2\0");
    digest.update(payload.as_bytes());
    format!(
        "{{\"schema\":\"{}\",\"digest\":\"sha256:{:x}\",\"bytes\":{},\"payload\":{payload}}}",
        package_build_v2::EVIDENCE_SCHEMA,
        semaprax::digest_hex::LowerHex(digest.finalize()),
        payload.len()
    )
}

fn verify(
    build: &LinkedOfflinePackageBuild,
    fixture: &Fixture,
) -> Result<
    package_build_v2::VerifiedLinkedOfflinePackageBuild,
    Box<semaprax::diagnostic::Diagnostic>,
> {
    verify_with_options(build, fixture, &fixture.build_options)
}

fn verify_with_options(
    build: &LinkedOfflinePackageBuild,
    fixture: &Fixture,
    options: &LinkedOfflinePackageBuildOptions,
) -> Result<
    package_build_v2::VerifiedLinkedOfflinePackageBuild,
    Box<semaprax::diagnostic::Diagnostic>,
> {
    package_build_v2::verify(
        build,
        &fixture.capsule,
        &fixture.sources,
        &fixture.resolution,
        &fixture.input,
        &ResolutionOptions::default(),
        &fixture.capsule_options,
        options,
    )
    .map_err(Box::new)
}

fn generate_error(fixture: &Fixture, options: &LinkedOfflinePackageBuildOptions) -> &'static str {
    package_build_v2::generate(
        &fixture.capsule,
        &fixture.sources,
        &fixture.resolution,
        &fixture.input,
        &ResolutionOptions::default(),
        &fixture.capsule_options,
        options,
    )
    .expect_err("linked build must reject")[0]
        .code
}

fn converge(
    fixture: &Fixture,
    artifact: bool,
) -> (LinkedOfflinePackageBuildOptions, LinkedOfflinePackageBuild) {
    let mut options = fixture.build_options.clone();
    for _ in 0..32 {
        let build = package_build_v2::generate(
            &fixture.capsule,
            &fixture.sources,
            &fixture.resolution,
            &fixture.input,
            &ResolutionOptions::default(),
            &fixture.capsule_options,
            &options,
        )
        .expect("fixed-point probe");
        let used = if artifact {
            artifact_bytes(&build)
        } else {
            build.evidence_json.len()
        };
        let current = if artifact {
            options.max_artifact_bytes
        } else {
            options.max_evidence_bytes
        };
        if used == current {
            return (options, build);
        }
        if artifact {
            options.max_artifact_bytes = used;
        } else {
            options.max_evidence_bytes = used;
        }
    }
    panic!("linked build byte accounting did not converge")
}
