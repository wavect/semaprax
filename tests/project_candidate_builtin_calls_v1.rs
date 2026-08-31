//! Typed byte builtin evidence. Authored without executing compiler or test gates.
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
    fn new(collision: bool) -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-builtin-candidate-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "builtin-candidate"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "builtin.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["builtin.public"]
tests = ["builtin.tests"]
"#,
        )
        .unwrap();
        let core = if collision {
            r#"module builtin.core;
@id("core.bytes.len") fn authored_length(value:i64)->i64 {value}
@id("builtin.length") fn length(input:borrow Slice<u8>)->usize {0usize}
@id("builtin.public") fn public_value(value:i64)->i64 {value}
@id("builtin.evaluate") fn evaluate()->i64 {42}
"#
        } else {
            r#"module builtin.core;
@id("builtin.consume") fn consume(bytes:own Bytes)->i64 {7}
@id("builtin.index") fn index(value:usize)->usize {value}
@id("builtin.length") fn length(input:borrow Slice<u8>)->usize {byte_len(input)}
@id("builtin.copy") fn copy(input:borrow Slice<u8>)->Bytes {bytes_copy(input)}
@id("builtin.inspect") fn inspect(bytes:own Bytes)->usize {let view=bytes_as_slice(bytes); byte_len(view)}
@id("builtin.shadow") fn shadow(byte_len:i64,input:borrow Slice<u8>)->usize {0usize}
@id("builtin.public") fn public_value(value:i64)->i64 {value}
@id("builtin.evaluate") fn evaluate()->i64 {
    let input=[7u8,8u8];
    if length(array_as_slice(input))==2usize && inspect(copy(array_as_slice(input)))==2usize {42}else{0}
}
"#
        };
        for (path, source) in [
            ("src/core.spx", core),
            ("src/app.spx", "module builtin.app;\nuse function @id(\"builtin.evaluate\") from builtin.core as evaluate;\n@id(\"builtin.main\") fn main()->i64 {evaluate()}\n"),
            ("src/tests.spx", "module builtin.tests;\nuse function @id(\"builtin.evaluate\") from builtin.core as evaluate;\n@id(\"builtin.test\") fn main()->i64 {if evaluate()==42 {0}else{1}}\n"),
        ] {
            let parsed = semaprax::parse(source, path).unwrap();
            std::fs::write(root.join(path), semaprax::format::canonical(&parsed)).unwrap();
        }
        Self(root.canonicalize().unwrap())
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
        .map(|path| std::fs::read(self.0.join(path)).unwrap())
        .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn place(name: &str) -> Value {
    json!({"kind":"place","name":name})
}
fn builtin(target: &str, arguments: Vec<Value>) -> Value {
    json!({"kind":"builtin_call","target":target,"arguments":arguments})
}
fn size(value: u64) -> Value {
    json!({"kind":"usize","value":value})
}
fn binding(name: &str, value: Value, body: Value) -> Value {
    json!({"kind":"let","name":name,"value":value,"body":body})
}
fn apply(base: &ProjectCandidate, intent: Value) -> Result<ProjectCandidate, Vec<Diagnostic>> {
    base.apply(
        base.candidate_digest(),
        &SemanticChange::new(base.revision().project_revision(), &intent)?,
    )
}
fn body(
    base: &ProjectCandidate,
    target: &str,
    value: Value,
) -> Result<ProjectCandidate, Vec<Diagnostic>> {
    apply(
        base,
        json!({"kind":"replace_function_body","target":target,"body":value}),
    )
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
fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    let errors = result.err().expect("expected rejection");
    assert!(errors.iter().any(|e| e.code == expected), "{errors:?}");
}
fn replay(candidate: &ProjectCandidate) {
    let restored = ProjectCandidate::restore(
        Arc::clone(candidate.base_revision()),
        candidate.base_revision().project_revision(),
        candidate.recovery_capsule().unwrap().as_bytes(),
    )
    .unwrap();
    assert_eq!(restored.to_json(), candidate.to_json());
    assert_eq!(
        restored.revision().semantic_graph(),
        candidate.revision().semantic_graph()
    );
}
fn selected(candidate: &ProjectCandidate, target: &str, snippet: &str) -> String {
    let catalog: Value =
        serde_json::from_str(&candidate.expression_catalog(target).unwrap()).unwrap();
    let rows = catalog["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| {
            let span = &row["source_span"];
            row["replaceable"] == true
                && source(candidate).get(
                    span["start"].as_u64().unwrap() as usize
                        ..span["end"].as_u64().unwrap() as usize,
                ) == Some(snippet)
        })
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 1);
    rows[0]["expression_id"].as_str().unwrap().to_owned()
}

#[test]
fn builtin_body_and_declaration_preserve_owned_cleanup_and_shared_loan_replay() {
    let fixture = Fixture::new(false);
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let before = base.to_json().to_owned();
    let measured = builtin("core.bytes.len", vec![place("view")]);
    let consume = json!({"kind":"call","target":"builtin.consume","arguments":[place("bytes")]});
    let replacement = binding(
        "view",
        builtin("core.bytes.as-slice", vec![place("bytes")]),
        binding(
            "observed",
            measured,
            binding("consumed", consume, place("observed")),
        ),
    );
    let changed = body(&base, "builtin.inspect", replacement).unwrap();
    let copied = body(
        &changed,
        "builtin.copy",
        builtin(
            "core.bytes.copy",
            vec![builtin(
                "core.bytes.range",
                vec![
                    place("input"),
                    size(0),
                    builtin("core.bytes.len", vec![place("input")]),
                ],
            )],
        ),
    )
    .unwrap();
    assert!(source(&copied).contains("bytes_copy(byte_range(input, 0usize, byte_len(input)))"));
    assert!(source(&copied).contains("bytes_as_slice(bytes)"));
    assert!(source(&copied).contains("consume(bytes)"));
    let checked = copied
        .revision()
        .entry_program()
        .functions
        .iter()
        .find(|f| f.id.as_str() == "builtin.inspect")
        .unwrap();
    assert!(!checked.loan_plan.loans.is_empty());
    assert!(!checked.cleanup.slots.is_empty());
    let added=apply(&copied,json!({"kind":"add_declaration","target":"builtin.copy","declaration":{
        "id":"builtin.copy-again","name":"copy_again","parameters":[{"name":"input","type":"Slice<u8>","mode":"borrow"}],
        "return_type":"Bytes","effects":[],"requires":[],"ensures":[],"body":builtin("core.bytes.copy",vec![place("input")])
    }})).unwrap();
    assert!(source(&added).contains("fn copy_again(input: borrow Slice<u8>) -> Bytes"));
    replay(&added);
    assert_eq!(base.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn builtin_expression_and_body_holes_keep_scope_and_shared_ownership_until_explicit_completion() {
    let fixture = Fixture::new(false);
    let disk = fixture.bytes();
    let base = Arc::new(fixture.candidate());
    let expression = selected(&base, "builtin.inspect", "byte_len(view)");
    let empty = ProjectCandidateDraft::open(Arc::clone(&base)).unwrap();
    let draft = empty
        .with_body_hole(empty.draft_digest(), "builtin.copy", "copy")
        .unwrap();
    let draft = draft
        .with_expression_hole(
            draft.draft_digest(),
            "builtin.inspect",
            &expression,
            "length",
        )
        .unwrap();
    let context: Value =
        serde_json::from_str(&draft.hole_context(draft.draft_digest(), "length").unwrap()).unwrap();
    let view = context["scope"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == "view")
        .unwrap();
    assert_eq!(view["ownership"], "borrow");
    assert_eq!(context["expected_type_id"], "usize");
    assert!(context["constructor_kinds"]
        .as_array()
        .unwrap()
        .contains(&json!("builtin_call")));
    assert!(context["builtin_calls"]
        .as_array()
        .unwrap()
        .iter()
        .any(|op| op["target"] == "core.bytes.len"));
    let before = draft.to_json().to_owned();
    assert!(draft
        .fill_hole(
            draft.draft_digest(),
            "length",
            &builtin("core.bytes.copy", vec![place("view")])
        )
        .is_err());
    assert_eq!(draft.to_json(), before);
    let partial=draft.fill_hole(draft.draft_digest(),"length",&json!({"kind":"binary","op":"+","left":builtin("core.bytes.len",vec![place("view")]),"right":size(0)})).unwrap();
    code(partial.complete(partial.draft_digest()), "SPX-G232");
    let done = partial
        .fill_hole(
            partial.draft_digest(),
            "copy",
            &builtin("core.bytes.copy", vec![place("input")]),
        )
        .unwrap();
    let candidate = done.complete(done.draft_digest()).unwrap();
    assert!(source(&candidate).contains("byte_len(view) + 0usize"));
    replay(&candidate);
    assert_eq!(draft.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn builtin_unknown_arity_scope_and_live_owner_failures_preserve_parent_and_source() {
    let fixture = Fixture::new(false);
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let before = base.to_json().to_owned();
    for expression in [
        builtin("unknown.byte.operation", vec![place("input")]),
        builtin("core.bytes.len", vec![]),
        builtin("core.bytes.len", vec![place("input"), size(0)]),
        json!({"kind":"call","target":"core.bytes.len","arguments":[place("input")]}),
    ] {
        code(body(&base, "builtin.length", expression), "SPX-G225");
    }
    code(
        body(
            &base,
            "builtin.shadow",
            builtin("core.bytes.len", vec![place("input")]),
        ),
        "SPX-G225",
    );
    let invalid = binding(
        "view",
        builtin("core.bytes.as-slice", vec![place("bytes")]),
        binding(
            "moved",
            json!({"kind":"call","target":"builtin.consume","arguments":[place("bytes")]}),
            builtin("core.bytes.len", vec![place("view")]),
        ),
    );
    code(body(&base, "builtin.inspect", invalid), "SPX-T265");
    assert_eq!(base.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn authored_builtin_identity_cannot_be_reinterpreted_as_compiler_operation() {
    let fixture = Fixture::new(true);
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let before = base.to_json().to_owned();
    code(
        body(
            &base,
            "builtin.length",
            builtin("core.bytes.len", vec![place("input")]),
        ),
        "SPX-G225",
    );
    let catalog: Value =
        serde_json::from_str(&base.change_catalog("builtin.length").unwrap()).unwrap();
    assert!(!catalog["builtin_calls"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["target"] == "core.bytes.len"));
    assert_eq!(base.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn same_intent_and_later_history_cannot_insert_an_authored_identity_for_a_selected_builtin() {
    let fixture = Fixture::new(false);
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let before = base.to_json().to_owned();
    let declaration = |id: &str, expression: Value| {
        json!({"kind":"add_declaration","target":"builtin.copy","declaration":{
            "id":id,"name":"authored_copy","parameters":[{"name":"input","type":"Slice<u8>","mode":"borrow"}],
            "return_type":"Bytes","effects":[],"requires":[],"ensures":[],"body":expression
        }})
    };
    code(
        apply(
            &base,
            declaration(
                "core.bytes.copy",
                builtin("core.bytes.copy", vec![place("input")]),
            ),
        ),
        "SPX-G225",
    );
    let selected = body(
        &base,
        "builtin.copy",
        builtin("core.bytes.copy", vec![place("input")]),
    )
    .unwrap();
    let selected_before = selected.to_json().to_owned();
    // This later declaration uses an ordinary user call; only the earlier
    // typed builtin selector establishes the namespace conflict being tested.
    code(
        apply(
            &selected,
            declaration(
                "core.bytes.copy",
                json!({"kind":"call","target":"builtin.copy","arguments":[place("input")]}),
            ),
        ),
        "SPX-G225",
    );
    assert_eq!(selected.to_json(), selected_before);
    assert_eq!(base.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn builtin_rebase_preserves_nested_user_call_identity_and_rejects_new_compiler_id_collisions() {
    let fixture = Fixture::new(false);
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let candidate = body(
        &base,
        "builtin.copy",
        builtin(
            "core.bytes.copy",
            vec![builtin(
                "core.bytes.range",
                vec![
                    place("input"),
                    json!({"kind":"call","target":"builtin.index","arguments":[size(0)]}),
                    builtin("core.bytes.len", vec![place("input")]),
                ],
            )],
        ),
    )
    .unwrap();
    let renamed = apply(
        &base,
        json!({"kind":"rename_declaration","target":"builtin.index","name":"offset"}),
    )
    .unwrap();
    let renamed = apply(
        &renamed,
        json!({"kind":"rename_declaration","target":"builtin.public","name":"public_number"}),
    )
    .unwrap();
    let rebased = candidate
        .rebase(
            candidate.candidate_digest(),
            Arc::clone(renamed.revision()),
            renamed.revision().project_revision(),
        )
        .unwrap();
    assert!(source(rebased.candidate())
        .contains("bytes_copy(byte_range(input, offset(0usize), byte_len(input)))"));
    assert!(source(rebased.candidate()).contains("fn public_number("));
    replay(rebased.candidate());
    let signature = apply(&base, json!({"kind":"change_function_signature","target":"builtin.index","append_parameters":[{"name":"extra","type":"usize","argument":size(0)}]})).unwrap();
    code(
        candidate.rebase(
            candidate.candidate_digest(),
            Arc::clone(signature.revision()),
            signature.revision().project_revision(),
        ),
        "SPX-G235",
    );
    assert_eq!(fixture.bytes(), disk);

    // Keep the independently admitted base free of actual byte_len calls;
    // source admission may otherwise reject the collision before rebase.
    let collision_fixture = Fixture::new(true);
    let path = collision_fixture.0.join("src/core.spx");
    let colliding_source = std::fs::read_to_string(&path).unwrap();
    let mut clean_program = semaprax::parse(&colliding_source, "src/core.spx").unwrap();
    clean_program
        .functions
        .retain(|f| f.stable_id != "core.bytes.len");
    std::fs::write(&path, semaprax::format::canonical(&clean_program)).unwrap();
    let clean = collision_fixture.candidate();
    let changed = body(
        &clean,
        "builtin.length",
        builtin("core.bytes.len", vec![place("input")]),
    )
    .unwrap();
    let changed_before = changed.to_json().to_owned();
    std::fs::write(&path, &colliding_source).unwrap();
    let colliding = collision_fixture.candidate();
    code(
        changed.rebase(
            changed.candidate_digest(),
            Arc::clone(colliding.revision()),
            colliding.revision().project_revision(),
        ),
        "SPX-G235",
    );
    assert_eq!(changed.to_json(), changed_before);
    assert_eq!(std::fs::read_to_string(path).unwrap(), colliding_source);
}
