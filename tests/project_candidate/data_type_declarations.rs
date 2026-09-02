//! Data-bearing type additions: authored regression sources, deliberately unrun.
use semaprax::ast::{Type, TypeDeclarationKind};
use semaprax::diagnostic::Diagnostic;
use semaprax::hir::{DeclarationId, PlaceProjection};
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
            "spx-data-type-addition-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let fixture = Self(root.canonicalize().unwrap());
        std::fs::write(
            fixture.0.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "data-type-addition"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "data.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["data.public"]
tests = ["data.tests"]
"#,
        )
        .unwrap();
        for (path, source) in [
            (
                "src/core.spx",
                r#"module data.core;
@id("data.existing") record Existing { @id("data.existing.value") value:i64, }
@id("data.box") record Box<T> { @id("data.box.value") value:T, }
@id("data.existing-choice") variant ExistingChoice { @id("data.existing-choice.no") No, @id("data.existing-choice.yes") Yes { @id("data.existing-choice.yes.value") value:bool, }, }
@id("data.make-bytes") fn make_bytes()->Bytes {let input=[7u8,8u8];let input_view=array_as_slice(input);bytes_copy(input_view)}
@id("data.evaluate") fn evaluate()->usize {0usize}
@id("data.public") fn public_value(value:i64)->i64 {value}
"#,
            ),
            (
                "src/app.spx",
                r#"module data.app;
use function @id("data.evaluate") from data.core as evaluate;
use function @id("data.public") from data.core as public_value;
@id("data.main") fn main()->i64 {if evaluate()==0usize {public_value(42)}else{0}}
"#,
            ),
            (
                "src/tests.spx",
                r#"module data.tests;
use function @id("data.evaluate") from data.core as evaluate;
@id("data.test") fn main()->i64 {if evaluate()==0usize {0}else{1}}
"#,
            ),
        ] {
            fixture.write(path, source);
        }
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
        .find(|source| source.path() == "src/core.spx")
        .unwrap()
        .source()
}
fn field(id: &str, name: &str, ty: Value) -> Value {
    json!({"id":id,"name":name,"type":ty})
}
fn nominal(target: &str, arguments: &[&str]) -> Value {
    json!({"kind":"nominal","target":target,"type_arguments":arguments})
}
fn record(fields: Vec<Value>) -> Value {
    json!({"kind":"record","id":"data.added","name":"Added","fields":fields})
}
fn variant(fields: Vec<Value>) -> Value {
    json!({"kind":"variant","id":"data.added","name":"Added","cases":[
        {"id":"data.added.none","name":"None","fields":[]},
        {"id":"data.added.some","name":"Some","fields":fields}]})
}
fn addition(declaration: Value) -> Value {
    json!({"kind":"add_declaration","target":"data.public","declaration":declaration})
}
fn apply(
    base: &ProjectCandidate,
    intent: Value,
) -> Result<(ProjectCandidate, SemanticChange), Vec<Diagnostic>> {
    let change = SemanticChange::new(base.revision().project_revision(), &intent)?;
    Ok((base.apply(base.candidate_digest(), &change)?, change))
}
fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    let diagnostics = result.err().expect("expected unchanged failed candidate");
    assert!(
        diagnostics.iter().any(|row| row.code == expected),
        "{diagnostics:?}"
    );
}
fn fact(candidate: &ProjectCandidate, id: &str, kind: &str, owner: Option<&str>) {
    let graph: Value = serde_json::from_str(candidate.revision().semantic_graph()).unwrap();
    let row = graph["declarations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == id)
        .unwrap();
    assert_eq!(row["kind"], kind);
    assert_eq!(row["owner"], json!(owner));
    assert_eq!(row["identity_origin"], "explicit");
    assert_eq!(row["path"], "src/core.spx");
    assert_eq!(row["module"], "data.core");
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
    let recovered = ProjectCandidate::restore(
        Arc::clone(base.base_revision()),
        base.base_revision().project_revision(),
        candidate.recovery_capsule().unwrap().as_bytes(),
    )
    .unwrap();
    assert_eq!(recovered.to_json(), candidate.to_json());
    let parsed = semaprax::parse(source(candidate), "src/core.spx").unwrap();
    assert_eq!(semaprax::format::canonical(&parsed), source(candidate));
    assert_eq!(
        candidate.revision().manifest().to_canonical_toml(),
        base.revision().manifest().to_canonical_toml()
    );
    for path in ["src/app.spx", "src/tests.spx"] {
        let text = |revision: &ProjectCandidate| {
            revision
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
fn place(name: &str) -> Value {
    json!({"kind":"place","name":name})
}
fn binding(name: &str, value: Value, body: Value) -> Value {
    json!({"kind":"let","name":name,"value":value,"body":body})
}
fn builtin(target: &str, arguments: Vec<Value>) -> Value {
    json!({"kind":"builtin_call","target":target,"arguments":arguments})
}

#[test]
fn unused_admitted_record_and_variant_fields_keep_exact_types_and_explicit_owner_order() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    // Each type is independent: declaration admission is not a claim that a
    // mixed String/Bytes variant belongs to the executable owned-data closure.
    for (wire, expected) in [
        ("i64", Type::I64),
        ("bool", Type::Bool),
        ("i32", Type::I32),
        ("u8", Type::U8),
        ("usize", Type::Usize),
        ("string", Type::String),
        ("Bytes", Type::Bytes),
    ] {
        for is_variant in [false, true] {
            if is_variant && !matches!(wire, "i64" | "bool" | "Bytes") {
                // Explicit unchanged-source diagnostics for these cases below.
                continue;
            }
            let owner = if is_variant {
                "data.added.some"
            } else {
                "data.added"
            };
            let first = format!("{owner}.z");
            let second = format!("{owner}.a");
            let fields = vec![
                field(&first, "z", json!(wire)),
                field(&second, "a", json!("bool")),
            ];
            let (candidate, change) = apply(
                &base,
                addition(if is_variant {
                    variant(fields)
                } else {
                    record(fields)
                }),
            )
            .unwrap();
            let parsed = semaprax::parse(source(&candidate), "src/core.spx").unwrap();
            let declaration = parsed
                .types
                .iter()
                .find(|row| row.stable_id == "data.added")
                .unwrap();
            assert!(declaration.explicit_id);
            assert!(declaration.type_parameters.is_empty());
            let fields = match &declaration.kind {
                TypeDeclarationKind::Record { fields } => {
                    assert!(!is_variant);
                    fields
                }
                TypeDeclarationKind::Variant { cases } => {
                    assert!(is_variant);
                    assert_eq!(
                        cases
                            .iter()
                            .map(|case| case.stable_id.as_str())
                            .collect::<Vec<_>>(),
                        ["data.added.none", "data.added.some"]
                    );
                    assert!(cases[0].fields.is_empty());
                    assert!(cases.iter().all(|case| case.explicit_id));
                    fact(
                        &candidate,
                        "data.added.none",
                        "variant_case",
                        Some("data.added"),
                    );
                    fact(
                        &candidate,
                        "data.added.some",
                        "variant_case",
                        Some("data.added"),
                    );
                    &cases[1].fields
                }
                _ => panic!("data addition changed declaration kind"),
            };
            assert_eq!(
                fields
                    .iter()
                    .map(|field| field.name.as_str())
                    .collect::<Vec<_>>(),
                ["z", "a"]
            );
            assert_eq!(fields[0].ty, expected);
            assert_eq!(fields[1].ty, Type::Bool);
            assert!(fields.iter().all(|field| field.explicit_id));
            fact(
                &candidate,
                "data.added",
                if is_variant { "variant" } else { "record" },
                None,
            );
            for id in [first, second] {
                fact(
                    &candidate,
                    &id,
                    if is_variant { "case_field" } else { "field" },
                    Some(owner),
                );
            }
            replay(&base, &candidate, &[change]);
        }
    }
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn added_flat_owned_record_is_constructed_locally_and_its_direct_field_loan_replays() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let (added, first) = apply(
        &base,
        addition(record(vec![
            field("data.added.bytes", "bytes", json!("Bytes")),
            field("data.added.marker", "marker", json!("usize")),
        ])),
    )
    .unwrap();
    let constructor = json!({"kind":"record","target":"data.added","fields":[
        {"target":"data.added.marker","value":{"kind":"usize","value":7}},
        {"target":"data.added.bytes","value":{"kind":"call","target":"data.make-bytes","arguments":[]}}]});
    let body = binding(
        "packet",
        constructor,
        binding(
            "view",
            builtin(
                "core.bytes.as-slice",
                vec![json!({"kind":"field_place","target":"data.added.bytes","root":"packet"})],
            ),
            builtin("core.bytes.len", vec![place("view")]),
        ),
    );
    let (candidate, second) = apply(
        &added,
        json!({"kind":"replace_function_body","target":"data.evaluate","body":body}),
    )
    .unwrap();
    assert!(source(&candidate).contains("bytes_as_slice(packet.bytes)"));
    assert!(source(&candidate).contains("byte_len(view)"));
    let marker = source(&candidate).find("marker: 7usize").unwrap();
    let bytes = source(&candidate)[marker..]
        .find("bytes: make_bytes()")
        .unwrap();
    assert!(bytes > 0);
    fact(&candidate, "data.added.bytes", "field", Some("data.added"));
    let function = candidate
        .revision()
        .entry_program()
        .functions
        .iter()
        .find(|function| function.id.as_str() == "data.evaluate")
        .unwrap();
    assert!(
        !function.loan_plan.loans.is_empty(),
        "named direct field view must retain checked loan evidence"
    );
    assert!(function
        .loan_plan
        .loans
        .iter()
        .any(|loan| loan.origin.projections
            == vec![PlaceProjection::Field(DeclarationId::new(
                "data.added.bytes".to_owned()
            ))]));
    assert!(
        !function.cleanup_plan.slots.is_empty(),
        "owned packet must retain ordinary cleanup evidence"
    );
    replay(&base, &candidate, &[first, second]);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn added_owned_variant_construction_and_cleanup_stay_local_without_new_signature_or_import_admission(
) {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let (added, first) = apply(
        &base,
        addition(variant(vec![
            field("data.added.some.bytes", "bytes", json!("Bytes")),
            field("data.added.some.marker", "marker", json!("i64")),
        ])),
    )
    .unwrap();
    let constructor = json!({"kind":"variant","target":"data.added.some","fields":[
        {"target":"data.added.some.marker","value":{"kind":"i64","value":7}},
        {"target":"data.added.some.bytes","value":{"kind":"call","target":"data.make-bytes","arguments":[]}}]});
    // No new owning-match constructor is assumed: the local is finalized by
    // the existing checked cleanup plan at the function's scalar return.
    let (candidate, second) = apply(&added, json!({"kind":"replace_function_body","target":"data.evaluate","body":binding("choice", constructor, json!({"kind":"usize","value":2}))})).unwrap();
    assert!(source(&candidate).contains("Added::Some"));
    fact(
        &candidate,
        "data.added.some.bytes",
        "case_field",
        Some("data.added.some"),
    );
    let function = candidate
        .revision()
        .entry_program()
        .functions
        .iter()
        .find(|function| function.id.as_str() == "data.evaluate")
        .unwrap();
    assert!(!function.cleanup_plan.slots.is_empty());
    replay(&base, &candidate, &[first, second]);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn stable_nominal_fields_use_existing_local_bindings_without_import_synthesis() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    for (selector, args, name, arguments) in [
        ("data.existing", vec![], "Existing", vec![]),
        ("data.existing-choice", vec![], "ExistingChoice", vec![]),
    ] {
        let (candidate, change) = apply(
            &base,
            addition(record(vec![field(
                "data.added.child",
                "child",
                nominal(selector, &args),
            )])),
        )
        .unwrap();
        let parsed = semaprax::parse(source(&candidate), "src/core.spx").unwrap();
        let added = parsed
            .types
            .iter()
            .find(|row| row.stable_id == "data.added")
            .unwrap();
        let TypeDeclarationKind::Record { fields } = &added.kind else {
            panic!("record missing")
        };
        assert_eq!(
            fields[0].ty,
            Type::Named {
                name: name.to_owned(),
                arguments
            }
        );
        assert!(
            parsed.module_uses.is_empty(),
            "local selectors must not synthesize imports"
        );
        fact(&candidate, "data.added.child", "field", Some("data.added"));
        replay(&base, &candidate, &[change]);
    }
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn wire_data_types_do_not_bypass_variant_or_nested_generic_source_restrictions() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let before = base.to_json().to_owned();
    for ty in [
        json!("i32"),
        json!("u8"),
        json!("usize"),
        json!("string"),
        nominal("data.existing", &[]),
    ] {
        code(
            apply(
                &base,
                addition(variant(vec![field("data.added.some.value", "value", ty)])),
            ),
            "SPX-T215",
        );
    }
    code(
        apply(
            &base,
            addition(record(vec![field(
                "data.added.child",
                "child",
                nominal("data.box", &["i64"]),
            )])),
        ),
        "SPX-T223",
    );
    assert_eq!(base.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn view_resource_self_reference_collision_and_stale_inputs_leave_sources_and_candidate_unchanged() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let before = base.to_json().to_owned();
    for ty in [
        json!("str"),
        json!("Slice<u8>"),
        json!("resource"),
        json!("own Bytes"),
        json!("[u8;2]"),
        nominal("data.added", &[]),
        nominal("data.missing", &[]),
        nominal("data.existing.value", &[]),
        nominal("data.box", &[]),
        json!({"kind":"nominal","target":"data.box","type_arguments":[{"kind":"nominal","target":"data.existing","type_arguments":[]}]}),
    ] {
        code(
            apply(
                &base,
                addition(record(vec![field("data.added.value", "value", ty)])),
            ),
            "SPX-G225",
        );
    }
    let mut borrowed = field("data.added.value", "value", json!("Bytes"));
    borrowed["mode"] = json!("borrow");
    code(apply(&base, addition(record(vec![borrowed]))), "SPX-G225");
    code(
        apply(
            &base,
            addition(record(vec![field(
                "data.existing.value",
                "value",
                json!("Bytes"),
            )])),
        ),
        "SPX-G225",
    );
    code(
        apply(
            &base,
            addition(record(vec![
                field("data.added.value", "first", json!("Bytes")),
                field("data.added.value", "second", json!("i32")),
            ])),
        ),
        "SPX-G225",
    );
    let change = SemanticChange::new(
        base.revision().project_revision(),
        &addition(record(vec![field(
            "data.added.value",
            "value",
            json!("Bytes"),
        )])),
    )
    .unwrap();
    code(
        base.apply(&format!("sha256:{}", "0".repeat(64)), &change),
        "SPX-G224",
    );
    assert_eq!(base.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn nominal_dependencies_survive_unrelated_edits_but_not_same_id_shape_or_member_identity_changes() {
    let fixture = Fixture::new();
    let base = fixture.candidate();
    let (candidate, _) = apply(
        &base,
        addition(record(vec![field(
            "data.added.child",
            "child",
            nominal("data.existing", &[]),
        )])),
    )
    .unwrap();
    let (unrelated, _) = apply(&base, json!({"kind":"replace_function_body","target":"data.evaluate","body":{"kind":"usize","value":1}})).unwrap();
    let rebased = candidate
        .rebase(
            candidate.candidate_digest(),
            Arc::clone(unrelated.revision()),
            unrelated.revision().project_revision(),
        )
        .unwrap();
    assert!(source(rebased.candidate()).contains("child: Existing"));
    let restored = ProjectCandidate::restore(
        Arc::clone(rebased.candidate().base_revision()),
        rebased.candidate().base_revision().project_revision(),
        rebased.candidate().recovery_capsule().unwrap().as_bytes(),
    )
    .unwrap();
    assert_eq!(restored.to_json(), rebased.candidate().to_json());
    let original = source(&base).to_owned();
    let before = candidate.to_json().to_owned();
    for changed in [
        original.replacen("value: i64", "value: bool", 1),
        original.replace("data.existing.value", "data.existing.reidentified"),
    ] {
        assert_ne!(changed, original);
        fixture.write("src/core.spx", &changed);
        let new_base = fixture.candidate();
        let disk = fixture.bytes();
        code(
            candidate.rebase(
                candidate.candidate_digest(),
                Arc::clone(new_base.revision()),
                new_base.revision().project_revision(),
            ),
            "SPX-G235",
        );
        assert_eq!(candidate.to_json(), before);
        assert_eq!(fixture.bytes(), disk);
    }
}
