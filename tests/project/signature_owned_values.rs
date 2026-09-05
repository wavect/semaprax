//! Ordered resource-free owning signatures and the bounded module-import boundary.
use semaprax::ast::{Expr, ExprKind, Function, ParamMode, Program, Statement};
use semaprax::diagnostic::Diagnostic;
use semaprax::hir::OwnershipMode;
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
            "spx-owned-signature-values-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "owned-signature-values"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "owned.app"
sources = ["src/app.spx", "src/bridge.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["owned.public"]
tests = ["owned.tests"]
"#,
        )
        .unwrap();
        for (path, source) in [
            (
                "src/core.spx",
                r#"module owned.core;
@id("owned.packet") record Packet { @id("owned.packet.bytes") bytes:Bytes, @id("owned.packet.marker") marker:i64, }
@id("owned.choice") variant Choice { @id("owned.choice.none") None, @id("owned.choice.data") Data { @id("owned.choice.data.bytes") bytes:Bytes, @id("owned.choice.data.marker") marker:i64, }, }
@id("owned.text-select") fn text_select(left:string,right:string,flag:i64)->string {if flag==0 {left}else{right}}
@id("owned.left-text") fn left_text()->string {"left"}
@id("owned.right-text") fn right_text()->string {"right"}
@id("owned.text-call") fn text_call()->string {text_select(left_text(),right_text(),4/2)}
@id("owned.record-select") fn record_select(left:own Packet,right:own Packet,flag:i64)->Packet {if flag==0 {left}else{right}}
@id("owned.record-call") fn record_call(input:borrow Slice<u8>)->Packet {record_select(Packet {marker:8/2,bytes:bytes_copy(input)},Packet {bytes:bytes_copy(input),marker:18/3},4/2)}
@id("owned.variant-select") fn variant_select(left:own Choice,right:own Choice,flag:i64)->Choice {let marker=flag+consume_variant(left);right}
@id("owned.variant-call") fn variant_call(input:borrow Slice<u8>)->Choice {variant_select(Choice::Data {marker:12/3,bytes:bytes_copy(input)},Choice::Data {bytes:bytes_copy(input),marker:24/4},4/2)}
@id("owned.second-text") fn second_text()->string {text_select("left","right",4/2)}
@id("owned.second-record") fn second_record(input:borrow Slice<u8>)->Packet {record_select(Packet {bytes:bytes_copy(input),marker:8/2},Packet {marker:18/3,bytes:bytes_copy(input)},4/2)}
@id("owned.second-variant") fn second_variant(input:borrow Slice<u8>)->Choice {variant_select(Choice::Data {bytes:bytes_copy(input),marker:12/3},Choice::Data {marker:24/4,bytes:bytes_copy(input)},4/2)}
@id("owned.consume-record") fn consume_record(value:own Packet)->i64 {match own value {Packet {bytes,marker}=>marker,}}
@id("owned.consume-variant") fn consume_variant(value:own Choice)->i64 {match own value {Choice::None {}=>0,Choice::Data {bytes,marker}=>marker,}}
@id("owned.borrow-record") fn borrow_record(value:borrow Packet)->i64 {0}
@id("owned.borrow-variant") fn borrow_variant(value:borrow Choice)->i64 {0}
@id("owned.borrow-view") fn borrow_view(value:borrow Slice<u8>)->i64 {0}
@id("owned.bytes") fn bytes(value:own Bytes)->Bytes {value}
@id("owned.public") fn public_value(value:i64)->i64 {value}
@id("owned.evaluate") fn evaluate()->i64 {let input=[1u8];if consume_record(record_call(array_as_slice(input)))==6 && consume_variant(variant_call(array_as_slice(input)))==6 {42}else{0}}
@id("owned.evaluate-second") fn evaluate_second()->i64 {let input=[1u8];if consume_record(second_record(array_as_slice(input)))==6 && consume_variant(second_variant(array_as_slice(input)))==6 {42}else{0}}
"#,
            ),
            (
                "src/bridge.spx",
                r#"module owned.bridge;
use function @id("owned.evaluate-second") from owned.core as evaluate_local_owners;
@id("owned.bridge") fn evaluate()->i64 {evaluate_local_owners()}
"#,
            ),
            (
                "src/app.spx",
                r#"module owned.app;
use function @id("owned.evaluate") from owned.core as evaluate;
use function @id("owned.bridge") from owned.bridge as bridge_evaluate;
@id("owned.main") fn main()->i64 {if bridge_evaluate()==42 {evaluate()}else{0}}
"#,
            ),
            (
                "src/tests.spx",
                r#"module owned.tests;
use function @id("owned.evaluate") from owned.core as evaluate;
@id("owned.test") fn main()->i64 {if evaluate()==42 {0}else{1}}
"#,
            ),
        ] {
            let program = semaprax::parse(source, path).unwrap();
            std::fs::write(root.join(path), semaprax::format::canonical(&program)).unwrap();
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
            "src/bridge.spx",
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
fn function<'a>(program: &'a Program, target: &str) -> &'a Function {
    program
        .functions
        .iter()
        .find(|f| f.stable_id == target)
        .unwrap()
}
fn tail(mut value: &Expr) -> &Expr {
    while let ExprKind::Block { statements, tail } = &value.kind {
        if !statements.is_empty() {
            break;
        }
        value = tail;
    }
    value
}
fn evolve(
    base: &ProjectCandidate,
    target: &str,
    parameters: Value,
) -> Result<(ProjectCandidate, SemanticChange), Vec<Diagnostic>> {
    let change = SemanticChange::new(
        base.revision().project_revision(),
        &json!({"kind":"change_function_signature","target":target,"parameters":parameters}),
    )?;
    Ok((base.apply(base.candidate_digest(), &change)?, change))
}
fn mapping() -> Value {
    json!([{"from":"right","name":"second"},{"from":"flag"},{"from":"left","name":"first"}])
}
fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    let errors = result.err().expect("expected rejection");
    assert!(errors.iter().any(|e| e.code == expected), "{errors:?}");
}
fn staging(before: &ProjectCandidate, after: &ProjectCandidate, path: &str, caller: &str) {
    let old = semaprax::parse(source(before, path), path).unwrap();
    let new = semaprax::parse(source(after, path), path).unwrap();
    let ExprKind::Call { name, args, .. } = &tail(&function(&old, caller).body).kind else {
        panic!("original direct call missing")
    };
    let ExprKind::Block {
        statements,
        tail: call,
    } = &tail(&function(&new, caller).body).kind
    else {
        panic!("migrated call lacks staging")
    };
    assert_eq!(statements.len(), args.len());
    let mut locals = Vec::new();
    for (statement, original) in statements.iter().zip(args) {
        let Statement::Let {
            name,
            value,
            mutable,
            ..
        } = statement
        else {
            panic!("staging must use let")
        };
        assert!(!mutable);
        assert_eq!(
            semaprax::format::expr(value, 0),
            semaprax::format::expr(original, 0)
        );
        locals.push(name.clone());
    }
    let ExprKind::Call {
        name: actual,
        args: ordered,
        ..
    } = &call.kind
    else {
        panic!("staging must end in one call")
    };
    assert_eq!(actual, name);
    assert_eq!(ordered.len(), 3);
    for (argument, index) in ordered.iter().zip([1, 2, 0]) {
        assert_eq!(argument.kind, ExprKind::Var(locals[index].clone()));
    }
}
fn replay(base: &ProjectCandidate, candidate: &ProjectCandidate, change: SemanticChange) {
    let replayed = ProjectCandidate::replay(
        Arc::clone(base.base_revision()),
        base.base_revision().project_revision(),
        &[change],
        candidate.to_json().as_bytes(),
    )
    .unwrap();
    let restored = ProjectCandidate::restore(
        Arc::clone(base.base_revision()),
        base.base_revision().project_revision(),
        candidate.recovery_capsule().unwrap().as_bytes(),
    )
    .unwrap();
    for rebuilt in [replayed, restored] {
        assert_eq!(rebuilt.to_json(), candidate.to_json());
        assert_eq!(
            rebuilt.revision().semantic_graph(),
            candidate.revision().semantic_graph()
        );
    }
}
fn catalog(base: &ProjectCandidate, target: &str) -> Value {
    serde_json::from_str(&base.change_catalog(target).unwrap()).unwrap()
}
fn ordered(report: &Value) -> bool {
    report["operations"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|op| op["kind"] == "change_function_signature")
        .any(|op| {
            op["exactly_one_form"]
                .as_array()
                .unwrap()
                .iter()
                .any(|form| form["selector"] == "parameters")
        })
}

#[test]
fn strings_and_owned_records_and_variants_reorder_each_local_call_once_and_replay() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    for (target, local, second, mode) in [
        (
            "owned.text-select",
            "owned.text-call",
            "owned.second-text",
            ParamMode::Value,
        ),
        (
            "owned.record-select",
            "owned.record-call",
            "owned.second-record",
            ParamMode::Own,
        ),
        (
            "owned.variant-select",
            "owned.variant-call",
            "owned.second-variant",
            ParamMode::Own,
        ),
    ] {
        let (candidate, change) = evolve(&base, target, mapping()).unwrap();
        let old = semaprax::parse(source(&base, "src/core.spx"), "src/core.spx").unwrap();
        let new = semaprax::parse(source(&candidate, "src/core.spx"), "src/core.spx").unwrap();
        let old = function(&old, target);
        let new = function(&new, target);
        assert_eq!(new.stable_id, old.stable_id);
        assert_eq!(
            new.params
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>(),
            ["second", "flag", "first"]
        );
        assert_eq!(new.params[0].mode, mode);
        assert_eq!(new.params[2].mode, mode);
        assert_eq!(new.params[0].ty, old.params[1].ty);
        assert_eq!(new.params[2].ty, old.params[0].ty);
        assert_eq!(new.return_type, old.return_type);
        // String declarations are checked and retained but are not reachable in
        // this Project's v6 aggregate executable closure. Check their exact
        // compiler-authenticated facts through the retained catalog as well.
        let migrated_catalog = catalog(&candidate, target);
        for (index, name) in [(0, "second"), (2, "first")] {
            let parameter = &migrated_catalog["parameters"][index];
            assert_eq!(parameter["name"], name);
            assert_eq!(parameter["type_provenance"]["ownership"], "own");
            assert_eq!(
                parameter["type_provenance"]["evidence_owner"],
                "retained_checked_hir"
            );
        }
        if target != "owned.text-select" {
            let checked = candidate
                .revision()
                .entry_program()
                .functions
                .iter()
                .find(|function| function.id.as_str() == target)
                .unwrap();
            assert_eq!(checked.params[0].ownership, OwnershipMode::Own);
            assert_eq!(checked.params[2].ownership, OwnershipMode::Own);
            assert_eq!(checked.params[0].name, "second");
            assert_eq!(checked.params[2].name, "first");
        }
        assert_eq!(
            candidate
                .revision()
                .entry_program()
                .functions
                .iter()
                .any(|function| function.id.as_str() == target),
            target != "owned.text-select"
        );
        let renamed_body = semaprax::format::expr(&new.body, 0);
        if target == "owned.variant-select" {
            assert!(renamed_body.contains("let marker = flag + consume_variant(first);"));
            assert!(renamed_body.ends_with("second }"));
        } else {
            assert!(renamed_body.contains("if flag == 0 { first } else { second }"));
        }
        staging(&base, &candidate, "src/core.spx", local);
        staging(&base, &candidate, "src/core.spx", second);
        assert_eq!(
            source(&base, "src/bridge.spx"),
            source(&candidate, "src/bridge.spx")
        );
        code(
            candidate.apply(candidate.candidate_digest(), &change),
            "SPX-G224",
        );
        replay(&base, &candidate, change);
    }
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn owned_type_imports_are_admitted_but_owned_argument_function_imports_remain_closed() {
    for (import, target, admitted) in [
        (
            "use type @id(\"owned.packet\") from owned.core as Frame;",
            "owned.packet",
            true,
        ),
        (
            "use type @id(\"owned.choice\") from owned.core as Signal;",
            "owned.choice",
            true,
        ),
        (
            "use function @id(\"owned.record-select\") from owned.core as select_frame;",
            "owned.record-select",
            false,
        ),
        (
            "use function @id(\"owned.variant-select\") from owned.core as select_signal;",
            "owned.variant-select",
            false,
        ),
    ] {
        let fixture = Fixture::new();
        let _base = fixture.candidate();
        let bridge = format!(
            "module owned.bridge;\n{import}\n@id(\"owned.bridge\") fn evaluate()->i64 {{42}}\n"
        );
        let parsed = semaprax::parse(&bridge, "src/bridge.spx").unwrap();
        std::fs::write(
            fixture.0.join("src/bridge.spx"),
            semaprax::format::canonical(&parsed),
        )
        .unwrap();
        let disk = fixture.bytes();
        let opened = with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.semantic_graph().to_owned())
        });
        if admitted {
            let graph = opened.unwrap();
            assert!(graph.contains(&format!("\"target\":\"{target}\"")));
            assert!(graph.contains("\"kind\":\"type_import\""));
        } else {
            code(opened, "SPX-G172");
        }
        assert_eq!(fixture.bytes(), disk);
    }
}

#[test]
fn asymmetric_conditional_variant_owners_remain_outside_cleanup_plan_admission() {
    let fixture = Fixture::new();
    let _base = fixture.candidate();
    let path = fixture.0.join("src/core.spx");
    let source = std::fs::read_to_string(&path).unwrap()
        + "\n@id(\"owned.asymmetric\") fn asymmetric(left:own Choice,right:own Choice,flag:bool)->Choice {if flag {left}else{right}}\n";
    let parsed = semaprax::parse(&source, "src/core.spx").unwrap();
    std::fs::write(path, semaprax::format::canonical(&parsed)).unwrap();
    let disk = fixture.bytes();
    let errors =
        with_authenticated_project(&fixture.0.join("semaprax.toml"), |_| Ok(())).unwrap_err();
    assert!(
        errors.iter().any(|error| error.code == "SPX-H006"
            && error
                .message
                .contains("branch join disagrees on conditional variant roots")),
        "{errors:?}"
    );
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn every_original_owner_must_be_retained_once_including_implicit_string_ownership() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let before = base.to_json().to_owned();
    for target in [
        "owned.text-select",
        "owned.record-select",
        "owned.variant-select",
    ] {
        for parameters in [
            json!([{"from":"left"},{"from":"flag"}]),
            json!([{"from":"right"},{"from":"flag"}]),
        ] {
            code(evolve(&base, target, parameters), "SPX-G260");
        }
        code(
            evolve(
                &base,
                target,
                json!([{"from":"left"},{"from":"right"},{"from":"right"},{"from":"flag"}]),
            ),
            "SPX-G225",
        );
        code(
            evolve(
                &base,
                target,
                json!([{"from":"left","name":"same"},{"from":"right","name":"same"},{"from":"flag"}]),
            ),
            "SPX-G225",
        );
        assert_eq!(base.to_json(), before);
    }
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn catalog_authenticates_owning_facts_and_admits_only_exact_borrowed_views() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    for (target, declaration, mode) in [
        ("owned.text-select", Value::Null, "value"),
        ("owned.record-select", json!("owned.packet"), "own"),
        ("owned.variant-select", json!("owned.choice"), "own"),
    ] {
        let report = catalog(&base, target);
        assert!(ordered(&report));
        assert_eq!(report["admission"], "constructor_discovery_only");
        assert_eq!(report["requires_full_candidate_validation"], true);
        for index in [0, 1] {
            let parameter = &report["parameters"][index];
            assert_eq!(parameter["mode"], mode);
            let facts = &parameter["type_provenance"];
            assert_eq!(facts["declaration"], declaration);
            assert_eq!(facts["arguments"], json!([]));
            assert_eq!(facts["ownership"], "own");
            assert_eq!(facts["evidence_owner"], "retained_checked_hir");
            assert_eq!(facts["copy"], false);
            assert_eq!(facts["sized"], true);
            assert_eq!(facts["contains_resource"], false);
            assert_eq!(facts["needs_drop"], true);
            if target == "owned.text-select" {
                assert_eq!(parameter["type"], "string");
                assert_eq!(parameter["type_identity"], "string");
            } else {
                assert!(parameter["type_identity"]
                    .as_str()
                    .unwrap()
                    .starts_with("nominal:"));
            }
        }
    }
    for target in ["owned.borrow-record", "owned.borrow-variant"] {
        let report = catalog(&base, target);
        assert!(!ordered(&report));
        code(
            evolve(&base, target, json!([{"from":"value","name":"renamed"}])),
            "SPX-G225",
        );
    }
    let borrowed = catalog(&base, "owned.borrow-view");
    assert!(ordered(&borrowed));
    assert_eq!(
        borrowed["parameters"],
        json!([{
            "name":"value", "type":"Slice<u8>", "mode":"borrow"
        }])
    );
    let (renamed, _) = evolve(
        &base,
        "owned.borrow-view",
        json!([{"from":"value","name":"renamed"}]),
    )
    .unwrap();
    assert!(source(&renamed, "src/core.spx").contains("fn borrow_view(renamed: borrow Slice<u8>)"));
    assert_eq!(
        catalog(&base, "owned.bytes")["parameters"],
        json!([{"name":"value","type":"Bytes","mode":"own"}])
    );
    assert_eq!(
        catalog(&base, "owned.public")["parameters"],
        json!([{"name":"value","type":"i64","mode":"value"}])
    );
    assert_eq!(fixture.bytes(), disk);
}
