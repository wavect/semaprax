//! Host-granted OpenAPI artifact transport regressions, authored and unrun.
use semaprax::image_transport::{VNextPolicy, VNextSession};
use semaprax::project::{
    with_authenticated_project, ImageArtifactKind, ProjectCandidate, ProjectSemanticImage,
    SemanticChange, MAX_IMAGE_ARTIFACT_BUILD_BYTES,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
static SERIAL: AtomicU64 = AtomicU64::new(0);
const PATHS: [&str; 4] = [
    "semaprax.toml",
    "src/app.spx",
    "src/core.spx",
    "src/tests.spx",
];
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-openapi-artifact-rpc-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in PATHS {
            std::fs::copy(sample.join(path), root.join(path)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn manifest(&self) -> PathBuf {
        self.0.join("semaprax.toml")
    }
    fn session(&self, candidates: bool, build: bool) -> VNextSession {
        VNextSession::open(
            &self.manifest(),
            VNextPolicy {
                candidate_prepare: candidates,
                build_enabled: build,
                ..Default::default()
            },
        )
        .unwrap()
    }
    fn candidate(&self) -> ProjectCandidate {
        with_authenticated_project(&self.manifest(), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
        })
        .unwrap()
    }
    fn bytes(&self) -> Vec<Vec<u8>> {
        PATHS
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
fn call(session: &mut VNextSession, method: &str, params: Value) -> Value {
    let frame = json!({"jsonrpc":"2.0","id":"openapi-artifacts","method":method,"params":params})
        .to_string();
    serde_json::from_slice(&session.handle_frame(frame.as_bytes()).unwrap()).unwrap()
}
fn bound(session: &mut VNextSession, method: &str, mut params: Value) -> Value {
    params["image_revision"] = json!(session.image_revision());
    call(session, method, params)
}
fn payload(response: Value) -> Value {
    assert!(response.get("error").is_none(), "{response}");
    response["result"]["payload"].clone()
}
fn report(session: &mut VNextSession, method: &str, candidate: &str) -> String {
    let mut result = String::new();
    for _ in 0..8193 {
        let chunk = payload(bound(
            session,
            method,
            json!({"candidate_revision":candidate,"kind":"openapi","offset":result.len(),"chunk_bytes":1024}),
        ));
        assert_eq!(chunk["kind"], "openapi");
        assert_eq!(chunk["candidate_revision"], candidate);
        assert_eq!(chunk["image_revision"], session.image_revision());
        assert_eq!(chunk["offset"], result.len());
        for field in [
            "source_authority",
            "artifact_materialization",
            "target_execution",
        ] {
            assert_eq!(chunk[field], false);
        }
        let text = chunk["chunk"].as_str().unwrap();
        assert!(!text.is_empty() && text.len() <= 1024);
        result.push_str(text);
        if chunk["next_offset"].is_null() {
            assert_eq!(chunk["total_bytes"], result.len());
            return result;
        }
        assert_eq!(chunk["next_offset"], result.len());
    }
    panic!("bounded OpenAPI artifact report did not terminate")
}

#[test]
fn build_and_delta_chunks_match_independent_source_replayed_openapi_projections() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let mut session = fixture.session(true, true);
    let root = payload(bound(&mut session, "candidate/open", json!({})))["candidate_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(root, base.candidate_digest());
    let initial = ProjectSemanticImage::derive(
        Arc::clone(base.revision()),
        base.revision().project_revision(),
    )
    .unwrap();
    assert_eq!(
        report(&mut session, "candidate/build", &root),
        initial
            .artifact_projection(
                initial.image_digest(),
                ImageArtifactKind::OpenApi,
                MAX_IMAGE_ARTIFACT_BUILD_BYTES
            )
            .unwrap()
    );
    let intent = json!({"kind":"change_function_signature","target":"calculator.add","append_parameters":[{"name":"offset","type":"i64","argument":{"kind":"i64","value":0}}]});
    let changed = base
        .apply(
            base.candidate_digest(),
            &SemanticChange::new(base.revision().project_revision(), &intent).unwrap(),
        )
        .unwrap();
    let applied = payload(bound(
        &mut session,
        "candidate/apply-intent",
        json!({"candidate_revision":root,"intent":intent}),
    ));
    assert_eq!(applied["candidate_revision"], changed.candidate_digest());
    let image = ProjectSemanticImage::derive(
        Arc::clone(changed.revision()),
        changed.revision().project_revision(),
    )
    .unwrap();
    assert_eq!(
        report(&mut session, "candidate/build", changed.candidate_digest()),
        image
            .artifact_projection(
                image.image_digest(),
                ImageArtifactKind::OpenApi,
                MAX_IMAGE_ARTIFACT_BUILD_BYTES
            )
            .unwrap()
    );
    assert_eq!(
        report(
            &mut session,
            "candidate/artifact-delta",
            changed.candidate_digest()
        ),
        changed
            .artifact_delta(changed.candidate_digest(), ImageArtifactKind::OpenApi)
            .unwrap()
    );
    let old = report(&mut session, "candidate/artifact-delta", &root);
    let old: Value = serde_json::from_str(&old).unwrap();
    assert_eq!(old["comparison"]["artifact_bytes_equal"], true);
    session.finish().unwrap();
    assert_eq!(fixture.bytes(), disk);
    let mut names = std::fs::read_dir(&fixture.0)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(names, ["semaprax.toml", "src"]);
}

#[test]
fn discovery_admits_openapi_only_through_existing_host_build_grant_and_closed_kind_enum() {
    let fixture = Fixture::new();
    assert!(VNextSession::open(
        &fixture.manifest(),
        VNextPolicy {
            build_enabled: true,
            ..Default::default()
        }
    )
    .is_err());
    for (candidate, build) in [(false, false), (true, false), (true, true)] {
        let mut session = fixture.session(candidate, build);
        let capabilities = payload(call(&mut session, "protocol/capabilities", json!({})));
        for method in ["candidate/build", "candidate/artifact-delta"] {
            assert_eq!(
                capabilities["methods"]
                    .as_array()
                    .unwrap()
                    .contains(&json!(method)),
                build
            );
            assert!(!session.parallel_read_methods().contains(&method));
            if !build {
                assert_eq!(
                    call(&mut session, method, json!({"kind":"openapi"}))["error"]["code"],
                    -32601
                );
            }
        }
        if !build {
            continue;
        }
        let schemas = payload(call(&mut session, "protocol/schemas", json!({})));
        for method in ["candidate/build", "candidate/artifact-delta"] {
            let descriptor = schemas["methods"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["method"] == method)
                .unwrap();
            assert_eq!(descriptor["capability"], "candidate_build");
            let params = &descriptor["request_schema"]["properties"]["params"];
            assert_eq!(params["additionalProperties"], false);
            assert_eq!(
                params["properties"]["kind"]["enum"],
                json!(["web", "npm", "openapi", "c"])
            );
            for absent in [
                "path",
                "max_bytes",
                "max_build_bytes",
                "build_enabled",
                "target",
            ] {
                assert!(params["properties"].get(absent).is_none());
            }
        }
        for language in ["typescript", "python", "rust"] {
            let client = payload(call(
                &mut session,
                "protocol/client",
                json!({"language":language}),
            ));
            let source = client["source"].as_str().unwrap();
            assert!(source.contains("openapi"));
            assert!(source.contains("request_candidate_build"));
            assert!(source.contains("request_candidate_artifact_delta"));
        }
        let instructions = payload(call(&mut session, "protocol/instructions", json!({})));
        assert!(instructions["instructions"]
            .as_str()
            .unwrap()
            .contains("openapi"));
    }
}

#[test]
fn stale_and_malformed_openapi_requests_preserve_candidate_and_source() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let mut session = fixture.session(true, true);
    let root = payload(bound(&mut session, "candidate/open", json!({})))["candidate_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let expected = report(&mut session, "candidate/build", &root);
    for method in ["candidate/build", "candidate/artifact-delta"] {
        for extra in [
            json!({"kind":"arbitrary"}),
            json!({"max_build_bytes":1024}),
            json!({"path":"/tmp/output"}),
            json!({"chunk_bytes":1023}),
        ] {
            let mut params = json!({"candidate_revision":root,"kind":"openapi"});
            params
                .as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            assert_eq!(bound(&mut session, method, params)["error"]["code"], -32602);
        }
        assert_eq!(
            call(
                &mut session,
                method,
                json!({"image_revision":format!("sha256:{}","0".repeat(64)),"candidate_revision":root,"kind":"openapi"})
            )["error"]["code"],
            -32000
        );
        assert_eq!(
            bound(
                &mut session,
                method,
                json!({"candidate_revision":format!("sha256:{}","0".repeat(64)),"kind":"openapi"})
            )["error"]["code"],
            -32000
        );
    }
    assert_eq!(report(&mut session, "candidate/build", &root), expected);
    session.finish().unwrap();
    assert_eq!(fixture.bytes(), disk);
}
