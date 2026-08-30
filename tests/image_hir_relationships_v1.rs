//! HIR relationship facets: authored regression evidence, deliberately unrun.
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ImageFacet, ImageFacetOptions, ProjectSemanticImage,
};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new(example: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-image-hir-relationships-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples")
            .join(example);
        std::fs::copy(source.join("semaprax.toml"), root.join("semaprax.toml")).unwrap();
        for entry in std::fs::read_dir(source.join("src")).unwrap() {
            let entry = entry.unwrap();
            if entry.path().extension().is_some_and(|ext| ext == "spx") {
                std::fs::copy(entry.path(), root.join("src").join(entry.file_name())).unwrap();
            }
        }
        Self(root)
    }
    fn image(&self) -> ProjectSemanticImage {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            ProjectSemanticImage::derive(snapshot.retain_revision(), snapshot.project_revision())
        })
        .unwrap()
    }
    fn replace_core(&self, transform: impl FnOnce(String) -> String) {
        let path = self.0.join("src/core.spx");
        let source = transform(std::fs::read_to_string(&path).unwrap());
        let canonical = semaprax::format::canonical(&semaprax::parse(&source, "core.spx").unwrap());
        std::fs::write(path, canonical).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    match result {
        Ok(_) => panic!("expected {expected}"),
        Err(errors) => assert!(
            errors.iter().any(|error| error.code == expected),
            "{errors:?}"
        ),
    }
}
fn handle(image: &ProjectSemanticImage, id: &str, facet: ImageFacet) -> String {
    let summary: Value =
        serde_json::from_str(&image.function_summary(image.image_digest(), id).unwrap()).unwrap();
    summary["facets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["facet"] == facet.name())
        .unwrap()["handle"]
        .as_str()
        .unwrap()
        .to_owned()
}
fn page(
    image: &ProjectSemanticImage,
    id: &str,
    facet: ImageFacet,
    cursor: Option<&str>,
    size: usize,
) -> Value {
    serde_json::from_str(
        &image
            .expand_facet(
                image.image_digest(),
                id,
                facet,
                &handle(image, id, facet),
                cursor,
                ImageFacetOptions::new(size, 1024 * 1024).unwrap(),
            )
            .unwrap(),
    )
    .unwrap()
}
fn all(image: &ProjectSemanticImage, id: &str, facet: ImageFacet) -> Vec<Value> {
    let mut cursor = None::<String>;
    let mut values = Vec::new();
    loop {
        let response = page(image, id, facet, cursor.as_deref(), 2);
        values.extend(response["items"].as_array().unwrap().iter().cloned());
        cursor = response["next_cursor"].as_str().map(str::to_owned);
        if cursor.is_none() {
            assert_eq!(
                values.len(),
                response["total_items"].as_u64().unwrap() as usize
            );
            break;
        }
    }
    values
}

#[test]
fn access_rows_bind_source_and_cover_contracts_nested_writes_and_paging() {
    let fixture = Fixture::new("calculator-project");
    fixture.replace_core(|source|source + "\n@id(\"calculator.access\") fn access(value: i64) -> i64 requires value >= 0 ensures result >= 0 { let mut total = value; if value > 0 { total = total + 1; total } else { total } }\n");
    let image = fixture.image();
    let before = image.to_json().to_owned();
    let rows = all(&image, "calculator.access", ImageFacet::DataAccess);
    assert!(rows.iter().any(|row| row["phase"] == "requires"));
    assert!(rows.iter().any(|row| row["phase"] == "ensures"));
    let initialize = rows
        .iter()
        .find(|row| row["edge_kind"] == "binding_initialize")
        .unwrap();
    let write = rows
        .iter()
        .find(|row| row["edge_kind"] == "binding_write")
        .unwrap();
    assert_eq!(initialize["value_id"], write["value_id"]);
    assert!(rows
        .iter()
        .any(|row| row["edge_kind"] == "place_read" && row["value_id"] == write["value_id"]));
    let source = std::fs::read(fixture.0.join("src/core.spx")).unwrap();
    for row in &rows {
        assert_eq!(row["schema"], "semaprax.image-hir-relationship.v1");
        assert_eq!(row["image_revision"], image.image_digest());
        assert_eq!(row["function_id"], "calculator.access");
        assert_eq!(row["evidence_owner"], "retained_validated_module_hir");
        assert_eq!(row["runtime_execution"], false);
        assert!(row["source_digest"]
            .as_str()
            .unwrap()
            .starts_with("sha256:"));
        assert!(row["span"]["end"].as_u64().unwrap() <= source.len() as u64);
        assert!(!row["expression_id"].as_str().unwrap().is_empty());
    }
    assert_eq!(
        rows,
        page(
            &image,
            "calculator.access",
            ImageFacet::DataAccess,
            None,
            128
        )["items"]
    );
    assert_eq!(image.to_json(), before);
    let old = handle(&image, "calculator.access", ImageFacet::DataAccess);
    code(
        image.expand_facet(
            image.image_digest(),
            "calculator.access",
            ImageFacet::UnsafeBoundaries,
            &old,
            None,
            ImageFacetOptions::default(),
        ),
        "SPX-G229",
    );
}

#[test]
fn actual_field_identities_and_owned_call_contexts_are_retained() {
    let fixture = Fixture::new("frame-payload-project");
    let path = fixture.0.join("src/frame.spx");
    let source = std::fs::read_to_string(&path).unwrap()
        + r#"
@id("facet.metric") record Metric { @id("facet.metric.count") count: i64, }
@id("facet.field") fn field(value: i64) -> i64 { let mut item = Metric { count: value }; item.count = value + 1; item.count }
@id("facet.owned") fn owned(value: own Bytes) -> Bytes { value }
@id("facet.relay") fn relay(value: own Bytes) -> Bytes { owned(value) }
@id("facet.count") fn count(value: borrow Slice<u8>) -> usize { byte_len(value) }
@id("facet.borrow") fn borrowed(value: borrow Slice<u8>) -> usize { count(value) }
"#;
    std::fs::write(
        &path,
        semaprax::format::canonical(&semaprax::parse(&source, "src/frame.spx").unwrap()),
    )
    .unwrap();
    let image = fixture.image();
    let fields = all(&image, "facet.field", ImageFacet::DataAccess);
    assert!(fields.iter().any(
        |row| row["edge_kind"] == "field_initialize" && row["field_id"] == "facet.metric.count"
    ));
    let write = fields
        .iter()
        .find(|row| row["edge_kind"] == "binding_write")
        .unwrap();
    assert_eq!(write["field_id"], "facet.metric.count");
    assert!(fields.iter().any(
        |row| row["projections"].as_array().is_some_and(|parts| parts
            .iter()
            .any(|part| part["field_id"] == "facet.metric.count"))
    ));
    let borrowed = all(&image, "facet.borrow", ImageFacet::DataAccess);
    assert!(borrowed
        .iter()
        .any(|row| row["edge_kind"] == "place_borrow" && row["use_context"] == "borrow"));
    for id in ["facet.owned", "facet.relay"] {
        let rows = all(&image, id, ImageFacet::DataAccess);
        let moved = rows
            .iter()
            .find(|row| row["edge_kind"] == "place_move")
            .unwrap();
        assert_eq!(moved["expression_ownership"], "own");
        assert_eq!(moved["use_context"], "consume");
    }
}

#[test]
fn unsafe_facet_does_not_widen_current_project_admission() {
    let fixture = Fixture::new("calculator-project");
    let image = fixture.image();
    assert!(all(&image, "calculator.add", ImageFacet::UnsafeBoundaries).is_empty());
    let summary: Value = serde_json::from_str(
        &image
            .function_summary(image.image_digest(), "calculator.add")
            .unwrap(),
    )
    .unwrap();
    let names = summary["facets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["facet"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec![
            "signature",
            "contracts",
            "callers",
            "ownership",
            "loans",
            "cleanup",
            "relationships",
            "data-access",
            "unsafe-boundaries"
        ]
    );
    fixture.replace_core(|source|source.replace("module calculator.core;","module calculator.core;\npermit { unsafe }")+"\n@id(\"facet.audit\") fn audit(value: i64) -> i64 { @audit(\"ordinary checked arithmetic\") unsafe { let copied = value; copied } value }\n");
    code(
        with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
            ProjectSemanticImage::derive(snapshot.retain_revision(), snapshot.project_revision())
        }),
        "SPX-G172",
    );
}
