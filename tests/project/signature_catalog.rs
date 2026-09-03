//! Checked signature catalogue evidence for Copy, owning and borrowed parameters.
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
@id("catalog.borrowed") fn borrowed(value: borrow Slice<u8>, text: borrow str, flag: i64) -> i64 { byte_len(value) + str_len_bytes(text) + flag }
@id("catalog.borrowed-call") fn borrowed_call(value: borrow Slice<u8>, text: borrow str) -> i64 { borrowed(value, text, 1) }
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
fn owned_records_and_borrowed_views_advertise_exact_retention_constraints() {
    let fixture = Fixture::new();
    let candidate = fixture.candidate();
    let owned = catalog(&candidate, "catalog.owned-select");
    assert!(ordered(&owned));
    assert_eq!(owned["parameters"][0]["mode"], "own");
    assert!(owned["parameters"][0]["type_identity"]
        .as_str()
        .unwrap()
        .starts_with("nominal:"));
    assert_eq!(
        owned["parameters"][0]["type_provenance"],
        json!({
            "declaration":"catalog.owned", "arguments":[], "ownership":"own",
            "evidence_owner":"retained_checked_hir", "copy":false, "sized":true,
            "contains_resource":false, "needs_drop":true
        })
    );
    let signature = owned["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|operation| operation["kind"] == "change_function_signature")
        .unwrap();
    let mapping = signature["exactly_one_form"]
        .as_array()
        .unwrap()
        .iter()
        .find(|form| form["selector"] == "parameters")
        .unwrap();
    assert!(mapping["constraints"]
        .as_array()
        .unwrap()
        .contains(&json!("checked_owning_parameters_retained_exactly_once")));

    let borrowed = catalog(&candidate, "catalog.borrowed");
    assert!(ordered(&borrowed));
    assert!(borrowed["parameters"][0].get("type_identity").is_none());
    assert!(borrowed["parameters"][0].get("type_provenance").is_none());
    assert_eq!(borrowed["parameters"][0]["mode"], "borrow");
    assert_eq!(borrowed["parameters"][1]["mode"], "borrow");
    let mapping = borrowed["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|operation| operation["kind"] == "change_function_signature")
        .unwrap()["exactly_one_form"]
        .as_array()
        .unwrap()
        .iter()
        .find(|form| form["selector"] == "parameters")
        .unwrap();
    assert!(mapping["constraints"]
        .as_array()
        .unwrap()
        .contains(&json!("borrowed_views_retained_exactly_once")));
    assert_eq!(
        mapping["borrowed_parameter_fields"],
        json!(["name", "borrow_from"])
    );
    assert_eq!(
        mapping["borrowed_parameter"],
        json!({
            "source":"authenticated_original_borrowed_view",
            "admitted_views":["borrow str","borrow Slice<u8>"],
            "caller_lowering":"reuse_exact_left_to_right_staged_view",
            "root_provenance":"ordinary_full_project_loan_and_provenance_replay",
            "source_must_be_retained_exactly_once":true,
            "new_root_or_lifetime":false,
        })
    );
}

#[test]
fn borrowed_slice_and_text_parameters_reorder_rename_and_replay() {
    let fixture = Fixture::new();
    let base = fixture.candidate();
    let change = semaprax::project::SemanticChange::new(
        base.revision().project_revision(),
        &json!({
            "kind":"change_function_signature",
            "target":"catalog.borrowed",
            "parameters":[
                {"from":"text","name":"label"},
                {"from":"flag"},
                {"from":"value","name":"bytes"}
            ]
        }),
    )
    .unwrap();
    let evolved = base.apply(base.candidate_digest(), &change).unwrap();
    let core = evolved
        .revision()
        .sources()
        .iter()
        .find(|source| source.path() == "src/core.spx")
        .unwrap()
        .source();
    assert!(core.contains("fn borrowed(label: borrow str, flag: i64, bytes: borrow Slice<u8>)"));
    assert!(core.contains("byte_len(bytes) + str_len_bytes(label) + flag"));
    assert!(core.contains("let spx_sig_stage_0 = value; let spx_sig_stage_1 = text; let spx_sig_stage_2 = 1; borrowed(spx_sig_stage_1, spx_sig_stage_2, spx_sig_stage_0)"));
    let replayed = semaprax::project::ProjectCandidate::replay(
        Arc::clone(base.base_revision()),
        base.base_revision().project_revision(),
        &[change],
        evolved.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(replayed.to_json(), evolved.to_json());
}

#[test]
fn fresh_borrowed_parameters_reuse_authenticated_staged_views_and_replay() {
    let fixture = Fixture::new();
    let base = fixture.candidate();
    let change = semaprax::project::SemanticChange::new(
        base.revision().project_revision(),
        &json!({
            "kind":"change_function_signature",
            "target":"catalog.borrowed",
            "parameters":[
                {"from":"value","name":"bytes"},
                {"name":"bytes_again","borrow_from":"value"},
                {"from":"text","name":"label"},
                {"name":"label_again","borrow_from":"text"},
                {"from":"flag"}
            ]
        }),
    )
    .unwrap();
    let evolved = base.apply(base.candidate_digest(), &change).unwrap();
    let core = evolved
        .revision()
        .sources()
        .iter()
        .find(|source| source.path() == "src/core.spx")
        .unwrap()
        .source();
    assert!(core.contains("fn borrowed(bytes: borrow Slice<u8>, bytes_again: borrow Slice<u8>, label: borrow str, label_again: borrow str, flag: i64)"));
    assert!(core.contains("let spx_sig_stage_0 = value; let spx_sig_stage_1 = text; let spx_sig_stage_2 = 1; borrowed(spx_sig_stage_0, spx_sig_stage_0, spx_sig_stage_1, spx_sig_stage_1, spx_sig_stage_2)"));
    let replayed = semaprax::project::ProjectCandidate::replay(
        Arc::clone(base.base_revision()),
        base.base_revision().project_revision(),
        &[change],
        evolved.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(replayed.to_json(), evolved.to_json());
}

#[test]
fn fresh_borrowed_parameters_reject_unknown_nonview_unretained_and_open_requests() {
    let fixture = Fixture::new();
    let base = fixture.candidate();
    for parameters in [
        json!([
            {"from":"value"},{"from":"text"},{"from":"flag"},
            {"name":"extra","borrow_from":"missing"}
        ]),
        json!([
            {"from":"value"},{"from":"text"},{"from":"flag"},
            {"name":"extra","borrow_from":"flag"}
        ]),
        json!([
            {"from":"text"},{"from":"flag"},
            {"name":"extra","borrow_from":"value"}
        ]),
        json!([
            {"from":"value"},{"from":"text"},{"from":"flag"},
            {"name":"extra","borrow_from":"value","type":"Slice<u8>"}
        ]),
        json!([
            {"from":"value"},{"from":"text"},{"from":"flag"},
            {"name":"value","borrow_from":"text"}
        ]),
    ] {
        let change = semaprax::project::SemanticChange::new(
            base.revision().project_revision(),
            &json!({
                "kind":"change_function_signature",
                "target":"catalog.borrowed",
                "parameters":parameters,
            }),
        )
        .unwrap();
        let errors = base
            .apply(base.candidate_digest(), &change)
            .err()
            .expect("unsupported borrowed source must fail closed");
        assert!(
            errors
                .iter()
                .any(|error| matches!(error.code, "SPX-G225" | "SPX-G260")),
            "{errors:?}"
        );
    }
}

#[test]
fn owned_bytes_catalogue_exposes_only_exact_replacement_and_no_external_migration() {
    let fixture = Fixture::new();
    let report = catalog(&fixture.candidate(), "catalog.bytes");
    let mapping = report["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|operation| operation["kind"] == "change_function_signature")
        .unwrap()["exactly_one_form"]
        .as_array()
        .unwrap()
        .iter()
        .find(|form| form["selector"] == "parameters")
        .unwrap();
    assert_eq!(
        mapping["owner_to_borrowed_slice_fields"],
        json!(["name", "borrow_slice_from_owner"])
    );
    let lane = &mapping["owner_to_borrowed_slice"];
    assert_eq!(lane["maximum_replacements"], 8);
    assert_eq!(lane["result"], "borrow Slice<u8>");
    assert_eq!(lane["owner_cleanup"], "caller_owned_ordinary_cleanup");
    assert_eq!(lane["full_project_replay"], true);
    assert!(lane["excludes"]
        .as_array()
        .unwrap()
        .contains(&json!("external_package_source_rewrite")));
    assert!(lane["excludes"]
        .as_array()
        .unwrap()
        .contains(&json!("additive_owner_alias")));
    assert!(lane["excludes"]
        .as_array()
        .unwrap()
        .contains(&json!("more_than_eight_owner_conversions")));
}
