//! Nominal extraction evidence: authored regressions, intentionally unrun.
use semaprax::ast::{ExprKind, Function, ParamMode, Type};
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
            "spx-nominal-extraction-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let root = root.canonicalize().unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "nominal-extraction"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "extract.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["extract.public"]
tests = ["extract.tests"]
"#,
        )
        .unwrap();
        for (path, text) in [
            (
                "src/core.spx",
                r#"module extract.core;
@id("extract.config") record Config { @id("extract.config.amount") amount: i64, }
@id("extract.box") record Box<T> { @id("extract.box.value") value: T, }
@id("extract.read") fn read(pair: Config) -> i64 { pair.amount + pair.amount }
@id("extract.identity") fn identity(pair: Config) -> Config { pair }
@id("extract.make-box") fn make_box(value: i64) -> i64 { let boxed = Box<i64> { value: value }; boxed.value + boxed.value }
@id("extract.make-option") fn make_option(value: i64) -> i64 { let option = Option<i64>::Some { value: value }; match option { Option::Some { value: payload } => payload + payload, Option::None {} => 0, } }
@id("extract.option-result") fn option_result(value: i64) -> Option<i64> { Option<i64>::Some { value: value } }
@id("extract.mutable") fn mutable_root(value: i64) -> i64 { let mut pair = Config { amount: value }; pair.amount + pair.amount }
@id("extract.owned") fn owned_value(bytes: own Bytes) -> Bytes { bytes }
@id("extract.borrowed") fn borrowed_value(bytes: borrow Slice<u8>) -> usize { byte_len(bytes) }
@id("extract.public") fn public_value(value: i64) -> i64 requires value >= 0 { value }
"#,
            ),
            (
                "src/app.spx",
                r#"module extract.app;
use function @id("extract.public") from extract.core as public_value;
@id("extract.main") fn main() -> i64 { public_value(42) }
"#,
            ),
            (
                "src/tests.spx",
                r#"module extract.tests;
use function @id("extract.public") from extract.core as public_value;
@id("extract.test") fn main() -> i64 { if public_value(42) == 42 { 0 } else { 1 } }
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
        .map(|p| std::fs::read(self.0.join(p)).unwrap())
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
        .find(|s| s.path() == path)
        .unwrap()
        .source()
}
fn function(candidate: &ProjectCandidate, id: &str) -> Function {
    semaprax::parse(source(candidate, "src/core.spx"), "src/core.spx")
        .unwrap()
        .functions
        .iter()
        .find(|f| f.stable_id == id)
        .unwrap()
        .clone()
}
fn selected(candidate: &ProjectCandidate, target: &str, snippet: Option<&str>) -> Value {
    let report: Value =
        serde_json::from_str(&candidate.expression_catalog(target).unwrap()).unwrap();
    let source = source(candidate, "src/core.spx");
    report["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| {
            let span = &row["source_span"];
            if let Some(snippet) = snippet {
                source.get(
                    span["start"].as_u64().unwrap() as usize
                        ..span["end"].as_u64().unwrap() as usize,
                ) == Some(snippet)
            } else {
                row["replaceable"] == true
            }
        })
        .max_by_key(|row| {
            row["source_span"]["end"].as_u64().unwrap()
                - row["source_span"]["start"].as_u64().unwrap()
        })
        .expect("authenticated expression not found")
        .clone()
}
fn request(candidate: &ProjectCandidate, target: &str, selected: &Value) -> SemanticChange {
    SemanticChange::new(candidate.revision().project_revision(),&json!({"kind":"extract_function","target":target,"expression_id":selected["expression_id"],"new_id":"extract.helper","new_name":"extracted_helper"})).unwrap()
}
fn extract(
    candidate: &ProjectCandidate,
    target: &str,
    snippet: Option<&str>,
) -> Result<(ProjectCandidate, SemanticChange), Vec<Diagnostic>> {
    let change = request(candidate, target, &selected(candidate, target, snippet));
    Ok((
        candidate.apply(candidate.candidate_digest(), &change)?,
        change,
    ))
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
    let errors = result.err().expect("unsupported extraction accepted");
    assert!(errors.iter().any(|e| e.code == "SPX-G225"), "{errors:?}");
}
fn named(name: &str, args: Vec<Type>) -> Type {
    Type::Named {
        name: name.to_owned(),
        arguments: args,
    }
}

#[test]
fn repeated_field_reads_capture_one_whole_immutable_nominal_root() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let catalogue: Value =
        serde_json::from_str(&base.change_catalog("extract.read").unwrap()).unwrap();
    let operation = catalogue["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|operation| operation["kind"] == "extract_function")
        .unwrap();
    let constraints = operation["constraints"].as_array().unwrap();
    assert!(constraints.contains(&json!("checked_sized_copy_scalar_or_nominal_values")));
    assert!(constraints.contains(&json!("field_reads_capture_immutable_copy_root")));
    let (candidate, change) =
        extract(&base, "extract.read", Some("pair.amount + pair.amount")).unwrap();
    let helper = function(&candidate, "extract.helper");
    assert!(helper.explicit_id);
    assert_eq!(helper.params.len(), 1);
    assert_eq!(helper.params[0].name, "pair");
    assert_eq!(helper.params[0].ty, named("Config", vec![]));
    assert_eq!(helper.params[0].mode, ParamMode::Value);
    assert_eq!(helper.return_type, Type::I64);
    assert!(source(&candidate, "src/core.spx").contains("pair.amount + pair.amount"));
    let original = function(&candidate, "extract.read");
    let ExprKind::Block { tail, .. } = &original.body.kind else {
        panic!("function block missing")
    };
    let ExprKind::Call { name, args, .. } = &tail.kind else {
        panic!("replacement call missing")
    };
    assert_eq!(name, "extracted_helper");
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].kind, ExprKind::Var("pair".into()));
    assert_eq!(
        source(&candidate, "src/app.spx"),
        source(&base, "src/app.spx")
    );
    replay(&base, &candidate, change.clone());
    assert!(candidate.apply(base.candidate_digest(), &change).is_err());
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn copy_nominal_and_prelude_results_keep_exact_checked_return_identity() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    for (target, snippet, parameter, result) in [
        (
            "extract.identity",
            "pair",
            named("Config", vec![]),
            named("Config", vec![]),
        ),
        (
            "extract.option-result",
            "Option<i64>::Some { value: value }",
            Type::I64,
            named("Option", vec![Type::I64]),
        ),
    ] {
        let (candidate, change) = extract(&base, target, Some(snippet)).unwrap();
        let helper = function(&candidate, "extract.helper");
        assert_eq!(helper.params.len(), 1);
        assert_eq!(helper.params[0].ty, parameter);
        assert_eq!(helper.return_type, result);
        replay(&base, &candidate, change);
    }
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn body_only_generic_instance_and_internal_match_binders_are_not_external_captures() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    // Box<i64> appears only inside a body, never in any original signature.
    for target in ["extract.make-box", "extract.make-option"] {
        let original = function(&base, target);
        let (candidate, change) = extract(&base, target, None).unwrap();
        let helper = function(&candidate, "extract.helper");
        assert_eq!(helper.params.len(), 1);
        assert_eq!(helper.params[0].name, "value");
        assert_eq!(helper.params[0].ty, Type::I64);
        assert_eq!(helper.return_type, Type::I64);
        // Exact canonical moved body, independently rebuilt by apply/replay.
        let mut old = original.clone();
        old.body = helper.body.clone();
        let mut projection = semaprax::parse("module temporary;", "temporary.spx").unwrap();
        projection.functions = vec![old];
        let old_body = semaprax::format::canonical(&projection);
        projection.functions = vec![original];
        let original_body = semaprax::format::canonical(&projection);
        assert_eq!(old_body, original_body);
        replay(&base, &candidate, change);
    }
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn mutable_nominal_roots_owned_values_borrows_and_contract_regions_remain_closed() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let before = base.to_json().to_owned();
    for (target, snippet) in [
        ("extract.mutable", "pair.amount + pair.amount"),
        ("extract.owned", "bytes"),
        ("extract.borrowed", "byte_len(bytes)"),
        ("extract.public", "value >= 0"),
    ] {
        grammar(extract(&base, target, Some(snippet)));
    }
    let mut forged = selected(&base, "extract.read", Some("pair.amount + pair.amount"));
    forged["expression_id"] = json!("not-a-retained-expression");
    let change = request(&base, "extract.read", &forged);
    assert!(base.apply(base.candidate_digest(), &change).is_err());
    assert_eq!(base.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn nominal_extraction_rebases_across_unrelated_display_rename_and_replays_from_shifted_base() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let (candidate, _) = extract(&base, "extract.read", Some("pair.amount + pair.amount")).unwrap();
    let rename = SemanticChange::new(
        base.revision().project_revision(),
        &json!({"kind":"rename_declaration","target":"extract.identity","name":"identity_renamed"}),
    )
    .unwrap();
    let shifted = base.apply(base.candidate_digest(), &rename).unwrap();
    let rebased = candidate
        .rebase(
            candidate.candidate_digest(),
            Arc::clone(shifted.revision()),
            shifted.revision().project_revision(),
        )
        .unwrap()
        .into_candidate();
    assert_eq!(
        function(&rebased, "extract.identity").name,
        "identity_renamed"
    );
    assert_eq!(
        function(&rebased, "extract.helper").params[0].ty,
        named("Config", vec![])
    );
    let capsule = rebased.recovery_capsule().unwrap();
    let restored = ProjectCandidate::restore(
        Arc::clone(rebased.base_revision()),
        rebased.base_revision().project_revision(),
        capsule.as_bytes(),
    )
    .unwrap();
    assert_eq!(restored.to_json(), rebased.to_json());
    assert_eq!(fixture.bytes(), disk);
}
