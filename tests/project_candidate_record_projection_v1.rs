//! Stable-ID field projections: authored regressions, intentionally unrun.
use semaprax::ast::{Expr, ExprKind, Function, Program, Statement, Type};
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
            "spx-record-projection-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let root = root.canonicalize().unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "record-projection"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "projection.app"
sources = ["src/app.spx", "src/bridge.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["projection.public"]
tests = ["projection.tests"]
"#,
        )
        .unwrap();
        for (path, source) in [
            (
                "src/core.spx",
                r#"module projection.core;
@id("projection.pair") record Pair { @id("projection.pair.value") value: i64, @id("projection.pair.flag") flag: bool, }
@id("projection.box") record Box<T> { @id("projection.box.value") value: T, }
@id("projection.duo") record Duo<T, U> { @id("projection.duo.left") left: T, @id("projection.duo.right") right: U, }
@id("projection.phantom") record Phantom<T> { @id("projection.phantom.marker") marker: i64, }
@id("projection.choice") variant Choice { @id("projection.choice.some") Some { @id("projection.choice.some.value") value: i64, }, @id("projection.choice.none") None, }
@id("projection.make") fn make(value: i64) -> Pair { Pair { value: value, flag: false } }
@id("projection.read") fn read(value: i64) -> i64 { make(value).value }
@id("projection.read-pair") fn read_pair(pair: Pair) -> i64 { pair.value }
@id("projection.hygiene") fn hygiene(spx_project_0: Pair, spx_project_1: i64) -> i64 { spx_project_0.value + spx_project_1 }
@id("projection.read-box") fn read_box(boxed: Box<i64>) -> i64 { boxed.value }
@id("projection.read-duo") fn read_duo(duo: Duo<i64, bool>) -> i64 { duo.left }
@id("projection.read-phantom") fn read_phantom(phantom: Phantom<i64>) -> i64 { phantom.marker }
@id("projection.public") fn public_value(value: i64) -> i64 { value }
@id("projection.evaluate") fn evaluate(value: i64) -> i64 {
    let pair = make(value);
    let boxed = Box<i64> { value: 0 };
    let duo = Duo<i64, bool> { left: 0, right: false };
    let phantom = Phantom<i64> { marker: 0 };
    read(value) + read_pair(pair) - value + hygiene(pair, 0) - value
        + read_box(boxed) + read_duo(duo) + read_phantom(phantom)
}
"#,
            ),
            (
                "src/bridge.spx",
                r#"module projection.bridge;
use type @id("projection.pair") from projection.core as Metric;
use type @id("projection.box") from projection.core as Wrapped;
@id("bridge.pair") record Pair { @id("bridge.pair.value") value: i64, @id("bridge.pair.flag") flag: bool, }
@id("bridge.read") fn read(pair: Metric) -> i64 { pair.value }
@id("bridge.wrong") fn wrong(pair: Pair) -> i64 { pair.value }
@id("bridge.read-box") fn read_box(boxed: Wrapped<i64>) -> i64 { boxed.value }
@id("bridge.evaluate") fn evaluate(value: i64) -> i64 {
    let pair = Metric { value: value, flag: false };
    let wrong_pair = Pair { value: 0, flag: false };
    let boxed = Wrapped<i64> { value: 0 };
    read(pair) + wrong(wrong_pair) + read_box(boxed)
}
"#,
            ),
            (
                "src/app.spx",
                r#"module projection.app;
use function @id("projection.evaluate") from projection.core as evaluate;
use function @id("bridge.evaluate") from projection.bridge as other;
@id("projection.main") fn main() -> i64 { evaluate(42) + other(0) }
"#,
            ),
            (
                "src/tests.spx",
                r#"module projection.tests;
use function @id("projection.evaluate") from projection.core as evaluate;
@id("projection.test") fn main() -> i64 { if evaluate(42) == 42 { 0 } else { 1 } }
"#,
            ),
        ] {
            let program = semaprax::parse(source, path).unwrap();
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
fn function<'a>(program: &'a Program, target: &str) -> &'a Function {
    program
        .functions
        .iter()
        .find(|function| function.stable_id == target)
        .unwrap()
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
    expression: &'a Expr,
    owner: &str,
    arguments: &[Type],
    field: &str,
) -> (&'a str, &'a Expr) {
    let ExprKind::Block { statements, tail } = &unwrapped(expression).kind else {
        panic!("projection must stage its base")
    };
    assert_eq!(
        statements.len(),
        1,
        "one base initializer, without duplicated evaluation"
    );
    let Statement::Let {
        name,
        mutable,
        declared,
        value,
        ..
    } = &statements[0]
    else {
        panic!("typed immutable let missing")
    };
    assert!(!mutable);
    assert_eq!(
        declared,
        &Some(Type::Named {
            name: owner.to_owned(),
            arguments: arguments.to_vec()
        })
    );
    let ExprKind::Project {
        base,
        field: actual,
        ..
    } = &tail.kind
    else {
        panic!("field projection missing")
    };
    assert_eq!(actual, field);
    assert_eq!(base.kind, ExprKind::Var(name.clone()));
    (name, value)
}
fn place(name: &str) -> Value {
    json!({"kind":"place","name":name})
}
fn project(field: &str, base: Value, arguments: &[&str]) -> Value {
    json!({"kind":"project","target":field,"base":base,"type_arguments":arguments})
}
fn call() -> Value {
    json!({"kind":"call","target":"projection.make","arguments":[place("value")]})
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
    let errors = result.err().expect("unsupported projection admitted");
    assert!(
        errors.iter().any(|error| error.code == "SPX-G225"),
        "{errors:?}"
    );
}
fn descriptor(context: &Value, field: &str, owner: &str, binding: &str) {
    let row = context["aggregate_projections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["target"] == field)
        .unwrap();
    assert_eq!(row["kind"], "project");
    assert_eq!(row["owner"], owner);
    assert_eq!(row["binding"], binding);
    assert_eq!(row["evidence_owner"], "retained_checked_hir");
    assert_eq!(row["base_evaluation"], "once_into_typed_value_binding");
    assert_eq!(row["requires_full_candidate_validation"], true);
}

#[test]
fn call_constructor_and_place_bases_stage_once_with_the_exact_local_or_imported_owner() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    for (target, path, binding, argument) in [
        ("projection.read", "src/core.spx", "Pair", call()),
        (
            "projection.read-pair",
            "src/core.spx",
            "Pair",
            place("pair"),
        ),
        ("bridge.read", "src/bridge.spx", "Metric", place("pair")),
    ] {
        let (candidate, change) = apply(
            &base,
            target,
            project("projection.pair.value", argument, &[]),
        )
        .unwrap();
        let program = program(&candidate, path);
        let (_, initializer) = lowered(&function(&program, target).body, binding, &[], "value");
        if target == "projection.read" {
            let ExprKind::Call { name, args, .. } = &initializer.kind else {
                panic!("call base lost")
            };
            assert_eq!(name, "make");
            assert_eq!(args.len(), 1);
            assert_eq!(args[0].kind, ExprKind::Var("value".to_owned()));
        } else {
            assert_eq!(initializer.kind, ExprKind::Var("pair".to_owned()));
        }
        replay(&base, &candidate, change);
    }
    let constructor = json!({"kind":"record","target":"projection.pair","fields":[
        {"target":"projection.pair.flag","value":{"kind":"bool","value":false}},
        {"target":"projection.pair.value","value":place("value")}
    ]});
    let (candidate, change) = apply(
        &base,
        "projection.read",
        project("projection.pair.value", constructor, &[]),
    )
    .unwrap();
    let program = program(&candidate, "src/core.spx");
    let (_, initializer) = lowered(
        &function(&program, "projection.read").body,
        "Pair",
        &[],
        "value",
    );
    let ExprKind::ConstructRecord { fields, .. } = &initializer.kind else {
        panic!("constructor base lost")
    };
    assert_eq!(
        fields
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        ["flag", "value"]
    );
    replay(&base, &candidate, change);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn generated_temporaries_cannot_capture_places_or_change_sibling_operands() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let body = json!({"kind":"binary","op":"+","left":project("projection.pair.value", place("spx_project_0"), &[]),"right":place("spx_project_1")});
    let (candidate, change) = apply(&base, "projection.hygiene", body).unwrap();
    let program = program(&candidate, "src/core.spx");
    let ExprKind::Binary { left, right, .. } =
        &unwrapped(&function(&program, "projection.hygiene").body).kind
    else {
        panic!("binary context lost")
    };
    let (temporary, initializer) = lowered(left, "Pair", &[], "value");
    assert_ne!(temporary, "spx_project_0");
    assert_ne!(temporary, "spx_project_1");
    assert_eq!(initializer.kind, ExprKind::Var("spx_project_0".to_owned()));
    assert_eq!(right.kind, ExprKind::Var("spx_project_1".to_owned()));
    replay(&base, &candidate, change);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn generic_owner_arguments_and_nominal_identity_are_checked_even_when_field_types_match() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let before = base.to_json().to_owned();
    for (target, path, owner, field, argument, types, expected) in [
        (
            "projection.read-box",
            "src/core.spx",
            "Box",
            "projection.box.value",
            "boxed",
            vec!["i64"],
            vec![Type::I64],
        ),
        (
            "bridge.read-box",
            "src/bridge.spx",
            "Wrapped",
            "projection.box.value",
            "boxed",
            vec!["i64"],
            vec![Type::I64],
        ),
        (
            "projection.read-duo",
            "src/core.spx",
            "Duo",
            "projection.duo.left",
            "duo",
            vec!["i64", "bool"],
            vec![Type::I64, Type::Bool],
        ),
    ] {
        let (candidate, change) =
            apply(&base, target, project(field, place(argument), &types)).unwrap();
        let program = program(&candidate, path);
        let (_, initializer) = lowered(
            &function(&program, target).body,
            owner,
            &expected,
            if owner == "Duo" { "left" } else { "value" },
        );
        assert_eq!(initializer.kind, ExprKind::Var(argument.to_owned()));
        replay(&base, &candidate, change);
    }
    // Both record declarations spell their field `value: i64`; explicit owner
    // annotation must still distinguish local Pair from imported Metric.
    assert!(apply(
        &base,
        "bridge.wrong",
        project("projection.pair.value", place("pair"), &[])
    )
    .is_err());
    // The phantom parameter does not affect marker:i64, but remains nominal.
    assert!(apply(
        &base,
        "projection.read-phantom",
        project("projection.phantom.marker", place("phantom"), &["bool"])
    )
    .is_err());
    assert!(apply(
        &base,
        "projection.read-duo",
        project("projection.duo.left", place("duo"), &["bool", "i64"])
    )
    .is_err());
    assert_eq!(base.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn body_and_expression_holes_discover_projections_and_recover_only_after_typed_completion() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = Arc::new(fixture.candidate());
    let catalog: Value =
        serde_json::from_str(&base.change_catalog("bridge.read").unwrap()).unwrap();
    descriptor(
        &catalog,
        "projection.pair.value",
        "projection.pair",
        "Metric",
    );
    let generic = catalog["aggregate_projections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["target"] == "projection.box.value")
        .unwrap();
    assert_eq!(generic["binding"], "Wrapped");
    assert_eq!(generic["generic"], true);
    assert_eq!(
        generic["type_parameters"][0]["allowed_types"],
        json!(["i64", "bool"])
    );
    assert!(!catalog["aggregate_projections"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["target"] == "projection.choice.some.value"));
    let expressions: Value =
        serde_json::from_str(&base.expression_catalog("bridge.read").unwrap()).unwrap();
    let canonical = source(&base, "src/bridge.spx");
    let selected = expressions["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| {
            let span = &row["source_span"];
            row["replaceable"] == true
                && canonical.get(
                    span["start"].as_u64().unwrap() as usize
                        ..span["end"].as_u64().unwrap() as usize,
                ) == Some("pair.value")
        })
        .unwrap()["expression_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let empty = ProjectCandidateDraft::open(Arc::clone(&base)).unwrap();
    let draft = empty
        .with_body_hole(empty.draft_digest(), "projection.read", "body")
        .unwrap();
    let draft = draft
        .with_expression_hole(draft.draft_digest(), "bridge.read", &selected, "field")
        .unwrap();
    for (hole, binding) in [("body", "Pair"), ("field", "Metric")] {
        let context: Value =
            serde_json::from_str(&draft.hole_context(draft.draft_digest(), hole).unwrap()).unwrap();
        descriptor(
            &context,
            "projection.pair.value",
            "projection.pair",
            binding,
        );
        assert_eq!(context["source_authority"], false);
        assert_eq!(context["materializable"], false);
    }
    let before = draft.to_json().to_owned();
    assert!(draft
        .fill_hole(
            draft.draft_digest(),
            "field",
            &project("projection.pair.flag", place("pair"), &[])
        )
        .is_err());
    assert_eq!(draft.to_json(), before);
    let first = draft
        .fill_hole(
            draft.draft_digest(),
            "body",
            &project("projection.pair.value", call(), &[]),
        )
        .unwrap();
    assert!(first.complete(first.draft_digest()).is_err());
    let done = first
        .fill_hole(
            first.draft_digest(),
            "field",
            &project("projection.pair.value", place("pair"), &[]),
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

#[test]
fn unsupported_field_selectors_type_arguments_and_stale_requests_leave_candidate_and_source_unchanged(
) {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let before = base.to_json().to_owned();
    for target in [
        "missing.field",
        "projection.pair",
        "projection.choice.some.value",
        "core.option.some.value",
    ] {
        grammar(apply(
            &base,
            "projection.read-pair",
            project(target, place("pair"), &[]),
        ));
    }
    for arguments in [vec![], vec!["bool", "i64"], vec!["Bytes"], vec!["Box<i64>"]] {
        grammar(apply(
            &base,
            "projection.read-box",
            project("projection.box.value", place("boxed"), &arguments),
        ));
    }
    let mut omitted = project("projection.box.value", place("boxed"), &[]);
    omitted.as_object_mut().unwrap().remove("type_arguments");
    grammar(apply(&base, "projection.read-box", omitted));
    let mut non_array = project("projection.box.value", place("boxed"), &[]);
    non_array["type_arguments"] = json!("i64");
    grammar(apply(&base, "projection.read-box", non_array));
    let oversized = vec!["i64"; 4096];
    let errors = apply(
        &base,
        "projection.read-box",
        project("projection.box.value", place("boxed"), &oversized),
    )
    .err()
    .unwrap();
    assert!(
        errors.iter().any(|error| error.code == "SPX-G226"),
        "{errors:?}"
    );
    grammar(apply(
        &base,
        "projection.read-pair",
        project("projection.pair.value", place("pair"), &["i64"]),
    ));
    let (candidate, change) = apply(
        &base,
        "projection.read",
        project("projection.pair.value", call(), &[]),
    )
    .unwrap();
    assert!(candidate.apply(base.candidate_digest(), &change).is_err());
    assert_eq!(base.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}
