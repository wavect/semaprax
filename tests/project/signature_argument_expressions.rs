//! Computed signature arguments: authored evidence, intentionally unrun.
use semaprax::ast::{Expr, ExprKind, Statement, Type};
use semaprax::hir::{ResolvedExprKind, ResolvedStatement};
use semaprax::project::{with_authenticated_project, ProjectCandidate, SemanticChange};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    root: PathBuf,
    module: &'static str,
}
impl Fixture {
    fn directory() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "spx-computed-signature-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        root.canonicalize().unwrap()
    }
    fn scalar() -> Self {
        let root = Self::directory();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v1"
name = "computed-signature"
entry = "computed.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["computed.public"]
tests = ["computed.tests"]
"#,
        )
        .unwrap();
        for (path, text) in [
            (
                "src/core.spx",
                r#"module computed.core;
@id("computed.select") fn select(left: i64, right: i64) -> i64 requires right >= 0 { right }
@id("computed.first") fn first() -> i64 { 20 }
@id("computed.second") fn second() -> i64 { 42 / 1 }
@id("computed.provider-only") fn provider_only(value: i64) -> i64 { value }
@id("computed.local") fn local(spx_sig_stage_0: i64) -> i64 {
    let spx_sig_stage_1 = 2;
    select(spx_sig_stage_0, spx_sig_stage_1)
}
@id("computed.public") fn public_value(value: i64) -> i64 { value }
"#,
            ),
            (
                "src/app.spx",
                r#"module computed.app;
use function @id("computed.select") from computed.core as select;
use function @id("computed.first") from computed.core as first;
use function @id("computed.second") from computed.core as second;
@id("computed.main") fn main() -> i64 { select(first(), second()) }
"#,
            ),
            (
                "src/tests.spx",
                r#"module computed.tests;
use function @id("computed.local") from computed.core as local;
@id("computed.test") fn main() -> i64 { if local(40) == 2 { 0 } else { 1 } }
"#,
            ),
        ] {
            let parsed = semaprax::parse(text, path).unwrap();
            std::fs::write(root.join(path), semaprax::format::canonical(&parsed)).unwrap();
        }
        Self {
            root,
            module: "src/core.spx",
        }
    }
    fn owned() -> Self {
        let root = Self::directory();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/frame-payload-project");
        for path in [
            "semaprax.toml",
            "src/app.spx",
            "src/frame.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(example.join(path), root.join(path)).unwrap();
        }
        let path = root.join("src/frame.spx");
        let source = std::fs::read_to_string(&path).unwrap()
            + r#"
@id("computed.own-select") fn own_select(left: own Bytes, right: own Bytes, flag: i64) -> Bytes { if flag == 0 { left } else { right } }
@id("computed.own-call") fn own_call(input: borrow Slice<u8>) -> Bytes { own_select(bytes_copy(input), bytes_copy(input), 4 / 2) }
@id("computed.consume") fn consume(bytes: own Bytes) -> i64 { 0 }
"#;
        let parsed = semaprax::parse(&source, "src/frame.spx").unwrap();
        std::fs::write(path, semaprax::format::canonical(&parsed)).unwrap();
        Self {
            root,
            module: "src/frame.spx",
        }
    }
    fn candidate(&self) -> ProjectCandidate {
        with_authenticated_project(&self.root.join("semaprax.toml"), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
        })
        .unwrap()
    }
    fn bytes(&self) -> Vec<Vec<u8>> {
        ["semaprax.toml", "src/app.spx", self.module, "src/tests.spx"]
            .iter()
            .map(|path| std::fs::read(self.root.join(path)).unwrap())
            .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
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
fn integer(value: i64) -> Value {
    json!({"kind":"i64","value":value})
}
fn binary(op: &str, left: Value, right: Value) -> Value {
    json!({"kind":"binary","op":op,"left":left,"right":right})
}
fn computed(name: &str, body: Value) -> Value {
    json!({"name":name,"type":"i64","argument_expression":body})
}
fn parameters(local: &str) -> Value {
    json!([
        computed("sum",json!({"kind":"let","name":local,"value":place("left"),"body":binary("+",place(local),place("right"))})),
        {"from":"right","name":"winner"},
        computed("difference",binary("-",place("right"),place("left")))
    ])
}
fn apply(
    base: &ProjectCandidate,
    target: &str,
    parameters: Value,
) -> Result<(ProjectCandidate, SemanticChange), Vec<semaprax::diagnostic::Diagnostic>> {
    let change = SemanticChange::new(
        base.revision().project_revision(),
        &json!({"kind":"change_function_signature","target":target,"parameters":parameters}),
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
        mutable,
        value,
        ..
    } = statement
    else {
        panic!("argument stage missing")
    };
    assert!(!mutable);
    (name, declared, value)
}
fn diagnostic<T>(result: Result<T, Vec<semaprax::diagnostic::Diagnostic>>, code: &str) {
    let errors = result.err().expect("invalid computed migration accepted");
    assert!(errors.iter().any(|error| error.code == code), "{errors:?}");
}

#[test]
fn cross_file_calls_stage_all_original_arguments_before_typed_computed_arguments_in_mapping_order()
{
    let fixture = Fixture::scalar();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let (candidate, change) = apply(&base, "computed.select", parameters("copied")).unwrap();
    let app = semaprax::parse(source(&candidate, "src/app.spx"), "src/app.spx").unwrap();
    let function = app
        .functions
        .iter()
        .find(|function| function.stable_id == "computed.main")
        .unwrap();
    let ExprKind::Block { statements, tail } = &unwrapped(&function.body).kind else {
        panic!("call staging block missing")
    };
    assert_eq!(statements.len(), 4);
    let mut names = Vec::new();
    for (index, statement) in statements.iter().enumerate() {
        let (name, declared, value) = stage(statement);
        names.push(name);
        if index < 2 {
            assert!(declared.is_none());
            let ExprKind::Call { name, args, .. } = &value.kind else {
                panic!("original argument call lost")
            };
            assert_eq!(name, if index == 0 { "first" } else { "second" });
            assert!(args.is_empty());
        } else {
            assert_eq!(declared, &Some(Type::I64));
        }
    }
    let ExprKind::Call { name, args, .. } = &tail.kind else {
        panic!("final call missing")
    };
    assert_eq!(name, "select");
    assert_eq!(
        args.iter()
            .map(|argument| match &argument.kind {
                ExprKind::Var(name) => name.as_str(),
                _ => panic!("final argument not staged"),
            })
            .collect::<Vec<_>>(),
        [names[2], names[1], names[3]]
    );
    let (_, _, sum) = stage(&statements[2]);
    let ExprKind::Block {
        statements: inner,
        tail: sum_body,
    } = &sum.kind
    else {
        panic!("computed let lost")
    };
    let (inner_name, _, value) = stage(&inner[0]);
    assert_eq!(value.kind, ExprKind::Var(names[0].to_owned()));
    let ExprKind::Binary { left, right, .. } = &sum_body.kind else {
        panic!("computed sum lost")
    };
    assert_eq!(left.kind, ExprKind::Var(inner_name.to_owned()));
    assert_eq!(right.kind, ExprKind::Var(names[1].to_owned()));
    let core = source(&candidate, "src/core.spx");
    assert!(core.contains("fn select(sum: i64, winner: i64, difference: i64)"));
    assert!(core.contains("requires winner >= 0"));
    let checked = candidate
        .revision()
        .entry_program()
        .functions
        .iter()
        .find(|function| function.id.as_str() == "computed.main")
        .unwrap();
    let mut body = &checked.body;
    while let ResolvedExprKind::Block { statements, tail } = &body.kind {
        if !statements.is_empty() {
            break;
        }
        body = tail;
    }
    let ResolvedExprKind::Block { statements, tail } = &body.kind else {
        panic!("checked stages lost")
    };
    assert_eq!(statements.len(), 4);
    let ids = statements
        .iter()
        .map(|statement| match statement {
            ResolvedStatement::Let { binding, .. } => binding.id.clone(),
            _ => panic!("not a checked stage"),
        })
        .collect::<Vec<_>>();
    let ResolvedExprKind::Call { callee, args, .. } = &tail.kind else {
        panic!("checked final call missing")
    };
    assert_eq!(callee.as_str(), "computed.select");
    for (argument, index) in args.iter().zip([2, 1, 3]) {
        let ResolvedExprKind::Place(place) = &argument.kind else {
            panic!("checked final argument not bound")
        };
        assert_eq!(place.root, ids[index]);
    }
    replay(&base, &candidate, change);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn computed_let_names_and_existing_caller_locals_do_not_capture_staged_old_parameters() {
    let fixture = Fixture::scalar();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let (candidate, change) =
        apply(&base, "computed.select", parameters("spx_sig_stage_2")).unwrap();
    let parsed = semaprax::parse(source(&candidate, "src/core.spx"), "src/core.spx").unwrap();
    let function = parsed
        .functions
        .iter()
        .find(|function| function.stable_id == "computed.local")
        .unwrap();
    let ExprKind::Block {
        statements: outer,
        tail,
    } = &function.body.kind
    else {
        panic!("caller local missing")
    };
    let (original_name, _, _) = stage(&outer[0]);
    assert_eq!(original_name, "spx_sig_stage_1");
    let ExprKind::Block { statements, .. } = &tail.kind else {
        panic!("nested call staging missing")
    };
    let (left, _, left_value) = stage(&statements[0]);
    let (right, _, right_value) = stage(&statements[1]);
    assert_ne!(left, "spx_sig_stage_0");
    assert_ne!(left, "spx_sig_stage_1");
    assert_ne!(right, "spx_sig_stage_0");
    assert_ne!(right, "spx_sig_stage_1");
    assert_eq!(left_value.kind, ExprKind::Var("spx_sig_stage_0".to_owned()));
    assert_eq!(
        right_value.kind,
        ExprKind::Var("spx_sig_stage_1".to_owned())
    );
    let (_, _, computed) = stage(&statements[2]);
    let ExprKind::Block {
        statements: inner,
        tail,
    } = &computed.kind
    else {
        panic!("computed local missing")
    };
    let (name, _, value) = stage(&inner[0]);
    assert_ne!(name, right);
    assert_eq!(value.kind, ExprKind::Var(left.to_owned()));
    let ExprKind::Binary {
        left: bound,
        right: original,
        ..
    } = &tail.kind
    else {
        panic!("computed body missing")
    };
    assert_eq!(bound.kind, ExprKind::Var(name.to_owned()));
    assert_eq!(original.kind, ExprKind::Var(right.to_owned()));
    replay(&base, &candidate, change);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn original_parameter_scope_rejects_new_names_mixed_forms_and_unbound_caller_functions() {
    let fixture = Fixture::scalar();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let before = base.to_json().to_owned();
    for name in ["unknown", "winner", "sum"] {
        let mut invalid = parameters("copied");
        invalid[0]["argument_expression"] = place(name);
        diagnostic(apply(&base, "computed.select", invalid), "SPX-G225");
    }
    let mut mixed = parameters("copied");
    mixed[0]["argument"] = integer(0);
    diagnostic(apply(&base, "computed.select", mixed), "SPX-G225");
    let mut mixed = parameters("copied");
    mixed[0]["from"] = json!("left");
    diagnostic(apply(&base, "computed.select", mixed), "SPX-G225");
    let mut unsupported = parameters("copied");
    unsupported[0]["type"] = json!("Bytes");
    diagnostic(apply(&base, "computed.select", unsupported), "SPX-G225");
    let mut unbound = parameters("copied");
    unbound[0]["argument_expression"] =
        json!({"kind":"call","target":"computed.provider-only","arguments":[place("left")]});
    diagnostic(apply(&base, "computed.select", unbound), "SPX-G225");
    let mut wrong_type = parameters("copied");
    wrong_type[0]["argument_expression"] = json!({"kind":"bool","value":true});
    assert!(apply(&base, "computed.select", wrong_type).is_err());
    let mut wrong_literal = parameters("copied");
    wrong_literal[0]
        .as_object_mut()
        .unwrap()
        .remove("argument_expression");
    wrong_literal[0]["argument"] =
        json!({"kind":"binary","op":"+","left":integer(1),"right":integer(2)});
    diagnostic(apply(&base, "computed.select", wrong_literal), "SPX-G225");
    assert_eq!(base.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn legacy_literal_and_computed_forms_compose_and_stale_migrations_preserve_source() {
    let fixture = Fixture::scalar();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let mapping = json!([{"from":"right"},{"name":"literal","type":"bool","argument":{"kind":"bool","value":false}},computed("added",binary("+",place("left"),integer(1)))]);
    let (candidate, change) = apply(&base, "computed.select", mapping).unwrap();
    assert!(candidate.apply(base.candidate_digest(), &change).is_err());
    let parsed = semaprax::parse(source(&candidate, "src/core.spx"), "src/core.spx").unwrap();
    let function = parsed
        .functions
        .iter()
        .find(|function| function.stable_id == "computed.select")
        .unwrap();
    assert_eq!(
        function
            .params
            .iter()
            .map(|parameter| parameter.name.as_str())
            .collect::<Vec<_>>(),
        ["right", "literal", "added"]
    );
    assert_eq!(function.params[1].ty, Type::Bool);
    replay(&base, &candidate, change);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn retaining_all_owned_arguments_does_not_allow_computed_expressions_to_consume_them_twice() {
    let fixture = Fixture::owned();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let before = base.to_json().to_owned();
    let mapping =
        json!([{"from":"left"},{"from":"right"},{"from":"flag"},computed("extra",integer(1))]);
    let (candidate, change) = apply(&base, "computed.own-select", mapping).unwrap();
    replay(&base, &candidate, change);
    let duplicated = json!([{"from":"left"},{"from":"right"},{"from":"flag"},computed("extra",json!({"kind":"call","target":"computed.consume","arguments":[place("left")]}))]);
    assert!(apply(&base, "computed.own-select", duplicated).is_err());
    assert_eq!(base.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn zero_caller_targets_preflight_structure_without_claiming_a_computed_argument_was_type_checked() {
    let fixture = Fixture::scalar();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    diagnostic(
        apply(
            &base,
            "computed.provider-only",
            json!([{"from":"value"}, computed("extra", place("unknown"))]),
        ),
        "SPX-G225",
    );
    // There is no call at which this bool expression could initialize an i64
    // argument. Constructor preflight succeeds, but it is not type evidence
    // for a future caller; ordinary admission must check any later migration.
    let (candidate, change) = apply(
        &base,
        "computed.provider-only",
        json!([{"from":"value"}, computed("extra", json!({"kind":"bool","value":true}))]),
    )
    .unwrap();
    let evidence: Value = serde_json::from_str(candidate.to_json()).unwrap();
    assert_eq!(evidence["operations"][0]["migrated_calls"], 0);
    let parsed = semaprax::parse(source(&candidate, "src/core.spx"), "src/core.spx").unwrap();
    let function = parsed
        .functions
        .iter()
        .find(|function| function.stable_id == "computed.provider-only")
        .unwrap();
    assert_eq!(function.params[1].ty, Type::I64);
    let ExprKind::Block { statements, tail } = &function.body.kind else {
        panic!("function body missing")
    };
    assert!(statements.is_empty());
    assert_eq!(tail.kind, ExprKind::Var("value".to_owned()));
    replay(&base, &candidate, change);
    assert_eq!(fixture.bytes(), disk);
}
