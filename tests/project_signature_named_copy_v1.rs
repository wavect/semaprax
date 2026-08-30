//! Named Copy signature evolution evidence, authored and intentionally unrun.
use semaprax::ast::{Expr, ExprKind, Function, ParamMode, Program, Statement, Type};
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
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-named-signature-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "named-signature"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "named.app"
sources = ["src/app.spx", "src/bridge.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["named.evaluate"]
tests = ["named.tests"]
"#,
        )
        .unwrap();
        for (path, source) in [
            (
                "src/core.spx",
                r#"module named.core;
@id("named.pair") record Pair { @id("named.pair.value") value: i64, }
@id("named.tag") variant Tag {
    @id("named.tag.number") Number { @id("named.tag.number.value") value: i64, },
    @id("named.tag.none") None,
}
@id("named.select") fn select(left: Pair, right: Pair, unused: Tag) -> i64
    requires left.value >= 0
    ensures result >= 0
{ left.value + right.value }
@id("named.local") fn local_call() -> i64 {
    select(Pair { value: 8 / 2 }, Pair { value: 18 / 3 }, Tag::Number { value: 20 / 4 })
}
@id("named.variant") fn variant_pick(left: Tag, unused: Tag, ignored: Pair) -> i64 {
    match left { Tag::Number { value: selected } => selected, Tag::None {} => 0 }
}
@id("named.variant-call") fn variant_call() -> i64 {
    variant_pick(Tag::Number { value: 10 / 2 }, Tag::Number { value: 1 / 0 }, Pair { value: 12 / 3 })
}
@id("named.option") fn option_pick(left: Option<i64>, right: Option<i64>) -> i64 {
    match left { Option::Some { value } => value, Option::None {} => 0 }
}
@id("named.option-call") fn option_call() -> i64 {
    option_pick(Option<i64>::Some { value: 8 / 2 }, Option<i64>::Some { value: 1 / 0 })
}
@id("named.evaluate") fn evaluate(input: i64) -> i64 {
    select(Pair { value: input }, Pair { value: 2 }, Tag::None {})
}
"#,
            ),
            (
                "src/bridge.spx",
                r#"module named.bridge;
use type @id("named.pair") from named.core as Metric;
use type @id("named.tag") from named.core as Signal;
use function @id("named.select") from named.core as imported_select;
@id("bridge.pair") record Pair { @id("bridge.pair.flag") flag: bool, }
@id("bridge.same-name") fn same_name(value: Pair) -> bool { value.flag }
@id("bridge.select") fn select(first: Metric, second: Metric, ignored: Signal) -> i64
    requires first.value >= 0
    ensures result >= 0
{ first.value + second.value }
@id("bridge.local") fn local_call() -> i64 {
    select(Metric { value: 6 / 2 }, Metric { value: 16 / 4 }, Signal::None {})
}
@id("bridge.imported") fn imported_call() -> i64 {
    imported_select(Metric { value: 14 / 2 }, Metric { value: 27 / 3 }, Signal::Number { value: 8 / 2 })
}
"#,
            ),
            (
                "src/app.spx",
                r#"module named.app;
use type @id("named.pair") from named.core as Datum;
use type @id("named.tag") from named.core as Choice;
use function @id("bridge.select") from named.bridge as choose;
use function @id("named.evaluate") from named.core as evaluate;
use function @id("named.variant") from named.core as variant_pick;
use function @id("named.option") from named.core as option_pick;
@id("named.main") fn main() -> i64 {
    evaluate(40) + choose(Datum { value: 0 }, Datum { value: 0 }, Choice::None {})
        + variant_pick(Choice::None {}, Choice::None {}, Datum { value: 0 })
        + option_pick(Option<i64>::None {}, Option<i64>::None {})
}
@id("named.app-call") fn app_call() -> i64 {
    choose(Datum { value: 15 / 3 }, Datum { value: 24 / 4 }, Choice::None {})
}
"#,
            ),
            (
                "src/tests.spx",
                r#"module named.tests;
use function @id("named.evaluate") from named.core as evaluate;
@id("named.test") fn main() -> i64 { if evaluate(40) == 42 { 0 } else { 1 } }
"#,
            ),
        ] {
            let program = semaprax::parse(source, path).unwrap();
            std::fs::write(root.join(path), semaprax::format::canonical(&program)).unwrap();
        }
        Self(root)
    }
    fn candidate(&self) -> ProjectCandidate {
        let revision = with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap();
        ProjectCandidate::open(Arc::clone(&revision), revision.project_revision()).unwrap()
    }
    fn append(&self, source: &str) {
        let path = self.0.join("src/core.spx");
        let source = std::fs::read_to_string(&path).unwrap() + source;
        let program = semaprax::parse(&source, "src/core.spx").unwrap();
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
        if !statements.is_empty() {
            break;
        }
        expression = tail;
    }
    expression
}
fn evolve(
    base: &ProjectCandidate,
    target: &str,
    parameters: Value,
) -> Result<(ProjectCandidate, SemanticChange), Vec<Diagnostic>> {
    let change = SemanticChange::new(
        base.revision().project_revision(),
        &json!({"kind":"change_function_signature","target":target,"parameters":parameters}),
    )?;
    let candidate = base.apply(base.candidate_digest(), &change)?;
    Ok((candidate, change))
}
/// Compare the original argument expressions exactly and prove the final call
/// only refers to the staged locals in the requested new order. No evaluation
/// is performed; checked failures remain inside their original initializer.
fn staging(
    before: &ProjectCandidate,
    after: &ProjectCandidate,
    path: &str,
    caller: &str,
    permutation: &[usize],
) {
    let old_program = program(before, path);
    let new_program = program(after, path);
    let ExprKind::Call { name, args, .. } = &tail(&function(&old_program, caller).body).kind else {
        panic!("fixture caller must directly call target")
    };
    let ExprKind::Block {
        statements,
        tail: call,
    } = &tail(&function(&new_program, caller).body).kind
    else {
        panic!("evolved call must stage arguments")
    };
    assert_eq!(statements.len(), args.len());
    let mut names = Vec::new();
    for (statement, original) in statements.iter().zip(args) {
        let Statement::Let {
            name,
            value,
            mutable,
            ..
        } = statement
        else {
            panic!("staging must use a let")
        };
        assert!(!mutable);
        assert_eq!(
            &source(before, path)[original.span.start..original.span.end],
            &source(after, path)[value.span.start..value.span.end]
        );
        names.push(name.to_owned());
    }
    let ExprKind::Call {
        name: after_name,
        args: mapped,
        ..
    } = &call.kind
    else {
        panic!("staging must end with one call")
    };
    assert_eq!(name, after_name);
    assert_eq!(mapped.len(), permutation.len());
    for (argument, index) in mapped.iter().zip(permutation) {
        assert_eq!(&argument.kind, &ExprKind::Var(names[*index].clone()));
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
    for (left, right) in restored
        .revision()
        .sources()
        .iter()
        .zip(candidate.revision().sources())
    {
        assert_eq!(left.source(), right.source());
    }
}

#[test]
fn local_copy_records_reorder_rename_and_remove_with_complete_call_migration() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let (candidate, change) = evolve(
        &base,
        "named.select",
        json!([{"from":"right","name":"second"},{"from":"left","name":"first"}]),
    )
    .unwrap();
    for (path, caller) in [
        ("src/core.spx", "named.local"),
        ("src/core.spx", "named.evaluate"),
        ("src/bridge.spx", "bridge.imported"),
    ] {
        staging(&base, &candidate, path, caller, &[1, 0]);
    }
    let before = program(&base, "src/core.spx");
    let after = program(&candidate, "src/core.spx");
    let old = function(&before, "named.select");
    let new = function(&after, "named.select");
    assert_eq!(
        new.params
            .iter()
            .map(|param| param.name.as_str())
            .collect::<Vec<_>>(),
        ["second", "first"]
    );
    for (param, index) in new.params.iter().zip([1, 0]) {
        assert_eq!(param.ty, old.params[index].ty);
        assert_eq!(param.mode, ParamMode::Value);
    }
    assert_eq!(new.effects, old.effects);
    assert_eq!(new.requires.len(), old.requires.len());
    assert_eq!(new.ensures.len(), old.ensures.len());
    assert!(source(&candidate, "src/core.spx").contains("requires first.value >= 0"));
    assert!(source(&candidate, "src/core.spx").contains("first.value + second.value"));
    replay(&base, &candidate, change);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn imported_type_aliases_keep_nominal_identity_despite_same_display_name_elsewhere() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let (candidate, change) = evolve(
        &base,
        "bridge.select",
        json!([{"from":"second","name":"right"},{"from":"first","name":"left"}]),
    )
    .unwrap();
    staging(&base, &candidate, "src/bridge.spx", "bridge.local", &[1, 0]);
    staging(&base, &candidate, "src/app.spx", "named.app-call", &[1, 0]);
    let projected = program(&candidate, "src/bridge.spx");
    let target = function(&projected, "bridge.select");
    assert!(target
        .params
        .iter()
        .all(|param| param.ty == Type::Named("Metric".to_owned(), vec![])
            && param.mode == ParamMode::Value));
    assert_eq!(
        function(&projected, "bridge.same-name").params[0].ty,
        Type::Named("Pair".to_owned(), vec![])
    );
    let delta: Value = serde_json::from_str(
        &candidate
            .semantic_delta(candidate.candidate_digest(), "bridge.select")
            .unwrap(),
    )
    .unwrap();
    let signature = delta["facets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|facet| facet["facet"] == "signature")
        .unwrap();
    let old = signature["base"].as_array().unwrap();
    let new = signature["candidate"].as_array().unwrap();
    assert_eq!(new[0]["type_id"], old[1]["type_id"]);
    assert_eq!(new[1]["type_id"], old[0]["type_id"]);
    assert!(new[0]["type_id"].as_str().unwrap().contains("named.pair"));
    assert!(!new[0]["type_id"].as_str().unwrap().contains("bridge.pair"));
    assert_eq!(
        source(&candidate, "src/core.spx"),
        source(&base, "src/core.spx")
    );
    replay(&base, &candidate, change);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn copy_variant_removal_keeps_checked_failure_order_and_renames_match_scope() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let (candidate, change) = evolve(
        &base,
        "named.variant",
        json!([{"from":"left","name":"selected"}]),
    )
    .unwrap();
    staging(
        &base,
        &candidate,
        "src/core.spx",
        "named.variant-call",
        &[0],
    );
    let projected = program(&candidate, "src/core.spx");
    let target = function(&projected, "named.variant");
    assert_eq!(target.params.len(), 1);
    assert_eq!(target.params[0].name, "selected");
    assert_eq!(target.params[0].ty, Type::Named("Tag".to_owned(), vec![]));
    let caller = function(&projected, "named.variant-call");
    let ExprKind::Block { statements, .. } = &tail(&caller.body).kind else {
        panic!("staging missing")
    };
    let Statement::Let { value, .. } = &statements[1] else {
        panic!("removed argument was not retained")
    };
    assert!(source(&candidate, "src/core.spx")[value.span.start..value.span.end].contains("1 / 0"));
    // This is structural failure-order evidence, not an executed outcome.
    replay(&base, &candidate, change);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn concrete_copy_option_is_retained_without_generalizing_generic_target_functions() {
    let fixture = Fixture::new();
    fixture.append(
        r#"
@id("named.generic") fn generic<T>(value: T) -> T { value }
"#,
    );
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let (candidate, change) = evolve(
        &base,
        "named.option",
        json!([{"from":"right"},{"from":"left","name":"item"}]),
    )
    .unwrap();
    staging(
        &base,
        &candidate,
        "src/core.spx",
        "named.option-call",
        &[1, 0],
    );
    let projected = program(&candidate, "src/core.spx");
    let old_program = program(&base, "src/core.spx");
    assert_eq!(
        function(&projected, "named.option").params[1].ty,
        function(&old_program, "named.option").params[0].ty
    );
    let generic_errors = evolve(&base, "named.generic", json!([{"from":"value"}]))
        .err()
        .expect("generic target must remain excluded");
    assert!(generic_errors.iter().any(|error| error.code == "SPX-G225"));
    replay(&base, &candidate, change);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn noncopy_nominals_borrow_modes_and_new_aggregate_defaults_remain_closed() {
    let fixture = Fixture::new();
    fixture.append(
        r#"
@id("named.owned") record Owned { @id("named.owned.bytes") bytes: Bytes, }
@id("named.owned-take") fn owned_take(value: own Owned) -> i64 { 1 }
@id("named.borrow-owned") fn borrow_owned(value: borrow Owned) -> i64 { 0 }
@id("named.borrow") fn borrow_bytes(value: borrow Slice<u8>) -> usize { byte_len(value) }
"#,
    );
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let unchanged = base.to_json().to_owned();
    for target in ["named.owned-take", "named.borrow-owned", "named.borrow"] {
        let errors = evolve(&base, target, json!([{"from":"value"}]))
            .err()
            .expect("unsupported mode admitted");
        assert!(
            errors.iter().any(|error| error.code == "SPX-G225"),
            "{errors:?}"
        );
    }
    let guessed = json!([{"from":"left"},{"from":"right"},{"from":"unused"},{"name":"new_record","type":"Pair","argument":{"kind":"i64","value":0}}]);
    assert!(evolve(&base, "named.select", guessed).is_err());
    assert_eq!(base.to_json(), unchanged);
    assert_eq!(fixture.bytes(), disk);
}
