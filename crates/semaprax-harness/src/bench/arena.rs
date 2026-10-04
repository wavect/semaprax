//! One private working area per profile: harness home, project copies, the
//! profile's `semaprax.harness.toml` and its adopted + trusted descriptors.
//!
//! Adoption and trust go through the same `adopt`/`trust` verbs a user runs.
//! Nothing here names a product: descriptors, upstream variables and runtime
//! variables all come from the corpus profile record.

use super::corpus::{Corpus, ProfileSpec};
use crate::cli::{run, Environment};
use crate::json::sha256_plain;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub struct Arena {
    pub root: PathBuf,
    pub env: Environment,
    pub profile: ProfileSpec,
    /// `Some(reason)` when the profile cannot run here: an untested cell.
    pub untested: Option<String>,
    pub projects: BTreeMap<String, PathBuf>,
    pub trust_before: String,
}

pub fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let (p, q) = (e.path(), to.join(e.file_name()));
        if e.file_type()?.is_dir() {
            copy_tree(&p, &q)?;
        } else {
            std::fs::copy(&p, &q)?;
        }
    }
    Ok(())
}

/// Nearest ancestor of `from` that holds `packages/semaprax-harness-adapters`.
pub fn locate_repo(from: &Path) -> Option<PathBuf> {
    from.canonicalize()
        .ok()?
        .ancestors()
        .find(|a| a.join("packages/semaprax-harness-adapters").is_dir())
        .map(Path::to_path_buf)
}

fn s(a: &[&str]) -> Vec<String> {
    a.iter().map(|x| x.to_string()).collect()
}

const ADAPTERS_REL: &str = "packages/semaprax-harness-adapters";

/// Staged descriptor path inside the arena: `repo:packages/semaprax-harness-adapters/<x>`
/// maps to `adapters/<x>`; a corpus-relative `adapters/<x>` maps to
/// `adapters/corpus-adapters/<x>`. Both keep the SDK at `adapters/sdk`.
pub fn staged_descriptor(root: &Path, p: &str) -> PathBuf {
    match p.strip_prefix("repo:") {
        Some(rel) => root
            .join("adapters")
            .join(rel.strip_prefix(&format!("{ADAPTERS_REL}/")).unwrap_or(rel)),
        None => root
            .join("adapters/corpus-adapters")
            .join(p.strip_prefix("adapters/").unwrap_or(p)),
    }
}

fn config_toml(p: &ProfileSpec) -> String {
    let mut t = String::from("schema = \"semaprax.harness-config.v1\"\n");
    for i in &p.installs {
        t.push_str(&format!(
            "\n[capability.\"{}\"]\nmode = \"required\"\nprovider = \"{}\"\n",
            i.capability, i.provider
        ));
    }
    if !p.skills.is_empty() {
        t.push_str("\n[skills]\nenabled = true\n");
    }
    t
}

fn policy_json(p: &ProfileSpec, vars: &BTreeMap<String, String>) -> Value {
    let mut pol = p.command_view_policy.clone().unwrap_or_else(|| json!({}));
    let m = pol.as_object_mut().expect("policy object");
    m.entry("schema")
        .or_insert(json!("semaprax.harness-command-view-policy.v1"));
    m.entry("min_bytes").or_insert(json!(256));
    m.entry("retention")
        .or_insert(json!({"enabled": true, "ttl_secs": 3600, "max_bytes": 67_108_864}));
    let mut rt = serde_json::Map::new();
    for (k, key) in [("python", "HARNESS_PYTHON"), ("node", "HARNESS_NODE")] {
        if let Some(v) = vars.get(key) {
            rt.insert(k.into(), json!(v));
        }
    }
    m.entry("runtimes").or_insert(Value::Object(rt));
    pol
}

/// Real downstream projects live in git repositories (some indexers need one).
/// Best effort with an isolated identity and config; absent git is not an error.
fn git_init(dir: &Path, home: &Path) {
    let _ = std::fs::create_dir_all(home);
    for args in [
        &["init", "-q"][..],
        &["add", "-A"],
        &["commit", "-qm", "benchmark base"],
    ] {
        let _ = std::process::Command::new("/usr/bin/git")
            .args(args)
            .current_dir(dir)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", home)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "bench")
            .env("GIT_AUTHOR_EMAIL", "bench@example.invalid")
            .env("GIT_COMMITTER_NAME", "bench")
            .env("GIT_COMMITTER_EMAIL", "bench@example.invalid")
            .output();
    }
}

fn strip_upstream(desc: &Path) -> Result<(), String> {
    let mut d: Value = serde_json::from_slice(&std::fs::read(desc).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    d.as_object_mut().ok_or("not an object")?.remove("upstream");
    std::fs::write(desc, d.to_string()).map_err(|e| e.to_string())
}

impl Arena {
    pub fn prepare(
        corpus: &Corpus,
        profile: &ProfileSpec,
        repo: &Path,
        work: &Path,
        vars: &BTreeMap<String, String>,
        compiler: Option<PathBuf>,
    ) -> Arena {
        let root = work.join(&profile.id);
        let _ = std::fs::remove_dir_all(&root);
        let home = root.join("home");
        let mut env_vars = BTreeMap::new();
        env_vars.insert("PATH".to_string(), "/usr/bin:/bin".to_string());
        env_vars.insert(
            "HOME".to_string(),
            root.join("userhome").display().to_string(),
        );
        for (k, v) in vars {
            if k.starts_with("HARNESS_") {
                env_vars.insert(k.clone(), v.clone());
            }
        }
        let env = Environment {
            harness_home: Some(home.clone()),
            compiler,
            cwd: root.clone(),
            vars: env_vars,
        };
        let mut a = Arena {
            root: root.clone(),
            env,
            profile: profile.clone(),
            untested: None,
            projects: BTreeMap::new(),
            trust_before: String::new(),
        };
        let fail = |a: &mut Arena, why: String| {
            a.untested = Some(why);
        };
        if let Some(why) = &profile.skip {
            fail(&mut a, why.clone());
            return a;
        }
        if let Some(missing) = profile.requires_env.iter().find(|k| !vars.contains_key(*k)) {
            fail(
                &mut a,
                format!(
                    "untested: `{missing}` not provided (tool or runtime absent on this machine)"
                ),
            );
            return a;
        }
        if let Err(e) = std::fs::create_dir_all(&home)
            .and_then(|_| std::fs::create_dir_all(root.join("userhome")))
        {
            fail(&mut a, format!("untested: arena: {e}"));
            return a;
        }
        let _ = std::fs::write(
            home.join("command-view.json"),
            policy_json(profile, vars).to_string(),
        );
        for (id, p) in &corpus.projects {
            let dst = root.join("projects").join(id);
            if let Err(e) = copy_tree(&p.dir, &dst) {
                fail(&mut a, format!("untested: project copy: {e}"));
                return a;
            }
            let _ = std::fs::write(dst.join("semaprax.harness.toml"), config_toml(profile));
            git_init(&dst, &root.join("userhome"));
            a.projects.insert(id.clone(), dst);
        }
        if let Err(e) = copy_tree(&repo.join(ADAPTERS_REL), &root.join("adapters")) {
            fail(&mut a, format!("untested: stage adapters: {e}"));
            return a;
        }
        if corpus.dir.join("adapters").is_dir() {
            let _ = copy_tree(
                &corpus.dir.join("adapters"),
                &root.join("adapters/corpus-adapters"),
            );
        }
        let mut adopted = std::collections::BTreeSet::new();
        for i in &profile.installs {
            if !adopted.insert(i.provider.clone()) {
                continue; // one adoption per provider, several pinned capabilities
            }
            let desc = staged_descriptor(&root, &i.descriptor);
            if i.strip_upstream {
                if let Err(e) = strip_upstream(&desc) {
                    fail(
                        &mut a,
                        format!("untested: descriptor `{}`: {e}", i.descriptor),
                    );
                    return a;
                }
            }
            let mut args = s(&["adopt", desc.to_str().unwrap_or("")]);
            if let Some(var) = &i.upstream_env {
                match vars.get(var) {
                    Some(v) => {
                        args.push("--upstream".into());
                        args.push(v.clone());
                    }
                    None => {
                        fail(
                            &mut a,
                            format!("untested: upstream variable `{var}` not provided"),
                        );
                        return a;
                    }
                }
            }
            let o = run(&args, &a.env);
            if o.code != 0 {
                let why = format!(
                    "untested: adopt `{}` refused: {}",
                    i.provider,
                    o.stderr.lines().next().unwrap_or("")
                );
                fail(&mut a, why);
                return a;
            }
            let o = run(&s(&["trust", &i.provider]), &a.env);
            if o.code != 0 {
                let why = format!(
                    "untested: trust `{}` refused: {}",
                    i.provider,
                    o.stderr.lines().next().unwrap_or("")
                );
                fail(&mut a, why);
                return a;
            }
        }
        // Skill roots: approved machine-locally through `adopt --skills`, then
        // enabled per project by the `[skills]` table written above.
        if let Some(first) = a.projects.values().next().cloned() {
            for sr in &profile.skills {
                let dir = staged_descriptor(&root, sr);
                let o = run(
                    &s(&[
                        "adopt",
                        "--skills",
                        dir.to_str().unwrap_or(""),
                        "--origin",
                        "benchmark",
                        "--project",
                        first.to_str().unwrap_or(""),
                    ]),
                    &a.env,
                );
                if o.code != 0 {
                    let why = format!(
                        "untested: adopt --skills refused: {}",
                        o.stderr.lines().next().unwrap_or("")
                    );
                    fail(&mut a, why);
                    return a;
                }
            }
        }
        a.trust_before = a.trust_digest();
        a
    }

    /// Staged skill root directories of this profile.
    pub fn skill_roots(&self) -> Vec<PathBuf> {
        self.profile
            .skills
            .iter()
            .map(|s| staged_descriptor(&self.root, s))
            .collect()
    }

    /// Re-open an arena another process prepared (cell subprocess): same
    /// layout and environment, no adoption, no copying.
    pub fn attach(
        corpus: &Corpus,
        profile: &ProfileSpec,
        work: &Path,
        vars: &BTreeMap<String, String>,
        compiler: Option<PathBuf>,
    ) -> Arena {
        let root = work.join(&profile.id);
        let mut env_vars = BTreeMap::new();
        env_vars.insert("PATH".to_string(), "/usr/bin:/bin".to_string());
        env_vars.insert(
            "HOME".to_string(),
            root.join("userhome").display().to_string(),
        );
        for (k, v) in vars {
            if k.starts_with("HARNESS_") {
                env_vars.insert(k.clone(), v.clone());
            }
        }
        let env = Environment {
            harness_home: Some(root.join("home")),
            compiler,
            cwd: root.clone(),
            vars: env_vars,
        };
        let projects = corpus
            .projects
            .keys()
            .map(|id| (id.clone(), root.join("projects").join(id)))
            .collect();
        let mut a = Arena {
            root,
            env,
            profile: profile.clone(),
            untested: None,
            projects,
            trust_before: String::new(),
        };
        a.trust_before = a.trust_digest();
        a
    }

    /// Digest of the machine-local grants; any change during cells is a
    /// permission-widening (or narrowing) signal for the gate.
    pub fn trust_digest(&self) -> String {
        let p = self.env.harness_home.as_ref().map(|h| h.join("trust.json"));
        sha256_plain(&p.and_then(|p| std::fs::read(p).ok()).unwrap_or_default())
    }

    /// Cold state: no provider cache or retained output survives.
    pub fn reset_cold(&self) {
        if let Some(h) = &self.env.harness_home {
            let _ = std::fs::remove_dir_all(h.join("cache"));
            let _ = std::fs::remove_dir_all(h.join("retention"));
        }
    }

    /// Bytes of state a provider left behind (indexes, caches, retention).
    pub fn disk_bytes(&self) -> u64 {
        match &self.env.harness_home {
            Some(h) => {
                super::measure::dir_bytes(&h.join("cache"))
                    + super::measure::dir_bytes(&h.join("retention"))
            }
            None => 0,
        }
    }
}
