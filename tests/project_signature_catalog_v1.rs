//! Checked signature catalogue evidence, authored and intentionally unrun.
use semaprax::project::{with_authenticated_project, ProjectCandidate};
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
            "spx-signature-catalog-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "signature-catalog"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "catalog.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["catalog.evaluate"]
tests = ["catalog.tests"]
"#,
        )
        .unwrap();
        for (path, source) in [
            (
                "src/core.spx",
                r#"module catalog.core;
@id("catalog.pair") record Pair { @id("catalog.pair.value") value: i64, }
@id("catalog.box") record Box<T> { @id("catalog.box.value") value: T, }
@id("catalog.owned") record Owned { @id("catalog.owned.bytes") bytes: Bytes, }
@id("catalog.select") fn select(pair: Pair, boxed: Box<i64>, flag: bool) -> i64 { pair.value + boxed.value }
@id("catalog.owned-select") fn owned_select(value: own Owned) -> i64 { 0 }
@id("catalog.borrowed") fn borrowed(value: borrow Slice<u8>) -> i64 { 0 }
@id("catalog.bytes") fn bytes(value: own Bytes) -> Bytes { value }
@id("catalog.evaluate") fn evaluate(input: i64) -> i64 { select(Pair { value: input }, Box<i64> { value: 2 }, true) }
"#,
            ),
            (
                "src/app.spx",
                r#"module catalog.app;
use type @id("catalog.pair") from catalog.core as Metric;
use function @id("catalog.evaluate") from catalog.core as evaluate;
@id("catalog.alias-select") fn alias_select(value: Metric) -> i64 { value.value }
@id("catalog.app.main") fn main() -> i64 { evaluate(alias_select(Metric { value: 1 })) }
"#,
            ),
            (
                "src/tests.spx",
                r#"module catalog.tests;
use function @id("catalog.evaluate") from catalog.core as evaluate;
@id("catalog.tests.main") fn main() -> i64 { if evaluate(1) == 3 { 0 } else { 1 } }
"#,
            ),
        ] {
            let program = semaprax::parse(source, path).unwrap();
            std::fs::write(root.join(path), semaprax::format::canonical(&program)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn candidate(&self) -> ProjectCandidate {
        let revision = with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap();
        ProjectCandidate::open(Arc::clone(&revision), revision.project_revision()).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn catalog(candidate: &ProjectCandidate, target: &str) -> Value {
    serde_json::from_str(&candidate.change_catalog(target).unwrap()).unwrap()
}
fn ordered(report: &Value) -> bool {
    report["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|operation| operation["kind"] == "change_function_signature")
        .is_some_and(|operation| {
            operation["exactly_one_form"]
                .as_array()
                .unwrap()
                .iter()
                .any(|form| form["selector"] == "parameters")
        })
}

#[test]
fn named_copy_catalogue_uses_checked_nominal_identity_including_generic_arguments() {
    let fixture = Fixture::new();
    let candidate = fixture.candidate();
    let report = catalog(&candidate, "catalog.select");
    assert!(ordered(&report));
    assert_eq!(report["admission"], "constructor_discovery_only");
    assert_eq!(report["requires_full_candidate_validation"], true);
    let pair = &report["parameters"][0];
    let boxed = &report["parameters"][1];
    assert_eq!(pair["type"], "Pair");
    assert_eq!(pair["type_provenance"]["declaration"], "catalog.pair");
    assert_eq!(pair["type_provenance"]["arguments"], json!([]));
    assert_eq!(boxed["type_provenance"]["declaration"], "catalog.box");
    assert_eq!(boxed["type_provenance"]["arguments"], json!(["i64"]));
    for parameter in [pair, boxed] {
        assert!(parameter["type_identity"]
            .as_str()
            .unwrap()
            .starts_with("nominal:"));
        assert_eq!(parameter["type_provenance"]["ownership"], "copy");
        assert_eq!(
            parameter["type_provenance"]["evidence_owner"],
            "retained_checked_hir"
        );
        assert_eq!(parameter["type_provenance"]["copy"], true);
        assert_eq!(parameter["type_provenance"]["sized"], true);
        assert_eq!(parameter["type_provenance"]["contains_resource"], false);
        assert_eq!(parameter["type_provenance"]["needs_drop"], false);
    }
    let alias = catalog(&candidate, "catalog.alias-select");
    assert!(ordered(&alias));
    assert_eq!(alias["parameters"][0]["type"], "Metric");
    assert_eq!(
        alias["parameters"][0]["type_identity"],
        pair["type_identity"]
    );
    assert_eq!(
        alias["parameters"][0]["type_provenance"],
        pair["type_provenance"]
    );
}

#[test]
fn scalar_and_direct_bytes_parameter_shapes_stay_unchanged() {
    let fixture = Fixture::new();
    let candidate = fixture.candidate();
    let scalar = catalog(&candidate, "catalog.evaluate");
    assert!(ordered(&scalar));
    assert_eq!(
        scalar["parameters"],
        json!([{"name":"input","type":"i64","mode":"value"}])
    );
    let bytes = catalog(&candidate, "catalog.bytes");
    assert!(ordered(&bytes));
    assert_eq!(
        bytes["parameters"],
        json!([{"name":"value","type":"Bytes","mode":"own"}])
    );
}

#[test]
fn owned_records_and_borrowed_views_do_not_advertise_ordered_mapping() {
    let fixture = Fixture::new();
    let candidate = fixture.candidate();
    for target in ["catalog.owned-select", "catalog.borrowed"] {
        let report = catalog(&candidate, target);
        assert!(!ordered(&report));
        assert!(report["parameters"][0].get("type_identity").is_none());
        assert!(report["parameters"][0].get("type_provenance").is_none());
    }
}
