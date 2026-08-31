//! Optional MCP stdio adapter evidence, authored and intentionally unrun.
use semaprax::image_transport::{McpSession, VNextPolicy, VNextSession};
use semaprax::project::{
    with_authenticated_project, CandidateTestPolicy, ProjectCandidate, ProjectCandidateDraft,
    SemanticChange,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-mcp-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for file in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(sample.join(file), root.join(file)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn host(&self, policy: VNextPolicy) -> VNextSession {
        VNextSession::open(&self.0.join("semaprax.toml"), policy).unwrap()
    }
    fn candidate(&self) -> Arc<ProjectCandidate> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
                .map(Arc::new)
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
        .map(|file| std::fs::read(self.0.join(file)).unwrap())
        .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn request(id: Value, method: &str, params: Value) -> Vec<u8> {
    json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
        .to_string()
        .into_bytes()
}
fn call(session: &mut McpSession, id: Value, method: &str, params: Value) -> Value {
    serde_json::from_slice(&session.handle_frame(&request(id, method, params)).unwrap()).unwrap()
}
fn initialize(session: &mut McpSession, version: &str) -> Value {
    call(
        session,
        json!(1),
        "initialize",
        json!({"protocolVersion":version,"capabilities":{},"clientInfo":{"name":"semaprax-evidence","version":"1"}}),
    )
}
fn ready(session: &mut McpSession) {
    let initialized = initialize(session, "2025-11-25");
    assert_eq!(initialized["result"]["protocolVersion"], "2025-11-25");
    assert!(session
        .handle_frame(br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
        .is_none());
}
fn invoke(session: &mut McpSession, id: Value, method: &str, arguments: Value) -> Value {
    call(
        session,
        id,
        "tools/call",
        json!({"name":method.replace('/',"__"),"arguments":arguments}),
    )
}
fn text(response: &Value) -> &str {
    response["result"]["content"][0]["text"].as_str().unwrap()
}
fn inner(response: &Value) -> Value {
    serde_json::from_str(text(response)).unwrap()
}
fn error(response: &Value) -> bool {
    response.get("error").is_some() || response["result"]["isError"] == true
}
fn tools(session: &mut McpSession) -> Vec<Value> {
    let mut all = Vec::new();
    let mut params = json!({});
    let mut cursors = std::collections::BTreeSet::new();
    loop {
        let page = call(session, json!(2), "tools/list", params);
        assert!(page.get("error").is_none(), "{page}");
        let rows = page["result"]["tools"].as_array().unwrap();
        assert!(rows.len() <= 8);
        all.extend(rows.iter().cloned());
        match page["result"]["nextCursor"].as_str() {
            Some(cursor) => {
                assert!(cursors.insert(cursor.to_owned()));
                params = json!({"cursor":cursor});
            }
            None => break,
        }
    }
    all
}
fn local_refs(value: &Value) {
    match value {
        Value::Object(map) => {
            if let Some(reference) = map.get("$ref") {
                assert!(reference.as_str().unwrap().starts_with("#/"), "{reference}");
            }
            for value in map.values() {
                local_refs(value);
            }
        }
        Value::Array(values) => {
            for value in values {
                local_refs(value);
            }
        }
        _ => {}
    }
}

#[test]
fn pinned_lifecycle_negotiation_paging_and_self_contained_inputs_are_explicit() {
    let fixture = Fixture::new();
    let mut session = McpSession::new(fixture.host(VNextPolicy::default())).unwrap();
    assert!(error(&call(
        &mut session,
        json!(0),
        "tools/list",
        json!({})
    )));
    let negotiated = initialize(&mut session, "1900-01-01");
    assert_eq!(negotiated["result"]["protocolVersion"], "2025-11-25");
    assert!(negotiated["result"]["capabilities"]["tools"].is_object());
    for absent in ["resources", "prompts", "sampling", "elicitation", "tasks"] {
        assert!(negotiated["result"]["capabilities"].get(absent).is_none());
    }
    assert!(error(&call(
        &mut session,
        json!(2),
        "tools/list",
        json!({})
    )));
    assert!(session
        .handle_frame(br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
        .is_none());
    assert_eq!(
        call(&mut session, json!(-7), "ping", json!({}))["result"],
        json!({})
    );
    let all = tools(&mut session);
    assert!(!all.is_empty());
    let names = all
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(names.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(names.contains(&"workspace__open"));
    assert!(!names.contains(&"candidate__open"));
    for tool in &all {
        assert!(tool.get("outputSchema").is_none());
        assert!(tool.get("annotations").is_none());
        assert_eq!(tool["inputSchema"]["type"], "object");
        assert_eq!(tool["inputSchema"]["additionalProperties"], false);
        local_refs(&tool["inputSchema"]);
    }
    assert_eq!(tools(&mut session), all);
    let page = call(&mut session, json!(3), "tools/list", json!({}));
    let cursor = page["result"]["nextCursor"].as_str().unwrap();
    let mut broader = McpSession::new(fixture.host(VNextPolicy {
        candidate_prepare: true,
        ..Default::default()
    }))
    .unwrap();
    ready(&mut broader);
    let rejected = call(
        &mut broader,
        json!(4),
        "tools/list",
        json!({"cursor":cursor}),
    );
    assert!(rejected["error"]["message"]
        .as_str()
        .unwrap()
        .contains("SPX-G350"));
    broader.finish().unwrap();
    assert!(error(&initialize(&mut session, "2025-11-25")));
    session.finish().unwrap();
}

#[test]
fn tools_embed_exact_inner_v5_bytes_for_reads_mutations_holes_and_semantic_errors() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let policy = VNextPolicy {
        candidate_prepare: true,
        ..Default::default()
    };
    let mut direct = fixture.host(policy);
    let image = direct.image_revision().to_owned();
    let mut session = McpSession::new(fixture.host(policy)).unwrap();
    ready(&mut session);
    let base = fixture.candidate();
    let change = SemanticChange::new(
        base.revision().project_revision(),
        &json!({"kind":"rename_declaration","target":"calculator.add","name":"addition"}),
    )
    .unwrap();
    let changed = Arc::new(base.apply(base.candidate_digest(), &change).unwrap());
    let draft = ProjectCandidateDraft::open(Arc::clone(&changed)).unwrap();
    let draft = draft
        .with_body_hole(draft.draft_digest(), "calculator.subtract", "subtract")
        .unwrap();
    let filled = draft
        .fill_hole(
            draft.draft_digest(),
            "subtract",
            &json!({"kind":"i64","value":23}),
        )
        .unwrap();
    let calls = [
        ("workspace/open", json!({})),
        ("candidate/open", json!({"image_revision":image})),
        (
            "candidate/apply-intent",
            json!({"image_revision":image,"candidate_revision":base.candidate_digest(),"intent":{"kind":"rename_declaration","target":"calculator.add","name":"addition"}}),
        ),
        (
            "hole/open",
            json!({"image_revision":image,"candidate_revision":changed.candidate_digest(),"target":"calculator.subtract","hole_id":"subtract"}),
        ),
        (
            "hole/query",
            json!({"image_revision":image,"draft_revision":draft.draft_digest(),"hole_id":"subtract"}),
        ),
        (
            "hole/complete",
            json!({"image_revision":image,"draft_revision":draft.draft_digest()}),
        ),
        (
            "hole/fill",
            json!({"image_revision":image,"draft_revision":draft.draft_digest(),"hole_id":"subtract","expression":{"kind":"i64","value":23}}),
        ),
        (
            "hole/complete",
            json!({"image_revision":image,"draft_revision":filled.draft_digest()}),
        ),
        (
            "candidate/query",
            json!({"image_revision":image,"candidate_revision":base.candidate_digest(),"chunk_bytes":1024}),
        ),
    ];
    for (index, (method, params)) in calls.into_iter().enumerate() {
        let bytes = direct
            .handle_frame(&request(json!(0), method, params.clone()))
            .unwrap();
        let id = json!(format!("outer-{index}"));
        let wrapped = invoke(&mut session, id.clone(), method, params);
        assert_eq!(wrapped["id"], id);
        assert_eq!(wrapped["result"]["content"][0]["type"], "text");
        assert_eq!(text(&wrapped).as_bytes(), bytes);
        let old: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(old.get("error").is_some(), index == 5, "{method}: {old}");
        assert_eq!(wrapped["result"]["isError"], old.get("error").is_some());
        assert_eq!(inner(&wrapped)["id"], 0);
    }
    assert_eq!(fixture.bytes(), disk);
    session.finish().unwrap();
    direct.finish().unwrap();
}

#[test]
fn host_grants_control_discovery_and_notifications_never_execute_candidate_actions() {
    let fixture = Fixture::new();
    let base = fixture.candidate();
    let disk = fixture.bytes();
    for (prepare, build, test) in [
        (false, false, false),
        (true, false, false),
        (true, true, true),
    ] {
        let policy = VNextPolicy {
            candidate_prepare: prepare,
            build_enabled: build,
            test_policy: test.then(|| CandidateTestPolicy::new(100, 4096, 16384).unwrap()),
            ..Default::default()
        };
        let host = fixture.host(policy);
        let image = host.image_revision().to_owned();
        let mut session = McpSession::new(host).unwrap();
        ready(&mut session);
        let names = tools(&mut session)
            .into_iter()
            .map(|tool| tool["name"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(names.iter().any(|name| name == "candidate__open"), prepare);
        assert_eq!(names.iter().any(|name| name == "candidate__build"), build);
        assert_eq!(names.iter().any(|name| name == "candidate__test"), test);
        assert!(!names.iter().any(|name| name == "candidate__commit"));
        let notification=json!({"jsonrpc":"2.0","method":"tools/call","params":{"name":"candidate__open","arguments":{"image_revision":image}}}).to_string();
        assert!(session.handle_frame(notification.as_bytes()).is_none());
        let query = invoke(
            &mut session,
            json!(3),
            "candidate/query",
            json!({"image_revision":image,"candidate_revision":base.candidate_digest()}),
        );
        assert!(error(&query));
        assert!(error(&invoke(
            &mut session,
            json!(4),
            "candidate/commit",
            json!({"image_revision":image,"candidate_prepare":true,"approved":true})
        )));
        if prepare {
            let opened = invoke(
                &mut session,
                json!(5),
                "candidate/open",
                json!({"image_revision":image}),
            );
            assert_eq!(
                inner(&opened)["result"]["payload"]["candidate_revision"],
                base.candidate_digest()
            );
            assert_eq!(
                inner(&opened)["result"]["payload"]["source_authority"],
                false
            );
        }
        session.finish().unwrap();
    }
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn malformed_duplicate_fields_ids_and_arguments_fail_without_corrupting_lifecycle() {
    let fixture = Fixture::new();
    let mut session = McpSession::new(fixture.host(VNextPolicy::default())).unwrap();
    ready(&mut session);
    for raw in [
        b"{".as_slice(),
        br#"{"jsonrpc":"2.0","id":1,"id":2,"method":"ping"}"#.as_slice(),
        br#"{"jsonrpc":"2.0","id":null,"method":"ping"}"#.as_slice(),
        br#"{"jsonrpc":"2.0","id":true,"method":"ping"}"#.as_slice(),
        br#"{"jsonrpc":"2.0","id":1.5,"method":"ping"}"#.as_slice(),
    ] {
        let response: Value = serde_json::from_slice(&session.handle_frame(raw).unwrap()).unwrap();
        assert!(response.get("error").is_some(), "{response}");
    }
    for id in [json!(""), json!(-1), json!("x".repeat(128))] {
        let response = call(&mut session, id.clone(), "ping", json!({}));
        assert_eq!(response["id"], id);
        assert_eq!(response["result"], json!({}));
    }
    assert!(error(&call(
        &mut session,
        json!("x".repeat(129)),
        "ping",
        json!({})
    )));
    for params in [
        json!({"name":"workspace__open","arguments":[]}),
        json!({"name":"workspace__open","arguments":{"source_authority":true}}),
        json!({"name":"not_a_tool","arguments":{}}),
    ] {
        assert!(error(&call(&mut session, json!(1), "tools/call", params)));
    }
    let duplicate=br#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"workspace__open","arguments":{"x":1,"x":2}}}"#;
    let response: Value =
        serde_json::from_slice(&session.handle_frame(duplicate).unwrap()).unwrap();
    assert!(response.get("error").is_some());
    assert!(!error(&invoke(
        &mut session,
        json!(2),
        "workspace/open",
        json!({})
    )));
    session.finish().unwrap();
}

#[test]
fn request_bounds_reject_before_forwarding_and_outer_overflow_is_terminal() {
    let fixture = Fixture::new();
    let mut session = McpSession::new(fixture.host(VNextPolicy {
        candidate_prepare: true,
        ..Default::default()
    }))
    .unwrap();
    ready(&mut session);
    let large_request = request(
        json!(1),
        "tools/call",
        json!({"name":"workspace__open","arguments":{"padding":"x".repeat(70*1024)}}),
    );
    assert!(large_request.len() < 128 * 1024);
    let response: Value =
        serde_json::from_slice(&session.handle_frame(&large_request).unwrap()).unwrap();
    assert!(error(&response));
    assert!(!error(&invoke(
        &mut session,
        json!(2),
        "workspace/open",
        json!({})
    )));
    let oversized = vec![b' '; 128 * 1024 + 1];
    let response: Value =
        serde_json::from_slice(&session.handle_frame(&oversized).unwrap()).unwrap();
    assert!(response.get("error").is_some());
    assert!(session
        .handle_frame(&request(json!(3), "ping", json!({})))
        .is_none());
}

#[test]
fn eof_and_live_source_drift_still_use_inner_workspace_authentication() {
    let fixture = Fixture::new();
    let mut session = McpSession::new(fixture.host(VNextPolicy::default())).unwrap();
    ready(&mut session);
    let path = fixture.0.join("src/app.spx");
    let original = std::fs::read(&path).unwrap();
    std::fs::write(&path, b"invalid drift\n").unwrap();
    let failed = invoke(&mut session, json!(2), "workspace/open", json!({}));
    assert!(error(&failed));
    std::fs::write(&path, &original).unwrap();
    assert!(error(&invoke(
        &mut session,
        json!(3),
        "workspace/open",
        json!({})
    )));
    assert!(session.finish().is_err());
    let mut session = McpSession::new(fixture.host(VNextPolicy::default())).unwrap();
    std::fs::write(&path, b"drift only at EOF\n").unwrap();
    assert!(session.finish().is_err());
    std::fs::write(path, original).unwrap();
}

// Public injected Git authority, not a process or physical durability claim.
mod publication {
    use super::*;
    use semaprax::image_transport::{serve_mcp, GitCommitHost, VNextSessionFailure};
    use semaprax::project::{
        CandidateGitAuthority, CandidateGitCommitMetadata, CandidateGitObject,
        CandidateGitObjectKind, CandidateGitRefUpdate, CandidateGitRepository, CandidateGitTarget,
    };
    use sha2::{Digest, Sha256};
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::io::{self, Cursor, Write};
    use std::rc::Rc;
    struct Git {
        objects: BTreeMap<String, (CandidateGitObjectKind, Vec<u8>)>,
        current: String,
        pivots: usize,
        uncertain: bool,
    }
    #[derive(Clone)]
    struct Authority(Rc<RefCell<Git>>);
    impl CandidateGitAuthority for Authority {
        fn repository(&self) -> io::Result<CandidateGitRepository> {
            Ok(CandidateGitRepository {
                identity: "mcp-proof-repository".into(),
                bare: true,
                sha256: true,
            })
        }
        fn read_ref(&mut self, _: &str) -> io::Result<Option<String>> {
            Ok(Some(self.0.borrow().current.clone()))
        }
        fn read_object(&mut self, oid: &str, max: usize) -> io::Result<CandidateGitObject> {
            let git = self.0.borrow();
            let (kind, bytes) = git
                .objects
                .get(oid)
                .ok_or_else(|| io::Error::other("unknown object"))?;
            if bytes.len() > max {
                return Err(io::Error::other("bound"));
            }
            Ok(CandidateGitObject {
                kind: *kind,
                bytes: bytes.clone(),
            })
        }
        fn write_object(
            &mut self,
            kind: CandidateGitObjectKind,
            bytes: &[u8],
            oid: &str,
        ) -> io::Result<()> {
            self.0
                .borrow_mut()
                .objects
                .insert(oid.into(), (kind, bytes.to_vec()));
            Ok(())
        }
        fn compare_and_swap_ref(
            &mut self,
            _: &str,
            old: &str,
            new: &str,
        ) -> io::Result<CandidateGitRefUpdate> {
            let mut git = self.0.borrow_mut();
            if git.current != old {
                return Ok(CandidateGitRefUpdate::NotMatched);
            }
            git.current = new.into();
            git.pivots += 1;
            if git.uncertain {
                Err(io::Error::other("lost post-pivot acknowledgement"))
            } else {
                Ok(CandidateGitRefUpdate::Updated)
            }
        }
    }
    fn object(git: &mut Git, kind: CandidateGitObjectKind, bytes: &[u8]) -> String {
        let name = match kind {
            CandidateGitObjectKind::Blob => "blob",
            CandidateGitObjectKind::Tree => "tree",
            CandidateGitObjectKind::Commit => "commit",
        };
        let mut hash = Sha256::new();
        hash.update(format!("{name} {}\0", bytes.len()).as_bytes());
        hash.update(bytes);
        let id = format!("{:x}", semaprax::digest_hex::LowerHex(hash.finalize()));
        git.objects.insert(id.clone(), (kind, bytes.to_vec()));
        id
    }
    fn tree(git: &mut Git, mut entries: Vec<(String, &str, String)>) -> String {
        entries.sort_by_key(|(name, mode, _)| {
            format!("{name}{}", if *mode == "40000" { "/" } else { "\0" })
        });
        let mut bytes = Vec::new();
        for (name, mode, id) in entries {
            bytes.extend_from_slice(format!("{mode} {name}\0").as_bytes());
            for index in (0..64).step_by(2) {
                bytes.push(u8::from_str_radix(&id[index..index + 2], 16).unwrap());
            }
        }
        object(git, CandidateGitObjectKind::Tree, &bytes)
    }
    fn approved(
        fixture: &Fixture,
        uncertain: bool,
    ) -> (McpSession, Authority, String, String, String) {
        let base = fixture.candidate();
        let revision = base.revision();
        let change=SemanticChange::new(revision.project_revision(),&json!({"kind":"replace_function_body","target":"calculator.add","body":{"kind":"i64","value":42}})).unwrap();
        let candidate = base.apply(base.candidate_digest(), &change).unwrap();
        let digest = candidate.candidate_digest().to_owned();
        let mut git = Git {
            objects: BTreeMap::new(),
            current: String::new(),
            pivots: 0,
            uncertain,
        };
        let mut sources = Vec::new();
        for source in revision.sources() {
            let oid = object(
                &mut git,
                CandidateGitObjectKind::Blob,
                source.source().as_bytes(),
            );
            sources.push((
                source.path().strip_prefix("src/").unwrap().to_owned(),
                "100644",
                oid,
            ));
        }
        let source_tree = tree(&mut git, sources);
        let manifest = object(
            &mut git,
            CandidateGitObjectKind::Blob,
            revision.manifest().to_canonical_toml().as_bytes(),
        );
        let root = tree(
            &mut git,
            vec![
                ("semaprax.toml".into(), "100644", manifest),
                ("src".into(), "40000", source_tree),
            ],
        );
        git.current=object(&mut git,CandidateGitObjectKind::Commit,format!("tree {root}\nauthor Host <host@example.invalid> 1 +0000\ncommitter Host <host@example.invalid> 1 +0000\n\nBase\n").as_bytes());
        let authority = Authority(Rc::new(RefCell::new(git)));
        let target = CandidateGitTarget::new(
            "mcp-proof-repository",
            "refs/heads/approved",
            &authority.0.borrow().current,
            "",
        )
        .unwrap();
        let metadata =
            CandidateGitCommitMetadata::new("Host", "host@example.invalid", 2, "Approved\n")
                .unwrap();
        let host = GitCommitHost::new(
            &fixture.0.join("semaprax.toml"),
            target,
            metadata,
            Box::new(authority.clone()),
        )
        .unwrap();
        let mut session = fixture.host(VNextPolicy {
            candidate_prepare: true,
            ..Default::default()
        });
        session
            .retain_archived_candidate(candidate, &digest)
            .unwrap();
        let mut session = session.with_git_commit_host(host).unwrap();
        let approval = session.approve_git_commit(&digest).unwrap();
        let image = session.image_revision().to_owned();
        (
            McpSession::new(session).unwrap(),
            authority,
            image,
            digest,
            approval,
        )
    }
    struct AfterPivot {
        authority: Authority,
        fail: bool,
        drift: Option<PathBuf>,
        bytes: Vec<u8>,
    }
    impl Write for AfterPivot {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.authority.0.borrow().pivots > 0 {
                if self.fail {
                    return Err(io::Error::new(
                        io::ErrorKind::BrokenPipe,
                        "injected after publication",
                    ));
                }
                if let Some(path) = self.drift.take() {
                    std::fs::write(path, b"post-publication source drift\n")?;
                }
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    fn input(image: &str, candidate: &str, approval: &str) -> Vec<u8> {
        let initialize = request(
            json!(1),
            "initialize",
            json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"publication-evidence","version":"1"}}),
        );
        let call = request(
            json!(2),
            "tools/call",
            json!({"name":"candidate__commit","arguments":{"image_revision":image,"candidate_revision":candidate,"approval_revision":approval}}),
        );
        [
            initialize,
            br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.to_vec(),
            call,
        ]
        .into_iter()
        .flat_map(|mut row| {
            row.push(b'\n');
            row
        })
        .collect()
    }
    #[test]
    fn writer_failure_after_mock_cas_preserves_known_or_uncertain_publication_classification() {
        for uncertain in [false, true] {
            let fixture = Fixture::new();
            let disk = fixture.bytes();
            let (session, authority, image, candidate, approval) = approved(&fixture, uncertain);
            let previous = authority.0.borrow().current.clone();
            let mut writer = AfterPivot {
                authority: authority.clone(),
                fail: true,
                drift: None,
                bytes: Vec::new(),
            };
            let error = serve_mcp(
                Cursor::new(input(&image, &candidate, &approval)),
                &mut writer,
                session,
            )
            .unwrap_err();
            let failure = error
                .get_ref()
                .unwrap()
                .downcast_ref::<VNextSessionFailure>()
                .unwrap();
            let expected = if uncertain {
                "publication_uncertain"
            } else {
                "published"
            };
            assert!(failure
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code == "SPX-G287"
                    && diagnostic.message.contains(expected)));
            assert_eq!(authority.0.borrow().pivots, 1);
            assert_ne!(authority.0.borrow().current, previous);
            assert_eq!(fixture.bytes(), disk);
        }
    }
    #[test]
    fn source_drift_after_delivered_mock_commit_keeps_g287_at_eof() {
        let fixture = Fixture::new();
        let (session, authority, image, candidate, approval) = approved(&fixture, false);
        let mut writer = AfterPivot {
            authority: authority.clone(),
            fail: false,
            drift: Some(fixture.0.join("src/app.spx")),
            bytes: Vec::new(),
        };
        let error = serve_mcp(
            Cursor::new(input(&image, &candidate, &approval)),
            &mut writer,
            session,
        )
        .unwrap_err();
        let failure = error
            .get_ref()
            .unwrap()
            .downcast_ref::<VNextSessionFailure>()
            .unwrap();
        assert!(failure
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-G287"
                && diagnostic.message.contains("published")));
        assert_eq!(authority.0.borrow().pivots, 1);
        let rows = String::from_utf8(writer.bytes)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            inner(rows.last().unwrap())["result"]["payload"]["state"],
            "published"
        );
    }
}
