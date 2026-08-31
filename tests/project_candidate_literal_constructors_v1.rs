//! Authored, unrun literal evidence; no target execution is claimed here.
use semaprax::ast::{Expr, ExprKind};
use semaprax::diagnostic::Diagnostic;
use semaprax::hir::ResolvedType;
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectCandidateDraft, ProjectExecutionOptions,
    ProjectExecutionOutcome, SemanticChange,
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
            "spx-literal-constructors-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let fixture = Self(root.canonicalize().unwrap());
        std::fs::write(
            fixture.0.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "literal-constructors"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "literal.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["literal.public"]
tests = ["literal.tests"]
"#,
        )
        .unwrap();
        for (path, text) in [
            (
                "src/core.spx",
                r#"module literal.core;
@id("literal.pair") record Pair { @id("literal.pair.value") value:i64, }
@id("literal.text") fn text()->string {"old"}
@id("literal.consume-text") fn consume_text(value:string)->i64 {1}
@id("literal.array") fn array()->[u8;2] {[1u8,2u8]}
@id("literal.empty") fn empty()->[u8;0] {[]}
@id("literal.maximum") fn maximum()->[u8;4095] {[0u8;4095]}
@id("literal.evaluate") fn evaluate()->usize {2usize}
@id("literal.public") fn public_value()->i64 {if evaluate()==2usize {42}else{0}}
"#,
            ),
            (
                "src/app.spx",
                r#"module literal.app;
use function @id("literal.public") from literal.core as public_value;
@id("literal.main") fn main()->i64 {public_value()}
"#,
            ),
            (
                "src/tests.spx",
                r#"module literal.tests;
use function @id("literal.public") from literal.core as public_value;
@id("literal.test") fn main()->i64 {if public_value()==42 {0}else{1}}
"#,
            ),
        ] {
            let program = semaprax::parse(text, path).unwrap();
            std::fs::write(fixture.0.join(path), semaprax::format::canonical(&program)).unwrap();
        }
        fixture
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
fn tail(expression: &Expr) -> &Expr {
    match &expression.kind {
        ExprKind::Block {
            statements,
            tail: value,
        } if statements.is_empty() => tail(value),
        _ => expression,
    }
}
fn body(candidate: &ProjectCandidate, target: &str) -> Expr {
    let program = semaprax::parse(source(candidate), "src/core.spx").unwrap();
    program
        .functions
        .into_iter()
        .find(|f| f.stable_id == target)
        .unwrap()
        .body
}
fn string(value: &str) -> Value {
    json!({"kind":"string","value":value})
}
fn array(values: Vec<u8>) -> Value {
    json!({"kind":"array_u8","values":values})
}
fn place(name: &str) -> Value {
    json!({"kind":"place","name":name})
}
fn binding(name: &str, value: Value, body: Value) -> Value {
    json!({"kind":"let","name":name,"value":value,"body":body})
}
fn builtin(target: &str, arguments: Vec<Value>) -> Value {
    json!({"kind":"builtin_call","target":target,"arguments":arguments})
}
fn call(target: &str, arguments: Vec<Value>) -> Value {
    json!({"kind":"call","target":target,"arguments":arguments})
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
    expression: Value,
) -> Result<(ProjectCandidate, SemanticChange), Vec<Diagnostic>> {
    apply(
        base,
        json!({"kind":"replace_function_body","target":target,"body":expression}),
    )
}
fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    let errors = result.err().expect("invalid literal transaction accepted");
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
        Arc::clone(base.base_revision()),
        base.base_revision().project_revision(),
        candidate.recovery_capsule().unwrap().as_bytes(),
    )
    .unwrap();
    assert_eq!(restored.to_json(), candidate.to_json());
    let parsed = semaprax::parse(source(candidate), "src/core.spx").unwrap();
    assert_eq!(semaprax::format::canonical(&parsed), source(candidate));
}
fn literal_row(candidate: &ProjectCandidate, target: &str, kind: &str) -> Value {
    let catalog: Value =
        serde_json::from_str(&candidate.expression_catalog(target).unwrap()).unwrap();
    let rows = catalog["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["kind"] == kind)
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 1);
    rows[0].clone()
}

#[test]
fn strings_preserve_decoded_contents_owned_hir_and_exact_canonical_replay() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let parent = base.to_json().to_owned();
    for contents in [
        "".to_owned(),
        "quotes \"; @id(\"not-code\") \\ newline\n\r\t\0\u{7f} Ελληνικά 😀".to_owned(),
        "é".repeat(8192),
    ] {
        let (candidate, change) = replace(&base, "literal.text", string(&contents)).unwrap();
        let expression = body(&candidate, "literal.text");
        assert!(matches!(&tail(&expression).kind,ExprKind::String(value) if value==&contents));
        let row = literal_row(&candidate, "literal.text", "string");
        assert_eq!(row["expected_type"], "string");
        assert_eq!(row["ownership"], "own");
        // This is actual source/HIR admission, not a claim that mixed String
        // and indexed-byte target closures are newly executable.
        assert!(!candidate
            .revision()
            .entry_program()
            .functions
            .iter()
            .any(|f| f.id.as_str() == "literal.text"));
        let original_functions = semaprax::parse(source(&base), "src/core.spx")
            .unwrap()
            .functions
            .len();
        assert_eq!(
            semaprax::parse(source(&candidate), "src/core.spx")
                .unwrap()
                .functions
                .len(),
            original_functions
        );
        replay(&base, &candidate, std::slice::from_ref(&change));
        if contents.is_empty() {
            let noncanonical = format!("{} ", candidate.to_json());
            code(
                ProjectCandidate::replay(
                    Arc::clone(base.base_revision()),
                    base.base_revision().project_revision(),
                    std::slice::from_ref(&change),
                    noncanonical.as_bytes(),
                ),
                "SPX-G224",
            );
        }
        code(
            candidate.apply(base.candidate_digest(), &change),
            "SPX-G224",
        );
    }
    assert_eq!(base.to_json(), parent);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn explicit_arrays_keep_copy_contents_and_named_views_feed_owned_bytes() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    for (target, bytes) in [
        ("literal.empty", vec![]),
        ("literal.array", vec![0, 255]),
        ("literal.maximum", vec![255; 4095]),
    ] {
        let (candidate, change) = replace(&base, target, array(bytes.clone())).unwrap();
        let expression = body(&candidate, target);
        assert!(matches!(&tail(&expression).kind,ExprKind::ArrayU8(values) if values==&bytes));
        let row = literal_row(&candidate, target, "array_u8");
        assert_eq!(row["expected_type"], format!("array:u8:{}", bytes.len()));
        assert_eq!(row["ownership"], "value");
        replay(&base, &candidate, std::slice::from_ref(&change));
    }
    let expression = binding(
        "raw",
        array(vec![0, 255]),
        binding(
            "view",
            builtin("core.array-u8.as-slice", vec![place("raw")]),
            binding(
                "owned",
                builtin("core.bytes.copy", vec![place("view")]),
                binding(
                    "owned_view",
                    builtin("core.bytes.as-slice", vec![place("owned")]),
                    builtin("core.bytes.len", vec![place("owned_view")]),
                ),
            ),
        ),
    );
    let (candidate, change) = replace(&base, "literal.evaluate", expression).unwrap();
    let program = candidate.revision().entry_program();
    let facts = program
        .declarations
        .type_facts(&ResolvedType::ArrayU8(2))
        .unwrap();
    assert!(facts.copy && facts.sized && !facts.needs_drop && !facts.contains_resource);
    let checked = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "literal.evaluate")
        .unwrap();
    assert!(!checked.loan_plan.loans.is_empty());
    assert!(!checked.cleanup.slots.is_empty());
    assert_eq!(
        literal_row(&candidate, "literal.evaluate", "array_u8")["ownership"],
        "value"
    );
    // Future interpreter evidence is authored, never executed by this task.
    assert_eq!(
        candidate
            .revision()
            .execute_entry(&ProjectExecutionOptions::default())
            .unwrap()
            .outcome(),
        &ProjectExecutionOutcome::Returned(42)
    );
    replay(&base, &candidate, std::slice::from_ref(&change));
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn body_and_expression_holes_fill_literals_only_through_explicit_completion() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = Arc::new(fixture.candidate());
    let empty = ProjectCandidateDraft::open(Arc::clone(&base)).unwrap();
    let draft = empty
        .with_body_hole(empty.draft_digest(), "literal.text", "text")
        .unwrap();
    let old_array = literal_row(&base, "literal.array", "array_u8");
    let draft = draft
        .with_expression_hole(
            draft.draft_digest(),
            "literal.array",
            old_array["expression_id"].as_str().unwrap(),
            "array",
        )
        .unwrap();
    let pending = draft.to_json().to_owned();
    let text_filled = draft
        .fill_hole(draft.draft_digest(), "text", &string("filled\0λ"))
        .unwrap();
    code(text_filled.complete(text_filled.draft_digest()), "SPX-G232");
    let ready = text_filled
        .fill_hole(text_filled.draft_digest(), "array", &array(vec![255, 0]))
        .unwrap();
    let completed = ready.complete(ready.draft_digest()).unwrap();
    assert!(
        matches!(&tail(&body(&completed,"literal.text")).kind,ExprKind::String(value) if value=="filled\0λ")
    );
    assert!(
        matches!(&tail(&body(&completed,"literal.array")).kind,ExprKind::ArrayU8(values) if values==&vec![255,0])
    );
    let restored = ProjectCandidate::restore(
        Arc::clone(completed.base_revision()),
        completed.base_revision().project_revision(),
        completed.recovery_capsule().unwrap().as_bytes(),
    )
    .unwrap();
    assert_eq!(restored.to_json(), completed.to_json());
    assert_eq!(draft.to_json(), pending);
    code(
        ready.fill_hole(draft.draft_digest(), "array", &array(vec![1, 2])),
        "SPX-G232",
    );
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn malformed_values_byte_limits_and_shared_node_budget_fail_without_source_changes() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let parent = base.to_json().to_owned();
    for invalid in [
        json!({"kind":"string","value":false}),
        json!({"kind":"string","value":"ok","extra":0}),
        json!({"kind":"array_u8","values":"0"}),
        json!({"kind":"array_u8","values":[-1]}),
        json!({"kind":"array_u8","values":[256]}),
        json!({"kind":"array_u8","values":[1.5]}),
        json!({"kind":"array_u8","values":[true]}),
        json!({"kind":"array_u8","values":[],"count":0}),
        json!({"kind":"repeat_array_u8","value":0,"count":2}),
    ] {
        code(replace(&base, "literal.text", invalid), "SPX-G225");
    }
    code(
        replace(&base, "literal.text", string(&("é".repeat(8192) + "x"))),
        "SPX-G226",
    );
    code(
        replace(&base, "literal.maximum", array(vec![0; 4096])),
        "SPX-G226",
    );
    let combined = binding(
        "first",
        array(vec![0; 2047]),
        binding(
            "second",
            array(vec![0; 2047]),
            json!({"kind":"usize","value":0}),
        ),
    );
    code(replace(&base, "literal.evaluate", combined), "SPX-G226");
    assert_eq!(base.to_json(), parent);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn literals_do_not_widen_ownership_borrow_or_scalar_default_admission() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let parent = base.to_json().to_owned();
    let temporary = binding(
        "view",
        builtin("core.array-u8.as-slice", vec![array(vec![0, 255])]),
        builtin("core.bytes.len", vec![place("view")]),
    );
    code(replace(&base, "literal.evaluate", temporary), "SPX-T266");
    let moved = binding(
        "message",
        string("owned"),
        binding(
            "first",
            call("literal.consume-text", vec![place("message")]),
            call("literal.consume-text", vec![place("message")]),
        ),
    );
    code(replace(&base, "literal.public", moved), "SPX-O101");
    code(
        apply(
            &base,
            json!({"kind":"change_function_signature","target":"literal.public","append_parameters":[{"name":"text","type":"string","argument":string("new")}]}),
        ),
        "SPX-G225",
    );
    code(
        apply(
            &base,
            json!({"kind":"add_record_field","target":"literal.pair","field":{"id":"literal.pair.text","name":"text","type":"string","default":string("new")}}),
        ),
        "SPX-G225",
    );
    code(
        apply(
            &base,
            json!({"kind":"add_record_field","target":"literal.pair","field":{"id":"literal.pair.raw","name":"raw","type":"[u8;2]","default":array(vec![0,1])}}),
        ),
        "SPX-G225",
    );
    assert_eq!(base.to_json(), parent);
    assert_eq!(fixture.bytes(), disk);
}
