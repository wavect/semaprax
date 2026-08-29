use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::package_lock_v2::{self, Coordinate};
use semaprax::package_report_v2::{self, PackageReportV2Options};
use semaprax::package_resolver::{self, Requirement, ResolutionInput, ResolutionOptions};
use semaprax::package_source_capsule::{self, PackageSource, SourceCapsuleOptions, SCHEMA};

static SERIAL: AtomicU64 = AtomicU64::new(0);

fn canonical(source: &str, path: &str) -> String {
    let program = semaprax::parse(source, Path::new(path)).expect("fixture parses");
    semaprax::format::canonical(&program)
}

fn report(root: &Path, name: &str, source: &str) -> String {
    let path = root.join(name);
    std::fs::write(&path, source).expect("write report fixture");
    package_report_v2::generate(&path, &PackageReportV2Options::default()).expect("generate report")
}

fn fixture() -> (
    PathBuf,
    Vec<PackageSource>,
    ResolutionInput,
    ResolutionOptions,
    String,
) {
    let root = std::env::temp_dir().join(format!(
        "semaprax-package-source-capsule-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root).expect("create fixture root");
    let app_interface = canonical(
        r#"
module app.main;

@id("app.main")
fn main() -> i64 { 0 }
"#,
        "app-interface.spx",
    );
    let library_interface = canonical(
        r#"
module lib.math;

@id("lib.answer")
fn answer() -> i64 { 0 }
"#,
        "lib-interface.spx",
    );
    let app_report = report(&root, "app-interface.spx", &app_interface);
    let library_report = report(&root, "lib-interface.spx", &library_interface);
    let app_coordinate = Coordinate {
        package: "app.main".to_owned(),
        version: "1.0.0".to_owned(),
    };
    let library_coordinate = Coordinate {
        package: "lib.math".to_owned(),
        version: "1.0.0".to_owned(),
    };
    let app_subject = package_lock_v2::create_subject(
        &app_coordinate,
        &app_report,
        std::slice::from_ref(&library_coordinate),
        &[],
    )
    .expect("app subject");
    let library_subject =
        package_lock_v2::create_subject(&library_coordinate, &library_report, &[], &[])
            .expect("library subject");
    let input = ResolutionInput {
        requirements: vec![Requirement {
            package: "app.main".to_owned(),
            range: "=1.0.0".to_owned(),
        }],
        subjects: vec![library_subject, app_subject],
        target: "wasm32".to_owned(),
        allowed_capabilities: Vec::new(),
    };
    let resolution_options = ResolutionOptions::default();
    let evidence =
        package_resolver::generate(&input, &resolution_options).expect("resolution evidence");
    let app_source = canonical(
        r#"
module app.main;
use function @id("lib.answer") from lib.math as answer;

@id("app.main")
fn main() -> i64 { answer() + 1 }
"#,
        "app.spx",
    );
    let library_source = canonical(
        r#"
module lib.math;

@id("lib.answer")
fn answer() -> i64 { 41 }
"#,
        "lib.spx",
    );
    (
        root,
        vec![
            PackageSource {
                package: "app.main".to_owned(),
                report: app_report,
                source: app_source,
            },
            PackageSource {
                package: "lib.math".to_owned(),
                report: library_report,
                source: library_source,
            },
        ],
        input,
        resolution_options,
        evidence,
    )
}

#[test]
fn capsule_replays_linked_sources_and_exposes_only_root_exports() {
    let (root, sources, input, resolution_options, evidence) = fixture();
    let options = SourceCapsuleOptions::new("app.main".to_owned(), 32 * 1024 * 1024)
        .expect("capsule options");
    let capsule = package_source_capsule::generate(
        &sources,
        &evidence,
        &input,
        &resolution_options,
        &options,
    )
    .expect("capsule");
    let receipt = package_source_capsule::verify(
        &capsule,
        &sources,
        &evidence,
        &input,
        &resolution_options,
        &options,
    )
    .expect("capsule replay");
    assert_eq!(receipt.schema(), SCHEMA);
    assert_eq!(receipt.root_package(), "app.main");
    assert_eq!(receipt.packages().len(), 2);
    assert_eq!(receipt.exports(), ["app.main"]);
    assert!(!receipt.exports().iter().any(|id| id == "lib.answer"));
    std::fs::remove_dir_all(root).expect("remove exact fixture root");
}

#[test]
fn dependency_metadata_cannot_replace_an_implementation_import() {
    let (root, mut sources, input, resolution_options, evidence) = fixture();
    sources[0].source = canonical(
        r#"
module app.main;

@id("app.main")
fn main() -> i64 { 42 }
"#,
        "app.spx",
    );
    let options = SourceCapsuleOptions::new("app.main".to_owned(), 32 * 1024 * 1024)
        .expect("capsule options");
    let error = package_source_capsule::generate(
        &sources,
        &evidence,
        &input,
        &resolution_options,
        &options,
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-PS503");
    std::fs::remove_dir_all(root).expect("remove exact fixture root");
}

#[test]
fn implementation_interface_must_exactly_match_selected_report_facts() {
    let (root, mut sources, input, resolution_options, evidence) = fixture();
    sources[0].source = canonical(
        r#"
module app.main;
use function @id("lib.answer") from lib.math as answer;

@id("app.main")
fn main() -> bool { answer() == 41 }
"#,
        "app.spx",
    );
    let options = SourceCapsuleOptions::new("app.main".to_owned(), 32 * 1024 * 1024)
        .expect("capsule options");
    let error = package_source_capsule::generate(
        &sources,
        &evidence,
        &input,
        &resolution_options,
        &options,
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-PS503");
    std::fs::remove_dir_all(root).expect("remove exact fixture root");
}

#[test]
fn replay_rejects_same_interface_implementation_source_tamper() {
    let (root, mut sources, input, resolution_options, evidence) = fixture();
    let options = SourceCapsuleOptions::new("app.main".to_owned(), 32 * 1024 * 1024)
        .expect("capsule options");
    let capsule = package_source_capsule::generate(
        &sources,
        &evidence,
        &input,
        &resolution_options,
        &options,
    )
    .expect("capsule");
    sources[1].source = canonical(
        r#"
module lib.math;

@id("lib.answer")
fn answer() -> i64 { 42 }
"#,
        "lib.spx",
    );
    let error = package_source_capsule::verify(
        &capsule,
        &sources,
        &evidence,
        &input,
        &resolution_options,
        &options,
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-PS507");
    std::fs::remove_dir_all(root).expect("remove exact fixture root");
}
