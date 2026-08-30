//! Typed lexical bindings: authored integration evidence, intentionally unrun.
use semaprax::ast::{Expr, ExprKind, Statement};
use semaprax::hir::{ResolvedExprKind, ResolvedStatement};
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectCandidateDraft, SemanticChange,
};
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
            "spx-lexical-binding-{}-{}",
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
            r#"schema = "semaprax.project.v8"
name = "lexical-binding"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "binding.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["binding.public"]
tests = ["binding.tests"]
"#,
        )
        .unwrap();
        for (path, text) in [
            (
                "src/core.spx",
                r#"module binding.core;
@id("binding.pair") record Pair { @id("binding.pair.value") value: i64, }
@id("binding.multiply") fn multiply(left: i64, right: i64) -> i64 { left * right }
@id("binding.add") fn add(left: i64, right: i64) -> i64 { left + right }
@id("binding.record") fn record_value(value: i64) -> i64 { value }
@id("binding.public") fn public_value(value: i64) -> i64 { value }
"#,
            ),
            (
                "src/app.spx",
                r#"module binding.app;
use function @id("binding.add") from binding.core as add;
@id("binding.main") fn main() -> i64 { add(20, 22) }
"#,
            ),
            (
                "src/tests.spx",
                r#"module binding.tests;
use function @id("binding.add") from binding.core as add;
@id("binding.test") fn main() -> i64 { if add(20, 22) == 42 { 0 } else { 1 } }
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
@id("binding.owned-wrapper") fn owned_wrapper(frame: borrow Slice<u8>) -> Bytes { payload(frame) }
@id("binding.owned-forward") fn owned_forward(bytes: own Bytes) -> Bytes { bytes }
@id("binding.take-two") fn take_two(first: own Bytes, second: own Bytes) -> Bytes { first }
"#;
        let parsed = semaprax::parse(&source, "src/frame.spx").unwrap();
        std::fs::write(path, semaprax::format::canonical(&parsed)).unwrap();
        Self {
            root,
            module: "src/frame.spx",
        }
    }
    fn candidate(&self) -> Arc<ProjectCandidate> {
        with_authenticated_project(&self.root.join("semaprax.toml"), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
                .map(Arc::new)
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
fn binding(name: &str, value: Value, body: Value) -> Value {
    json!({"kind":"let","name":name,"value":value,"body":body})
}
fn plus(left: Value, right: Value) -> Value {
    json!({"kind":"binary","op":"+","left":left,"right":right})
}
fn call(target: &str, arguments: Vec<Value>) -> Value {
    json!({"kind":"call","target":target,"arguments":arguments})
}
fn attempt(
    base: &ProjectCandidate,
    target: &str,
    body: Value,
) -> Result<(ProjectCandidate, SemanticChange), Vec<semaprax::diagnostic::Diagnostic>> {
    let change = SemanticChange::new(
        base.revision().project_revision(),
        &json!({"kind":"replace_function_body","target":target,"body":body}),
    )?;
    Ok((base.apply(base.candidate_digest(), &change)?, change))
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
fn let_parts(expr: &Expr) -> (&str, &Expr, &Expr) {
    let ExprKind::Block { statements, tail } = &unwrapped(expr).kind else {
        panic!("lexical block missing")
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
        panic!("let missing")
    };
    assert!(!mutable);
    assert!(declared.is_none());
    (name, value, tail)
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
fn grammar<T>(result: Result<T, Vec<semaprax::diagnostic::Diagnostic>>) {
    let errors = result.err().expect("invalid lexical binding accepted");
    assert!(
        errors.iter().any(|error| error.code == "SPX-G225"),
        "{errors:?}"
    );
}
fn selection(candidate: &ProjectCandidate, target: &str, snippet: &str) -> String {
    let catalog: Value =
        serde_json::from_str(&candidate.expression_catalog(target).unwrap()).unwrap();
    let text = source(candidate, "src/core.spx");
    catalog["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| {
            let span = &row["source_span"];
            row["replaceable"] == true
                && text.get(
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
fn one_call_initializer_feeds_two_uses_of_the_same_checked_value_identity() {
    let fixture = Fixture::scalar();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let (candidate, change) = attempt(
        &base,
        "binding.add",
        binding(
            "once",
            call("binding.multiply", vec![place("left"), place("right")]),
            plus(place("once"), place("once")),
        ),
    )
    .unwrap();
    let parsed = semaprax::parse(source(&candidate, "src/core.spx"), "src/core.spx").unwrap();
    let function = parsed
        .functions
        .iter()
        .find(|function| function.stable_id == "binding.add")
        .unwrap();
    let (name, initializer, tail) = let_parts(&function.body);
    assert_eq!(name, "once");
    let ExprKind::Call { name, args, .. } = &initializer.kind else {
        panic!("single initializer call missing")
    };
    assert_eq!(name, "multiply");
    assert_eq!(args.len(), 2);
    assert!(matches!(tail.kind, ExprKind::Binary { .. }));
    let checked = candidate
        .revision()
        .entry_program()
        .functions
        .iter()
        .find(|function| function.id.as_str() == "binding.add")
        .unwrap();
    let mut root = &checked.body;
    while let ResolvedExprKind::Block { statements, tail } = &root.kind {
        if !statements.is_empty() {
            break;
        }
        root = tail;
    }
    let ResolvedExprKind::Block { statements, tail } = &root.kind else {
        panic!("checked let block missing")
    };
    assert_eq!(statements.len(), 1);
    let ResolvedStatement::Let {
        binding,
        value,
        mutable,
        ..
    } = &statements[0]
    else {
        panic!("checked binding missing")
    };
    assert!(!mutable);
    let ResolvedExprKind::Call { callee, args, .. } = &value.kind else {
        panic!("checked call missing")
    };
    assert_eq!(callee.as_str(), "binding.multiply");
    assert_eq!(args.len(), 2);
    let ResolvedExprKind::Binary { left, right, .. } = &tail.kind else {
        panic!("two checked uses missing")
    };
    for operand in [left, right] {
        let ResolvedExprKind::Place(place) = &operand.kind else {
            panic!("use must refer to the bound value")
        };
        assert_eq!(place.root, binding.id);
        assert!(place.projections.is_empty());
    }
    replay(&base, &candidate, change);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn nested_record_projection_and_match_values_compose_with_lexical_scope_and_replay() {
    let fixture = Fixture::scalar();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let record = json!({"kind":"record","target":"binding.pair","fields":[{"target":"binding.pair.value","value":place("value")}]});
    let projection =
        json!({"kind":"project","target":"binding.pair.value","base":place("spx_project_0")});
    let record_body = binding(
        "spx_project_0",
        record,
        plus(projection.clone(), projection),
    );
    let matched = json!({"kind":"match","target":"core.option","type_arguments":["i64"],"value":{"kind":"variant","target":"core.option.some","type_arguments":["i64"],"fields":[{"target":"core.option.some.value","value":place("value")}]},"arms":[{"target":"core.option.none","fields":[],"body":integer(0)},{"target":"core.option.some","fields":[{"target":"core.option.some.value","name":"picked"}],"body":place("picked")}]});
    for body in [
        record_body,
        binding("matched", matched, plus(place("matched"), place("matched"))),
    ] {
        let (candidate, change) = attempt(&base, "binding.record", body).unwrap();
        replay(&base, &candidate, change);
    }
    let sibling_body = plus(
        binding("same", integer(1), place("same")),
        binding("same", integer(2), place("same")),
    );
    let (candidate, change) = attempt(&base, "binding.record", sibling_body).unwrap();
    replay(&base, &candidate, change);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn self_reference_scope_escape_shadowing_and_invalid_shapes_fail_before_mutation() {
    let fixture = Fixture::scalar();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let before = base.to_json().to_owned();
    grammar(attempt(
        &base,
        "binding.add",
        binding("new_value", place("new_value"), place("new_value")),
    ));
    grammar(attempt(
        &base,
        "binding.add",
        plus(
            binding("scoped", integer(1), place("scoped")),
            place("scoped"),
        ),
    ));
    grammar(attempt(
        &base,
        "binding.add",
        binding(
            "outer",
            integer(1),
            binding("outer", integer(2), place("outer")),
        ),
    ));
    for name in ["left", "multiply", "Pair", "_", "fn", "bad-name"] {
        grammar(attempt(
            &base,
            "binding.add",
            binding(name, integer(1), integer(2)),
        ));
    }
    let match_shadow = json!({"kind":"match","target":"core.option","type_arguments":["i64"],
        "value":{"kind":"variant","target":"core.option.some","type_arguments":["i64"],"fields":[{"target":"core.option.some.value","value":integer(1)}]},
        "arms":[{"target":"core.option.none","fields":[],"body":integer(0)},
        {"target":"core.option.some","fields":[{"target":"core.option.some.value","name":"picked"}],"body":binding("picked",integer(2),place("picked"))}]});
    grammar(attempt(&base, "binding.add", match_shadow));
    let mut extra = binding("new_value", integer(1), place("new_value"));
    extra["type"] = json!("i64");
    grammar(attempt(&base, "binding.add", extra));
    let mut mutable = binding("new_value", integer(1), place("new_value"));
    mutable["mutable"] = json!(true);
    grammar(attempt(&base, "binding.add", mutable));
    assert!(attempt(
        &base,
        "binding.add",
        binding(
            "flag",
            json!({"kind":"bool","value":true}),
            plus(place("flag"), integer(1))
        )
    )
    .is_err());
    // Projection staging contributes two AST levels per wire node. This
    // remains below the outer JSON depth cap but exhausts the shared
    // constructor budget before type-checking the innermost placeholder.
    let mut too_deep = integer(0);
    for _ in 0..33 {
        too_deep = json!({"kind":"project","target":"binding.pair.value","base":too_deep});
    }
    let too_deep = binding("deep", too_deep, place("deep"));
    let errors = attempt(&base, "binding.add", too_deep).err().unwrap();
    assert!(
        errors.iter().any(|error| error.code == "SPX-G226"),
        "{errors:?}"
    );
    assert_eq!(base.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn body_and_expression_holes_fill_bindings_recover_exactly_and_reject_stale_handles() {
    let fixture = Fixture::scalar();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let selected = selection(&base, "binding.record", "value");
    let initial = ProjectCandidateDraft::open(Arc::clone(&base)).unwrap();
    let draft = initial
        .with_body_hole(initial.draft_digest(), "binding.add", "body")
        .unwrap();
    let draft = draft
        .with_expression_hole(
            draft.draft_digest(),
            "binding.record",
            &selected,
            "expression",
        )
        .unwrap();
    for hole in ["body", "expression"] {
        let context: Value =
            serde_json::from_str(&draft.hole_context(draft.draft_digest(), hole).unwrap()).unwrap();
        assert!(context["constructor_kinds"]
            .as_array()
            .unwrap()
            .contains(&json!("let")));
        assert_eq!(context["materializable"], false);
    }
    let first = draft
        .fill_hole(
            draft.draft_digest(),
            "body",
            &binding("sum", plus(place("left"), place("right")), place("sum")),
        )
        .unwrap();
    assert!(first.complete(first.draft_digest()).is_err());
    assert!(first
        .fill_hole(draft.draft_digest(), "expression", &integer(1))
        .is_err());
    let partial = ProjectCandidateDraft::restore(
        Arc::clone(base.base_revision()),
        base.base_revision().project_revision(),
        first.recovery_capsule().unwrap().as_bytes(),
    )
    .unwrap();
    assert_eq!(partial.to_json(), first.to_json());
    let done = partial
        .fill_hole(
            partial.draft_digest(),
            "expression",
            &binding("saved", place("value"), place("saved")),
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
fn owned_byte_initializer_transfers_once_and_double_use_is_rejected_by_ordinary_admission() {
    let fixture = Fixture::owned();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    for (target, value) in [
        (
            "binding.owned-wrapper",
            call("frame.payload", vec![place("frame")]),
        ),
        ("binding.owned-forward", place("bytes")),
    ] {
        let (candidate, change) =
            attempt(&base, target, binding("held", value, place("held"))).unwrap();
        let parsed = semaprax::parse(source(&candidate, "src/frame.spx"), "src/frame.spx").unwrap();
        let function = parsed
            .functions
            .iter()
            .find(|function| function.stable_id == target)
            .unwrap();
        let (name, _, body) = let_parts(&function.body);
        assert_eq!(name, "held");
        assert_eq!(body.kind, ExprKind::Var("held".to_owned()));
        replay(&base, &candidate, change);
    }
    let before = base.to_json().to_owned();
    let duplicated = binding(
        "held",
        place("bytes"),
        call("binding.take-two", vec![place("held"), place("held")]),
    );
    assert!(attempt(&base, "binding.owned-forward", duplicated).is_err());
    assert_eq!(base.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}
