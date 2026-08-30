//! Generic aggregate intentions, authored and intentionally unrun.
use semaprax::ast::{Expr, ExprKind, Function, Program, Type};
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
            "spx-generic-aggregate-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let root = root.canonicalize().unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "generic-aggregate"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "generic.app"
sources = ["src/app.spx", "src/bridge.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["generic.public"]
tests = ["generic.tests"]
"#,
        )
        .unwrap();
        for (path, source) in [
            (
                "src/core.spx",
                r#"module generic.core;
@id("generic.box") record Box<T> { @id("generic.box.value") value: T, }
@id("generic.duo") record Duo<T, U> { @id("generic.duo.left") left: T, @id("generic.duo.right") right: U, }
@id("generic.phantom") record Phantom<T> { @id("generic.phantom.marker") marker: bool, }
@id("generic.choice") variant Choice<T> {
    @id("generic.choice.value") Value { @id("generic.choice.value.value") value: T, },
    @id("generic.choice.empty") Empty,
}
@id("generic.make-box") fn make_box(value: i64) -> Box<i64> { Box<i64> { value: value } }
@id("generic.make-duo") fn make_duo(number: i64, flag: bool) -> Duo<i64, bool> { Duo<i64, bool> { left: number, right: flag } }
@id("generic.make-phantom") fn make_phantom() -> Phantom<i64> { Phantom<i64> { marker: false } }
@id("generic.make-choice") fn make_choice(value: i64) -> Choice<i64> { Choice<i64>::Value { value: value } }
@id("generic.make-option") fn make_option(value: i64) -> Option<i64> { Option<i64>::Some { value: value } }
@id("generic.make-result") fn make_result(value: i64) -> Result<i64, bool> { Result<i64, bool>::Ok { value: value } }
@id("generic.public") fn public_value(value: i64) -> i64 { value }
@id("generic.evaluate") fn evaluate(input: i64) -> i64 {
    let boxed = make_box(input);
    let duo = make_duo(0, false);
    let phantom = make_phantom();
    let choice = make_choice(0);
    let optional = make_option(0);
    let outcome = make_result(0);
    boxed.value + duo.left + if duo.right || phantom.marker { 1 } else { 0 }
        + match choice { Choice::Value { value } => value, Choice::Empty {} => 0, }
        + match optional { Option::Some { value } => value, Option::None {} => 0, }
        + match outcome { Result::Ok { value } => value, Result::Err { error } => if error { 1 } else { 0 }, }
}
"#,
            ),
            (
                "src/bridge.spx",
                r#"module generic.bridge;
@id("bridge.box") record Wrapped<T> { @id("bridge.box.value") value: T, }
@id("bridge.duo") record Envelope<T, U> { @id("bridge.duo.left") left: T, @id("bridge.duo.right") right: U, }
@id("bridge.choice") variant Signal<T> { @id("bridge.choice.value") Value { @id("bridge.choice.value.value") value: T, }, @id("bridge.choice.empty") Empty, }
@id("bridge.make-box") fn make_box(value: i64) -> Wrapped<i64> { Wrapped<i64> { value: value } }
@id("bridge.make-duo") fn make_duo(flag: bool, number: i64) -> Envelope<bool, i64> { Envelope<bool, i64> { left: flag, right: number } }
@id("bridge.make-choice") fn make_choice(flag: bool) -> Signal<bool> { Signal<bool>::Value { value: flag } }
@id("bridge.evaluate") fn evaluate() -> i64 {
    let boxed = make_box(0);
    let duo = make_duo(false, 0);
    let choice = make_choice(false);
    boxed.value + duo.right + if duo.left { 1 } else { 0 }
        + match choice { Signal::Value { value } => if value { 1 } else { 0 }, Signal::Empty {} => 0, }
}
"#,
            ),
            (
                "src/app.spx",
                r#"module generic.app;
use function @id("generic.evaluate") from generic.core as evaluate;
use function @id("bridge.evaluate") from generic.bridge as other;
@id("generic.main") fn main() -> i64 { evaluate(42) + other() }
"#,
            ),
            (
                "src/tests.spx",
                r#"module generic.tests;
use function @id("generic.evaluate") from generic.core as evaluate;
@id("generic.test") fn main() -> i64 { if evaluate(42) == 42 { 0 } else { 1 } }
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
fn boolean(value: bool) -> Value {
    json!({"kind":"bool","value":value})
}
fn place(name: &str) -> Value {
    json!({"kind":"place","name":name})
}
fn aggregate(kind: &str, target: &str, arguments: &[&str], fields: &[(&str, Value)]) -> Value {
    json!({"kind":kind,"target":target,"type_arguments":arguments,"fields":fields.iter().map(|(target,value)|json!({"target":target,"value":value})).collect::<Vec<_>>()})
}
fn boxed() -> Value {
    aggregate(
        "record",
        "generic.box",
        &["i64"],
        &[("generic.box.value", place("value"))],
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
fn selection(base: &ProjectCandidate, target: &str, path: &str, snippet: &str) -> String {
    let catalog: Value = serde_json::from_str(&base.expression_catalog(target).unwrap()).unwrap();
    let source = source(base, path);
    catalog["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| {
            let span = &row["source_span"];
            row["replaceable"] == true
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
fn grammar<T>(result: Result<T, Vec<Diagnostic>>) {
    let errors = result.err().expect("generic constructor grammar admitted");
    assert!(
        errors.iter().any(|error| error.code == "SPX-G225"),
        "{errors:?}"
    );
}

#[test]
fn generic_type_imports_remain_rejected_without_source_changes() {
    for target in ["generic.box", "generic.duo", "generic.choice"] {
        let fixture = Fixture::new();
        let path = fixture.0.join("src/bridge.spx");
        let source = std::fs::read_to_string(&path).unwrap().replacen(
            "module generic.bridge;",
            &format!(
                "module generic.bridge;\nuse type @id(\"{target}\") from generic.core as Imported;"
            ),
            1,
        );
        let program = semaprax::parse(&source, "src/bridge.spx").unwrap();
        std::fs::write(path, semaprax::format::canonical(&program)).unwrap();
        let before = fixture.bytes();
        let errors = with_authenticated_project(&fixture.0.join("semaprax.toml"), |_| Ok(()))
            .expect_err("generic cross-file imports must retain the linker boundary");
        assert!(
            errors.iter().any(|error| error.code == "SPX-G172"),
            "{errors:?}"
        );
        assert_eq!(fixture.bytes(), before);
    }
}

#[test]
fn module_local_generic_records_preserve_type_argument_and_field_order() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    for (target, path, binding, owner) in [
        ("generic.make-box", "src/core.spx", "Box", "generic.box"),
        ("bridge.make-box", "src/bridge.spx", "Wrapped", "bridge.box"),
    ] {
        let body = aggregate(
            "record",
            owner,
            &["i64"],
            &[(&format!("{owner}.value"), place("value"))],
        );
        let (candidate, change) = apply(&base, target, body).unwrap();
        let projected = program(&candidate, path);
        let ExprKind::ConstructRecord {
            type_name,
            type_arguments,
            fields,
            ..
        } = &tail(&function(&projected, target).body).kind
        else {
            panic!("record missing")
        };
        assert_eq!(type_name, binding);
        assert_eq!(type_arguments, &[Type::I64]);
        assert_eq!(fields[0].name, "value");
        assert_eq!(fields[0].value.kind, ExprKind::Var("value".to_owned()));
        replay(&base, &candidate, change);
    }
    for (target, path, binding, owner, args, expected) in [
        (
            "generic.make-duo",
            "src/core.spx",
            "Duo",
            "generic.duo",
            ["i64", "bool"],
            vec![Type::I64, Type::Bool],
        ),
        (
            "bridge.make-duo",
            "src/bridge.spx",
            "Envelope",
            "bridge.duo",
            ["bool", "i64"],
            vec![Type::Bool, Type::I64],
        ),
    ] {
        let first = if args[0] == "i64" {
            place("number")
        } else {
            place("flag")
        };
        let second = if args[1] == "i64" {
            place("number")
        } else {
            place("flag")
        };
        let body = aggregate(
            "record",
            owner,
            &args,
            &[
                (&format!("{owner}.right"), second),
                (&format!("{owner}.left"), first),
            ],
        );
        let (candidate, change) = apply(&base, target, body).unwrap();
        let projected = program(&candidate, path);
        let ExprKind::ConstructRecord {
            type_name,
            type_arguments,
            fields,
            ..
        } = &tail(&function(&projected, target).body).kind
        else {
            panic!("generic pair missing")
        };
        assert_eq!(type_name, binding);
        assert_eq!(type_arguments, &expected);
        assert_eq!(
            fields
                .iter()
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>(),
            ["right", "left"]
        );
        replay(&base, &candidate, change);
    }
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn phantom_and_reordered_generic_instances_cannot_satisfy_another_expected_identity() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let before = base.to_json().to_owned();
    let phantom = aggregate(
        "record",
        "generic.phantom",
        &["bool"],
        &[("generic.phantom.marker", boolean(false))],
    );
    assert!(apply(&base, "generic.make-phantom", phantom.clone()).is_err());
    let selected = selection(
        &base,
        "generic.make-phantom",
        "src/core.spx",
        "Phantom<i64> { marker: false }",
    );
    let change=SemanticChange::new(base.revision().project_revision(),&json!({"kind":"replace_expression","target":"generic.make-phantom","expression_id":selected,"replacement":phantom})).unwrap();
    assert!(base.apply(base.candidate_digest(), &change).is_err());
    // Every field is individually well typed, but the ordered type identity
    // differs from the function's Duo<i64,bool> result.
    let reversed = aggregate(
        "record",
        "generic.duo",
        &["bool", "i64"],
        &[
            ("generic.duo.left", boolean(true)),
            ("generic.duo.right", integer(7)),
        ],
    );
    assert!(apply(&base, "generic.make-duo", reversed).is_err());
    assert_eq!(base.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn generic_variant_and_compiler_owned_option_result_cases_have_exact_arguments() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    for (target, path, binding, owner, args, value, expected) in [
        (
            "generic.make-choice",
            "src/core.spx",
            "Choice",
            "generic.choice",
            ["i64"],
            integer(7),
            Type::I64,
        ),
        (
            "bridge.make-choice",
            "src/bridge.spx",
            "Signal",
            "bridge.choice",
            ["bool"],
            boolean(true),
            Type::Bool,
        ),
    ] {
        let body = aggregate(
            "variant",
            &format!("{owner}.value"),
            &args,
            &[(&format!("{owner}.value.value"), value)],
        );
        let (candidate, change) = apply(&base, target, body).unwrap();
        let projected = program(&candidate, path);
        let ExprKind::ConstructVariant {
            type_name,
            type_arguments,
            case_name,
            fields,
            ..
        } = &tail(&function(&projected, target).body).kind
        else {
            panic!("generic variant missing")
        };
        assert_eq!(type_name, binding);
        assert_eq!(type_arguments, &[expected]);
        assert_eq!(case_name, "Value");
        assert_eq!(fields[0].name, "value");
        replay(&base, &candidate, change);
        let (empty, change) = apply(
            &base,
            target,
            aggregate("variant", &format!("{owner}.empty"), &args, &[]),
        )
        .unwrap();
        assert!(source(&empty, path).contains(&format!("{binding}<{}>::Empty {{}}", args[0])));
        replay(&base, &empty, change);
    }
    for (target, body, spelling) in [
        (
            "generic.make-option",
            aggregate(
                "variant",
                "core.option.some",
                &["i64"],
                &[("core.option.some.value", integer(9))],
            ),
            "Option<i64>::Some { value: 9 }",
        ),
        (
            "generic.make-option",
            aggregate("variant", "core.option.none", &["i64"], &[]),
            "Option<i64>::None {}",
        ),
        (
            "generic.make-result",
            aggregate(
                "variant",
                "core.result.ok",
                &["i64", "bool"],
                &[("core.result.ok.value", integer(9))],
            ),
            "Result<i64, bool>::Ok { value: 9 }",
        ),
        (
            "generic.make-result",
            aggregate(
                "variant",
                "core.result.err",
                &["i64", "bool"],
                &[("core.result.err.error", boolean(true))],
            ),
            "Result<i64, bool>::Err { error: true }",
        ),
    ] {
        let (candidate, change) = apply(&base, target, body).unwrap();
        assert!(source(&candidate, "src/core.spx").contains(spelling));
        assert!(!source(&candidate, "src/core.spx").contains("variant Option"));
        assert!(!source(&candidate, "src/core.spx").contains("variant Result"));
        replay(&base, &candidate, change);
    }
    // Payload-free cases still carry exact type identity; there is no
    // contextual inference from the expected result after a mismatched request.
    assert!(apply(
        &base,
        "generic.make-option",
        aggregate("variant", "core.option.none", &["bool"], &[])
    )
    .is_err());
    assert!(apply(
        &base,
        "generic.make-result",
        aggregate(
            "variant",
            "core.result.err",
            &["bool", "i64"],
            &[("core.result.err.error", integer(9))]
        )
    )
    .is_err());
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn generic_body_and_expression_holes_recover_only_after_complete_typed_fill() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = Arc::new(fixture.candidate());
    let selected = selection(
        &base,
        "generic.make-option",
        "src/core.spx",
        "Option<i64>::Some { value: value }",
    );
    let empty = ProjectCandidateDraft::open(Arc::clone(&base)).unwrap();
    let draft = empty
        .with_body_hole(empty.draft_digest(), "bridge.make-duo", "duo")
        .unwrap();
    let draft = draft
        .with_expression_hole(
            draft.draft_digest(),
            "generic.make-option",
            &selected,
            "option",
        )
        .unwrap();
    assert!(draft.complete(draft.draft_digest()).is_err());
    let context: Value =
        serde_json::from_str(&draft.hole_context(draft.draft_digest(), "duo").unwrap()).unwrap();
    let descriptor = context["aggregate_constructors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["target"] == "bridge.duo")
        .unwrap();
    assert_eq!(descriptor["generic"], true);
    assert_eq!(descriptor["binding"], "Envelope");
    assert_eq!(descriptor["type_parameters"].as_array().unwrap().len(), 2);
    assert_eq!(descriptor["type_parameters"][0]["index"], 0);
    assert_eq!(descriptor["type_parameters"][1]["index"], 1);
    assert_eq!(
        descriptor["type_parameters"][0]["allowed_types"],
        json!(["i64", "bool"])
    );
    let context: Value =
        serde_json::from_str(&draft.hole_context(draft.draft_digest(), "option").unwrap()).unwrap();
    let descriptor = context["aggregate_constructors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["target"] == "core.option.none")
        .unwrap();
    assert_eq!(descriptor["identity_origin"], "compiler_owned");
    assert!(descriptor["path"].is_null());
    assert!(descriptor["module"].is_null());
    assert_eq!(
        descriptor["compiler_prelude"]["schema"],
        "semaprax.prelude.v1"
    );
    assert!(descriptor["compiler_prelude"]["digest"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
    let before = draft.to_json().to_owned();
    assert!(draft
        .fill_hole(
            draft.draft_digest(),
            "option",
            &aggregate("variant", "core.option.none", &["bool"], &[])
        )
        .is_err());
    assert_eq!(draft.to_json(), before);
    let duo = aggregate(
        "record",
        "bridge.duo",
        &["bool", "i64"],
        &[
            ("bridge.duo.right", place("number")),
            ("bridge.duo.left", place("flag")),
        ],
    );
    let first = draft.fill_hole(draft.draft_digest(), "duo", &duo).unwrap();
    assert!(first.complete(first.draft_digest()).is_err());
    let done = first
        .fill_hole(
            first.draft_digest(),
            "option",
            &aggregate("variant", "core.option.none", &["i64"], &[]),
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
fn omitted_wrong_arity_unsupported_and_mistyped_arguments_are_not_inferred() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let before = base.to_json().to_owned();
    let mut omitted = boxed();
    omitted.as_object_mut().unwrap().remove("type_arguments");
    grammar(apply(&base, "generic.make-box", omitted));
    for arguments in [
        json!([]),
        json!(["i64", "bool"]),
        json!(["u8"]),
        json!(["Bytes"]),
        json!(["Box<i64>"]),
        json!([true]),
        json!([{"kind":"i64"}]),
    ] {
        let mut request = boxed();
        request["type_arguments"] = arguments;
        grammar(apply(&base, "generic.make-box", request));
    }
    let mut missing = aggregate("variant", "core.option.none", &["i64"], &[]);
    missing.as_object_mut().unwrap().remove("type_arguments");
    grammar(apply(&base, "generic.make-option", missing));
    grammar(apply(
        &base,
        "generic.make-result",
        aggregate(
            "variant",
            "core.result.err",
            &["bool"],
            &[("core.result.err.error", boolean(true))],
        ),
    ));
    let wrong_field = aggregate(
        "record",
        "generic.box",
        &["i64"],
        &[("generic.box.value", boolean(true))],
    );
    assert!(apply(&base, "generic.make-box", wrong_field).is_err());
    let mut excessive = boxed();
    excessive["type_arguments"] = json!(vec!["i64"; 4096]);
    let errors = apply(&base, "generic.make-box", excessive)
        .err()
        .expect("type argument capacity must reject before construction");
    assert!(
        errors.iter().any(|error| error.code == "SPX-G226"),
        "{errors:?}"
    );
    assert_eq!(base.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}
