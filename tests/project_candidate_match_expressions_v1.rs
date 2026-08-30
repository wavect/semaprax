//! Exhaustive stable-ID match constructors: authored, intentionally unrun.
use semaprax::ast::{Expr, ExprKind, MatchArm, MatchMode, MatchPattern, Program, Statement, Type};
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
            "spx-match-constructor-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let root = root.canonicalize().unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "match-constructor"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "matching.app"
sources = ["src/app.spx", "src/bridge.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["matching.public"]
tests = ["matching.tests"]
"#,
        )
        .unwrap();
        for (path, text) in [
            (
                "src/core.spx",
                r#"module matching.core;
@id("matching.choice") variant Choice<T> {
    @id("matching.choice.some") Some { @id("matching.choice.some.value") value: T, @id("matching.choice.some.flag") flag: bool, },
    @id("matching.choice.none") None,
}
@id("matching.switch") variant Switch { @id("matching.switch.on") On { @id("matching.switch.on.value") value: i64, }, @id("matching.switch.off") Off, }
@id("matching.phantom") variant Phantom<T> { @id("matching.phantom.marker") Marker { @id("matching.phantom.marker.value") value: i64, }, }
@id("matching.make") fn make_choice(value: i64) -> Choice<i64> { Choice<i64>::Some { value: value, flag: false } }
@id("matching.read") fn read(item: Choice<i64>) -> i64 { match item { Choice::Some { value, flag } => value + if flag { 1 } else { 0 }, Choice::None {} => 0, } }
@id("matching.call") fn read_call(value: i64) -> i64 { read(make_choice(value)) }
@id("matching.switch-read") fn read_switch(item: Switch) -> i64 { match item { Switch::On { value } => value, Switch::Off {} => 0, } }
@id("matching.phantom-read") fn read_phantom(item: Phantom<i64>) -> i64 { match item { Phantom::Marker { value } => value, } }
@id("matching.option-read") fn read_option(item: Option<i64>) -> i64 { match item { Option::Some { value } => value, Option::None {} => 0, } }
@id("matching.result-read") fn read_result(item: Result<i64, bool>) -> i64 { match item { Result::Ok { value } => value, Result::Err { error } => if error { 1 } else { 0 }, } }
@id("matching.public") fn public_value(value: i64) -> i64 { value }
@id("matching.evaluate") fn evaluate(value: i64) -> i64 {
    read_call(value) + read_switch(Switch::On { value: 0 })
        + read_phantom(Phantom<i64>::Marker { value: 0 })
        + read_option(Option<i64>::Some { value: 0 })
        + read_result(Result<i64, bool>::Ok { value: 0 })
}
"#,
            ),
            (
                "src/bridge.spx",
                r#"module matching.bridge;
use type @id("matching.choice") from matching.core as Signal;
@id("bridge.choice") variant Choice<T> { @id("bridge.choice.some") Some { @id("bridge.choice.some.value") value: T, @id("bridge.choice.some.flag") flag: bool, }, @id("bridge.choice.none") None, }
@id("bridge.read") fn read(item: Signal<i64>) -> i64 { match item { Signal::Some { value, flag } => value + if flag { 1 } else { 0 }, Signal::None {} => 0, } }
@id("bridge.wrong") fn wrong(item: Choice<i64>) -> i64 { match item { Choice::Some { value, flag } => value + if flag { 1 } else { 0 }, Choice::None {} => 0, } }
@id("bridge.evaluate") fn evaluate() -> i64 { read(Signal<i64>::None {}) + wrong(Choice<i64>::None {}) }
"#,
            ),
            (
                "src/app.spx",
                r#"module matching.app;
use function @id("matching.evaluate") from matching.core as evaluate;
use function @id("bridge.evaluate") from matching.bridge as other;
@id("matching.main") fn main() -> i64 { evaluate(42) + other() }
"#,
            ),
            (
                "src/tests.spx",
                r#"module matching.tests;
use function @id("matching.evaluate") from matching.core as evaluate;
@id("matching.test") fn main() -> i64 { if evaluate(42) == 42 { 0 } else { 1 } }
"#,
            ),
        ] {
            let program = semaprax::parse(text, path).unwrap();
            std::fs::write(root.join(path), semaprax::format::canonical(&program)).unwrap();
        }
        Self(root)
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
fn body<'a>(program: &'a Program, id: &str) -> &'a Expr {
    &program
        .functions
        .iter()
        .find(|function| function.stable_id == id)
        .unwrap()
        .body
}
fn unwrapped(mut expr: &Expr) -> &Expr {
    while let ExprKind::Block { statements, tail } = &expr.kind {
        if !statements.is_empty() {
            break;
        }
        expr = tail;
    }
    expr
}
fn lowered<'a>(expr: &'a Expr, binding: &str, arguments: &[Type]) -> (&'a Expr, &'a [MatchArm]) {
    let ExprKind::Block { statements, tail } = &unwrapped(expr).kind else {
        panic!("typed single-evaluation staging missing")
    };
    assert_eq!(statements.len(), 1);
    let Statement::Let {
        name,
        declared,
        mutable,
        value,
        ..
    } = &statements[0]
    else {
        panic!("typed staging let missing")
    };
    assert!(!mutable);
    assert_eq!(
        declared,
        &Some(Type::Named {
            name: binding.to_owned(),
            arguments: arguments.to_vec()
        })
    );
    let ExprKind::Match {
        mode,
        scrutinee,
        arms,
    } = &tail.kind
    else {
        panic!("match missing")
    };
    assert_eq!(*mode, MatchMode::Value);
    assert_eq!(scrutinee.kind, ExprKind::Var(name.clone()));
    assert!(arms.iter().all(|arm| arm.guard.is_none()));
    (value, arms)
}
fn integer(value: i64) -> Value {
    json!({"kind":"i64","value":value})
}
fn place(name: &str) -> Value {
    json!({"kind":"place","name":name})
}
fn arm(target: &str, fields: &[(&str, &str)], body: Value) -> Value {
    json!({"target":target,"fields":fields.iter().map(|(target,name)|json!({"target":target,"name":name})).collect::<Vec<_>>(),"body":body})
}
fn matching(target: &str, value: Value, args: &[&str], arms: Vec<Value>) -> Value {
    json!({"kind":"match","target":target,"value":value,"type_arguments":args,"arms":arms})
}
fn choice(value: Value) -> Value {
    matching(
        "matching.choice",
        value,
        &["i64"],
        vec![
            arm("matching.choice.none", &[], integer(0)),
            arm(
                "matching.choice.some",
                &[
                    ("matching.choice.some.flag", "seen_flag"),
                    ("matching.choice.some.value", "captured"),
                ],
                json!({"kind":"binary","op":"+","left":place("captured"),"right":{"kind":"if","condition":place("seen_flag"),"then":integer(1),"else":integer(0)}}),
            ),
        ],
    )
}
fn apply(
    base: &ProjectCandidate,
    target: &str,
    body: Value,
) -> Result<(ProjectCandidate, SemanticChange), Vec<Diagnostic>> {
    let change = SemanticChange::new(
        base.revision().project_revision(),
        &json!({"kind":"replace_function_body","target":target,"body":body}),
    )?;
    Ok((base.apply(base.candidate_digest(), &change)?, change))
}
fn replay(base: &ProjectCandidate, candidate: &ProjectCandidate, change: SemanticChange) {
    let restored = ProjectCandidate::replay(
        Arc::clone(base.base_revision()),
        base.base_revision().project_revision(),
        &[change],
        candidate.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(restored.to_json(), candidate.to_json());
}
fn grammar<T>(result: Result<T, Vec<Diagnostic>>) {
    let errors = result.err().expect("unsupported match accepted");
    assert!(
        errors.iter().any(|error| error.code == "SPX-G225"),
        "{errors:?}"
    );
}

#[test]
fn local_imported_and_call_scrutinees_stage_once_and_preserve_requested_arm_and_payload_order() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    for (target, path, binding, value) in [
        ("matching.read", "src/core.spx", "Choice", place("item")),
        ("bridge.read", "src/bridge.spx", "Signal", place("item")),
        (
            "matching.call",
            "src/core.spx",
            "Choice",
            json!({"kind":"call","target":"matching.make","arguments":[place("value")]}),
        ),
    ] {
        let (candidate, change) = apply(&base, target, choice(value)).unwrap();
        let program = program(&candidate, path);
        let (initializer, arms) = lowered(body(&program, target), binding, &[Type::I64]);
        if target == "matching.call" {
            let ExprKind::Call { name, args, .. } = &initializer.kind else {
                panic!("call initializer lost")
            };
            assert_eq!(name, "make_choice");
            assert_eq!(args.len(), 1);
            assert_eq!(args[0].kind, ExprKind::Var("value".to_owned()));
        } else {
            assert_eq!(initializer.kind, ExprKind::Var("item".to_owned()));
        }
        assert_eq!(arms.len(), 2);
        for (arm, expected) in arms.iter().zip(["None", "Some"]) {
            let MatchPattern::Variant {
                type_name,
                case_name,
                fields,
                ..
            } = &arm.pattern
            else {
                panic!("exact variant pattern missing")
            };
            assert_eq!(type_name, binding);
            assert_eq!(case_name, expected);
            if expected == "Some" {
                assert_eq!(
                    fields
                        .iter()
                        .map(|field| (field.name.as_str(), field.binding.as_str()))
                        .collect::<Vec<_>>(),
                    [("flag", "seen_flag"), ("value", "captured")]
                );
            } else {
                assert!(fields.is_empty());
            }
        }
        replay(&base, &candidate, change);
    }
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn monomorphic_and_compiler_prelude_matches_use_complete_exact_case_tables() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    for (target, owner, binding, arguments, types, arms) in [
        (
            "matching.switch-read",
            "matching.switch",
            "Switch",
            vec![],
            vec![],
            vec![
                arm("matching.switch.off", &[], integer(0)),
                arm(
                    "matching.switch.on",
                    &[("matching.switch.on.value", "captured")],
                    place("captured"),
                ),
            ],
        ),
        (
            "matching.option-read",
            "core.option",
            "Option",
            vec!["i64"],
            vec![Type::I64],
            vec![
                arm("core.option.none", &[], integer(0)),
                arm(
                    "core.option.some",
                    &[("core.option.some.value", "captured")],
                    place("captured"),
                ),
            ],
        ),
        (
            "matching.result-read",
            "core.result",
            "Result",
            vec!["i64", "bool"],
            vec![Type::I64, Type::Bool],
            vec![
                arm(
                    "core.result.err",
                    &[("core.result.err.error", "captured")],
                    json!({"kind":"if","condition":place("captured"),"then":integer(1),"else":integer(0)}),
                ),
                arm(
                    "core.result.ok",
                    &[("core.result.ok.value", "captured")],
                    place("captured"),
                ),
            ],
        ),
    ] {
        let (candidate, change) = apply(
            &base,
            target,
            matching(owner, place("item"), &arguments, arms),
        )
        .unwrap();
        let program = program(&candidate, "src/core.spx");
        let (value, arms) = lowered(body(&program, target), binding, &types);
        assert_eq!(value.kind, ExprKind::Var("item".to_owned()));
        assert_eq!(arms.len(), 2);
        // The two Result arms deliberately reuse a binder name in disjoint scopes.
        replay(&base, &candidate, change);
    }
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn coverage_binder_scope_nominal_types_and_constructor_bounds_fail_closed() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let before = base.to_json().to_owned();
    let valid = choice(place("item"));
    let mut missing = valid.clone();
    missing["arms"].as_array_mut().unwrap().pop();
    grammar(apply(&base, "matching.read", missing));
    let mut duplicate = valid.clone();
    duplicate["arms"][0] = duplicate["arms"][1].clone();
    grammar(apply(&base, "matching.read", duplicate));
    let mut foreign = valid.clone();
    foreign["arms"][0]["target"] = json!("core.option.none");
    grammar(apply(&base, "matching.read", foreign));
    let mut missing_field = valid.clone();
    missing_field["arms"][1]["fields"]
        .as_array_mut()
        .unwrap()
        .pop();
    grammar(apply(&base, "matching.read", missing_field));
    let mut duplicate_field = valid.clone();
    duplicate_field["arms"][1]["fields"][0]["target"] = json!("matching.choice.some.value");
    grammar(apply(&base, "matching.read", duplicate_field));
    let mut foreign_field = valid.clone();
    foreign_field["arms"][1]["fields"][0]["target"] = json!("core.option.some.value");
    grammar(apply(&base, "matching.read", foreign_field));
    for collision in ["item", "make_choice", "Choice", "captured", "_"] {
        let mut request = valid.clone();
        request["arms"][1]["fields"][0]["name"] = json!(collision);
        grammar(apply(&base, "matching.read", request));
    }
    let mut leaked = valid.clone();
    leaked["arms"][0]["body"] = place("captured");
    grammar(apply(&base, "matching.read", leaked));
    let mut nested = valid.clone();
    nested["arms"][1]["body"] = choice(place("item"));
    grammar(apply(&base, "matching.read", nested));
    let mut body_type = valid.clone();
    body_type["arms"][0]["body"] = json!({"kind":"bool","value":true});
    assert!(apply(&base, "matching.read", body_type).is_err());
    assert!(apply(&base, "bridge.wrong", valid.clone()).is_err());
    let phantom = matching(
        "matching.phantom",
        place("item"),
        &["bool"],
        vec![arm(
            "matching.phantom.marker",
            &[("matching.phantom.marker.value", "captured")],
            place("captured"),
        )],
    );
    assert!(apply(&base, "matching.phantom-read", phantom).is_err());
    for args in [
        json!([]),
        json!(["i64", "bool"]),
        json!(["Bytes"]),
        json!("i64"),
    ] {
        let mut request = valid.clone();
        request["type_arguments"] = args;
        grammar(apply(&base, "matching.read", request));
    }
    let mut omitted = valid.clone();
    omitted.as_object_mut().unwrap().remove("type_arguments");
    grammar(apply(&base, "matching.read", omitted));
    let mut oversized = valid.clone();
    oversized["arms"] = Value::Array(vec![valid["arms"][0].clone(); 4096]);
    let errors = apply(&base, "matching.read", oversized).err().unwrap();
    assert!(
        errors.iter().any(|error| error.code == "SPX-G226"),
        "{errors:?}"
    );
    let (candidate, change) = apply(&base, "matching.read", valid).unwrap();
    assert!(candidate.apply(base.candidate_digest(), &change).is_err());
    assert_eq!(base.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn disjoint_and_nested_arm_scopes_keep_outer_bindings_available_without_capture() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let mut outer = choice(place("item"));
    let inner = matching(
        "core.option",
        json!({"kind":"variant","target":"core.option.some","type_arguments":["i64"],"fields":[{"target":"core.option.some.value","value":place("captured")}]}),
        &["i64"],
        vec![
            arm("core.option.none", &[], integer(0)),
            arm(
                "core.option.some",
                &[("core.option.some.value", "inner_value")],
                place("inner_value"),
            ),
        ],
    );
    outer["arms"][1]["body"] = inner;
    let (candidate, change) = apply(&base, "matching.read", outer).unwrap();
    let rendered = program(&candidate, "src/core.spx");
    let (_, arms) = lowered(body(&rendered, "matching.read"), "Choice", &[Type::I64]);
    let (inner_value, inner_arms) = lowered(&arms[1].value, "Option", &[Type::I64]);
    let ExprKind::ConstructVariant { fields, .. } = &inner_value.kind else {
        panic!("outer binder not passed to nested match")
    };
    assert_eq!(fields[0].value.kind, ExprKind::Var("captured".to_owned()));
    let MatchPattern::Variant { fields, .. } = &inner_arms[1].pattern else {
        panic!("nested pattern missing")
    };
    assert_eq!(fields[0].binding, "inner_value");
    replay(&base, &candidate, change);
    // A name resembling the generated prefix is an ordinary requested binder.
    // Reserving it before allocating the staging name prevents capture.
    let mut prefixed = choice(place("item"));
    prefixed["arms"][1]["fields"][1]["name"] = json!("spx_project_0");
    prefixed["arms"][1]["body"] = place("spx_project_0");
    let (candidate, change) = apply(&base, "matching.read", prefixed).unwrap();
    let program = program(&candidate, "src/core.spx");
    let expression = unwrapped(body(&program, "matching.read"));
    let ExprKind::Block { statements, .. } = &expression.kind else {
        panic!("staging missing")
    };
    let Statement::Let { name, .. } = &statements[0] else {
        panic!("staging missing")
    };
    assert_ne!(name, "spx_project_0");
    let (_, arms) = lowered(expression, "Choice", &[Type::I64]);
    let MatchPattern::Variant { fields, .. } = &arms[1].pattern else {
        panic!("pattern missing")
    };
    assert_eq!(fields[1].binding, "spx_project_0");
    assert_eq!(
        arms[1].value.kind,
        ExprKind::Var("spx_project_0".to_owned())
    );
    replay(&base, &candidate, change);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn body_and_expression_holes_discover_exhaustive_matches_and_recover_exactly() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = Arc::new(fixture.candidate());
    let expressions: Value =
        serde_json::from_str(&base.expression_catalog("bridge.read").unwrap()).unwrap();
    let text = source(&base, "src/bridge.spx");
    let selected = expressions["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| {
            let span = &row["source_span"];
            row["replaceable"] == true
                && text
                    .get(
                        span["start"].as_u64().unwrap() as usize
                            ..span["end"].as_u64().unwrap() as usize,
                    )
                    .is_some_and(|text| text.starts_with("match item"))
        })
        .unwrap()["expression_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let empty = ProjectCandidateDraft::open(Arc::clone(&base)).unwrap();
    let draft = empty
        .with_body_hole(empty.draft_digest(), "matching.read", "body")
        .unwrap();
    let draft = draft
        .with_expression_hole(draft.draft_digest(), "bridge.read", &selected, "expression")
        .unwrap();
    for (hole, binding) in [("body", "Choice"), ("expression", "Signal")] {
        let context: Value =
            serde_json::from_str(&draft.hole_context(draft.draft_digest(), hole).unwrap()).unwrap();
        let row = context["aggregate_matches"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["target"] == "matching.choice")
            .unwrap();
        assert_eq!(row["binding"], binding);
        assert_eq!(row["generic"], true);
        assert_eq!(row["cases"].as_array().unwrap().len(), 2);
        assert_eq!(row["cases"][0]["target"], "matching.choice.some");
        assert_eq!(
            row["cases"][0]["fields"][0]["target"],
            "matching.choice.some.value"
        );
        assert_eq!(context["source_authority"], false);
        assert_eq!(context["materializable"], false);
    }
    let catalog: Value =
        serde_json::from_str(&base.change_catalog("matching.read").unwrap()).unwrap();
    let prelude = catalog["aggregate_matches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["target"] == "core.option")
        .unwrap();
    assert_eq!(prelude["identity_origin"], "compiler_owned");
    assert_eq!(prelude["compiler_prelude"]["schema"], "semaprax.prelude.v1");
    assert!(prelude["path"].is_null());
    let before = draft.to_json().to_owned();
    let mut invalid = choice(place("item"));
    invalid["arms"].as_array_mut().unwrap().pop();
    assert!(draft
        .fill_hole(draft.draft_digest(), "body", &invalid)
        .is_err());
    assert_eq!(draft.to_json(), before);
    let first = draft
        .fill_hole(draft.draft_digest(), "body", &choice(place("item")))
        .unwrap();
    assert!(first.complete(first.draft_digest()).is_err());
    let done = first
        .fill_hole(first.draft_digest(), "expression", &choice(place("item")))
        .unwrap();
    let candidate = done.complete(done.draft_digest()).unwrap();
    let capsule = candidate.recovery_capsule().unwrap();
    let restored = ProjectCandidate::restore(
        Arc::clone(base.base_revision()),
        base.base_revision().project_revision(),
        capsule.as_bytes(),
    )
    .unwrap();
    assert_eq!(restored.to_json(), candidate.to_json());
    assert_eq!(fixture.bytes(), disk);
}
