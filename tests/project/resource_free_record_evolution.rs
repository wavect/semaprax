//! Resource-free record evolution regression source.
use semaprax::ast::{Expr, ExprKind, TypeDeclarationKind};
use semaprax::diagnostic::Diagnostic;
use semaprax::hir::{self, DeclarationId, ResolvedType, TypeFacts};
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
            "spx-resource-free-record-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let fixture = Self(root.canonicalize().unwrap());
        std::fs::write(
            fixture.0.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "resource-free-record"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "evolve.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["evolve.public"]
tests = ["evolve.tests"]
"#,
        )
        .unwrap();
        for (path, source) in [
            (
                "src/core.spx",
                r#"module evolve.core;
@id("evolve.text") record Text { @id("evolve.text.text") text:string, @id("evolve.text.marker") marker:i64, }
@id("evolve.nested") record Nested { @id("evolve.nested.child") child:Text, @id("evolve.nested.marker") marker:i64, }
@id("evolve.choice") variant Choice { @id("evolve.choice.none") None, @id("evolve.choice.some") Some { @id("evolve.choice.some.value") value:i64, }, }
@id("evolve.store") record Store { @id("evolve.store.raw") raw:[u8;2], @id("evolve.store.marker") marker:i64, }
@id("evolve.variant-store") record VariantStore { @id("evolve.variant-store.choice") choice:Choice, @id("evolve.variant-store.marker") marker:i64, }
@id("evolve.generic") record Generic<T> { @id("evolve.generic.value") value:T, }
@id("evolve.marker") fn marker()->i64 {7}
@id("evolve.make-text") fn make_text()->Text {Text {marker:marker(),text:"hello"}}
@id("evolve.make-nested") fn make_nested()->Nested {Nested {marker:marker(),child:make_text()}}
@id("evolve.make-store") fn make_store()->Store {Store {marker:marker(),raw:[7u8,8u8]}}
@id("evolve.make-variant-store") fn make_variant_store()->VariantStore {VariantStore {marker:marker(),choice:Choice::Some {value:marker()}}}
@id("evolve.public") fn public_value(value:i64)->i64 {value}
@id("evolve.evaluate") fn evaluate()->i64 {let store=make_store();let raw=store.raw;let view=array_as_slice(raw);if byte_len(view)==2usize && store.marker==7 {42}else{0}}
"#,
            ),
            (
                "src/app.spx",
                r#"module evolve.app;
use function @id("evolve.evaluate") from evolve.core as evaluate;
@id("evolve.main") fn main()->i64 {evaluate()}
"#,
            ),
            (
                "src/tests.spx",
                r#"module evolve.tests;
use function @id("evolve.evaluate") from evolve.core as evaluate;
@id("evolve.test") fn main()->i64 {if evaluate()==42 {0}else{1}}
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
fn request(target: &str, ty: &str, value: Value) -> Value {
    json!({"kind":"add_record_field","target":target,"field":{"id":format!("{target}.added"),"name":"added","type":ty,"default":{"kind":ty,"value":value}}})
}
fn apply(
    base: &ProjectCandidate,
    intent: Value,
) -> Result<(ProjectCandidate, SemanticChange), Vec<Diagnostic>> {
    let change = SemanticChange::new(base.revision().project_revision(), &intent)?;
    Ok((base.apply(base.candidate_digest(), &change)?, change))
}
fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    let errors = result.err().expect("expected rejected evolution");
    assert!(
        errors.iter().any(|error| error.code == expected),
        "{errors:?}"
    );
}
fn facts(candidate: &ProjectCandidate, target: &str) -> TypeFacts {
    // Re-resolve the exact admitted source to inspect unused declarations too;
    // this does not claim they belong to the entry target's executable closure.
    // The public standalone HIR resolver requires a main function. Add one only
    // to this analysis copy; neither the Project source nor its call graph is
    // changed, and all inspected type declarations retain their exact bytes.
    let analysis = format!(
        "{}\n@id(\"evolve.analysis-only\") fn main()->i64 {{0}}\n",
        source(candidate)
    );
    let parsed = semaprax::parse(&analysis, "src/core.spx").unwrap();
    hir::resolve(&parsed)
        .unwrap()
        .declarations
        .type_facts(&ResolvedType::Nominal {
            declaration: DeclarationId::new(target),
            arguments: vec![],
        })
        .unwrap()
}
fn flags(facts: &TypeFacts) -> (bool, bool, bool, bool) {
    (
        facts.copy,
        facts.needs_drop,
        facts.sized,
        facts.contains_resource,
    )
}
fn tail(mut value: &Expr) -> &Expr {
    while let ExprKind::Block { statements, tail } = &value.kind {
        assert!(statements.is_empty());
        value = tail;
    }
    value
}
fn assert_constructor(
    base: &ProjectCandidate,
    candidate: &ProjectCandidate,
    function: &str,
    added: &str,
) {
    let old = semaprax::parse(source(base), "src/core.spx").unwrap();
    let new = semaprax::parse(source(candidate), "src/core.spx").unwrap();
    let before = old
        .functions
        .iter()
        .find(|row| row.stable_id == function)
        .unwrap();
    let after = new
        .functions
        .iter()
        .find(|row| row.stable_id == function)
        .unwrap();
    let ExprKind::ConstructRecord { fields: before, .. } = &tail(&before.body).kind else {
        panic!("original constructor missing")
    };
    let ExprKind::ConstructRecord { fields: after, .. } = &tail(&after.body).kind else {
        panic!("migrated constructor missing")
    };
    assert_eq!(after.len(), before.len() + 1);
    for (old, new) in before.iter().zip(after) {
        assert_eq!(old.name, new.name);
        assert_eq!(
            semaprax::format::expr(&old.value, 0),
            semaprax::format::expr(&new.value, 0)
        );
    }
    assert_eq!(after.last().unwrap().name, "added");
    assert_eq!(
        semaprax::format::expr(&after.last().unwrap().value, 0),
        added
    );
}
fn replay(base: &ProjectCandidate, candidate: &ProjectCandidate, change: SemanticChange) {
    let rebuilt = ProjectCandidate::replay(
        Arc::clone(base.base_revision()),
        base.base_revision().project_revision(),
        &[change],
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
    assert_eq!(
        semaprax::format::canonical(&semaprax::parse(source(candidate), "src/core.spx").unwrap()),
        source(candidate)
    );
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
                .find(|source| source.path() == path)
                .unwrap()
                .source()
                .to_owned()
        };
        assert_eq!(text(candidate), text(base));
    }
}

#[test]
fn string_and_nested_string_record_source_constructors_append_defaults_without_claiming_entry_target_support(
) {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let before = base.to_json().to_owned();
    for (target, constructor) in [
        ("evolve.text", "evolve.make-text"),
        ("evolve.nested", "evolve.make-nested"),
    ] {
        let original = facts(&base, target);
        assert_eq!(flags(&original), (false, true, true, false));
        let catalog: Value = serde_json::from_str(&base.change_catalog(target).unwrap()).unwrap();
        let operation = catalog["operations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|operation| operation["kind"] == "add_record_field")
            .expect("checked owned record must expose scalar field evolution");
        assert_eq!(
            operation["field_types"],
            json!(["i64", "bool", "i32", "u8", "usize", "string", "Bytes"])
        );
        assert_eq!(
            operation["owning_field_lane"]["requires"],
            "original_copy_sized_drop_free_resource_free_record_with_authenticated_constructor_and_no_target_record_patterns"
        );
        assert!(operation["constraints"]
            .as_array()
            .unwrap()
            .contains(&json!("monomorphic_checked_sized_resource_free_record")));
        let (candidate, change) = apply(&base, request(target, "i32", json!(-7))).unwrap();
        assert_eq!(flags(&facts(&candidate, target)), flags(&original));
        assert_constructor(&base, &candidate, constructor, "-7i32");
        let old = semaprax::parse(source(&base), "src/core.spx").unwrap();
        let new = semaprax::parse(source(&candidate), "src/core.spx").unwrap();
        let old = old
            .types
            .iter()
            .find(|row| row.stable_id == target)
            .unwrap();
        let new = new
            .types
            .iter()
            .find(|row| row.stable_id == target)
            .unwrap();
        let TypeDeclarationKind::Record { fields: old } = &old.kind else {
            panic!("old record missing")
        };
        let TypeDeclarationKind::Record { fields: new } = &new.kind else {
            panic!("new record missing")
        };
        for (old, new) in old.iter().zip(new) {
            assert_eq!(old.stable_id, new.stable_id);
            assert_eq!(old.name, new.name);
            assert_eq!(old.ty, new.ty);
        }
        assert_eq!(new.len(), old.len() + 1);
        assert_eq!(new.last().unwrap().stable_id, format!("{target}.added"));
        let graph: Value = serde_json::from_str(candidate.revision().semantic_graph()).unwrap();
        let added = graph["declarations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == format!("{target}.added"))
            .unwrap();
        assert_eq!(added["owner"], target);
        assert_eq!(added["kind"], "field");
        assert_eq!(added["identity_origin"], "explicit");
        // Both constructors are checked source declarations, but deliberately
        // absent from the scalar entry closure; no String-aggregate Wasm claim.
        assert!(!candidate
            .revision()
            .entry_program()
            .functions
            .iter()
            .any(|row| row.id.as_str() == constructor));
        replay(&base, &candidate, change);
    }
    assert_eq!(base.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn copy_variant_storage_constructor_migrates_outside_the_entry_target_closure() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let before = base.to_json().to_owned();
    let original = facts(&base, "evolve.variant-store");
    assert_eq!(flags(&original), (true, false, true, false));
    let (candidate, change) =
        apply(&base, request("evolve.variant-store", "bool", json!(false))).unwrap();
    assert_constructor(&base, &candidate, "evolve.make-variant-store", "false");
    assert_eq!(
        flags(&facts(&candidate, "evolve.variant-store")),
        flags(&original)
    );
    // Source checking permits this storage. The selected aggregate target does
    // not claim record-contained variant layout, so keep its constructor out
    // of the executable closure before and after source migration.
    for revision in [&base, &candidate] {
        assert!(!revision
            .revision()
            .entry_program()
            .functions
            .iter()
            .any(|row| row.id.as_str() == "evolve.make-variant-store"));
    }
    replay(&base, &candidate, change);
    assert_eq!(base.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn inline_array_storage_preserves_constructor_order_and_checked_entry_plans() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let original = facts(&base, "evolve.store");
    assert_eq!(flags(&original), (true, false, true, false));
    let (candidate, change) =
        apply(&base, request("evolve.store", "usize", json!(u64::MAX))).unwrap();
    assert_constructor(
        &base,
        &candidate,
        "evolve.make-store",
        "18446744073709551615usize",
    );
    assert_eq!(flags(&facts(&candidate, "evolve.store")), flags(&original));
    let find = |candidate: &ProjectCandidate| {
        candidate
            .revision()
            .entry_program()
            .functions
            .iter()
            .find(|row| row.id.as_str() == "evolve.evaluate")
            .unwrap()
            .clone()
    };
    let old = find(&base);
    let new = find(&candidate);
    // Copy arrays do not select the owning-root loan plan merely because a
    // local array_as_slice view exists. Do not invent an owning obligation.
    assert!(old.loan_plan.loans.is_empty());
    assert_eq!(old.loan_plan, new.loan_plan);
    assert_eq!(old.cleanup, new.cleanup);
    assert_eq!(old.cleanup_plan, new.cleanup_plan);
    let checked = candidate
        .revision()
        .entry_program()
        .declarations
        .type_facts(&ResolvedType::Nominal {
            declaration: DeclarationId::new("evolve.store"),
            arguments: vec![],
        })
        .unwrap();
    assert_eq!(flags(&checked), flags(&original));
    replay(&base, &candidate, change);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn generic_nonrecord_owned_defaults_collision_range_and_field_count_limits_remain_closed() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let before = base.to_json().to_owned();
    for target in ["evolve.generic", "evolve.choice", "evolve.public"] {
        code(apply(&base, request(target, "i64", json!(0))), "SPX-G225");
    }
    for (ty, value) in [
        ("Bytes", json!([])),
        ("string", json!("owned")),
        ("u8", json!(256)),
        ("i32", json!(i64::MAX)),
    ] {
        code(apply(&base, request("evolve.text", ty, value)), "SPX-G225");
    }
    let mut wrong = request("evolve.text", "i64", json!(0));
    wrong["field"]["default"]["kind"] = json!("usize");
    code(apply(&base, wrong), "SPX-G225");
    let mut duplicate = request("evolve.text", "i64", json!(0));
    duplicate["field"]["id"] = json!("evolve.nested.child");
    code(apply(&base, duplicate), "SPX-G225");
    let change = SemanticChange::new(
        base.revision().project_revision(),
        &request("evolve.text", "bool", json!(true)),
    )
    .unwrap();
    code(
        base.apply(&format!("sha256:{}", "0".repeat(64)), &change),
        "SPX-G224",
    );
    assert_eq!(base.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
    let mut wide = source(&base).to_owned();
    wide.push_str("\n@id(\"evolve.wide\") record Wide {\n");
    for index in 0..64 {
        wide.push_str(&format!("@id(\"evolve.wide.f{index}\") f{index}:i64,\n"));
    }
    wide.push_str("}\n");
    fixture.write("src/core.spx", &wide);
    let wide_base = fixture.candidate();
    let wide_disk = fixture.bytes();
    code(
        apply(&wide_base, request("evolve.wide", "i64", json!(0))),
        "SPX-G226",
    );
    assert_eq!(fixture.bytes(), wide_disk);
}

#[test]
fn broader_candidate_selection_accepts_nested_owned_patterns_but_keeps_unsupported_storage_closed()
{
    let fixture = Fixture::new();
    let original = std::fs::read_to_string(fixture.0.join("src/core.spx")).unwrap();
    for (extra,expected) in [
        ("@id(\"evolve.inner-bytes\") record InnerBytes { @id(\"evolve.inner-bytes.bytes\") bytes:Bytes, }\n@id(\"evolve.outer-bytes\") record OuterBytes { @id(\"evolve.outer-bytes.inner\") inner:InnerBytes, }\nfn invalid(value:own OuterBytes)->i64 {match own value {OuterBytes {inner:InnerBytes {bytes}}=>0,}}", "SPX-T268"),
        ("@id(\"evolve.borrowed\") record Borrowed { @id(\"evolve.borrowed.value\") value:Slice<u8>, }", "SPX-T264"),
        ("@id(\"evolve.borrowed\") record Borrowed { @id(\"evolve.borrowed.value\") value:str, }", "SPX-O116"),
    ] {
        fixture.write("src/core.spx",&format!("{original}\n{extra}\n"));let disk=fixture.bytes();
        let result=with_authenticated_project(&fixture.0.join("semaprax.toml"),|snapshot|ProjectCandidate::open(snapshot.retain_revision(),snapshot.project_revision()));
        code(result,expected);assert_eq!(fixture.bytes(),disk);
    }
}
