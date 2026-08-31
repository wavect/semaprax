//! Authored, unrun StringOp candidate evidence. No target execution is claimed.
use semaprax::diagnostic::Diagnostic;
use semaprax::hir::{self, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedType};
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
    fn new(collision: bool) -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-string-builtins-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let fixture = Self(root.canonicalize().unwrap());
        std::fs::write(
            fixture.0.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "string-builtins"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "strings.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["strings.public"]
tests = ["strings.tests"]
"#,
        )
        .unwrap();
        let extra = if collision {
            r#"@id("core.string.len") fn authored_length(value:i64)->i64 {value}"#
        } else {
            ""
        };
        fixture.write(
            "src/core.spx",
            &format!(
                r#"module strings.core;
@id("strings.core-main") fn main()->i64 {{0}}
@id("strings.measure") fn measure(value:string)->i64 {{0}}
@id("strings.merge") fn merge(left:string,right:string)->string {{left}}
@id("strings.text") fn text()->string {{"seed"}}
@id("strings.predicate") fn predicate(value:string)->bool {{false}}
@id("strings.from-scalar") fn from_scalar(value:char)->string {{""}}
@id("strings.glyph") fn glyph()->char {{'é'}}
@id("strings.shadow") fn shadow(string_len:i64)->i64 {{string_len}}
@id("strings.public") fn public_value(value:i64)->i64 {{value}}
{extra}
"#
            ),
        );
        fixture.write(
            "src/app.spx",
            r#"module strings.app;
use function @id("strings.public") from strings.core as public_value;
@id("strings.main") fn main()->i64 {public_value(42)}
"#,
        );
        fixture.write(
            "src/tests.spx",
            r#"module strings.tests;
use function @id("strings.public") from strings.core as public_value;
@id("strings.test") fn main()->i64 {if public_value(42)==42 {0}else{1}}
"#,
        );
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
fn source(candidate: &ProjectCandidate) -> &str {
    candidate
        .revision()
        .sources()
        .iter()
        .find(|s| s.path() == "src/core.spx")
        .unwrap()
        .source()
}
fn checked(candidate: &ProjectCandidate) -> hir::ResolvedProgram {
    // core-main is real checked fixture source, not a synthetic entry added to
    // make an otherwise unresolvable snippet pass. It is not the Project entry.
    hir::resolve(&semaprax::parse(source(candidate), "src/core.spx").unwrap()).unwrap()
}
fn calls<'a>(program: &'a hir::ResolvedProgram, target: &str) -> Vec<&'a ResolvedExpr> {
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == target)
        .unwrap();
    let mut pending = vec![&function.body];
    let mut calls = Vec::new();
    while let Some(expression) = pending.pop() {
        match &expression.kind {
            ResolvedExprKind::Call { args, .. } => {
                calls.push(expression);
                pending.extend(args);
            }
            ResolvedExprKind::Block { statements, tail } => {
                pending.extend(statements.iter().map(|s| s.value()));
                pending.push(tail);
            }
            ResolvedExprKind::Binary { left, right, .. } => {
                pending.push(left);
                pending.push(right);
            }
            ResolvedExprKind::Unary { value, .. } => pending.push(value),
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                pending.push(condition);
                pending.push(then_branch);
                pending.push(else_branch);
            }
            _ => {}
        }
    }
    calls
}
fn string(value: &str) -> Value {
    json!({"kind":"string","value":value})
}
fn place(name: &str) -> Value {
    json!({"kind":"place","name":name})
}
fn builtin(target: &str, args: Vec<Value>) -> Value {
    json!({"kind":"builtin_call","target":target,"arguments":args})
}
fn call(target: &str, args: Vec<Value>) -> Value {
    json!({"kind":"call","target":target,"arguments":args})
}
fn binding(name: &str, value: Value, body: Value) -> Value {
    json!({"kind":"let","name":name,"value":value,"body":body})
}
fn apply(
    base: &ProjectCandidate,
    intent: Value,
) -> Result<(ProjectCandidate, SemanticChange), Vec<Diagnostic>> {
    let change = SemanticChange::new(base.revision().project_revision(), &intent)?;
    Ok((base.apply(base.candidate_digest(), &change)?, change))
}
fn replace(
    base: &ProjectCandidate,
    target: &str,
    body: Value,
) -> Result<(ProjectCandidate, SemanticChange), Vec<Diagnostic>> {
    apply(
        base,
        json!({"kind":"replace_function_body","target":target,"body":body}),
    )
}
fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    let errors = result.err().expect("invalid string builtin accepted");
    assert!(errors.iter().any(|e| e.code == expected), "{errors:?}");
}
fn replay(base: &ProjectCandidate, candidate: &ProjectCandidate, changes: &[SemanticChange]) {
    let replayed = ProjectCandidate::replay(
        Arc::clone(base.base_revision()),
        base.base_revision().project_revision(),
        changes,
        candidate.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(replayed.to_json(), candidate.to_json());
    assert_eq!(
        replayed.revision().semantic_graph(),
        candidate.revision().semantic_graph()
    );
    let restored = ProjectCandidate::restore(
        Arc::clone(candidate.base_revision()),
        candidate.base_revision().project_revision(),
        candidate.recovery_capsule().unwrap().as_bytes(),
    )
    .unwrap();
    assert_eq!(restored.to_json(), candidate.to_json());
    assert_eq!(
        semaprax::format::canonical(&semaprax::parse(source(candidate), "src/core.spx").unwrap()),
        source(candidate)
    );
}

#[test]
fn all_seven_string_operations_use_exact_compiler_ids_and_parameter_ownership() {
    let fixture = Fixture::new(false);
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let parent = base.to_json().to_owned();
    let catalog: Value =
        serde_json::from_str(&base.change_catalog("strings.measure").unwrap()).unwrap();
    for (id, target, args, returns, mode, parameter_mode) in [
        (
            "core.string.len",
            "strings.measure",
            vec![string("é\0")],
            ResolvedType::I64,
            OwnershipMode::Value,
            "borrow",
        ),
        (
            "core.string.concat",
            "strings.merge",
            vec![string("é"), string("😀")],
            ResolvedType::String,
            OwnershipMode::Own,
            "own",
        ),
        (
            "core.string.is_empty",
            "strings.predicate",
            vec![string("")],
            ResolvedType::Bool,
            OwnershipMode::Value,
            "borrow",
        ),
        (
            "core.string.starts_with",
            "strings.predicate",
            vec![string("\0é"), string("\0")],
            ResolvedType::Bool,
            OwnershipMode::Value,
            "borrow",
        ),
        (
            "core.string.contains",
            "strings.predicate",
            vec![string("é\0😀"), string("\0")],
            ResolvedType::Bool,
            OwnershipMode::Value,
            "borrow",
        ),
        (
            "core.string.len_chars",
            "strings.measure",
            vec![string("é\0😀")],
            ResolvedType::I64,
            OwnershipMode::Value,
            "borrow",
        ),
        (
            "core.string.from_char",
            "strings.from-scalar",
            vec![place("value")],
            ResolvedType::String,
            OwnershipMode::Own,
            "value",
        ),
    ] {
        let descriptor = catalog["builtin_calls"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["target"] == id)
            .unwrap();
        assert_eq!(descriptor["arity"], args.len());
        assert_eq!(descriptor["return_type_id"], returns.identity_key());
        assert_eq!(descriptor["evidence_owner"], "compiler_string_operations");
        assert_eq!(descriptor["effects"], json!([]));
        for parameter in descriptor["parameters"].as_array().unwrap() {
            assert_eq!(parameter["ownership"], parameter_mode);
            assert_eq!(
                parameter["type_id"],
                if id == "core.string.from_char" {
                    "char"
                } else {
                    "string"
                }
            );
            assert!(parameter["type_family"].is_null());
        }
        let (candidate, change) = replace(&base, target, builtin(id, args)).unwrap();
        let hir = checked(&candidate);
        let nodes = calls(&hir, target);
        let operation=nodes.iter().find(|expr|matches!(&expr.kind,ResolvedExprKind::Call{callee,..} if callee.as_str()==id)).unwrap();
        assert_eq!(operation.ty, returns);
        assert_eq!(operation.ownership, mode);
        let ResolvedExprKind::Call {
            type_arguments,
            instance,
            ..
        } = &operation.kind
        else {
            unreachable!()
        };
        assert!(type_arguments.is_empty() && instance.is_none());
        let graph =
            semaprax::graph::to_json(&semaprax::parse(source(&candidate), "src/core.spx").unwrap())
                .unwrap();
        assert!(
            graph.contains(id),
            "source Graph omitted compiler identity {id}"
        );
        assert!(!candidate
            .revision()
            .entry_program()
            .functions
            .iter()
            .any(|f| f.id.as_str() == target));
        replay(&base, &candidate, std::slice::from_ref(&change));
    }
    assert_eq!(base.to_json(), parent);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn borrowed_reads_leave_strings_available_but_concat_consumption_is_sticky() {
    let fixture = Fixture::new(false);
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let expression = binding(
        "length",
        builtin("core.string.len", vec![place("value")]),
        binding(
            "count",
            builtin("core.string.len_chars", vec![place("value")]),
            binding(
                "joined",
                builtin("core.string.concat", vec![place("value"), string("!")]),
                json!({"kind":"binary","op":"+","left":place("length"),"right":{"kind":"binary","op":"+","left":place("count"),"right":builtin("core.string.len",vec![place("joined")])}}),
            ),
        ),
    );
    let (candidate, change) = replace(&base, "strings.measure", expression).unwrap();
    let hir = checked(&candidate);
    assert_eq!(calls(&hir, "strings.measure").len(), 4);
    assert!(source(&candidate).contains("string_concat(value, \"!\")"));
    replay(&base, &candidate, std::slice::from_ref(&change));
    let invalid = binding(
        "joined",
        builtin("core.string.concat", vec![place("value"), string("!")]),
        builtin("core.string.len", vec![place("value")]),
    );
    code(replace(&base, "strings.measure", invalid), "SPX-O101");
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn string_builtin_body_and_expression_holes_require_explicit_completion() {
    let fixture = Fixture::new(false);
    let disk = fixture.bytes();
    let base = Arc::new(fixture.candidate());
    let empty = ProjectCandidateDraft::open(Arc::clone(&base)).unwrap();
    let draft = empty
        .with_body_hole(empty.draft_digest(), "strings.measure", "measure")
        .unwrap();
    let catalog: Value =
        serde_json::from_str(&base.expression_catalog("strings.text").unwrap()).unwrap();
    let literal = catalog["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["kind"] == "string")
        .unwrap();
    let draft = draft
        .with_expression_hole(
            draft.draft_digest(),
            "strings.text",
            literal["expression_id"].as_str().unwrap(),
            "text",
        )
        .unwrap();
    let original = draft.to_json().to_owned();
    let filled = draft
        .fill_hole(
            draft.draft_digest(),
            "measure",
            &builtin("core.string.len_chars", vec![place("value")]),
        )
        .unwrap();
    code(filled.complete(filled.draft_digest()), "SPX-G232");
    let ready = filled
        .fill_hole(
            filled.draft_digest(),
            "text",
            &builtin("core.string.concat", vec![string(""), string("é\0")]),
        )
        .unwrap();
    let candidate = ready.complete(ready.draft_digest()).unwrap();
    let hir = checked(&candidate);
    assert!(calls(&hir,"strings.text").iter().any(|expr|matches!(&expr.kind,ResolvedExprKind::Call{callee,..} if callee.as_str()=="core.string.concat")));
    let restored = ProjectCandidate::restore(
        Arc::clone(candidate.base_revision()),
        candidate.base_revision().project_revision(),
        candidate.recovery_capsule().unwrap().as_bytes(),
    )
    .unwrap();
    assert_eq!(restored.to_json(), candidate.to_json());
    assert_eq!(draft.to_json(), original);
    code(
        ready.fill_hole(draft.draft_digest(), "text", &string("late")),
        "SPX-G232",
    );
    assert_eq!(fixture.bytes(), disk);
}

fn declaration(id: &str, body: Value) -> Value {
    json!({"kind":"add_declaration","target":"strings.public","declaration":{"id":id,"name":"authored_builtin","parameters":[{"name":"value","type":"string","mode":"value"}],"return_type":"i64","effects":[],"requires":[],"ensures":[],"body":body}})
}
#[test]
fn arity_types_names_and_source_identity_collisions_remain_closed() {
    let fixture = Fixture::new(false);
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let parent = base.to_json().to_owned();
    for bad in [
        builtin("core.string.unknown", vec![]),
        builtin("core.string.len", vec![]),
        builtin("core.string.concat", vec![string("one")]),
        call("core.string.len", vec![string("ordinary")]),
    ] {
        code(replace(&base, "strings.measure", bad), "SPX-G225");
    }
    code(
        replace(
            &base,
            "strings.measure",
            builtin("core.string.len", vec![json!({"kind":"i64","value":1})]),
        ),
        "SPX-T205",
    );
    code(
        replace(
            &base,
            "strings.shadow",
            builtin("core.string.len", vec![string("hidden")]),
        ),
        "SPX-G225",
    );
    code(
        apply(
            &base,
            declaration(
                "core.string.len",
                builtin("core.string.len", vec![place("value")]),
            ),
        ),
        "SPX-G225",
    );
    let (selected, _) = replace(
        &base,
        "strings.measure",
        builtin("core.string.len", vec![place("value")]),
    )
    .unwrap();
    let selected_before = selected.to_json().to_owned();
    code(
        apply(
            &selected,
            declaration("core.string.len", json!({"kind":"i64","value":0})),
        ),
        "SPX-G225",
    );
    assert_eq!(selected.to_json(), selected_before);
    let collision = Fixture::new(true);
    let collision_disk = collision.bytes();
    let colliding = collision.candidate();
    code(
        replace(
            &colliding,
            "strings.measure",
            builtin("core.string.len", vec![place("value")]),
        ),
        "SPX-G225",
    );
    let catalog: Value =
        serde_json::from_str(&colliding.change_catalog("strings.measure").unwrap()).unwrap();
    assert!(!catalog["builtin_calls"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["target"] == "core.string.len"));
    assert_eq!(collision.bytes(), collision_disk);
    let reserved = Fixture::new(false);
    let core = std::fs::read_to_string(reserved.0.join("src/core.spx")).unwrap()
        + r#"
@id("strings.named-collision") fn string_len(value:string)->i64 {0}
"#;
    reserved.write("src/core.spx", &core);
    let reserved_disk = reserved.bytes();
    code(
        with_authenticated_project(&reserved.0.join("semaprax.toml"), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
        }),
        "SPX-S113",
    );
    assert_eq!(reserved.bytes(), reserved_disk);
    assert_eq!(base.to_json(), parent);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn nested_char_producer_rebases_by_identity_and_new_builtin_id_cannot_take_its_place() {
    let fixture = Fixture::new(false);
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let (candidate, _) = replace(
        &base,
        "strings.from-scalar",
        builtin("core.string.from_char", vec![call("strings.glyph", vec![])]),
    )
    .unwrap();
    let (renamed, _) = apply(
        &base,
        json!({"kind":"rename_declaration","target":"strings.glyph","name":"letter"}),
    )
    .unwrap();
    let rebased = candidate
        .rebase(
            candidate.candidate_digest(),
            Arc::clone(renamed.revision()),
            renamed.revision().project_revision(),
        )
        .unwrap()
        .into_candidate();
    assert!(source(&rebased).contains("string_from_char(letter())"));
    let hir = checked(&rebased);
    assert!(calls(&hir,"strings.from-scalar").iter().any(|expr|matches!(&expr.kind,ResolvedExprKind::Call{callee,..} if callee.as_str()=="strings.glyph")));
    let restored = ProjectCandidate::restore(
        Arc::clone(rebased.base_revision()),
        rebased.base_revision().project_revision(),
        rebased.recovery_capsule().unwrap().as_bytes(),
    )
    .unwrap();
    assert_eq!(restored.to_json(), rebased.to_json());
    let (colliding, _) = apply(
        &base,
        declaration("core.string.from_char", json!({"kind":"i64","value":0})),
    )
    .unwrap();
    code(
        candidate.rebase(
            candidate.candidate_digest(),
            Arc::clone(colliding.revision()),
            colliding.revision().project_revision(),
        ),
        "SPX-G235",
    );
    assert_eq!(fixture.bytes(), disk);
}
