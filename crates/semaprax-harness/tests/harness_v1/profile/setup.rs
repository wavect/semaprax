//! HN-07 `harness setup` and the shared runtime resolver (fixture prefix `hp-hn07`).
//! Fake tools are `#!/bin/sh` scripts that only print a version; the shipped
//! descriptors list `macos-aarch64`, so the adopt/trust cases run there only.

use crate::support::{fixture_dir, write};
use semaprax_harness::cli::{run, Environment};
use semaprax_harness::contract::{CapabilityKind, Runtime};
use semaprax_harness::profile::{self, LocalState};
use serde_json::Value;
use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const SHIPPED: bool = cfg!(all(target_os = "macos", target_arch = "aarch64"));

fn env(home: &Path, cwd: &Path) -> Environment {
    Environment {
        harness_home: Some(home.to_path_buf()),
        compiler: None,
        cwd: cwd.to_path_buf(),
        vars: BTreeMap::new(),
    }
}

fn tool(dir: &Path, name: &str, out: &str) -> PathBuf {
    let p = write(dir, name, &format!("#!/bin/sh\necho \"{out}\"\n"));
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p.canonicalize().unwrap()
}

struct Fx {
    home: PathBuf,
    project: PathBuf,
    bin: PathBuf,
}

fn fx() -> Fx {
    let root = fixture_dir("hp-hn07");
    let project = root.join("proj");
    std::fs::create_dir_all(&project).unwrap();
    let bin = root.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    tool(&bin, "node", "v24.3.0");
    tool(&bin, "python3", "Python 3.12.1");
    Fx {
        home: root.join("home"),
        project: project.canonicalize().unwrap(),
        bin: bin.canonicalize().unwrap(),
    }
}

fn setup(f: &Fx, extra: &[&str]) -> semaprax_harness::cli::Outcome {
    let mut a: Vec<String> = ["setup", "--project"].map(String::from).to_vec();
    a.push(f.project.to_string_lossy().into_owned());
    a.push("--path-dirs".into());
    a.push(f.bin.to_string_lossy().into_owned());
    a.extend(extra.iter().map(|s| s.to_string()));
    run(&a, &env(&f.home, &f.project))
}

fn doc(o: &semaprax_harness::cli::Outcome) -> Value {
    serde_json::from_str(&o.stdout).unwrap_or_else(|e| panic!("{e}: {}{}", o.stdout, o.stderr))
}

fn provider<'a>(d: &'a Value, name: &str) -> &'a Value {
    d["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == name)
        .unwrap()
}

#[test]
fn dry_run_and_plan_only_change_nothing() {
    let f = fx();
    tool(&f.bin, "graft", "graft 0.18.0");
    for extra in [vec!["--dry-run", "--json"], vec!["--json"]] {
        let mut e = vec!["--preset", "local-efficient"];
        e.extend(extra);
        let o = setup(&f, &e);
        assert_eq!(o.code, 0, "{}", o.stderr);
        assert!(!f.home.exists(), "plan must not create machine-local state");
        assert!(!f.project.join("semaprax.harness.toml").exists());
        assert_eq!(doc(&o)["schema"], "semaprax.harness-setup.v1");
    }
}

#[test]
fn setup_adopts_trusts_writes_profile_and_repeat_is_noop() {
    if !SHIPPED {
        return;
    }
    let f = fx();
    let graft = tool(&f.bin, "graft", "graft 0.18.0");
    tool(&f.bin, "rtk", "rtk 0.51.0");
    let a = ["--preset", "local-efficient", "--yes", "--json"];
    let first = setup(&f, &a);
    assert_eq!(first.code, 0, "{}", first.stderr);
    let d = doc(&first);
    assert_eq!(d["noop"], false);
    assert_eq!(provider(&d, "graft")["action"], "adopt");
    // Adopted and trusted from the content-addressed store, not the checkout.
    let st = LocalState::load(&env(&f.home, &f.project)).unwrap();
    let inst = &st.installations["org.nanonets/graft-context"];
    assert!(inst
        .descriptor_path
        .starts_with(f.home.canonicalize().unwrap().join("artifacts")));
    assert_eq!(inst.upstream.as_ref().unwrap().path, graft);
    assert!(inst.runtime.is_some());
    assert!(st.trust.contains_key("org.nanonets/graft-context"));
    let toml = std::fs::read_to_string(f.project.join("semaprax.harness.toml")).unwrap();
    assert!(
        toml.contains("org.nanonets/graft-context") && toml.contains("ai.rtk/rtk-command-view")
    );
    assert!(
        !toml.contains(f.bin.to_str().unwrap()),
        "machine paths never reach the project file"
    );
    // The profile now selects the adopted providers.
    let res = profile::resolve_project(&env(&f.home, &f.project), &f.project).unwrap();
    let launch = &res.launches[&CapabilityKind::ContextRepository];
    assert_eq!(launch.provider_id, "org.nanonets/graft-context");
    // Every resolved launch carries the adopted runtime (context needs no HARNESS_*).
    assert_eq!(launch.runtime, inst.runtime);
    // Repeating is a no-op.
    let again = doc(&setup(&f, &a));
    assert_eq!(again["noop"], true);
    assert_eq!(again["changes"].as_array().unwrap().len(), 0);
    assert_eq!(provider(&again, "graft")["action"], "current");
}

#[test]
fn only_one_repository_provider_is_ever_adopted() {
    if !SHIPPED {
        return;
    }
    let f = fx();
    tool(&f.bin, "graft", "graft 0.18.0");
    tool(&f.bin, "graphify", "graphify 0.9.75");
    let d = doc(&setup(
        &f,
        &["--preset", "local-efficient", "--yes", "--json"],
    ));
    assert_eq!(provider(&d, "graft")["action"], "adopt");
    assert_eq!(provider(&d, "graphify")["action"], "not-selected");
    let st = LocalState::load(&env(&f.home, &f.project)).unwrap();
    assert!(!st
        .installations
        .contains_key("com.graphify-labs/graphify-context"));
    // An explicit graphify choice replaces the default, still exactly one.
    let g = fx();
    tool(&g.bin, "graft", "graft 0.18.0");
    tool(&g.bin, "graphify", "graphify 0.9.75");
    let d = doc(&setup(&g, &["--provider", "graphify", "--yes", "--json"]));
    assert_eq!(provider(&d, "graft")["action"], "not-selected");
    assert_eq!(provider(&d, "graphify")["action"], "adopt");
}

#[test]
fn missing_optional_tools_fall_back_to_builtin() {
    let f = fx();
    let o = setup(&f, &["--preset", "local-efficient", "--yes", "--json"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    let d = doc(&o);
    assert_eq!(provider(&d, "graft")["status"], "unavailable");
    let toml = std::fs::read_to_string(f.project.join("semaprax.harness.toml")).unwrap();
    assert!(toml.contains("semaprax/native-context") && toml.contains("semaprax/raw-command"));
    let res = profile::resolve_project(&env(&f.home, &f.project), &f.project).unwrap();
    assert!(res.launches.is_empty(), "no external provider is launched");
}

#[test]
fn required_missing_provider_is_an_actionable_failure_and_changes_nothing() {
    let f = fx();
    let o = setup(&f, &["--require", "graft", "--yes"]);
    assert_eq!(o.code, 1);
    assert!(
        o.stderr.contains("SPX-HPB060") && o.stderr.contains("@nanonets/graft"),
        "{}",
        o.stderr
    );
    assert!(o.stderr.contains("--path-dirs") && o.stderr.contains("updates"));
    assert!(!f.home.exists() && !f.project.join("semaprax.harness.toml").exists());
}

#[test]
fn untested_version_is_explained_not_substituted() {
    let f = fx();
    tool(&f.bin, "graft", "graft 9.9.9");
    let d = doc(&setup(
        &f,
        &["--preset", "local-efficient", "--dry-run", "--json"],
    ));
    let g = provider(&d, "graft");
    assert_eq!(g["status"], "unavailable");
    assert!(g["detail"].as_str().unwrap().contains("untested"), "{g}");
    let o = setup(&f, &["--provider", "graft", "--dry-run"]);
    assert_eq!(o.code, 1);
    assert!(o.stderr.contains("untested"));
}

#[test]
fn repository_local_executable_is_never_chosen_or_trusted() {
    let f = fx();
    let local = f.project.join("tools");
    tool(&local, "graft", "graft 0.18.0");
    // Neither a searched directory nor an explicit --tool inside the project is used.
    let a = [
        "--path-dirs",
        local.to_str().unwrap(),
        "--require",
        "graft",
        "--yes",
    ];
    let o = setup(&f, &a);
    assert_eq!(o.code, 1, "{}", o.stdout);
    let explicit = format!("graft={}", local.join("graft").display());
    let o = setup(&f, &["--tool", &explicit, "--require", "graft", "--yes"]);
    assert_eq!(o.code, 1);
    assert!(o.stderr.contains("inside the project"), "{}", o.stderr);
    assert!(!f.home.exists());
}

#[test]
fn existing_project_profile_is_preserved() {
    let f = fx();
    let mine = "schema = \"semaprax.harness-config.v1\"\n\n[profile]\nenabled = true\n";
    write(&f.project, "semaprax.harness.toml", mine);
    let d = doc(&setup(&f, &["--yes", "--json"]));
    assert_eq!(d["project_profile"]["action"], "kept");
    assert_eq!(
        std::fs::read_to_string(f.project.join("semaprax.harness.toml")).unwrap(),
        mine
    );
}

#[test]
fn usage_errors_are_refused_before_any_work() {
    let f = fx();
    for bad in [
        vec!["--tool", "graft=relative/graft"],
        vec!["--tool", "emacs=/bin/ls"],
        vec!["--dry-run", "--yes"],
        vec!["--preset", "huge"],
        vec!["--provider", "graft", "--require", "graphify"],
    ] {
        let o = setup(&f, &bad);
        assert_ne!(o.code, 0, "{bad:?}");
    }
    assert!(!f.home.exists());
}

#[test]
fn runtime_resolution_order_is_flag_adopted_policy_env() {
    use semaprax_harness::profile::runtime::{pick, require};
    let mut e = env(Path::new("/h"), Path::new("/c"));
    e.vars.insert("HARNESS_PYTHON".into(), "/env/python".into());
    let (flag, adopted, policy) = (
        Path::new("/flag"),
        Path::new("/adopted"),
        Path::new("/policy"),
    );
    let p = |a, b, c| pick(Runtime::Python, a, b, c, &e);
    assert_eq!(
        p(Some(flag), Some(adopted), Some(policy)),
        Some(flag.into())
    );
    assert_eq!(p(None, Some(adopted), Some(policy)), Some(adopted.into()));
    assert_eq!(p(None, None, Some(policy)), Some(policy.into()));
    assert_eq!(p(None, None, None), Some("/env/python".into()));
    // Node reads its own variable; native adapters need none.
    assert_eq!(pick(Runtime::Node, None, None, None, &e), None);
    assert_eq!(pick(Runtime::Native, Some(flag), None, None, &e), None);
    let err = require("SPX-HPD091", "x/y", Runtime::Node, None, None, None, &e).unwrap_err();
    assert!(err.message.contains("harness setup") && err.message.contains("HARNESS_NODE"));
    assert_eq!(
        require("SPX-HPD091", "x/y", Runtime::Native, None, None, None, &e).unwrap(),
        None
    );
}

#[test]
fn embedded_adapters_materialize_idempotently_without_the_checkout() {
    use semaprax_harness::assets;
    let home = fixture_dir("hp-hn07").join("home");
    let a = assets::materialize(&home).unwrap();
    let b = assets::materialize(&home).unwrap();
    assert_eq!(a.digest, b.digest);
    assert_eq!(a.digest, assets::bundle_digest().unwrap());
    for (_, dir) in assets::PROVIDERS {
        let d = a.files_dir.join(dir).join("harness-provider.json");
        assert!(d.is_file(), "{}", d.display());
    }
    // Shared SDK helpers the adapters import by relative path travel along.
    assert!(a
        .files_dir
        .join("sdk/node/semaprax-harness-adapter.mjs")
        .is_file());
    assert!(a
        .files_dir
        .join("sdk/python/semaprax_harness_adapter.py")
        .is_file());
    // Tests, evidence and research are not shipped.
    assert!(!a.files_dir.join("graft/EVIDENCE.md").exists());
    assert!(!a.files_dir.join("rtk/RESEARCH.md").exists());
    assert!(!a.files_dir.join("graft/test").exists());
    assert!(!a.digest.is_empty() && a.files_dir.starts_with(home.join("artifacts")));
}
