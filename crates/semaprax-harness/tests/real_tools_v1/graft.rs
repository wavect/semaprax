//! Provisioned real-tool evidence for the Graft context provider (HP-06) and
//! the shared downstream-project rig also used by `graphify.rs` (HP-07).
//! Requires SEMAPRAX_COMPILER, HARNESS_GRAFT, HARNESS_NODE (see tests/real_tools_v1.rs).

use crate::support::{fixture_dir, repo_root, required_tool};
use semaprax_harness::cli::{run, Environment, Outcome};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const GRAFT_ID: &str = "org.nanonets/graft-context";
pub const GRAPHIFY_ID: &str = "com.graphify-labs/graphify-context";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Graft,
    /// The newer qualified Graft install (`HARNESS_GRAFT_NEW`); same provider id and adapter.
    GraftNew,
    Graphify,
}

impl Tool {
    pub fn id(self) -> &'static str {
        match self {
            Tool::Graft | Tool::GraftNew => GRAFT_ID,
            Tool::Graphify => GRAPHIFY_ID,
        }
    }
    pub fn dir(self) -> &'static str {
        match self {
            Tool::Graft | Tool::GraftNew => "graft",
            Tool::Graphify => "graphify",
        }
    }
    pub fn upstream_var(self) -> &'static str {
        match self {
            Tool::Graft => "HARNESS_GRAFT",
            Tool::GraftNew => "HARNESS_GRAFT_NEW",
            Tool::Graphify => "HARNESS_GRAPHIFY",
        }
    }
}

pub fn copy_tree(from: &Path, to: &Path) {
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

pub fn git(dir: &Path, args: &[&str]) {
    let s = Command::new("/usr/bin/git")
        .args([
            "-C",
            dir.to_str().unwrap(),
            "-c",
            "user.email=t@t",
            "-c",
            "user.name=t",
        ])
        .args(args)
        .output()
        .expect("git");
    assert!(
        s.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&s.stderr)
    );
}

/// A downstream mixed-language project plus a private harness home.
pub struct Rig {
    pub tool: Tool,
    pub base: PathBuf,
    pub home: PathBuf,
    pub project: PathBuf,
    pub env: Environment,
}

impl Rig {
    pub fn new(tool: Tool, tag: &str) -> Rig {
        Rig::from_dir(
            tool,
            tag,
            &repo_root().join("crates/semaprax-harness/tests/fixtures/real_context"),
        )
    }

    /// A rig whose project is a git-initialised copy of `src`.
    pub fn from_dir(tool: Tool, tag: &str, src: &Path) -> Rig {
        let base = fixture_dir(&format!("hp-hp0607-{tag}"))
            .canonicalize()
            .unwrap();
        let project = base.join("project");
        copy_tree(src, &project);
        git(&project, &["init", "-q"]);
        git(&project, &["add", "-A"]);
        git(&project, &["commit", "-qm", "init"]);
        let home = base.join("home");
        std::fs::create_dir_all(&home).unwrap();
        let mut vars = BTreeMap::new();
        for k in ["HARNESS_NODE", "HARNESS_PYTHON"] {
            if let Some(v) = std::env::var_os(k) {
                vars.insert(k.to_string(), v.to_string_lossy().into_owned());
            }
        }
        vars.insert("PATH".into(), "/usr/bin:/bin".into());
        let env = Environment {
            harness_home: Some(home.clone()),
            compiler: Some(required_tool("SEMAPRAX_COMPILER")),
            cwd: base.clone(),
            vars,
        };
        Rig {
            tool,
            base,
            home,
            project,
            env,
        }
    }

    pub fn sh(&self, args: &[&str]) -> Outcome {
        run(
            &args.iter().map(|a| a.to_string()).collect::<Vec<_>>(),
            &self.env,
        )
    }

    pub fn descriptor(tool: Tool) -> PathBuf {
        repo_root()
            .join("packages/semaprax-harness-adapters")
            .join(tool.dir())
            .join("harness-provider.json")
    }

    /// `adopt --upstream` + `trust` one real tool without selecting it.
    pub fn adopt_trust(&self, tool: Tool) {
        let up = required_tool(tool.upstream_var());
        let d = Self::descriptor(tool);
        let o = self.sh(&[
            "adopt",
            d.to_str().unwrap(),
            "--upstream",
            up.to_str().unwrap(),
        ]);
        assert_eq!(o.code, 0, "adopt: {}{}", o.stdout, o.stderr);
        assert!(
            o.stdout.contains("compatible=true"),
            "upstream not identified: {}",
            o.stdout
        );
        let o = self.sh(&["trust", tool.id()]);
        assert_eq!(o.code, 0, "trust: {}{}", o.stdout, o.stderr);
    }

    /// Adopt, trust, select and resolve the rig's tool.
    pub fn install(&self, _mode: &str) {
        self.adopt_trust(self.tool);
        let o = self.sh(&["resolve", self.project.to_str().unwrap()]);
        assert_eq!(o.code, 0, "resolve: {}{}", o.stdout, o.stderr);
    }

    /// The only thing that changes when a project switches provider.
    pub fn select(&self, tool: Tool, mode: &str) {
        std::fs::write(
            self.project.join("semaprax.harness.toml"),
            format!(
                "schema = \"semaprax.harness-config.v1\"\n\n[capability.\"context.repository\"]\nmode = \"{mode}\"\nprovider = \"{}\"\n",
                tool.id()
            ),
        )
        .unwrap();
    }

    pub fn write(&self, rel: &str, text: &str) {
        let p = self.project.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    pub fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.project.join(rel)).unwrap()
    }

    /// Harness `context --json`; returns the parsed document.
    pub fn context(&self, args: &[&str]) -> Result<Value, Outcome> {
        let mut a = vec!["context", self.project.to_str().unwrap()];
        a.extend_from_slice(args);
        a.push("--json");
        let o = self.sh(&a);
        if o.code != 0 {
            return Err(o);
        }
        Ok(serde_json::from_str(o.stdout.trim()).expect("context json"))
    }

    pub fn ctx(&self, args: &[&str]) -> Value {
        self.context(args)
            .unwrap_or_else(|o| panic!("context failed: {}{}", o.stdout, o.stderr))
    }

    /// Adapter-owned index files: (count, newest mtime) for reuse checks.
    pub fn index_stamp(&self) -> (usize, std::time::SystemTime) {
        fn walk(d: &Path, out: &mut Vec<std::time::SystemTime>) {
            for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.components().any(|c| {
                    matches!(
                        c.as_os_str().to_str(),
                        Some("gen" | "idx" | "graphify-index")
                    )
                }) {
                    // Only the index itself: adapter scratch dirs are rewritten on every start.
                    if let Ok(m) = e.metadata().and_then(|m| m.modified()) {
                        out.push(m);
                    }
                }
            }
        }
        let mut v = Vec::new();
        walk(&self.home.join("cache/adapters"), &mut v);
        (
            v.len(),
            v.into_iter().max().unwrap_or(std::time::UNIX_EPOCH),
        )
    }

    pub fn purge_context_cache(&self) {
        assert_eq!(self.sh(&["context", "x", "--purge-cache"]).code, 0);
    }
}

/// Standalone `semaprax context <project> <id>` stdout, the reference for native facts.
pub fn standalone(rig: &Rig, id: &str, max_bytes: &str) -> String {
    let o = Command::new(required_tool("SEMAPRAX_COMPILER"))
        .args([
            "context",
            rig.project.to_str().unwrap(),
            id,
            "--depth",
            "1",
            "--max-bytes",
            max_bytes,
        ])
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout).unwrap()
}

/// All items of `native` + `external`.
pub fn items(doc: &Value) -> Vec<&Value> {
    ["native", "external"]
        .iter()
        .flat_map(|k| doc[k].as_array().into_iter().flatten())
        .collect()
}

pub fn external(doc: &Value) -> Vec<&Value> {
    doc["external"].as_array().into_iter().flatten().collect()
}

// ---- scenarios shared by graft.rs and graphify.rs ---------------------------

const Q: &str = "ledger.core.split-evenly renderTotal";
const SPX_FILES: [&str; 3] = ["src/app.spx", "src/core.spx", "src/tests.spx"];

/// Digest convention of the broker: lines joined by LF, no trailing terminator.
pub fn line_digest(text: &str, start: u64, end: u64) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let body = lines[(start - 1) as usize..end as usize].join("\n");
    semaprax_harness::json::sha256_plain(body.as_bytes())
}

pub fn provider_report<'a>(doc: &'a Value, id: &str) -> &'a Value {
    doc["providers"]
        .as_array()
        .and_then(|a| a.iter().find(|p| p["provider_id"] == id))
        .unwrap_or_else(|| panic!("no provider report for {id}: {}", doc["providers"]))
}

pub fn text_has(doc: &Value, needle: &str) -> bool {
    items(doc)
        .iter()
        .any(|i| i["text"].as_str().is_some_and(|t| t.contains(needle)))
}

/// Checked `.spx` facts come from the compiler; external items are labelled and exactly spanned.
pub fn scenario_facts_and_spans(tool: Tool) {
    let rig = Rig::new(tool, "facts");
    rig.install("required");
    let doc = rig.ctx(&[Q, "--max-bytes", "16000"]);
    let native = doc["native"].as_array().unwrap();
    assert_eq!(native.len(), 1, "one compiler fact: {native:?}");
    assert_eq!(native[0]["provenance"], "compiler-verified");
    assert_eq!(native[0]["stable_id"], "ledger.core.split-evenly");
    assert_eq!(native[0]["provider_id"], "semaprax.compiler");
    assert_eq!(
        native[0]["text"].as_str().unwrap().trim(),
        standalone(&rig, "ledger.core.split-evenly", "16000").trim(),
        "native text is the standalone compiler output"
    );
    let ext = external(&doc);
    assert!(
        !ext.is_empty(),
        "{tool_id} returned no items",
        tool_id = tool.id()
    );
    for i in &ext {
        assert!(
            matches!(i["provenance"].as_str(), Some("structural" | "inferred")),
            "{i}"
        );
        assert_eq!(i["provider_id"], tool.id());
        assert_eq!(i["authorizes_edits"], false);
        let (path, s, e) = (
            i["path"].as_str().unwrap(),
            i["span"]["start_line"].as_u64().unwrap(),
            i["span"]["end_line"].as_u64().unwrap(),
        );
        assert!(!path.ends_with(".spx"), "no external item for .spx: {i}");
        let want = line_digest(&rig.read(path), s, e);
        assert_eq!(
            i["digest"].as_str().unwrap(),
            want,
            "digest of {path}:{s}-{e}"
        );
        assert_eq!(i["verified"], true, "{i}");
    }
    assert!(text_has(&doc, "renderTotal"));
    // .spx files are reported as skipped, not silently dropped.
    let rep = provider_report(&doc, tool.id());
    let skipped = rep["coverage"]["skipped"].as_array().unwrap();
    for f in SPX_FILES {
        let hit = skipped
            .iter()
            .find(|s| s["path"] == f)
            .unwrap_or_else(|| panic!("{f} not skipped: {skipped:?}"));
        assert!(
            hit["reason"]
                .as_str()
                .unwrap()
                .to_lowercase()
                .contains("semaprax"),
            "{hit}"
        );
    }
    assert_eq!(rep["coverage"]["complete"], false);
    // The project tree is untouched apart from the files the test wrote.
    let status = Command::new("/usr/bin/git")
        .args([
            "-C",
            rig.project.to_str().unwrap(),
            "status",
            "--porcelain",
            "--ignored",
            "-uall",
        ])
        .output()
        .unwrap();
    let dirty: Vec<String> = String::from_utf8_lossy(&status.stdout)
        .lines()
        .filter(|l| !l.contains("semaprax.harness"))
        .map(str::to_string)
        .collect();
    assert!(
        dirty.is_empty(),
        "adapter wrote into the user's tree: {dirty:?}"
    );
}

/// Rename removes the old symbol; an edit refreshes a stale index; a purged cache reuses the warm index.
pub fn scenario_rename_stale_warm(tool: Tool) {
    let rig = Rig::new(tool, "rename");
    rig.install("required");
    let d0 = rig.ctx(&["renderTotal", "--max-bytes", "16000"]);
    assert!(text_has(&d0, "renderTotal"));
    let built = rig.index_stamp();
    // Warm: cache purged, nothing changed -> the adapter's index is reused, not rebuilt.
    rig.purge_context_cache();
    let d1 = rig.ctx(&["renderTotal", "--max-bytes", "16000"]);
    assert!(text_has(&d1, "renderTotal"));
    assert_eq!(rig.index_stamp(), built, "warm run rebuilt the index");
    // Edit: old symbol gone, new one found, index refreshed.
    let edited = rig
        .read("web/render.ts")
        .replace("renderTotal", "renderSum");
    rig.write("web/render.ts", &edited);
    let d2 = rig.ctx(&["renderTotal renderSum", "--max-bytes", "16000"]);
    assert!(
        !text_has(&d2, "renderTotal"),
        "stale symbol survived the rename: {d2}"
    );
    assert!(text_has(&d2, "renderSum"));
    assert_ne!(rig.index_stamp(), built, "edit did not refresh the index");
    for i in external(&d2) {
        assert_eq!(
            i["verified"], true,
            "post-edit item must match the working tree: {i}"
        );
    }
}

/// A second copy of the project (worktree switch) gets its own index and its own answers.
pub fn scenario_worktree_switch(tool: Tool) {
    let rig = Rig::new(tool, "wt");
    rig.install("required");
    let a = rig.ctx(&["parse_amount", "--max-bytes", "16000"]);
    assert!(text_has(&a, "parse_amount"));
    let other = rig.base.join("project-b");
    copy_tree(&rig.project, &other);
    let edited = std::fs::read_to_string(other.join("tools/report.py"))
        .unwrap()
        .replace("parse_amount", "parse_cents");
    std::fs::write(other.join("tools/report.py"), edited).unwrap();
    let mut args = vec!["context".to_string(), other.display().to_string()];
    args.extend([
        "parse_cents parse_amount".into(),
        "--max-bytes".into(),
        "16000".into(),
        "--json".into(),
    ]);
    let o = run(&args, &rig.env);
    assert_eq!(o.code, 0, "{}{}", o.stdout, o.stderr);
    let b: Value = serde_json::from_str(o.stdout.trim()).unwrap();
    assert!(
        text_has(&b, "parse_cents") && !text_has(&b, "parse_amount"),
        "{b}"
    );
    assert_ne!(a["snapshot"]["worktree_id"], b["snapshot"]["worktree_id"]);
    let dirs = std::fs::read_dir(rig.home.join("cache/adapters"))
        .unwrap()
        .count();
    assert_eq!(dirs, 2, "one adapter cache per worktree");
    // The original worktree is unaffected.
    let again = rig.ctx(&["parse_amount", "--max-bytes", "16000"]);
    assert!(text_has(&again, "parse_amount") && !text_has(&again, "parse_cents"));
}

/// Provider absent: auto falls back to native context, required refuses clearly.
pub fn scenario_absent(tool: Tool) {
    let rig = Rig::new(tool, "absent");
    rig.install("auto");
    assert!(!external(&rig.ctx(&[Q, "--max-bytes", "16000"])).is_empty());
    let o = rig.sh(&["revoke", tool.id()]);
    assert_eq!(o.code, 0, "revoke: {}{}", o.stdout, o.stderr);
    let doc = rig.ctx(&[Q, "--max-bytes", "16000"]);
    assert_eq!(
        doc["native"].as_array().unwrap().len(),
        1,
        "native fallback keeps compiler facts"
    );
    assert!(
        external(&doc).is_empty(),
        "revoked provider must not answer"
    );
    rig.select(tool, "required");
    let err = rig
        .context(&[Q, "--max-bytes", "16000"])
        .expect_err("required with no provider must fail");
    assert_ne!(err.code, 0);
    assert!(
        err.stderr.contains("SPX-HP"),
        "stable diagnostic: {}",
        err.stderr
    );
    eprintln!("required-without-provider: {}", err.stderr.trim());
}

/// Planted credentials never reach the adapter or the index; the answer still works.
pub fn scenario_planted_secrets(tool: Tool) {
    let mut rig = Rig::new(tool, "secrets");
    let planted = [
        ("GRAFT_API_KEY", "sk-planted-graft-0607"),
        ("OPENAI_API_KEY", "sk-planted-openai-0607"),
        ("ANTHROPIC_API_KEY", "sk-planted-anthropic-0607"),
        ("GRAFT_PROVIDER", "openai-planted-0607"),
        ("HTTPS_PROXY", "http://planted-proxy-0607.invalid:9"),
    ];
    for (k, v) in planted {
        std::env::set_var(k, v);
        rig.env.vars.insert(k.into(), v.into());
    }
    rig.install("required");
    let doc = rig.ctx(&[Q, "--max-bytes", "16000"]);
    assert!(!external(&doc).is_empty());
    let mut hay = String::new();
    fn walk(d: &Path, out: &mut String) {
        for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if let Ok(b) = std::fs::read(&p) {
                out.push_str(&String::from_utf8_lossy(&b));
            }
        }
    }
    walk(&rig.home, &mut hay);
    hay.push_str(&doc.to_string());
    for (_, v) in planted {
        assert!(
            !hay.contains(v),
            "planted value {v} leaked into the index, cache or output"
        );
    }
    for (k, _) in planted {
        std::env::remove_var(k);
    }
}

/// Cold build and warm reuse with every network operation denied by the OS sandbox.
pub fn scenario_offline_inner(tool: Tool) {
    let rig = Rig::new(tool, "offline");
    rig.install("required");
    let cold = rig.ctx(&[Q, "--max-bytes", "16000"]);
    assert!(!external(&cold).is_empty());
    let built = rig.index_stamp();
    rig.purge_context_cache();
    let warm = rig.ctx(&[Q, "--max-bytes", "16000"]);
    assert_eq!(external(&cold).len(), external(&warm).len());
    assert_eq!(rig.index_stamp(), built, "warm run rebuilt the index");
}

/// Re-runs `test_name` (an ignored test of this binary) under a network-denying sandbox.
pub fn run_offline(test_name: &str) {
    let exe = std::env::current_exe().unwrap();
    let out = Command::new("/usr/bin/sandbox-exec")
        .args(["-p", "(version 1)(allow default)(deny network*)"])
        .arg(&exe)
        .args([test_name, "--exact", "--ignored", "--nocapture"])
        .output()
        .expect("sandbox-exec");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.status.success() && text.contains("1 passed"),
        "offline run failed:\n{text}"
    );
    // The sandbox really denies network: a connect from inside it must fail.
    let probe = Command::new("/usr/bin/sandbox-exec")
        .args([
            "-p",
            "(version 1)(allow default)(deny network*)",
            "/usr/bin/nc",
            "-z",
            "-w",
            "2",
            "1.1.1.1",
            "53",
        ])
        .output()
        .expect("probe");
    assert!(
        !probe.status.success(),
        "sandbox did not block the network probe"
    );
}

/// Provider switch by editing only semaprax.harness.toml: compiler facts identical, external provenance differs.
pub fn scenario_switch_provider() {
    let rig = Rig::new(Tool::Graft, "switch");
    rig.adopt_trust(Tool::Graft);
    rig.adopt_trust(Tool::Graphify);
    rig.select(Tool::Graft, "required");
    let g = rig.ctx(&[Q, "--max-bytes", "16000"]);
    rig.select(Tool::Graphify, "required");
    let h = rig.ctx(&[Q, "--max-bytes", "16000"]);
    assert_eq!(
        g["native"], h["native"],
        "compiler facts must not depend on the provider"
    );
    let ids = |d: &Value| -> std::collections::BTreeSet<String> {
        external(d)
            .iter()
            .map(|i| i["provider_id"].as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(ids(&g), [GRAFT_ID.to_string()].into());
    assert_eq!(ids(&h), [GRAPHIFY_ID.to_string()].into());
    for d in [&g, &h] {
        assert!(external(d)
            .iter()
            .all(|i| i["provenance"] != "compiler-verified"));
        assert!(text_has(d, "renderTotal"));
    }
    eprintln!(
        "provenance graft={:?} graphify={:?}",
        prov_counts(&g),
        prov_counts(&h)
    );
}

pub fn prov_counts(d: &Value) -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    for i in external(d) {
        *m.entry(i["provenance"].as_str().unwrap().to_string())
            .or_default() += 1;
    }
    m
}

// ---- Graft tests ---------------------------------------------------------------

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER HARNESS_GRAFT HARNESS_NODE"]
fn graft_facts_and_spans() {
    scenario_facts_and_spans(Tool::Graft);
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER HARNESS_GRAFT HARNESS_NODE"]
fn graft_rename_stale_index_and_warm_reuse() {
    scenario_rename_stale_warm(Tool::Graft);
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER HARNESS_GRAFT HARNESS_NODE"]
fn graft_worktree_switch() {
    scenario_worktree_switch(Tool::Graft);
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER HARNESS_GRAFT HARNESS_NODE"]
fn graft_absent_provider_fallback_and_required() {
    scenario_absent(Tool::Graft);
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER HARNESS_GRAFT HARNESS_NODE"]
fn graft_planted_secrets_not_inherited() {
    scenario_planted_secrets(Tool::Graft);
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER HARNESS_GRAFT HARNESS_NODE"]
fn graft_offline_inner() {
    scenario_offline_inner(Tool::Graft);
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER HARNESS_GRAFT HARNESS_NODE, macOS sandbox-exec"]
fn graft_network_denied_cold_and_warm() {
    run_offline("graft::graft_offline_inner");
}

// ---- newer qualified Graft (HN-08): the same scenarios through the host ----------------

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER HARNESS_GRAFT_NEW HARNESS_NODE"]
fn graft_new_facts_and_spans() {
    scenario_facts_and_spans(Tool::GraftNew);
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER HARNESS_GRAFT_NEW HARNESS_NODE"]
fn graft_new_rename_stale_index_and_warm_reuse() {
    scenario_rename_stale_warm(Tool::GraftNew);
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER HARNESS_GRAFT_NEW HARNESS_NODE"]
fn graft_new_worktree_switch() {
    scenario_worktree_switch(Tool::GraftNew);
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER HARNESS_GRAFT_NEW HARNESS_NODE"]
fn graft_new_planted_secrets_not_inherited() {
    scenario_planted_secrets(Tool::GraftNew);
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER HARNESS_GRAFT_NEW HARNESS_NODE"]
fn graft_new_reference_labels_and_no_absence_claim() {
    scenario_references_labels(Tool::GraftNew);
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER HARNESS_GRAFT_NEW HARNESS_NODE"]
fn graft_new_offline_inner() {
    scenario_offline_inner(Tool::GraftNew);
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER HARNESS_GRAFT_NEW HARNESS_NODE, macOS sandbox-exec"]
fn graft_new_network_denied_cold_and_warm() {
    run_offline("graft::graft_new_offline_inner");
}

/// The newer install is adopted by exact version from the descriptor data alone, and the
/// report names the version the adapter qualified (no compiler edit selects it).
#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER HARNESS_GRAFT_NEW HARNESS_NODE"]
fn graft_new_adopted_by_declared_version() {
    let rig = Rig::new(Tool::GraftNew, "adoptver");
    let up = required_tool("HARNESS_GRAFT_NEW");
    let d = Rig::descriptor(Tool::GraftNew);
    let o = rig.sh(&[
        "adopt",
        d.to_str().unwrap(),
        "--upstream",
        up.to_str().unwrap(),
    ]);
    assert_eq!(o.code, 0, "{}{}", o.stdout, o.stderr);
    assert!(o.stdout.contains("compatible=true"), "{}", o.stdout);
    assert!(
        o.stdout.contains("0.21"),
        "adopted version not reported: {}",
        o.stdout
    );
}

// ---- measurement (HP-06 criterion 4, HP-07 revisit gate) ---------------------------

enum Fact {
    /// Some returned item's span covers the first line of `path` containing `needle`.
    Line(&'static str, &'static str),
    /// Some returned item's text contains the string.
    Text(&'static str),
}

struct Task {
    name: &'static str,
    corpus: &'static str,
    args: &'static [&'static str],
    facts: &'static [Fact],
    /// Files a human would paste for the same question.
    reference: &'static [&'static str],
}

const TASKS: &[Task] = &[
    Task {
        name: "T1 where is renderTotal defined and who calls it",
        corpus: "fixture",
        args: &["renderTotal", "--references", "--symbol", "renderTotal"],
        facts: &[
            Fact::Line("web/render.ts", "export function renderTotal"),
            Fact::Line("web/render.ts", "return renderTotal(this.rows)"),
        ],
        reference: &["web/render.ts"],
    },
    Task {
        name: "T2 compiler facts for ledger.core.split-evenly",
        corpus: "fixture",
        args: &["ledger.core.split-evenly"],
        facts: &[
            Fact::Text("ledger.core.split-evenly"),
            Fact::Line("src/core.spx", "fn split_evenly"),
        ],
        reference: &["src/core.spx", "src/app.spx"],
    },
    Task {
        name: "T3 totals parsing in report.py and rendering in render.ts",
        corpus: "fixture",
        args: &["total report render"],
        facts: &[
            Fact::Line("tools/report.py", "def build_report"),
            Fact::Line("web/render.ts", "export function renderTotal"),
        ],
        reference: &["tools/report.py", "web/render.ts"],
    },
    Task {
        name: "T4 where is span_digest defined (Rust)",
        corpus: "repo",
        args: &["span_digest"],
        facts: &[Fact::Line(
            "crates/semaprax-harness/src/context/identity.rs",
            "pub fn span_digest",
        )],
        reference: &["crates/semaprax-harness/src/context/identity.rs"],
    },
    Task {
        name: "T5 where is ensureFresh defined (JavaScript)",
        corpus: "repo",
        args: &["ensureFresh"],
        facts: &[Fact::Line(
            "packages/semaprax-harness-adapters/graft/lib/project.mjs",
            "export async function ensureFresh",
        )],
        reference: &["packages/semaprax-harness-adapters/graft/lib/project.mjs"],
    },
    Task {
        name: "T6 where is extraction_errors defined (Python)",
        corpus: "repo",
        args: &["extraction_errors"],
        facts: &[Fact::Line(
            "packages/semaprax-harness-adapters/graphify/adapter.py",
            "def extraction_errors",
        )],
        reference: &["packages/semaprax-harness-adapters/graphify/adapter.py"],
    },
];

fn line_of(project: &Path, path: &str, needle: &str) -> u64 {
    let text =
        std::fs::read_to_string(project.join(path)).unwrap_or_else(|e| panic!("{path}: {e}"));
    text.lines()
        .position(|l| l.contains(needle))
        .unwrap_or_else(|| panic!("{needle} not in {path}")) as u64
        + 1
}

fn fact_present(project: &Path, doc: &Value, f: &Fact) -> bool {
    match f {
        Fact::Text(t) => text_has(doc, t),
        Fact::Line(p, n) => {
            let line = line_of(project, p, n);
            items(doc).iter().any(|i| {
                i["path"] == *p
                    && i["span"]["start_line"].as_u64().unwrap() <= line
                    && line <= i["span"]["end_line"].as_u64().unwrap()
            })
        }
    }
}

fn repo_corpus(dst: &Path) {
    let root = repo_root();
    for rel in [
        "crates/semaprax-harness/src/context",
        "packages/semaprax-harness-adapters/graft/lib",
    ] {
        copy_tree(&root.join(rel), &dst.join(rel));
    }
    let g = "packages/semaprax-harness-adapters/graphify/adapter.py";
    std::fs::create_dir_all(dst.join(g).parent().unwrap()).unwrap();
    std::fs::copy(root.join(g), dst.join(g)).unwrap();
}

fn dir_bytes(d: &Path, only: &[&str]) -> u64 {
    let mut n = 0;
    for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
        let p = e.path();
        let tagged = only.is_empty()
            || p.components()
                .any(|c| only.contains(&c.as_os_str().to_str().unwrap_or("")));
        if p.is_dir() {
            n += dir_bytes(&p, only);
        } else if tagged {
            n += e.metadata().map(|m| m.len()).unwrap_or(0);
        }
    }
    n
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER HARNESS_GRAFT HARNESS_NODE HARNESS_GRAPHIFY HARNESS_PYTHON"]
fn measure_model_facing_bytes() {
    let tmp = fixture_dir("hp-hp0607-measure").canonicalize().unwrap();
    let repo_src = tmp.join("repo-src");
    repo_corpus(&repo_src);
    let fixture_src = repo_root().join("crates/semaprax-harness/tests/fixtures/real_context");
    let budget = "16000";
    let mut rows = Vec::new();
    for corpus in ["fixture", "repo"] {
        let src = if corpus == "fixture" {
            fixture_src.clone()
        } else {
            repo_src.clone()
        };
        // One rig per arm so each arm's first query is a genuine cold start.
        let native = Rig::from_dir(Tool::Graft, &format!("m-{corpus}-native"), &src);
        let graft = Rig::from_dir(Tool::Graft, &format!("m-{corpus}-graft"), &src);
        graft.install("required");
        let graphify = Rig::from_dir(Tool::Graphify, &format!("m-{corpus}-graphify"), &src);
        graphify.install("required");
        for t in TASKS.iter().filter(|t| t.corpus == corpus) {
            let reference: u64 = t
                .reference
                .iter()
                .map(|f| std::fs::metadata(native.project.join(f)).unwrap().len())
                .sum();
            rows.push(
                serde_json::json!({"task": t.name, "arm": "full-source", "bytes": reference,
                "facts": format!("{}/{}", t.facts.len(), t.facts.len()), "files": t.reference}),
            );
            for (arm, rig) in [
                ("native-only", &native),
                ("native+graft", &graft),
                ("native+graphify", &graphify),
            ] {
                let mut a: Vec<&str> = t.args.to_vec();
                a.extend(["--max-bytes", budget]);
                let time = |rig: &Rig| {
                    let t0 = std::time::Instant::now();
                    let o = {
                        let mut v = vec!["context", rig.project.to_str().unwrap()];
                        v.extend(a.iter().copied());
                        v.push("--json");
                        rig.sh(&v)
                    };
                    (o, t0.elapsed().as_millis() as u64)
                };
                let (o, cold_ms) = time(rig);
                assert_eq!(o.code, 0, "{arm} {}: {}{}", t.name, o.stdout, o.stderr);
                let doc: Value = serde_json::from_str(o.stdout.trim()).unwrap();
                rig.purge_context_cache();
                let (_, warm_ms) = time(rig); // adapter restarted, warm on-disk index
                let (_, cached_ms) = time(rig); // broker cache hit
                let found: Vec<bool> = t
                    .facts
                    .iter()
                    .map(|f| fact_present(&native.project, &doc, f))
                    .collect();
                let idx = dir_bytes(&rig.home.join("cache/adapters"), &["idx", "graphify-index"]);
                rows.push(serde_json::json!({"task": t.name, "arm": arm, "bytes": o.stdout.trim().len(),
                    "facts": format!("{}/{}", found.iter().filter(|b| **b).count(), found.len()),
                    "native_items": doc["native"].as_array().unwrap().len(),
                    "external_items": doc["external"].as_array().unwrap().len(),
                    "larger_than_reference": o.stdout.trim().len() as u64 > reference,
                    "cold_ms": cold_ms, "warm_ms": warm_ms, "cached_ms": cached_ms, "index_bytes": idx}));
            }
        }
    }
    for r in &rows {
        eprintln!("MEASURE {r}");
    }
}

/// Reference queries: edges are labelled structural/inferred, never exhaustive, never definitive absence.
pub fn scenario_references_labels(tool: Tool) {
    let rig = Rig::new(tool, "refs");
    rig.install("required");
    let doc = rig.ctx(&[
        "formatCents",
        "--references",
        "--symbol",
        "formatCents",
        "--max-bytes",
        "16000",
    ]);
    eprintln!(
        "refs {}: {:?}",
        tool.id(),
        external(&doc)
            .iter()
            .map(|i| (
                i["path"].as_str().unwrap().to_string(),
                i["span"].clone(),
                i["provenance"].as_str().unwrap().to_string()
            ))
            .collect::<Vec<_>>()
    );
    assert!(!external(&doc).is_empty(), "{doc}");
    assert!(external(&doc)
        .iter()
        .all(|i| matches!(i["provenance"].as_str(), Some("structural" | "inferred"))));
    assert_eq!(doc["references"]["exhaustive"], false);
    assert_eq!(doc["references"]["definitive_absence"], false);
    let none = rig.ctx(&[
        "noSuchSymbolAnywhere",
        "--references",
        "--symbol",
        "noSuchSymbolAnywhere",
        "--max-bytes",
        "16000",
    ]);
    assert_eq!(
        none["references"]["definitive_absence"], false,
        "absence must not be asserted by an index that skips files"
    );
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER HARNESS_GRAFT HARNESS_NODE"]
fn graft_reference_labels_and_no_absence_claim() {
    scenario_references_labels(Tool::Graft);
}
