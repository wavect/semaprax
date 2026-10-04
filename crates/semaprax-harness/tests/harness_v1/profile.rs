//! HP-02 profile tests (fixture prefix `hp-hp02`).

use crate::support::{fixture_dir, write};
use semaprax_harness::cli::{run, Environment};
use semaprax_harness::contract::CapabilityKind;
use semaprax_harness::profile::config::{self, Mode};
use semaprax_harness::profile::resolve::{current_platform, BindingState};
use semaprax_harness::profile::{self, check_grant_current, grant_for, lock, trust, LocalState};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const CTX: &str = "context.repository";
const CTX_KIND: CapabilityKind = CapabilityKind::ContextRepository;

fn s(args: &[&str]) -> Vec<String> {
    args.iter().map(|a| a.to_string()).collect()
}

fn env(home: &Path, cwd: &Path) -> Environment {
    let mut vars = BTreeMap::new();
    vars.insert("PATH".to_string(), "/usr/bin:/bin".to_string());
    Environment {
        harness_home: Some(home.to_path_buf()),
        compiler: None,
        cwd: cwd.to_path_buf(),
        vars,
    }
}

struct Prov {
    desc: PathBuf,
    upstream: PathBuf,
}

fn descriptor_text(id: &str, kinds: &[&str], versions: &[&str], read: &[&str]) -> String {
    let caps: Vec<Value> = kinds
        .iter()
        .map(|k| json!({"kind": k, "version": 1, "required": true, "operations": CapabilityKind::parse(k).unwrap().operations()}))
        .collect();
    let mut platforms = vec![current_platform()];
    platforms.extend(["macos-aarch64", "linux-x86_64"].map(String::from));
    json!({
        "schema": "semaprax.harness-provider.v1",
        "provider": {"id": id, "version": "0.1.0"},
        "adapter": {"runtime": "native", "entry": ["adapter.sh"], "version": "0.1.0"},
        "upstream": {"name": "tool", "package": "npm:tool", "repository": "https://example.invalid/tool",
                     "versions": versions, "identity_probe": ["--version"]},
        "protocol": {"name": "semaprax.harness-rpc.v1", "min": 1, "max": 1},
        "capabilities": caps,
        "platforms": platforms,
        "permissions": {"read": read, "write": [], "network": [], "process": ["upstream"], "secrets": []},
        "resources": {"handshake_timeout_ms": 5000, "invoke_timeout_ms": 30000, "max_frame_bytes": 1048576,
                      "max_concurrency": 1, "idle_shutdown_ms": 60000},
        "cancellation": "cooperative",
        "support": {"license": "MIT", "isolation": "subprocess", "tested": []}
    })
    .to_string()
}

/// Descriptor + adapter entry + a tiny upstream script that prints a version.
fn provision(
    root: &Path,
    name: &str,
    id: &str,
    kinds: &[&str],
    version: &str,
    log: Option<&Path>,
) -> Prov {
    let desc = write(
        root,
        &format!("{name}/descriptor.json"),
        &descriptor_text(id, kinds, &["1.0.0"], &["project"]),
    );
    write(root, &format!("{name}/adapter.sh"), "#!/bin/sh\nexit 0\n");
    let logline = log.map_or(String::new(), |l| {
        format!(
            "echo \"$PATH|$HOME|$(pwd)|${{LEAK:-unset}}\" >> \"{}\"\n",
            l.display()
        )
    });
    let upstream = write(
        root,
        &format!("{name}-bin/tool"),
        &format!("#!/bin/sh\n{logline}echo \"tool {version}\"\n"),
    );
    std::fs::set_permissions(&upstream, std::fs::Permissions::from_mode(0o755)).unwrap();
    Prov { desc, upstream }
}

fn ok(e: &Environment, args: &[&str]) -> String {
    let o = run(&s(args), e);
    assert_eq!(o.code, 0, "{args:?} failed: {}", o.stderr);
    o.stdout
}

fn adopt_trust(e: &Environment, p: &Prov, id: &str) {
    ok(
        e,
        &[
            "adopt",
            p.desc.to_str().unwrap(),
            "--upstream",
            p.upstream.to_str().unwrap(),
        ],
    );
    ok(e, &["trust", id]);
}

fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(base, &p, out);
                } else {
                    out.insert(
                        p.strip_prefix(base).unwrap().display().to_string(),
                        std::fs::read(&p).unwrap(),
                    );
                }
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

fn pin(project: &Path, mode: &str, id: &str) {
    write(
        project,
        "semaprax.harness.toml",
        &format!("schema = \"semaprax.harness-config.v1\"\n[capability.\"{CTX}\"]\nmode = \"{mode}\"\nprovider = \"{id}\"\n"),
    );
}

// ---- configuration parser -------------------------------------------------

#[test]
fn hp_hp02_config_valid_fixture_and_stable_digest() {
    let text = include_str!("../fixtures/profile/valid.toml");
    let c = config::parse(text.as_bytes()).unwrap();
    let ctx = &c.capabilities[&CTX_KIND];
    assert_eq!(
        (ctx.mode, ctx.provider.as_deref()),
        (Mode::Required, Some("org.example/ctx-a"))
    );
    assert_eq!(ctx.scope, ["src", "docs"]);
    assert_eq!(
        c.capabilities[&CapabilityKind::CommandView].mode,
        Mode::Auto
    );
    assert_eq!(c.inactive, ["x.example/notes"]);
    assert_eq!(
        (
            c.budget.context_max_bytes,
            c.skills.select.len(),
            c.skills.max_bytes
        ),
        (8192, 2, 4096)
    );
    // Comments, whitespace and table order do not change the digest.
    let reordered = "schema=\"semaprax.harness-config.v1\"\n[skills]\nmax_bytes=4096\nselect=[\"review\",\"plain\"]\n[budget]\ncontext_max_bytes=8192\n[capability.\"command.view\"]\nmode=\"auto\"\n[capability.\"x.example/notes\"]\n[capability.\"context.repository\"]\nscope=[\"src\",\"docs\"]\nprovider=\"org.example/ctx-a\"\nmode=\"required\"\n";
    assert_eq!(
        config::parse(reordered.as_bytes()).unwrap().digest(),
        c.digest()
    );
}

fn refused(text: &str) -> (String, String) {
    let e = config::parse(text.as_bytes()).unwrap_err();
    (e.code.to_string(), e.message)
}

const HEAD: &str = "schema = \"semaprax.harness-config.v1\"\n";

#[test]
fn hp_hp02_config_negative_cases() {
    let (c, m) = refused(&format!("{HEAD}[budget]\ncontext_max_byte = 5\n"));
    assert_eq!(c, "SPX-HPB003");
    assert!(
        m.contains(":3:") && m.contains("did you mean `context_max_bytes`"),
        "{m}"
    );
    let (c, m) = refused(&format!("{HEAD}[profil]\nenabled = true\n"));
    assert_eq!(c, "SPX-HPB003");
    assert!(
        m.contains(":2:") && m.contains("unknown table") && m.contains("profile"),
        "{m}"
    );
    let (c, m) = refused(&format!(
        "{HEAD}[profile]\nenabled = true\nenabled = false\n"
    ));
    assert_eq!(c, "SPX-HPB002");
    assert!(m.contains(":4:") && m.contains("first at line 3"), "{m}");
    assert_eq!(
        refused(&format!("{HEAD}[profile]\n[profile]\n")).0,
        "SPX-HPB002"
    );
    let (c, m) = refused(&format!("{HEAD}[profile]\nenabled = \"yes\"\n"));
    assert_eq!(c, "SPX-HPB004");
    assert!(m.contains(":3:"), "{m}");
    assert_eq!(
        refused(&format!("{HEAD}[budget]\ncontext_max_bytes = 0\n")).0,
        "SPX-HPB004"
    );
    assert_eq!(
        refused(&format!(
            "{HEAD}[capability.\"context.repository\"]\nmode = \"always\"\n"
        ))
        .0,
        "SPX-HPB004"
    );
    assert_eq!(
        refused(&format!(
            "{HEAD}[capability.\"context.repository\"]\nprovider = \"Not A Provider\"\n"
        ))
        .0,
        "SPX-HPB004"
    );
    let (c, m) = refused(&format!(
        "{HEAD}[capability.\"context.repositry\"]\nmode = \"auto\"\n"
    ));
    assert_eq!(c, "SPX-HPB006");
    assert!(m.contains("did you mean `context.repository`"), "{m}");
    assert_eq!(refused("[profile]\nenabled = true\n").0, "SPX-HPB005");
    assert_eq!(
        refused("schema = \"semaprax.harness-config.v2\"\n").0,
        "SPX-HPB005"
    );
    assert_eq!(refused(&format!("{HEAD}extra = 1\n")).0, "SPX-HPB003");
    assert_eq!(
        refused(&format!("{HEAD}[profile\nenabled = true\n")).0,
        "SPX-HPB001"
    );
    assert_eq!(
        refused(&format!("{HEAD}[profile]\nenabled = maybe\n")).0,
        "SPX-HPB001"
    );
    assert_eq!(
        refused(&format!("{HEAD}[skills]\nselect = [1]\n")).0,
        "SPX-HPB004"
    );
    assert_eq!(
        refused(&format!("{HEAD}[profile]\na.b = 1\n")).0,
        "SPX-HPB001"
    );
}

#[test]
fn hp_hp02_config_refuses_absolute_paths_and_secrets() {
    for v in [
        "/Users/kevin/tool",
        "~/bin/tool",
        "C:\\\\tools\\\\x",
        "\\\\\\\\host\\\\share",
    ] {
        let t = format!("{HEAD}[skills]\nselect = [\"{v}\"]\n");
        assert_eq!(refused(&t).0, "SPX-HPB007", "{v}");
    }
    for v in [
        "sk-live-abcdef0123456789",
        "ghp_abcdef",
        "Bearer abc",
        "https://u:pw@example.com/x",
        "api_key=abc",
        "AKIA0123456789ABCDEF",
    ] {
        let t = format!("{HEAD}[skills]\nselect = [\"{v}\"]\n");
        assert_eq!(refused(&t).0, "SPX-HPB007", "{v}");
    }
    let (c, m) = refused(&format!(
        "{HEAD}[capability.\"context.repository\"]\nscope = [\"/etc\"]\n"
    ));
    assert_eq!(c, "SPX-HPB007");
    assert!(m.contains(":3:"), "{m}");
}

// ---- first run, fallback, required ---------------------------------------

#[test]
fn hp_hp02_first_run_builtin_fallback_and_required_failure() {
    let home = fixture_dir("hp-hp02-home");
    let project = fixture_dir("hp-hp02-proj");
    let e = env(&home, &project);
    let out = ok(&e, &["resolve"]);
    assert!(
        out.contains("context.repository fallback semaprax/native-context"),
        "{out}"
    );
    let l = lock::load(&project).unwrap().unwrap();
    let b = |k: CapabilityKind| l.bindings.iter().find(|b| b.kind == k).unwrap().clone();
    assert_eq!(
        (b(CTX_KIND).provider_id.as_str(), b(CTX_KIND).state),
        ("semaprax/native-context", BindingState::Fallback)
    );
    assert_eq!(
        b(CapabilityKind::CommandView).provider_id,
        "semaprax/raw-command"
    );
    assert_eq!(
        b(CapabilityKind::DecisionEvaluate).provider_id,
        "semaprax/rules-decision"
    );
    assert_eq!(
        b(CapabilityKind::SkillCatalog).provider_id,
        "semaprax/plain-skills"
    );
    assert_eq!(
        b(CapabilityKind::ModelGenerate).state,
        BindingState::Unavailable
    );
    let status = ok(&e, &["status"]);
    assert!(
        status.contains("lock: matches") && status.contains("no external provider installed"),
        "{status}"
    );
    // A required external provider fails clearly and names the exact identity.
    pin(&project, "required", "org.example/ctx-a");
    let _ = std::fs::remove_file(project.join("semaprax.harness.lock"));
    let o = run(&s(&["resolve"]), &e);
    assert_eq!(o.code, 1);
    assert!(
        o.stderr.contains("SPX-HPB040") && o.stderr.contains("org.example/ctx-a"),
        "{}",
        o.stderr
    );
    assert!(!project.join("semaprax.harness.lock").exists());
    // Without a pin, required with nothing installed is also refused.
    write(
        &project,
        "semaprax.harness.toml",
        &format!("{HEAD}[capability.\"{CTX}\"]\nmode = \"required\"\n"),
    );
    assert!(run(&s(&["resolve"]), &e).stderr.contains("SPX-HPB041"));
    // Status still reports (exit 1) instead of crashing.
    assert_eq!(run(&s(&["status"]), &e).code, 1);
}

#[test]
fn hp_hp02_disabled_and_profile_off() {
    let home = fixture_dir("hp-hp02-home");
    let project = fixture_dir("hp-hp02-proj");
    let e = env(&home, &project);
    write(
        &project,
        "semaprax.harness.toml",
        &format!("{HEAD}[capability.\"{CTX}\"]\nmode = \"disabled\"\n"),
    );
    let r = profile::resolve_project(&e, &project).unwrap();
    assert_eq!(
        r.profile.binding(CTX_KIND).unwrap().state,
        BindingState::Disabled
    );
    write(
        &project,
        "semaprax.harness.toml",
        &format!("{HEAD}[profile]\nenabled = false\n"),
    );
    let r = profile::resolve_project(&e, &project).unwrap();
    assert!(r
        .profile
        .bindings
        .iter()
        .all(|b| b.state == BindingState::Disabled));
}

// ---- adopt, trust, reuse, per-project choice -----------------------------

#[test]
fn hp_hp02_adopt_trust_reuse_without_repeat_confirmation() {
    let home = fixture_dir("hp-hp02-home");
    let project = fixture_dir("hp-hp02-proj");
    let root = fixture_dir("hp-hp02-tools");
    let log = root.join("probe.log");
    let e = env(&home, &project);
    let p = provision(&root, "a", "org.example/ctx-a", &[CTX], "1.0.0", Some(&log));
    let out = ok(
        &e,
        &[
            "adopt",
            p.desc.to_str().unwrap(),
            "--upstream",
            p.upstream.to_str().unwrap(),
        ],
    );
    assert!(out.contains("not trusted yet"), "{out}");
    // Adoption ran exactly the explicit identity probe, with a scrubbed environment.
    let line = std::fs::read_to_string(&log).unwrap();
    assert_eq!(line.lines().count(), 1);
    let fields: Vec<&str> = line.trim().split('|').collect();
    assert_eq!(fields[0], "/usr/bin:/bin");
    assert!(
        fields[1].contains(home.file_name().unwrap().to_str().unwrap())
            && fields[1].contains("probe-"),
        "{line}"
    );
    assert_eq!(fields[3], "unset");
    // Untrusted: not selected, explained.
    assert!(ok(&e, &["status"]).contains("untrusted"));
    assert_eq!(
        profile::resolve_project(&e, &project)
            .unwrap()
            .profile
            .binding(CTX_KIND)
            .unwrap()
            .state,
        BindingState::Fallback
    );
    let out = ok(&e, &["trust", "org.example/ctx-a"]);
    assert!(
        out.contains("read=[project]") && out.contains("process=[upstream]"),
        "{out}"
    );
    let before = snapshot(&home);
    let r1 = profile::resolve_project(&e, &project).unwrap();
    let b = r1.profile.binding(CTX_KIND).unwrap();
    assert_eq!(
        (b.state, b.provider_id.as_str()),
        (BindingState::Selected, "org.example/ctx-a")
    );
    assert_eq!(b.upstream.as_ref().unwrap().version, "1.0.0");
    let launch = &r1.launches[&CTX_KIND];
    assert_eq!(
        launch.upstream_path.as_ref().unwrap(),
        &p.upstream.canonicalize().unwrap()
    );
    // Reuse: repeated resolution, status and grant checks neither probe, write, nor ask again.
    for _ in 0..3 {
        profile::resolve_project(&e, &project).unwrap();
        ok(&e, &["status", "--json"]);
        check_grant_current(&e, &launch.grant).unwrap();
    }
    assert_eq!(
        std::fs::read_to_string(&log).unwrap().lines().count(),
        1,
        "no further execution after adopt"
    );
    assert_eq!(
        snapshot(&home),
        before,
        "resolution is read-only on machine-local state"
    );
}

#[test]
fn hp_hp02_two_projects_choose_different_providers_without_global_changes() {
    let home = fixture_dir("hp-hp02-home");
    let root = fixture_dir("hp-hp02-tools");
    let (pa, pb) = (fixture_dir("hp-hp02-proj"), fixture_dir("hp-hp02-proj"));
    let e = env(&home, &pa);
    let a = provision(&root, "a", "org.example/ctx-a", &[CTX], "1.0.0", None);
    let b = provision(&root, "b", "org.example/ctx-b", &[CTX], "1.0.0", None);
    adopt_trust(&e, &a, "org.example/ctx-a");
    adopt_trust(&e, &b, "org.example/ctx-b");
    pin(&pa, "required", "org.example/ctx-a");
    pin(&pb, "required", "org.example/ctx-b");
    let before = snapshot(&home);
    let ra = profile::resolve_project(&e, &pa).unwrap();
    let rb = profile::resolve_project(&e, &pb).unwrap();
    assert_eq!(
        ra.profile.binding(CTX_KIND).unwrap().provider_id,
        "org.example/ctx-a"
    );
    assert_eq!(
        rb.profile.binding(CTX_KIND).unwrap().provider_id,
        "org.example/ctx-b"
    );
    ok(&env(&home, &pa), &["resolve"]);
    ok(&env(&home, &pb), &["resolve"]);
    assert_eq!(
        snapshot(&home),
        before,
        "choosing per project never writes machine-local or global state"
    );
    assert!(!home.join("preferences.json").exists());
    assert_ne!(
        std::fs::read(pa.join("semaprax.harness.lock")).unwrap(),
        std::fs::read(pb.join("semaprax.harness.lock")).unwrap()
    );
}

#[test]
fn hp_hp02_ambiguity_is_explained_not_guessed_and_preference_resolves_it() {
    let home = fixture_dir("hp-hp02-home");
    let root = fixture_dir("hp-hp02-tools");
    let project = fixture_dir("hp-hp02-proj");
    let e = env(&home, &project);
    // Adopt in reverse order: candidates must still be listed sorted.
    adopt_trust(
        &e,
        &provision(&root, "b", "org.example/ctx-b", &[CTX], "1.0.0", None),
        "org.example/ctx-b",
    );
    adopt_trust(
        &e,
        &provision(&root, "a", "org.example/ctx-a", &[CTX], "1.0.0", None),
        "org.example/ctx-a",
    );
    let r = profile::resolve_project(&e, &project).unwrap();
    let b = r.profile.binding(CTX_KIND).unwrap();
    assert_eq!(b.state, BindingState::Fallback);
    assert!(
        b.reason
            .contains("ambiguous: org.example/ctx-a, org.example/ctx-b"),
        "{}",
        b.reason
    );
    assert!(r.launches.is_empty());
    let status = ok(&e, &["status"]);
    assert!(status.contains("ambiguous"), "{status}");
    let explained = ok(&e, &["explain", CTX]);
    assert!(
        explained.contains("ambiguous between: org.example/ctx-a, org.example/ctx-b"),
        "{explained}"
    );
    write(
        &project,
        "semaprax.harness.toml",
        &format!("{HEAD}[capability.\"{CTX}\"]\nmode = \"required\"\n"),
    );
    assert!(profile::resolve_project(&e, &project).unwrap_err().code == "SPX-HPB042");
    // An approved user preference selects one deterministically.
    ok(&e, &["trust", "org.example/ctx-b", "--prefer", CTX]);
    let r = profile::resolve_project(&e, &project).unwrap();
    assert_eq!(
        r.profile.binding(CTX_KIND).unwrap().provider_id,
        "org.example/ctx-b"
    );
    assert_eq!(
        r.profile.binding(CTX_KIND).unwrap().reason,
        "user preference"
    );
    // The project pin outranks the preference.
    pin(&project, "auto", "org.example/ctx-a");
    assert_eq!(
        profile::resolve_project(&e, &project)
            .unwrap()
            .profile
            .binding(CTX_KIND)
            .unwrap()
            .provider_id,
        "org.example/ctx-a"
    );
}

// ---- adoption safety -----------------------------------------------------

#[test]
fn hp_hp02_workspace_fake_executable_is_refused_unless_explicitly_allowed() {
    let home = fixture_dir("hp-hp02-home");
    let project = fixture_dir("hp-hp02-proj");
    let root = fixture_dir("hp-hp02-tools");
    let marker = root.join("ran");
    let e = env(&home, &project);
    let p = provision(&root, "a", "org.example/ctx-a", &[CTX], "1.0.0", None);
    let fake = write(
        &project,
        "bin/tool",
        &format!(
            "#!/bin/sh\ntouch \"{}\"\necho \"tool 1.0.0\"\n",
            marker.display()
        ),
    );
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let o = run(
        &s(&[
            "adopt",
            p.desc.to_str().unwrap(),
            "--upstream",
            fake.to_str().unwrap(),
        ]),
        &e,
    );
    assert_eq!(o.code, 1);
    assert!(
        o.stderr.contains("SPX-HPB024") && o.stderr.contains("--allow-project-local"),
        "{}",
        o.stderr
    );
    assert!(!marker.exists(), "the workspace executable must not run");
    assert!(LocalState::load(&e).unwrap().installations.is_empty());
    // A relative path is refused; PATH is never searched.
    assert_eq!(
        run(
            &s(&["adopt", p.desc.to_str().unwrap(), "--upstream", "tool"]),
            &e
        )
        .code,
        1
    );
    // Only an explicit flag permits it.
    ok(
        &e,
        &[
            "adopt",
            p.desc.to_str().unwrap(),
            "--upstream",
            fake.to_str().unwrap(),
            "--allow-project-local",
        ],
    );
    assert!(marker.exists());
}

#[test]
fn hp_hp02_path_shadow_is_never_consulted() {
    let home = fixture_dir("hp-hp02-home");
    let project = fixture_dir("hp-hp02-proj");
    let root = fixture_dir("hp-hp02-tools");
    let shadow = fixture_dir("hp-hp02-shadow");
    let marker = root.join("shadow-ran");
    let g = write(
        &shadow,
        "tool",
        &format!(
            "#!/bin/sh\ntouch \"{}\"\necho \"tool 1.0.0\"\n",
            marker.display()
        ),
    );
    std::fs::set_permissions(&g, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut e = env(&home, &project);
    e.vars
        .insert("PATH".into(), format!("{}:/usr/bin:/bin", shadow.display()));
    let p = provision(&root, "a", "org.example/ctx-a", &[CTX], "1.0.0", None);
    // Adopt without --upstream: nothing is looked up or run; a recipe is printed.
    let out = ok(&e, &["adopt", p.desc.to_str().unwrap()]);
    assert!(
        out.contains("Nothing was installed or downloaded") && out.contains("--upstream"),
        "{out}"
    );
    // Trust is refused without an adopted upstream; the PATH match is not a substitute.
    assert!(run(&s(&["trust", "org.example/ctx-a"]), &e)
        .stderr
        .contains("SPX-HPB033"));
    pin(&project, "required", "org.example/ctx-a");
    let o = run(&s(&["resolve"]), &e);
    assert_eq!(o.code, 1);
    assert!(
        o.stderr.contains("SPX-HPB040") && o.stderr.contains("org.example/ctx-a"),
        "{}",
        o.stderr
    );
    assert_eq!(
        run(&s(&["status"]), &e).code,
        1,
        "unmet required capability"
    );
    assert!(!marker.exists(), "a PATH match must never be executed");
}

#[test]
fn hp_hp02_incompatible_upstream_is_detected_but_unusable() {
    let home = fixture_dir("hp-hp02-home");
    let project = fixture_dir("hp-hp02-proj");
    let root = fixture_dir("hp-hp02-tools");
    let e = env(&home, &project);
    let p = provision(&root, "a", "org.example/ctx-a", &[CTX], "9.9.9", None);
    let out = ok(
        &e,
        &[
            "adopt",
            p.desc.to_str().unwrap(),
            "--upstream",
            p.upstream.to_str().unwrap(),
        ],
    );
    assert!(
        out.contains("detected-but-incompatible") && out.contains("9.9.9"),
        "{out}"
    );
    let o = run(&s(&["trust", "org.example/ctx-a"]), &e);
    assert!(o.stderr.contains("SPX-HPB033"), "{}", o.stderr);
    assert!(ok(&e, &["status"]).contains("unsupported"));
    pin(&project, "required", "org.example/ctx-a");
    assert!(profile::resolve_project(&e, &project)
        .unwrap_err()
        .message
        .contains("incompatible"));
}

#[test]
fn hp_hp02_probe_is_bounded() {
    let home = fixture_dir("hp-hp02-home");
    let project = fixture_dir("hp-hp02-proj");
    let root = fixture_dir("hp-hp02-tools");
    let e = env(&home, &project);
    let p = provision(&root, "a", "org.example/ctx-a", &[CTX], "1.0.0", None);
    // Output flood: the version sits beyond the 64 KiB cap, so it is never read.
    std::fs::write(
        &p.upstream,
        "#!/bin/sh\nhead -c 200000 /dev/zero | tr '\\0' x\necho \"tool 1.0.0\"\n",
    )
    .unwrap();
    let out = ok(
        &e,
        &[
            "adopt",
            p.desc.to_str().unwrap(),
            "--upstream",
            p.upstream.to_str().unwrap(),
        ],
    );
    assert!(out.contains("no version"), "{out}");
    // A hung probe is killed at the deadline.
    std::fs::write(&p.upstream, "#!/bin/sh\nexec sleep 30\n").unwrap();
    let t = std::time::Instant::now();
    let out = ok(
        &e,
        &[
            "adopt",
            p.desc.to_str().unwrap(),
            "--upstream",
            p.upstream.to_str().unwrap(),
        ],
    );
    assert!(out.contains("exceeded 5s"), "{out}");
    assert!(t.elapsed() < std::time::Duration::from_secs(15));
}

// ---- trust bound to digests ----------------------------------------------

#[test]
fn hp_hp02_changed_binary_invalidates_grant() {
    let home = fixture_dir("hp-hp02-home");
    let project = fixture_dir("hp-hp02-proj");
    let root = fixture_dir("hp-hp02-tools");
    let e = env(&home, &project);
    let p = provision(&root, "a", "org.example/ctx-a", &[CTX], "1.0.0", None);
    adopt_trust(&e, &p, "org.example/ctx-a");
    pin(&project, "auto", "org.example/ctx-a");
    let grant = profile::resolve_project(&e, &project).unwrap().launches[&CTX_KIND]
        .grant
        .clone();
    check_grant_current(&e, &grant).unwrap();
    let mut body = std::fs::read(&p.upstream).unwrap();
    body.extend_from_slice(b"# tampered\n");
    std::fs::write(&p.upstream, body).unwrap();
    let state = LocalState::load(&e).unwrap();
    let cur = state.installations["org.example/ctx-a"]
        .inspect()
        .unwrap()
        .current;
    assert_eq!(
        grant_for(&state, "org.example/ctx-a", &cur)
            .unwrap_err()
            .code,
        "SPX-HPB031"
    );
    assert_eq!(
        check_grant_current(&e, &grant).unwrap_err().code,
        "SPX-HPB034"
    );
    let r = profile::resolve_project(&e, &project).unwrap();
    assert_eq!(
        r.profile.binding(CTX_KIND).unwrap().state,
        BindingState::Fallback
    );
    assert!(r.launches.is_empty());
    assert!(
        r.profile.candidates[0].detail.contains("changed"),
        "{:?}",
        r.profile.candidates
    );
    assert!(
        run(&s(&["trust", "org.example/ctx-a"]), &e)
            .stderr
            .contains("SPX-HPB031"),
        "trust refuses a changed binary before re-adoption"
    );
}

#[test]
fn hp_hp02_permission_widening_is_refused_until_trusted_again() {
    let home = fixture_dir("hp-hp02-home");
    let project = fixture_dir("hp-hp02-proj");
    let root = fixture_dir("hp-hp02-tools");
    let e = env(&home, &project);
    let p = provision(&root, "a", "org.example/ctx-a", &[CTX], "1.0.0", None);
    adopt_trust(&e, &p, "org.example/ctx-a");
    pin(&project, "auto", "org.example/ctx-a");
    let grant = profile::resolve_project(&e, &project).unwrap().launches[&CTX_KIND]
        .grant
        .clone();
    assert_eq!(grant.permissions().read, ["project"]);
    // The adapter now asks for more.
    write(
        &root,
        "a/descriptor.json",
        &descriptor_text(
            "org.example/ctx-a",
            &[CTX],
            &["1.0.0"],
            &["project", "home"],
        ),
    );
    assert_eq!(
        profile::resolve_project(&e, &project)
            .unwrap()
            .profile
            .binding(CTX_KIND)
            .unwrap()
            .state,
        BindingState::Fallback
    );
    assert_eq!(
        check_grant_current(&e, &grant).unwrap_err().code,
        "SPX-HPB034"
    );
    assert_eq!(
        run(&s(&["trust", "org.example/ctx-a"]), &e).code,
        1,
        "must re-adopt before trusting"
    );
    ok(
        &e,
        &[
            "adopt",
            p.desc.to_str().unwrap(),
            "--upstream",
            p.upstream.to_str().unwrap(),
        ],
    );
    let state = LocalState::load(&e).unwrap();
    let cur = state.installations["org.example/ctx-a"]
        .inspect()
        .unwrap()
        .current;
    assert_eq!(
        grant_for(&state, "org.example/ctx-a", &cur)
            .unwrap_err()
            .code,
        "SPX-HPB031"
    );
    // Only an explicit new trust command widens the grant.
    ok(&e, &["trust", "org.example/ctx-a"]);
    let wide = profile::resolve_project(&e, &project).unwrap().launches[&CTX_KIND]
        .grant
        .clone();
    assert_eq!(wide.permissions().read, ["project", "home"]);
    assert!(
        check_grant_current(&e, &grant).is_err(),
        "the old narrow grant is stale"
    );
}

#[test]
fn hp_hp02_revocation_blocks_the_next_dispatch() {
    let home = fixture_dir("hp-hp02-home");
    let project = fixture_dir("hp-hp02-proj");
    let root = fixture_dir("hp-hp02-tools");
    let e = env(&home, &project);
    let p = provision(&root, "a", "org.example/ctx-a", &[CTX], "1.0.0", None);
    adopt_trust(&e, &p, "org.example/ctx-a");
    let grant = profile::resolve_project(&e, &project).unwrap().launches[&CTX_KIND]
        .grant
        .clone();
    check_grant_current(&e, &grant).unwrap();
    assert!(ok(&e, &["revoke", "org.example/ctx-a"]).contains("revoked"));
    assert_eq!(
        check_grant_current(&e, &grant).unwrap_err().code,
        "SPX-HPB034"
    );
    let state = LocalState::load(&e).unwrap();
    let cur = state.installations["org.example/ctx-a"]
        .inspect()
        .unwrap()
        .current;
    assert_eq!(
        grant_for(&state, "org.example/ctx-a", &cur)
            .unwrap_err()
            .code,
        "SPX-HPB030"
    );
    assert_eq!(
        profile::resolve_project(&e, &project)
            .unwrap()
            .profile
            .binding(CTX_KIND)
            .unwrap()
            .state,
        BindingState::Fallback
    );
    assert!(ok(&e, &["revoke", "org.example/ctx-a"]).contains("was not trusted"));
}

// ---- frozen lock ---------------------------------------------------------

#[test]
fn hp_hp02_frozen_second_machine_and_missing_provider() {
    let project = fixture_dir("hp-hp02-proj");
    pin(&project, "required", "org.example/ctx-a");
    // Machine one.
    let (home1, root1) = (fixture_dir("hp-hp02-home"), fixture_dir("hp-hp02-tools"));
    let e1 = env(&home1, &project);
    let p1 = provision(&root1, "a", "org.example/ctx-a", &[CTX], "1.0.0", None);
    adopt_trust(&e1, &p1, "org.example/ctx-a");
    ok(&e1, &["resolve"]);
    let lock_bytes = std::fs::read(project.join("semaprax.harness.lock")).unwrap();
    ok(&e1, &["resolve", "--frozen"]);
    // Machine two: same provider bytes at different absolute paths.
    let (home2, root2) = (
        fixture_dir("hp-hp02-home"),
        fixture_dir("hp-hp02-tools-other"),
    );
    let e2 = env(&home2, &project);
    let p2 = provision(
        &root2,
        "elsewhere",
        "org.example/ctx-a",
        &[CTX],
        "1.0.0",
        None,
    );
    assert_ne!(p1.desc, p2.desc);
    adopt_trust(&e2, &p2, "org.example/ctx-a");
    ok(&e2, &["resolve", "--frozen"]);
    assert_eq!(
        std::fs::read(project.join("semaprax.harness.lock")).unwrap(),
        lock_bytes
    );
    ok(&e2, &["resolve"]);
    assert_eq!(
        std::fs::read(project.join("semaprax.harness.lock")).unwrap(),
        lock_bytes,
        "identical lock bytes on both machines"
    );
    // Machine three: provider missing -> refusal naming it, never another provider.
    let e3 = env(&fixture_dir("hp-hp02-home"), &project);
    let o = run(&s(&["resolve", "--frozen"]), &e3);
    assert_eq!(o.code, 1);
    assert!(o.stderr.contains("org.example/ctx-a"), "{}", o.stderr);
    assert_eq!(
        std::fs::read(project.join("semaprax.harness.lock")).unwrap(),
        lock_bytes
    );
}

#[test]
fn hp_hp02_frozen_never_substitutes_another_provider() {
    let project = fixture_dir("hp-hp02-proj");
    let (home1, root1) = (fixture_dir("hp-hp02-home"), fixture_dir("hp-hp02-tools"));
    let e1 = env(&home1, &project);
    adopt_trust(
        &e1,
        &provision(&root1, "a", "org.example/ctx-a", &[CTX], "1.0.0", None),
        "org.example/ctx-a",
    );
    ok(&e1, &["resolve"]); // auto: single trusted installation
                           // Another machine only has ctx-b (auto would pick it): frozen must refuse and name ctx-a.
    let (home2, root2) = (fixture_dir("hp-hp02-home"), fixture_dir("hp-hp02-tools"));
    let e2 = env(&home2, &project);
    adopt_trust(
        &e2,
        &provision(&root2, "b", "org.example/ctx-b", &[CTX], "1.0.0", None),
        "org.example/ctx-b",
    );
    let o = run(&s(&["resolve", "--frozen"]), &e2);
    assert_eq!(o.code, 1);
    assert!(
        o.stderr.contains("SPX-HPB014")
            && o.stderr.contains("org.example/ctx-a")
            && o.stderr.contains("ctx-b"),
        "{}",
        o.stderr
    );
    // A version drift of the adopted provider is refused too.
    write(
        &project,
        "semaprax.harness.toml",
        &format!("{HEAD}[budget]\ncontext_max_bytes = 100\n"),
    );
    assert!(run(&s(&["resolve", "--frozen"]), &e1)
        .stderr
        .contains("SPX-HPB013"));
    // And a missing lock.
    std::fs::remove_file(project.join("semaprax.harness.lock")).unwrap();
    assert!(run(&s(&["resolve", "--frozen"]), &e1)
        .stderr
        .contains("SPX-HPB012"));
}

#[test]
fn hp_hp02_committed_files_hold_no_paths_or_secrets() {
    let home = fixture_dir("hp-hp02-home");
    let project = fixture_dir("hp-hp02-proj");
    let root = fixture_dir("hp-hp02-tools");
    let e = env(&home, &project);
    let p = provision(&root, "a", "org.example/ctx-a", &[CTX], "1.0.0", None);
    adopt_trust(&e, &p, "org.example/ctx-a");
    pin(&project, "required", "org.example/ctx-a");
    ok(&e, &["resolve"]);
    let lock_text = std::fs::read_to_string(project.join("semaprax.harness.lock")).unwrap();
    let cfg_text = std::fs::read_to_string(project.join("semaprax.harness.toml")).unwrap();
    for text in [&lock_text, &cfg_text] {
        for needle in [
            root.to_str().unwrap(),
            home.to_str().unwrap(),
            root.canonicalize().unwrap().to_str().unwrap(),
            "/Users/",
            "/private/",
            "/var/",
            "/tmp/",
            "granted",
            "trust",
        ] {
            assert!(
                !text.contains(needle),
                "committed file leaks `{needle}`: {text}"
            );
        }
        for tok in text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '/' || c == '.')) {
            assert!(!tok.starts_with('/'), "absolute path-like token `{tok}`");
        }
    }
    let parsed: Value = serde_json::from_str(&lock_text).unwrap();
    assert_eq!(parsed["schema"], "semaprax.harness-lock.v1");
    let ctx = parsed["bindings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["kind"] == CTX)
        .unwrap();
    assert_eq!(ctx["provider_id"], "org.example/ctx-a");
    assert_eq!(ctx["upstream"]["version"], "1.0.0");
    assert!(ctx["upstream"]["digest"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
    // Lock parse round-trips.
    assert_eq!(lock::load(&project).unwrap().unwrap().bindings.len(), 5);
    assert!(lock::parse(b"{\"schema\":\"semaprax.harness-lock.v1\",\"config_digest\":\"x\",\"bindings\":[],\"grants\":[]}").is_err());
    // The machine-local state, by contrast, does hold paths.
    assert!(std::fs::read_to_string(home.join("installations.json"))
        .unwrap()
        .contains("descriptor_path"));
}

// ---- verbs ---------------------------------------------------------------

#[test]
fn hp_hp02_status_json_explain_inspect_and_usage() {
    let home = fixture_dir("hp-hp02-home");
    let project = fixture_dir("hp-hp02-proj");
    let root = fixture_dir("hp-hp02-tools");
    let e = env(&home, &project);
    let p = provision(&root, "a", "org.example/ctx-a", &[CTX], "1.0.0", None);
    adopt_trust(&e, &p, "org.example/ctx-a");
    let doc: Value = serde_json::from_str(&ok(&e, &["status", "--json"])).unwrap();
    assert_eq!(doc["schema"], "semaprax.harness-status.v1");
    let ctx = doc["bindings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["kind"] == CTX)
        .unwrap();
    assert_eq!(
        (ctx["state"].as_str(), ctx["provider_id"].as_str()),
        (Some("selected"), Some("org.example/ctx-a"))
    );
    let ex = ok(&e, &["explain", CTX]);
    assert!(
        ex.contains("single compatible trusted installation") && ex.contains("precedence:"),
        "{ex}"
    );
    let ins = ok(&e, &["inspect", "org.example/ctx-a"]);
    for want in [
        "requested permissions: read=[project]",
        "granted permissions: read=[project]",
        "descriptor digest: sha256:",
        "upstream digest: sha256:",
        "compatibility and trust: trusted",
    ] {
        assert!(ins.contains(want), "{want} in {ins}");
    }
    assert!(ok(&e, &["inspect", "semaprax/native-context"]).contains("builtin"));
    assert!(run(&s(&["inspect", "no/such"]), &e)
        .stderr
        .contains("SPX-HPB023"));
    assert_eq!(run(&s(&["explain", "nonsense"]), &e).code, 2);
    assert_eq!(run(&s(&["status", "--bogus"]), &e).code, 2);
    assert!(run(&s(&["adopt", "builtin.json"]), &e).code != 0);
    // --project selects another directory.
    let other = fixture_dir("hp-hp02-proj");
    write(
        &other,
        "semaprax.harness.toml",
        &format!("{HEAD}[profile]\nenabled = false\n"),
    );
    assert!(ok(&e, &["status", "--project", other.to_str().unwrap()]).contains("disabled"));
}

#[test]
fn hp_hp02_malformed_machine_state_is_refused_not_ignored() {
    let home = fixture_dir("hp-hp02-home");
    let project = fixture_dir("hp-hp02-proj");
    let e = env(&home, &project);
    write(
        &home,
        "trust.json",
        "{\"schema\":\"semaprax.harness-trust.v1\",\"trust\":{\"a/b\":{}}}",
    );
    let o = run(&s(&["status"]), &e);
    assert_eq!(o.code, 1);
    assert!(o.stderr.contains("SPX-HPB020"), "{}", o.stderr);
    // No harness home at all: reading is a first-run state, writing is refused.
    let mut no_home = env(&home, &project);
    no_home.harness_home = None;
    assert_eq!(run(&s(&["status"]), &no_home).code, 0);
    assert!(run(&s(&["adopt", "x.json"]), &no_home)
        .stderr
        .contains("SPX-HPB020"));
}

#[test]
fn hp_hp02_trust_store_issues_only_through_grant_for() {
    // The only `Grant::issue` call site is trust::grant_for.
    let mut hits = Vec::new();
    for entry in std::fs::read_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/profile"))
        .unwrap()
        .flatten()
    {
        let text = std::fs::read_to_string(entry.path()).unwrap();
        if text.matches("Grant::issue(").count() > 0 {
            hits.push(entry.file_name().to_string_lossy().to_string());
        }
    }
    assert_eq!(hits, ["trust.rs"]);
    let _ = trust::TRUST_SCHEMA;
}

#[test]
fn hp_hp02_bundled_local_upstream_is_trusted_without_an_upstream_executable() {
    let home = crate::support::fixture_dir("hp-hp02-bundled-home");
    let project = crate::support::fixture_dir("hp-hp02-bundled-project");
    let e = env(&home, &project);
    let desc = crate::support::repo_root().join(
        "packages/semaprax-harness-adapters/examples/output-view-python/harness-provider.json",
    );
    ok(&e, &["adopt", desc.to_str().unwrap()]);
    ok(&e, &["trust", "org.example/output-view"]);
    let status = ok(
        &e,
        &["status", "--project", project.to_str().unwrap(), "--json"],
    );
    assert!(status.contains("org.example/output-view"), "{status}");
}

// ---- hpwire: machine-local runtimes, skill roots, workflow/model config ----

#[test]
fn hp_hpwire_adopt_runtime_is_recorded_machine_local_and_resolved() {
    let root = fixture_dir("hp-hpwire-rt").canonicalize().unwrap();
    let (home, project) = (root.join("home"), root.join("project"));
    std::fs::create_dir_all(&project).unwrap();
    let e = env(&home, &project);
    let p = provision(&root, "ctx", "org.example/ctx-rt", &[CTX], "1.0.0", None);
    let rt = write(&root, "rt/node", "#!/bin/sh\nexit 0\n");
    std::fs::set_permissions(&rt, std::fs::Permissions::from_mode(0o755)).unwrap();
    let out = ok(
        &e,
        &[
            "adopt",
            p.desc.to_str().unwrap(),
            "--upstream",
            p.upstream.to_str().unwrap(),
            "--runtime",
            rt.to_str().unwrap(),
            "--project",
            project.to_str().unwrap(),
        ],
    );
    assert!(out.contains("runtime "), "{out}");
    ok(&e, &["trust", "org.example/ctx-rt"]);
    let state = LocalState::load(&e).unwrap();
    assert_eq!(
        state.installations["org.example/ctx-rt"].runtime.as_deref(),
        Some(rt.canonicalize().unwrap().as_path())
    );
    // Re-adopting without --runtime keeps the recorded choice.
    ok(
        &e,
        &[
            "adopt",
            p.desc.to_str().unwrap(),
            "--upstream",
            p.upstream.to_str().unwrap(),
        ],
    );
    let state = LocalState::load(&e).unwrap();
    assert!(state.installations["org.example/ctx-rt"].runtime.is_some());
    // Relative and project-local runtimes are refused.
    let o = run(
        &s(&["adopt", p.desc.to_str().unwrap(), "--runtime", "node"]),
        &e,
    );
    assert_eq!(o.code, 1, "{}", o.stderr);
    let local = write(&project, "bin/node", "#!/bin/sh\n");
    let o = run(
        &s(&[
            "adopt",
            p.desc.to_str().unwrap(),
            "--runtime",
            local.to_str().unwrap(),
            "--project",
            project.to_str().unwrap(),
        ]),
        &e,
    );
    assert!(o.stderr.contains("SPX-HPB024"), "{}", o.stderr);
    // The resolution carries it to launch consumers.
    write(
        &project,
        "semaprax.harness.toml",
        "schema = \"semaprax.harness-config.v1\"\n",
    );
    let res = profile::resolve_project(&e, &project).unwrap();
    assert_eq!(
        res.launches[&CTX_KIND].runtime,
        state.installations["org.example/ctx-rt"].runtime
    );
}

#[test]
fn hp_hpwire_adopt_skills_root_is_machine_local_and_never_from_the_project() {
    let root = fixture_dir("hp-hpwire-sk").canonicalize().unwrap();
    let (home, project) = (root.join("home"), root.join("project"));
    std::fs::create_dir_all(&project).unwrap();
    let e = env(&home, &project);
    let skills = root.join("skills");
    std::fs::create_dir_all(&skills).unwrap();
    ok(
        &e,
        &[
            "adopt",
            "--skills",
            skills.to_str().unwrap(),
            "--origin",
            "team-skills",
            "--project",
            project.to_str().unwrap(),
        ],
    );
    let st = LocalState::load(&e).unwrap();
    assert_eq!(st.skill_roots.len(), 1);
    assert_eq!(st.skill_roots[0].origin, "team-skills");
    let inside = project.join("skills");
    std::fs::create_dir_all(&inside).unwrap();
    let o = run(
        &s(&[
            "adopt",
            "--skills",
            inside.to_str().unwrap(),
            "--project",
            project.to_str().unwrap(),
        ]),
        &e,
    );
    assert!(o.stderr.contains("SPX-HPB024"), "{}", o.stderr);
    let o = run(&s(&["adopt", "--skills", "relative"]), &e);
    assert_eq!(o.code, 1);
    assert_eq!(LocalState::load(&e).unwrap().skill_roots.len(), 1);
}

#[test]
fn hp_hpwire_config_workflow_checks_and_model_policy() {
    let text = "schema = \"semaprax.harness-config.v1\"\n[workflow.check.unit]\nargv = [\"cargo\", \"test\", \"-q\"]\n[model]\nlocal_only = true\nstrict_one_attempt = true\nlogical = \"local-strong\"\n";
    let c = config::parse(text.as_bytes()).unwrap();
    assert_eq!(c.workflow.checks["unit"], ["cargo", "test", "-q"]);
    assert!(c.model.local_only && c.model.strict_one_attempt);
    assert_eq!(c.model.logical.as_deref(), Some("local-strong"));
    // A default configuration keeps its pre-existing digest (new members are additive).
    let base = config::parse(b"schema = \"semaprax.harness-config.v1\"\n").unwrap();
    assert!(base.to_json().get("workflow").is_none() && base.to_json().get("model").is_none());
    assert_ne!(base.digest(), c.digest());
    for (bad, code) in [
        (
            "[workflow.check.x]\nargv = [\"/usr/bin/git\"]\n",
            "SPX-HPB007",
        ),
        ("[workflow.check.x]\n", "SPX-HPB004"),
        ("[workflow.check.x]\nargv = []\n", "SPX-HPB004"),
        (
            "[workflow.check.x]\nargv = [\"a\"]\nshell = \"x\"\n",
            "SPX-HPB003",
        ),
        ("[model]\nlocal_only = \"yes\"\n", "SPX-HPB004"),
    ] {
        let (c, m) = refused(&format!("schema = \"semaprax.harness-config.v1\"\n{bad}"));
        assert_eq!(c, code, "{bad}: {m}");
    }
}

#[test]
fn hp_hpwire_environment_forwards_only_declared_credential_and_marker_vars() {
    use semaprax_harness::endpoint::{AttemptOwnership, Catalog, EndpointKind, EndpointRecord};
    let root = fixture_dir("hp-hpwire-env").canonicalize().unwrap();
    let home = root.join("home");
    let mut cat = Catalog::default();
    cat.endpoints.insert(
        "gw".into(),
        EndpointRecord {
            id: "gw".into(),
            kind: EndpointKind::LiteLlm,
            url: "http://127.0.0.1:4000".into(),
            credential_env: Some("HPWIRE_CRED_DECLARED".into()),
            probe_model: "m".into(),
            returned_model: None,
            models: vec![],
            probes: BTreeMap::new(),
            ownership: AttemptOwnership::direct(),
            destinations: vec![],
            disclosed: true,
        },
    );
    cat.save(&home).unwrap();
    // Process-wide environment: unique names, restored below.
    let set = [
        ("SEMAPRAX_HARNESS_HOME", home.to_str().unwrap()),
        ("HPWIRE_CRED_DECLARED", "declared-value"),
        ("HPWIRE_CRED_UNDECLARED", "must-not-leak"),
        ("SEMAPRAX_HARNESS_BRIDGE_DEPTH", "2"),
        ("SEMAPRAX_HARNESS_COMMAND_VIEW_LINEAGE", "exec:abc"),
        ("SEMAPRAX_HARNESS_EXTERNAL_VIEW_OWNER", "rtk-hook"),
    ];
    for (k, v) in set {
        std::env::set_var(k, v);
    }
    let e = Environment::from_process();
    for (k, _) in set {
        std::env::remove_var(k);
    }
    assert_eq!(
        e.vars.get("HPWIRE_CRED_DECLARED").map(String::as_str),
        Some("declared-value")
    );
    assert!(
        !e.vars.contains_key("HPWIRE_CRED_UNDECLARED"),
        "{:?}",
        e.vars.keys()
    );
    assert_eq!(e.vars["SEMAPRAX_HARNESS_BRIDGE_DEPTH"], "2");
    assert_eq!(e.vars["SEMAPRAX_HARNESS_COMMAND_VIEW_LINEAGE"], "exec:abc");
    assert_eq!(e.vars["SEMAPRAX_HARNESS_EXTERNAL_VIEW_OWNER"], "rtk-hook");
    assert!(!e.vars.contains_key("SEMAPRAX_HARNESS_HOME"));
}

// ---- HN-19: artifact-v2 closure identity bound to adoption, trust and grants ----

#[test]
fn hp_hn19_adoption_binds_the_adapter_closure_and_helper_edits_refuse_the_old_grant() {
    let home = fixture_dir("hp-hn19-home");
    let project = fixture_dir("hp-hn19-proj");
    let root = fixture_dir("hp-hn19-tools");
    let e = env(&home, &project);
    let p = provision(&root, "a", "org.example/ctx-a", &[CTX], "1.0.0", None);
    adopt_trust(&e, &p, "org.example/ctx-a");
    pin(&project, "auto", "org.example/ctx-a");
    let st = LocalState::load(&e).unwrap();
    let rec = st.installations["org.example/ctx-a"]
        .entry_digest
        .clone()
        .unwrap();
    assert!(rec.starts_with("artifact-v2:sha256:"), "{rec}");
    let grant = profile::resolve_project(&e, &project).unwrap().launches[&CTX_KIND]
        .grant
        .clone();
    assert_eq!(grant.entry_digest(), Some(rec.as_str()));
    check_grant_current(&e, &grant).unwrap();
    // A helper module next to the entry changes the closure, not the entry.
    write(&root, "a/helper.py", "VALUE = 1\n");
    let st = LocalState::load(&e).unwrap();
    let cur = st.installations["org.example/ctx-a"]
        .inspect()
        .unwrap()
        .current;
    let err = grant_for(&st, "org.example/ctx-a", &cur).unwrap_err();
    assert_eq!(err.code, "SPX-HPB031");
    assert!(err.message.contains("artifact closure"), "{}", err.message);
    assert_eq!(
        check_grant_current(&e, &grant).unwrap_err().code,
        "SPX-HPB034"
    );
    assert!(profile::resolve_project(&e, &project)
        .unwrap()
        .launches
        .is_empty());
    // Re-adoption alone does not restore trust: the user must trust the new closure.
    ok(
        &e,
        &[
            "adopt",
            p.desc.to_str().unwrap(),
            "--upstream",
            p.upstream.to_str().unwrap(),
        ],
    );
    assert!(run(&s(&["trust", "org.example/ctx-a"]), &e).code == 0);
    let fresh = profile::resolve_project(&e, &project).unwrap().launches[&CTX_KIND]
        .grant
        .clone();
    assert_ne!(fresh.entry_digest(), grant.entry_digest());
}

#[test]
fn hp_hn19_closure_rules_exclude_only_what_is_declared_and_never_the_entry() {
    let home = fixture_dir("hp-hn19-rules-home");
    let project = fixture_dir("hp-hn19-rules-proj");
    let root = fixture_dir("hp-hn19-rules-tools");
    let e = env(&home, &project);
    let p = provision(&root, "a", "org.example/ctx-a", &[CTX], "1.0.0", None);
    write(&root, "a/notes/readme.txt", "v1");
    write(&root, "a/__pycache__/x.pyc", "cache");
    write(
        &root,
        "a/harness-closure.json",
        r#"{"schema":"semaprax.harness-closure.v1","exclude":["notes/"]}"#,
    );
    adopt_trust(&e, &p, "org.example/ctx-a");
    pin(&project, "auto", "org.example/ctx-a");
    let grant = profile::resolve_project(&e, &project).unwrap().launches[&CTX_KIND]
        .grant
        .clone();
    // Excluded notes and python caches do not change identity.
    write(&root, "a/notes/readme.txt", "v2");
    write(&root, "a/__pycache__/x.pyc", "other cache");
    check_grant_current(&e, &grant).unwrap();
    // Editing the rules themselves does.
    write(
        &root,
        "a/harness-closure.json",
        r#"{"schema":"semaprax.harness-closure.v1","exclude":["notes/","tests/"]}"#,
    );
    assert!(check_grant_current(&e, &grant).is_err());
    // Excluding the entry is refused at adoption.
    write(
        &root,
        "a/harness-closure.json",
        r#"{"schema":"semaprax.harness-closure.v1","exclude":["adapter.sh"]}"#,
    );
    let o = run(
        &s(&[
            "adopt",
            p.desc.to_str().unwrap(),
            "--upstream",
            p.upstream.to_str().unwrap(),
        ]),
        &e,
    );
    assert_ne!(o.code, 0);
    assert!(
        o.stderr.contains("SPX-HPB021") && o.stderr.contains("cannot exclude"),
        "{}",
        o.stderr
    );
}

#[test]
fn hp_hn19_legacy_entry_only_installations_stay_readable_and_are_labelled_legacy() {
    let home = fixture_dir("hp-hn19-legacy-home");
    let project = fixture_dir("hp-hn19-legacy-proj");
    let root = fixture_dir("hp-hn19-legacy-tools");
    let e = env(&home, &project);
    let p = provision(&root, "a", "org.example/ctx-a", &[CTX], "1.0.0", None);
    adopt_trust(&e, &p, "org.example/ctx-a");
    // Rewrite the machine-local state as a pre-v2 build recorded it: plain entry digest.
    let plain = profile::installations::file_digest(&root.join("a/adapter.sh")).unwrap();
    let v2 = LocalState::load(&e).unwrap().installations["org.example/ctx-a"]
        .entry_digest
        .clone()
        .unwrap();
    for f in ["installations.json", "trust.json"] {
        let text = std::fs::read_to_string(home.join(f)).unwrap();
        std::fs::write(home.join(f), text.replace(&v2, &plain)).unwrap();
    }
    let st = LocalState::load(&e).unwrap();
    let cur = st.installations["org.example/ctx-a"]
        .inspect()
        .unwrap()
        .current;
    assert_eq!(
        cur.entry_digest.as_deref(),
        Some(plain.as_str()),
        "legacy-v1 interpretation preserved"
    );
    assert!(grant_for(&st, "org.example/ctx-a", &cur).is_ok());
    // legacy-v1 does not bind helpers; this is exactly the gap v2 closes.
    write(&root, "a/helper.py", "x");
    let cur = st.installations["org.example/ctx-a"]
        .inspect()
        .unwrap()
        .current;
    assert!(grant_for(&st, "org.example/ctx-a", &cur).is_ok());
}
