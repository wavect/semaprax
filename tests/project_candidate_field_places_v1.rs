//! Direct field-place evidence, authored without running compiler/test gates.
use semaprax::diagnostic::Diagnostic;
use semaprax::hir::{DeclarationId, PlaceProjection};
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectCandidateDraft, SemanticChange,
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
            "spx-field-places-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "field-places"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "fields.app"
sources = ["src/app.spx", "src/core.spx", "src/bridge.spx", "src/tests.spx"]
web_exports = ["fields.public"]
tests = ["fields.tests"]
"#,
        )
        .unwrap();
        let fixture = Self(root.canonicalize().unwrap());
        for (path, source) in [
            (
                "src/core.spx",
                r#"module fields.core;
@id("fields.packet") record Packet { @id("fields.packet.left") left:Bytes, @id("fields.packet.right") right:Bytes, @id("fields.packet.marker") marker:i64, }
@id("fields.other") record Other { @id("fields.other.left") left:Bytes, }
@id("fields.metric") record Metric { @id("fields.metric.value") value:i64, }
@id("fields.other-metric") record OtherMetric { @id("fields.other-metric.value") value:i64, }
@id("fields.pair") record Pair { @id("fields.pair.left") left:i64, @id("fields.pair.right") right:i64, }
@id("fields.probe") record Probe { @id("fields.probe.value") value:i64, }
@id("fields.other-probe") record OtherProbe { @id("fields.other-probe.value") value:i64, }
@id("fields.box") record Box<T> { @id("fields.box.value") value:T, }
@id("fields.consume") fn consume(bytes:own Bytes)->i64 {7}
@id("fields.take") fn take(packet:own Packet)->i64 {7}
@id("fields.make") fn make(input:borrow Slice<u8>)->Packet {Packet {left:bytes_copy(input),right:bytes_copy(input),marker:35}}
@id("fields.inspect") fn inspect(packet:own Packet)->usize {let view=bytes_as_slice(packet.left); let sibling=consume(packet.right); byte_len(view)}
@id("fields.pending-inspect") fn pending_inspect(packet:own Packet)->usize {0usize}
@id("fields.borrowed") fn borrowed(packet:borrow Packet)->usize {0usize}
@id("fields.wrong") fn wrong(packet:own Other)->usize {0usize}
@id("fields.read") fn read(metric:Metric)->i64 {metric.value}
@id("fields.read-caller") fn read_caller()->i64 {read(Metric {value:42})}
@id("fields.pair-read") fn pair_read(pair:Pair)->i64 {pair.left - pair.right}
@id("fields.owner-read") fn owner_read(metric:Metric,other:OtherMetric)->i64 {metric.value}
@id("fields.constrained") fn constrained(pair:Pair)->i64 requires pair.left >= 0 requires pair.right >= 0 ensures result == pair.left {pair.left}
@id("fields.probe-read") fn probe_read(p:Probe)->bool {p.value == p.value}
@id("fields.make-probe") fn make_probe()->Probe {Probe {value:0}}
@id("fields.inferred-probe") fn inferred_probe()->bool {let p=make_probe(); p.value == p.value}
@id("fields.wrong-metric") fn wrong_metric(metric:OtherMetric)->i64 {metric.value}
@id("fields.make-other-metric") fn make_other_metric(value:i64)->OtherMetric {OtherMetric {value:value}}
@id("fields.read-box") fn read_box(boxed:Box<i64>)->i64 {boxed.value}
@id("fields.immutable") fn immutable(input:borrow Slice<u8>)->usize {let packet=make(input); byte_len(bytes_as_slice(packet.left))}
@id("fields.mutable") fn mutable(input:borrow Slice<u8>)->usize {let mut packet=make(input); byte_len(bytes_as_slice(packet.left))}
@id("fields.public") fn public_value(value:i64)->i64 {value}
@id("fields.evaluate") fn evaluate()->i64 {let input=[7u8,8u8]; if inspect(make(array_as_slice(input)))==2usize && immutable(array_as_slice(input))==2usize && mutable(array_as_slice(input))==2usize && read(Metric {value:0})==0 && read_box(Box<i64> {value:0})==0 {42}else{0}}
"#,
            ),
            (
                "src/bridge.spx",
                r#"module fields.bridge;
use type @id("fields.packet") from fields.core as Frame;
use function @id("fields.make") from fields.core as make;
use function @id("fields.consume") from fields.core as consume;
@id("fields.bridge-inspect") fn inspect(packet:own Frame)->usize {let view=bytes_as_slice(packet.left); let sibling=consume(packet.right); byte_len(view)}
@id("fields.bridge-evaluate") fn evaluate(input:borrow Slice<u8>)->usize {inspect(make(input))}
"#,
            ),
            (
                "src/app.spx",
                r#"module fields.app;
use function @id("fields.evaluate") from fields.core as evaluate;
use function @id("fields.bridge-evaluate") from fields.bridge as other;
@id("fields.main") fn main()->i64 {let input=[7u8,8u8]; if other(array_as_slice(input))==2usize {evaluate()}else{0}}
"#,
            ),
            (
                "src/tests.spx",
                r#"module fields.tests;
use function @id("fields.evaluate") from fields.core as evaluate;
@id("fields.test") fn main()->i64 {if evaluate()==42 {0}else{1}}
"#,
            ),
        ] {
            fixture.write(path, source);
        }
        fixture
    }
    fn write(&self, path: &str, source: &str) {
        let program = semaprax::parse(source, path).unwrap();
        std::fs::write(self.0.join(path), semaprax::format::canonical(&program)).unwrap();
    }
    fn candidate(&self) -> ProjectCandidate {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
        })
        .unwrap()
    }
    fn bytes(&self) -> Vec<Vec<u8>> {
        [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/bridge.spx",
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
fn source<'a>(candidate: &'a ProjectCandidate, path: &str) -> &'a str {
    candidate
        .revision()
        .sources()
        .iter()
        .find(|source| source.path() == path)
        .unwrap()
        .source()
}
fn place(name: &str) -> Value {
    json!({"kind":"place","name":name})
}
fn field(target: &str, root: &str) -> Value {
    json!({"kind":"field_place","target":target,"root":root})
}
fn builtin(target: &str, arguments: Vec<Value>) -> Value {
    json!({"kind":"builtin_call","target":target,"arguments":arguments})
}
fn binding(name: &str, value: Value, body: Value) -> Value {
    json!({"kind":"let","name":name,"value":value,"body":body})
}
fn call(target: &str, arguments: Vec<Value>) -> Value {
    json!({"kind":"call","target":target,"arguments":arguments})
}
fn apply(base: &ProjectCandidate, intent: Value) -> Result<ProjectCandidate, Vec<Diagnostic>> {
    base.apply(
        base.candidate_digest(),
        &SemanticChange::new(base.revision().project_revision(), &intent)?,
    )
}
fn body(
    base: &ProjectCandidate,
    target: &str,
    value: Value,
) -> Result<ProjectCandidate, Vec<Diagnostic>> {
    apply(
        base,
        json!({"kind":"replace_function_body","target":target,"body":value}),
    )
}
fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    let errors = result.err().expect("expected rejection");
    assert!(
        errors.iter().any(|error| error.code == expected),
        "{errors:?}"
    );
}
fn replay(candidate: &ProjectCandidate) {
    let restored = ProjectCandidate::restore(
        Arc::clone(candidate.base_revision()),
        candidate.base_revision().project_revision(),
        candidate.recovery_capsule().unwrap().as_bytes(),
    )
    .unwrap();
    assert_eq!(restored.to_json(), candidate.to_json());
    assert_eq!(
        restored.revision().semantic_graph(),
        candidate.revision().semantic_graph()
    );
}
fn selected(candidate: &ProjectCandidate, target: &str, snippet: &str) -> (String, Value) {
    let catalog: Value =
        serde_json::from_str(&candidate.expression_catalog(target).unwrap()).unwrap();
    let text = source(candidate, catalog["source"]["path"].as_str().unwrap());
    let rows = catalog["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| {
            let span = &row["source_span"];
            row["replaceable"] == true
                && text.get(
                    span["start"].as_u64().unwrap() as usize
                        ..span["end"].as_u64().unwrap() as usize,
                ) == Some(snippet)
        })
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 1);
    (
        rows[0]["expression_id"].as_str().unwrap().to_owned(),
        rows[0].clone(),
    )
}

fn selected_contract(candidate: &ProjectCandidate, target: &str, snippet: &str) -> String {
    let catalog: Value =
        serde_json::from_str(&candidate.contract_expression_catalog(target).unwrap()).unwrap();
    let text = source(candidate, catalog["source"]["path"].as_str().unwrap());
    let selected = catalog["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| {
            let span = &row["source_span"];
            row["phase"] == "ensures"
                && row["replaceable"] == true
                && text.get(
                    span["start"].as_u64().unwrap() as usize
                        ..span["end"].as_u64().unwrap() as usize,
                ) == Some(snippet)
        })
        .collect::<Vec<_>>();
    assert_eq!(selected.len(), 1);
    selected[0]["expression_id"].as_str().unwrap().to_owned()
}

fn projected_body() -> Value {
    binding(
        "view",
        builtin(
            "core.bytes.as-slice",
            vec![field("fields.packet.left", "packet")],
        ),
        binding(
            "alias",
            place("view"),
            binding(
                "sibling",
                call(
                    "fields.consume",
                    vec![field("fields.packet.right", "packet")],
                ),
                builtin("core.bytes.len", vec![place("alias")]),
            ),
        ),
    )
}

#[test]
fn direct_field_borrow_keeps_the_owned_parameter_and_exact_field_provenance_across_import_aliases()
{
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    for (target, path) in [
        ("fields.inspect", "src/core.spx"),
        ("fields.bridge-inspect", "src/bridge.spx"),
    ] {
        let candidate = body(&base, target, projected_body()).unwrap();
        let function = candidate
            .revision()
            .entry_program()
            .functions
            .iter()
            .find(|function| function.id.as_str() == target)
            .unwrap();
        let projected = function
            .loan_plan
            .loans
            .iter()
            .find(|loan| !loan.origin.projections.is_empty())
            .unwrap();
        assert_eq!(projected.origin.root, function.params[0].id);
        assert_eq!(
            projected.origin.projections,
            [PlaceProjection::Field(DeclarationId::new(
                "fields.packet.left"
            ))]
        );
        let graph: Value = serde_json::from_str(candidate.revision().semantic_graph()).unwrap();
        assert!(graph["declarations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["id"] == "fields.packet.left" && row["owner"] == "fields.packet"));
        let parsed = semaprax::parse(source(&candidate, path), path).unwrap();
        let authored = parsed
            .functions
            .iter()
            .find(|function| function.stable_id == target)
            .unwrap();
        let semaprax::ast::ExprKind::Block { statements, .. } = &authored.body.kind else {
            panic!("body block expected")
        };
        let semaprax::ast::Statement::Let { value, .. } = &statements[0] else {
            panic!("view binding expected")
        };
        let semaprax::ast::ExprKind::Call { name, args, .. } = &value.kind else {
            panic!("direct builtin call expected")
        };
        assert_eq!(name, "bytes_as_slice");
        let semaprax::ast::ExprKind::Project {
            base: root, field, ..
        } = &args[0].kind
        else {
            panic!("unstaged field place expected")
        };
        assert_eq!(field, "left");
        assert_eq!(root.kind, semaprax::ast::ExprKind::Var("packet".to_owned()));
        assert!(!source(&candidate, path).contains("spx_project_"));
        assert!(source(&candidate, path).contains("consume(packet.right)"));
        replay(&candidate);
        let empty = ProjectCandidateDraft::open(Arc::new(fixture.candidate())).unwrap();
        let draft = empty
            .with_body_hole(empty.draft_digest(), target, "projected")
            .unwrap();
        code(draft.complete(draft.draft_digest()), "SPX-G232");
        let filled = draft
            .fill_hole(draft.draft_digest(), "projected", &projected_body())
            .unwrap();
        let completed = filled.complete(filled.draft_digest()).unwrap();
        assert_eq!(completed.to_json(), candidate.to_json());
    }
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn immutable_and_mutable_checked_local_roots_both_allow_shared_field_borrows_in_expression_holes() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = Arc::new(fixture.candidate());
    for (target, mutable) in [("fields.immutable", false), ("fields.mutable", true)] {
        let (expression, row) = selected(&base, target, "byte_len(bytes_as_slice(packet.left))");
        let root = row["scope"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["name"] == "packet")
            .unwrap();
        assert_eq!(root["mutable"], mutable);
        assert_eq!(root["ownership"], "own");
        let empty = ProjectCandidateDraft::open(Arc::clone(&base)).unwrap();
        let draft = empty
            .with_expression_hole(empty.draft_digest(), target, &expression, "view")
            .unwrap();
        let before = draft.to_json().to_owned();
        let replacement = builtin(
            "core.bytes.len",
            vec![builtin(
                "core.bytes.as-slice",
                vec![field("fields.packet.right", "packet")],
            )],
        );
        let done = draft
            .fill_hole(draft.draft_digest(), "view", &replacement)
            .unwrap();
        let candidate = done.complete(done.draft_digest()).unwrap();
        assert!(
            source(&candidate, "src/core.spx").contains("byte_len(bytes_as_slice(packet.right))")
        );
        assert_eq!(draft.to_json(), before);
        replay(&candidate);
    }
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn constructor_let_roots_and_generic_scalar_instances_use_their_actual_nominal_owners() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let record = json!({"kind":"record","target":"fields.metric","fields":[{"target":"fields.metric.value","value":{"kind":"i64","value":7}}]});
    let constructed = body(
        &base,
        "fields.read",
        binding("local", record, field("fields.metric.value", "local")),
    )
    .unwrap();
    assert!(source(&constructed, "src/core.spx").contains("local.value"));
    replay(&constructed);
    let called = body(
        &base,
        "fields.immutable",
        binding(
            "packet",
            call("fields.make", vec![place("input")]),
            builtin(
                "core.bytes.len",
                vec![builtin(
                    "core.bytes.as-slice",
                    vec![field("fields.packet.left", "packet")],
                )],
            ),
        ),
    )
    .unwrap();
    assert!(source(&called, "src/core.spx").contains("bytes_as_slice(packet.left)"));
    replay(&called);
    let generic = body(&base, "fields.read-box", field("fields.box.value", "boxed")).unwrap();
    assert!(source(&generic, "src/core.spx").contains("boxed.value"));
    replay(&generic);
    let other = json!({"kind":"record","target":"fields.other-metric","fields":[{"target":"fields.other-metric.value","value":{"kind":"i64","value":7}}]});
    code(
        body(
            &base,
            "fields.read",
            binding(
                "local",
                other.clone(),
                field("fields.metric.value", "local"),
            ),
        ),
        "SPX-G225",
    );
    code(
        body(
            &base,
            "fields.read",
            binding(
                "local",
                call(
                    "fields.make-other-metric",
                    vec![json!({"kind":"i64","value":7})],
                ),
                field("fields.metric.value", "local"),
            ),
        ),
        "SPX-G225",
    );
    code(
        body(
            &base,
            "fields.wrong-metric",
            binding(
                "local",
                place("metric"),
                field("fields.metric.value", "local"),
            ),
        ),
        "SPX-G225",
    );
    let conflicting = json!({"kind":"if","condition":{"kind":"bool","value":true},
        "then":{"kind":"record","target":"fields.metric","fields":[{"target":"fields.metric.value","value":{"kind":"i64","value":7}}]},"else":other});
    code(
        body(
            &base,
            "fields.read",
            binding("local", conflicting, field("fields.metric.value", "local")),
        ),
        "SPX-G225",
    );
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn wrong_field_owners_invalid_roots_and_live_overlaps_leave_candidate_and_source_unchanged() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let before = base.to_json().to_owned();
    for target in ["missing.field", "fields.packet", "fields.other.left"] {
        code(
            body(
                &base,
                "fields.inspect",
                builtin(
                    "core.bytes.len",
                    vec![builtin(
                        "core.bytes.as-slice",
                        vec![field(target, "packet")],
                    )],
                ),
            ),
            "SPX-G225",
        );
    }
    for root in ["missing", "packet.left", "make(input)"] {
        code(
            body(
                &base,
                "fields.inspect",
                builtin(
                    "core.bytes.len",
                    vec![builtin(
                        "core.bytes.as-slice",
                        vec![field("fields.packet.left", root)],
                    )],
                ),
            ),
            "SPX-G225",
        );
    }
    code(
        body(
            &base,
            "fields.wrong-metric",
            field("fields.metric.value", "metric"),
        ),
        "SPX-G225",
    );
    code(
        body(
            &base,
            "fields.wrong",
            builtin(
                "core.bytes.len",
                vec![builtin(
                    "core.bytes.as-slice",
                    vec![field("fields.packet.left", "packet")],
                )],
            ),
        ),
        "SPX-G225",
    );
    code(
        body(
            &base,
            "fields.borrowed",
            builtin(
                "core.bytes.len",
                vec![builtin(
                    "core.bytes.as-slice",
                    vec![field("fields.packet.left", "packet")],
                )],
            ),
        ),
        "SPX-T266",
    );
    for moved in [
        call(
            "fields.consume",
            vec![field("fields.packet.left", "packet")],
        ),
        call("fields.take", vec![place("packet")]),
    ] {
        let invalid = binding(
            "view",
            builtin(
                "core.bytes.as-slice",
                vec![field("fields.packet.left", "packet")],
            ),
            binding(
                "moved",
                moved,
                builtin("core.bytes.len", vec![place("view")]),
            ),
        );
        code(body(&base, "fields.inspect", invalid), "SPX-T265");
    }
    let mut temporary = field("fields.packet.left", "packet");
    temporary["root"] = call("fields.make", vec![place("input")]);
    code(
        body(
            &base,
            "fields.immutable",
            builtin(
                "core.bytes.len",
                vec![builtin("core.bytes.as-slice", vec![temporary])],
            ),
        ),
        "SPX-G225",
    );
    assert_eq!(base.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn direct_field_rebase_tracks_display_renames_but_rejects_a_reidentified_member() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let candidate = body(&base, "fields.inspect", projected_body()).unwrap();
    let pending = body(&base, "fields.pending-inspect", projected_body()).unwrap();
    let renamed = apply(
        &base,
        json!({"kind":"rename_declaration","target":"fields.packet.left","name":"payload"}),
    )
    .unwrap();
    // Authenticated occurrences normalize the field used in the original body,
    // while replay still emits its new source spelling.
    let existing = candidate
        .rebase(
            candidate.candidate_digest(),
            Arc::clone(renamed.revision()),
            renamed.revision().project_revision(),
        )
        .unwrap();
    assert!(source(existing.candidate(), "src/core.spx").contains("bytes_as_slice(packet.payload)"));
    let classifications: Value = serde_json::from_str(existing.to_json()).unwrap();
    assert_eq!(
        classifications["classifications"][0]["concurrent_body_change"],
        false
    );
    replay(existing.candidate());
    assert!(!source(existing.candidate(), "src/core.spx").contains("spx_rebase_ref_"));
    let merged = candidate
        .merge(
            candidate.candidate_digest(),
            &renamed,
            renamed.candidate_digest(),
        )
        .unwrap();
    assert_eq!(
        source(merged.candidate(), "src/core.spx"),
        source(existing.candidate(), "src/core.spx")
    );
    replay(merged.candidate());
    // The pending function has no original field use: only the new constructor
    // dependency must survive the field's display-name change.
    let rebased = pending
        .rebase(
            pending.candidate_digest(),
            Arc::clone(renamed.revision()),
            renamed.revision().project_revision(),
        )
        .unwrap();
    assert!(source(rebased.candidate(), "src/core.spx").contains("bytes_as_slice(packet.payload)"));
    let program =
        semaprax::parse(source(rebased.candidate(), "src/core.spx"), "src/core.spx").unwrap();
    let pending_function = program
        .functions
        .iter()
        .find(|function| function.stable_id == "fields.pending-inspect")
        .unwrap();
    let semaprax::ast::ExprKind::Block { statements, .. } = &pending_function.body.kind else {
        panic!("missing rebased pending body")
    };
    let semaprax::ast::Statement::Let { value, .. } = &statements[0] else {
        panic!("missing rebased borrow")
    };
    let semaprax::ast::ExprKind::Call { args, .. } = &value.kind else {
        panic!("missing rebased byte borrow")
    };
    assert!(
        matches!(&args[0].kind, semaprax::ast::ExprKind::Project { field, .. } if field == "payload")
    );
    replay(rebased.candidate());
    assert_eq!(fixture.bytes(), disk);
    let before = candidate.to_json().to_owned();
    let pending_before = pending.to_json().to_owned();
    let original = source(&base, "src/core.spx");
    let changed = original.replace(
        "@id(\"fields.packet.left\")",
        "@id(\"fields.packet.reidentified\")",
    );
    assert_ne!(changed, original);
    fixture.write("src/core.spx", &changed);
    let incoming = fixture.candidate();
    code(
        candidate.rebase(
            candidate.candidate_digest(),
            Arc::clone(incoming.revision()),
            incoming.revision().project_revision(),
        ),
        "SPX-G235",
    );
    assert_eq!(candidate.to_json(), before);
    code(
        pending.rebase(
            pending.candidate_digest(),
            Arc::clone(incoming.revision()),
            incoming.revision().project_revision(),
        ),
        "SPX-G235",
    );
    assert_eq!(pending.to_json(), pending_before);
    assert_eq!(
        source(&incoming, "src/core.spx"),
        std::fs::read_to_string(fixture.0.join("src/core.spx")).unwrap()
    );
}

#[test]
fn new_declaration_parameters_and_nominal_result_contracts_receive_exact_field_owner_facts() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let nominal = json!({"kind":"nominal","target":"fields.metric","type_arguments":[]});
    let read = json!({"kind":"add_declaration","target":"fields.read","declaration":{
        "id":"fields.added-read","name":"added_read","parameters":[{"name":"input","type":nominal,"mode":"value"}],
        "return_type":"i64","effects":[],"requires":[],"ensures":[],"body":field("fields.metric.value","input")
    }});
    let added = apply(&base, read).unwrap();
    assert!(source(&added, "src/core.spx").contains("fn added_read(input: Metric) -> i64"));
    let forward = json!({"kind":"add_declaration","target":"fields.read","declaration":{
        "id":"fields.added-forward","name":"added_forward","parameters":[{"name":"input","type":nominal,"mode":"value"}],
        "return_type":nominal,"effects":[],"requires":[],
        "ensures":[{"kind":"binary","op":"==","left":field("fields.metric.value","result"),"right":field("fields.metric.value","input")}],
        "body":place("input")
    }});
    let completed = apply(&added, forward.clone()).unwrap();
    assert!(
        source(&completed, "src/core.spx").contains("fn added_forward(input: Metric) -> Metric")
    );
    assert!(source(&completed, "src/core.spx").contains("ensures result.value == input.value"));
    replay(&completed);
    let mut wrong = forward;
    wrong["declaration"]["return_type"] =
        json!({"kind":"nominal","target":"fields.other-metric","type_arguments":[]});
    code(apply(&added, wrong), "SPX-G225");
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn computed_signature_field_defaults_use_original_nominal_parameters_and_staged_arguments() {
    use semaprax::ast::{ExprKind, Statement, Type};

    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let changed = apply(
        &base,
        json!({"kind":"change_function_signature","target":"fields.read","parameters":[
            {"from":"metric"},
            {"name":"computed","type":"i64","argument_expression":field("fields.metric.value","metric")}
        ]}),
    )
    .unwrap();
    let program = semaprax::parse(source(&changed, "src/core.spx"), "src/core.spx").unwrap();
    let provider = program
        .functions
        .iter()
        .find(|f| f.stable_id == "fields.read")
        .unwrap();
    assert_eq!(provider.params.len(), 2);
    assert_eq!(provider.params[0].name, "metric");
    assert_eq!(provider.params[1].name, "computed");
    assert_eq!(provider.params[1].ty, Type::I64);
    let caller = program
        .functions
        .iter()
        .find(|f| f.stable_id == "fields.read-caller")
        .unwrap();
    let mut expression = &caller.body;
    while let ExprKind::Block { statements, tail } = &expression.kind {
        if !statements.is_empty() {
            break;
        }
        expression = tail;
    }
    let ExprKind::Block { statements, tail } = &expression.kind else {
        panic!("missing caller stages")
    };
    assert_eq!(statements.len(), 2);
    let Statement::Let {
        name: original,
        declared,
        value,
        mutable,
        ..
    } = &statements[0]
    else {
        panic!("missing original argument")
    };
    assert!(!mutable);
    assert!(declared.is_none());
    assert!(
        matches!(&value.kind, ExprKind::ConstructRecord { type_name, fields, .. } if type_name == "Metric" && fields.len() == 1)
    );
    let Statement::Let {
        name: computed,
        declared,
        value,
        mutable,
        ..
    } = &statements[1]
    else {
        panic!("missing computed argument")
    };
    assert!(!mutable);
    assert_eq!(declared, &Some(Type::I64));
    let ExprKind::Project { base, field, .. } = &value.kind else {
        panic!("missing direct field projection")
    };
    assert_eq!(base.kind, ExprKind::Var(original.clone()));
    assert_eq!(field, "value");
    let ExprKind::Call { name, args, .. } = &tail.kind else {
        panic!("missing migrated call")
    };
    assert_eq!(name, "read");
    assert_eq!(args.len(), 2);
    assert_eq!(args[0].kind, ExprKind::Var(original.clone()));
    assert_eq!(args[1].kind, ExprKind::Var(computed.clone()));
    replay(&changed);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn authenticated_field_fingerprints_preserve_real_body_edits_owner_ids_and_operand_order() {
    for (target, replacement_body, changed_identity) in [
        ("fields.pair-read", Some("{pair.left + pair.right}"), None),
        ("fields.pair-read", Some("{pair.right - pair.left}"), None),
        ("fields.pair-read", Some("{pair.left - pair.left}"), None),
        ("fields.owner-read", Some("{other.value}"), None),
        ("fields.read", None, Some("fields.metric.value")),
        ("fields.pair-read", None, Some("fields.pair.right")),
    ] {
        let fixture = Fixture::new();
        let base = fixture.candidate();
        // No field constructor dependency: the original-body fingerprint itself
        // must distinguish these checked occurrences and ordered operations.
        let candidate = body(&base, target, json!({"kind":"i64","value":7})).unwrap();
        let before = candidate.to_json().to_owned();
        let mut changed = source(&base, "src/core.spx").to_owned();
        if let Some(replacement) = replacement_body {
            let parsed = semaprax::parse(&changed, "src/core.spx").unwrap();
            let function = parsed
                .functions
                .iter()
                .find(|f| f.stable_id == target)
                .unwrap();
            changed.replace_range(
                function.body.span.start..function.body.span.end,
                replacement,
            );
        }
        if let Some(identity) = changed_identity {
            let old = format!("@id(\"{identity}\")");
            assert_eq!(changed.matches(&old).count(), 1);
            changed = changed.replace(&old, &format!("@id(\"{identity}.changed\")"));
        }
        assert_ne!(changed, source(&base, "src/core.spx"));
        fixture.write("src/core.spx", &changed);
        let incoming = fixture.candidate();
        let incoming_before = incoming.to_json().to_owned();
        let disk = fixture.bytes();
        code(
            candidate.rebase(
                candidate.candidate_digest(),
                Arc::clone(incoming.revision()),
                incoming.revision().project_revision(),
            ),
            "SPX-G235",
        );
        assert_eq!(candidate.to_json(), before);
        assert_eq!(incoming.to_json(), incoming_before);
        assert_eq!(fixture.bytes(), disk);
    }
}

#[test]
fn authenticated_field_contract_fingerprints_preserve_predicates_siblings_and_clause_order() {
    for edit in ["predicate", "sibling", "order", "identity"] {
        let fixture = Fixture::new();
        let base = fixture.candidate();
        let original_source = source(&base, "src/core.spx");
        let expression = selected_contract(&base, "fields.constrained", "result == pair.left");
        let candidate = apply(&base, json!({"kind":"replace_contract_expression","target":"fields.constrained","expression_id":expression,"replacement":{"kind":"bool","value":true}})).unwrap();
        let before = candidate.to_json().to_owned();
        let changed = match edit {
            "predicate" => original_source
                .replace("ensures result == pair.left", "ensures result >= pair.left"),
            "sibling" => original_source.replace(
                "ensures result == pair.left",
                "ensures result == pair.right",
            ),
            "identity" => original_source.replace(
                "@id(\"fields.pair.left\")",
                "@id(\"fields.pair.reidentified\")",
            ),
            "order" => {
                let mut program = semaprax::parse(original_source, "src/core.spx").unwrap();
                let function = program
                    .functions
                    .iter_mut()
                    .find(|f| f.stable_id == "fields.constrained")
                    .unwrap();
                assert_eq!(function.requires.len(), 2);
                function.requires.swap(0, 1);
                semaprax::format::canonical(&program)
            }
            _ => unreachable!(),
        };
        assert_ne!(changed, original_source);
        fixture.write("src/core.spx", &changed);
        let incoming = fixture.candidate();
        let incoming_before = incoming.to_json().to_owned();
        let disk = fixture.bytes();
        code(
            candidate.rebase(
                candidate.candidate_digest(),
                Arc::clone(incoming.revision()),
                incoming.revision().project_revision(),
            ),
            "SPX-G235",
        );
        assert_eq!(candidate.to_json(), before);
        assert_eq!(incoming.to_json(), incoming_before);
        assert_eq!(fixture.bytes(), disk);
    }
}

#[test]
fn authenticated_contract_occurrences_rebase_across_field_and_nominal_display_names() {
    for (target, name) in [
        ("fields.pair.left", "first"),
        ("fields.pair", "RenamedPair"),
    ] {
        let fixture = Fixture::new();
        let disk = fixture.bytes();
        let base = fixture.candidate();
        let expression = selected_contract(&base, "fields.constrained", "result == pair.left");
        let candidate = apply(&base, json!({"kind":"replace_contract_expression","target":"fields.constrained","expression_id":expression,"replacement":{"kind":"bool","value":true}})).unwrap();
        let before = candidate.to_json().to_owned();
        let renamed = apply(
            &base,
            json!({"kind":"rename_declaration","target":target,"name":name}),
        )
        .unwrap();
        let rebased = candidate
            .rebase(
                candidate.candidate_digest(),
                Arc::clone(renamed.revision()),
                renamed.revision().project_revision(),
            )
            .unwrap();
        let report: Value = serde_json::from_str(rebased.to_json()).unwrap();
        assert_eq!(report["classifications"].as_array().unwrap().len(), 1);
        let classification = &report["classifications"][0];
        assert_eq!(classification["concurrent_signature_change"], false);
        assert_eq!(classification["concurrent_body_change"], false);
        assert_eq!(classification["concurrent_contract_change"], false);
        let text = source(rebased.candidate(), "src/core.spx");
        assert!(text.contains("ensures true"));
        if target == "fields.pair.left" {
            assert!(text.contains("requires pair.first >= 0"));
        } else {
            assert!(text.contains("fn constrained(pair: RenamedPair)"));
        }
        assert!(!text.contains("spx_rebase_ref_"));
        replay(rebased.candidate());
        assert_eq!(candidate.to_json(), before);
        assert_eq!(fixture.bytes(), disk);
    }
}

#[test]
fn identical_field_ids_and_body_spelling_cannot_hide_changed_type_or_owner() {
    for edit in ["type", "owner"] {
        let fixture = Fixture::new();
        let base = fixture.candidate();
        let target = if edit == "type" {
            "fields.probe-read"
        } else {
            "fields.inferred-probe"
        };
        let candidate = body(&base, target, json!({"kind":"bool","value":true})).unwrap();
        let before = candidate.to_json().to_owned();
        let original = source(&base, "src/core.spx");
        let parsed = semaprax::parse(original, "src/core.spx").unwrap();
        let original_body = parsed
            .functions
            .iter()
            .find(|f| f.stable_id == target)
            .unwrap()
            .body
            .clone();
        let mut changed_program = parsed.clone();
        if edit == "type" {
            let probe = changed_program
                .types
                .iter_mut()
                .find(|ty| ty.stable_id == "fields.probe")
                .unwrap();
            let semaprax::ast::TypeDeclarationKind::Record { fields } = &mut probe.kind else {
                panic!("missing probe record")
            };
            assert_eq!(fields[0].stable_id, "fields.probe.value");
            fields[0].ty = semaprax::ast::Type::Bool;
            // Keep the unused constructor source-valid; the selected function's
            // signature, body text and selected field identity remain unchanged.
            let make = changed_program
                .functions
                .iter_mut()
                .find(|f| f.stable_id == "fields.make-probe")
                .unwrap();
            let semaprax::ast::ExprKind::Block { tail, .. } = &mut make.body.kind else {
                panic!("missing constructor body")
            };
            let semaprax::ast::ExprKind::ConstructRecord { fields, .. } = &mut tail.kind else {
                panic!("missing probe constructor")
            };
            fields[0].value.kind = semaprax::ast::ExprKind::Bool(false);
        } else {
            for declaration in &mut changed_program.types {
                let replacement = match declaration.stable_id.as_str() {
                    "fields.probe" => "fields.other-probe.value",
                    "fields.other-probe" => "fields.probe.value",
                    _ => continue,
                };
                let semaprax::ast::TypeDeclarationKind::Record { fields } = &mut declaration.kind
                else {
                    panic!("missing probe record")
                };
                fields[0].stable_id = replacement.to_owned();
            }
            let make = changed_program
                .functions
                .iter_mut()
                .find(|f| f.stable_id == "fields.make-probe")
                .unwrap();
            make.return_type = semaprax::ast::Type::Named {
                name: "OtherProbe".into(),
                arguments: vec![],
            };
            let semaprax::ast::ExprKind::Block { tail, .. } = &mut make.body.kind else {
                panic!("missing constructor body")
            };
            let semaprax::ast::ExprKind::ConstructRecord { type_name, .. } = &mut tail.kind else {
                panic!("missing probe constructor")
            };
            *type_name = "OtherProbe".to_owned();
        }
        assert_eq!(
            changed_program
                .functions
                .iter()
                .find(|f| f.stable_id == target)
                .unwrap()
                .body,
            original_body
        );
        fixture.write(
            "src/core.spx",
            &semaprax::format::canonical(&changed_program),
        );
        let incoming = fixture.candidate();
        let disk = fixture.bytes();
        code(
            candidate.rebase(
                candidate.candidate_digest(),
                Arc::clone(incoming.revision()),
                incoming.revision().project_revision(),
            ),
            "SPX-G235",
        );
        assert_eq!(candidate.to_json(), before);
        assert_eq!(fixture.bytes(), disk);
    }
}

#[test]
fn authored_identifiers_cannot_impersonate_private_rebase_markers() {
    let fixture = Fixture::new();
    let base = fixture.candidate();
    let candidate = body(&base, "fields.read", json!({"kind":"i64","value":7})).unwrap();
    let before = candidate.to_json().to_owned();
    let changed = format!("{}\n@id(\"fields.marker\") fn marker(spx_rebase_ref_user:i64)->i64 {{spx_rebase_ref_user}}\n", source(&base, "src/core.spx"));
    fixture.write("src/core.spx", &changed);
    let incoming = fixture.candidate();
    let disk = fixture.bytes();
    code(
        candidate.rebase(
            candidate.candidate_digest(),
            Arc::clone(incoming.revision()),
            incoming.revision().project_revision(),
        ),
        "SPX-G235",
    );
    assert_eq!(candidate.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}
