pub(super) use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub(super) use semaprax::package_compatibility::{self, CompatibilityInput, CompatibilityOptions};
pub(super) use semaprax::package_lock_v2::{self, Coordinate, LockOptions};
pub(super) use semaprax::package_report::{self, PackageReportOptions};
pub(super) use semaprax::package_report_v2::{self, PackageReportV2Options};
pub(super) use semaprax::package_resolver::{
    self, Requirement, ResolutionInput, ResolutionOptions, MAX_ALLOWED_CAPABILITIES, MAX_DECISIONS,
    MAX_DEPTH, MAX_EDGES, MAX_JSON_DEPTH, MAX_OUTPUT_BYTES, MAX_RENDER_BYTES, MAX_REQUIREMENTS,
    MAX_SELECTED_PACKAGES, MAX_SUBJECTS, MAX_SUBJECT_BYTES, MAX_TOTAL_SUBJECT_BYTES,
    MAX_VERSIONS_PER_PACKAGE, MAX_WORK_UNITS,
};
use sha2::{Digest as _, Sha256};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

pub(super) fn report(package: &str) -> String {
    report_from_source(
        package,
        &format!("module {package};\n@id(\"{package}.main\")\nfn main() -> i64 {{ 42 }}\n"),
    )
}

pub(super) fn unavailable_report(package: &str) -> String {
    report_from_source(
        package,
        &format!("module {package};\nfn main() -> i64 {{ 42 }}\n"),
    )
}

pub(super) fn unproven_native_report(package: &str) -> String {
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

pub(super) fn report_from_source(package: &str, source: &str) -> String {
    let path = fixture_path(package);
    std::fs::write(&path, source).expect("write report fixture");
    let result = package_report_v2::generate(&path, &PackageReportV2Options::default());
    std::fs::remove_file(path).expect("remove report fixture");
    result.expect("source-replayed report")
}

pub(super) fn fixture_path(tag: &str) -> PathBuf {
    let safe = tag.replace('.', "-");
    std::env::temp_dir().join(format!(
        "semaprax-offline-resolver-{}-{}-{safe}.spx",
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

pub(super) fn generate(input: &ResolutionInput) -> String {
    package_resolver::generate(input, &ResolutionOptions::default()).expect("resolution evidence")
}

pub(super) fn error_code(input: &ResolutionInput) -> String {
    package_resolver::generate(input, &ResolutionOptions::default())
        .expect_err("resolution must reject")[0]
        .code
        .clone()
}

pub(super) fn remint(schema: &str, domain: &[u8], payload: &str) -> String {
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

pub(super) fn payload(envelope: &str) -> &str {
    let start = envelope.find("\"payload\":").expect("payload marker") + "\"payload\":".len();
    &envelope[start..envelope.len() - 1]
}

pub(super) fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    )
}

pub(super) fn sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    )
}

pub(super) fn catalog_digest(subjects: &[&str]) -> String {
    const DOMAIN: &[u8] = b"semaprax.offline-package-resolution-catalog.v1\0";
    let mut hasher = Sha256::new();
    hasher.update(DOMAIN);
    hasher.update((subjects.len() as u64).to_le_bytes());
    for subject in subjects {
        hasher.update((subject.len() as u64).to_le_bytes());
        hasher.update(subject.as_bytes());
    }
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    )
}

pub(super) fn assert_ordered(haystack: &str, needles: &[&str]) {
    let mut offset = 0;
    for needle in needles {
        let found = haystack[offset..]
            .find(needle)
            .unwrap_or_else(|| panic!("missing ordered fragment {needle}"));
        offset += found + needle.len();
    }
}

pub(super) fn between<'a>(wire: &'a str, start: &str, end: &str) -> &'a str {
    let offset = wire.rfind(start).expect("start marker") + start.len();
    let finish = wire[offset..]
        .find(end)
        .map(|value| offset + value)
        .expect("end marker");
    &wire[offset..finish]
}
