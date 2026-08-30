//! Authored facet evidence. Tests are intentionally not run in this change.
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
            "spx-image-facets-{}-{}",
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
        Self(root.canonicalize().unwrap())
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
fn summary_and_facets_preserve_v1_image_bytes_and_retain_actual_contracts() {
    let fixture = Fixture::new("calculator-project");
    let image = fixture.image();
    let original = image.to_json().to_owned();
    let summary = image
        .function_summary(image.image_digest(), "calculator.divide")
        .unwrap();
    let other = Fixture::new("calculator-project").image();
    assert_eq!(
        summary,
        other
            .function_summary(other.image_digest(), "calculator.divide")
            .unwrap()
    );
    let signature = all(&image, "calculator.divide", ImageFacet::Signature);
    assert_eq!(
        signature
            .iter()
            .filter(|item| item["kind"] == "parameter")
            .count(),
        2
    );
    let contracts = all(&image, "calculator.divide", ImageFacet::Contracts);
    assert_eq!(contracts.len(), 1);
    assert_eq!(contracts[0]["phase"], "requires");
    assert_eq!(contracts[0]["expression"]["kind"], "binary");
    assert_eq!(contracts[0]["expression"]["op"], "!=");
    assert!(contracts[0]["expression_id"].as_str().is_some());
    for facet in ImageFacet::ALL {
        let _ = all(&image, "calculator.divide", facet);
    }
    assert_eq!(image.to_json(), original);
    assert!(!fixture.0.join(".semaprax-images").exists());
}

#[test]
fn callers_include_local_and_cross_file_callers_from_all_regions() {
    let fixture = Fixture::new("calculator-project");
    fixture.replace_core(|mut source| {
        source.push_str("\n@id(\"calculator.local\") fn local(value: i64) -> i64 requires add(value, 0) == value { add(value, 0) }\n"); source
    });
    let image = fixture.image();
    let callers = all(&image, "calculator.add", ImageFacet::Callers);
    assert!(callers
        .iter()
        .any(|item| item["caller"] == "calculator.local"
            && item["phase"] == "requires"
            && item["cross_file"] == false));
    assert!(callers
        .iter()
        .any(|item| item["caller"] == "calculator.local"
            && item["phase"] == "body"
            && item["call_sites"] == 1));
    assert!(callers
        .iter()
        .any(|item| item["caller"] == "calculator.app.main" && item["cross_file"] == true));
    assert!(callers
        .iter()
        .any(|item| item["caller"] == "calculator.tests.main" && item["cross_file"] == true));
}

#[test]
fn handles_and_cursors_reject_cross_target_cross_facet_and_new_revision_replay() {
    let fixture = Fixture::new("calculator-project");
    let old = fixture.image();
    let id = "calculator.divide";
    let facet = ImageFacet::Signature;
    let token = handle(&old, id, facet);
    let first = page(&old, id, facet, None, 1);
    let cursor = first["next_cursor"].as_str().unwrap();
    code(
        old.expand_facet(
            old.image_digest(),
            "calculator.add",
            facet,
            &token,
            None,
            ImageFacetOptions::default(),
        ),
        "SPX-G229",
    );
    code(
        old.expand_facet(
            old.image_digest(),
            id,
            ImageFacet::Contracts,
            &token,
            None,
            ImageFacetOptions::default(),
        ),
        "SPX-G229",
    );
    code(
        old.expand_facet(
            old.image_digest(),
            id,
            facet,
            &token,
            Some(cursor),
            ImageFacetOptions::new(2, 65536).unwrap(),
        ),
        "SPX-G229",
    );
    code(
        old.expand_facet(
            old.image_digest(),
            id,
            facet,
            &token,
            Some("01:invalid"),
            ImageFacetOptions::default(),
        ),
        "SPX-G229",
    );
    fixture.replace_core(|source| source.replace("left + right", "right + left"));
    let new = fixture.image();
    code(
        new.expand_facet(
            new.image_digest(),
            id,
            facet,
            &token,
            None,
            ImageFacetOptions::default(),
        ),
        "SPX-G229",
    );
    assert_eq!(first, page(&old, id, facet, None, 1));
    code(
        old.function_summary(old.image_digest(), "missing.function"),
        "SPX-G227",
    );
    code(ImageFacetOptions::new(129, 65536), "SPX-G228");
    code(ImageFacetOptions::new(1, 1024 * 1024 + 1), "SPX-G228");
    code(ImageFacet::parse("unknown"), "SPX-G227");
}

#[test]
fn owned_borrowing_facets_preserve_plan_vectors_and_do_not_claim_execution() {
    let fixture = Fixture::new("binary-frame-project");
    let image = fixture.image();
    let id = "binary-frame.checksum";
    let loans = all(&image, id, ImageFacet::Loans);
    assert!(loans.iter().any(|item| item["section"] == "loans"));
    assert!(loans.iter().any(|item| item["section"] == "edges"));
    let cleanup = all(&image, id, ImageFacet::Cleanup);
    let blocks = cleanup
        .iter()
        .filter(|item| item["section"] == "blocks")
        .collect::<Vec<_>>();
    assert!(!blocks.is_empty());
    for (index, item) in blocks.iter().enumerate() {
        assert_eq!(item["index"], index);
    }
    let ownership = all(&image, id, ImageFacet::Ownership);
    assert!(ownership
        .iter()
        .any(|item| item["kind"] == "structural_slot"));
    let relationships = all(&image, id, ImageFacet::Relationships);
    assert!(relationships
        .iter()
        .any(|item| item["native_target_check"] == "not_performed"));
    assert!(relationships
        .iter()
        .any(|item| item["coverage"] == "not_inferred" && item["executed"] == false));
}
