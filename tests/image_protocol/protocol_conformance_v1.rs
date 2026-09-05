//! Source-backed protocol image evidence; authored and deliberately unrun.
use semaprax::project::{
    with_authenticated_project, ProjectSemanticImage, IMAGE_PROTOCOL_CONFORMANCE_SCHEMA,
};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-image-protocols-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/core.spx",
            "src/app.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(example.join(path), root.join(path)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn image(&self) -> ProjectSemanticImage {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            ProjectSemanticImage::derive(snapshot.retain_revision(), snapshot.project_revision())
        })
        .unwrap()
    }
    fn append(&self, path: &str, addition: &str) {
        let file = self.0.join(path);
        let source = std::fs::read_to_string(&file).unwrap() + addition;
        let program = semaprax::parse(&source, path).unwrap();
        std::fs::write(file, semaprax::format::canonical(&program)).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn conformance_binds_exact_source_and_image_without_runtime_graph_nodes() {
    let fixture = Fixture::new();
    let manifest = fixture.0.join("semaprax.toml");
    let source = std::fs::read_to_string(&manifest)
        .unwrap()
        .replace(
            "schema = \"semaprax.project.v1\"\nname = \"calculator\"",
            "schema = \"semaprax.project.v8\"\nname = \"calculator\"\nversion = \"1.0.0\"\nprofile = \"owned-data-api.v1\"",
        )
        .lines()
        .map(|line| {
            if line.starts_with("web_exports = ") {
                "web_exports = [\"calculator.add\"]"
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n") + "\n";
    std::fs::write(&manifest, source).unwrap();
    let empty = fixture.image();
    let empty_report: Value =
        serde_json::from_str(&empty.protocol_conformance(empty.image_digest()).unwrap()).unwrap();
    assert_eq!(empty_report["modules"], serde_json::json!([]));
    fixture.append(
        "src/core.spx",
        r#"
@id("shape.point") record Point { @id("shape.point.x") x: i64, }
@id("shape.protocol") protocol Shape {
    @id("shape.area") fn area(self: Self) -> i64;
}
@id("shape.point.area") fn point_area(self: Point) -> i64 { self.x }
@id("shape.point.impl") impl "shape.protocol" for "shape.point" {
    "shape.area" = "shape.point.area";
}
"#,
    );
    let image = fixture.image();
    assert_ne!(image.image_digest(), empty.image_digest());
    let report = image.protocol_conformance(image.image_digest()).unwrap();
    assert!(!report.ends_with('\n'));
    image
        .verify_protocol_conformance(image.image_digest(), report.as_bytes())
        .unwrap();
    let value: Value = serde_json::from_str(&report).unwrap();
    assert_eq!(value["schema"], IMAGE_PROTOCOL_CONFORMANCE_SCHEMA);
    assert_eq!(value["image_revision"], image.image_digest());
    assert_eq!(value["project_source_admission"], true);
    let module = &value["modules"][0];
    let source = image
        .revision()
        .sources()
        .iter()
        .find(|s| s.path() == "src/core.spx")
        .unwrap();
    assert_eq!(module["source_digest"], source.source_digest());
    let local = &module["declarations"];
    assert_eq!(local["full_source_admitted"], false);
    assert_eq!(local["implementations"][0]["id"], "shape.point.impl");
    assert_eq!(
        local["implementations"][0]["members"][0]["function_id"],
        "shape.point.area"
    );
    assert!(image
        .symbol(image.image_digest(), "shape.point.impl")
        .is_err());
    assert_eq!(
        image
            .protocol_conformance(empty.image_digest())
            .unwrap_err()[0]
            .code,
        "SPX-G221"
    );
    let mut modified = report.into_bytes();
    modified.push(b'\n');
    assert_eq!(
        image
            .verify_protocol_conformance(image.image_digest(), &modified)
            .unwrap_err()[0]
            .code,
        "SPX-G221"
    );
}

#[test]
fn scalar_project_still_rejects_record_conformance_subjects() {
    let fixture = Fixture::new();
    fixture.append(
        "src/core.spx",
        r#"
@id("shape.point") record Point { @id("shape.point.x") x: i64, }
@id("shape.point.area") fn point_area(self: Point) -> i64 { self.x }
"#,
    );
    let result = with_authenticated_project(&fixture.0.join("semaprax.toml"), |_| Ok(()));
    let diagnostics = result.unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-G174"
                && diagnostic
                    .message
                    .contains("signature outside the selected profile")),
        "{diagnostics:?}"
    );
}

#[test]
fn imported_identity_cannot_become_local_conformance_through_synthetic_stub() {
    let fixture = Fixture::new();
    fixture.append(
        "src/app.spx",
        r#"
@id("shape.app.point") record Point { @id("shape.app.point.x") x: i64, }
@id("shape.app.protocol") protocol Shape {
    @id("shape.app.area") fn area(self: Self) -> i64;
}
@id("shape.app.impl") impl "shape.app.protocol" for "shape.app.point" {
    "shape.app.area" = "calculator.add";
}
"#,
    );
    let result = with_authenticated_project(&fixture.0.join("semaprax.toml"), |_| Ok(()));
    assert!(result
        .unwrap_err()
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-Q107"));
}

#[test]
fn source_only_protocol_ids_are_globally_unique_across_project_modules() {
    let fixture = Fixture::new();
    fixture.append(
        "src/core.spx",
        r#"
@id("shared.protocol") protocol Shape {
    @id("core.shape.area") fn area(self: Self) -> i64;
}
"#,
    );
    fixture.append(
        "src/app.spx",
        r#"
@id("shared.protocol") protocol Shape {
    @id("app.shape.area") fn area(self: Self) -> i64;
}
"#,
    );
    let result = with_authenticated_project(&fixture.0.join("semaprax.toml"), |_| Ok(()));
    assert!(result
        .unwrap_err()
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-Q108"));
}
