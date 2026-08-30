//! Nominal declaration rename evidence: authored and intentionally unrun.
use semaprax::ast::{Program, Type, TypeDeclarationKind};
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
            "spx-nominal-rename-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let fixture = Self(root.canonicalize().unwrap());
        std::fs::write(
            fixture.0.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "nominal-rename"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "rename.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["rename.public"]
tests = ["rename.tests"]
"#,
        )
        .unwrap();
        fixture.write("core",r#"module rename.core;
@id("rename.pair") record Pair { @id("rename.pair.value") value:i64, }
@id("rename.outer") record Outer { @id("rename.outer.inner") inner:Pair, }
@id("rename.labels") record Labels { @id("rename.labels.pair") Pair:i64, }
@id("rename.choice") variant Choice { @id("rename.choice.some") Some { @id("rename.choice.some.value") value:i64, }, @id("rename.choice.none") None, }
@id("rename.box") record Box<T> { @id("rename.box.value") value:T, }
@id("rename.generic-choice") variant GenericChoice<T> { @id("rename.generic-choice.some") Some { @id("rename.generic-choice.some.value") value:T, }, @id("rename.generic-choice.none") None, }
@id("rename.make") fn make(value:i64)->Pair {Pair {value:value}}
@id("rename.transform") fn transform(pair:Pair)->Pair requires pair.value >= 0 ensures result.value >= 0 {
    let rebuilt:Pair = Pair {value:pair.value}; rebuilt with {value:rebuilt.value + 1}
}
@id("rename.nested") fn nested(input:Outer)->i64 {
    match input {Outer {inner:Pair {value}} => value,}
}
@id("rename.choose") fn choose(choice:Choice)->i64 {
    match choice {Choice::Some {value} => value, Choice::None {} => 0,}
}
@id("rename.boxed") fn boxed(value:i64)->Box<i64> {Box<i64> {value:value}}
@id("rename.generic-selected") fn generic_selected(value:i64)->i64 {
    let selected:GenericChoice<i64> = GenericChoice<i64>::Some {value:value};
    match selected {GenericChoice::Some {value} => value, GenericChoice::None {} => 0,}
}
@id("rename.shadow") fn shadow<Pair>(Pair:Pair)->Pair {Pair}
@id("rename.local") fn local(value:i64)->i64 {let Pair = value; let labels = Labels {Pair:Pair}; labels.Pair}
@id("rename.evaluate") fn evaluate(value:i64)->i64 {
    let pair = transform(make(value)); let boxed_value = boxed(0);
    nested(Outer {inner:pair}) + choose(Choice::None {}) + boxed_value.value + generic_selected(0) + local(0)
}
@id("rename.public") fn public_value(value:i64)->i64 {value}
"#);
        fixture.write("app",r#"module rename.app;
use type @id("rename.pair") from rename.core as Metric;
use type @id("rename.choice") from rename.core as Signal;
use function @id("rename.evaluate") from rename.core as evaluate;
@id("rename.app-local") fn local(pair:Metric)->i64 {pair.value}
@id("rename.app-choice") fn choose(value:Signal)->i64 {match value {Signal::Some {value} => value, Signal::None {} => 0,}}
@id("rename.main") fn main()->i64 {evaluate(41) + local(Metric {value:0}) + choose(Signal::None {})}
"#);
        fixture.write(
            "tests",
            r#"module rename.tests;
use function @id("rename.evaluate") from rename.core as evaluate;
@id("rename.other-pair") record Pair { @id("rename.other-pair.flag") flag:bool, }
@id("rename.test") fn main()->i64 {if evaluate(41)==42 {0}else{1}}
"#,
        );
        fixture
    }
    fn write(&self, module: &str, text: &str) {
        let path = format!("src/{module}.spx");
        let program = semaprax::parse(text, &path).unwrap();
        std::fs::write(self.0.join(path), semaprax::format::canonical(&program)).unwrap();
    }
    fn append(&self, text: &str) {
        let source = std::fs::read_to_string(self.0.join("src/core.spx")).unwrap() + text;
        self.write("core", &source);
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
fn rename(target: &str, name: &str) -> Value {
    json!({"kind":"rename_declaration","target":target,"name":name})
}
fn named(name: &str, args: Vec<Type>) -> Type {
    Type::Named {
        name: name.to_owned(),
        arguments: args,
    }
}
fn replay(candidate: &ProjectCandidate) {
    let capsule = candidate.recovery_capsule().unwrap();
    let recovered = ProjectCandidate::restore(
        Arc::clone(candidate.base_revision()),
        candidate.base_revision().project_revision(),
        capsule.as_bytes(),
    )
    .unwrap();
    assert_eq!(candidate.to_json(), recovered.to_json());
    assert_eq!(
        candidate.revision().semantic_graph(),
        recovered.revision().semantic_graph()
    );
}
fn preserved(base: &ProjectCandidate, candidate: &ProjectCandidate) {
    fn inventory(candidate: &ProjectCandidate) -> Value {
        let graph: Value = serde_json::from_str(candidate.revision().semantic_graph()).unwrap();
        // Project declarations contain identity/origin/location, not display
        // names. Edges contain stable endpoints/structural sites/import aliases,
        // not source spans, source revisions or nominal display names.
        assert!(!graph["declarations"].as_array().unwrap().is_empty());
        assert!(graph["edges"]
            .as_array()
            .unwrap()
            .iter()
            .any(|edge| edge["kind"] == "type_import"));
        json!({"declarations":graph["declarations"],"edges":graph["edges"]})
    }
    assert_eq!(inventory(base), inventory(candidate));
    for module in ["app", "tests"] {
        assert_eq!(source(base, module), source(candidate, module));
    }
}
fn error<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    match result {
        Ok(_) => panic!("expected {expected}"),
        Err(errors) => assert!(errors.iter().any(|e| e.code == expected), "{errors:?}"),
    }
}

#[test]
#[ignore = "repro at be5d3da: choose Signal::Some value shadowing"]
fn record_rename_updates_nested_types_patterns_and_contract_bodies_without_alias_edits() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let catalog: Value =
        serde_json::from_str(&base.change_catalog("rename.pair").unwrap()).unwrap();
    let operation = catalog["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|operation| operation["kind"] == "rename_declaration")
        .unwrap();
    assert_eq!(
        operation["required_fields"],
        json!(["kind", "target", "name"])
    );
    for constraint in [
        "source_record_or_variant_owner",
        "preserve_stable_identity_and_member_identities",
        "preserve_import_aliases",
        "migrate_authenticated_type_occurrences",
        "full_candidate_revalidation",
    ] {
        assert!(operation["constraints"]
            .as_array()
            .unwrap()
            .contains(&json!(constraint)));
    }
    let candidate = apply(&base, rename("rename.pair", "MetricPair")).unwrap();
    let parsed = program(&candidate, "core");
    let pair = parsed
        .types
        .iter()
        .find(|t| t.stable_id == "rename.pair")
        .unwrap();
    assert_eq!(pair.name, "MetricPair");
    assert!(pair.explicit_id);
    for id in ["rename.make", "rename.transform"] {
        let function = parsed.functions.iter().find(|f| f.stable_id == id).unwrap();
        assert_eq!(function.return_type, named("MetricPair", vec![]));
    }
    let transform = parsed
        .functions
        .iter()
        .find(|f| f.stable_id == "rename.transform")
        .unwrap();
    assert_eq!(transform.params[0].ty, named("MetricPair", vec![]));
    assert_eq!(transform.requires.len(), 1);
    assert_eq!(transform.ensures.len(), 1);
    let text = source(&candidate, "core");
    assert!(text.contains("inner: MetricPair"));
    assert!(text.contains("inner: MetricPair { value"));
    assert!(text.contains("let rebuilt: MetricPair = MetricPair"));
    assert!(text.contains("fn shadow<Pair>(Pair: Pair) -> Pair"));
    assert!(text.contains("let Pair = value"));
    assert!(text.contains("labels.Pair"));
    assert!(source(&candidate, "app").contains("as Metric;"));
    assert!(source(&candidate, "tests").contains("record Pair"));
    preserved(&base, &candidate);
    replay(&candidate);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
#[ignore = "repro at be5d3da: choose Signal::Some value shadowing"]
fn variant_owner_rename_preserves_case_payload_ids_and_order_and_imported_match_labels() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let candidate = apply(&base, rename("rename.choice", "Outcome")).unwrap();
    let text = source(&candidate, "core");
    assert!(text.contains("variant Outcome"));
    assert!(text.contains("choice: Outcome"));
    assert!(text.contains("Outcome::Some"));
    assert!(text.contains("Outcome::None"));
    assert!(!text.contains(" Choice::Some"));
    assert!(source(&candidate, "app").contains("Signal::Some"));
    let before = program(&base, "core");
    let after = program(&candidate, "core");
    let old = before
        .types
        .iter()
        .find(|t| t.stable_id == "rename.choice")
        .unwrap();
    let new = after
        .types
        .iter()
        .find(|t| t.stable_id == "rename.choice")
        .unwrap();
    assert_eq!(old.type_parameters, new.type_parameters);
    let members = |kind: &TypeDeclarationKind| match kind {
        TypeDeclarationKind::Variant { cases } => cases
            .iter()
            .map(|case| {
                (
                    case.stable_id.clone(),
                    case.name.clone(),
                    case.fields
                        .iter()
                        .map(|field| {
                            (
                                field.stable_id.clone(),
                                field.name.clone(),
                                field.ty.clone(),
                            )
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>(),
        _ => panic!("expected variant"),
    };
    assert_eq!(members(&old.kind), members(&new.kind));
    assert_eq!(
        members(&new.kind)
            .iter()
            .map(|case| case.0.as_str())
            .collect::<Vec<_>>(),
        ["rename.choice.some", "rename.choice.none"]
    );
    preserved(&base, &candidate);
    replay(&candidate);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
#[ignore = "repro at be5d3da: choose Signal::Some value shadowing"]
fn local_generic_record_and_variant_rename_preserve_concrete_arguments_and_shadowed_parameters() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let record = apply(&base, rename("rename.box", "Container")).unwrap();
    let candidate = apply(&record, rename("rename.generic-choice", "Selection")).unwrap();
    let parsed = program(&candidate, "core");
    let boxed = parsed
        .functions
        .iter()
        .find(|f| f.stable_id == "rename.boxed")
        .unwrap();
    assert_eq!(boxed.return_type, named("Container", vec![Type::I64]));
    assert!(source(&candidate, "core").contains("Container<i64> { value: value }"));
    assert!(source(&candidate, "core").contains("Selection<i64>::Some"));
    assert!(source(&candidate, "core").contains("Selection::Some"));
    assert!(source(&candidate, "core").contains("fn shadow<Pair>(Pair: Pair) -> Pair"));
    preserved(&base, &candidate);
    replay(&candidate);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
#[ignore = "repro at be5d3da: choose Signal::Some value shadowing"]
fn owned_record_rename_retains_own_signature_and_ordinary_cleanup_admission() {
    let fixture = Fixture::new();
    fixture.append(
        r#"
@id("rename.owned") record Owned { @id("rename.owned.bytes") bytes:Bytes, }
@id("rename.owned-make") fn owned_make(bytes:own Bytes)->Owned {Owned {bytes:bytes}}
@id("rename.owned-forward") fn owned_forward(value:own Owned)->Owned {value}
"#,
    );
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let candidate = apply(&base, rename("rename.owned", "Buffer")).unwrap();
    let parsed = program(&candidate, "core");
    let forward = parsed
        .functions
        .iter()
        .find(|f| f.stable_id == "rename.owned-forward")
        .unwrap();
    assert_eq!(forward.params[0].mode, semaprax::ast::ParamMode::Own);
    assert_eq!(forward.params[0].ty, named("Buffer", vec![]));
    assert_eq!(forward.return_type, named("Buffer", vec![]));
    assert!(source(&candidate, "core").contains("Buffer { bytes: bytes }"));
    preserved(&base, &candidate);
    replay(&candidate);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
#[ignore = "repro at be5d3da: choose Signal::Some value shadowing"]
fn collisions_nonowners_and_implicit_identity_reject_without_changing_sources() {
    let fixture = Fixture::new();
    fixture.append("\nrecord Occupied { flag:bool, }\n");
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let before = base.to_json().to_owned();
    let parsed = program(&base, "core");
    let implicit = parsed
        .types
        .iter()
        .find(|ty| ty.name == "Occupied")
        .unwrap();
    assert!(!implicit.explicit_id);
    for request in [
        rename("rename.pair", "Outer"),
        rename("rename.pair", "Occupied"),
        rename("rename.pair", "Option"),
        rename("rename.pair", "Pair"),
        rename("rename.pair", "Pair {}"),
        rename("rename.pair.value", "Different"),
        rename("rename.choice.some", "Different"),
        rename("core.option", "Different"),
        rename(&implicit.stable_id, "Different"),
    ] {
        assert!(apply(&base, request).is_err());
        assert_eq!(base.to_json(), before);
    }
    error(apply(&base, rename("rename.pair", "Pair {}")), "SPX-G225");
    error(
        apply(&base, rename("rename.pair.value", "Different")),
        "SPX-G225",
    );
    assert_eq!(fixture.bytes(), disk);
}

#[test]
#[ignore = "repro at be5d3da: choose Signal::Some value shadowing"]
fn nominal_rename_history_replays_and_rebases_without_reinterpreting_stale_revision() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let own = apply(&base, rename("rename.pair", "MetricPair")).unwrap();
    let other = apply(&base, rename("rename.public", "public_renamed")).unwrap();
    let rebased = own
        .rebase(
            own.candidate_digest(),
            Arc::clone(other.revision()),
            other.revision().project_revision(),
        )
        .unwrap();
    assert!(source(rebased.candidate(), "core").contains("record MetricPair"));
    assert!(source(rebased.candidate(), "core").contains("fn public_renamed("));
    replay(rebased.candidate());
    let competing = apply(&base, rename("rename.pair", "OtherPair")).unwrap();
    error(
        own.rebase(
            own.candidate_digest(),
            Arc::clone(competing.revision()),
            competing.revision().project_revision(),
        ),
        "SPX-G235",
    );
    let stale = SemanticChange::new(
        base.revision().project_revision(),
        &rename("rename.choice", "Outcome"),
    )
    .unwrap();
    error(own.apply(own.candidate_digest(), &stale), "SPX-G224");
    assert_eq!(fixture.bytes(), disk);
}
