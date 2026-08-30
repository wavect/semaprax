//! Stable-ID aggregate constructor regressions, authored and intentionally unrun.
use semaprax::ast::{Expr, ExprKind, FieldInitializer, Function, Program};
use semaprax::diagnostic::Diagnostic;
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
            "spx-aggregate-expression-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "aggregate-expression"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "aggregate.app"
sources = ["src/app.spx", "src/bridge.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["aggregate.public"]
tests = ["aggregate.tests"]
"#,
        )
        .unwrap();
        for (path, source) in [
            (
                "src/core.spx",
                r#"module aggregate.core;
@id("aggregate.pair") record Pair {
    @id("aggregate.pair.left") left: i64,
    @id("aggregate.pair.right") right: i64,
}
@id("aggregate.wrapper") record Wrapper {
    @id("aggregate.wrapper.pair") pair: Pair,
    @id("aggregate.wrapper.flag") flag: bool,
}
@id("aggregate.choice") variant Choice {
    @id("aggregate.choice.value") Value {
        @id("aggregate.choice.value.first") first: i64,
        @id("aggregate.choice.value.second") second: i64,
    },
    @id("aggregate.choice.empty") Empty,
}
@id("aggregate.make-pair") fn make_pair(value: i64) -> Pair { Pair { left: value, right: 0 } }
@id("aggregate.make-choice") fn make_choice(value: i64) -> Choice { Choice::Value { first: value, second: 0 } }
@id("aggregate.make-wrapper") fn make_wrapper(value: i64) -> Wrapper { Wrapper { pair: Pair { left: value, right: 0 }, flag: true } }
@id("aggregate.public") fn public_value(value: i64) -> i64 { value }
@id("aggregate.evaluate") fn evaluate(value: i64) -> i64 {
    let pair = make_pair(value);
    let choice = make_choice(2);
    pair.left + match choice { Choice::Value { first, second: _ } => first, Choice::Empty {} => 0, }
}
"#,
            ),
            (
                "src/bridge.spx",
                r#"module aggregate.bridge;
use type @id("aggregate.pair") from aggregate.core as Metric;
use type @id("aggregate.choice") from aggregate.core as Signal;
@id("bridge.pair") record Pair { @id("bridge.pair.flag") flag: bool, }
@id("bridge.choice") variant Choice { @id("bridge.choice.empty") Empty, }
@id("bridge.make-pair") fn make_pair(value: i64) -> Metric { Metric { left: value, right: 0 } }
@id("bridge.make-choice") fn make_choice(value: i64) -> Signal { Signal::Value { first: value, second: 0 } }
@id("bridge.evaluate") fn evaluate(value: i64) -> i64 {
    let pair = make_pair(value);
    let choice = make_choice(0);
    pair.left + match choice { Signal::Value { first, second: _ } => first, Signal::Empty {} => 0, }
}
"#,
            ),
            (
                "src/app.spx",
                r#"module aggregate.app;
use function @id("aggregate.evaluate") from aggregate.core as evaluate;
use function @id("bridge.evaluate") from aggregate.bridge as other;
@id("aggregate.main") fn main() -> i64 { evaluate(40) + other(0) }
"#,
            ),
            (
                "src/tests.spx",
                r#"module aggregate.tests;
use function @id("aggregate.evaluate") from aggregate.core as evaluate;
@id("aggregate.test") fn main() -> i64 { if evaluate(40) == 42 { 0 } else { 1 } }
"#,
            ),
        ] {
            let program = semaprax::parse(source, path).unwrap();
            std::fs::write(root.join(path), semaprax::format::canonical(&program)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn candidate(&self) -> ProjectCandidate {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
        })
        .unwrap()
    }
    fn append(&self, relative: &str, addition: &str) {
        let path = self.0.join(relative);
        let source = std::fs::read_to_string(&path).unwrap() + addition;
        let program = semaprax::parse(&source, relative).unwrap();
        std::fs::write(path, semaprax::format::canonical(&program)).unwrap();
    }
    fn bytes(&self) -> Vec<Vec<u8>> {
        [
            "semaprax.toml",
            "src/app.spx",
            "src/bridge.spx",
            "src/core.spx",
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
fn program(candidate: &ProjectCandidate, path: &str) -> Program {
    semaprax::parse(source(candidate, path), path).unwrap()
}
fn function<'a>(program: &'a Program, target: &str) -> &'a Function {
    program
        .functions
        .iter()
        .find(|function| function.stable_id == target)
        .unwrap()
}
fn tail(mut expression: &Expr) -> &Expr {
    while let ExprKind::Block { statements, tail } = &expression.kind {
        assert!(statements.is_empty());
        expression = tail;
    }
    expression
}
fn integer(value: i64) -> Value {
    json!({"kind":"i64","value":value})
}
fn division(left: i64, right: i64) -> Value {
    json!({"kind":"binary","op":"/","left":integer(left),"right":integer(right)})
}
fn record() -> Value {
    // Reverse declaration order is intentional and affects checked failure order.
    json!({"kind":"record","target":"aggregate.pair","fields":[
        {"target":"aggregate.pair.right","value":division(1,0)},
        {"target":"aggregate.pair.left","value":division(8,2)}
    ]})
}
fn variant() -> Value {
    json!({"kind":"variant","target":"aggregate.choice.value","fields":[
        {"target":"aggregate.choice.value.second","value":division(1,0)},
        {"target":"aggregate.choice.value.first","value":division(6,2)}
    ]})
}
fn apply(
    candidate: &ProjectCandidate,
    target: &str,
    body: Value,
) -> Result<(ProjectCandidate, SemanticChange), Vec<Diagnostic>> {
    let change = SemanticChange::new(
        candidate.revision().project_revision(),
        &json!({"kind":"replace_function_body","target":target,"body":body}),
    )?;
    Ok((
        candidate.apply(candidate.candidate_digest(), &change)?,
        change,
    ))
}
fn assert_fields(fields: &[FieldInitializer], source: &str, expected: &[(&str, &str)]) {
    assert_eq!(fields.len(), expected.len());
    for (field, (name, value)) in fields.iter().zip(expected) {
        assert_eq!(field.name, *name);
        assert_eq!(
            &source[field.value.span.start..field.value.span.end],
            *value
        );
    }
}
fn replay(base: &ProjectCandidate, candidate: &ProjectCandidate, change: SemanticChange) {
    let restored = ProjectCandidate::replay(
        Arc::clone(base.base_revision()),
        base.base_revision().project_revision(),
        &[change],
        candidate.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(restored.candidate_digest(), candidate.candidate_digest());
    assert_eq!(restored.to_json(), candidate.to_json());
}
fn selection(candidate: &ProjectCandidate, target: &str, path: &str, snippet: &str) -> String {
    let catalog: Value =
        serde_json::from_str(&candidate.expression_catalog(target).unwrap()).unwrap();
    let source = source(candidate, path);
    catalog["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| {
            let span = &item["source_span"];
            item["replaceable"] == true
                && source.get(
                    span["start"].as_u64().unwrap() as usize
                        ..span["end"].as_u64().unwrap() as usize,
                ) == Some(snippet)
        })
        .unwrap()["expression_id"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn record_body_uses_stable_ids_and_preserves_supplied_field_order_through_aliases() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    for (target, path, spelling) in [
        ("aggregate.make-pair", "src/core.spx", "Pair"),
        ("bridge.make-pair", "src/bridge.spx", "Metric"),
    ] {
        let discovery: Value = serde_json::from_str(&base.change_catalog(target).unwrap()).unwrap();
        let descriptor = discovery["aggregate_constructors"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["target"] == "aggregate.pair")
            .unwrap();
        assert_eq!(descriptor["kind"], "record");
        assert_eq!(descriptor["binding"], spelling);
        assert_eq!(descriptor["generic"], false);
        assert_eq!(descriptor["evidence_owner"], "retained_checked_hir");
        assert_eq!(descriptor["requires_full_candidate_validation"], true);
        assert_eq!(
            descriptor["fields"]
                .as_array()
                .unwrap()
                .iter()
                .map(|field| field["target"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["aggregate.pair.left", "aggregate.pair.right"]
        );
        let (candidate, change) = apply(&base, target, record()).unwrap();
        let projected = program(&candidate, path);
        let ExprKind::ConstructRecord {
            type_name,
            type_arguments,
            fields,
            ..
        } = &tail(&function(&projected, target).body).kind
        else {
            panic!("record constructor missing")
        };
        assert_eq!(type_name, spelling);
        assert!(type_arguments.is_empty());
        assert_fields(
            fields,
            source(&candidate, path),
            &[("right", "1 / 0"), ("left", "8 / 2")],
        );
        assert_eq!(function(&projected, target).stable_id, target);
        replay(&base, &candidate, change);
    }
    // This is checked evaluation-order structure, not executed failure evidence.
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn variant_case_identity_selects_exact_payloads_and_empty_cases_locally_and_via_alias() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    for (target, path, spelling) in [
        ("aggregate.make-choice", "src/core.spx", "Choice"),
        ("bridge.make-choice", "src/bridge.spx", "Signal"),
    ] {
        let (candidate, change) = apply(&base, target, variant()).unwrap();
        let projected = program(&candidate, path);
        let ExprKind::ConstructVariant {
            type_name,
            case_name,
            fields,
            ..
        } = &tail(&function(&projected, target).body).kind
        else {
            panic!("variant constructor missing")
        };
        assert_eq!(type_name, spelling);
        assert_eq!(case_name, "Value");
        assert_fields(
            fields,
            source(&candidate, path),
            &[("second", "1 / 0"), ("first", "6 / 2")],
        );
        replay(&base, &candidate, change);
        let (empty, change) = apply(
            &base,
            target,
            json!({"kind":"variant","target":"aggregate.choice.empty","fields":[]}),
        )
        .unwrap();
        let projected = program(&empty, path);
        let ExprKind::ConstructVariant {
            type_name,
            case_name,
            fields,
            ..
        } = &tail(&function(&projected, target).body).kind
        else {
            panic!("empty variant constructor missing")
        };
        assert_eq!(type_name, spelling);
        assert_eq!(case_name, "Empty");
        assert!(fields.is_empty());
        replay(&base, &empty, change);
    }
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn nested_constructor_and_revision_scoped_expression_replacement_replay_as_source() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let nested = json!({"kind":"record","target":"aggregate.wrapper","fields":[
        {"target":"aggregate.wrapper.flag","value":{"kind":"bool","value":false}},
        {"target":"aggregate.wrapper.pair","value":{"kind":"record","target":"aggregate.pair","fields":[
            {"target":"aggregate.pair.right","value":integer(2)},
            {"target":"aggregate.pair.left","value":{"kind":"place","name":"value"}}
        ]}}
    ]});
    let (candidate, change) = apply(&base, "aggregate.make-wrapper", nested).unwrap();
    assert!(source(&candidate, "src/core.spx")
        .contains("Wrapper { flag: false, pair: Pair { right: 2, left: value } }"));
    replay(&base, &candidate, change);
    let expression = selection(
        &base,
        "bridge.make-pair",
        "src/bridge.spx",
        "Metric { left: value, right: 0 }",
    );
    let change = SemanticChange::new(base.revision().project_revision(), &json!({"kind":"replace_expression","target":"bridge.make-pair","expression_id":expression,"replacement":record()})).unwrap();
    let replaced = base.apply(base.candidate_digest(), &change).unwrap();
    assert!(source(&replaced, "src/bridge.spx").contains("Metric { right: 1 / 0, left: 8 / 2 }"));
    replay(&base, &replaced, change.clone());
    assert!(replaced
        .apply(replaced.candidate_digest(), &change)
        .is_err());
    assert!(replaced.apply(base.candidate_digest(), &change).is_err());
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn body_and_expression_holes_share_aggregate_constructor_admission() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = Arc::new(fixture.candidate());
    let selected = selection(
        &base,
        "bridge.make-choice",
        "src/bridge.spx",
        "Signal::Value { first: value, second: 0 }",
    );
    let empty = ProjectCandidateDraft::open(Arc::clone(&base)).unwrap();
    let draft = empty
        .with_body_hole(empty.draft_digest(), "aggregate.make-pair", "pair")
        .unwrap();
    let draft = draft
        .with_expression_hole(
            draft.draft_digest(),
            "bridge.make-choice",
            &selected,
            "choice",
        )
        .unwrap();
    for (hole, target, binding, kind) in [
        ("pair", "aggregate.pair", "Pair", "record"),
        ("choice", "aggregate.choice.value", "Signal", "variant"),
    ] {
        let context: Value =
            serde_json::from_str(&draft.hole_context(draft.draft_digest(), hole).unwrap()).unwrap();
        assert_eq!(context["materializable"], false);
        assert_eq!(context["source_authority"], false);
        assert!(context["constructor_kinds"]
            .as_array()
            .unwrap()
            .contains(&json!(kind)));
        let descriptor = context["aggregate_constructors"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["target"] == target)
            .unwrap();
        assert_eq!(descriptor["binding"], binding);
        assert_eq!(descriptor["requires_full_candidate_validation"], true);
    }
    assert!(draft.complete(draft.draft_digest()).is_err());
    let old = draft.to_json().to_owned();
    assert!(draft
        .fill_hole(
            draft.draft_digest(),
            "pair",
            &json!({"kind":"bool","value":true})
        )
        .is_err());
    assert_eq!(draft.to_json(), old);
    let first = draft
        .fill_hole(draft.draft_digest(), "pair", &record())
        .unwrap();
    assert!(first.complete(first.draft_digest()).is_err());
    let second = first
        .fill_hole(first.draft_digest(), "choice", &variant())
        .unwrap();
    let candidate = second.complete(second.draft_digest()).unwrap();
    assert!(source(&candidate, "src/core.spx").contains("Pair { right: 1 / 0, left: 8 / 2 }"));
    assert!(source(&candidate, "src/bridge.spx")
        .contains("Signal::Value { second: 1 / 0, first: 6 / 2 }"));
    let capsule = candidate.recovery_capsule().unwrap();
    let restored = ProjectCandidate::restore(
        Arc::clone(base.base_revision()),
        base.base_revision().project_revision(),
        capsule.as_bytes(),
    )
    .unwrap();
    assert_eq!(restored.candidate_digest(), candidate.candidate_digest());
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn wrong_duplicate_missing_foreign_and_mistyped_members_never_mutate_the_candidate() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let unchanged = base.to_json().to_owned();
    let mut absent = record();
    absent["target"] = json!("aggregate.absent");
    let mut wrong_kind = record();
    wrong_kind["target"] = json!("aggregate.choice");
    let mut display_name = record();
    display_name["target"] = json!("Pair");
    let mut duplicate = record();
    duplicate["fields"][1] = duplicate["fields"][0].clone();
    let mut missing = record();
    missing["fields"].as_array_mut().unwrap().pop();
    let mut foreign = record();
    foreign["fields"][0]["target"] = json!("bridge.pair.flag");
    let mut extra = record();
    extra["type_name"] = json!("Pair");
    for expression in [
        absent,
        wrong_kind,
        display_name,
        duplicate,
        missing,
        foreign,
        extra,
    ] {
        let errors = apply(&base, "aggregate.make-pair", expression)
            .err()
            .expect("invalid identity shape admitted");
        assert!(
            errors.iter().any(|error| error.code == "SPX-G225"),
            "{errors:?}"
        );
    }
    let mut missing = variant();
    missing["fields"].as_array_mut().unwrap().pop();
    let mut duplicate = variant();
    duplicate["fields"][1] = duplicate["fields"][0].clone();
    let mut owner = variant();
    owner["target"] = json!("aggregate.choice");
    let mut wrong_case = variant();
    wrong_case["target"] = json!("aggregate.choice.empty");
    for expression in [missing, duplicate, owner, wrong_case] {
        let errors = apply(&base, "aggregate.make-choice", expression)
            .err()
            .expect("invalid case membership admitted");
        assert!(
            errors.iter().any(|error| error.code == "SPX-G225"),
            "{errors:?}"
        );
    }
    let mut wrong_type = record();
    wrong_type["fields"][0]["value"] = json!({"kind":"bool","value":true});
    assert!(apply(&base, "aggregate.make-pair", wrong_type).is_err());
    // Correct local spelling is insufficient: the local Pair has another ID/type.
    assert!(apply(&base, "bridge.make-pair", json!({"kind":"record","target":"bridge.pair","fields":[{"target":"bridge.pair.flag","value":{"kind":"bool","value":true}}]})).is_err());
    assert_eq!(base.to_json(), unchanged);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn generic_or_implicit_shapes_are_not_discovered_and_unbound_source_types_reject() {
    let fixture = Fixture::new();
    fixture.append(
        "src/core.spx",
        r#"
@id("aggregate.box") record Box<T> { @id("aggregate.box.value") value: T, }
@id("aggregate.implicit") record Implicit { value: i64, }
@id("aggregate.empty") record Empty {}
@id("aggregate.make-empty") fn make_empty() -> Empty { Empty {} }
"#,
    );
    fixture.append(
        "src/app.spx",
        r#"
@id("aggregate.unbound") fn unbound() -> i64 { 0 }
"#,
    );
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let catalog: Value =
        serde_json::from_str(&base.change_catalog("aggregate.make-pair").unwrap()).unwrap();
    let constructors = catalog["aggregate_constructors"].as_array().unwrap();
    for unsupported in ["aggregate.box", "aggregate.implicit"] {
        assert!(!constructors
            .iter()
            .any(|item| item["target"] == unsupported));
    }
    let unavailable: Value =
        serde_json::from_str(&base.change_catalog("aggregate.unbound").unwrap()).unwrap();
    assert!(!unavailable
        .get("aggregate_constructors")
        .is_some_and(|items| items
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["target"] == "aggregate.pair")));
    for (target, expression) in [
        (
            "aggregate.make-pair",
            json!({"kind":"record","target":"aggregate.box","fields":[{"target":"aggregate.box.value","value":integer(1)}]}),
        ),
        ("aggregate.unbound", record()),
    ] {
        let errors = apply(&base, target, expression)
            .err()
            .expect("unsupported or unbound constructor admitted");
        assert!(
            errors.iter().any(|error| error.code == "SPX-G225"),
            "{errors:?}"
        );
    }
    let (empty, change) = apply(
        &base,
        "aggregate.make-empty",
        json!({"kind":"record","target":"aggregate.empty","fields":[]}),
    )
    .unwrap();
    let projected = program(&empty, "src/core.spx");
    let ExprKind::ConstructRecord {
        type_name, fields, ..
    } = &tail(&function(&projected, "aggregate.make-empty").body).kind
    else {
        panic!("empty record construction missing")
    };
    assert_eq!(type_name, "Empty");
    assert!(fields.is_empty());
    replay(&base, &empty, change);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn duplicate_type_aliases_are_rejected_by_source_admission_before_candidate_construction() {
    let fixture = Fixture::new();
    let base = fixture.candidate();
    let old_candidate = base.to_json().to_owned();
    let source_before = source(&base, "src/bridge.spx").to_owned();
    let path = fixture.0.join("src/bridge.spx");
    let duplicate = source_before.replacen(
        "module aggregate.bridge;",
        "module aggregate.bridge;\nuse type @id(\"aggregate.pair\") from aggregate.core as SecondMetric;",
        1,
    );
    let parsed = semaprax::parse(&duplicate, "src/bridge.spx").unwrap();
    std::fs::write(&path, semaprax::format::canonical(&parsed)).unwrap();
    let hostile_bytes = fixture.bytes();
    let errors =
        with_authenticated_project(&fixture.0.join("semaprax.toml"), |_| Ok(())).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.code == "SPX-G172"
                && error.message.contains("imported more than once")),
        "{errors:?}"
    );
    // The invalid live proposal never enters an admitted candidate. An existing
    // immutable candidate remains bound to its original unique type alias.
    assert_eq!(base.to_json(), old_candidate);
    assert_eq!(source(&base, "src/bridge.spx"), source_before);
    let (candidate, _) = apply(&base, "bridge.make-pair", record()).unwrap();
    assert!(source(&candidate, "src/bridge.spx").contains("Metric { right: 1 / 0, left: 8 / 2 }"));
    assert_eq!(fixture.bytes(), hostile_bytes);
}
