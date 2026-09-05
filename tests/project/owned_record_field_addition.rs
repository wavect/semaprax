//! Append-only scalar additions over checked Copy and flat owned records.
use semaprax::ast::{ExprKind, MatchMode, MatchPattern, RecordMatchFieldPattern};
use semaprax::cleanup::FieldLivenessShape;
use semaprax::diagnostic::Diagnostic;
use semaprax::hir::{DeclarationId, PlaceProjection};
use semaprax::project::{with_authenticated_project, ProjectCandidate, SemanticChange};
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
            "spx-owned-field-addition-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "owned-field-addition"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "append.app"
sources = ["src/app.spx", "src/bridge.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["append.public"]
tests = ["append.tests"]
"#,
        )
        .unwrap();
        let fixture = Self(root.canonicalize().unwrap());
        for (path, text) in [
            (
                "src/core.spx",
                r#"module append.core;
@id("append.packet") record Packet { @id("append.packet.left") left:Bytes, @id("append.packet.right") right:Bytes, @id("append.packet.marker") marker:i64, }
@id("append.small") record Small { @id("append.small.signed") signed:i32, @id("append.small.byte") byte:u8, @id("append.small.size") size:usize, }
@id("append.small-make") fn small_make()->Small {Small {size:3usize,byte:2u8,signed:1i32}}
@id("append.small-read") fn small_read(value:Small)->i64 {0}
@id("append.left") fn left_bytes(input:borrow Slice<u8>)->Bytes {bytes_copy(input)}
@id("append.right") fn right_bytes(input:borrow Slice<u8>)->Bytes {bytes_copy(input)}
@id("append.make") fn make(input:borrow Slice<u8>)->Packet {Packet {right:right_bytes(input),left:left_bytes(input),marker:7}}
@id("append.consume") fn consume(value:own Bytes)->i64 {1}
@id("append.inspect") fn inspect(packet:own Packet)->usize {let view=bytes_as_slice(packet.left);let sibling=consume(packet.right);byte_len(view)}
@id("append.unpack") fn unpack(packet:own Packet)->i64 {match own packet {Packet {left,right,marker}=>{let a=consume(left);let b=consume(right);marker+a+b},}}
@id("append.alternate-make") fn make_other(input:borrow Slice<u8>)->Packet {Packet {marker:8,left:left_bytes(input),right:right_bytes(input)}}
@id("append.alternate-unpack") fn unpack_other(packet:own Packet)->i64 {match own packet {Packet {right:r,marker:m,left:l}=>{let a=consume(l);let b=consume(r);m+a+b},}}
@id("append.public") fn public_value(value:i64)->i64 {value}
@id("append.evaluate") fn evaluate()->i64 {let input=[7u8,8u8];if inspect(make(array_as_slice(input)))==2usize && unpack(make(array_as_slice(input)))==9 {42}else{0}}
"#,
            ),
            (
                "src/bridge.spx",
                r#"module append.bridge;
use type @id("append.small") from append.core as Gauge;
@id("append.bridge-small") fn make()->Gauge {Gauge {byte:2u8,signed:1i32,size:3usize}}
@id("append.bridge-evaluate") fn evaluate()->i64 {10}
"#,
            ),
            (
                "src/app.spx",
                r#"module append.app;
use function @id("append.evaluate") from append.core as evaluate;
use function @id("append.bridge-evaluate") from append.bridge as bridge;
@id("append.main") fn main()->i64 {if bridge()==10 {evaluate()}else{0}}
"#,
            ),
            (
                "src/tests.spx",
                r#"module append.tests;
use function @id("append.evaluate") from append.core as evaluate;
@id("append.test") fn main()->i64 {if evaluate()==42 {0}else{1}}
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
        .map(|p| std::fs::read(self.0.join(p)).unwrap())
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
        .find(|s| s.path() == path)
        .unwrap()
        .source()
}
fn request(target: &str, ty: &str, value: Value) -> Value {
    json!({"kind":"add_record_field","target":target,"field":{"id":format!("{target}.added"),"name":"added","type":ty,"default":{"kind":ty,"value":value}}})
}
fn apply(base: &ProjectCandidate, intent: &Value) -> Result<ProjectCandidate, Vec<Diagnostic>> {
    base.apply(
        base.candidate_digest(),
        &SemanticChange::new(base.revision().project_revision(), intent)?,
    )
}
fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    let errors = result.err().expect("expected rejection");
    assert!(errors.iter().any(|e| e.code == expected), "{errors:?}");
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
fn tail(mut expression: &semaprax::ast::Expr) -> &semaprax::ast::Expr {
    while let ExprKind::Block { statements, tail } = &expression.kind {
        if !statements.is_empty() {
            break;
        }
        expression = tail;
    }
    expression
}

#[test]
fn owned_constructors_keep_source_evaluation_order_and_match_binding_aliases() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let candidate = apply(&base, &request("append.packet", "i32", json!(-7))).unwrap();
    for (path, target, names) in [
        (
            "src/core.spx",
            "append.make",
            vec!["right", "left", "marker", "added"],
        ),
        (
            "src/core.spx",
            "append.alternate-make",
            vec!["marker", "left", "right", "added"],
        ),
    ] {
        let old = semaprax::parse(source(&base, path), path).unwrap();
        let new = semaprax::parse(source(&candidate, path), path).unwrap();
        let before = old
            .functions
            .iter()
            .find(|f| f.stable_id == target)
            .unwrap();
        let after = new
            .functions
            .iter()
            .find(|f| f.stable_id == target)
            .unwrap();
        let ExprKind::ConstructRecord {
            fields: old_fields, ..
        } = &tail(&before.body).kind
        else {
            panic!("missing original record")
        };
        let ExprKind::ConstructRecord { fields, .. } = &tail(&after.body).kind else {
            panic!("missing migrated record")
        };
        assert_eq!(
            fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(),
            names
        );
        for (old, new) in old_fields.iter().zip(fields) {
            assert_eq!(
                semaprax::format::expr(&old.value, 0),
                semaprax::format::expr(&new.value, 0)
            );
        }
        assert_eq!(
            semaprax::format::expr(&fields.last().unwrap().value, 0),
            "-7i32"
        );
    }
    for (path, target) in [
        ("src/core.spx", "append.unpack"),
        ("src/core.spx", "append.alternate-unpack"),
    ] {
        let old = semaprax::parse(source(&base, path), path).unwrap();
        let new = semaprax::parse(source(&candidate, path), path).unwrap();
        let before = old
            .functions
            .iter()
            .find(|f| f.stable_id == target)
            .unwrap();
        let after = new
            .functions
            .iter()
            .find(|f| f.stable_id == target)
            .unwrap();
        let ExprKind::Match { mode, arms, .. } = &tail(&after.body).kind else {
            panic!("missing owning match")
        };
        assert_eq!(*mode, MatchMode::Own);
        let ExprKind::Match { arms: old_arms, .. } = &tail(&before.body).kind else {
            panic!("missing original match")
        };
        let MatchPattern::Record { fields, .. } = &arms[0].pattern else {
            panic!("missing record pattern")
        };
        let MatchPattern::Record {
            fields: old_fields, ..
        } = &old_arms[0].pattern
        else {
            panic!("missing original record pattern")
        };
        assert_eq!(fields.len(), old_fields.len() + 1);
        for (old, new) in old_fields.iter().zip(fields) {
            assert_eq!(old.name, new.name);
            let RecordMatchFieldPattern::Binding { name: old_name, .. } = &old.pattern else {
                panic!("fixture requires original owned field bindings")
            };
            let RecordMatchFieldPattern::Binding { name: new_name, .. } = &new.pattern else {
                panic!("migration removed an owned field binding")
            };
            assert_eq!(old_name, new_name);
        }
        assert_eq!(fields.last().unwrap().name, "added");
        assert!(matches!(
            fields.last().unwrap().pattern,
            RecordMatchFieldPattern::Wildcard { .. }
        ));
        assert_eq!(
            semaprax::format::expr(&old_arms[0].value, 0),
            semaprax::format::expr(&arms[0].value, 0)
        );
    }
    replay(&candidate);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn added_nodrop_scalar_preserves_live_field_loan_and_ordered_cleanup_actions() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let candidate = apply(&base, &request("append.packet", "usize", json!(u64::MAX))).unwrap();
    let before = base
        .revision()
        .entry_program()
        .functions
        .iter()
        .find(|f| f.id.as_str() == "append.inspect")
        .unwrap();
    let after = candidate
        .revision()
        .entry_program()
        .functions
        .iter()
        .find(|f| f.id.as_str() == "append.inspect")
        .unwrap();
    assert_eq!(before.loan_plan, after.loan_plan);
    assert!(after
        .loan_plan
        .loans
        .iter()
        .any(|loan| loan.origin.projections
            == [PlaceProjection::Field(DeclarationId::new(
                "append.packet.left"
            ))]));
    assert_eq!(before.cleanup.flags, after.cleanup.flags);
    assert_eq!(
        before.cleanup_plan.entry_state,
        after.cleanup_plan.entry_state
    );
    assert_eq!(before.cleanup_plan.blocks, after.cleanup_plan.blocks);
    assert_eq!(before.cleanup_plan.edges, after.cleanup_plan.edges);
    assert_eq!(before.cleanup_plan.regions, after.cleanup_plan.regions);
    assert_eq!(before.cleanup_plan.exits, after.cleanup_plan.exits);
    assert_eq!(before.cleanup.slots.len(), after.cleanup.slots.len());
    let mut found = false;
    for (old, new) in before.cleanup.slots.iter().zip(&after.cleanup.slots) {
        if let (
            FieldLivenessShape::Record {
                declaration,
                fields: old_fields,
            },
            FieldLivenessShape::Record { fields, .. },
        ) = (&old.shape, &new.shape)
        {
            if declaration.as_str() == "append.packet" {
                found = true;
                assert_eq!(fields.len(), old_fields.len() + 1);
                assert_eq!(&fields[..old_fields.len()], old_fields);
                assert_eq!(fields.last().unwrap().field.as_str(), "append.packet.added");
                assert!(matches!(
                    fields.last().unwrap().shape,
                    FieldLivenessShape::NoDrop
                ));
            }
        }
    }
    assert!(found);
    replay(&candidate);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn checked_copy_records_accept_exact_inert_scalar_defaults_and_reject_range_or_kind_confusion() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let before = base.to_json().to_owned();
    for (ty, value) in [
        ("i64", json!(-i64::MAX)),
        ("i64", json!(i64::MAX)),
        ("bool", json!(true)),
        ("i32", json!(-i32::MAX)),
        ("i32", json!(i32::MAX)),
        ("u8", json!(u8::MAX)),
        ("usize", json!(u64::MAX)),
    ] {
        let candidate = apply(&base, &request("append.small", ty, value)).unwrap();
        replay(&candidate);
        let core = source(&candidate, "src/core.spx");
        assert!(core.contains("Small { size: 3usize, byte: 2u8, signed: 1i32, added:"));
        let bridge = source(&candidate, "src/bridge.spx");
        assert!(bridge.contains("Gauge { byte: 2u8, signed: 1i32, size: 3usize, added:"));
    }
    for (ty, value) in [
        // The frozen lexer rejects these positive magnitudes before parsing
        // unary minus; field evolution does not widen source literal syntax.
        ("i64", json!(i64::MIN)),
        ("i32", json!(i32::MIN)),
        ("i32", json!(2147483648i64)),
        ("i32", json!(-2147483649i64)),
        ("u8", json!(256)),
        ("u8", json!(-1)),
        ("usize", json!(-1)),
        ("bool", json!(1)),
    ] {
        code(
            apply(&base, &request("append.small", ty, value)),
            "SPX-G225",
        );
    }
    let mut wrong_kind = request("append.small", "i32", json!(7));
    wrong_kind["field"]["default"]["kind"] = json!("u8");
    code(apply(&base, &wrong_kind), "SPX-G225");
    let mut effectful = request("append.packet", "i64", json!(0));
    effectful["field"]["default"] =
        json!({"kind":"call","target":"append.public","arguments":[{"kind":"i64","value":0}]});
    code(apply(&base, &effectful), "SPX-G225");
    code(
        apply(&base, &request("append.packet", "Bytes", json!(0))),
        "SPX-G225",
    );
    assert_eq!(base.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn owned_field_history_replays_unrelated_changes_but_rejects_stale_and_competing_shapes() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let intent = request("append.packet", "bool", json!(false));
    let change = SemanticChange::new(base.revision().project_revision(), &intent).unwrap();
    let candidate = base.apply(base.candidate_digest(), &change).unwrap();
    code(
        candidate.apply(candidate.candidate_digest(), &change),
        "SPX-G224",
    );
    let renamed = apply(
        &base,
        &json!({"kind":"rename_declaration","target":"append.public","name":"renamed_public"}),
    )
    .unwrap();
    let rebased = candidate
        .rebase(
            candidate.candidate_digest(),
            Arc::clone(renamed.revision()),
            renamed.revision().project_revision(),
        )
        .unwrap();
    replay(rebased.candidate());
    let mut competing = request("append.packet", "i64", json!(3));
    competing["field"]["id"] = json!("append.packet.other");
    competing["field"]["name"] = json!("other");
    let competing = apply(&base, &competing).unwrap();
    let before = candidate.to_json().to_owned();
    code(
        candidate.rebase(
            candidate.candidate_digest(),
            Arc::clone(competing.revision()),
            competing.revision().project_revision(),
        ),
        "SPX-G235",
    );
    assert_eq!(candidate.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn owned_type_aliases_are_admitted_as_exact_type_imports() {
    let fixture = Fixture::new();
    let bridge = std::fs::read_to_string(fixture.0.join("src/bridge.spx")).unwrap();
    fixture.write(
        "src/bridge.spx",
        &bridge.replacen(
            "module append.bridge;",
            "module append.bridge;\nuse type @id(\"append.packet\") from append.core as Frame;",
            1,
        ),
    );
    let disk = fixture.bytes();
    let graph = with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        Ok(snapshot.semantic_graph().to_owned())
    })
    .unwrap();
    assert!(graph.contains("\"target\":\"append.packet\""));
    assert!(graph.contains("\"kind\":\"type_import\""));
    assert_eq!(fixture.bytes(), disk);
}
