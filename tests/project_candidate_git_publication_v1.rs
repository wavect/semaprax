//! Real bare-Git publication and host-lease regressions.
#![cfg(unix)]
use semaprax::project::{
    apply_candidate_git_publication, with_authenticated_project, CandidateGitCommitMetadata,
    CandidateGitProcessAuthority, CandidateGitTarget, GitObjectFormat, ProjectCandidate,
    SemanticChange,
};
use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    root: PathBuf,
    repo: PathBuf,
    git: PathBuf,
    base: String,
    candidate: ProjectCandidate,
}
impl Fixture {
    fn new(corrupt_base: bool) -> Self {
        Self::with_format(corrupt_base, "sha256")
    }
    fn with_format(corrupt_base: bool, format: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-git-publication-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        let root = root.canonicalize().unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            fs::copy(example.join(path), root.join(path)).unwrap();
        }
        let revision = with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap();
        fs::write(
            root.join("semaprax.toml"),
            revision.manifest().to_canonical_toml(),
        )
        .unwrap();
        for source in revision.sources() {
            fs::write(root.join(source.path()), source.source()).unwrap();
        }
        let candidate =
            ProjectCandidate::open(revision.clone(), revision.project_revision()).unwrap();
        let change=SemanticChange::new(candidate.revision().project_revision(),&json!({"kind":"change_function_signature","target":"calculator.add","append_parameters":[{"name":"unused","type":"i64","argument":{"kind":"i64","value":0}}]})).unwrap();
        let candidate = candidate
            .apply(candidate.candidate_digest(), &change)
            .unwrap();
        let git = PathBuf::from(
            std::env::var_os("SEMAPRAX_TEST_GIT").unwrap_or_else(|| "/usr/bin/git".into()),
        )
        .canonicalize()
        .unwrap();
        let repo = root.join("published.git");
        let mut init = Command::new(&git);
        init.env_clear()
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .args([
                "-c",
                "init.templateDir=",
                "init",
                "--bare",
                &format!("--object-format={format}"),
            ])
            .arg(&repo);
        assert!(init.output().unwrap().status.success());
        // git init probes host filesystem settings (including ignorecase and
        // precomposeunicode on macOS). This host admits only a minimal config.
        let config = match format {
            "sha1" => "[core]\nrepositoryformatversion = 0\nbare = true\n",
            "sha256" => "[core]\nrepositoryformatversion = 1\nbare = true\n[extensions]\nobjectformat = sha256\n",
            _ => panic!("unsupported fixture object format"),
        };
        fs::write(repo.join("config"), config).unwrap();
        let mut value = Self {
            root,
            repo,
            git,
            base: String::new(),
            candidate,
        };
        let mut sources = Vec::new();
        for source in revision.sources() {
            let body = if corrupt_base && source.path() == "src/core.spx" {
                b"wrong original bytes\n".as_slice()
            } else {
                source.source().as_bytes()
            };
            let oid = value.object("blob", body);
            sources.push((
                "100644",
                source.path().strip_prefix("src/").unwrap().to_owned(),
                oid,
            ));
        }
        let source_tree = value.tree(sources);
        let manifest = value.object("blob", revision.manifest().to_canonical_toml().as_bytes());
        let unrelated = value.object("blob", b"unrelated existing entry\n");
        let tree = value.tree(vec![
            ("100755", "keep.sh".to_owned(), unrelated),
            ("100644", "semaprax.toml".to_owned(), manifest),
            ("40000", "src".to_owned(), source_tree),
        ]);
        let commit=format!("tree {tree}\nauthor Test <test@example.invalid> 1 +0000\ncommitter Test <test@example.invalid> 1 +0000\n\nOriginal\n");
        value.base = value.object("commit", commit.as_bytes());
        value.run(&["update-ref", "refs/heads/review", &value.base], &[]);
        value
    }
    fn run(&self, args: &[&str], input: &[u8]) -> Vec<u8> {
        let mut child = Command::new(&self.git)
            .env_clear()
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .arg(format!("--git-dir={}", self.repo.display()))
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(input).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success(), "{:?}", output);
        output.stdout
    }
    fn object(&self, kind: &str, bytes: &[u8]) -> String {
        String::from_utf8(self.run(&["hash-object", "-w", "--stdin", "-t", kind], bytes))
            .unwrap()
            .trim_end()
            .to_owned()
    }
    fn tree(&self, mut entries: Vec<(&str, String, String)>) -> String {
        entries.sort_by_key(|(mode, name, _)| {
            format!("{name}{}", if *mode == "40000" { "/" } else { "\0" })
        });
        let mut bytes = Vec::new();
        for (mode, name, oid) in entries {
            bytes.extend_from_slice(format!("{mode} {name}\0").as_bytes());
            for index in (0..oid.len()).step_by(2) {
                bytes.push(u8::from_str_radix(&oid[index..index + 2], 16).unwrap());
            }
        }
        self.object("tree", &bytes)
    }
    fn authority(&self) -> CandidateGitProcessAuthority {
        CandidateGitProcessAuthority::open(&self.git, &self.repo, 4096, 60_000).unwrap()
    }
    fn publish(&self, base: &str) -> Result<String, Vec<semaprax::diagnostic::Diagnostic>> {
        let mut authority = self.authority();
        let target = CandidateGitTarget::new(
            authority.repository_identity(),
            "refs/heads/review",
            base,
            "",
        )
        .unwrap();
        let metadata = CandidateGitCommitMetadata::new(
            "Host reviewer",
            "review@example.invalid",
            2,
            "Approved semantic candidate\n",
        )
        .unwrap();
        apply_candidate_git_publication(
            &self.candidate,
            self.candidate.candidate_digest(),
            &self.root.join("semaprax.toml"),
            &target,
            &metadata,
            &mut authority,
        )
    }
    fn current(&self) -> String {
        String::from_utf8(self.run(
            &["show-ref", "--verify", "--hash", "refs/heads/review"],
            &[],
        ))
        .unwrap()
        .trim_end()
        .to_owned()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
#[test]
fn actual_ref_publication_preserves_unrelated_tree_and_raw_project_and_disables_hooks() {
    let fixture = Fixture::new(false);
    let before = fs::read(fixture.root.join("src/core.spx")).unwrap();
    fs::create_dir_all(fixture.repo.join("hooks")).unwrap();
    let hook = fixture.repo.join("hooks/reference-transaction");
    fs::write(&hook, "#!/bin/sh\nexit 91\n").unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    let receipt: Value = serde_json::from_str(&fixture.publish(&fixture.base).unwrap()).unwrap();
    assert_eq!(
        receipt["schema"],
        "semaprax.project-candidate-git-publication.v1"
    );
    assert_eq!(receipt["published_commit"], fixture.current());
    assert_ne!(fixture.current(), fixture.base);
    assert_eq!(
        fixture.run(&["show", "refs/heads/review:keep.sh"], &[]),
        b"unrelated existing entry\n"
    );
    assert!(
        String::from_utf8(fixture.run(&["ls-tree", "refs/heads/review", "keep.sh"], &[]))
            .unwrap()
            .starts_with("100755 blob ")
    );
    let expected = fixture
        .candidate
        .revision()
        .sources()
        .iter()
        .find(|source| source.path() == "src/core.spx")
        .unwrap();
    assert_eq!(
        fixture.run(&["show", "refs/heads/review:src/core.spx"], &[]),
        expected.source().as_bytes()
    );
    assert_eq!(fs::read(fixture.root.join("src/core.spx")).unwrap(), before);
    assert_eq!(receipt["working_tree_rewritten"], false);
    assert!(receipt.get("sha256_object_content_binding").is_none());
    assert!(receipt.get("sha1_security").is_none());
}
#[test]
fn stale_ref_and_original_blob_mismatch_never_pivot() {
    let fixture = Fixture::new(false);
    let errors = fixture.publish(&"0".repeat(64)).unwrap_err();
    assert!(errors.iter().any(|e| e.code == "SPX-G265"));
    assert_eq!(fixture.current(), fixture.base);
    let fixture = Fixture::new(true);
    let errors = fixture.publish(&fixture.base).unwrap_err();
    assert!(errors.iter().any(|e| e.code == "SPX-G265"));
    assert_eq!(fixture.current(), fixture.base);
}
#[test]
fn unsafe_config_and_nested_storage_redirection_are_rejected() {
    let fixture = Fixture::new(false);
    let config = fixture.repo.join("config");
    let original = fs::read(&config).unwrap();
    for setting in ["ignorecase", "precomposeunicode"] {
        let mut changed = original.clone();
        changed.extend_from_slice(format!("[core]\n{setting} = true\n").as_bytes());
        fs::write(&config, changed).unwrap();
        let errors = CandidateGitProcessAuthority::open(&fixture.git, &fixture.repo, 100, 60_000)
            .err()
            .expect("unknown filesystem settings must remain rejected");
        assert!(errors.iter().any(|error| error.code == "SPX-G263"));
    }
    let mut changed = original.clone();
    changed.extend_from_slice(b"[include]\npath = /tmp/ambient-git-config\n");
    fs::write(&config, changed).unwrap();
    assert!(CandidateGitProcessAuthority::open(&fixture.git, &fixture.repo, 100, 60_000).is_err());
    fs::write(&config, original).unwrap();
    fs::create_dir(fixture.root.join("outside")).unwrap();
    symlink(
        fixture.root.join("outside"),
        fixture.repo.join("objects/zz"),
    )
    .unwrap();
    assert!(CandidateGitProcessAuthority::open(&fixture.git, &fixture.repo, 100, 60_000).is_err());
    assert_eq!(fixture.current(), fixture.base);
}

#[test]
fn host_lease_excludes_contenders_and_reopens_after_drop_and_rejected_admission() {
    let fixture = Fixture::with_format(false, "sha1");
    let before = fs::read(fixture.root.join("src/core.spx")).unwrap();
    let authority = fixture.authority();
    for _ in 0..2 {
        let errors = CandidateGitProcessAuthority::open(&fixture.git, &fixture.repo, 4096, 60_000)
            .err()
            .expect("a rejected contender must not release the active host lease");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "SPX-G266");
        assert_eq!(errors[0].message, "Git publication host is already leased");
        assert_eq!(fixture.current(), fixture.base);
    }
    drop(authority);

    let config = fixture.repo.join("config");
    let original = fs::read(&config).unwrap();
    fs::write(&config, b"[include]\npath = /tmp/ambient-git-config\n").unwrap();
    let errors = CandidateGitProcessAuthority::open(&fixture.git, &fixture.repo, 4096, 60_000)
        .err()
        .expect("invalid config must reject after taking the lease");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, "SPX-G263");
    fs::write(&config, original).unwrap();
    assert_eq!(fixture.current(), fixture.base);
    let receipt: Value = serde_json::from_str(&fixture.publish(&fixture.base).unwrap()).unwrap();
    assert_eq!(receipt["published_commit"], fixture.current());
    assert_ne!(fixture.current(), fixture.base);
    assert_eq!(fs::read(fixture.root.join("src/core.spx")).unwrap(), before);
    assert!(fixture
        .repo
        .join(".semaprax-git-publication.lock")
        .is_file());
}

#[test]
fn sha1_bare_publication_binds_exact_object_bytes_with_sha256() {
    let fixture = Fixture::with_format(false, "sha1");
    assert_eq!(fixture.base.len(), 40);
    assert_eq!(fixture.authority().object_format(), GitObjectFormat::Sha1);
    let receipt: Value = serde_json::from_str(&fixture.publish(&fixture.base).unwrap()).unwrap();
    assert_eq!(receipt["git_object_format"], "sha1");
    assert_eq!(receipt["published_commit"].as_str().unwrap().len(), 40);
    assert_eq!(receipt["published_commit"], fixture.current());
    assert_eq!(
        receipt["sha256_object_content_binding"]
            .as_str()
            .unwrap()
            .len(),
        71
    );
    assert_eq!(
        receipt["sha1_security"],
        "legacy_git_compatibility_no_collision_detection_or_collision_resistance_claim"
    );
    let expected = fixture
        .candidate
        .revision()
        .sources()
        .iter()
        .find(|source| source.path() == "src/core.spx")
        .unwrap();
    assert_eq!(
        fixture.run(&["show", "refs/heads/review:src/core.spx"], &[]),
        expected.source().as_bytes()
    );
    assert_eq!(
        fixture.run(&["show", "refs/heads/review:keep.sh"], &[]),
        b"unrelated existing entry\n"
    );
}
#[test]
fn sha1_host_rejects_sha256_target_and_accepts_explicit_format_one_sha1() {
    let fixture = Fixture::with_format(false, "sha1");
    assert!(fixture
        .publish(&"0".repeat(64))
        .unwrap_err()
        .iter()
        .any(|error| error.code == "SPX-G263"));
    assert_eq!(fixture.current(), fixture.base);
    let config = fixture.repo.join("config");
    fs::write(
        &config,
        "[core]\nrepositoryformatversion = 1\nbare = true\n[extensions]\nobjectformat = sha1\n",
    )
    .unwrap();
    assert_eq!(fixture.authority().object_format(), GitObjectFormat::Sha1);
    fs::write(
        &config,
        "[core]\nrepositoryformatversion = 0\nbare = true\n[extensions]\nobjectformat = sha256\n",
    )
    .unwrap();
    assert!(CandidateGitProcessAuthority::open(&fixture.git, &fixture.repo, 100, 60_000).is_err());
}
