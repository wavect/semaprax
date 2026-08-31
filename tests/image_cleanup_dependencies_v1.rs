//! Source-bound cleanup/loan dependency regressions, authored and unrun.
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectRevision, ProjectSemanticImage,
    SemanticChange,
};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const CORE: &str = r#"module cleanup.core;
@id("cleanup.packet") record Packet { @id("cleanup.packet.left") left:Bytes, @id("cleanup.packet.right") right:Bytes, @id("cleanup.packet.marker") marker:i64, }
@id("cleanup.copy") record CopyValue { @id("cleanup.copy.value") value:i64, }
@id("cleanup.other") record Other { @id("cleanup.other.left") left:Bytes, }
@id("cleanup.consume") fn consume(input:own Bytes)->i64 {7}
@id("cleanup.discard") fn discard(input:own Packet)->i64 {0}
@id("cleanup.forward") fn forward(input:own Packet)->Packet {input}
@id("cleanup.other-discard") fn other_discard(input:own Other)->i64 {0}
@id("cleanup.copy-read") fn copy_read(input:CopyValue)->i64 {input.value}
@id("cleanup.projected") fn projected()->i64 {
    let left_source = [8u8,9u8];
    let right_source = [7u8];
    let packet = Packet {left:bytes_copy(array_as_slice(left_source)), right:bytes_copy(array_as_slice(right_source)), marker:35};
    let view = bytes_as_slice(packet.left);
    let alias = view;
    let range = byte_range(alias,0usize,byte_len(alias));
    let sibling = consume(packet.right);
    let observed = if byte_len(range)==2usize {packet.marker} else {0};
    sibling+observed
}
@id("cleanup.public") fn public_value(value:i64)->i64 {value}
"#;
const APP: &str = r#"module cleanup.app;
use type @id("cleanup.packet") from cleanup.core as Envelope;
use function @id("cleanup.projected") from cleanup.core as projected;
@id("cleanup.app-discard") fn discard(input:own Envelope)->i64 {0}
@id("cleanup.main") fn main()->i64 {projected()}
"#;
const VARIANT: &str = r#"module cleanup.core;
@id("cleanup.choice") variant Choice {
    @id("cleanup.choice.none") None,
    @id("cleanup.choice.data") Data { @id("cleanup.choice.data.payload") payload:Bytes, @id("cleanup.choice.data.marker") marker:i64, },
    @id("cleanup.choice.error") Error { @id("cleanup.choice.error.code") code:i64, },
}
@id("cleanup.make") fn make(input:borrow Slice<u8>)->Choice {Choice::Data {payload:bytes_copy(input), marker:20}}
@id("cleanup.identity") fn identity(input:own Choice)->Choice {input}
@id("cleanup.consume-choice") fn consume(input:own Choice)->i64 {match own input {Choice::None {} => 0, Choice::Data {payload, marker} => marker, Choice::Error {code} => code,}}
@id("cleanup.public") fn public_value(value:i64)->i64 {value}
"#;
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        Self::from_sources(CORE, APP)
    }
    fn variant() -> Self {
        Self::from_sources(
            VARIANT,
            r#"module cleanup.app;
use function @id("cleanup.public") from cleanup.core as public_value;
@id("cleanup.main") fn main()->i64 {public_value(42)}
"#,
        )
    }
    fn from_sources(core: &str, app: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-cleanup-dependencies-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let fixture = Self(root.canonicalize().unwrap());
        std::fs::write(
            fixture.0.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "cleanup-dependencies"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "cleanup.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["cleanup.public"]
tests = ["cleanup.tests"]
"#,
        )
        .unwrap();
        for (path, text) in [
            ("src/core.spx", core),
            ("src/app.spx", app),
            (
                "src/tests.spx",
                r#"module cleanup.tests;
use function @id("cleanup.public") from cleanup.core as public_value;
@id("cleanup.test") fn main()->i64 {if public_value(42)==42 {0}else{1}}
"#,
            ),
        ] {
            let program = semaprax::parse(text, path).unwrap();
            std::fs::write(fixture.0.join(path), semaprax::format::canonical(&program)).unwrap();
        }
        fixture
    }
    fn revision(&self) -> Arc<ProjectRevision> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap()
    }
    fn bytes(&self) -> Vec<Vec<u8>> {
        [
            "semaprax.toml",
            "src/core.spx",
            "src/app.spx",
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
fn image(revision: &Arc<ProjectRevision>) -> ProjectSemanticImage {
    ProjectSemanticImage::derive(Arc::clone(revision), revision.project_revision()).unwrap()
}
fn report(image: &ProjectSemanticImage, target: &str) -> Value {
    serde_json::from_str(
        &image
            .cleanup_dependencies(image.image_digest(), target)
            .unwrap(),
    )
    .unwrap()
}
fn rows(report: &Value) -> &[Value] {
    report["obligations"].as_array().unwrap()
}
fn error<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    let diagnostics = result.err().expect("invalid cleanup query accepted");
    assert!(
        diagnostics.iter().any(|error| error.code == expected),
        "{diagnostics:?}"
    );
}
fn provenance(value: &Value, revision: &ProjectRevision, image: &ProjectSemanticImage) {
    assert_eq!(value["schema"], "semaprax.image-cleanup-dependencies.v1");
    assert_eq!(value["image_digest"], image.image_digest());
    assert_eq!(value["project_revision"], revision.project_revision());
    let selected = value["selected_declaration_ids"].as_array().unwrap();
    assert!(!selected.is_empty());
    for row in rows(value) {
        let path = row["path"].as_str().unwrap();
        let source = revision
            .sources()
            .iter()
            .find(|source| source.path() == path)
            .unwrap();
        assert_eq!(row["source_revision"], source.source_revision());
        assert_eq!(row["source_digest"], source.source_digest());
        assert!(row["fact"].is_object());
        assert!(row["coordinate"].is_object());
        assert!(row["function_id"].is_string());
        assert!(!row["reason"].as_str().unwrap().is_empty());
        let matched = row["matched_declaration_ids"].as_array().unwrap();
        assert!(!matched.is_empty());
        assert!(matched.iter().all(|id| selected.contains(id)));
    }
}

#[test]
fn projected_field_selects_real_cleanup_and_loan_dependencies_across_source_aliases() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let image = image(&revision);
    let value = report(&image, "cleanup.packet.left");
    provenance(&value, &revision, &image);
    assert_eq!(
        value["selected_declaration_ids"],
        json!(["cleanup.packet.left"])
    );
    assert_eq!(value["source_binding"]["path"], "src/core.spx");
    assert!(!rows(&value).is_empty());
    for function in [
        "cleanup.discard",
        "cleanup.forward",
        "cleanup.projected",
        "cleanup.app-discard",
    ] {
        assert!(
            rows(&value)
                .iter()
                .any(|row| row["function_id"] == function),
            "missing {function}: {value}"
        );
    }
    assert!(!rows(&value)
        .iter()
        .any(|row| row["function_id"] == "cleanup.other-discard"
            || row["function_id"] == "cleanup.copy-read"));
    for facet in ["inventory_flag", "cleanup_slot", "loan", "loan_edge"] {
        assert!(
            rows(&value).iter().any(|row| row["facet"] == facet),
            "missing {facet}: {value}"
        );
    }
    let loans = rows(&value)
        .iter()
        .filter(|row| row["facet"] == "loan")
        .collect::<Vec<_>>();
    assert!(!loans.is_empty());
    assert!(loans
        .iter()
        .all(|row| row["function_id"] == "cleanup.projected"));
    assert!(loans.iter().all(|row| row["fact"]["origin"]["projections"]
        .as_array()
        .unwrap()
        .iter()
        .any(|projection| projection["field"] == "cleanup.packet.left")));
    assert!(loans.iter().any(|row| !row["fact"]["parent"].is_null()));
    let other = report(&image, "cleanup.other.left");
    assert!(rows(&other)
        .iter()
        .any(|row| row["function_id"] == "cleanup.other-discard"));
    assert!(!rows(&other)
        .iter()
        .any(|row| row["function_id"] == "cleanup.projected"));
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn copy_fields_do_not_inherit_sibling_byte_finalizers_or_loan_obligations() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let image = image(&revision);
    let copy = report(&image, "cleanup.copy.value");
    provenance(&copy, &revision, &image);
    assert!(
        rows(&copy).is_empty(),
        "Copy field acquired cleanup authority: {copy}"
    );
    let marker = report(&image, "cleanup.packet.marker");
    provenance(&marker, &revision, &image);
    // A no-drop sibling can belong to a structural slot without owning a
    // byte finalizer or a live loan; whole-slot metadata is not an obligation.
    assert!(
        !rows(&marker).iter().any(|row| matches!(
            row["facet"].as_str(),
            Some("inventory_flag" | "cleanup_finalize" | "loan" | "loan_endpoint" | "loan_edge")
        )),
        "Copy marker inherited its siblings' obligations: {marker}"
    );
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn type_and_variant_case_queries_retain_case_qualified_owned_leaves_without_inventing_unit_drops() {
    let fixture = Fixture::variant();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let image = image(&revision);
    let owner = report(&image, "cleanup.choice");
    provenance(&owner, &revision, &image);
    for id in [
        "cleanup.choice",
        "cleanup.choice.none",
        "cleanup.choice.data",
        "cleanup.choice.data.payload",
        "cleanup.choice.data.marker",
        "cleanup.choice.error",
        "cleanup.choice.error.code",
    ] {
        assert!(owner["selected_declaration_ids"]
            .as_array()
            .unwrap()
            .contains(&json!(id)));
    }
    let case = report(&image, "cleanup.choice.data");
    provenance(&case, &revision, &image);
    assert!(rows(&case)
        .iter()
        .any(|row| row["function_id"] == "cleanup.consume-choice"
            && row["facet"] == "inventory_flag"));
    let payload = report(&image, "cleanup.choice.data.payload");
    assert!(!rows(&payload).is_empty());
    assert!(rows(&payload)
        .iter()
        .filter(|row| row["facet"] == "inventory_flag")
        .any(|row| row["fact"]["place"]["projections"]
            == json!(["cleanup.choice.data", "cleanup.choice.data.payload"])));
    for target in ["cleanup.choice.none", "cleanup.choice.error.code"] {
        let no_drop = report(&image, target);
        assert!(
            !rows(&no_drop)
                .iter()
                .any(|row| row["facet"] == "cleanup_finalize"),
            "non-owning target acquired a finalizer: {no_drop}"
        );
    }
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn queries_are_deterministic_and_leave_image_and_existing_dependency_bytes_unchanged() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let image = Arc::new(image(&revision));
    let old_image = image.to_json().to_owned();
    let digest = image.image_digest().to_owned();
    let old_dependencies = image
        .declaration_dependencies(image.image_digest(), "cleanup.packet.left")
        .unwrap();
    let handles = (0..4)
        .map(|_| {
            let image = Arc::clone(&image);
            std::thread::spawn(move || {
                image
                    .cleanup_dependencies(image.image_digest(), "cleanup.packet.left")
                    .unwrap()
            })
        })
        .collect::<Vec<_>>();
    let results = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect::<Vec<_>>();
    assert!(results.iter().all(|result| result == &results[0]));
    assert_eq!(image.to_json(), old_image);
    assert_eq!(image.image_digest(), digest);
    assert_eq!(
        image
            .declaration_dependencies(image.image_digest(), "cleanup.packet.left")
            .unwrap(),
        old_dependencies
    );
    let independently_derived =
        ProjectSemanticImage::derive(Arc::clone(&revision), revision.project_revision()).unwrap();
    assert_eq!(
        independently_derived
            .cleanup_dependencies(independently_derived.image_digest(), "cleanup.packet.left")
            .unwrap(),
        results[0]
    );
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn malformed_unknown_and_stale_selection_fails_without_poisoning_subsequent_queries() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let image = image(&revision);
    let original = image.to_json().to_owned();
    let expected = image
        .cleanup_dependencies(image.image_digest(), "cleanup.packet.left")
        .unwrap();
    error(
        image.cleanup_dependencies(&format!("sha256:{}", "0".repeat(64)), "cleanup.packet.left"),
        "SPX-G221",
    );
    for target in ["", "missing.field", "cleanup.packet\0left"] {
        assert!(image
            .cleanup_dependencies(image.image_digest(), target)
            .is_err());
    }
    assert!(image
        .cleanup_dependencies(image.image_digest(), &"x".repeat(4097))
        .is_err());
    assert_eq!(
        image
            .cleanup_dependencies(image.image_digest(), "cleanup.packet.left")
            .unwrap(),
        expected
    );
    assert_eq!(image.to_json(), original);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn every_plan_coordinate_selects_the_exact_source_graph_fact_without_vector_reordering() {
    for fixture in [Fixture::new(), Fixture::variant()] {
        let disk = fixture.bytes();
        let revision = fixture.revision();
        let image = image(&revision);
        let source = revision
            .sources()
            .iter()
            .find(|source| source.path() == "src/core.spx")
            .unwrap();
        let parsed = semaprax::parse(source.source(), source.path()).unwrap();
        // Rebuild independently from canonical source; this is authored test
        // code, never an execution step of the read-only query itself.
        let graph: Value =
            serde_json::from_str(&semaprax::graph::to_json(&parsed).unwrap()).unwrap();
        let target = if source.source().contains("variant Choice") {
            "cleanup.choice"
        } else {
            "cleanup.packet"
        };
        let value = report(&image, target);
        let mut sequences = std::collections::BTreeMap::<(String, String), Vec<Vec<usize>>>::new();
        let mut compared = 0usize;
        for row in rows(&value)
            .iter()
            .filter(|row| row["path"] == "src/core.spx")
        {
            assert!(row["instance_id"].is_null());
            let function = graph["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|node| node["id"] == row["function_id"])
                .unwrap();
            let cleanup = &function["cleanup"];
            let loans = &function["loans"];
            let coordinate = &row["coordinate"];
            let index = |key: &str| coordinate[key].as_u64().unwrap() as usize;
            let facet = row["facet"].as_str().unwrap();
            let (actual, ordinals) = match facet {
                "cleanup_slot" => (&cleanup["slots"][index("slot")], vec![index("slot")]),
                "cleanup_entry" => (&cleanup["entry_state"], vec![]),
                "cleanup_transition" => (
                    &cleanup["blocks"][index("block")]["transitions"][index("transition")],
                    vec![index("block"), index("transition")],
                ),
                "cleanup_edge" => (&cleanup["edges"][index("edge")], vec![index("edge")]),
                "cleanup_region" => (&cleanup["regions"][index("region")], vec![index("region")]),
                "cleanup_finalize" => (
                    &cleanup["exits"][index("exit")]["finalize_in_order"][index("action")],
                    vec![index("exit"), index("action")],
                ),
                "cleanup_exit" => (&cleanup["exits"][index("exit")], vec![index("exit")]),
                "loan" => (&loans["loans"][index("loan")], vec![index("loan")]),
                "loan_edge" => (&loans["edges"][index("edge")], vec![index("edge")]),
                "loan_endpoint" => (
                    &loans["endpoints"][index("endpoint")],
                    vec![index("endpoint")],
                ),
                "inventory_slot" | "inventory_flag" | "inventory_entry" => continue,
                unexpected => panic!("uncovered fact kind {unexpected}"),
            };
            assert!(!actual.is_null(), "missing source graph fact for {row}");
            assert_eq!(
                &row["fact"], actual,
                "coordinate disagrees with source-derived plan: {row}"
            );
            sequences
                .entry((
                    row["function_id"].as_str().unwrap().to_owned(),
                    facet.to_owned(),
                ))
                .or_default()
                .push(ordinals);
            compared += 1;
        }
        assert!(compared > 0);
        for sequence in sequences.values() {
            assert!(
                sequence.windows(2).all(|pair| pair[0] < pair[1]),
                "plan vector coordinates reordered or duplicated: {sequence:?}"
            );
        }
        assert_eq!(fixture.bytes(), disk);
    }
}

#[test]
fn exact_image_recomputation_rejects_reordered_and_noncanonical_report_bytes() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let image = image(&revision);
    let target = "cleanup.packet.left";
    let bytes = image
        .cleanup_dependencies(image.image_digest(), target)
        .unwrap();
    let receipt: Value = serde_json::from_str(
        &image
            .verify_cleanup_dependencies(image.image_digest(), target, bytes.as_bytes())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(receipt["result"], "exact_retained_hir_recomputation");
    assert_eq!(receipt["source_authority"], false);
    let mut reordered: Value = serde_json::from_str(&bytes).unwrap();
    assert!(rows(&reordered).len() > 1);
    reordered["obligations"].as_array_mut().unwrap().swap(0, 1);
    reordered.sort_all_objects();
    error(
        image.verify_cleanup_dependencies(
            image.image_digest(),
            target,
            format!("{reordered}\n").as_bytes(),
        ),
        "SPX-G336",
    );
    error(
        image.verify_cleanup_dependencies(
            image.image_digest(),
            target,
            format!("{bytes} ").as_bytes(),
        ),
        "SPX-G336",
    );
    error(
        image.cleanup_dependencies(image.image_digest(), "missing.field"),
        "SPX-G334",
    );
    assert_eq!(
        image
            .cleanup_dependencies(image.image_digest(), target)
            .unwrap(),
        bytes
    );
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn candidate_cleanup_report_compares_real_body_changes_and_replays_exact_history() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let base = ProjectCandidate::open(Arc::clone(&revision), revision.project_revision()).unwrap();
    let before = base
        .cleanup_dependencies(base.candidate_digest(), "cleanup.packet.left")
        .unwrap();
    let change = SemanticChange::new(base.revision().project_revision(), &json!({"kind":"replace_function_body","target":"cleanup.projected","body":{"kind":"i64","value":0}})).unwrap();
    let changed = base.apply(base.candidate_digest(), &change).unwrap();
    let bytes = changed
        .cleanup_dependencies(changed.candidate_digest(), "cleanup.packet.left")
        .unwrap();
    let value: Value = serde_json::from_str(&bytes).unwrap();
    assert_eq!(
        value["schema"],
        "semaprax.project-candidate-cleanup-dependencies.v1"
    );
    assert_eq!(value["presence"], "both");
    assert_eq!(value["comparison"]["obligations_exact_equal"], false);
    assert_eq!(value["source_authority"], false);
    assert_eq!(value["execution"], false);
    assert!(rows(&value["base"]["report"])
        .iter()
        .any(|row| row["function_id"] == "cleanup.projected" && row["facet"] == "loan"));
    assert!(!rows(&value["candidate"]["report"])
        .iter()
        .any(|row| row["function_id"] == "cleanup.projected"));
    for (side, revision) in [("base", base.revision()), ("candidate", changed.revision())] {
        let image = image(revision);
        let independently_derived = report(&image, "cleanup.packet.left");
        assert_eq!(value[side]["report"], independently_derived);
        assert_eq!(value[side]["image_digest"], image.image_digest());
    }
    let receipt: Value = serde_json::from_str(
        &changed
            .verify_cleanup_dependencies(
                changed.candidate_digest(),
                "cleanup.packet.left",
                bytes.as_bytes(),
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(receipt["result"], "exact_source_history_recomputation");
    assert_eq!(receipt["source_authority"], false);
    let mut tampered = value.clone();
    tampered["base"]["report"]["obligations"]
        .as_array_mut()
        .unwrap()
        .swap(0, 1);
    tampered.sort_all_objects();
    error(
        changed.verify_cleanup_dependencies(
            changed.candidate_digest(),
            "cleanup.packet.left",
            format!("{tampered}\n").as_bytes(),
        ),
        "SPX-G339",
    );
    error(
        changed.verify_cleanup_dependencies(
            changed.candidate_digest(),
            "cleanup.packet.left",
            format!("{bytes} ").as_bytes(),
        ),
        "SPX-G339",
    );
    error(
        changed.cleanup_dependencies(base.candidate_digest(), "cleanup.packet.left"),
        "SPX-G224",
    );
    assert_eq!(
        base.cleanup_dependencies(base.candidate_digest(), "cleanup.packet.left")
            .unwrap(),
        before
    );
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn newly_added_copy_field_has_absent_base_and_present_empty_obligations() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let base = ProjectCandidate::open(Arc::clone(&revision), revision.project_revision()).unwrap();
    let change = SemanticChange::new(base.revision().project_revision(), &json!({"kind":"add_declaration","target":"cleanup.public","declaration":{"kind":"record","id":"cleanup.added","name":"Added","fields":[{"id":"cleanup.added.value","name":"value","type":"i64"}]}})).unwrap();
    let added = base.apply(base.candidate_digest(), &change).unwrap();
    let bytes = added
        .cleanup_dependencies(added.candidate_digest(), "cleanup.added.value")
        .unwrap();
    let value: Value = serde_json::from_str(&bytes).unwrap();
    assert_eq!(value["presence"], "added");
    assert!(value["base"].is_null());
    assert!(value["candidate"].is_object());
    assert_eq!(value["candidate"]["report"]["obligations"], json!([]));
    assert!(value["comparison"]["obligations_exact_equal"].is_null());
    added
        .verify_cleanup_dependencies(
            added.candidate_digest(),
            "cleanup.added.value",
            bytes.as_bytes(),
        )
        .unwrap();
    error(
        base.cleanup_dependencies(base.candidate_digest(), "cleanup.added.value"),
        "SPX-G337",
    );
    assert_eq!(fixture.bytes(), disk);
}
