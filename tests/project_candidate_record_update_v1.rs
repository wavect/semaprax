//! Stable-ID record updates: authored regressions, intentionally unrun.
use semaprax::ast::{Expr, ExprKind, FieldInitializer, MatchPattern, Program, Statement, Type};
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
            "spx-record-update-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let root = root.canonicalize().unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "record-update"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "updating.app"
sources = ["src/app.spx", "src/bridge.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["updating.public"]
tests = ["updating.tests"]
"#,
        )
        .unwrap();
        for (path, text) in [
            (
                "src/core.spx",
                r#"module updating.core;
@id("updating.pair") record Pair { @id("updating.pair.x") x: i64, @id("updating.pair.y") y: i64, @id("updating.pair.flag") flag: bool, }
@id("updating.box") record Box<T> { @id("updating.box.value") value: T, }
@id("updating.duo") record Duo<T, U> { @id("updating.duo.left") left: T, @id("updating.duo.right") right: U, }
@id("updating.phantom") record Phantom<T> { @id("updating.phantom.marker") marker: i64, }
@id("updating.make") fn make(value: i64) -> Pair { Pair { x: value, y: 0, flag: false } }
@id("updating.first") fn first(value: i64) -> i64 { value }
@id("updating.second") fn second(value: i64) -> i64 { value / 1 }
@id("updating.edit") fn edit(value: i64) -> Pair { make(value) }
@id("updating.place") fn edit_place(pair: Pair) -> Pair { pair }
@id("updating.hygiene") fn hygiene(spx_project_0: Pair, spx_project_1: i64) -> Pair { spx_project_0 }
@id("updating.box-edit") fn edit_box(boxed: Box<i64>) -> Box<i64> { boxed }
@id("updating.duo-edit") fn edit_duo(duo: Duo<i64, bool>) -> Duo<i64, bool> { duo }
@id("updating.phantom-edit") fn edit_phantom(phantom: Phantom<i64>) -> Phantom<i64> { phantom }
@id("updating.read-pair") fn read_pair(pair: Pair) -> i64 { pair.x }
@id("updating.option-edit") fn edit_option(item: Option<i64>, pair: Pair) -> i64 { read_pair(pair) }
@id("updating.public") fn public_value(value: i64) -> i64 { value }
@id("updating.evaluate") fn evaluate(value: i64) -> i64 {
    let a = edit(value);
    let b = edit_place(make(0));
    let c = hygiene(make(0), 0);
    let boxed = edit_box(Box<i64> { value: 0 });
    let duo = edit_duo(Duo<i64, bool> { left: 0, right: false });
    let phantom = edit_phantom(Phantom<i64> { marker: 0 });
    let d = edit_option(Option<i64>::Some { value: 0 }, make(0));
    a.x + b.x + c.x + boxed.value + duo.left + phantom.marker + d
}
"#,
            ),
            (
                "src/bridge.spx",
                r#"module updating.bridge;
use type @id("updating.pair") from updating.core as Metric;
@id("bridge.wrapped") record Wrapped<T> { @id("bridge.wrapped.value") value: T, }
@id("bridge.pair") record Pair { @id("bridge.pair.x") x: i64, @id("bridge.pair.y") y: i64, @id("bridge.pair.flag") flag: bool, }
@id("bridge.edit") fn edit(pair: Metric) -> Metric { pair }
@id("bridge.wrong") fn wrong(pair: Pair) -> Pair { pair }
@id("bridge.box-edit") fn edit_box(boxed: Wrapped<i64>) -> Wrapped<i64> { boxed }
@id("bridge.evaluate") fn evaluate() -> i64 {
    let a = edit(Metric { x: 0, y: 0, flag: false });
    let b = wrong(Pair { x: 0, y: 0, flag: false });
    let c = edit_box(Wrapped<i64> { value: 0 });
    a.x + b.x + c.value
}
"#,
            ),
            (
                "src/app.spx",
                r#"module updating.app;
use function @id("updating.evaluate") from updating.core as evaluate;
use function @id("bridge.evaluate") from updating.bridge as other;
@id("updating.main") fn main() -> i64 { evaluate(42) + other() }
"#,
            ),
            (
                "src/tests.spx",
                r#"module updating.tests;
use function @id("updating.evaluate") from updating.core as evaluate;
@id("updating.test") fn main() -> i64 { if evaluate(42) == 42 { 0 } else { 1 } }
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
fn lowered<'a>(
    expr: &'a Expr,
    binding: &str,
    arguments: &[Type],
) -> (&'a str, &'a Expr, &'a [FieldInitializer]) {
    let ExprKind::Block { statements, tail } = &unwrapped(expr).kind else {
        panic!("typed base staging missing")
    };
    assert_eq!(statements.len(), 1);
    let Statement::Let {
        name,
        mutable,
        declared,
        value,
        ..
    } = &statements[0]
    else {
        panic!("base must have exactly one initializer")
    };
    assert!(!mutable);
    assert_eq!(
        declared,
        &Some(Type::Named {
            name: binding.to_owned(),
            arguments: arguments.to_vec()
        })
    );
    let ExprKind::UpdateRecord { base, fields } = &tail.kind else {
        panic!("record update missing")
    };
    assert_eq!(base.kind, ExprKind::Var(name.clone()));
    (name, value, fields)
}
fn place(name: &str) -> Value {
    json!({"kind":"place","name":name})
}
fn integer(value: i64) -> Value {
    json!({"kind":"i64","value":value})
}
fn call(target: &str, value: Value) -> Value {
    json!({"kind":"call","target":target,"arguments":[value]})
}
fn update(target: &str, base: Value, args: &[&str], fields: &[(&str, Value)]) -> Value {
    json!({"kind":"update","target":target,"base":base,"type_arguments":args,"fields":fields.iter().map(|(target,value)|json!({"target":target,"value":value})).collect::<Vec<_>>()})
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
    let errors = result.err().expect("invalid record update accepted");
    assert!(
        errors.iter().any(|error| error.code == "SPX-G225"),
        "{errors:?}"
    );
}

#[test]
fn typed_base_evaluates_once_before_replacement_calls_in_supplied_order() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let request = update(
        "updating.pair",
        call("updating.make", place("value")),
        &[],
        &[
            ("updating.pair.y", call("updating.first", place("value"))),
            ("updating.pair.x", call("updating.second", place("value"))),
        ],
    );
    let (candidate, change) = apply(&base, "updating.edit", request).unwrap();
    let rendered = program(&candidate, "src/core.spx");
    let (_, initializer, fields) = lowered(body(&rendered, "updating.edit"), "Pair", &[]);
    let ExprKind::Call { name, args, .. } = &initializer.kind else {
        panic!("original call base lost")
    };
    assert_eq!(name, "make");
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].kind, ExprKind::Var("value".to_owned()));
    assert_eq!(
        fields
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        ["y", "x"]
    );
    for (field, expected) in fields.iter().zip(["first", "second"]) {
        let ExprKind::Call { name, args, .. } = &field.value.kind else {
            panic!("replacement call lost")
        };
        assert_eq!(name, expected);
        assert_eq!(args.len(), 1);
    }
    // Only x/y are replaced. Existing UpdateRecord semantics retain flag from
    // the staged base; the intention must not inject a default for it.
    assert!(!fields.iter().any(|field| field.name == "flag"));
    assert_eq!(rendered.types, program(&base, "src/core.spx").types);
    replay(&base, &candidate, change);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn local_alias_generic_and_empty_updates_preserve_exact_owner_and_subset_shape() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    for (target, path, binding, owner, value, arguments, types, fields) in [
        (
            "updating.place",
            "src/core.spx",
            "Pair",
            "updating.pair",
            "pair",
            vec![],
            vec![],
            vec![("updating.pair.x", integer(5))],
        ),
        (
            "bridge.edit",
            "src/bridge.spx",
            "Metric",
            "updating.pair",
            "pair",
            vec![],
            vec![],
            vec![("updating.pair.x", integer(5))],
        ),
        (
            "updating.box-edit",
            "src/core.spx",
            "Box",
            "updating.box",
            "boxed",
            vec!["i64"],
            vec![Type::I64],
            vec![("updating.box.value", integer(5))],
        ),
        (
            "bridge.box-edit",
            "src/bridge.spx",
            "Wrapped",
            "bridge.wrapped",
            "boxed",
            vec!["i64"],
            vec![Type::I64],
            vec![("bridge.wrapped.value", integer(5))],
        ),
        (
            "updating.duo-edit",
            "src/core.spx",
            "Duo",
            "updating.duo",
            "duo",
            vec!["i64", "bool"],
            vec![Type::I64, Type::Bool],
            vec![("updating.duo.right", json!({"kind":"bool","value":true}))],
        ),
        (
            "updating.place",
            "src/core.spx",
            "Pair",
            "updating.pair",
            "pair",
            vec![],
            vec![],
            vec![],
        ),
    ] {
        let (candidate, change) = apply(
            &base,
            target,
            update(owner, place(value), &arguments, &fields),
        )
        .unwrap();
        let rendered = program(&candidate, path);
        let (_, initializer, replacements) = lowered(body(&rendered, target), binding, &types);
        assert_eq!(initializer.kind, ExprKind::Var(value.to_owned()));
        assert_eq!(replacements.len(), fields.len());
        replay(&base, &candidate, change);
    }
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn scope_hygiene_and_match_payload_bindings_remain_available_inside_updates() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let (candidate, change) = apply(
        &base,
        "updating.hygiene",
        update(
            "updating.pair",
            place("spx_project_0"),
            &[],
            &[("updating.pair.x", place("spx_project_1"))],
        ),
    )
    .unwrap();
    let rendered = program(&candidate, "src/core.spx");
    let (name, initializer, fields) = lowered(body(&rendered, "updating.hygiene"), "Pair", &[]);
    assert_ne!(name, "spx_project_0");
    assert_ne!(name, "spx_project_1");
    assert_eq!(initializer.kind, ExprKind::Var("spx_project_0".to_owned()));
    assert_eq!(
        fields[0].value.kind,
        ExprKind::Var("spx_project_1".to_owned())
    );
    replay(&base, &candidate, change);
    let request = json!({"kind":"match","target":"core.option","type_arguments":["i64"],"value":place("item"),"arms":[
        {"target":"core.option.none","fields":[],"body":call("updating.read-pair",place("pair"))},
        {"target":"core.option.some","fields":[{"target":"core.option.some.value","name":"captured"}],"body":call("updating.read-pair",update("updating.pair",place("pair"),&[],&[("updating.pair.x",place("captured"))]))}
    ]});
    let (candidate, change) = apply(&base, "updating.option-edit", request).unwrap();
    let rendered = program(&candidate, "src/core.spx");
    let ExprKind::Block { statements, tail } =
        &unwrapped(body(&rendered, "updating.option-edit")).kind
    else {
        panic!("match staging lost")
    };
    let Statement::Let {
        name: match_name, ..
    } = &statements[0]
    else {
        panic!("match staging lost")
    };
    let ExprKind::Match { arms, .. } = &tail.kind else {
        panic!("match lost")
    };
    let MatchPattern::Variant {
        fields: bindings, ..
    } = &arms[1].pattern
    else {
        panic!("payload binding lost")
    };
    assert_eq!(bindings[0].binding, "captured");
    let ExprKind::Call { name, args, .. } = &arms[1].value.kind else {
        panic!("scalar match result call missing")
    };
    assert_eq!(name, "read_pair");
    assert_eq!(args.len(), 1);
    let (update_name, initializer, fields) = lowered(&args[0], "Pair", &[]);
    assert_ne!(update_name, match_name);
    assert_eq!(initializer.kind, ExprKind::Var("pair".to_owned()));
    assert_eq!(fields[0].value.kind, ExprKind::Var("captured".to_owned()));
    replay(&base, &candidate, change);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn exact_member_identity_owner_arguments_types_bounds_and_stale_handles_fail_closed() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let before = base.to_json().to_owned();
    for owner in ["missing.record", "updating.pair.x", "core.option"] {
        grammar(apply(
            &base,
            "updating.place",
            update(owner, place("pair"), &[], &[]),
        ));
    }
    for field in ["missing.field", "bridge.pair.x", "core.option.some.value"] {
        grammar(apply(
            &base,
            "updating.place",
            update("updating.pair", place("pair"), &[], &[(field, integer(1))]),
        ));
    }
    grammar(apply(
        &base,
        "updating.place",
        update(
            "updating.pair",
            place("pair"),
            &[],
            &[
                ("updating.pair.x", integer(1)),
                ("updating.pair.x", integer(2)),
            ],
        ),
    ));
    assert!(apply(
        &base,
        "bridge.wrong",
        update(
            "updating.pair",
            place("pair"),
            &[],
            &[("updating.pair.x", integer(1))]
        )
    )
    .is_err());
    assert!(apply(
        &base,
        "updating.phantom-edit",
        update(
            "updating.phantom",
            place("phantom"),
            &["bool"],
            &[("updating.phantom.marker", integer(1))]
        )
    )
    .is_err());
    assert!(apply(
        &base,
        "updating.duo-edit",
        update("updating.duo", place("duo"), &["bool", "i64"], &[])
    )
    .is_err());
    assert!(apply(
        &base,
        "updating.place",
        update(
            "updating.pair",
            place("pair"),
            &[],
            &[("updating.pair.flag", integer(1))]
        )
    )
    .is_err());
    for arguments in [vec![], vec!["i64", "bool"], vec!["Bytes"]] {
        grammar(apply(
            &base,
            "updating.box-edit",
            update("updating.box", place("boxed"), &arguments, &[]),
        ));
    }
    let mut missing = update("updating.box", place("boxed"), &[], &[]);
    missing.as_object_mut().unwrap().remove("type_arguments");
    grammar(apply(&base, "updating.box-edit", missing));
    // Null placeholders stay below the outer JSON-node cap; the constructor
    // checks field count before attempting to decode individual field objects.
    let mut oversized = update("updating.pair", place("pair"), &[], &[]);
    oversized["fields"] = Value::Array(vec![Value::Null; 4096]);
    let errors = apply(&base, "updating.place", oversized).err().unwrap();
    assert!(
        errors.iter().any(|error| error.code == "SPX-G226"),
        "{errors:?}"
    );
    let (candidate, change) = apply(
        &base,
        "updating.place",
        update(
            "updating.pair",
            place("pair"),
            &[],
            &[("updating.pair.x", integer(5))],
        ),
    )
    .unwrap();
    assert!(candidate.apply(base.candidate_digest(), &change).is_err());
    assert_eq!(base.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn update_catalog_and_body_expression_holes_preserve_revision_bound_recovery() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = Arc::new(fixture.candidate());
    let catalog: Value =
        serde_json::from_str(&base.change_catalog("bridge.edit").unwrap()).unwrap();
    let row = catalog["aggregate_updates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["target"] == "updating.pair")
        .unwrap();
    assert_eq!(row["kind"], "update");
    assert_eq!(row["binding"], "Metric");
    assert_eq!(row["field_coverage"], "subset");
    assert_eq!(row["base_evaluation"], "once_into_typed_value_binding");
    assert_eq!(row["fields"][0]["target"], "updating.pair.x");
    assert!(!catalog["aggregate_updates"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["target"] == "core.option"));
    let expressions: Value =
        serde_json::from_str(&base.expression_catalog("bridge.edit").unwrap()).unwrap();
    let text = source(&base, "src/bridge.spx");
    let selected = expressions["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| {
            let span = &row["source_span"];
            row["replaceable"] == true
                && text.get(
                    span["start"].as_u64().unwrap() as usize
                        ..span["end"].as_u64().unwrap() as usize,
                ) == Some("pair")
        })
        .unwrap()["expression_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let empty = ProjectCandidateDraft::open(Arc::clone(&base)).unwrap();
    let draft = empty
        .with_body_hole(empty.draft_digest(), "updating.edit", "body")
        .unwrap();
    let draft = draft
        .with_expression_hole(draft.draft_digest(), "bridge.edit", &selected, "expression")
        .unwrap();
    for (hole, binding) in [("body", "Pair"), ("expression", "Metric")] {
        let context: Value =
            serde_json::from_str(&draft.hole_context(draft.draft_digest(), hole).unwrap()).unwrap();
        let row = context["aggregate_updates"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["target"] == "updating.pair")
            .unwrap();
        assert_eq!(row["binding"], binding);
        assert_eq!(row["field_coverage"], "subset");
        assert_eq!(context["source_authority"], false);
        assert_eq!(context["materializable"], false);
    }
    let before = draft.to_json().to_owned();
    assert!(draft
        .fill_hole(
            draft.draft_digest(),
            "expression",
            &update(
                "updating.pair",
                place("pair"),
                &[],
                &[("updating.pair.flag", integer(1))]
            )
        )
        .is_err());
    assert_eq!(draft.to_json(), before);
    let first = draft
        .fill_hole(
            draft.draft_digest(),
            "body",
            &update(
                "updating.pair",
                call("updating.make", place("value")),
                &[],
                &[("updating.pair.x", integer(5))],
            ),
        )
        .unwrap();
    assert!(first.complete(first.draft_digest()).is_err());
    let done = first
        .fill_hole(
            first.draft_digest(),
            "expression",
            &update(
                "updating.pair",
                place("pair"),
                &[],
                &[("updating.pair.y", integer(6))],
            ),
        )
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
