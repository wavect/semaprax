//! Nominal rename transport/rebase regressions, authored and intentionally unrun.
use semaprax::image_transport::{VNextPolicy, VNextSession};
use semaprax::project::{
    with_authenticated_project, CandidateTestPolicy, ProjectCandidate, SemanticChange,
};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-nominal-rename-rpc-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
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
        for (path, source) in [
            (
                "src/core.spx",
                r#"module rename.core;
@id("rename.record") record Existing { @id("rename.field") value: i64, }
@id("rename.variant") variant Choice { @id("rename.some") Some { @id("rename.payload") flag: bool, }, @id("rename.none") None, }
@id("rename.public") fn public_value(value:i64)->i64 { let record = Existing { value: value }; record.value }
@id("rename.spare") fn spare(value:i64)->i64 {value}
"#,
            ),
            (
                "src/app.spx",
                r#"module rename.app;
use type @id("rename.record") from rename.core as Metric;
use type @id("rename.variant") from rename.core as Decision;
use function @id("rename.public") from rename.core as public_value;
@id("rename.main") fn main()->i64 {let input = Metric { value: 42 }; public_value(input.value)}
"#,
            ),
            (
                "src/tests.spx",
                r#"module rename.tests;
use function @id("rename.public") from rename.core as public_value;
@id("rename.test") fn main()->i64 {if public_value(42) == 42 {0}else{1}}
"#,
            ),
        ] {
            let program = semaprax::parse(source, path).unwrap();
            std::fs::write(root.join(path), semaprax::format::canonical(&program)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn manifest(&self) -> PathBuf {
        self.0.join("semaprax.toml")
    }
    fn candidate(&self) -> ProjectCandidate {
        with_authenticated_project(&self.manifest(), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
        })
        .unwrap()
    }
    fn session(&self) -> VNextSession {
        VNextSession::open(
            &self.manifest(),
            VNextPolicy {
                candidate_prepare: true,
                test_policy: Some(CandidateTestPolicy::new(10_000, 4096, 16_384).unwrap()),
                ..Default::default()
            },
        )
        .unwrap()
    }
    fn bytes(&self) -> Vec<Vec<u8>> {
        [
            "semaprax.toml",
            "src/core.spx",
            "src/app.spx",
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
fn call(session: &mut VNextSession, method: &str, mut params: Value) -> Value {
    params["image_revision"] = json!(session.image_revision());
    let request =
        json!({"jsonrpc":"2.0","id":"nominal","method":method,"params":params}).to_string();
    serde_json::from_slice(&session.handle_frame(request.as_bytes()).unwrap()).unwrap()
}
fn payload(value: Value) -> Value {
    assert!(value.get("error").is_none(), "{value}");
    value["result"]["payload"].clone()
}
fn rename(target: &str, name: &str) -> Value {
    json!({"kind":"rename_declaration","target":target,"name":name})
}
fn apply(candidate: &ProjectCandidate, intent: Value) -> ProjectCandidate {
    candidate
        .apply(
            candidate.candidate_digest(),
            &SemanticChange::new(candidate.revision().project_revision(), &intent).unwrap(),
        )
        .unwrap()
}

#[test]
fn existing_rpc_discovers_and_validates_record_and_variant_display_renames() {
    let fixture = Fixture::new();
    let before = fixture.bytes();
    let mut session = fixture.session();
    let root = payload(call(&mut session, "candidate/open", json!({})))["candidate_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let library = fixture.candidate();
    for (target, name) in [
        ("rename.record", "RenamedRecord"),
        ("rename.variant", "RenamedChoice"),
    ] {
        let catalog = payload(call(
            &mut session,
            "change/catalog",
            json!({"candidate_revision":root,"target":target}),
        ));
        assert_eq!(catalog["parameters"], json!([]));
        let operation = catalog["operations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|op| op["kind"] == "rename_declaration")
            .unwrap();
        assert_eq!(
            operation["required_fields"],
            json!(["kind", "target", "name"])
        );
        assert!(operation["constraints"]
            .as_array()
            .unwrap()
            .contains(&json!("source_record_or_variant_owner")));
        let expected = apply(&library, rename(target, name));
        let changed = payload(call(
            &mut session,
            "candidate/apply-intent",
            json!({"candidate_revision":root,"intent":rename(target,name)}),
        ));
        assert_eq!(changed["candidate_revision"], expected.candidate_digest());
        payload(call(
            &mut session,
            "candidate/validate",
            json!({"candidate_revision":expected.candidate_digest()}),
        ));
        let plan = payload(call(
            &mut session,
            "candidate/test-plan",
            json!({"candidate_revision":expected.candidate_digest()}),
        ));
        assert_eq!(plan["selected"], true);
        assert_eq!(plan["execution"], "not_run");
        assert!(plan["conservative_reasons"]
            .as_array()
            .unwrap()
            .contains(&json!("non_callable_type_display_change")));
        let app = expected
            .revision()
            .sources()
            .iter()
            .find(|source| source.path() == "src/app.spx")
            .unwrap()
            .source();
        assert!(app.contains("as Metric;"));
        assert!(app.contains("as Decision;"));
    }
    for target in [
        "rename.field",
        "rename.some",
        "rename.payload",
        "core.option",
    ] {
        let catalog = payload(call(
            &mut session,
            "change/catalog",
            json!({"candidate_revision":root,"target":target}),
        ));
        assert!(!catalog["operations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|op| op["kind"] == "rename_declaration"));
    }
    let mut readonly = VNextSession::open(&fixture.manifest(), VNextPolicy::default()).unwrap();
    assert_eq!(
        call(
            &mut readonly,
            "candidate/apply-intent",
            json!({"candidate_revision":root,"intent":rename("rename.record","NoAuthority")})
        )["error"]["code"],
        -32601
    );
    session.finish().unwrap();
    assert_eq!(fixture.bytes(), before);
}

#[test]
fn type_rename_merges_unrelated_calls_but_rejects_shape_and_competing_display_changes() {
    let fixture = Fixture::new();
    let before = fixture.bytes();
    let root = fixture.candidate();
    let renamed = apply(&root, rename("rename.record", "RenamedRecord"));
    let unrelated = apply(&root, rename("rename.spare", "spare_value"));
    let merged = renamed
        .merge(
            renamed.candidate_digest(),
            &unrelated,
            unrelated.candidate_digest(),
        )
        .unwrap()
        .into_candidate();
    let capsule = merged.recovery_capsule().unwrap();
    let restored = with_authenticated_project(&fixture.manifest(), |snapshot| {
        ProjectCandidate::restore(
            snapshot.retain_revision(),
            snapshot.project_revision(),
            capsule.as_bytes(),
        )
    })
    .unwrap();
    assert_eq!(restored.to_json(), merged.to_json());
    let competing = apply(&root, rename("rename.record", "OtherRecord"));
    let net_zero = apply(&competing, rename("rename.record", "Existing"));
    let reshaped = apply(
        &root,
        json!({"kind":"add_record_field","target":"rename.record","field":{"id":"rename.extra","name":"extra","type":"bool","default":{"kind":"bool","value":false}}}),
    );
    for other in [&competing, &net_zero, &reshaped] {
        let errors = renamed
            .merge(renamed.candidate_digest(), other, other.candidate_digest())
            .err()
            .expect("competing type history must conflict");
        assert!(errors.iter().any(|error| error.code == "SPX-G235"));
    }
    // An earlier addition creates a real owner before its rename is rebound.
    let added = apply(
        &root,
        json!({"kind":"add_declaration","target":"rename.spare","declaration":{"kind":"record","id":"rename.new","name":"NewRecord","fields":[]}}),
    );
    let added_renamed = apply(&added, rename("rename.new", "NewDisplay"));
    let merged = added_renamed
        .merge(
            added_renamed.candidate_digest(),
            &unrelated,
            unrelated.candidate_digest(),
        )
        .unwrap()
        .into_candidate();
    assert!(merged
        .revision()
        .sources()
        .iter()
        .any(|source| source.source().contains("record NewDisplay")));
    assert_eq!(fixture.bytes(), before);
}
