//! Conservative interface-intention rebase evidence, authored and intentionally unrun.
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectRevision, SemanticChange,
};
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
            "spx-interface-rebase-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "interface-rebase"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "iface.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["iface.evaluate"]
tests = ["iface.tests"]
"#,
        )
        .unwrap();
        for (path, source) in [
            (
                "src/core.spx",
                r#"module iface.core;
@id("iface.payload") record Payload { @id("iface.payload.value") value: i64, }
@id("iface.counter") record Counter {
    @id("iface.counter.value") value: i64,
    @id("iface.counter.payload") payload: Payload,
}
@id("iface.readable") protocol Readable {
    @id("iface.readable.read") fn read(receiver: Self) -> i64;
    @id("iface.readable.positive") fn positive(receiver: Self) -> bool;
}
@id("iface.read") fn counter_read(receiver: Counter) -> i64 { receiver.value }
@id("iface.positive") fn counter_positive(receiver: Counter) -> bool { receiver.value > 0 }
@id("iface.evaluate") fn evaluate(value: i64) -> i64 {
    counter_read(Counter { value: value, payload: Payload { value: value } })
}
"#,
            ),
            (
                "src/app.spx",
                r#"module iface.app;
use function @id("iface.evaluate") from iface.core as evaluate;
@id("iface.main") fn main() -> i64 { evaluate(42) }
"#,
            ),
            (
                "src/tests.spx",
                r#"module iface.tests;
use function @id("iface.evaluate") from iface.core as evaluate;
@id("iface.test") fn main() -> i64 { if evaluate(42) == 42 { 0 } else { 1 } }
"#,
            ),
        ] {
            let program = semaprax::parse(source, path).unwrap();
            std::fs::write(root.join(path), semaprax::format::canonical(&program)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }

    fn revision(&self) -> Arc<ProjectRevision> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap()
    }

    fn candidate(&self) -> ProjectCandidate {
        let revision = self.revision();
        ProjectCandidate::open(Arc::clone(&revision), revision.project_revision()).unwrap()
    }

    fn rewrite_core(&self, edit: impl FnOnce(String) -> String) -> Arc<ProjectRevision> {
        let path = self.0.join("src/core.spx");
        let source = edit(std::fs::read_to_string(&path).unwrap());
        let program = semaprax::parse(&source, "src/core.spx").unwrap();
        std::fs::write(path, semaprax::format::canonical(&program)).unwrap();
        self.revision()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn interface_intent(id: &str) -> Value {
    json!({"kind":"implement_interface","target":"iface.counter","protocol":"iface.readable","id":id,"members":[
        {"method":"iface.readable.read","implementation":"iface.read"},
        {"method":"iface.readable.positive","implementation":"iface.positive"}
    ]})
}

fn apply(candidate: &ProjectCandidate, intent: Value) -> ProjectCandidate {
    candidate
        .apply(
            candidate.candidate_digest(),
            &SemanticChange::new(candidate.revision().project_revision(), &intent).unwrap(),
        )
        .unwrap()
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

fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    match result {
        Ok(_) => panic!("expected {expected}"),
        Err(errors) => assert!(
            errors.iter().any(|error| error.code == expected),
            "{errors:?}"
        ),
    }
}

#[test]
fn unrelated_body_and_selected_display_changes_rebase_and_merge_without_writes() {
    let fixture = Fixture::new();
    let root = fixture.candidate();
    let disk = std::fs::read(fixture.0.join("src/core.spx")).unwrap();
    let interface = apply(&root, interface_intent("iface.counter.readable"));
    let renamed = apply(
        &root,
        json!({"kind":"rename_declaration","target":"iface.read","name":"read_again"}),
    );
    let other = apply(
        &renamed,
        json!({"kind":"replace_function_body","target":"iface.evaluate","body":{"kind":"i64","value":7}}),
    );
    let interface_before = interface.to_json().to_owned();
    let other_before = other.to_json().to_owned();
    let rebased = interface
        .rebase(
            interface.candidate_digest(),
            Arc::clone(other.revision()),
            other.revision().project_revision(),
        )
        .unwrap();
    let merged = interface
        .merge(
            interface.candidate_digest(),
            &other,
            other.candidate_digest(),
        )
        .unwrap();
    assert_eq!(
        rebased.candidate().revision().project_revision(),
        merged.candidate().revision().project_revision()
    );
    assert!(source(merged.candidate()).contains("fn read_again("));
    assert!(source(merged.candidate()).contains("    7\n"));
    assert!(source(merged.candidate()).contains("impl \"iface.readable\" for \"iface.counter\""));
    let candidate_report: Value = serde_json::from_str(merged.candidate().to_json()).unwrap();
    let implementation = candidate_report["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|operation| operation["kind"] == "implement_interface")
        .unwrap();
    assert_eq!(
        implementation["new_declaration"]["runtime_graph_declaration"],
        false
    );
    let merge_report: Value = serde_json::from_str(merged.to_json()).unwrap();
    assert_eq!(merge_report["source_authority"], false);
    assert_eq!(interface.to_json(), interface_before);
    assert_eq!(other.to_json(), other_before);
    assert_eq!(std::fs::read(fixture.0.join("src/core.spx")).unwrap(), disk);
}

#[test]
fn receiver_protocol_and_selected_function_conformance_drift_conflict() {
    for edit in [
        "receiver",
        "nominal_identity",
        "protocol",
        "protocol_order",
        "implementation",
    ] {
        let fixture = Fixture::new();
        let candidate = apply(
            &fixture.candidate(),
            interface_intent("iface.counter.readable"),
        );
        let before = candidate.to_json().to_owned();
        let revision = fixture.rewrite_core(|source| match edit {
            "receiver" => source.replace("iface.counter.value", "iface.counter.changed-value"),
            "nominal_identity" => source.replace("iface.payload\")", "iface.payload.changed\")"),
            "protocol" => source.replace("protocol Readable", "protocol ReadableAgain"),
            "protocol_order" => source.replace(
                "    @id(\"iface.readable.read\") fn read(receiver: Self) -> i64;\n    @id(\"iface.readable.positive\") fn positive(receiver: Self) -> bool;",
                "    @id(\"iface.readable.positive\") fn positive(receiver: Self) -> bool;\n    @id(\"iface.readable.read\") fn read(receiver: Self) -> i64;",
            ),
            "implementation" => source.replace(
                "fn counter_read(receiver: Counter) -> i64 {",
                "fn counter_read(receiver: Counter) -> i64 requires true {",
            ),
            _ => unreachable!(),
        });
        code(
            candidate.rebase(
                candidate.candidate_digest(),
                Arc::clone(&revision),
                revision.project_revision(),
            ),
            "SPX-G235",
        );
        assert_eq!(candidate.to_json(), before);
    }
}

#[test]
fn occupied_pair_and_global_implementation_identity_conflict() {
    for collision in ["pair", "identity"] {
        let fixture = Fixture::new();
        let candidate = apply(
            &fixture.candidate(),
            interface_intent("iface.counter.readable"),
        );
        let before = candidate.to_json().to_owned();
        let revision = fixture.rewrite_core(|source| {
            if collision == "pair" {
                source
                    + r#"
@id("iface.other-binding")
impl "iface.readable" for "iface.counter" {
    "iface.readable.read" = "iface.read";
    "iface.readable.positive" = "iface.positive";
}
"#
            } else {
                source
                    + r#"
@id("iface.counter.readable") fn occupied_identity() -> i64 { 0 }
"#
            }
        });
        code(
            candidate.rebase(
                candidate.candidate_digest(),
                Arc::clone(&revision),
                revision.project_revision(),
            ),
            "SPX-G235",
        );
        assert_eq!(candidate.to_json(), before);
    }
}

#[test]
fn sibling_pair_and_implementation_id_collisions_fail_closed() {
    let fixture = Fixture::new();
    let root = fixture.candidate();
    let left = apply(&root, interface_intent("iface.counter.readable"));
    let competing = apply(&root, interface_intent("iface.other-binding"));
    code(
        left.merge(
            left.candidate_digest(),
            &competing,
            competing.candidate_digest(),
        ),
        "SPX-G235",
    );
    let identity = apply(
        &root,
        json!({"kind":"add_declaration","target":"iface.evaluate","declaration":{
            "id":"iface.counter.readable","name":"occupied_identity","parameters":[],
            "return_type":"i64","effects":[],"requires":[],"ensures":[],
            "body":{"kind":"i64","value":0}
        }}),
    );
    code(
        left.merge(
            left.candidate_digest(),
            &identity,
            identity.candidate_digest(),
        ),
        "SPX-G235",
    );
    code(
        identity.merge(identity.candidate_digest(), &left, left.candidate_digest()),
        "SPX-G235",
    );
}
