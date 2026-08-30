//! Relocation evidence authored without running compiler or test gates.
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{with_authenticated_project, ProjectCandidate, SemanticChange};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-movement-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let fixture = Self(root);
        std::fs::write(fixture.0.join("semaprax.toml"), "schema = \"semaprax.project.v1\"\nname = \"mover\"\nentry = \"mover.app\"\nsources = [\"src/app.spx\", \"src/core.spx\", \"src/support.spx\", \"src/tests.spx\"]\nweb_exports = [\"move.export\"]\ntests = [\"mover.tests\"]\n").unwrap();
        fixture.write("core", "module mover.core; @id(\"move.export\") fn exported(value:i64)->i64 {value} @id(\"move.helper\") fn helper(value:i64)->i64 requires value>=0 ensures result>=0 {exported(value)}");
        fixture.write("app", "module mover.app; use function @id(\"move.helper\") from mover.core as via_helper; @id(\"move.app.main\") fn main()->i64 {via_helper(1)}");
        fixture.write("support", "module mover.support; @id(\"move.support.identity\") fn identity(value:i64)->i64 {value}");
        fixture.write("tests", "module mover.tests; use function @id(\"move.helper\") from mover.core as helper_check; @id(\"move.tests.main\") fn main()->i64 {if helper_check(1)==1 {0} else {1}}");
        fixture
    }
    fn write(&self, module: &str, source: &str) {
        let path = format!("src/{module}.spx");
        let program = semaprax::parse(source, &path).unwrap();
        std::fs::write(self.0.join(path), semaprax::format::canonical(&program)).unwrap();
    }
    fn candidate(&self) -> ProjectCandidate {
        let revision = with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap();
        ProjectCandidate::open(Arc::clone(&revision), revision.project_revision()).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn movement(destination: &str) -> Value {
    json!({"kind":"move_declaration","target":"move.helper","destination":destination})
}
fn apply(
    candidate: &ProjectCandidate,
    request: Value,
) -> Result<ProjectCandidate, Vec<Diagnostic>> {
    candidate.apply(
        candidate.candidate_digest(),
        &SemanticChange::new(candidate.revision().project_revision(), &request)?,
    )
}
fn source<'a>(candidate: &'a ProjectCandidate, module: &str) -> &'a str {
    let path = format!("src/{module}.spx");
    candidate
        .revision()
        .sources()
        .iter()
        .find(|source| source.path() == path)
        .unwrap()
        .source()
}
fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    match result {
        Ok(_) => panic!("expected {expected}"),
        Err(errors) => assert!(errors.iter().any(|d| d.code == expected), "{errors:?}"),
    }
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
    let replay = ProjectCandidate::replay(
        Arc::clone(candidate.base_revision()),
        candidate.base_revision().project_revision(),
        &changes,
        candidate.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(replay.to_json(), candidate.to_json());
}

#[test]
fn move_rebinds_local_destination_external_callers_and_body_dependencies_without_writes() {
    let fixture = Fixture::new();
    let root = fixture.candidate();
    let before = root
        .revision()
        .sources()
        .iter()
        .map(|s| {
            (
                s.path().to_owned(),
                std::fs::read(fixture.0.join(s.path())).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let moved = apply(&root, movement("move.app.main")).unwrap();
    assert!(!source(&moved, "core").contains("fn helper("));
    assert!(source(&moved, "app").contains("fn helper("));
    assert!(!source(&moved, "app").contains("as via_helper;"));
    assert!(source(&moved, "app").contains("    helper(1)"));
    assert!(source(&moved, "app")
        .contains("use function @id(\"move.export\") from mover.core as exported;"));
    assert!(source(&moved, "tests")
        .contains("use function @id(\"move.helper\") from mover.app as helper_check;"));
    assert!(source(&moved, "app").contains("requires value >= 0"));
    assert!(source(&moved, "app").contains("ensures result >= 0"));
    let graph: Value = serde_json::from_str(moved.revision().semantic_graph()).unwrap();
    let identity = graph["declarations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["id"] == "move.helper")
        .unwrap();
    assert_eq!(identity["path"], "src/app.spx");
    assert_eq!(identity["identity_origin"], "explicit");
    replay(&moved);
    for (path, bytes) in before {
        assert_eq!(std::fs::read(fixture.0.join(path)).unwrap(), bytes);
    }
}

#[test]
fn source_callers_gain_import_while_relocated_dependency_becomes_local() {
    let fixture = Fixture::new();
    fixture.write("core", "module mover.core; use function @id(\"move.support.identity\") from mover.support as identity; @id(\"move.export\") fn exported(value:i64)->i64 {value} @id(\"move.helper\") fn helper(value:i64)->i64 {identity(value)} @id(\"move.local-caller\") fn local_caller(value:i64)->i64 {helper(value)}");
    let root = fixture.candidate();
    let moved = apply(&root, movement("move.support.identity")).unwrap();
    assert!(source(&moved, "core")
        .contains("use function @id(\"move.helper\") from mover.support as helper;"));
    assert!(!source(&moved, "core").contains("as identity;"));
    assert!(source(&moved, "support").contains("    identity(value)"));
    assert!(!source(&moved, "support").contains("use function"));
    assert!(source(&moved, "app").contains("from mover.support as via_helper;"));
    replay(&moved);
}

#[test]
fn conflicting_dependency_alias_is_hygienically_derived_and_replayed() {
    let fixture = Fixture::new();
    fixture.write("app", "module mover.app; use function @id(\"move.helper\") from mover.core as via_helper; @id(\"move.app.main\") fn main()->i64 {via_helper(exported(1))} @id(\"move.app.exported\") fn exported(value:i64)->i64 {value} @id(\"move.app.reserved\") fn _spx_move_0(value:i64)->i64 {value}");
    let root = fixture.candidate();
    let moved = apply(&root, movement("move.app.main")).unwrap();
    assert!(source(&moved, "app").contains("@id(\"move.export\") from mover.core as _spx_move_1;"));
    assert!(source(&moved, "app").contains("    _spx_move_1(value)"));
    assert!(source(&moved, "app").contains("    helper(exported(1))"));
    replay(&moved);
}

#[test]
fn fixed_exports_entry_identity_bad_destinations_and_cycle_attempts_leave_candidate_unchanged() {
    let fixture = Fixture::new();
    let root = fixture.candidate();
    let before = root.to_json().to_owned();
    for request in [
        json!({"kind":"move_declaration","target":"move.export","destination":"move.app.main"}),
        json!({"kind":"move_declaration","target":"move.app.main","destination":"move.support.identity"}),
        movement("move.export"),
        movement("unknown.anchor"),
        json!({"kind":"move_declaration","target":"move.helper","destination":"move.app.main","path":"src/app.spx"}),
    ] {
        code(apply(&root, request), "SPX-G225");
        assert_eq!(root.to_json(), before);
    }
    fixture.write("core", "module mover.core; @id(\"move.export\") fn exported(value:i64)->i64 {value} @id(\"move.helper\") fn helper(value:i64)->i64 {exported(value)} @id(\"move.local-caller\") fn local_caller(value:i64)->i64 {helper(value)}");
    let cyclic_root = fixture.candidate();
    let before = cyclic_root.to_json().to_owned();
    assert!(apply(&cyclic_root, movement("move.app.main")).is_err());
    assert_eq!(cyclic_root.to_json(), before);
}

#[test]
fn movement_merges_unrelated_body_and_dependency_rename_and_rejects_competing_locations() {
    let fixture = Fixture::new();
    let root = fixture.candidate();
    let left = apply(&root, movement("move.app.main")).unwrap();
    let renamed = apply(
        &root,
        json!({"kind":"rename_declaration","target":"move.export","name":"published"}),
    )
    .unwrap();
    let right = apply(&renamed, json!({"kind":"replace_function_body","target":"move.support.identity","body":{"kind":"i64","value":9}})).unwrap();
    let merged = left
        .merge(left.candidate_digest(), &right, right.candidate_digest())
        .unwrap();
    assert!(source(merged.candidate(), "app").contains("fn helper("));
    assert!(source(merged.candidate(), "app").contains("published(value)"));
    assert!(source(merged.candidate(), "support").contains("    9\n"));
    replay(merged.candidate());
    let competing = apply(&root, movement("move.support.identity")).unwrap();
    code(
        left.merge(
            left.candidate_digest(),
            &competing,
            competing.candidate_digest(),
        ),
        "SPX-G235",
    );
    let stale_change = SemanticChange::new(
        root.revision().project_revision(),
        &movement("move.support.identity"),
    )
    .unwrap();
    code(
        left.apply(root.candidate_digest(), &stale_change),
        "SPX-G224",
    );
}
