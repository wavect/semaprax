//! Authored, unrun evidence for candidate-bound generic template renames.

use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ImageFacetOptions, ProjectCandidate, ProjectRevision,
    ProjectSemanticImage, SemanticChange,
};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-candidate-generic-rename-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let fixture = Self(root.canonicalize().unwrap());
        fixture.write(
            "semaprax.toml",
            r#"schema = "semaprax.project.v8"
name = "candidate-generic-rename"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "generic.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["generic.public"]
tests = ["generic.tests"]
"#,
        );
        fixture.write(
            "src/app.spx",
            r#"module generic.app;
use function @id("generic.public") from generic.core as public_value;
@id("generic.main") fn main()->i64 {public_value(7)}
"#,
        );
        fixture.write(
            "src/core.spx",
            r#"module generic.core;
@id("generic.keep") fn keep<T>(value:T)->T {value}
@id("generic.use-i64") fn use_i64(value:i64)->i64 {keep<i64>(value)}
@id("generic.use-bool") fn use_bool(value:bool)->bool {keep<bool>(value)}
@id("generic.public") fn public_value(value:i64)->i64 {keep<i64>(value)}
"#,
        );
        fixture.write(
            "src/tests.spx",
            r#"module generic.tests;
use function @id("generic.public") from generic.core as check;
@id("generic.test") fn main()->i64 {if check(4)==4 {0}else{1}}
"#,
        );
        fixture
    }

    fn write(&self, path: &str, source: &str) {
        let destination = self.0.join(path);
        if path.ends_with(".spx") {
            let program = semaprax::parse(source, path).unwrap();
            std::fs::write(destination, semaprax::format::canonical(&program)).unwrap();
        } else {
            std::fs::write(destination, source).unwrap();
        }
    }

    fn revision(&self) -> Arc<ProjectRevision> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn instances(revision: Arc<ProjectRevision>) -> Value {
    let image =
        ProjectSemanticImage::derive(Arc::clone(&revision), revision.project_revision()).unwrap();
    serde_json::from_str(
        &image
            .function_instances(
                image.image_digest(),
                "generic.keep",
                None,
                ImageFacetOptions::new(32, 1024 * 1024).unwrap(),
            )
            .unwrap(),
    )
    .unwrap()
}

/// The exact retained instance descriptors, keeping every identity, type
/// argument, signature count and facet name, and dropping only the opaque
/// facet handles. Those handles bind the image revision by contract
/// (`image_protocol::function_instances_v1::
/// opaque_references_bind_template_instance_facet_page_shape_and_image`
/// rejects a surviving instance's prior-image handle with SPX-G229), so a
/// preserved instance necessarily rebinds them.
fn descriptors(report: &Value) -> Vec<Value> {
    report["instances"]
        .as_array()
        .expect("instance rows")
        .iter()
        .map(|instance| {
            let mut row = instance.clone();
            let object = row.as_object_mut().expect("instance row object");
            let facets = object["facets"]
                .as_array()
                .expect("facet rows")
                .iter()
                .map(|facet| facet["facet"].clone())
                .collect::<Vec<_>>();
            object.insert("facets".to_owned(), Value::Array(facets));
            row
        })
        .collect()
}

fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    let diagnostics = result.err().expect("unsupported generic change accepted");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == expected),
        "{diagnostics:?}"
    );
}

#[test]
fn rename_preserves_two_checked_concrete_instances_and_cross_module_aliases() {
    let fixture = Fixture::new();
    let revision = fixture.revision();
    let before = instances(Arc::clone(&revision));
    assert_eq!(before["total_instances"], 2);

    let base = ProjectCandidate::open(Arc::clone(&revision), revision.project_revision()).unwrap();
    let catalog: Value =
        serde_json::from_str(&base.change_catalog("generic.keep").unwrap()).unwrap();
    assert_eq!(catalog["operations"].as_array().unwrap().len(), 1);
    assert_eq!(catalog["operations"][0]["kind"], "rename_declaration");
    assert_eq!(
        catalog["operations"][0]["generic_instances"],
        "preserve_exact_retained_checked_hir"
    );

    let change = SemanticChange::new(
        revision.project_revision(),
        &json!({"kind":"rename_declaration","target":"generic.keep","name":"retain"}),
    )
    .unwrap();
    let candidate = base.apply(base.candidate_digest(), &change).unwrap();
    let report: Value = serde_json::from_str(candidate.to_json()).unwrap();
    assert_eq!(
        report["operations"][0]["generic_instances"]["preserved"],
        true
    );
    assert_eq!(report["validation"]["tests"], "not_run");

    let after = instances(Arc::clone(candidate.revision()));
    assert_eq!(after["template_id"], before["template_id"]);
    assert_eq!(after["total_instances"], before["total_instances"]);
    assert_eq!(descriptors(&after), descriptors(&before));
    assert_ne!(after["instances"], before["instances"]);
    assert_eq!(after["name"], "retain");
    let app = candidate
        .revision()
        .sources()
        .iter()
        .find(|source| source.path() == "src/app.spx")
        .unwrap();
    assert!(app.source().contains(" as public_value"));
    let core = candidate
        .revision()
        .sources()
        .iter()
        .find(|source| source.path() == "src/core.spx")
        .unwrap();
    assert!(core.source().contains("fn retain<T>"));
    assert!(core.source().contains("retain<i64>(value)"));
    assert!(core.source().contains("retain<bool>(value)"));

    let unsupported = SemanticChange::new(
        revision.project_revision(),
        &json!({"kind":"replace_function_body","target":"generic.keep","body":{"kind":"place","name":"value"}}),
    )
    .unwrap();
    // Only the display rename is admitted on a template. Every other candidate
    // intention keeps the ordinary monomorphic-target grammar rejection that
    // PROJECT-GENERIC-RENAME-V1 leaves authoritative; the change never reaches
    // a profile or linker gate.
    code(
        base.apply(base.candidate_digest(), &unsupported),
        "SPX-G225",
    );
}
