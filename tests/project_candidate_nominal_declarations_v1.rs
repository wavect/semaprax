//! Named Copy declaration signatures: authored regressions, intentionally unrun.
use semaprax::ast::{ExprKind, ParamMode, Type};
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
            "spx-nominal-declaration-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let root = root.canonicalize().unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "nominal-declaration"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "nominal.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["nominal.public"]
tests = ["nominal.tests"]
"#,
        )
        .unwrap();
        for (path, text) in [
            (
                "src/core.spx",
                r#"module nominal.core;
@id("nominal.pair") record Pair { @id("nominal.pair.value") value: i64, }
@id("nominal.box") record Box<T> { @id("nominal.box.value") value: T, }
@id("nominal.choice") variant Choice<T> { @id("nominal.choice.some") Some { @id("nominal.choice.some.value") value: T, }, @id("nominal.choice.none") None, }
@id("nominal.owned") record Owned { @id("nominal.owned.bytes") bytes: Bytes, }
@id("nominal.public") fn public_value(value: i64) -> i64 { value }
"#,
            ),
            (
                "src/app.spx",
                r#"module nominal.app;
use type @id("nominal.pair") from nominal.core as Metric;
use function @id("nominal.public") from nominal.core as public_value;
@id("nominal.main") fn main() -> i64 { public_value(42) }
"#,
            ),
            (
                "src/tests.spx",
                r#"module nominal.tests;
use function @id("nominal.public") from nominal.core as public_value;
@id("nominal.test") fn main() -> i64 { if public_value(42) == 42 { 0 } else { 1 } }
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
fn nominal(target: &str, args: &[&str]) -> Value {
    json!({"kind":"nominal","target":target,"type_arguments":args})
}
fn place(name: &str) -> Value {
    json!({"kind":"place","name":name})
}
fn request(target: &str, parameters: Vec<Value>, returns: Value, body: Value) -> Value {
    json!({"kind":"add_declaration","target":target,"declaration":{"id":"nominal.generated","name":"generated","parameters":parameters,"return_type":returns,"effects":[],"requires":[],"ensures":[],"body":body}})
}
fn parameter(ty: Value) -> Value {
    json!({"name":"value","type":ty,"mode":"value"})
}
fn apply(
    base: &ProjectCandidate,
    request: &Value,
) -> Result<(ProjectCandidate, SemanticChange), Vec<Diagnostic>> {
    let change = SemanticChange::new(base.revision().project_revision(), request)?;
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
fn grammar<T>(result: Result<T, Vec<Diagnostic>>) {
    let errors = result
        .err()
        .expect("unsupported nominal declaration accepted");
    assert!(
        errors.iter().any(|error| error.code == "SPX-G225"),
        "{errors:?}"
    );
}

#[test]
fn unused_generic_parameter_is_instantiated_and_checked_without_existing_instance_or_caller() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    assert!(!source(&base, "src/core.spx").contains("Box<i64>"));
    let intent = request(
        "nominal.public",
        vec![parameter(nominal("nominal.box", &["i64"]))],
        json!("i64"),
        json!({"kind":"i64","value":0}),
    );
    let (candidate, change) = apply(&base, &intent).unwrap();
    let parsed = semaprax::parse(source(&candidate, "src/core.spx"), "src/core.spx").unwrap();
    let generated = parsed
        .functions
        .iter()
        .find(|f| f.stable_id == "nominal.generated")
        .unwrap();
    assert_eq!(generated.params.len(), 1);
    assert_eq!(generated.params[0].mode, ParamMode::Value);
    assert_eq!(
        generated.params[0].ty,
        Type::Named {
            name: "Box".to_owned(),
            arguments: vec![Type::I64]
        }
    );
    assert_eq!(generated.return_type, Type::I64);
    assert_eq!(
        source(&candidate, "src/app.spx"),
        source(&base, "src/app.spx")
    );
    assert_eq!(
        candidate.revision().manifest().to_canonical_toml(),
        base.revision().manifest().to_canonical_toml()
    );
    replay(&base, &candidate, change);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn return_only_generic_record_and_variant_instances_are_checked_and_recoverable() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    for (owner, binding, constructor) in [
        (
            "nominal.box",
            "Box",
            json!({"kind":"record","target":"nominal.box","type_arguments":["i64"],"fields":[{"target":"nominal.box.value","value":{"kind":"i64","value":7}}]}),
        ),
        (
            "nominal.choice",
            "Choice",
            json!({"kind":"variant","target":"nominal.choice.none","type_arguments":["i64"],"fields":[]}),
        ),
    ] {
        let (candidate, change) = apply(
            &base,
            &request(
                "nominal.public",
                vec![],
                nominal(owner, &["i64"]),
                constructor,
            ),
        )
        .unwrap();
        let parsed = semaprax::parse(source(&candidate, "src/core.spx"), "src/core.spx").unwrap();
        let generated = parsed
            .functions
            .iter()
            .find(|f| f.stable_id == "nominal.generated")
            .unwrap();
        assert!(generated.params.is_empty());
        assert_eq!(
            generated.return_type,
            Type::Named {
                name: binding.to_owned(),
                arguments: vec![Type::I64]
            }
        );
        let ExprKind::Block { tail, .. } = &generated.body.kind else {
            panic!("canonical function block missing")
        };
        assert!(matches!(
            tail.kind,
            ExprKind::ConstructRecord { .. } | ExprKind::ConstructVariant { .. }
        ));
        replay(&base, &candidate, change);
    }
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn monomorphic_alias_and_authenticated_option_result_signatures_use_visible_spellings() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    for (anchor, path, owner, binding, args, expected) in [
        (
            "nominal.main",
            "src/app.spx",
            "nominal.pair",
            "Metric",
            vec![],
            vec![],
        ),
        (
            "nominal.public",
            "src/core.spx",
            "core.option",
            "Option",
            vec!["i64"],
            vec![Type::I64],
        ),
        (
            "nominal.public",
            "src/core.spx",
            "core.result",
            "Result",
            vec!["i64", "bool"],
            vec![Type::I64, Type::Bool],
        ),
    ] {
        let ty = nominal(owner, &args);
        let (candidate, change) = apply(
            &base,
            &request(anchor, vec![parameter(ty.clone())], ty, place("value")),
        )
        .unwrap();
        let parsed = semaprax::parse(source(&candidate, path), path).unwrap();
        let generated = parsed
            .functions
            .iter()
            .find(|f| f.stable_id == "nominal.generated")
            .unwrap();
        let expected = Type::Named {
            name: binding.to_owned(),
            arguments: expected,
        };
        assert_eq!(generated.params[0].ty, expected);
        assert_eq!(generated.return_type, expected);
        replay(&base, &candidate, change);
    }
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn malformed_noncopy_modes_owner_identity_type_arguments_and_body_mismatches_fail_closed() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let before = base.to_json().to_owned();
    let valid = request(
        "nominal.public",
        vec![parameter(nominal("nominal.pair", &[]))],
        json!("i64"),
        json!({"kind":"i64","value":0}),
    );
    for ty in [
        json!({"kind":"nominal","target":"nominal.pair"}),
        json!({"kind":"nominal","target":"nominal.pair","type_arguments":[],"name":"Pair"}),
        nominal("nominal.choice.some", &["i64"]),
        nominal("nominal.pair.value", &[]),
        nominal("missing.type", &[]),
        nominal("nominal.pair", &["i64"]),
        nominal("nominal.box", &[]),
        nominal("nominal.box", &["Bytes"]),
    ] {
        let mut invalid = valid.clone();
        invalid["declaration"]["parameters"][0]["type"] = ty;
        grammar(apply(&base, &invalid));
    }
    for mode in ["own", "borrow", "shared"] {
        let mut invalid = valid.clone();
        invalid["declaration"]["parameters"][0]["mode"] = json!(mode);
        grammar(apply(&base, &invalid));
    }
    let mut wrong = valid.clone();
    wrong["declaration"]["return_type"] = nominal("nominal.pair", &[]);
    assert!(apply(&base, &wrong).is_err());
    // A valid body does not make value mode admissible for a non-Copy owner.
    let mut owned = valid.clone();
    owned["declaration"]["return_type"] = nominal("nominal.owned", &[]);
    owned["declaration"]["parameters"][0]["type"] = nominal("nominal.owned", &[]);
    owned["declaration"]["body"] = place("value");
    assert!(apply(&base, &owned).is_err());
    let mut unbound = valid.clone();
    unbound["target"] = json!("nominal.main");
    unbound["declaration"]["parameters"][0]["type"] = nominal("nominal.box", &["i64"]);
    grammar(apply(&base, &unbound));
    let (candidate, change) = apply(&base, &valid).unwrap();
    assert!(candidate.apply(base.candidate_digest(), &change).is_err());
    assert_eq!(base.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn declaration_catalog_describes_nominal_templates_without_granting_type_admission() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    for (target, owner, binding) in [
        ("nominal.public", "nominal.box", "Box"),
        ("nominal.main", "nominal.pair", "Metric"),
    ] {
        let catalog: Value = serde_json::from_str(&base.change_catalog(target).unwrap()).unwrap();
        let row = catalog["nominal_types"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["target"] == owner)
            .unwrap();
        assert_eq!(row["binding"], binding);
        assert_eq!(row["requires_full_candidate_validation"], true);
        assert_eq!(row["copy_admission"], "checked_candidate_signature");
        assert_eq!(row["kind"], "nominal");
        if owner == "nominal.box" {
            assert_eq!(row["generic"], true);
            assert_eq!(
                row["type_parameters"][0]["allowed_types"],
                json!(["i64", "bool"])
            );
        }
    }
    assert_eq!(fixture.bytes(), disk);
}
