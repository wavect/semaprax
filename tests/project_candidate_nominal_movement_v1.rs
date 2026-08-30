//! Nominal movement evidence: authored regressions, intentionally unrun.
use semaprax::ast::{ExprKind, ModuleUseKind, Program, Statement, Type};
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
    fn new(existing_aliases: bool, collision: bool) -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-nominal-movement-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let fixture = Self(root.canonicalize().unwrap());
        std::fs::write(
            fixture.0.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "nominal-movement"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "movement.app"
sources = ["src/app.spx", "src/core.spx", "src/support.spx", "src/tests.spx", "src/types.spx"]
web_exports = ["movement.public"]
tests = ["movement.tests"]
"#,
        )
        .unwrap();
        fixture.write("types",r#"module movement.types;
@id("movement.pair") record Pair { @id("movement.pair.amount") amount: i64, }
@id("movement.tag") variant Tag { @id("movement.tag.number") Number { @id("movement.tag.number.value") value: i64, }, @id("movement.tag.none") None, }
@id("movement.types-anchor") fn types_anchor() -> i64 { 0 }
"#);
        fixture.write("core",r#"module movement.core;
use type @id("movement.pair") from movement.types as SourcePair;
use type @id("movement.tag") from movement.types as SourceTag;
@id("movement.transform") fn transform(pair: SourcePair) -> SourcePair
requires pair.amount >= 0 ensures result.amount >= 0
{ let _spx_move_1 = 1; let rebuilt: SourcePair = SourcePair { amount: pair.amount }; rebuilt with { amount: rebuilt.amount + _spx_move_1 } }
@id("movement.pick") fn pick(tag: SourceTag) -> i64 {
    let empty: SourceTag = SourceTag::None {};
    match tag { SourceTag::Number { value: payload } => payload, SourceTag::None {} => match empty { SourceTag::Number { value: fallback } => fallback, SourceTag::None {} => 0, }, }
}
@id("movement.evaluate") fn evaluate(value:i64)->i64 { let changed = transform(SourcePair { amount: value }); changed.amount + pick(SourceTag::None {}) }
@id("movement.own") fn own_value(bytes:own Bytes)->Bytes {bytes}
@id("movement.borrow") fn borrow_value(bytes:borrow Slice<u8>)->usize {byte_len(bytes)}
@id("movement.generic") fn generic<T>(value:T)->T {value}
@id("movement.public") fn public_value(value:i64)->i64 {value}
"#);
        let mut support = String::from("module movement.support;\n");
        if existing_aliases {
            support.push_str("use type @id(\"movement.pair\") from movement.types as Metric;\nuse type @id(\"movement.tag\") from movement.types as Signal;\n");
        }
        if collision {
            support.push_str("@id(\"movement.other-pair\") record SourcePair { @id(\"movement.other-pair.flag\") flag:bool, }\n@id(\"movement.reserved\") fn _spx_move_0(value:i64)->i64 {value}\n");
        }
        support.push_str("@id(\"movement.destination\") fn destination(value:i64)->i64 {value}\n");
        fixture.write("support", &support);
        fixture.write(
            "app",
            r#"module movement.app;
use function @id("movement.evaluate") from movement.core as evaluate;
@id("movement.main") fn main()->i64 {evaluate(41)}
"#,
        );
        fixture.write(
            "tests",
            r#"module movement.tests;
use function @id("movement.evaluate") from movement.core as evaluate;
@id("movement.test") fn main()->i64 {if evaluate(41)==42 {0}else{1}}
"#,
        );
        fixture
    }
    fn write(&self, module: &str, text: &str) {
        let path = format!("src/{module}.spx");
        let program = semaprax::parse(text, &path).unwrap();
        std::fs::write(self.0.join(path), semaprax::format::canonical(&program)).unwrap();
    }
    fn append(&self, module: &str, text: &str) {
        let source =
            std::fs::read_to_string(self.0.join(format!("src/{module}.spx"))).unwrap() + text;
        self.write(module, &source);
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
            "src/support.spx",
            "src/tests.spx",
            "src/types.spx",
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
fn source<'a>(candidate: &'a ProjectCandidate, module: &str) -> &'a str {
    let path = format!("src/{module}.spx");
    candidate
        .revision()
        .sources()
        .iter()
        .find(|s| s.path() == path)
        .unwrap()
        .source()
}
fn program(candidate: &ProjectCandidate, module: &str) -> Program {
    semaprax::parse(source(candidate, module), format!("src/{module}.spx")).unwrap()
}
fn apply(candidate: &ProjectCandidate, intent: Value) -> Result<ProjectCandidate, Vec<Diagnostic>> {
    candidate.apply(
        candidate.candidate_digest(),
        &SemanticChange::new(candidate.revision().project_revision(), &intent)?,
    )
}
fn movement(target: &str) -> Value {
    json!({"kind":"move_declaration","target":target,"destination":"movement.destination"})
}
fn replay(candidate: &ProjectCandidate) {
    let report: Value = serde_json::from_str(candidate.to_json()).unwrap();
    let changes = report["changes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| SemanticChange::new(c["base_revision"].as_str().unwrap(), &c["intent"]).unwrap())
        .collect::<Vec<_>>();
    let replayed = ProjectCandidate::replay(
        Arc::clone(candidate.base_revision()),
        candidate.base_revision().project_revision(),
        &changes,
        candidate.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(replayed.to_json(), candidate.to_json());
    let capsule = candidate.recovery_capsule().unwrap();
    let restored = ProjectCandidate::restore(
        Arc::clone(candidate.base_revision()),
        candidate.base_revision().project_revision(),
        capsule.as_bytes(),
    )
    .unwrap();
    assert_eq!(restored.to_json(), candidate.to_json());
}
fn type_alias(program: &Program, id: &str) -> String {
    let aliases = program
        .module_uses
        .iter()
        .filter(|u| u.kind == ModuleUseKind::Type && u.persistent_id == id)
        .collect::<Vec<_>>();
    assert_eq!(aliases.len(), 1);
    assert_eq!(aliases[0].target_module, "movement.types");
    aliases[0].alias.clone()
}
fn named(name: &str) -> Type {
    Type::Named {
        name: name.to_owned(),
        arguments: vec![],
    }
}
fn grammar<T>(result: Result<T, Vec<Diagnostic>>) {
    let errors = result.err().expect("unsupported movement accepted");
    assert!(errors.iter().any(|e| e.code == "SPX-G225"), "{errors:?}");
}

#[test]
fn new_type_import_uses_actual_third_module_provider_and_preserves_original_imports() {
    let fixture = Fixture::new(false, false);
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let catalogue: Value =
        serde_json::from_str(&base.change_catalog("movement.transform").unwrap()).unwrap();
    let operation = catalogue["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|operation| operation["kind"] == "move_declaration")
        .unwrap();
    let constraints = operation["constraints"].as_array().unwrap();
    assert!(constraints.contains(&json!("preserve_checked_nominal_type_identities")));
    assert!(constraints.contains(&json!("migrate_authenticated_type_bindings")));
    let moved = apply(&base, movement("movement.transform")).unwrap();
    let destination = program(&moved, "support");
    let alias = type_alias(&destination, "movement.pair");
    let function = destination
        .functions
        .iter()
        .find(|f| f.stable_id == "movement.transform")
        .unwrap();
    assert!(function.explicit_id);
    assert_eq!(function.params[0].ty, named(&alias));
    assert_eq!(function.return_type, named(&alias));
    assert_eq!(function.requires.len(), 1);
    assert_eq!(function.ensures.len(), 1);
    assert!(!source(&moved, "core").contains("fn transform("));
    assert!(source(&moved, "core")
        .contains("use type @id(\"movement.pair\") from movement.types as SourcePair;"));
    assert!(source(&moved, "core")
        .contains("use function @id(\"movement.transform\") from movement.support as transform;"));
    assert_eq!(source(&moved, "types"), source(&base, "types"));
    let graph: Value = serde_json::from_str(moved.revision().semantic_graph()).unwrap();
    let identity = graph["declarations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["id"] == "movement.transform")
        .unwrap();
    assert_eq!(identity["path"], "src/support.spx");
    assert_eq!(identity["identity_origin"], "explicit");
    replay(&moved);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn existing_distinct_aliases_rewrite_signatures_typed_locals_constructors_and_match_labels() {
    let fixture = Fixture::new(true, false);
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let moved = apply(&base, movement("movement.transform")).unwrap();
    let moved = apply(&moved, movement("movement.pick")).unwrap();
    let destination = program(&moved, "support");
    assert_eq!(type_alias(&destination, "movement.pair"), "Metric");
    assert_eq!(type_alias(&destination, "movement.tag"), "Signal");
    let function = destination
        .functions
        .iter()
        .find(|f| f.stable_id == "movement.transform")
        .unwrap();
    assert_eq!(function.params[0].ty, named("Metric"));
    assert_eq!(function.return_type, named("Metric"));
    let ExprKind::Block { statements, tail } = &function.body.kind else {
        panic!("moved body missing")
    };
    let Statement::Let {
        declared, value, ..
    } = &statements[1]
    else {
        panic!("typed local missing")
    };
    assert_eq!(declared, &Some(named("Metric")));
    let ExprKind::ConstructRecord { type_name, .. } = &value.kind else {
        panic!("record constructor missing")
    };
    assert_eq!(type_name, "Metric");
    assert!(matches!(tail.kind, ExprKind::UpdateRecord { .. }));
    let text = source(&moved, "support");
    assert!(text.contains("Signal::Number"));
    assert!(text.contains("Signal::None"));
    assert!(!text.contains("SourcePair"));
    assert!(!text.contains("SourceTag"));
    assert!(text.contains("requires pair.amount >= 0"));
    assert!(text.contains("ensures result.amount >= 0"));
    assert!(source(&moved, "core").contains("from movement.types as SourceTag;"));
    replay(&moved);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn generated_type_alias_avoids_destination_type_and_function_names_and_moved_local_bindings() {
    let fixture = Fixture::new(false, true);
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let moved = apply(&base, movement("movement.transform")).unwrap();
    let destination = program(&moved, "support");
    let alias = type_alias(&destination, "movement.pair");
    for occupied in ["SourcePair", "_spx_move_0", "_spx_move_1"] {
        assert_ne!(alias, occupied);
    }
    let function = destination
        .functions
        .iter()
        .find(|f| f.stable_id == "movement.transform")
        .unwrap();
    assert_eq!(function.params[0].ty, named(&alias));
    assert_eq!(function.return_type, named(&alias));
    assert!(source(&moved, "support").contains("let _spx_move_1 = 1;"));
    let local_type = destination
        .types
        .iter()
        .find(|t| t.stable_id == "movement.other-pair")
        .unwrap();
    assert_eq!(local_type.name, "SourcePair");
    let independently_moved = apply(&base, movement("movement.transform")).unwrap();
    assert_eq!(moved.to_json(), independently_moved.to_json());
    replay(&moved);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn owned_borrowed_generic_exported_and_cyclic_nominal_moves_leave_sources_unchanged() {
    let fixture = Fixture::new(false, false);
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let before = base.to_json().to_owned();
    for target in [
        "movement.own",
        "movement.borrow",
        "movement.generic",
        "movement.public",
        "movement.main",
    ] {
        grammar(apply(&base, movement(target)));
    }
    assert_eq!(base.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
    fixture.append(
        "core",
        r#"
@id("movement.local-pair") record LocalPair { @id("movement.local-pair.amount") amount:i64, }
@id("movement.cyclic") fn cyclic(value:LocalPair)->LocalPair {value}
@id("movement.cyclic-caller") fn cyclic_caller(value:LocalPair)->LocalPair {cyclic(value)}
"#,
    );
    let cyclic = fixture.candidate();
    let disk = fixture.bytes();
    let before = cyclic.to_json().to_owned();
    // Surviving source caller needs support, while the moved signature needs
    // the source-owned type: ordinary workspace cycle admission must reject.
    assert!(apply(&cyclic, movement("movement.cyclic")).is_err());
    assert_eq!(cyclic.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn nominal_movement_rebases_over_unrelated_display_rename_with_exact_recovery_and_stale_guard() {
    let fixture = Fixture::new(true, false);
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let moved = apply(&base, movement("movement.transform")).unwrap();
    let renamed = apply(
        &base,
        json!({"kind":"rename_declaration","target":"movement.pick","name":"pick_renamed"}),
    )
    .unwrap();
    let rebased = moved
        .rebase(
            moved.candidate_digest(),
            Arc::clone(renamed.revision()),
            renamed.revision().project_revision(),
        )
        .unwrap()
        .into_candidate();
    assert!(source(&rebased, "core").contains("fn pick_renamed("));
    assert!(source(&rebased, "support").contains("fn transform("));
    assert_eq!(
        type_alias(&program(&rebased, "support"), "movement.pair"),
        "Metric"
    );
    replay(&rebased);
    let old = SemanticChange::new(
        base.revision().project_revision(),
        &movement("movement.transform"),
    )
    .unwrap();
    assert!(moved.apply(base.candidate_digest(), &old).is_err());
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn checked_option_constructor_and_return_type_move_without_a_synthetic_prelude_import() {
    let fixture = Fixture::new(false, false);
    fixture.append(
        "core",
        r#"
@id("movement.option") fn option_value(value:i64)->Option<i64> {Option<i64>::Some {value:value}}
"#,
    );
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let moved = apply(&base, movement("movement.option")).unwrap();
    let destination = program(&moved, "support");
    assert!(destination.module_uses.is_empty());
    let function = destination
        .functions
        .iter()
        .find(|function| function.stable_id == "movement.option")
        .unwrap();
    assert_eq!(
        function.return_type,
        Type::Named {
            name: "Option".to_owned(),
            arguments: vec![Type::I64],
        }
    );
    let ExprKind::Block { tail, .. } = &function.body.kind else {
        panic!("moved function body missing")
    };
    let ExprKind::ConstructVariant {
        type_name,
        type_arguments,
        case_name,
        fields,
        ..
    } = &tail.kind
    else {
        panic!("moved prelude constructor missing")
    };
    assert_eq!(type_name, "Option");
    assert_eq!(type_arguments, &vec![Type::I64]);
    assert_eq!(case_name, "Some");
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].value.kind, ExprKind::Var("value".to_owned()));
    assert!(!source(&moved, "core").contains("fn option_value("));
    assert_eq!(source(&moved, "types"), source(&base, "types"));
    replay(&moved);
    assert_eq!(fixture.bytes(), disk);
}
