use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::package_build_v2::{
    self, LinkedOfflinePackageBuild, LinkedOfflinePackageBuildOptions,
};
use semaprax::package_lock_v2::{self, Coordinate};
use semaprax::package_report_v2::{self, PackageReportV2Options};
use semaprax::package_resolver::{self, Requirement, ResolutionInput, ResolutionOptions};
use semaprax::package_source_capsule::{self, PackageSource, SourceCapsuleOptions};
use semaprax_offline_wasm_package::{
    publish_linked, PublicationError, PublishedLinkedOfflinePackageBuild, EVIDENCE_FILE,
    MANIFEST_FILE, MODULE_FILE,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);
pub const ROOT: &str = "app.main";
pub const PROVIDER: &str = "lib.math";
pub const FILES: [&str; 3] = [MODULE_FILE, EVIDENCE_FILE, MANIFEST_FILE];

pub struct Fixture {
    pub sources: Vec<PackageSource>,
    pub resolution: String,
    pub input: ResolutionInput,
    pub resolution_options: ResolutionOptions,
    pub capsule_options: SourceCapsuleOptions,
    pub capsule: String,
    pub build_options: LinkedOfflinePackageBuildOptions,
    pub build: LinkedOfflinePackageBuild,
}

impl Fixture {
    pub fn new(answer: i64) -> Self {
        let root_source = canonical(
            r#"module app.main;
use function @id("lib.math.answer") from lib.math as answer;
use function @id("lib.math.invert") from lib.math as invert;
use function @id("lib.math.sum") from lib.math as sum;
@id("app.main.add")
fn add(left: i64, right: i64) -> i64 { sum(left, right) }
@id("app.main.invert")
fn flip(value: bool) -> bool { invert(value) }
@id("app.main.run")
fn run() -> i64 { answer() + 1 }
"#,
        );
        let root_interface = canonical(
            r#"module app.main;
@id("app.main.add")
fn add(left: i64, right: i64) -> i64 { left + right }
@id("app.main.invert")
fn flip(value: bool) -> bool { !value }
@id("app.main.run")
fn main() -> i64 { 0 }
"#,
        );
        let provider_source = canonical(&format!(
            "module lib.math;\n@id(\"lib.math.answer\")\nfn answer() -> i64 {{ {answer} }}\n\
             @id(\"lib.math.invert\")\nfn invert(value: bool) -> bool {{ !value }}\n\
             @id(\"lib.math.sum\")\nfn sum(left: i64, right: i64) -> i64 {{ left + right }}\n"
        ));
        let provider_interface = provider_source.replace("fn answer()", "fn main()");
        let report_root = private_root("reports");
        let root_report = report(&report_root.join("app.spx"), &root_interface);
        let provider_report = report(&report_root.join("lib.spx"), &provider_interface);
        let provider_coordinate = coordinate(PROVIDER);
        let root_subject = package_lock_v2::create_subject(
            &coordinate(ROOT),
            &root_report,
            std::slice::from_ref(&provider_coordinate),
            &[],
        )
        .expect("root Subject v2");
        let provider_subject =
            package_lock_v2::create_subject(&provider_coordinate, &provider_report, &[], &[])
                .expect("provider Subject v2");
        let input = ResolutionInput {
            requirements: vec![Requirement {
                package: ROOT.to_owned(),
                range: "=1.0.0".to_owned(),
            }],
            subjects: vec![root_subject, provider_subject],
            target: "wasm32".to_owned(),
            allowed_capabilities: vec![],
        };
        let resolution_options = ResolutionOptions::default();
        let resolution =
            package_resolver::generate(&input, &resolution_options).expect("Resolver v1");
        let sources = vec![
            PackageSource {
                package: ROOT.to_owned(),
                report: root_report,
                source: root_source,
            },
            PackageSource {
                package: PROVIDER.to_owned(),
                report: provider_report,
                source: provider_source,
            },
        ];
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
        .expect("real linked source capsule");
        let build_options = LinkedOfflinePackageBuildOptions {
            root_package: ROOT.to_owned(),
            exports: ["app.main.add", "app.main.invert", "app.main.run"]
                .map(str::to_owned)
                .to_vec(),
            max_artifact_bytes: package_build_v2::MAX_ARTIFACT_BYTES,
            max_evidence_bytes: package_build_v2::MAX_EVIDENCE_BYTES,
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
        .expect("real linked Build v2");
        // Keep report files on any preceding failure; remove only this exact successful inventory.
        cleanup_files(&report_root, &["app.spx", "lib.spx"]);
        Self {
            sources,
            resolution,
            input,
            resolution_options,
            capsule_options,
            capsule,
            build_options,
            build,
        }
    }

    pub fn publish(
        &self,
        output: &Path,
        build: LinkedOfflinePackageBuild,
    ) -> Result<PublishedLinkedOfflinePackageBuild, PublicationError> {
        publish_linked(
            output,
            build,
            self.capsule.clone(),
            self.sources.clone(),
            self.resolution.clone(),
            self.input.clone(),
            self.resolution_options,
            self.capsule_options.clone(),
            self.build_options.clone(),
        )
    }

    pub fn verify(
        &self,
        build: &LinkedOfflinePackageBuild,
    ) -> Result<
        package_build_v2::VerifiedLinkedOfflinePackageBuild,
        Box<semaprax::diagnostic::Diagnostic>,
    > {
        package_build_v2::verify(
            build,
            &self.capsule,
            &self.sources,
            &self.resolution,
            &self.input,
            &self.resolution_options,
            &self.capsule_options,
            &self.build_options,
        )
        .map_err(Box::new)
    }
}

fn canonical(source: &str) -> String {
    semaprax::format::canonical(
        &semaprax::parse(source, Path::new("fixture.spx")).expect("parse fixture"),
    )
}

fn report(path: &Path, source: &str) -> String {
    fs::write(path, source).expect("write Report source");
    package_report_v2::generate(path, &PackageReportV2Options::default()).expect("Report v2")
}

pub fn coordinate(package: &str) -> Coordinate {
    Coordinate {
        package: package.to_owned(),
        version: "1.0.0".to_owned(),
    }
}

pub fn private_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir()
        .canonicalize()
        .expect("canonical temporary root")
        .join(format!(
            "semaprax-linked-publication-{label}-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed),
        ));
    fs::create_dir(&root).expect("create unique fixture root");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("private parent mode");
    }
    root
}

pub fn reopen(output: &Path) -> LinkedOfflinePackageBuild {
    assert_inventory(output, &FILES);
    LinkedOfflinePackageBuild {
        module_wasm: fs::read(output.join(MODULE_FILE)).expect("reopen module"),
        evidence_json: fs::read_to_string(output.join(EVIDENCE_FILE)).expect("reopen evidence"),
        manifest_json: fs::read_to_string(output.join(MANIFEST_FILE)).expect("reopen manifest"),
    }
}

pub fn assert_inventory(directory: &Path, names: &[&str]) {
    let mut actual = fs::read_dir(directory)
        .expect("list exact inventory")
        .map(|entry| entry.expect("directory entry").file_name())
        .collect::<Vec<_>>();
    actual.sort();
    let mut expected = names
        .iter()
        .map(|name| std::ffi::OsString::from(*name))
        .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(actual, expected);
}

pub fn cleanup_files(directory: &Path, names: &[&str]) {
    assert_inventory(directory, names);
    for name in names {
        assert!(fs::symlink_metadata(directory.join(name))
            .expect("fixture metadata")
            .file_type()
            .is_file());
    }
    for name in names {
        fs::remove_file(directory.join(name)).expect("remove exact fixture file");
    }
    fs::remove_dir(directory).expect("remove now-empty fixture directory");
}
