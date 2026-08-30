//! Injected-authority regressions; authored, not executed in this batch.
use super::*;
use crate::project::{with_authenticated_project, SemanticChange};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
static SERIAL: AtomicU64 = AtomicU64::new(0);
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
            identity: "host-selected-test-repository".to_owned(),
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
            .insert(oid.to_owned(), (kind, bytes.to_vec()));
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
        git.pivots += 1;
        git.current = new.to_owned();
        if git.uncertain {
            Err(io::Error::other("host lost post-pivot acknowledgment"))
        } else {
            Ok(CandidateGitRefUpdate::Updated)
        }
    }
}
struct Fixture {
    root: PathBuf,
    candidate: Arc<ProjectCandidate>,
    authority: Authority,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-commit-host-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let root = root.canonicalize().unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(example.join(path), root.join(path)).unwrap();
        }
        let revision = with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap();
        let base =
            ProjectCandidate::open(Arc::clone(&revision), revision.project_revision()).unwrap();
        let change=SemanticChange::new(revision.project_revision(),&json!({"kind":"replace_function_body","target":"calculator.add","body":{"kind":"i64","value":42}})).unwrap();
        let candidate = Arc::new(base.apply(base.candidate_digest(), &change).unwrap());
        let mut git = Git {
            objects: BTreeMap::new(),
            current: String::new(),
            pivots: 0,
            uncertain: false,
        };
        let mut source_entries = Vec::new();
        for source in revision.sources() {
            let oid = object(
                &mut git,
                CandidateGitObjectKind::Blob,
                source.source().as_bytes(),
            );
            source_entries.push((
                source.path().strip_prefix("src/").unwrap().to_owned(),
                "100644",
                oid,
            ));
        }
        let source_tree = tree(&mut git, source_entries);
        let manifest = object(
            &mut git,
            CandidateGitObjectKind::Blob,
            revision.manifest().to_canonical_toml().as_bytes(),
        );
        let root_tree = tree(
            &mut git,
            vec![
                ("semaprax.toml".to_owned(), "100644", manifest),
                ("src".to_owned(), "40000", source_tree),
            ],
        );
        git.current=object(&mut git,CandidateGitObjectKind::Commit,format!("tree {root_tree}\nauthor Host <host@example.invalid> 1 +0000\ncommitter Host <host@example.invalid> 1 +0000\n\nBase\n").as_bytes());
        Self {
            root,
            candidate,
            authority: Authority(Rc::new(RefCell::new(git))),
        }
    }
    fn manifest(&self) -> PathBuf {
        self.root.join("semaprax.toml")
    }
    fn host(&self) -> GitCommitHost {
        self.host_with_base(&self.authority.0.borrow().current)
    }
    fn host_with_base(&self, base: &str) -> GitCommitHost {
        let target = CandidateGitTarget::new(
            "host-selected-test-repository",
            "refs/heads/approved",
            base,
            "",
        )
        .unwrap();
        let metadata =
            CandidateGitCommitMetadata::new("Host", "host@example.invalid", 2, "Approved change\n")
                .unwrap();
        GitCommitHost::new(
            &self.manifest(),
            target,
            metadata,
            Box::new(self.authority.clone()),
        )
        .unwrap()
    }
    fn params(&self, approval: &str) -> Map<String, Value> {
        json!({"candidate_revision":self.candidate.candidate_digest(),"approval_revision":approval})
            .as_object()
            .unwrap()
            .clone()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
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
    let oid = format!("{:x}", crate::digest_hex::LowerHex(hash.finalize()));
    git.objects.insert(oid.clone(), (kind, bytes.to_vec()));
    oid
}
fn tree(git: &mut Git, mut entries: Vec<(String, &str, String)>) -> String {
    entries.sort_by_key(|(name, mode, _)| {
        format!("{name}{}", if *mode == "40000" { "/" } else { "\0" })
    });
    let mut bytes = Vec::new();
    for (name, mode, oid) in entries {
        bytes.extend_from_slice(format!("{mode} {name}\0").as_bytes());
        for n in (0..64).step_by(2) {
            bytes.push(u8::from_str_radix(&oid[n..n + 2], 16).unwrap());
        }
    }
    object(git, CandidateGitObjectKind::Tree, &bytes)
}
#[test]
fn request_digest_cannot_self_approve_or_replace_host_approval() {
    let fixture = Fixture::new();
    let mut host = fixture.host();
    let params = fixture.params(&format!("sha256:{}", "0".repeat(64)));
    assert!(host
        .execute(&fixture.candidate, &fixture.manifest(), &params)
        .unwrap_err()
        .iter()
        .any(|e| e.code == "SPX-G286"));
    assert_eq!(fixture.authority.0.borrow().pivots, 0);
    let approval = host.approve(fixture.candidate.candidate_digest()).unwrap();
    assert!(host.approve(fixture.candidate.candidate_digest()).is_err());
    assert!(host
        .execute(&fixture.candidate, &fixture.manifest(), &params)
        .is_err());
    assert_eq!(
        host.status()["pending_approval"]["approval_revision"],
        approval
    );
}
#[test]
fn success_consumes_approval_retains_receipt_and_cannot_publish_twice() {
    let fixture = Fixture::new();
    let before = std::fs::read(fixture.root.join("src/core.spx")).unwrap();
    let mut host = fixture.host();
    let approval = host.approve(fixture.candidate.candidate_digest()).unwrap();
    let params = fixture.params(&approval);
    let result = host
        .execute(&fixture.candidate, &fixture.manifest(), &params)
        .unwrap();
    assert_eq!(result["state"], "published");
    assert!(host.is_terminal());
    assert!(host.status()["pending_approval"].is_null());
    assert_eq!(fixture.authority.0.borrow().pivots, 1);
    assert!(host
        .execute(&fixture.candidate, &fixture.manifest(), &params)
        .is_err());
    assert!(host.approve(fixture.candidate.candidate_digest()).is_err());
    let query = json!({"report_revision":result["report_revision"],"chunk_bytes":32768});
    let report = host.report(query.as_object().unwrap()).unwrap();
    assert!(report["chunk"]
        .as_str()
        .unwrap()
        .contains("git_branch_ref_compare_and_swap"));
    assert_eq!(
        std::fs::read(fixture.root.join("src/core.spx")).unwrap(),
        before
    );
}
#[test]
fn uncertain_pivot_is_terminal_but_definite_preflight_consumes_only_approval() {
    let fixture = Fixture::new();
    let mut host = fixture.host_with_base(&"0".repeat(64));
    let approval = host.approve(fixture.candidate.candidate_digest()).unwrap();
    assert!(host
        .execute(
            &fixture.candidate,
            &fixture.manifest(),
            &fixture.params(&approval)
        )
        .unwrap_err()
        .iter()
        .any(|e| e.code == "SPX-G265"));
    assert!(!host.is_terminal());
    assert!(host.status()["pending_approval"].is_null());
    assert_eq!(fixture.authority.0.borrow().pivots, 0);
    let mut host = fixture.host();
    fixture.authority.0.borrow_mut().uncertain = true;
    let approval = host.approve(fixture.candidate.candidate_digest()).unwrap();
    assert!(host
        .execute(
            &fixture.candidate,
            &fixture.manifest(),
            &fixture.params(&approval)
        )
        .unwrap_err()
        .iter()
        .any(|e| e.code == "SPX-G267"));
    assert!(host.is_terminal());
    assert_eq!(host.status()["state"], "publication_uncertain");
    assert!(host.approve(fixture.candidate.candidate_digest()).is_err());
    assert_eq!(fixture.authority.0.borrow().pivots, 1);
}

#[test]
fn v5_commit_requires_startup_authority_and_preserves_terminal_status() {
    use crate::image_transport::vnext::{VNextPolicy, VNextSession};
    fn request(session: &mut VNextSession, method: &str, mut params: Value) -> Value {
        params["image_revision"] = json!(session.image_revision());
        let bytes = json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}).to_string();
        serde_json::from_slice(&session.handle_frame(bytes.as_bytes()).unwrap()).unwrap()
    }
    let fixture = Fixture::new();
    let policy = VNextPolicy {
        candidate_prepare: true,
        ..VNextPolicy::default()
    };
    let mut unprivileged = VNextSession::open(&fixture.manifest(), policy).unwrap();
    assert_eq!(
        request(&mut unprivileged, "source-commit/status", json!({}))["error"]["code"],
        -32601
    );
    let mut host = fixture.host();
    let approval = host.approve(fixture.candidate.candidate_digest()).unwrap();
    let mut session = VNextSession::open(&fixture.manifest(), policy)
        .unwrap()
        .with_git_commit_host(host)
        .unwrap();
    let capsule: Value =
        serde_json::from_str(&fixture.candidate.recovery_capsule().unwrap()).unwrap();
    let restored = request(
        &mut session,
        "candidate/recovery-restore",
        json!({"capsule":capsule}),
    );
    assert!(restored.get("error").is_none(), "{restored}");
    assert!(session
        .approve_git_commit(fixture.candidate.candidate_digest())
        .is_err());
    let committed = request(
        &mut session,
        "candidate/commit",
        json!({"candidate_revision":fixture.candidate.candidate_digest(),"approval_revision":approval}),
    );
    assert!(committed.get("error").is_none(), "{committed}");
    assert_eq!(committed["result"]["payload"]["state"], "published");
    std::fs::write(
        fixture.root.join("src/core.spx"),
        "post-publication source drift",
    )
    .unwrap();
    let status = request(&mut session, "source-commit/status", json!({}));
    assert_eq!(status["result"]["payload"]["state"], "published");
    assert_eq!(fixture.authority.0.borrow().pivots, 1);
}
