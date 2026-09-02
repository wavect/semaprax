//! Explicit record/variant addition evidence: authored and intentionally unrun.
use semaprax::ast::{Type, TypeDeclarationKind};
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
            "spx-type-declaration-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let root = root.canonicalize().unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "type-declaration"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "types.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["types.public"]
tests = ["types.tests"]
"#,
        )
        .unwrap();
        for (path, text) in [
            (
                "src/core.spx",
                r#"module types.core;
@id("types.existing") record Existing { @id("types.existing.value") value: i64, }
@id("types.existing-choice") variant ExistingChoice { @id("types.existing-choice.some") Some { @id("types.existing-choice.some.flag") flag: bool, }, @id("types.existing-choice.none") None, }
@id("types.public") fn public_value(value: i64) -> i64 { value }
"#,
            ),
            (
                "src/app.spx",
                r#"module types.app;
use type @id("types.existing") from types.core as Metric;
use function @id("types.public") from types.core as public_value;
@id("types.main") fn main() -> i64 { public_value(42) }
"#,
            ),
            (
                "src/tests.spx",
                r#"module types.tests;
use function @id("types.public") from types.core as public_value;
@id("types.test") fn main() -> i64 { if public_value(42) == 42 { 0 } else { 1 } }
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
fn field(id: &str, name: &str, ty: &str) -> Value {
    json!({"id":id,"name":name,"type":ty})
}
fn record() -> Value {
    json!({"kind":"record","id":"types.added","name":"Added","fields":[field("types.added.z","z","bool"),field("types.added.a","a","i64")]})
}
fn variant() -> Value {
    json!({"kind":"variant","id":"types.added","name":"Added","cases":[
        {"id":"types.added.done","name":"Done","fields":[]},
        {"id":"types.added.data","name":"Data","fields":[field("types.added.data.flag","flag","bool"),field("types.added.data.value","value","i64")]}
    ]})
}
fn intent(anchor: &str, declaration: Value) -> Value {
    json!({"kind":"add_declaration","target":anchor,"declaration":declaration})
}
fn apply(
    base: &ProjectCandidate,
    request: &Value,
) -> Result<(ProjectCandidate, SemanticChange), Vec<Diagnostic>> {
    let change = SemanticChange::new(base.revision().project_revision(), request)?;
    Ok((base.apply(base.candidate_digest(), &change)?, change))
}
fn graph_fact(candidate: &ProjectCandidate, id: &str) -> Value {
    let graph: Value = serde_json::from_str(candidate.revision().semantic_graph()).unwrap();
    graph["declarations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == id)
        .unwrap()
        .clone()
}
fn assert_fact(
    candidate: &ProjectCandidate,
    id: &str,
    kind: &str,
    owner: Option<&str>,
    path: &str,
    module: &str,
) {
    let row = graph_fact(candidate, id);
    assert_eq!(row["kind"], kind);
    assert_eq!(row["owner"], json!(owner));
    assert_eq!(row["path"], path);
    assert_eq!(row["module"], module);
    assert_eq!(row["identity_origin"], "explicit");
}
fn replay(base: &ProjectCandidate, candidate: &ProjectCandidate, changes: &[SemanticChange]) {
    let restored = ProjectCandidate::replay(
        Arc::clone(base.base_revision()),
        base.base_revision().project_revision(),
        changes,
        candidate.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(restored.to_json(), candidate.to_json());
    let capsule = candidate.recovery_capsule().unwrap();
    let recovered = ProjectCandidate::restore(
        Arc::clone(base.base_revision()),
        base.base_revision().project_revision(),
        capsule.as_bytes(),
    )
    .unwrap();
    assert_eq!(recovered.to_json(), candidate.to_json());
}
fn diagnostic<T>(result: Result<T, Vec<Diagnostic>>, code: &str) {
    let errors = result.err().expect("unsupported type addition accepted");
    assert!(errors.iter().any(|error| error.code == code), "{errors:?}");
}

#[test]
fn explicit_record_fields_preserve_source_order_and_graph_owner_provenance_including_empty_records()
{
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    for empty in [false, true] {
        let mut declaration = record();
        if empty {
            declaration["fields"] = json!([]);
        }
        let (candidate, change) = apply(&base, &intent("types.public", declaration)).unwrap();
        let parsed = semaprax::parse(source(&candidate, "src/core.spx"), "src/core.spx").unwrap();
        let added = parsed
            .types
            .iter()
            .find(|ty| ty.stable_id == "types.added")
            .unwrap();
        assert!(added.explicit_id);
        assert!(added.type_parameters.is_empty());
        assert_eq!(added.name, "Added");
        let TypeDeclarationKind::Record { fields } = &added.kind else {
            panic!("record addition missing")
        };
        assert_eq!(fields.len(), if empty { 0 } else { 2 });
        if !empty {
            assert_eq!(
                fields
                    .iter()
                    .map(|field| field.name.as_str())
                    .collect::<Vec<_>>(),
                ["z", "a"]
            );
            assert_eq!(fields[0].ty, Type::Bool);
            assert_eq!(fields[1].ty, Type::I64);
            assert!(fields.iter().all(|field| field.explicit_id));
            for id in ["types.added.z", "types.added.a"] {
                assert_fact(
                    &candidate,
                    id,
                    "field",
                    Some("types.added"),
                    "src/core.spx",
                    "types.core",
                );
            }
        }
        assert_fact(
            &candidate,
            "types.added",
            "record",
            None,
            "src/core.spx",
            "types.core",
        );
        assert_eq!(
            source(&candidate, "src/app.spx"),
            source(&base, "src/app.spx")
        );
        assert_eq!(
            candidate.revision().manifest().to_canonical_toml(),
            base.revision().manifest().to_canonical_toml()
        );
        replay(&base, &candidate, &[change]);
    }
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn variant_unit_cases_payloads_and_main_anchor_retain_exact_explicit_identity_order() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let (candidate, change) = apply(&base, &intent("types.main", variant())).unwrap();
    let parsed = semaprax::parse(source(&candidate, "src/app.spx"), "src/app.spx").unwrap();
    let added = parsed
        .types
        .iter()
        .find(|ty| ty.stable_id == "types.added")
        .unwrap();
    let TypeDeclarationKind::Variant { cases } = &added.kind else {
        panic!("variant addition missing")
    };
    assert_eq!(
        cases
            .iter()
            .map(|case| case.name.as_str())
            .collect::<Vec<_>>(),
        ["Done", "Data"]
    );
    assert!(cases[0].fields.is_empty());
    assert!(cases.iter().all(|case| case.explicit_id));
    assert_eq!(
        cases[1]
            .fields
            .iter()
            .map(|field| field.stable_id.as_str())
            .collect::<Vec<_>>(),
        ["types.added.data.flag", "types.added.data.value"]
    );
    assert!(cases[1].fields.iter().all(|field| field.explicit_id));
    assert_fact(
        &candidate,
        "types.added",
        "variant",
        None,
        "src/app.spx",
        "types.app",
    );
    for case in ["types.added.done", "types.added.data"] {
        assert_fact(
            &candidate,
            case,
            "variant_case",
            Some("types.added"),
            "src/app.spx",
            "types.app",
        );
    }
    for id in ["types.added.data.flag", "types.added.data.value"] {
        assert_fact(
            &candidate,
            id,
            "case_field",
            Some("types.added.data"),
            "src/app.spx",
            "types.app",
        );
    }
    assert_eq!(
        source(&candidate, "src/core.spx"),
        source(&base, "src/core.spx")
    );
    replay(&base, &candidate, &[change]);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn newly_added_types_are_discovered_and_compose_with_nominal_function_signatures_and_constructors()
{
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    for (declaration, constructor, collection, selector) in [
        (
            record(),
            json!({"kind":"record","target":"types.added","fields":[{"target":"types.added.a","value":{"kind":"i64","value":7}},{"target":"types.added.z","value":{"kind":"bool","value":false}}]}),
            "aggregate_constructors",
            "types.added",
        ),
        (
            variant(),
            json!({"kind":"variant","target":"types.added.done","fields":[]}),
            "aggregate_matches",
            "types.added",
        ),
    ] {
        let (added, first) = apply(&base, &intent("types.public", declaration)).unwrap();
        let catalog: Value =
            serde_json::from_str(&added.change_catalog("types.public").unwrap()).unwrap();
        let operation = catalog["operations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["kind"] == "add_declaration")
            .unwrap();
        let forms = operation["type_declaration_forms"].as_array().unwrap();
        assert_eq!(forms.len(), 2);
        assert!(forms
            .iter()
            .all(|form| form["requires_full_candidate_validation"] == true
                && form["max_combined_identities"] == 4096));
        assert!(catalog["nominal_types"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["target"] == "types.added"));
        assert!(catalog[collection]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["target"] == selector));
        let function = json!({"id":"types.make-added","name":"make_added","parameters":[],"return_type":{"kind":"nominal","target":"types.added","type_arguments":[]},"effects":[],"requires":[],"ensures":[],"body":constructor});
        let (candidate, second) = apply(&added, &intent("types.public", function)).unwrap();
        let parsed = semaprax::parse(source(&candidate, "src/core.spx"), "src/core.spx").unwrap();
        let generated = parsed
            .functions
            .iter()
            .find(|function| function.stable_id == "types.make-added")
            .unwrap();
        assert_eq!(
            generated.return_type,
            Type::Named {
                name: "Added".to_owned(),
                arguments: vec![]
            }
        );
        assert!(generated.params.is_empty());
        assert_fact(
            &candidate,
            "types.make-added",
            "function",
            None,
            "src/core.spx",
            "types.core",
        );
        replay(&base, &candidate, &[first, second]);
    }
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn formerly_scalar_only_fields_replay_as_checked_data_declarations() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    for (requested, expected) in [
        (json!("Bytes"), Type::Bytes),
        (json!("i32"), Type::I32),
        (
            json!({"kind":"nominal","target":"types.existing","type_arguments":[]}),
            Type::Named {
                name: "Existing".to_owned(),
                arguments: vec![],
            },
        ),
    ] {
        let mut declaration = record();
        declaration["fields"][0]["type"] = requested;
        let (candidate, change) = apply(&base, &intent("types.public", declaration)).unwrap();
        let parsed = semaprax::parse(source(&candidate, "src/core.spx"), "src/core.spx").unwrap();
        let added = parsed
            .types
            .iter()
            .find(|ty| ty.stable_id == "types.added")
            .unwrap();
        let TypeDeclarationKind::Record { fields } = &added.kind else {
            panic!("new declaration must remain a record");
        };
        assert_eq!(fields[0].ty, expected);
        assert_eq!(fields[0].stable_id, "types.added.z");
        assert_fact(
            &candidate,
            "types.added.z",
            "field",
            Some("types.added"),
            "src/core.spx",
            "types.core",
        );
        replay(&base, &candidate, &[change]);
    }
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn closed_shapes_duplicate_names_and_global_identity_collisions_leave_candidate_unchanged() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let before = base.to_json().to_owned();
    let mut invalid = Vec::new();
    let mut empty_variant = variant();
    empty_variant["cases"] = json!([]);
    invalid.push(empty_variant);
    for key in ["type_parameters", "methods", "source", "exports"] {
        let mut declaration = record();
        declaration[key] = json!([]);
        invalid.push(declaration);
    }
    for id in [
        "types.public",
        "types.main",
        "types.existing",
        "types.existing.value",
        "types.existing-choice.some",
        "types.existing-choice.some.flag",
        "core.option",
    ] {
        let mut declaration = record();
        declaration["id"] = json!(id);
        invalid.push(declaration);
    }
    for name in ["Existing", "public_value", "fn"] {
        let mut declaration = record();
        declaration["name"] = json!(name);
        invalid.push(declaration);
    }
    let mut duplicate = record();
    duplicate["fields"][1]["id"] = duplicate["fields"][0]["id"].clone();
    invalid.push(duplicate);
    let mut duplicate = record();
    duplicate["fields"][1]["name"] = duplicate["fields"][0]["name"].clone();
    invalid.push(duplicate);
    let mut collision = record();
    collision["fields"][0]["id"] = json!("types.public");
    invalid.push(collision);
    let mut collision = record();
    collision["fields"][0]["id"] = json!("types.added");
    invalid.push(collision);
    for ty in [
        json!("str"),
        json!("Slice<u8>"),
        json!({"kind":"nominal","target":"types.added","type_arguments":[]}),
    ] {
        let mut declaration = record();
        declaration["fields"][0]["type"] = ty;
        invalid.push(declaration);
    }
    let mut duplicate = variant();
    duplicate["cases"][1]["id"] = duplicate["cases"][0]["id"].clone();
    invalid.push(duplicate);
    let mut duplicate = variant();
    duplicate["cases"][1]["name"] = duplicate["cases"][0]["name"].clone();
    invalid.push(duplicate);
    let mut collision = variant();
    collision["cases"][1]["fields"][0]["id"] = json!("types.public");
    invalid.push(collision);
    let mut collision = variant();
    collision["cases"][1]["fields"][0]["id"] = json!("types.added.done");
    invalid.push(collision);
    for declaration in invalid {
        diagnostic(
            apply(&base, &intent("types.public", declaration)),
            "SPX-G225",
        );
        assert_eq!(base.to_json(), before);
    }
    let mut alias_collision = record();
    alias_collision["name"] = json!("Metric");
    diagnostic(
        apply(&base, &intent("types.main", alias_collision)),
        "SPX-G225",
    );
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn bounded_field_and_case_lists_and_stale_requests_fail_before_mutation() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let before = base.to_json().to_owned();
    let mut record_fields = record();
    record_fields["fields"] = Value::Array(vec![Value::Null; 65]);
    diagnostic(
        apply(&base, &intent("types.public", record_fields)),
        "SPX-G226",
    );
    let mut cases = variant();
    cases["cases"] = Value::Array(vec![Value::Null; 65]);
    diagnostic(apply(&base, &intent("types.public", cases)), "SPX-G226");
    let mut fields = variant();
    fields["cases"][1]["fields"] = Value::Array(vec![Value::Null; 65]);
    diagnostic(apply(&base, &intent("types.public", fields)), "SPX-G226");
    let (candidate, change) = apply(&base, &intent("types.public", record())).unwrap();
    assert!(candidate.apply(base.candidate_digest(), &change).is_err());
    assert_eq!(base.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}
