use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

pub(super) use semaprax::package_build::{self, OfflinePackageBuild, OfflinePackageBuildOptions};
pub(super) use semaprax::package_lock_v2::{self, Coordinate};
pub(super) use semaprax::package_report_v2::{self, PackageReportV2Options};
pub(super) use semaprax::package_resolver::{
    self, Requirement, ResolutionInput, ResolutionOptions,
};
use sha2::{Digest as _, Sha256};

pub(super) const BUILD_SCHEMA: &str = "semaprax.offline-effect-free-wasm-package-build.v1";
pub(super) const EVIDENCE_SCHEMA: &str =
    "semaprax.offline-effect-free-wasm-package-build-evidence.v1";
pub(super) const EVIDENCE_DOMAIN: &[u8] =
    b"semaprax.offline-effect-free-wasm-package-build-evidence.v1\0";
pub(super) const ROOT: &str = "examples.calculator";
pub(super) const MAX_BYTES: usize = 16 * 1024 * 1024;

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

pub(super) struct Fixture {
    pub(super) input: ResolutionInput,
    pub(super) resolution: String,
    pub(super) options: OfflinePackageBuildOptions,
    pub(super) build: OfflinePackageBuild,
}

pub(super) fn fixture() -> Fixture {
    fixture_from_source(
        ROOT,
        "1.0.0",
        include_str!("../../examples/calculator.spx"),
        &["calculator.add", "calculator.not"],
    )
}

pub(super) fn fixture_from_source(
    package: &str,
    version: &str,
    source: &str,
    exports: &[&str],
) -> Fixture {
    let report = report_from_source(package, source);
    let subject = subject(&report, package, version, &[], &[]);
    let input = input(&[(package, version)], vec![subject], "wasm32", &[]);
    let resolution = package_resolver::generate(&input, &ResolutionOptions::default())
        .expect("canonical resolution evidence");
    let options = build_options(package, exports, MAX_BYTES, MAX_BYTES);
    let build =
        package_build::generate(&resolution, &input, &ResolutionOptions::default(), &options)
            .expect("effect-free package build");
    Fixture {
        input,
        resolution,
        options,
        build,
    }
}

pub(super) fn report_from_source(package: &str, source: &str) -> String {
    let path = fixture_path(package);
    std::fs::write(&path, source).expect("write semantic report fixture");
    let report = package_report_v2::generate(&path, &PackageReportV2Options::default())
        .expect("source-authenticated package report");
    std::fs::remove_file(path).expect("remove semantic report fixture");
    report
}

pub(super) fn simple_source(package: &str, value: i64) -> String {
    format!(
        "module {package};\n\n@id(\"{package}.answer\")\nfn answer() -> i64\n{{\n    {value}\n}}\n\n@id(\"{package}.main\")\nfn main() -> i64\n{{\n    answer()\n}}\n"
    )
}

fn fixture_path(tag: &str) -> PathBuf {
    let safe = tag.replace('.', "_").replace('-', "_");
    std::env::temp_dir().join(format!(
        "semaprax-package-build-{}-{}-{safe}.spx",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
    ))
}

pub(super) fn coordinate(package: &str, version: &str) -> Coordinate {
    Coordinate {
        package: package.to_owned(),
        version: version.to_owned(),
    }
}

pub(super) fn subject(
    report: &str,
    package: &str,
    version: &str,
    dependencies: &[Coordinate],
    capabilities: &[&str],
) -> String {
    package_lock_v2::create_subject(
        &coordinate(package, version),
        report,
        dependencies,
        &capabilities
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>(),
    )
    .expect("canonical semantic subject")
}

pub(super) fn input(
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
                range: if range
                    .as_bytes()
                    .first()
                    .is_some_and(|byte| matches!(byte, b'=' | b'~' | b'^'))
                {
                    (*range).to_owned()
                } else {
                    format!("={range}")
                },
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

pub(super) fn build_options(
    root: &str,
    exports: &[&str],
    max_artifact_bytes: usize,
    max_evidence_bytes: usize,
) -> OfflinePackageBuildOptions {
    OfflinePackageBuildOptions {
        root_package: root.to_owned(),
        exports: exports.iter().map(|value| (*value).to_owned()).collect(),
        max_artifact_bytes,
        max_evidence_bytes,
    }
}

pub(super) fn copied_build(build: &OfflinePackageBuild) -> OfflinePackageBuild {
    OfflinePackageBuild {
        module_wasm: build.module_wasm.clone(),
        manifest_json: build.manifest_json.clone(),
        evidence_json: build.evidence_json.clone(),
    }
}

pub(super) fn artifact_bytes(build: &OfflinePackageBuild) -> usize {
    build
        .module_wasm
        .len()
        .checked_add(build.manifest_json.len())
        .and_then(|value| value.checked_add(build.evidence_json.len()))
        .expect("fixture artifact byte sum")
}

pub(super) fn generate_error(
    resolution: &str,
    input: &ResolutionInput,
    options: &OfflinePackageBuildOptions,
) -> String {
    package_build::generate(resolution, input, &ResolutionOptions::default(), options)
        .expect_err("package build must reject")[0]
        .code
        .to_owned()
}

pub(super) fn verify_error(build: &OfflinePackageBuild, fixture: &Fixture) -> String {
    package_build::verify(
        build,
        &fixture.resolution,
        &fixture.input,
        &ResolutionOptions::default(),
        &fixture.options,
    )
    .expect_err("package build replay must reject")
    .code
    .to_owned()
}

pub(super) fn payload(envelope: &str) -> &str {
    let start = envelope.find("\"payload\":").expect("payload marker") + "\"payload\":".len();
    &envelope[start..envelope.len() - 1]
}

pub(super) fn remint_evidence(payload: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(EVIDENCE_DOMAIN);
    hasher.update((payload.len() as u64).to_le_bytes());
    hasher.update(payload.as_bytes());
    format!(
        "{{\"schema\":\"{EVIDENCE_SCHEMA}\",\"digest\":\"sha256:{:x}\",\"bytes\":{},\"payload\":{payload}}}",
        semaprax::digest_hex::LowerHex(hasher.finalize()),
        payload.len()
    )
}

pub(super) fn mutate_decimal_member(wire: &str, member: &str) -> String {
    let marker = format!("\"{member}\":");
    let start = wire.rfind(&marker).expect("numeric member") + marker.len();
    let end = wire[start..]
        .find(|character: char| !character.is_ascii_digit())
        .map(|offset| start + offset)
        .expect("numeric member terminator");
    let original = &wire[start..end];
    let replacement = if original.bytes().all(|byte| byte == b'9') {
        "1".repeat(original.len())
    } else {
        let mut bytes = original.as_bytes().to_vec();
        let last = bytes.last_mut().expect("non-empty decimal");
        *last = if *last == b'9' { b'8' } else { *last + 1 };
        String::from_utf8(bytes).expect("ASCII decimal")
    };
    format!("{}{}{}", &wire[..start], replacement, &wire[end..])
}

pub(super) fn assert_ordered(haystack: &str, fragments: &[&str]) {
    let mut offset = 0;
    for fragment in fragments {
        let found = haystack[offset..]
            .find(fragment)
            .unwrap_or_else(|| panic!("missing ordered fragment {fragment}"));
        offset += found + fragment.len();
    }
}

pub(super) fn raw_scalar_symbol(stable_id: &str) -> String {
    let mut result = String::from("spx_scalar_");
    for byte in stable_id.bytes() {
        result.push_str(&format!("{byte:02x}"));
    }
    result
}
