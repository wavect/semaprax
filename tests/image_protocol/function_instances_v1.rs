//! Authored, unrun navigation evidence; retained instances do not imply target execution.
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ImageFacet, ImageFacetOptions, ProjectCandidate,
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
            "spx-instance-navigation-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let fixture = Self(root.canonicalize().unwrap());
        std::fs::write(
            fixture.0.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "instance-navigation"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "instances.app"
sources = ["src/app.spx", "src/core.spx", "src/provider.spx", "src/tests.spx"]
web_exports = ["instances.public"]
tests = ["instances.tests"]
"#,
        )
        .unwrap();
        for (path, text) in [
            (
                "src/app.spx",
                r#"module instances.app;
use function @id("instances.public") from instances.core as public_value;
@id("instances.main") fn main()->i64 {public_value(0)}
"#,
            ),
            (
                "src/core.spx",
                r#"module instances.core;
use function @id("instances.answer") from instances.provider as imported_answer;
@id("instances.keep") fn keep<T>(value:T)->T requires imported_answer()==42 {let observed=imported_answer();if observed==42 {value}else{value}}
@id("instances.other") fn other<T>(value:T)->T {value}
@id("instances.unused") fn unused<T>(value:T)->T {value}
@id("instances.use-i64") fn use_i64(value:i64)->i64 {keep<i64>(value)+keep<i64>(value)}
@id("instances.use-bool") fn use_bool(value:bool)->bool {keep<bool>(value)}
@id("instances.use-other") fn use_other(value:i64)->i64 {other<i64>(value)}
@id("instances.public") fn public_value(value:i64)->i64 {imported_answer()+value}
"#,
            ),
            (
                "src/provider.spx",
                r#"module instances.provider;
@id("instances.answer") fn answer()->i64 {42}
"#,
            ),
            (
                "src/tests.spx",
                r#"module instances.tests;
use function @id("instances.public") from instances.core as public_value;
@id("instances.test") fn main()->i64 {if public_value(0)==42 {0}else{1}}
"#,
            ),
        ] {
            fixture.write(path, text);
        }
        fixture
    }
    fn write(&self, path: &str, text: &str) {
        let program = semaprax::parse(text, path).unwrap();
        std::fs::write(self.0.join(path), semaprax::format::canonical(&program)).unwrap();
    }
    fn image(&self) -> ProjectSemanticImage {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            ProjectSemanticImage::derive(snapshot.retain_revision(), snapshot.project_revision())
        })
        .unwrap()
    }
    fn bytes(&self) -> Vec<Vec<u8>> {
        [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/provider.spx",
            "src/tests.spx",
        ]
        .iter()
        .map(|path| std::fs::read(self.0.join(path)).unwrap())
        .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn options(size: usize) -> ImageFacetOptions {
    ImageFacetOptions::new(size, 1024 * 1024).unwrap()
}
fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    let errors = result.err().expect("invalid selection accepted");
    assert!(errors.iter().any(|e| e.code == expected), "{errors:?}");
}
fn list(image: &ProjectSemanticImage, target: &str, cursor: Option<&str>, size: usize) -> Value {
    serde_json::from_str(
        &image
            .function_instances(image.image_digest(), target, cursor, options(size))
            .unwrap(),
    )
    .unwrap()
}
fn handle(row: &Value, facet: ImageFacet) -> &str {
    row["facets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["facet"] == facet.name())
        .unwrap()["handle"]
        .as_str()
        .unwrap()
}
fn page(
    image: &ProjectSemanticImage,
    target: &str,
    row: &Value,
    facet: ImageFacet,
    cursor: Option<&str>,
    size: usize,
) -> Value {
    serde_json::from_str(
        &image
            .expand_instance_facet(
                image.image_digest(),
                target,
                row["instance_id"].as_str().unwrap(),
                facet,
                handle(row, facet),
                cursor,
                options(size),
            )
            .unwrap(),
    )
    .unwrap()
}
fn all(
    image: &ProjectSemanticImage,
    target: &str,
    row: &Value,
    facet: ImageFacet,
    size: usize,
) -> Vec<Value> {
    let mut result = Vec::new();
    let mut cursor = None;
    for _ in 0..128 {
        let current = page(image, target, row, facet, cursor.as_deref(), size);
        assert_eq!(current["schema"], "semaprax.image-instance-facet.v1");
        assert_eq!(current.as_object().unwrap().len(), 21);
        assert_eq!(current["image_revision"], image.image_digest());
        assert_eq!(current["template_id"], target);
        assert_eq!(current["instance_id"], row["instance_id"]);
        assert_eq!(current["type_arguments"], row["type_arguments"]);
        assert_eq!(current["offset"], result.len());
        assert_eq!(current["source_authority"], false);
        assert_eq!(current["target_execution"], false);
        result.extend(current["items"].as_array().unwrap().iter().cloned());
        if current["next_cursor"].is_null() {
            assert_eq!(current["total_items"], result.len());
            return result;
        }
        let next = current["next_cursor"].as_str().unwrap().to_owned();
        assert_ne!(cursor.as_ref(), Some(&next));
        cursor = Some(next);
    }
    panic!("nonterminating instance facet pagination")
}
fn without_i64(image: &ProjectSemanticImage) -> ProjectCandidate {
    let base = ProjectCandidate::open(
        Arc::clone(image.revision()),
        image.revision().project_revision(),
    )
    .unwrap();
    let change=SemanticChange::new(base.revision().project_revision(),&json!({"kind":"replace_function_body","target":"instances.use-i64","body":{"kind":"i64","value":0}})).unwrap();
    base.apply(base.candidate_digest(), &change).unwrap()
}

#[test]
fn source_only_instances_match_retained_owner_evidence_and_never_become_template_plans() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let image = fixture.image();
    let old_image = image.to_json().to_owned();
    let old_summary = image
        .function_summary(image.image_digest(), "instances.public")
        .unwrap();
    let report = list(&image, "instances.keep", None, 128);
    assert_eq!(report["schema"], "semaprax.image-function-instances.v1");
    assert_eq!(report.as_object().unwrap().len(), 20);
    assert_eq!(report["template_id"], "instances.keep");
    assert_eq!(report["type_parameter_count"], 1);
    assert_eq!(report["total_instances"], 2);
    assert_eq!(report["source_authority"], false);
    assert_eq!(report["target_execution"], false);
    assert_eq!(
        report["nonclaims"],
        json!([
            "no_source_or_commit_authority",
            "no_target_execution_or_test_coverage",
            "retained_instances_not_all_possible_instantiations",
            "template_spans_are_source_provenance_not_executed_sites",
            "no_external_or_dynamic_callers"
        ])
    );
    assert!(report["next_cursor"].is_null());
    let source = image
        .revision()
        .sources()
        .iter()
        .find(|s| s.path() == "src/core.spx")
        .unwrap();
    assert_eq!(report["path"], source.path());
    assert_eq!(report["module"], "instances.core");
    assert_eq!(report["source_revision"], source.source_revision());
    assert_eq!(report["source_digest"], source.source_digest());
    let span = &report["template_span"];
    assert!(source.source()
        [span["start"].as_u64().unwrap() as usize..span["end"].as_u64().unwrap() as usize]
        .contains("fn keep<T>"));
    // Independent existing report owner exposes the original checked instances
    // when removing their i64 call sites changes the inventory.
    let changed = without_i64(&image);
    let delta: Value =
        serde_json::from_str(&changed.ownership_delta(changed.candidate_digest()).unwrap())
            .unwrap();
    let template = delta["functions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == "instances.keep")
        .unwrap();
    assert_eq!(
        template["base"]["hir_availability"],
        "retained_checked_template"
    );
    assert!(template["base"]["cleanup_plan"].is_null());
    for row in report["instances"].as_array().unwrap() {
        let owner = template["base"]["instances"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["id"] == row["instance_id"])
            .unwrap();
        assert_eq!(row["type_arguments"], owner["type_arguments"]);
        assert_eq!(row["parameter_count"], 1);
        assert_eq!(row["return_type_id"], row["type_arguments"][0]);
        assert_eq!(row["requires_count"], 1);
        assert_eq!(row["facets"].as_array().unwrap().len(), 9);
        let signature = all(&image, "instances.keep", row, ImageFacet::Signature, 1);
        assert!(signature.iter().any(|item| item["kind"] == "parameter"
            && item["type_id"] == row["type_arguments"][0]
            && item["ownership"] == "value"));
        let contracts = all(&image, "instances.keep", row, ImageFacet::Contracts, 1);
        assert_eq!(contracts.len(), 1);
        assert_eq!(contracts[0]["phase"], "requires");
        assert_eq!(
            contracts[0]["expression"]["left"]["callee"],
            "instances.answer"
        );
        let callers = all(&image, "instances.keep", row, ImageFacet::Callers, 1);
        assert_eq!(
            callers.len(),
            1,
            "do not conflate two concrete type arguments"
        );
        let is_i64 = row["type_arguments"] == json!(["i64"]);
        assert_eq!(
            callers[0]["caller_id"],
            if is_i64 {
                "instances.use-i64"
            } else {
                "instances.use-bool"
            }
        );
        assert_eq!(callers[0]["caller_kind"], "function");
        assert!(callers[0]["caller_template_id"].is_null());
        assert_eq!(callers[0]["call_sites"], if is_i64 { 2 } else { 1 });
        assert_eq!(callers[0]["phase"], "body");
        assert_eq!(callers[0]["path"], "src/core.spx");
        assert_eq!(callers[0]["source_revision"], source.source_revision());
        assert_eq!(callers[0]["source_digest"], source.source_digest());
        let relationships = all(
            &image,
            "instances.keep",
            row,
            ImageFacet::Relationships,
            128,
        );
        let entry = relationships
            .iter()
            .find(|item| item["kind"] == "entry_relationship")
            .unwrap();
        assert_eq!(entry["in_entry_instance_inventory"], false);
        assert_eq!(entry["executed"], false);
        let tests = relationships
            .iter()
            .find(|item| item["kind"] == "test_relationship")
            .unwrap();
        assert_eq!(tests["in_test_instance_inventory"], false);
        assert_eq!(tests["coverage"], "not_inferred");
        let exports = relationships
            .iter()
            .find(|item| item["kind"] == "export_relationship")
            .unwrap();
        assert_eq!(exports["template_selected_web_export"], false);
        assert_eq!(exports["instance_export"], "not_inferred");
        assert_eq!(exports["artifact_emitted"], false);
        for facet in ImageFacet::ALL {
            assert_eq!(
                all(&image, "instances.keep", row, facet, 1),
                all(&image, "instances.keep", row, facet, 128)
            );
        }
    }
    // These generic source helpers never enter the admitted executable roots.
    assert!(image
        .revision()
        .entry_program()
        .function_instances
        .is_empty());
    assert!(!image
        .revision()
        .entry_program()
        .functions
        .iter()
        .any(|f| f.id.as_str() == "instances.use-i64"));
    let empty = list(&image, "instances.unused", None, 1);
    assert_eq!(empty["total_instances"], 0);
    assert_eq!(empty["instances"], json!([]));
    assert!(empty["next_cursor"].is_null());
    code(
        image.function_summary(image.image_digest(), "instances.keep"),
        "SPX-G227",
    );
    assert_eq!(image.to_json(), old_image);
    assert_eq!(
        image
            .function_summary(image.image_digest(), "instances.public")
            .unwrap(),
        old_summary
    );
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn opaque_references_bind_template_instance_facet_page_shape_and_image() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let image = fixture.image();
    let first = list(&image, "instances.keep", None, 1);
    let cursor = first["next_cursor"].as_str().unwrap();
    let second = list(&image, "instances.keep", Some(cursor), 1);
    let mut rows = first["instances"].as_array().unwrap().clone();
    rows.extend(second["instances"].as_array().unwrap().iter().cloned());
    assert_eq!(
        json!(rows),
        list(&image, "instances.keep", None, 128)["instances"]
    );
    assert!(second["next_cursor"].is_null());
    code(
        image.function_instances(
            image.image_digest(),
            "instances.other",
            Some(cursor),
            options(1),
        ),
        "SPX-G229",
    );
    code(
        image.function_instances(
            image.image_digest(),
            "instances.keep",
            Some(cursor),
            options(2),
        ),
        "SPX-G229",
    );
    code(
        image.function_instances(
            image.image_digest(),
            "instances.keep",
            Some("bad cursor"),
            options(1),
        ),
        "SPX-G229",
    );
    for target in ["instances.missing", "instances.public"] {
        code(
            image.function_instances(image.image_digest(), target, None, options(1)),
            "SPX-G227",
        );
    }
    let row = &rows[0];
    let id = row["instance_id"].as_str().unwrap();
    let token = handle(row, ImageFacet::Signature);
    code(
        image.expand_instance_facet(
            image.image_digest(),
            "instances.other",
            id,
            ImageFacet::Signature,
            token,
            None,
            options(1),
        ),
        "SPX-G227",
    );
    code(
        image.expand_instance_facet(
            image.image_digest(),
            "instances.keep",
            id,
            ImageFacet::Contracts,
            token,
            None,
            options(1),
        ),
        "SPX-G229",
    );
    code(
        image.expand_instance_facet(
            image.image_digest(),
            "instances.keep",
            rows[1]["instance_id"].as_str().unwrap(),
            ImageFacet::Signature,
            token,
            None,
            options(1),
        ),
        "SPX-G229",
    );
    let signature = page(
        &image,
        "instances.keep",
        row,
        ImageFacet::Signature,
        None,
        1,
    );
    code(
        image.expand_instance_facet(
            image.image_digest(),
            "instances.keep",
            id,
            ImageFacet::Signature,
            token,
            signature["next_cursor"].as_str(),
            options(2),
        ),
        "SPX-G229",
    );
    code(
        image.expand_instance_facet(
            image.image_digest(),
            "instances.keep",
            &"x".repeat(65537),
            ImageFacet::Signature,
            token,
            None,
            options(1),
        ),
        "SPX-G228",
    );
    code(
        image.function_instances(image.image_digest(), &"x".repeat(4097), None, options(1)),
        "SPX-G228",
    );
    code(ImageFacetOptions::new(0, 65536), "SPX-G228");
    code(ImageFacetOptions::new(129, 65536), "SPX-G228");
    assert!(
        image
            .function_instances(image.image_digest(), "instances.keep", None, options(1))
            .unwrap()
            .len()
            > 1024
    );
    code(
        image.function_instances(
            image.image_digest(),
            "instances.keep",
            None,
            ImageFacetOptions::new(1, 1024).unwrap(),
        ),
        "SPX-G228",
    );
    let changed = without_i64(&image);
    let new_image = ProjectSemanticImage::derive(
        Arc::clone(changed.revision()),
        changed.revision().project_revision(),
    )
    .unwrap();
    assert_eq!(
        list(&new_image, "instances.keep", None, 128)["total_instances"],
        1
    );
    code(
        new_image.function_instances(image.image_digest(), "instances.keep", None, options(1)),
        "SPX-G221",
    );
    code(
        new_image.function_instances(
            new_image.image_digest(),
            "instances.keep",
            Some(cursor),
            options(1),
        ),
        "SPX-G229",
    );
    let surviving = list(&new_image, "instances.keep", None, 128)["instances"][0].clone();
    let old_surviving = rows
        .iter()
        .find(|item| item["type_arguments"] == surviving["type_arguments"])
        .unwrap();
    code(
        new_image.expand_instance_facet(
            new_image.image_digest(),
            "instances.keep",
            surviving["instance_id"].as_str().unwrap(),
            ImageFacet::Signature,
            handle(old_surviving, ImageFacet::Signature),
            None,
            options(1),
        ),
        "SPX-G229",
    );
    assert_eq!(list(&image, "instances.keep", None, 1), first);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn cross_module_template_import_is_an_existing_admission_error_not_a_navigation_edge() {
    let fixture = Fixture::new();
    fixture.write(
        "src/provider.spx",
        r#"module instances.provider;
@id("instances.answer") fn answer()->i64 {42}
@id("instances.external-template") fn external<T>(value:T)->T {value}
"#,
    );
    let core=std::fs::read_to_string(fixture.0.join("src/core.spx")).unwrap().replace("module instances.core;","module instances.core;\nuse function @id(\"instances.external-template\") from instances.provider as external;");
    fixture.write("src/core.spx", &core);
    let disk = fixture.bytes();
    code(
        with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
            ProjectSemanticImage::derive(snapshot.retain_revision(), snapshot.project_revision())
        }),
        "SPX-G172",
    );
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn generic_to_generic_relay_is_rejected_before_inventing_concrete_caller_evidence() {
    let fixture = Fixture::new();
    let core = std::fs::read_to_string(fixture.0.join("src/core.spx")).unwrap()
        + r#"
@id("instances.relay") fn relay<T>(value:T)->T {let observed=keep<i64>(1);value}
@id("instances.use-relay") fn use_relay(value:bool)->bool {relay<bool>(value)}
"#;
    fixture.write("src/core.spx", &core);
    let disk = fixture.bytes();
    // Generic Functions v1 explicitly forbids this edge. The navigation
    // collector's defensive concrete-caller branch does not admit new source
    // or justify claiming that this relay has a retained executable instance.
    code(
        with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
            ProjectSemanticImage::derive(snapshot.retain_revision(), snapshot.project_revision())
        }),
        "SPX-T226",
    );
    assert_eq!(fixture.bytes(), disk);
}
