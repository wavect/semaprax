//! Nominal computed signature arguments: authored regressions, intentionally unrun.
use semaprax::ast::{Expr, ExprKind, Statement, Type};
use semaprax::diagnostic::Diagnostic;
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
    fn new(app_type_binding: bool) -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-nominal-arguments-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let root = root.canonicalize().unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "nominal-arguments"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "argument.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["argument.public"]
tests = ["argument.tests"]
"#,
        )
        .unwrap();
        let app = format!(
            r#"module argument.app;
{}
use function @id("argument.select") from argument.core as choose;
use function @id("argument.first") from argument.core as first;
use function @id("argument.second") from argument.core as second;
@id("argument.main") fn main() -> i64 {{ choose(first(), second()) }}
"#,
            if app_type_binding {
                "use type @id(\"argument.config\") from argument.core as Settings;"
            } else {
                ""
            }
        );
        for (path, text) in [
            (
                "src/core.spx",
                r#"module argument.core;
@id("argument.config") record Config { @id("argument.config.amount") amount: i64, }
@id("argument.other") record Other { @id("argument.other.amount") amount: i64, }
@id("argument.box") record Box<T> { @id("argument.box.value") value: T, }
@id("argument.owned") record Owned { @id("argument.owned.bytes") bytes: Bytes, }
@id("argument.select") fn select(left: i64, right: i64) -> i64 requires right >= 0 { right }
@id("argument.first") fn first() -> i64 { 20 }
@id("argument.second") fn second() -> i64 { 42 / 1 }
@id("argument.local") fn local_select(left: i64, right: i64) -> i64 { right }
@id("argument.local-call") fn local_call(value: i64) -> i64 { local_select(value, 0) }
@id("argument.unused") fn unused(left: i64, right: i64) -> i64 { right }
@id("argument.public") fn public_value(value: i64) -> i64 { value }
"#,
            ),
            ("src/app.spx", app.as_str()),
            (
                "src/tests.spx",
                r#"module argument.tests;
use type @id("argument.config") from argument.core as TestConfig;
use function @id("argument.select") from argument.core as checked_select;
@id("argument.test") fn main() -> i64 { checked_select(40, 0) }
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
            "src/core.spx",
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
fn place(name: &str) -> Value {
    json!({"kind":"place","name":name})
}
fn nominal(target: &str, args: &[&str]) -> Value {
    json!({"kind":"nominal","target":target,"type_arguments":args})
}
fn record(target: &str, field: &str, args: &[&str], value: Value) -> Value {
    json!({"kind":"record","target":target,"type_arguments":args,"fields":[{"target":field,"value":value}]})
}
fn config() -> Value {
    record(
        "argument.config",
        "argument.config.amount",
        &[],
        place("left"),
    )
}
fn parameters(ty: Value, expression: Value) -> Value {
    json!([{"name":"config","type":ty,"argument_expression":expression},{"from":"right","name":"winner"}])
}
fn apply(
    base: &ProjectCandidate,
    target: &str,
    parameters: Value,
) -> Result<(ProjectCandidate, SemanticChange), Vec<Diagnostic>> {
    let change = SemanticChange::new(
        base.revision().project_revision(),
        &json!({"kind":"change_function_signature","target":target,"parameters":parameters}),
    )?;
    Ok((base.apply(base.candidate_digest(), &change)?, change))
}
fn replay(base: &ProjectCandidate, candidate: &ProjectCandidate, change: SemanticChange) {
    let replayed = ProjectCandidate::replay(
        Arc::clone(base.base_revision()),
        base.base_revision().project_revision(),
        &[change],
        candidate.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(replayed.to_json(), candidate.to_json());
    let capsule = candidate.recovery_capsule().unwrap();
    let restored = ProjectCandidate::restore(
        Arc::clone(base.base_revision()),
        base.base_revision().project_revision(),
        capsule.as_bytes(),
    )
    .unwrap();
    assert_eq!(restored.to_json(), candidate.to_json());
}
fn unwrapped(mut expression: &Expr) -> &Expr {
    while let ExprKind::Block { statements, tail } = &expression.kind {
        if !statements.is_empty() {
            break;
        }
        expression = tail;
    }
    expression
}
fn stage(statement: &Statement) -> (&str, &Option<Type>, &Expr) {
    let Statement::Let {
        name,
        declared,
        value,
        mutable,
        ..
    } = statement
    else {
        panic!("missing argument stage")
    };
    assert!(!mutable);
    (name, declared, value)
}
fn grammar<T>(result: Result<T, Vec<Diagnostic>>) {
    let errors = result.err().expect("invalid nominal migration accepted");
    assert!(errors.iter().any(|e| e.code == "SPX-G225"), "{errors:?}");
}

#[test]
fn removed_scalar_computes_nominal_argument_with_each_callers_own_type_alias() {
    let fixture = Fixture::new(true);
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let (candidate, change) = apply(
        &base,
        "argument.select",
        parameters(nominal("argument.config", &[]), config()),
    )
    .unwrap();
    let provider = semaprax::parse(source(&candidate, "src/core.spx"), "src/core.spx").unwrap();
    let select = provider
        .functions
        .iter()
        .find(|f| f.stable_id == "argument.select")
        .unwrap();
    assert_eq!(
        select.params[0].ty,
        Type::Named {
            name: "Config".into(),
            arguments: vec![]
        }
    );
    assert_eq!(select.params[1].name, "winner");
    assert!(source(&candidate, "src/core.spx").contains("requires winner >= 0"));
    for (path, id, alias) in [
        ("src/app.spx", "argument.main", "Settings"),
        ("src/tests.spx", "argument.test", "TestConfig"),
    ] {
        let program = semaprax::parse(source(&candidate, path), path).unwrap();
        let function = program
            .functions
            .iter()
            .find(|f| f.stable_id == id)
            .unwrap();
        let ExprKind::Block { statements, tail } = &unwrapped(&function.body).kind else {
            panic!("missing caller staging")
        };
        assert_eq!(statements.len(), 3);
        let (left, left_type, left_value) = stage(&statements[0]);
        let (right, right_type, right_value) = stage(&statements[1]);
        assert!(left_type.is_none() && right_type.is_none());
        if id == "argument.main" {
            for (value, expected) in [(left_value, "first"), (right_value, "second")] {
                let ExprKind::Call { name, args, .. } = &value.kind else {
                    panic!("original call missing")
                };
                assert_eq!(name, expected);
                assert!(args.is_empty());
            }
        }
        let (computed, ty, value) = stage(&statements[2]);
        assert_eq!(
            ty,
            &Some(Type::Named {
                name: alias.into(),
                arguments: vec![]
            })
        );
        let ExprKind::ConstructRecord {
            type_name, fields, ..
        } = &value.kind
        else {
            panic!("computed constructor missing")
        };
        assert_eq!(type_name, alias);
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].value.kind, ExprKind::Var(left.to_owned()));
        let ExprKind::Call { args, .. } = &tail.kind else {
            panic!("final call missing")
        };
        assert_eq!(args.len(), 2);
        assert_eq!(args[0].kind, ExprKind::Var(computed.to_owned()));
        assert_eq!(args[1].kind, ExprKind::Var(right.to_owned()));
    }
    replay(&base, &candidate, change.clone());
    assert!(candidate.apply(base.candidate_digest(), &change).is_err());
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn module_local_generic_and_authenticated_prelude_nominals_are_computed_without_generic_imports() {
    let fixture = Fixture::new(true);
    let disk = fixture.bytes();
    let base = fixture.candidate();
    for (target, args, expression, expected_name) in [
        (
            "argument.box",
            vec!["i64"],
            record(
                "argument.box",
                "argument.box.value",
                &["i64"],
                place("left"),
            ),
            "Box",
        ),
        (
            "core.option",
            vec!["i64"],
            json!({"kind":"variant","target":"core.option.some","type_arguments":["i64"],"fields":[{"target":"core.option.some.value","value":place("left")}]}),
            "Option",
        ),
        (
            "core.result",
            vec!["i64", "bool"],
            json!({"kind":"variant","target":"core.result.ok","type_arguments":["i64","bool"],"fields":[{"target":"core.result.ok.value","value":place("left")}]}),
            "Result",
        ),
    ] {
        let (candidate, change) = apply(
            &base,
            "argument.local",
            parameters(nominal(target, &args), expression),
        )
        .unwrap();
        let program = semaprax::parse(source(&candidate, "src/core.spx"), "src/core.spx").unwrap();
        let function = program
            .functions
            .iter()
            .find(|f| f.stable_id == "argument.local")
            .unwrap();
        assert_eq!(
            function.params[0].ty,
            Type::Named {
                name: expected_name.into(),
                arguments: args
                    .iter()
                    .map(|a| if *a == "i64" { Type::I64 } else { Type::Bool })
                    .collect()
            }
        );
        assert_eq!(
            source(&candidate, "src/app.spx"),
            source(&base, "src/app.spx")
        );
        assert_eq!(
            source(&candidate, "src/tests.spx"),
            source(&base, "src/tests.spx")
        );
        replay(&base, &candidate, change);
    }
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn nominal_identity_argument_types_and_closed_descriptor_are_not_inferred_from_display_shape() {
    let fixture = Fixture::new(true);
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let before = base.to_json().to_owned();
    for bad_type in [
        json!({"kind":"nominal","target":"argument.box"}),
        nominal("argument.box", &[]),
        nominal("argument.config.amount", &[]),
        nominal("argument.box", &["Bytes"]),
        json!({"kind":"nominal","target":"argument.config","type_arguments":[],"name":"Config"}),
    ] {
        grammar(apply(
            &base,
            "argument.local",
            parameters(bad_type, config()),
        ));
    }
    for (ty, expression) in [
        (
            nominal("argument.config", &[]),
            record(
                "argument.other",
                "argument.other.amount",
                &[],
                place("left"),
            ),
        ),
        (
            nominal("argument.box", &["i64"]),
            record(
                "argument.box",
                "argument.box.value",
                &["bool"],
                json!({"kind":"bool","value":true}),
            ),
        ),
        (
            nominal("core.option", &["i64"]),
            json!({"kind":"variant","target":"core.option.none","type_arguments":["bool"],"fields":[]}),
        ),
        (
            nominal("argument.config", &[]),
            json!({"kind":"bool","value":true}),
        ),
    ] {
        assert!(apply(&base, "argument.local", parameters(ty, expression)).is_err());
    }
    assert_eq!(base.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn every_affected_caller_must_already_have_the_nominal_type_binding() {
    let fixture = Fixture::new(false);
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let before = base.to_json().to_owned();
    grammar(apply(
        &base,
        "argument.select",
        parameters(nominal("argument.config", &[]), config()),
    ));
    assert_eq!(base.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn zero_callers_still_require_checked_copy_nominal_signature_but_do_not_execute_or_typecheck_unused_template(
) {
    let fixture = Fixture::new(true);
    let disk = fixture.bytes();
    let base = fixture.candidate();
    // A structurally valid, unused bool expression is not an instance of Box<i64>.
    // This succeeds only because there is no call to materialize the template;
    // the newly declared Box<i64> parameter itself must still be checked Copy.
    let (candidate, change) = apply(
        &base,
        "argument.unused",
        parameters(
            nominal("argument.box", &["i64"]),
            json!({"kind":"bool","value":true}),
        ),
    )
    .unwrap();
    let report: Value = serde_json::from_str(candidate.to_json()).unwrap();
    assert_eq!(report["operations"][0]["migrated_calls"], 0);
    let program = semaprax::parse(source(&candidate, "src/core.spx"), "src/core.spx").unwrap();
    let function = program
        .functions
        .iter()
        .find(|f| f.stable_id == "argument.unused")
        .unwrap();
    assert_eq!(
        function.params[0].ty,
        Type::Named {
            name: "Box".into(),
            arguments: vec![Type::I64]
        }
    );
    assert!(!source(&candidate, "src/core.spx").contains("spx_sig_stage_"));
    replay(&base, &candidate, change);
    assert!(apply(
        &base,
        "argument.unused",
        parameters(
            nominal("argument.owned", &[]),
            json!({"kind":"bool","value":true})
        )
    )
    .is_err());
    grammar(apply(
        &base,
        "argument.unused",
        parameters(nominal("argument.box", &["i64"]), place("unknown")),
    ));
    assert_eq!(fixture.bytes(), disk);
}
