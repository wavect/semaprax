//! Owned movement boundaries: authored regressions, not local execution evidence.
use semaprax::ast::{ExprKind, ModuleUseKind, ParamMode, Program, Type};
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{with_authenticated_project, ProjectCandidate, SemanticChange};
use serde_json::{json, Value};
use std::collections::BTreeMap;
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
            "spx-owned-move-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "owned-movement"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "relocate.app"
sources = ["src/app.spx", "src/core.spx", "src/support.spx", "src/tests.spx", "src/util.spx"]
web_exports = ["relocate.public"]
tests = ["relocate.tests"]
"#,
        )
        .unwrap();
        let fixture = Self(root.canonicalize().unwrap());
        for (module, text) in [
            (
                "core",
                r#"module relocate.core;
use function @id("relocate.select") from relocate.util as choose;
@id("relocate.packet") record Packet { @id("relocate.packet.bytes") bytes:Bytes, @id("relocate.packet.marker") marker:i64, }
@id("relocate.choice") variant Choice { @id("relocate.choice.none") None, @id("relocate.choice.data") Data { @id("relocate.choice.data.bytes") bytes:Bytes, @id("relocate.choice.data.marker") marker:i64, }, }
@id("relocate.text") fn text(left:string,right:string)->string {choose(left,right,4/2)}
@id("relocate.text-call") fn text_call()->string {text("left","right")}
@id("relocate.bytes") fn bytes(value:own Bytes)->Bytes {value}
@id("relocate.byte-work") fn byte_work()->usize {let input=[1u8,2u8];let input_view=array_as_slice(input);let bytes=bytes_copy(input_view);let view=bytes_as_slice(bytes);byte_len(view)}
@id("relocate.record") fn record_value(value:own Packet)->Packet {value}
@id("relocate.variant") fn variant_value(value:own Choice)->Choice {value}
@id("relocate.record-match") fn record_match(value:own Packet)->i64 {match own value {Packet {bytes,marker}=>marker,}}
@id("relocate.variant-match") fn variant_match(value:own Choice)->i64 {match own value {Choice::None {}=>0,Choice::Data {bytes,marker}=>marker,}}
@id("relocate.borrow") fn borrowed(value:borrow Slice<u8>)->usize {byte_len(value)}
@id("relocate.public") fn public_value(value:i64)->i64 {value}
@id("relocate.evaluate") fn evaluate()->i64 {if text_call()=="right" && byte_work()==2usize {42}else{0}}
"#,
            ),
            (
                "support",
                r#"module relocate.support;
use function @id("relocate.text") from relocate.core as via_text;
use function @id("relocate.select") from relocate.util as select_text;
@id("relocate.destination") fn destination(value:i64)->i64 {value}
@id("relocate.destination-call") fn destination_call()->string {via_text("a","b")}
"#,
            ),
            (
                "util",
                r#"module relocate.util;
@id("relocate.select") fn select(left:string,right:string,flag:i64)->string {if flag==0 {left}else{right}}
"#,
            ),
            (
                "app",
                r#"module relocate.app;
use function @id("relocate.evaluate") from relocate.core as evaluate;
use function @id("relocate.destination-call") from relocate.support as destination_call;
@id("relocate.main") fn main()->i64 {if destination_call()=="b" {evaluate()}else{0}}
"#,
            ),
            (
                "tests",
                r#"module relocate.tests;
use function @id("relocate.evaluate") from relocate.core as evaluate;
@id("relocate.test") fn main()->i64 {if evaluate()==42 {0}else{1}}
"#,
            ),
        ] {
            fixture.write(module, text);
        }
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
            "src/util.spx",
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
fn movement(target: &str) -> Value {
    json!({"kind":"move_declaration","target":target,"destination":"relocate.destination"})
}
fn apply(base: &ProjectCandidate, intent: &Value) -> Result<ProjectCandidate, Vec<Diagnostic>> {
    base.apply(
        base.candidate_digest(),
        &SemanticChange::new(base.revision().project_revision(), intent)?,
    )
}
fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    let errors = result.err().expect("expected rejection");
    assert!(errors.iter().any(|e| e.code == expected), "{errors:?}");
}
fn replay(candidate: &ProjectCandidate) {
    let report: Value = serde_json::from_str(candidate.to_json()).unwrap();
    let changes = report["changes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|change| {
            SemanticChange::new(change["base_revision"].as_str().unwrap(), &change["intent"])
                .unwrap()
        })
        .collect::<Vec<_>>();
    let replayed = ProjectCandidate::replay(
        Arc::clone(candidate.base_revision()),
        candidate.base_revision().project_revision(),
        &changes,
        candidate.to_json().as_bytes(),
    )
    .unwrap();
    let restored = ProjectCandidate::restore(
        Arc::clone(candidate.base_revision()),
        candidate.base_revision().project_revision(),
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
fn calls(candidate: &ProjectCandidate) -> BTreeMap<String, Vec<String>> {
    let mut inventory = BTreeMap::new();
    for source in candidate.revision().sources() {
        let program = semaprax::parse(source.source(), source.path()).unwrap();
        let mut names = program
            .functions
            .iter()
            .map(|f| (f.name.clone(), f.stable_id.clone()))
            .collect::<BTreeMap<_, _>>();
        names.extend(
            program
                .module_uses
                .iter()
                .filter(|u| u.kind == ModuleUseKind::Function)
                .map(|u| (u.alias.clone(), u.persistent_id.clone())),
        );
        for function in &program.functions {
            let mut ordered = Vec::new();
            function.body.visit_calls(&mut |name, _| {
                ordered.push(
                    names
                        .get(name)
                        .cloned()
                        .unwrap_or_else(|| format!("compiler:{name}")),
                )
            });
            inventory.insert(function.stable_id.clone(), ordered);
        }
    }
    inventory
}

#[test]
fn string_parameters_results_callees_and_aliases_move_with_exact_argument_order() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let moved = apply(&base, &movement("relocate.text")).unwrap();
    let destination = program(&moved, "support");
    let function = destination
        .functions
        .iter()
        .find(|f| f.stable_id == "relocate.text")
        .unwrap();
    assert!(function.explicit_id);
    assert_eq!(function.name, "text");
    assert_eq!(function.return_type, Type::String);
    assert_eq!(
        function
            .params
            .iter()
            .map(|p| (p.name.as_str(), p.mode, p.ty.clone()))
            .collect::<Vec<_>>(),
        vec![
            ("left", ParamMode::Value, Type::String),
            ("right", ParamMode::Value, Type::String)
        ]
    );
    let ExprKind::Block { statements, tail } = &function.body.kind else {
        panic!("body block missing")
    };
    assert!(statements.is_empty());
    let ExprKind::Call { name, args, .. } = &tail.kind else {
        panic!("direct checked callee missing")
    };
    assert_eq!(name, "select_text");
    assert_eq!(
        args.iter()
            .map(|value| semaprax::format::expr(value, 0))
            .collect::<Vec<_>>(),
        ["left", "right", "4 / 2"]
    );
    assert!(!destination
        .module_uses
        .iter()
        .any(|u| u.persistent_id == "relocate.text"));
    assert!(source(&moved, "support").contains("text(\"a\", \"b\")"));
    assert!(source(&moved, "core")
        .contains("use function @id(\"relocate.text\") from relocate.support as text;"));
    assert!(!source(&moved, "core").contains("fn text("));
    assert_eq!(calls(&base), calls(&moved));
    let graph: Value = serde_json::from_str(moved.revision().semantic_graph()).unwrap();
    let identity = graph["declarations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["id"] == "relocate.text")
        .unwrap();
    assert_eq!(identity["path"], "src/support.spx");
    assert_eq!(identity["identity_origin"], "explicit");
    let stale = SemanticChange::new(
        base.revision().project_revision(),
        &movement("relocate.text"),
    )
    .unwrap();
    code(moved.apply(base.candidate_digest(), &stale), "SPX-G224");
    replay(&moved);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn scalar_signature_byte_work_keeps_compiler_operations_loans_and_cleanup_in_place() {
    let fixture = Fixture::new();
    // Avoid a pre-existing destination->source edge unrelated to byte_work;
    // no source behavior is changed to repair an attempted movement cycle.
    fixture.write("support","module relocate.support; @id(\"relocate.destination\") fn destination(value:i64)->i64 {value} @id(\"relocate.destination-call\") fn destination_call()->string {\"b\"}");
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let moved = apply(&base, &movement("relocate.byte-work")).unwrap();
    let before = base
        .revision()
        .entry_program()
        .functions
        .iter()
        .find(|f| f.id.as_str() == "relocate.byte-work")
        .unwrap();
    let after = moved
        .revision()
        .entry_program()
        .functions
        .iter()
        .find(|f| f.id.as_str() == "relocate.byte-work")
        .unwrap();
    assert!(!before.loan_plan.loans.is_empty());
    assert_eq!(before.loan_plan, after.loan_plan);
    assert_eq!(before.cleanup_plan, after.cleanup_plan);
    let old = program(&base, "core");
    let new = program(&moved, "support");
    let old = old
        .functions
        .iter()
        .find(|f| f.stable_id == "relocate.byte-work")
        .unwrap();
    let new = new
        .functions
        .iter()
        .find(|f| f.stable_id == "relocate.byte-work")
        .unwrap();
    assert_eq!(
        semaprax::format::expr(&old.body, 0),
        semaprax::format::expr(&new.body, 0)
    );
    assert_eq!(calls(&base), calls(&moved));
    let moved = apply(&moved, &movement("relocate.bytes")).unwrap();
    let support = program(&moved, "support");
    let bytes = support
        .functions
        .iter()
        .find(|f| f.stable_id == "relocate.bytes")
        .unwrap();
    assert_eq!(bytes.params[0].mode, ParamMode::Own);
    assert_eq!(bytes.params[0].ty, Type::Bytes);
    assert_eq!(bytes.return_type, Type::Bytes);
    assert!(!program(&moved, "core")
        .module_uses
        .iter()
        .any(|u| u.persistent_id == "relocate.bytes"));
    replay(&moved);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn owned_nominal_imports_and_real_module_cycles_remain_full_project_rejections() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let before = base.to_json().to_owned();
    for target in [
        "relocate.record",
        "relocate.variant",
        "relocate.record-match",
        "relocate.variant-match",
    ] {
        code(apply(&base, &movement(target)), "SPX-G172");
        assert_eq!(base.to_json(), before);
    }
    for target in ["relocate.borrow", "relocate.public", "relocate.main"] {
        code(apply(&base, &movement(target)), "SPX-G225");
        assert_eq!(base.to_json(), before);
    }
    // Existing support->core text import plus new core->support byte_work
    // import is a real cycle, even though the relocated signature is scalar.
    code(apply(&base, &movement("relocate.byte-work")), "SPX-G172");
    assert_eq!(base.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn audited_sources_are_rejected_by_project_admission_before_movement_is_available() {
    let fixture = Fixture::new();
    let source = std::fs::read_to_string(fixture.0.join("src/core.spx"))
        .unwrap()
        .replacen(
            "module relocate.core;",
            "module relocate.core;\npermit { unsafe }",
            1,
        );
    fixture.write("core", &source);
    fixture.append("core",r#"
@id("relocate.audited") fn audited()->i64 {let mut value=0; @audit("arithmetic-only movement rejection fixture") unsafe {value=42;0} value}
"#);
    let disk = fixture.bytes();
    // The Project permit fence rejects this audit fixture before a candidate
    // exists. This does not claim that the movement audit guard was reached.
    code(
        with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        }),
        "SPX-G172",
    );
    assert_eq!(fixture.bytes(), disk);
}
