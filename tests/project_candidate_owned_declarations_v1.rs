//! Local owning function declarations: authored tests, never executed here.
use semaprax::ast::{ParamMode, Type};
use semaprax::diagnostic::Diagnostic;
use semaprax::hir::{DeclarationId, OwnershipMode, PlaceProjection, ResolvedType};
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
    fn new(text: bool) -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-own-declaration-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let fixture = Self(root.canonicalize().unwrap());
        let (schema, profile) = if text {
            ("semaprax.project.v10", "owned-utf8-api.v1")
        } else {
            ("semaprax.project.v8", "owned-data-api.v1")
        };
        std::fs::write(
            fixture.0.join("semaprax.toml"),
            format!(
                r#"schema = "{schema}"
name = "own-declaration"
version = "1.0.0"
profile = "{profile}"
entry = "decl.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["decl.public"]
tests = ["decl.tests"]
"#
            ),
        )
        .unwrap();
        let core = if text {
            r#"module decl.core;
@id("decl.make-text") fn make_text()->string {"text"}
@id("decl.evaluate") fn evaluate()->string {make_text()}
@id("decl.public") fn public_value(value:i64)->i64 {value}
"#
        } else {
            r#"module decl.core;
@id("decl.copy") record CopyValue { @id("decl.copy.value") value:i64, }
@id("decl.existing") record Existing { @id("decl.existing.bytes") bytes:Bytes, }
@id("decl.make-bytes") fn make_bytes()->Bytes {let input=[7u8,8u8];let input_view=array_as_slice(input);bytes_copy(input_view)}
@id("decl.evaluate") fn evaluate()->usize {0usize}
@id("decl.public") fn public_value(value:i64)->i64 {value}
"#
        };
        fixture.write("src/core.spx", core);
        let expected = if text { "\"text\"" } else { "0usize" };
        fixture.write("src/app.spx", &format!("module decl.app;\nuse function @id(\"decl.evaluate\") from decl.core as evaluate;\nuse function @id(\"decl.public\") from decl.core as public_value;\n@id(\"decl.main\") fn main()->i64 {{if evaluate()=={expected} {{public_value(42)}}else{{0}}}}\n"));
        fixture.write("src/tests.spx", &format!("module decl.tests;\nuse function @id(\"decl.evaluate\") from decl.core as evaluate;\n@id(\"decl.test\") fn main()->i64 {{if evaluate()=={expected} {{0}}else{{1}}}}\n"));
        fixture
    }
    fn write(&self, path: &str, source: &str) {
        let parsed = semaprax::parse(source, path).unwrap();
        std::fs::write(self.0.join(path), semaprax::format::canonical(&parsed)).unwrap();
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
fn source(candidate: &ProjectCandidate) -> &str {
    candidate
        .revision()
        .sources()
        .iter()
        .find(|row| row.path() == "src/core.spx")
        .unwrap()
        .source()
}
fn nominal(target: &str) -> Value {
    json!({"kind":"nominal","target":target,"type_arguments":[]})
}
fn place(name: &str) -> Value {
    json!({"kind":"place","name":name})
}
fn call(target: &str, arguments: Vec<Value>) -> Value {
    json!({"kind":"call","target":target,"arguments":arguments})
}
fn builtin(target: &str, arguments: Vec<Value>) -> Value {
    json!({"kind":"builtin_call","target":target,"arguments":arguments})
}
fn binding(name: &str, value: Value, body: Value) -> Value {
    json!({"kind":"let","name":name,"value":value,"body":body})
}
fn parameter(ty: Value, mode: &str) -> Value {
    json!({"name":"value","type":ty,"mode":mode})
}
fn add_function(parameters: Vec<Value>, returns: Value, body: Value) -> Value {
    json!({"kind":"add_declaration","target":"decl.public","declaration":{"id":"decl.forward","name":"forward","parameters":parameters,"return_type":returns,"effects":[],"requires":[],"ensures":[],"body":body}})
}
fn add_type(variant: bool) -> Value {
    let fields = json!([{"id":"decl.packet.bytes","name":"bytes","type":"Bytes"}]);
    let declaration = if variant {
        json!({"kind":"variant","id":"decl.packet","name":"Packet","cases":[{"id":"decl.packet.empty","name":"Empty","fields":[]},{"id":"decl.packet.full","name":"Full","fields":fields}]})
    } else {
        json!({"kind":"record","id":"decl.packet","name":"Packet","fields":fields})
    };
    json!({"kind":"add_declaration","target":"decl.public","declaration":declaration})
}
fn constructor(variant: bool) -> Value {
    json!({"kind":if variant {"variant"} else {"record"},"target":if variant {"decl.packet.full"} else {"decl.packet"},"fields":[{"target":"decl.packet.bytes","value":call("decl.make-bytes",vec![])}]})
}
fn apply(
    base: &ProjectCandidate,
    intent: Value,
) -> Result<(ProjectCandidate, SemanticChange), Vec<Diagnostic>> {
    let change = SemanticChange::new(base.revision().project_revision(), &intent)?;
    Ok((base.apply(base.candidate_digest(), &change)?, change))
}
fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    let errors = result
        .err()
        .expect("invalid owning declaration was accepted");
    assert!(errors.iter().any(|row| row.code == expected), "{errors:?}");
}
fn replay(base: &ProjectCandidate, candidate: &ProjectCandidate, changes: &[SemanticChange]) {
    let rebuilt = ProjectCandidate::replay(
        Arc::clone(base.base_revision()),
        base.base_revision().project_revision(),
        changes,
        candidate.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(rebuilt.to_json(), candidate.to_json());
    assert_eq!(
        rebuilt.revision().semantic_graph(),
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
    assert!(parsed.module_uses.is_empty());
    assert_eq!(
        candidate.revision().manifest().to_canonical_toml(),
        base.revision().manifest().to_canonical_toml()
    );
    for path in ["src/app.spx", "src/tests.spx"] {
        let text = |candidate: &ProjectCandidate| {
            candidate
                .revision()
                .sources()
                .iter()
                .find(|row| row.path() == path)
                .unwrap()
                .source()
                .to_owned()
        };
        assert_eq!(text(candidate), text(base));
    }
}
fn assert_forward(
    candidate: &ProjectCandidate,
    ty: Type,
    mode: ParamMode,
    checked_ty: ResolvedType,
) {
    let parsed = semaprax::parse(source(candidate), "src/core.spx").unwrap();
    let function = parsed
        .functions
        .iter()
        .find(|row| row.stable_id == "decl.forward")
        .unwrap();
    assert!(function.explicit_id);
    assert_eq!(function.params.len(), 1);
    assert_eq!(function.params[0].mode, mode);
    assert_eq!(function.params[0].ty, ty);
    assert_eq!(function.return_type, ty);
    let checked = candidate
        .revision()
        .entry_program()
        .functions
        .iter()
        .find(|row| row.id.as_str() == "decl.forward")
        .unwrap();
    assert_eq!(checked.params[0].ownership, OwnershipMode::Own);
    assert_eq!(checked.params[0].ty, checked_ty);
    assert_eq!(checked.return_type, checked_ty);
    assert!(checked.effects.is_empty());
    let graph: Value = serde_json::from_str(candidate.revision().semantic_graph()).unwrap();
    let row = graph["declarations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == "decl.forward")
        .unwrap();
    assert_eq!(row["kind"], "function");
    assert_eq!(row["identity_origin"], "explicit");
    assert_eq!(row["path"], "src/core.spx");
    assert_eq!(row["module"], "decl.core");
}

#[test]
fn newly_added_record_composes_with_owning_function_and_local_caller_without_imports() {
    let fixture = Fixture::new(false);
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let (typed, first) = apply(&base, add_type(false)).unwrap();
    let (declared, second) = apply(
        &typed,
        add_function(
            vec![parameter(nominal("decl.packet"), "own")],
            nominal("decl.packet"),
            place("value"),
        ),
    )
    .unwrap();
    let body = binding(
        "packet",
        call("decl.forward", vec![constructor(false)]),
        binding(
            "view",
            builtin(
                "core.bytes.as-slice",
                vec![json!({"kind":"field_place","target":"decl.packet.bytes","root":"packet"})],
            ),
            builtin("core.bytes.len", vec![place("view")]),
        ),
    );
    let (candidate, third) = apply(
        &declared,
        json!({"kind":"replace_function_body","target":"decl.evaluate","body":body}),
    )
    .unwrap();
    assert_forward(
        &candidate,
        Type::Named {
            name: "Packet".to_owned(),
            arguments: vec![],
        },
        ParamMode::Own,
        ResolvedType::Nominal {
            declaration: DeclarationId::new("decl.packet".to_owned()),
            arguments: vec![],
        },
    );
    assert!(source(&candidate).contains("bytes_as_slice(packet.bytes)"));
    let caller = candidate
        .revision()
        .entry_program()
        .functions
        .iter()
        .find(|row| row.id.as_str() == "decl.evaluate")
        .unwrap();
    assert!(caller
        .loan_plan
        .loans
        .iter()
        .any(|loan| loan.origin.projections
            == vec![PlaceProjection::Field(DeclarationId::new(
                "decl.packet.bytes".to_owned()
            ))]));
    assert!(!caller.cleanup_plan.slots.is_empty());
    replay(&base, &candidate, &[first, second, third]);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn added_variant_owner_forwarding_has_one_direct_move_and_local_cleanup_without_asymmetric_branches(
) {
    let fixture = Fixture::new(false);
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let (typed, first) = apply(&base, add_type(true)).unwrap();
    let (declared, second) = apply(
        &typed,
        add_function(
            vec![parameter(nominal("decl.packet"), "own")],
            nominal("decl.packet"),
            place("value"),
        ),
    )
    .unwrap();
    let body = binding(
        "choice",
        call("decl.forward", vec![constructor(true)]),
        json!({"kind":"usize","value":2}),
    );
    let (candidate, third) = apply(
        &declared,
        json!({"kind":"replace_function_body","target":"decl.evaluate","body":body}),
    )
    .unwrap();
    assert_forward(
        &candidate,
        Type::Named {
            name: "Packet".to_owned(),
            arguments: vec![],
        },
        ParamMode::Own,
        ResolvedType::Nominal {
            declaration: DeclarationId::new("decl.packet".to_owned()),
            arguments: vec![],
        },
    );
    assert!(source(&candidate).contains("forward(Packet::Full"));
    let caller = candidate
        .revision()
        .entry_program()
        .functions
        .iter()
        .find(|row| row.id.as_str() == "decl.evaluate")
        .unwrap();
    assert!(!caller.cleanup_plan.slots.is_empty());
    replay(&base, &candidate, &[first, second, third]);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn string_value_wire_mode_is_bare_source_string_and_checked_owned_forwarding() {
    // This fixture has no mixed String-bearing variant in its executable closure.
    let fixture = Fixture::new(true);
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let (declared, first) = apply(
        &base,
        add_function(
            vec![parameter(json!("string"), "value")],
            json!("string"),
            place("value"),
        ),
    )
    .unwrap();
    let (candidate, second) = apply(&declared, json!({"kind":"replace_function_body","target":"decl.evaluate","body":call("decl.forward",vec![call("decl.make-text",vec![])])})).unwrap();
    assert_forward(
        &candidate,
        Type::String,
        ParamMode::Value,
        ResolvedType::String,
    );
    assert!(source(&candidate).contains("fn forward(value: string) -> string"));
    assert!(!source(&candidate).contains("own string"));
    replay(&base, &candidate, &[first, second]);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn explicit_wrong_modes_do_not_grant_borrow_shared_resource_or_implicit_owning_admission() {
    let fixture = Fixture::new(false);
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let before = base.to_json().to_owned();
    for (ty, mode) in [
        (nominal("decl.copy"), "own"),
        (nominal("decl.existing"), "borrow"),
        (nominal("decl.existing"), "shared"),
        (json!("string"), "own"),
        (json!("Bytes"), "value"),
        (json!("resource"), "own"),
    ] {
        code(
            apply(
                &base,
                add_function(
                    vec![parameter(ty, mode)],
                    json!("i64"),
                    json!({"kind":"i64","value":0}),
                ),
            ),
            "SPX-G225",
        );
    }
    // The ordinary source ownership checker, rather than a new constructor
    // shortcut, owns the omitted explicit mode on a Bytes-bearing record.
    code(
        apply(
            &base,
            add_function(
                vec![parameter(nominal("decl.existing"), "value")],
                json!("i64"),
                json!({"kind":"i64","value":0}),
            ),
        ),
        "SPX-O001",
    );
    let mut extra = add_function(
        vec![parameter(nominal("decl.existing"), "own")],
        nominal("decl.existing"),
        place("value"),
    );
    extra["declaration"]["parameters"][0]["default"] = json!({"kind":"i64","value":0});
    code(apply(&base, extra), "SPX-G225");
    let mut inaccessible = add_function(
        vec![parameter(nominal("decl.existing"), "own")],
        nominal("decl.existing"),
        place("value"),
    );
    inaccessible["target"] = json!("decl.main");
    code(apply(&base, inaccessible), "SPX-G225");
    assert_eq!(base.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn stale_owned_declarations_and_changed_nominal_dependency_rebase_leave_predecessors_unchanged() {
    let fixture = Fixture::new(false);
    let base = fixture.candidate();
    let disk = fixture.bytes();
    let request = add_function(
        vec![parameter(nominal("decl.existing"), "own")],
        nominal("decl.existing"),
        place("value"),
    );
    let change = SemanticChange::new(base.revision().project_revision(), &request).unwrap();
    code(
        base.apply(&format!("sha256:{}", "0".repeat(64)), &change),
        "SPX-G224",
    );
    let candidate = base.apply(base.candidate_digest(), &change).unwrap();
    let before = candidate.to_json().to_owned();
    replay(&base, &candidate, &[change]);
    assert_eq!(fixture.bytes(), disk);
    let changed = source(&base).replace("decl.existing.bytes", "decl.existing.reidentified");
    assert_ne!(changed, source(&base));
    fixture.write("src/core.spx", &changed);
    let new_base = fixture.candidate();
    let changed_disk = fixture.bytes();
    code(
        candidate.rebase(
            candidate.candidate_digest(),
            Arc::clone(new_base.revision()),
            new_base.revision().project_revision(),
        ),
        "SPX-G235",
    );
    assert_eq!(candidate.to_json(), before);
    assert_eq!(fixture.bytes(), changed_disk);
}
