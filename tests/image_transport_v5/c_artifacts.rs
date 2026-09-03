//! Host-granted C artifact transport evidence, authored and intentionally unrun.
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
            "spx-c-artifact-rpc-{}-{}",
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
    fn session(&self, candidate: bool, build: bool) -> VNextSession {
        VNextSession::open(
            &self.manifest(),
            VNextPolicy {
                candidate_prepare: candidate,
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
    let request =
        json!({"jsonrpc":"2.0","id":"c-artifact","method":method,"params":params}).to_string();
    serde_json::from_slice(&session.handle_frame(request.as_bytes()).unwrap()).unwrap()
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
    let mut text = String::new();
    for _ in 0..8193 {
        let chunk = payload(bound(
            session,
            method,
            json!({"candidate_revision":candidate,"kind":"c","offset":text.len(),"chunk_bytes":1024}),
        ));
        assert_eq!(chunk["kind"], "c");
        assert_eq!(chunk["candidate_revision"], candidate);
        assert_eq!(chunk["image_revision"], session.image_revision());
        assert_eq!(chunk["offset"], text.len());
        for flag in [
            "source_authority",
            "artifact_materialization",
            "target_execution",
        ] {
            assert_eq!(chunk[flag], false);
        }
        let piece = chunk["chunk"].as_str().unwrap();
        assert!(!piece.is_empty() && piece.len() <= 1024);
        text.push_str(piece);
        if chunk["next_offset"].is_null() {
            assert_eq!(chunk["total_bytes"], text.len());
            return text;
        }
        assert_eq!(chunk["next_offset"], text.len());
    }
    panic!("bounded C artifact report did not terminate")
}

#[test]
fn c_build_and_signature_delta_chunks_reassemble_exact_independent_library_reports() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let mut session = fixture.session(true, true);
    let root = payload(bound(&mut session, "candidate/open", json!({})))["candidate_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(root, base.candidate_digest());
    let image = ProjectSemanticImage::derive(
        Arc::clone(base.revision()),
        base.revision().project_revision(),
    )
    .unwrap();
    assert_eq!(
        report(&mut session, "candidate/build", &root),
        image
            .artifact_projection(
                image.image_digest(),
                ImageArtifactKind::C,
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
                ImageArtifactKind::C,
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
            .artifact_delta(changed.candidate_digest(), ImageArtifactKind::C)
            .unwrap()
    );
    assert_eq!(
        report(&mut session, "candidate/artifact-delta", &root),
        base.artifact_delta(base.candidate_digest(), ImageArtifactKind::C)
            .unwrap()
    );
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
fn c_kind_discovery_retains_host_build_authority_and_generated_client_request_bounds() {
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
                    call(&mut session, method, json!({"kind":"c"}))["error"]["code"],
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
                "target",
                "build_enabled",
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
            assert!(source.contains("request_candidate_build"));
            assert!(source.contains("request_candidate_artifact_delta"));
            assert!(source.contains("openapi"));
        }
        assert!(
            payload(call(&mut session, "protocol/instructions", json!({})))["instructions"]
                .as_str()
                .unwrap()
                .contains("native-emitter-derived C declarations")
        );
    }
}

#[test]
fn stale_or_authority_shaped_c_requests_cannot_change_source_or_cached_candidate() {
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
            json!({"kind":"shared-library"}),
            json!({"path":"/tmp/output.c"}),
            json!({"compile":true}),
            json!({"max_build_bytes":1024}),
            json!({"chunk_bytes":1023}),
        ] {
            let mut params = json!({"candidate_revision":root,"kind":"c"});
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
                json!({"image_revision":format!("sha256:{}","0".repeat(64)),"candidate_revision":root,"kind":"c"})
            )["error"]["code"],
            -32000
        );
        assert_eq!(
            bound(
                &mut session,
                method,
                json!({"candidate_revision":format!("sha256:{}","0".repeat(64)),"kind":"c"})
            )["error"]["code"],
            -32000
        );
    }
    assert_eq!(report(&mut session, "candidate/build", &root), expected);
    session.finish().unwrap();
    assert_eq!(fixture.bytes(), disk);
}
