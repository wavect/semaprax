//! HP-05 context broker tests (fixture prefix `hp-hp05`). Compiler-backed cases
//! use `$SEMAPRAX_COMPILER` when set and a deterministic fake otherwise; the
//! fake never stands in for the comparison against standalone output.

use crate::support::{fixture_dir, write};
use semaprax_harness::cli::{run, Environment};
use semaprax_harness::context::cache::{system_clock, CacheConfig, ResultCache};
use semaprax_harness::context::external::{Coverage, RawItem};
use semaprax_harness::context::identity::{span_digest, Snapshot};
use semaprax_harness::context::item::{Link, Span, Tier};
use semaprax_harness::context::native::{
    parse_facts, NativeContextSource, NativeFacts, NativeQuery, SubprocessNative,
};
use semaprax_harness::context::{
    Broker, BrokerRequest, ExternalQuery, ExternalResponse, ExternalSource, HostExternal,
    ProviderIdentity,
};
use semaprax_harness::contract::CapabilityKind;
use semaprax_harness::diag::{HarnessDiagnostic, HarnessResult};
use semaprax_harness::json::sha256_plain;
use semaprax_harness::profile::{resolve_project, HarnessConfig};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

const PREFIX: &str = "hp-hp05";
const SI_ID: &str = "org.example/source-index";
const GI_ID: &str = "org.example/graph-index";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/context")
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let (p, q) = (e.path(), to.join(e.file_name()));
        if p.is_dir() {
            copy_tree(&p, &q);
        } else {
            std::fs::copy(&p, &q).unwrap();
        }
    }
}

fn python() -> String {
    let out = std::process::Command::new("/usr/bin/which")
        .arg("python3")
        .output()
        .expect("which");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn real_compiler() -> Option<PathBuf> {
    std::env::var_os("SEMAPRAX_COMPILER").map(PathBuf::from)
}

/// Deterministic stand-in used only when no real compiler is configured.
struct FakeNative {
    pad: usize,
}

impl NativeContextSource for FakeNative {
    fn identity(&self) -> String {
        "fake-compiler".into()
    }
    fn facts(&self, _p: &Path, target: &str, q: &NativeQuery) -> HarnessResult<NativeFacts> {
        let raw = json!({"schema": "semaprax.agent-context.v1", "root": target, "query": {"max_bytes": q.max_bytes},
                         "truncation": {"truncated": false}, "pad": "x".repeat(self.pad),
                         "facts": [{"id": target, "effects": [], "types": {"result": "i64"}, "contracts": {"requires": []}}]})
        .to_string();
        parse_facts(&raw)
    }
}

fn native() -> Box<dyn NativeContextSource> {
    match real_compiler() {
        Some(c) => Box::new(SubprocessNative::new(c)),
        None => Box::new(FakeNative { pad: 0 }),
    }
}

struct World {
    home: PathBuf,
    project: PathBuf,
    env: Environment,
}

fn world() -> World {
    let root = fixture_dir(PREFIX).canonicalize().unwrap();
    let project = root.join("project");
    copy_tree(&fixtures().join("mixed"), &project);
    let home = root.join("home");
    std::fs::create_dir_all(&home).unwrap();
    let mut vars = BTreeMap::new();
    vars.insert(
        "HARNESS_PYTHON".to_string(),
        std::env::var("HARNESS_PYTHON").unwrap_or_else(|_| python()),
    );
    vars.insert("PATH".to_string(), "/usr/bin:/bin".to_string());
    let env = Environment {
        harness_home: Some(home.clone()),
        compiler: real_compiler(),
        cwd: root,
        vars,
    };
    World { home, project, env }
}

fn sh(w: &World, args: &[&str]) -> semaprax_harness::cli::Outcome {
    run(
        &args.iter().map(|a| a.to_string()).collect::<Vec<_>>(),
        &w.env,
    )
}

/// Adopt + trust a descriptor, then pin it for `context.repository`.
fn install(w: &World, desc: &Path, id: &str) {
    let o = sh(w, &["adopt", desc.to_str().unwrap()]);
    assert_eq!(o.code, 0, "adopt: {}{}", o.stdout, o.stderr);
    let o = sh(w, &["trust", id]);
    assert_eq!(o.code, 0, "trust: {}", o.stderr);
    write(
        &w.project,
        "semaprax.harness.toml",
        &format!("schema = \"semaprax.harness-config.v1\"\n\n[capability.\"context.repository\"]\nmode = \"required\"\nprovider = \"{id}\"\n"),
    );
}

fn install_source_index(w: &World) {
    let ex = w.home.parent().unwrap().join("ex");
    let adapters = crate::support::repo_root().join("packages/semaprax-harness-adapters");
    let dir = ex.join("examples/source-index-python");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::create_dir_all(ex.join("sdk/python")).unwrap();
    std::fs::copy(
        adapters.join("sdk/python/semaprax_harness_adapter.py"),
        ex.join("sdk/python/semaprax_harness_adapter.py"),
    )
    .unwrap();
    let src = adapters.join("examples/source-index-python");
    for f in ["adapter.py", "index.py"] {
        std::fs::copy(src.join(f), dir.join(f)).unwrap();
    }
    let mut d: Value =
        serde_json::from_slice(&std::fs::read(src.join("harness-provider.json")).unwrap()).unwrap();
    d.as_object_mut().unwrap().remove("upstream");
    std::fs::write(dir.join("harness-provider.json"), d.to_string()).unwrap();
    install(w, &dir.join("harness-provider.json"), SI_ID);
}

fn install_graph_index(w: &World) {
    install(
        w,
        &fixtures().join("graph-index/harness-provider.json"),
        GI_ID,
    );
}

fn broker_with_cache(w: &World, cache: bool) -> Broker {
    let res = resolve_project(&w.env, &w.project).expect("resolve");
    let cfg = HarnessConfig::load(&w.project).unwrap();
    let cache = cache.then(|| {
        ResultCache::new(
            w.home.join("cache/context"),
            CacheConfig::default(),
            system_clock(),
        )
    });
    let mut b = Broker::new(Some(native()), cache);
    let l = res
        .launches
        .get(&CapabilityKind::ContextRepository)
        .expect("selected external provider");
    let scope = cfg.capability(CapabilityKind::ContextRepository).scope;
    b.add_provider(Box::new(HostExternal::new(
        l.clone(),
        w.env.clone(),
        res.profile.lock_digest(),
        res.profile.config_digest.clone(),
        scope,
    )))
    .unwrap();
    b
}

fn doc(o: &semaprax_harness::context::BrokerOutput) -> Value {
    serde_json::from_str(&o.rendered).unwrap()
}

fn req(q: &str, max: usize) -> BrokerRequest {
    BrokerRequest::new(q, max)
}

// ---- counting fake provider --------------------------------------------------

type Answer = Box<dyn Fn(&Snapshot, &ExternalQuery) -> ExternalResponse + Send + Sync>;

struct FakeSource {
    id: String,
    version: String,
    scope: Vec<String>,
    answer: Answer,
    queries: Arc<AtomicUsize>,
    rechecks: Arc<AtomicUsize>,
    allowed: Arc<AtomicBool>,
}

impl FakeSource {
    fn new(id: &str, answer: Answer) -> Self {
        Self {
            id: id.into(),
            version: "1.0.0".into(),
            scope: vec![],
            answer,
            queries: Arc::default(),
            rechecks: Arc::default(),
            allowed: Arc::new(AtomicBool::new(true)),
        }
    }
}

impl ExternalSource for FakeSource {
    fn identity(&self) -> ProviderIdentity {
        ProviderIdentity {
            provider_id: self.id.clone(),
            provider_version: self.version.clone(),
            adapter_version: "0.1.0".into(),
            upstream_version: None,
            descriptor_digest: "sha256:d".into(),
            config_digest: "sha256:c".into(),
            permission_scope: json!({"read": ["project"]}),
        }
    }
    fn scope(&self) -> Vec<String> {
        self.scope.clone()
    }
    fn recheck_authority(&self) -> HarnessResult<()> {
        self.rechecks.fetch_add(1, Ordering::SeqCst);
        if self.allowed.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err(HarnessDiagnostic::new("SPX-HPB034", "revoked"))
        }
    }
    fn query(
        &self,
        snap: &Snapshot,
        q: &ExternalQuery,
        _max: usize,
    ) -> HarnessResult<ExternalResponse> {
        self.queries.fetch_add(1, Ordering::SeqCst);
        Ok((self.answer)(snap, q))
    }
}

fn complete() -> Coverage {
    Coverage {
        complete: true,
        exhaustive: true,
        indexed_files: 1,
        skipped: vec![],
    }
}

/// One item per line of every non-`.spx` file containing the query token.
fn grep_answer(id: &'static str) -> Answer {
    Box::new(move |snap, q| {
        let tok = q.payload["query"]
            .as_str()
            .or(q.payload["symbol"].as_str())
            .unwrap_or("")
            .to_string();
        let mut items = Vec::new();
        for rel in snap.files.keys() {
            let Ok(t) = std::fs::read_to_string(snap.root.join(rel)) else {
                continue;
            };
            for (i, l) in t.split('\n').enumerate() {
                if !tok.is_empty() && l.contains(&tok) {
                    items.push(RawItem {
                        path: rel.clone(),
                        span: Span {
                            start_line: i as u64 + 1,
                            end_line: i as u64 + 1,
                        },
                        digest: sha256_plain(l.as_bytes()),
                        tier: Tier::Structural,
                        language: "text".into(),
                        rank: 1.0,
                        text: Some(l.to_string()),
                        span_kind: None,
                        edges: vec![],
                    });
                }
            }
        }
        ExternalResponse {
            status: "complete".into(),
            no_references: items.is_empty(),
            items,
            coverage: complete(),
            upstream_version: None,
            provider_id: id.into(),
            diagnostics: vec![],
        }
    })
}

fn fake_world_project() -> PathBuf {
    let root = fixture_dir(PREFIX).canonicalize().unwrap();
    copy_tree(&fixtures().join("mixed"), &root.join("project"));
    root.join("project")
}

fn fake_broker(src: FakeSource, cache_dir: &Path) -> Broker {
    let cache = ResultCache::new(
        cache_dir.to_path_buf(),
        CacheConfig::default(),
        system_clock(),
    );
    let mut b = Broker::new(Some(native()), Some(cache));
    b.add_provider(Box::new(src)).unwrap();
    b
}

// ---- acceptance: mixed language ---------------------------------------------

#[test]
fn hp_hp05_mixed_language_request_is_one_bounded_native_first_document() {
    let w = world();
    install_source_index(&w);
    let b = broker_with_cache(&w, true);
    let mut r = req("calculator.add add", 12_000);
    r.max_items = 40;
    let o = b.context(&w.project, &r).expect("context");
    assert!(o.rendered.len() <= 12_000, "{} bytes", o.rendered.len());
    assert_eq!(o.native.len(), 1, "one mandatory fact set, no duplicates");
    let n = &o.native[0];
    assert_eq!(n.stable_id.as_deref(), Some("calculator.add"));
    assert_eq!(n.provenance, Tier::CompilerVerified);
    assert!(n.authorizes_edits());
    let langs: Vec<&str> = o.external.iter().map(|i| i.language.as_str()).collect();
    assert!(
        langs.contains(&"rust") && langs.contains(&"typescript"),
        "{langs:?}"
    );
    for i in &o.external {
        assert_ne!(i.provenance, Tier::CompilerVerified);
        assert!(!i.authorizes_edits());
        assert_ne!(
            i.stable_id.as_deref(),
            Some("calculator.add"),
            "mandatory fact duplicated: {}:{}",
            i.path,
            i.span.start_line
        );
    }
    let d = doc(&o);
    assert_eq!(d["budget"]["unit"], "byte-v1");
    assert_eq!(d["native"].as_array().unwrap().len(), 1);
}

#[test]
fn hp_hp05_standalone_compiler_output_is_embedded_unchanged() {
    let Some(c) = real_compiler() else {
        eprintln!("skipped: SEMAPRAX_COMPILER not set");
        return;
    };
    let w = world();
    install_source_index(&w);
    let b = broker_with_cache(&w, false);
    let o = b
        .context(&w.project, &req("calculator.add", 12_000))
        .unwrap();
    let out = std::process::Command::new(&c)
        .args(["context"])
        .arg(&w.project)
        .args(["calculator.add", "--depth", "1", "--max-bytes", "12000"])
        .env_clear()
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let standalone = String::from_utf8(out.stdout).unwrap();
    assert_eq!(
        o.native[0].text.as_deref(),
        Some(standalone.trim_end_matches('\n'))
    );
    // Budget meaning is the compiler's: the brokered max_bytes echoes in the embedded bytes.
    let v: Value =
        serde_json::from_str(o.native[0].text.as_deref().unwrap()).unwrap_or(Value::Null);
    assert!(
        v["schema"].as_str().unwrap_or("").contains("context"),
        "{v}"
    );
}

// ---- acceptance: provider switch --------------------------------------------

#[test]
fn hp_hp05_switching_providers_changes_provenance_not_compiler_facts() {
    let (a, b) = (world(), world());
    install_source_index(&a);
    install_graph_index(&b);
    let r = req("calculator.add add", 12_000);
    let oa = broker_with_cache(&a, false)
        .context(&a.project, &r)
        .unwrap();
    let ob = broker_with_cache(&b, false)
        .context(&b.project, &r)
        .unwrap();
    assert_eq!(oa.native.len(), 1);
    let facts = |o: &semaprax_harness::context::BrokerOutput| {
        o.native
            .iter()
            .map(|n| {
                (
                    n.stable_id.clone(),
                    n.text.clone(),
                    n.provenance,
                    n.digest.clone(),
                    n.path.clone(),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        facts(&oa),
        facts(&ob),
        "compiler-derived facts must not depend on the provider"
    );
    let prov = |o: &semaprax_harness::context::BrokerOutput| {
        o.external
            .iter()
            .map(|i| (i.provider_id.clone(), i.provenance))
            .collect::<std::collections::BTreeSet<_>>()
    };
    assert_ne!(prov(&oa), prov(&ob));
    assert!(
        ob.external.iter().any(|i| i.provenance == Tier::Inferred),
        "graph-index reports markdown as inferred"
    );
    // The graph-index fixture falsely claims compiler-verified for `.spx` lines: refused.
    assert!(ob
        .external
        .iter()
        .all(|i| i.provenance != Tier::CompilerVerified));
    let d = doc(&ob);
    let diags = d["providers"][0]["diagnostics"].to_string();
    assert!(diags.contains("SPX-HPE050"), "{diags}");
}

// ---- acceptance: cache invalidation -----------------------------------------

fn counted(w: &Path) -> (Broker, Arc<AtomicUsize>, Arc<AtomicUsize>, Arc<AtomicBool>) {
    let s = FakeSource::new("org.example/fake", grep_answer("org.example/fake"));
    let (q, r, a) = (s.queries.clone(), s.rechecks.clone(), s.allowed.clone());
    (fake_broker(s, &w.parent().unwrap().join("cache")), q, r, a)
}

#[test]
fn hp_hp05_cache_hits_recheck_authority_every_read() {
    let p = fake_world_project();
    let (b, q, r, _) = counted(&p);
    let r1 = req("add", 12_000);
    let o1 = b.context(&p, &r1).unwrap();
    let o2 = b.context(&p, &r1).unwrap();
    assert_eq!(q.load(Ordering::SeqCst), 1, "second read is a cache hit");
    assert_eq!(
        r.load(Ordering::SeqCst),
        2,
        "authority rechecked on the hit too"
    );
    assert!(!o1.cache_hits["org.example/fake"]);
    assert!(o2.cache_hits["org.example/fake"]);
    assert_eq!(
        o1.rendered, o2.rendered,
        "cached and fresh output are byte-identical"
    );
}

fn misses_after(mutate: impl FnOnce(&Path)) {
    let p = fake_world_project();
    let (b, q, _, _) = counted(&p);
    let r = req("add", 12_000);
    b.context(&p, &r).unwrap();
    mutate(&p);
    let o = b.context(&p, &r).unwrap();
    assert_eq!(
        q.load(Ordering::SeqCst),
        2,
        "a changed tree must not hit the cache"
    );
    assert!(!o.cache_hits["org.example/fake"]);
}

#[test]
fn hp_hp05_dirty_edit_invalidates_the_cache() {
    // Same length, same mtime granularity concerns: only content digests can tell.
    misses_after(|p| {
        let f = p.join("src/host.rs");
        let t = std::fs::read_to_string(&f)
            .unwrap()
            .replace("left + right", "left - right");
        std::fs::write(f, t).unwrap();
    });
}

#[test]
fn hp_hp05_rename_and_delete_invalidate_the_cache() {
    misses_after(|p| std::fs::rename(p.join("web/app.ts"), p.join("web/renamed.ts")).unwrap());
    misses_after(|p| std::fs::remove_file(p.join("docs/notes.md")).unwrap());
}

#[test]
fn hp_hp05_provider_version_change_invalidates_the_cache() {
    let p = fake_world_project();
    let cache = p.parent().unwrap().join("cache");
    let r = req("add", 12_000);
    let mut s1 = FakeSource::new("org.example/fake", grep_answer("org.example/fake"));
    let q = s1.queries.clone();
    s1.version = "1.0.0".into();
    fake_broker(s1, &cache).context(&p, &r).unwrap();
    let mut s2 = FakeSource::new("org.example/fake", grep_answer("org.example/fake"));
    s2.version = "2.0.0".into();
    s2.queries = q.clone();
    let o = fake_broker(s2, &cache).context(&p, &r).unwrap();
    assert_eq!(q.load(Ordering::SeqCst), 2);
    assert!(!o.cache_hits["org.example/fake"]);
}

#[test]
fn hp_hp05_same_head_different_worktrees_do_not_share_cache() {
    // Two worktrees of one repository: same common git dir and identical content.
    let root = fixture_dir(PREFIX).canonicalize().unwrap();
    std::fs::create_dir_all(root.join("repo/.git/worktrees/w1")).unwrap();
    std::fs::create_dir_all(root.join("repo/.git/worktrees/w2")).unwrap();
    for w in ["w1", "w2"] {
        write(
            &root,
            &format!("repo/.git/worktrees/{w}/commondir"),
            "../..\n",
        );
        copy_tree(&fixtures().join("mixed"), &root.join(w));
        write(
            &root,
            &format!("{w}/.git"),
            &format!(
                "gitdir: {}\n",
                root.join(format!("repo/.git/worktrees/{w}")).display()
            ),
        );
    }
    let s1 = Snapshot::capture(&root.join("w1")).unwrap();
    let s2 = Snapshot::capture(&root.join("w2")).unwrap();
    assert_eq!(s1.project_id, s2.project_id);
    assert_ne!(s1.worktree_id, s2.worktree_id);
    let cache = root.join("cache");
    let s = FakeSource::new("org.example/fake", grep_answer("org.example/fake"));
    let q = s.queries.clone();
    let b = fake_broker(s, &cache);
    let r = req("add", 12_000);
    b.context(&root.join("w1"), &r).unwrap();
    let o = b.context(&root.join("w2"), &r).unwrap();
    assert_eq!(q.load(Ordering::SeqCst), 2);
    assert!(!o.cache_hits["org.example/fake"]);
    assert_eq!(
        b.cache().unwrap().len(),
        2,
        "per-worktree entries, never a shared index"
    );
}

#[test]
fn hp_hp05_revocation_purges_and_blocks_cached_results() {
    let p = fake_world_project();
    let (b, q, _, allowed) = counted(&p);
    let r = req("add", 12_000);
    b.context(&p, &r).unwrap();
    assert_eq!(b.cache().unwrap().len(), 1);
    allowed.store(false, Ordering::SeqCst);
    let o = b.context(&p, &r).unwrap();
    assert!(
        o.external.is_empty(),
        "a revoked provider's cached items are never served"
    );
    assert_eq!(q.load(Ordering::SeqCst), 1);
    assert_eq!(
        b.cache().unwrap().len(),
        0,
        "revocation purges the provider's entries"
    );
    let d = doc(&o);
    assert_eq!(d["providers"][0]["status"], "unavailable");
    assert!(d["providers"][0]["diagnostics"]
        .to_string()
        .contains("SPX-HPB034"));
}

#[test]
fn hp_hp05_real_revocation_through_the_trust_store() {
    let w = world();
    install_source_index(&w);
    let b = broker_with_cache(&w, true);
    let r = req("add", 12_000);
    let o = b.context(&w.project, &r).unwrap();
    assert!(!o.external.is_empty());
    assert_eq!(b.cache().unwrap().len(), 1);
    assert_eq!(sh(&w, &["revoke", SI_ID]).code, 0);
    let o = b.context(&w.project, &r).unwrap();
    assert!(o.external.is_empty());
    assert_eq!(b.cache().unwrap().len(), 0);
    assert!(doc(&o)["providers"][0]["diagnostics"]
        .to_string()
        .contains("SPX-HPB034"));
}

#[test]
fn hp_hp05_cache_bounds_ttl_purge_and_location() {
    let root = fixture_dir(PREFIX);
    let now = Arc::new(Mutex::new(100u64));
    let clock = {
        let n = now.clone();
        Box::new(move || *n.lock().unwrap())
    };
    let c = ResultCache::new(
        root.join("c"),
        CacheConfig {
            max_entries: 2,
            max_bytes: 1 << 20,
            ttl_secs: 50,
        },
        clock,
    );
    let snap = Snapshot::capture(&fake_world_project()).unwrap();
    let ident = FakeSource::new("p/a", grep_answer("p/a")).identity();
    let resp = (grep_answer("p/a"))(
        &snap,
        &ExternalQuery {
            op: "search".into(),
            payload: json!({"query": "add"}),
        },
    );
    let key = |q: &str| {
        semaprax_harness::context::CacheKey::new(
            &snap,
            &ident,
            &ExternalQuery {
                op: "search".into(),
                payload: json!({"query": q}),
            },
        )
    };
    for (t, q) in [(100u64, "a"), (101, "b"), (102, "c")] {
        *now.lock().unwrap() = t;
        c.put(&key(q), &resp);
    }
    assert_eq!(c.len(), 2, "oldest evicted at the entry bound");
    assert!(c.get(&key("a")).is_none() && c.get(&key("c")).is_some());
    *now.lock().unwrap() = 200;
    assert!(c.get(&key("c")).is_none(), "expired entry is a miss");
    *now.lock().unwrap() = 201;
    c.put(&key("d"), &resp);
    assert_eq!(c.purge_provider("p/a"), 1);
    c.put(&key("e"), &resp);
    assert_eq!(c.purge_all(), 1);
    assert!(c.is_empty());
}

#[test]
fn hp_hp05_cache_lives_under_host_home_never_in_the_project() {
    if real_compiler().is_none() {
        eprintln!("skipped: the CLI path needs SEMAPRAX_COMPILER");
        return;
    }
    let w = world();
    install_source_index(&w);
    let before = Snapshot::capture(&w.project).unwrap();
    let out = sh(
        &w,
        &["context", w.project.to_str().unwrap(), "add", "--json"],
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert_eq!(
        Snapshot::capture(&w.project).unwrap().files,
        before.files,
        "the project tree is not written"
    );
    let cached: Vec<_> = std::fs::read_dir(w.home.join("cache/context"))
        .unwrap()
        .flatten()
        .collect();
    assert_eq!(cached.len(), 1);
    assert!(sh(
        &w,
        &["context", w.project.to_str().unwrap(), "--purge-cache"]
    )
    .stdout
    .contains("purged 1"));
}

// ---- acceptance: exhaustive vs ranked ---------------------------------------

#[test]
fn hp_hp05_incomplete_index_never_claims_no_callers() {
    // Clean tree: an exhaustive provider may say there are no references.
    let w = world();
    std::fs::remove_file(w.project.join("docs/diagram.xyzlang")).unwrap();
    install_graph_index(&w);
    let mut r = req("zzz_nothing", 12_000);
    r.references = true;
    r.symbol = Some("zzz_nothing".into());
    let d = doc(&broker_with_cache(&w, false)
        .context(&w.project, &r)
        .unwrap());
    assert_eq!(d["references"]["exhaustive"], true, "{d}");
    assert_eq!(d["references"]["definitive_absence"], true);

    // Same query, but the index could not read two files: ranked, not exhaustive.
    let w = world();
    std::fs::write(w.project.join("src/bad.py"), [0xff, 0xfe, 0x00]).unwrap();
    install_graph_index(&w);
    let o = broker_with_cache(&w, false)
        .context(&w.project, &r)
        .unwrap();
    let d = doc(&o);
    assert_eq!(d["references"]["exhaustive"], false);
    assert_eq!(d["references"]["definitive_absence"], false);
    assert!(d["references"]["recommendation"]
        .as_str()
        .unwrap()
        .contains("source search"));
    assert!(!o.definitive_absence && !o.exhaustive);
    let skipped = d["providers"][0]["coverage"]["skipped"].to_string();
    assert!(
        skipped.contains("src/bad.py") && skipped.contains("diagram.xyzlang"),
        "unsupported files stay visible: {skipped}"
    );
    assert_eq!(d["coverage"]["complete"], false);
}

#[test]
fn hp_hp05_budget_truncation_downgrades_references_to_non_exhaustive() {
    let p = fake_world_project();
    let (b, ..) = counted(&p);
    let mut r = req("add", 12_000);
    r.references = true;
    r.symbol = Some("add".into());
    r.native_targets = Some(vec![]);
    let roomy = b.context(&p, &r).unwrap();
    assert!(roomy.exhaustive && roomy.omitted == 0);
    r.max_bytes = 1500;
    let tight = b.context(&p, &r).unwrap();
    assert!(tight.omitted > 0 && !tight.exhaustive);
    let d = doc(&tight);
    assert_eq!(d["references"]["exhaustive"], false);
    assert_eq!(d["references"]["definitive_absence"], false);
}

// ---- acceptance: budget -----------------------------------------------------

#[test]
fn hp_hp05_all_output_fits_every_budget_and_handles_preserve_omissions() {
    let p = fake_world_project();
    let (b, ..) = counted(&p);
    let mut saw_handles = false;
    for max in (2048..9000).step_by(250) {
        let r = req("add", max);
        match b.context(&p, &r) {
            Ok(o) => {
                assert!(o.rendered.len() <= max, "{} > {max}", o.rendered.len());
                let d = doc(&o);
                let handles = d["omitted"]["handles"].as_array().unwrap().len() as u64;
                assert_eq!(
                    d["omitted"]["count"].as_u64().unwrap(),
                    handles + d["omitted"]["handles_dropped"].as_u64().unwrap()
                );
                saw_handles |= handles > 0;
                for i in d["external"].as_array().unwrap() {
                    assert!(
                        i["text"].is_string() || i["text"].is_null(),
                        "items are whole or absent"
                    );
                }
            }
            Err(e) => assert_eq!(e.code, "SPX-HPE001", "{e}"),
        }
    }
    assert!(
        saw_handles,
        "some budget must omit items yet keep retrieval handles"
    );
}

#[test]
fn hp_hp05_mandatory_native_overflow_is_an_explicit_refusal() {
    let p = fake_world_project();
    let s = FakeSource::new("org.example/fake", grep_answer("org.example/fake"));
    let cache = p.parent().unwrap().join("c2");
    let mut b = Broker::new(
        Some(Box::new(FakeNative { pad: 3000 })),
        Some(ResultCache::new(
            cache,
            CacheConfig::default(),
            system_clock(),
        )),
    );
    b.add_provider(Box::new(s)).unwrap();
    let e = b.context(&p, &req("calculator.add", 2048)).unwrap_err();
    assert_eq!(e.code, "SPX-HPE001");
    assert!(e.message.contains("mandatory"));
    let e = b.context(&p, &req("calculator.add", 1000)).unwrap_err();
    assert_eq!(e.code, "SPX-HPE001", "below the compiler's own floor");
    assert!(b.context(&p, &req("calculator.add", 20_000)).is_ok());
}

// ---- acceptance: external .spx references -----------------------------------

fn spx_item(snap: &Snapshot, rel: &str, line: u64, digest: Option<&str>) -> RawItem {
    let t = std::fs::read_to_string(snap.root.join(rel)).unwrap();
    RawItem {
        path: rel.into(),
        span: Span {
            start_line: line,
            end_line: line,
        },
        digest: digest
            .map(str::to_string)
            .unwrap_or_else(|| span_digest(&t, line, line).unwrap()),
        tier: Tier::Structural,
        language: "semaprax".into(),
        rank: 3.5,
        text: None,
        span_kind: None,
        edges: vec![],
    }
}

#[test]
fn hp_hp05_external_spx_references_resolve_or_stay_hints() {
    let p = fake_world_project();
    let answer: Answer = Box::new(|snap, _| {
        let items = vec![
            spx_item(snap, "src/core.spx", 4, None), // `fn add`: resolvable
            spx_item(snap, "src/core.spx", 1, None), // module line: no declaration
            spx_item(
                snap,
                "src/core.spx",
                10,
                Some(&format!("sha256:{}", "0".repeat(64))),
            ), // stale digest
        ];
        ExternalResponse {
            status: "complete".into(),
            items,
            coverage: complete(),
            no_references: false,
            upstream_version: None,
            provider_id: "org.example/fake".into(),
            diagnostics: vec![],
        }
    });
    let b = fake_broker(
        FakeSource::new("org.example/fake", answer),
        &p.parent().unwrap().join("c"),
    );
    let mut r = req("zzz", 12_000);
    r.native_targets = Some(vec![]);
    let o = b.context(&p, &r).unwrap();
    assert!(o.native.is_empty());
    let by_line = |l: u64| o.external.iter().find(|i| i.span.start_line == l).unwrap();
    assert_eq!(by_line(4).link, Link::Resolved("calculator.add".into()));
    assert!(matches!(by_line(1).link, Link::Unmappable(_)));
    assert!(matches!(by_line(10).link, Link::Stale(_)));
    for l in [4, 1, 10] {
        assert!(
            !by_line(l).authorizes_edits(),
            "external items are hints even when resolved"
        );
    }
    assert_eq!(by_line(10).provenance, Tier::Inferred);
    assert!(!by_line(10).verified);

    // When the compiler item is already present, the resolved reference is not repeated.
    r.native_targets = Some(vec!["calculator.add".into()]);
    let o = b.context(&p, &r).unwrap();
    assert_eq!(o.native.len(), 1);
    assert!(o.external.iter().all(|i| i.span.start_line != 4));
}

#[test]
fn hp_hp05_stale_index_after_a_dirty_edit_degrades_items() {
    let p = fake_world_project();
    let digests: Arc<Mutex<Option<ExternalResponse>>> = Arc::default();
    let d2 = digests.clone();
    let answer: Answer = Box::new(move |snap, q| {
        let mut slot = d2.lock().unwrap();
        slot.get_or_insert_with(|| (grep_answer("org.example/fake"))(snap, q))
            .clone()
    });
    let b = fake_broker(
        FakeSource::new("org.example/fake", answer),
        &p.parent().unwrap().join("c"),
    );
    let r = req("add", 12_000);
    let before = b.context(&p, &r).unwrap();
    assert!(before.external.iter().all(|i| i.verified));
    // The tree changes but the (stale) provider keeps answering with old digests.
    let f = p.join("src/host.rs");
    std::fs::write(
        &f,
        std::fs::read_to_string(&f)
            .unwrap()
            .replace("through", "via"),
    )
    .unwrap();
    let after = b.context(&p, &r).unwrap();
    let stale: Vec<_> = after.external.iter().filter(|i| !i.verified).collect();
    assert!(!stale.is_empty());
    assert!(stale
        .iter()
        .all(|i| i.provenance == Tier::Inferred
            && i.omission_reason.as_deref() == Some("stale-digest")));
}

// ---- selection and federation -----------------------------------------------

#[test]
fn hp_hp05_one_provider_per_scope_and_explicit_disjoint_federation() {
    let mk = |id: &str, scope: &[&str]| {
        let mut s = FakeSource::new(id, grep_answer("x"));
        s.scope = scope.iter().map(|s| s.to_string()).collect();
        Box::new(s) as Box<dyn ExternalSource>
    };
    let mut b = Broker::new(None, None);
    b.add_provider(mk("p/a", &["src"])).unwrap();
    assert_eq!(
        b.add_provider(mk("p/b", &["web"])).unwrap_err().code,
        "SPX-HPE040"
    );
    b.enable_federation();
    assert_eq!(
        b.add_provider(mk("p/b", &["src/sub"])).unwrap_err().code,
        "SPX-HPE041"
    );
    assert_eq!(
        b.add_provider(mk("p/c", &[])).unwrap_err().code,
        "SPX-HPE041"
    );
    b.add_provider(mk("p/b", &["web"])).unwrap();
}

#[test]
fn hp_hp05_federation_keeps_per_provider_rank_and_budget() {
    let p = fake_world_project();
    let mut b = Broker::new(
        None,
        Some(ResultCache::new(
            p.parent().unwrap().join("c"),
            CacheConfig::default(),
            system_clock(),
        )),
    );
    b.enable_federation();
    for (id, scope, rank) in [("p/src", "src", 0.9), ("p/web", "web", 1000.0)] {
        let mut s = FakeSource::new(
            id,
            Box::new(move |snap, q| {
                let mut r = (grep_answer("x"))(snap, q);
                for i in &mut r.items {
                    i.rank = rank;
                }
                r.provider_id = id.into();
                r
            }),
        );
        s.scope = vec![scope.into()];
        b.add_provider(Box::new(s)).unwrap();
    }
    let mut r = req("add", 12_000);
    r.native_targets = Some(vec![]);
    let o = b.context(&p, &r).unwrap();
    let ranks: BTreeMap<String, Vec<f64>> = o.external.iter().fold(BTreeMap::new(), |mut m, i| {
        m.entry(i.provider_id.clone())
            .or_default()
            .push(i.provider_rank.unwrap());
        m
    });
    assert_eq!(ranks["p/src"][0], 0.9);
    assert_eq!(
        ranks["p/web"][0], 1000.0,
        "raw scores are preserved, never normalized or compared"
    );
    assert!(o.external.iter().all(|i| if i.provider_id == "p/src" {
        i.path.starts_with("src/")
    } else {
        i.path.starts_with("web/")
    }));
    let d = doc(&o);
    assert_eq!(d["providers"].as_array().unwrap().len(), 2);
    assert_eq!(d["providers"][0]["rank_scale"], "provider-local");
}

// ---- CLI --------------------------------------------------------------------

#[test]
fn hp_hp05_cli_context_json_human_and_usage() {
    if real_compiler().is_none() {
        eprintln!("skipped: the CLI path needs SEMAPRAX_COMPILER");
        return;
    }
    let w = world();
    install_source_index(&w);
    let p = w.project.to_str().unwrap();
    let o = sh(
        &w,
        &[
            "context",
            p,
            "calculator.add add",
            "--max-bytes",
            "9000",
            "--json",
        ],
    );
    assert_eq!(o.code, 0, "{}", o.stderr);
    let v: Value = serde_json::from_str(&o.stdout).unwrap();
    assert_eq!(v["schema"], "semaprax.harness-context.v1");
    assert!(o.stdout.trim_end().len() <= 9000);
    let h = sh(
        &w,
        &["context", p, "calculator.add add", "--max-bytes", "9000"],
    );
    assert_eq!(h.code, 0);
    assert!(
        h.stdout.contains("native semaprax.compiler") && h.stdout.contains("external"),
        "{}",
        h.stdout
    );
    assert_eq!(sh(&w, &["context"]).code, 2);
    assert_eq!(sh(&w, &["context", p, "x", "--bogus"]).code, 2);
    let refused = sh(&w, &["context", p, "calculator.add", "--max-bytes", "100"]);
    assert_eq!(refused.code, 1);
    assert!(refused.stderr.contains("SPX-HPE001"), "{}", refused.stderr);
    let refs = sh(
        &w,
        &[
            "context",
            p,
            "x",
            "--references",
            "--symbol",
            "add",
            "--json",
        ],
    );
    assert_eq!(refs.code, 0, "{}", refs.stderr);
    assert!(
        serde_json::from_str::<Value>(&refs.stdout).unwrap()["references"]["definitive_absence"]
            == false
    );
}

// ---- provisioned real tool ---------------------------------------------------

#[test]
#[ignore = "provisioned: needs HARNESS_GRAPHIFY and HARNESS_PYTHON"]
fn hp_hp05_real_graphify_through_the_broker() {
    let graphify = crate::support::required_tool("HARNESS_GRAPHIFY");
    let python = crate::support::required_tool("HARNESS_PYTHON");
    let mut w = world();
    w.env
        .vars
        .insert("HARNESS_PYTHON".into(), python.display().to_string());
    let desc = crate::support::repo_root()
        .join("packages/semaprax-harness-adapters/graphify/harness-provider.json");
    let o = sh(
        &w,
        &[
            "adopt",
            desc.to_str().unwrap(),
            "--upstream",
            graphify.to_str().unwrap(),
        ],
    );
    assert_eq!(o.code, 0, "adopt: {}{}", o.stdout, o.stderr);
    assert_eq!(
        sh(&w, &["trust", "com.graphify-labs/graphify-context"]).code,
        0
    );
    write(
        &w.project,
        "semaprax.harness.toml",
        "schema = \"semaprax.harness-config.v1\"\n\n[capability.\"context.repository\"]\nmode = \"required\"\nprovider = \"com.graphify-labs/graphify-context\"\n",
    );
    let b = broker_with_cache(&w, true);
    let o = b
        .context(&w.project, &req("calculator.add add", 16_000))
        .unwrap();
    let d = doc(&o);
    eprintln!("{}", serde_json::to_string_pretty(&d["providers"]).unwrap());
    assert_eq!(
        o.native.len(),
        1,
        "native facts come from the compiler even with Graphify selected"
    );
    assert_eq!(
        d["providers"][0]["status"]
            .as_str()
            .map(|s| s != "unavailable"),
        Some(true),
        "{d}"
    );
    assert!(!o.external.is_empty(), "graphify returned items");
    assert!(o
        .external
        .iter()
        .all(|i| i.provenance != Tier::CompilerVerified));
}

#[path = "context_plan.rs"]
mod hn13;

#[path = "context_binding.rs"]
mod mn05;

// ---- HN-10 index adoption and worktree-safe refresh (fixture prefix `hp-hnf`) ----------

mod index_adoption_tests {
    use super::*;
    use semaprax_harness::context::index_adoption::{
        copy_snapshot, first_diagnostic, verify, Expected, GenerationStore, IndexDescriptor,
        Outcome, Ownership,
    };
    use std::collections::BTreeSet;
    use std::time::Duration;

    fn dg(s: &str) -> String {
        sha256_plain(s.as_bytes())
    }

    fn expected() -> Expected {
        Expected {
            provider: "org.example/idx".into(),
            upstream_version: "1.0.0".into(),
            index_schema: "wiring.v1".into(),
            source_root: "/work/a".into(),
            worktree_id: "wt-a".into(),
            config_digest: dg("cfg"),
            tree: BTreeMap::from([
                ("a.rs".into(), dg("fn a(){}")),
                ("b.py".into(), dg("def b(): pass")),
            ]),
            excluded: BTreeSet::from(["secret/k.py".to_string()]),
        }
    }

    fn descriptor(e: &Expected) -> IndexDescriptor {
        IndexDescriptor {
            provider: e.provider.clone(),
            upstream_version: e.upstream_version.clone(),
            index_schema: e.index_schema.clone(),
            source_root: e.source_root.clone(),
            worktree_id: e.worktree_id.clone(),
            config_digest: e.config_digest.clone(),
            inputs: e.tree.clone(),
            coverage_languages: vec!["python".into(), "rust".into()],
            ownership: Ownership::ReadOnly,
        }
    }

    fn codes(d: &IndexDescriptor, e: &Expected) -> Vec<&'static str> {
        verify(d, e).iter().map(|m| m.code).collect()
    }

    #[test]
    fn hn10_matching_descriptor_verifies_and_serializes_deterministically() {
        let e = expected();
        let d = descriptor(&e);
        assert!(verify(&d, &e).is_empty());
        let j = d.to_json();
        assert_eq!(j["schema"], "semaprax.harness-index-adoption.v1");
        assert_eq!(j["ownership"], "read-only");
        assert_eq!(j["coverage"]["files"], 2);
        assert_eq!(j, descriptor(&e).to_json());
        assert_eq!(Outcome::ReusedUserIndex.as_str(), "reused-user-index");
        assert_eq!(
            Outcome::CopiedValidatedIndex.as_str(),
            "copied-validated-index"
        );
        assert_eq!(Outcome::IncrementalRefresh.as_str(), "incremental-refresh");
        assert_eq!(Outcome::Rebuilt.as_str(), "rebuilt");
        assert_eq!(Outcome::Incompatible.as_str(), "incompatible");
        assert_eq!(
            Ownership::parse("copied-snapshot"),
            Some(Ownership::CopiedSnapshot)
        );
        assert_eq!(Ownership::parse("write-through"), None);
    }

    #[test]
    fn hn10_each_mismatch_class_has_its_own_stable_code() {
        let e = expected();
        let mut d = descriptor(&e);
        d.upstream_version = "2.0.0".into();
        assert_eq!(codes(&d, &e), ["SPX-HPF001"]);
        let mut d = descriptor(&e);
        d.worktree_id = "wt-b".into();
        d.source_root = "/work/b".into();
        assert_eq!(codes(&d, &e), ["SPX-HPF002"], "another worktree");
        let mut d = descriptor(&e);
        d.config_digest = dg("other parser");
        assert_eq!(codes(&d, &e), ["SPX-HPF003"]);
        let mut d = descriptor(&e);
        d.inputs.insert("secret/k.py".into(), dg("key"));
        assert_eq!(codes(&d, &e), ["SPX-HPF004"], "private excluded path");
        // Same-size edit: the digest differs although length and Git revision match.
        let mut e2 = e.clone();
        e2.tree.insert("b.py".into(), dg("def c(): pass"));
        assert_eq!("def b(): pass".len(), "def c(): pass".len());
        assert_eq!(codes(&descriptor(&e), &e2), ["SPX-HPF005"]);
        let mut e3 = e.clone();
        e3.tree.insert("new.rs".into(), dg("fn n(){}"));
        let v = verify(&descriptor(&e), &e3);
        assert_eq!(v.len(), 1);
        assert_eq!(first_diagnostic(&v).unwrap().code, "SPX-HPF006");
        let mut e4 = e.clone();
        e4.tree.remove("a.rs");
        assert_eq!(
            codes(&descriptor(&e), &e4),
            ["SPX-HPF005"],
            "indexed file gone"
        );
    }

    #[test]
    fn hn10_generation_swap_is_atomic_for_concurrent_readers() {
        let root = fixture_dir("hp-hnf-gen").canonicalize().unwrap();
        let store = GenerationStore::new(root.join("owned"));
        assert!(store.current().is_none());
        let put = |n: u64| {
            let _lock = store.lock(Duration::from_secs(5)).unwrap();
            let (g, dir) = store.begin().unwrap();
            assert_eq!(g, n);
            // A large file written in pieces, then the marker: a torn read would disagree with the name.
            let body = format!("generation-{n}\n").repeat(20_000);
            std::fs::write(dir.join("graph.json"), &body).unwrap();
            store.publish(g).unwrap();
        };
        put(1);
        let stop = Arc::new(AtomicBool::new(false));
        let bad = Arc::new(AtomicUsize::new(0));
        let reads = Arc::new(AtomicUsize::new(0));
        let readers: Vec<_> = (0..3)
            .map(|_| {
                let (s, stop, bad, reads) = (
                    GenerationStore::new(root.join("owned")),
                    stop.clone(),
                    bad.clone(),
                    reads.clone(),
                );
                std::thread::spawn(move || {
                    while !stop.load(Ordering::Relaxed) {
                        match s.current().and_then(|(n, d)| {
                            Some((n, std::fs::read_to_string(d.join("graph.json")).ok()?))
                        }) {
                            Some((n, body))
                                if body.lines().all(|l| l == format!("generation-{n}"))
                                    && body.lines().count() == 20_000 =>
                            {
                                reads.fetch_add(1, Ordering::Relaxed);
                            }
                            _ => {
                                bad.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }
                })
            })
            .collect();
        for n in 2..=25 {
            // Interleave readers with every swap (bounded wait) so the read
            // count does not depend on runner speed.
            let before = reads.load(Ordering::Relaxed);
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
            while reads.load(Ordering::Relaxed) == before && std::time::Instant::now() < deadline {
                std::thread::yield_now();
            }
            put(n);
        }
        stop.store(true, Ordering::Relaxed);
        for r in readers {
            r.join().unwrap();
        }
        assert_eq!(
            bad.load(Ordering::Relaxed),
            0,
            "a reader saw a torn or missing generation"
        );
        assert!(reads.load(Ordering::Relaxed) > 10);
        let live: Vec<_> = std::fs::read_dir(root.join("owned/gen"))
            .unwrap()
            .flatten()
            .collect();
        assert!(
            live.len() <= 2,
            "old generations pruned, previous kept: {}",
            live.len()
        );
        assert!(!live
            .iter()
            .any(|e| e.file_name().to_string_lossy().ends_with(".partial")));
    }

    #[test]
    fn hn10_unpublished_partial_build_is_invisible_and_lock_is_single_flight() {
        let root = fixture_dir("hp-hnf-lock").canonicalize().unwrap();
        let store = GenerationStore::new(root.join("owned"));
        let first = store.lock(Duration::from_secs(1)).unwrap();
        let err = store
            .lock(Duration::from_millis(100))
            .err()
            .expect("second holder must wait");
        assert_eq!(err.code, "SPX-HPF010");
        let (g, dir) = store.begin().unwrap();
        std::fs::write(dir.join("graph.json"), "half").unwrap();
        assert!(
            store.current().is_none(),
            "a build in progress is never visible"
        );
        drop(first);
        assert!(
            store.lock(Duration::from_millis(100)).is_ok(),
            "released on drop"
        );
        // A crashed holder (dead pid) does not wedge refresh.
        let lockdir = root.join("owned/refresh.lock");
        std::fs::create_dir_all(&lockdir).unwrap();
        std::fs::write(lockdir.join("owner"), "2000000000").unwrap();
        assert!(store.lock(Duration::from_millis(200)).is_ok());
        store.publish(g).unwrap();
        assert_eq!(store.current().unwrap().0, g);
    }

    #[test]
    fn hn10_copied_snapshot_is_read_only_idempotent_and_never_touches_the_source() {
        let root = fixture_dir("hp-hnf-snap").canonicalize().unwrap();
        let src = root.join("user-index");
        std::fs::create_dir_all(src.join(".graph")).unwrap();
        std::fs::write(src.join(".graph/wiring.json"), "{\"meta\":{}}").unwrap();
        let before = std::fs::read(src.join(".graph/wiring.json")).unwrap();
        let (dir, bytes) = copy_snapshot(&src, &root.join("adopted"), &dg("digest-1")).unwrap();
        assert!(bytes > 0);
        assert!(std::fs::metadata(dir.join(".graph/wiring.json"))
            .unwrap()
            .permissions()
            .readonly());
        let (again, bytes2) = copy_snapshot(&src, &root.join("adopted"), &dg("digest-1")).unwrap();
        assert_eq!(
            (again, bytes2),
            (dir, 0),
            "same digest reuses the immutable copy"
        );
        assert_eq!(
            std::fs::read(src.join(".graph/wiring.json")).unwrap(),
            before
        );
        assert!(
            !std::fs::metadata(src.join(".graph/wiring.json"))
                .unwrap()
                .permissions()
                .readonly(),
            "source permissions untouched"
        );
    }

    #[test]
    fn hn10_result_cache_concurrent_puts_never_cross_entries() {
        let root = fixture_dir("hp-hnf-cache").canonicalize().unwrap();
        let c = Arc::new(ResultCache::new(
            root.join("c"),
            CacheConfig {
                max_entries: 1000,
                max_bytes: 1 << 26,
                ttl_secs: 3600,
            },
            system_clock(),
        ));
        let snap = Snapshot::capture(&fake_world_project()).unwrap();
        let ident = FakeSource::new("p/a", grep_answer("p/a")).identity();
        let mk = |q: String| {
            semaprax_harness::context::CacheKey::new(
                &snap,
                &ident,
                &ExternalQuery {
                    op: "search".into(),
                    payload: json!({"query": q}),
                },
            )
        };
        let resp = |q: &str| {
            (grep_answer("p/a"))(
                &snap,
                &ExternalQuery {
                    op: "search".into(),
                    payload: json!({"query": q}),
                },
            )
        };
        let hs: Vec<_> = (0..4)
            .map(|t| {
                let (c, keys): (_, Vec<_>) = (
                    c.clone(),
                    (0..30)
                        .map(|i| (mk(format!("q{t}-{i}")), resp(&format!("q{t}-{i}"))))
                        .collect(),
                );
                std::thread::spawn(move || {
                    for (k, r) in &keys {
                        c.put(k, r);
                    }
                })
            })
            .collect();
        for h in hs {
            h.join().unwrap();
        }
        let tmp_left = std::fs::read_dir(root.join("c"))
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with(".tmp"))
            .count();
        assert_eq!(tmp_left, 0, "no orphaned temp files");
        for t in 0..4 {
            for i in 0..30 {
                assert!(
                    c.get(&mk(format!("q{t}-{i}"))).is_some(),
                    "entry q{t}-{i} lost or overwritten"
                );
            }
        }
    }
}
