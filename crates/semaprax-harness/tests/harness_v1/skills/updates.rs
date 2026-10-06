//! HN-05: curated skill and adapter updates through the fixture fetcher.
//! Fixture prefix `hp-hn05`.

use crate::support::*;
use semaprax_harness::cli::Environment;
use semaprax_harness::skills::inventory::{self, Bounds, ScanRules};
use semaprax_harness::skills::official::OfficialSet;
use semaprax_harness::updates::fixture::{fake_sha, FixtureFile};
use semaprax_harness::updates::ops::{self, Ctx, Report};
use semaprax_harness::updates::resolve::{resolve, Channel};
use semaprax_harness::updates::sha1::git_blob_sha;
use semaprax_harness::updates::state::{Kind, Source, State};
use semaprax_harness::updates::{cli_updates, effective_set_from, session_pin, MemoryFetcher};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const REPO: &str = "https://github.com/acme/demo-skill";
const AREPO: &str = "https://github.com/acme/demo-adapter";

fn skill_md(name: &str, extra: &str, body: &str) -> String {
    format!("---\nname: {name}\ndescription: Demo skill\nlicense: MIT\n{extra}---\n{body}\n")
}

const LICENSE: &str = "MIT License\nacme\n";

struct Fx {
    home: PathBuf,
    up: MemoryFetcher,
    set: OfficialSet,
    c1: String,
}

fn catalog(c1: &str, md: &str) -> (OfficialSet, ()) {
    let tmp = fixture_dir("hp-hn05-cat");
    write(&tmp, "SKILL.md", md);
    write(&tmp, "LICENSE", LICENSE);
    let digest = inventory::scan(&tmp, &ScanRules::skill(), &Bounds::SKILL)
        .unwrap()
        .digest();
    let file = |p: &str, up: &str, c: &str| {
        serde_json::json!({"path": p, "upstream_path": up, "git_blob_sha": git_blob_sha(c.as_bytes()),
            "sha256": semaprax_harness::json::sha256_plain(c.as_bytes()).trim_start_matches("sha256:"),
            "bytes": c.len()})
    };
    let doc = serde_json::json!({
        "schema": "semaprax.curated-skill-catalog.v1", "presets": {},
        "skills": [{"id": "demo", "aliases": ["demo"], "authorship": "upstream-authored",
            "source": {"repo": REPO, "subpath": "skills/demo", "channel": "latest-stable", "head_branch": "main",
                "tag": "v1.0.0", "commit": c1},
            "version": "v1.0.0", "license": "MIT", "license_file": "LICENSE", "bundle_digest": digest,
            "files": [file("SKILL.md", "skills/demo/SKILL.md", md), file("LICENSE", "LICENSE", LICENSE)],
            "compatibility": {"status": "primary-skill-supported"}, "embedded": true}]
    });
    let mut assets = BTreeMap::new();
    assets.insert(
        ("demo".to_string(), "SKILL.md".to_string()),
        md.as_bytes().to_vec(),
    );
    assets.insert(
        ("demo".to_string(), "LICENSE".to_string()),
        LICENSE.as_bytes().to_vec(),
    );
    (
        OfficialSet::from_parts(&doc.to_string(), assets).unwrap(),
        (),
    )
}

fn commit_files(up: &MemoryFetcher, repo: &str, commit: &str, md: &str, license: &str) {
    up.add_commit(
        repo,
        commit,
        &[
            ("skills/demo/SKILL.md", md),
            ("LICENSE", license),
            ("README.md", "readme"),
        ],
    );
}

impl Fx {
    fn new() -> Fx {
        let c1 = fake_sha("demo-c1");
        let md = skill_md("demo", "", "body v1");
        let up = MemoryFetcher::new();
        commit_files(&up, REPO, &c1, &md, LICENSE);
        up.add_release(REPO, "v1.0.0", &c1, false);
        Fx {
            home: fixture_dir("hp-hn05-home"),
            up,
            set: catalog(&c1, &md).0,
            c1,
        }
    }

    fn ctx(&self) -> Ctx<'_> {
        Ctx {
            home: &self.home,
            fetcher: Some(&self.up),
            now: 1_000,
            offline: false,
            frozen: false,
            gate: None,
            catalog: &self.set,
        }
    }

    fn release(&self, tag: &str, md: &str, license: &str, annotated: bool) -> String {
        let c = fake_sha(&format!("commit-{tag}"));
        commit_files(&self.up, REPO, &c, md, license);
        self.up.add_release(REPO, tag, &c, annotated);
        c
    }

    fn policy(&self) {
        ops::approve_policy(&self.home, true, Some(60), Some(2000), None).unwrap();
    }

    fn state(&self) -> State {
        State::load(&self.home).unwrap()
    }

    fn src(&self) -> Source {
        self.state().sources.remove("demo").unwrap()
    }
}

fn one(r: &Report) -> &ops::SourceReport {
    assert_eq!(r.sources.len(), 1, "{r:?}");
    &r.sources[0]
}

#[test]
fn hp_hn05_new_release_activates_only_for_new_sessions_and_rolls_back() {
    let fx = Fx::new();
    fx.policy();
    let user = write(&fx.home, "skills/user.json", "{\"preset\":\"mine\"}\n");
    let before = std::fs::read(&user).unwrap();
    // A session that started before the release locks its revision.
    let old_pin = session_pin(&fx.ctx(), "demo").unwrap().unwrap();
    assert_eq!(old_pin.version, "v1.0.0");

    fx.release(
        "v1.1.0",
        &skill_md("demo", "", "body v1.1 improved"),
        LICENSE,
        true,
    );
    let r = ops::check(&fx.ctx(), &[]).unwrap();
    let s = one(&r);
    assert_eq!(s.state, "activated", "{s:?}");
    assert_eq!(s.candidate.as_deref(), Some("v1.1.0"));
    assert_eq!(
        s.diff.as_ref().unwrap().modified,
        vec!["SKILL.md".to_string()]
    );

    // A new session sees the new revision; the old pin still loads from the store.
    let new_pin = session_pin(&fx.ctx(), "demo").unwrap().unwrap();
    assert_eq!(new_pin.version, "v1.1.0");
    assert_ne!(new_pin.digest, old_pin.digest);
    let store = fx.home.join("artifacts");
    semaprax_harness::skills::snapshot::open(&store, &old_pin.digest)
        .expect("old snapshot retained");
    let set = effective_set_from(&fx.home, fx.set.clone()).unwrap();
    let k = set.find("demo").unwrap();
    assert_eq!(k.version, "v1.1.0");
    assert!(
        String::from_utf8_lossy(set.file_bytes("demo", "SKILL.md").unwrap()).contains("improved")
    );
    let snap = set
        .materialize("demo", &store, &fx.home.join("scratch"))
        .unwrap();
    assert_eq!(snap.digest, new_pin.digest);

    // Rollback restores v1.0.0 and touches no user setting.
    let r = ops::rollback(&fx.ctx(), "demo").unwrap();
    assert_eq!(one(&r).active.as_deref(), Some("v1.0.0"));
    assert_eq!(
        session_pin(&fx.ctx(), "demo").unwrap().unwrap().digest,
        old_pin.digest
    );
    assert_eq!(std::fs::read(&user).unwrap(), before);
    assert_eq!(
        effective_set_from(&fx.home, fx.set.clone())
            .unwrap()
            .find("demo")
            .unwrap()
            .version,
        "v1.0.0"
    );

    // The rolled-back release is held: never re-activated automatically.
    let r = ops::check(&fx.ctx(), &[]).unwrap();
    assert_eq!(one(&r).state, "pending");
    assert!(one(&r).reasons.contains(&"rolled-back-hold".to_string()));
    assert_eq!(fx.src().active.unwrap().version, "v1.0.0");
}

#[test]
fn hp_hn05_without_policy_update_is_pending_until_applied() {
    let fx = Fx::new();
    fx.release(
        "v1.0.1",
        &skill_md("demo", "", "body v1.0.1"),
        LICENSE,
        false,
    );
    let r = ops::check(&fx.ctx(), &[]).unwrap();
    assert_eq!(one(&r).state, "pending");
    assert_eq!(
        one(&r).reasons,
        vec!["auto-update-not-approved".to_string()]
    );
    assert_eq!(fx.src().active.unwrap().version, "v1.0.0");
    let r = ops::apply(&fx.ctx(), "demo", false).unwrap();
    assert_eq!(one(&r).active.as_deref(), Some("v1.0.1"));
}

fn assert_unchanged_active(fx: &Fx) {
    assert_eq!(fx.src().active.unwrap().commit, fx.c1);
}

#[test]
fn hp_hn05_moved_tag_cannot_replace_active() {
    let fx = Fx::new();
    let c = fx.release("v1.1.0", &skill_md("demo", "", "v1.1"), LICENSE, false);
    let r = ops::check(&fx.ctx(), &[]).unwrap();
    assert_eq!(one(&r).state, "pending");
    // Upstream force-moves the tag to different content.
    let evil = fake_sha("evil");
    commit_files(
        &fx.up,
        REPO,
        &evil,
        &skill_md("demo", "", "swapped"),
        LICENSE,
    );
    fx.up.move_tag(REPO, "v1.1.0", &evil);
    assert_ne!(c, evil);
    let r = ops::check(&fx.ctx(), &[]).unwrap();
    assert_eq!(one(&r).state, "rejected");
    assert!(one(&r).message.as_ref().unwrap().contains("SPX-HPU003"));
    assert!(fx.src().pending.is_none());
    assert!(ops::apply(&fx.ctx(), "demo", true).is_err());
    assert_unchanged_active(&fx);
    // Moving the tag of the ACTIVE release is refused as well.
    fx.up.move_tag(REPO, "v1.0.0", &evil);
    let r = ops::check(&fx.ctx(), &[]).unwrap();
    assert!(one(&r).message.as_ref().unwrap().contains("SPX-HPU003"));
    assert_unchanged_active(&fx);
}

#[test]
fn hp_hn05_changed_identity_is_rejected_even_under_auto_policy() {
    let fx = Fx::new();
    fx.policy();
    fx.release(
        "v2.0.0",
        &skill_md("not-demo", "", "other skill"),
        LICENSE,
        false,
    );
    let r = ops::check(&fx.ctx(), &[]).unwrap();
    assert_eq!(one(&r).state, "rejected");
    assert!(one(&r).message.as_ref().unwrap().contains("SPX-HPU004"));
    assert_unchanged_active(&fx);
}

#[test]
fn hp_hn05_truncated_and_corrupt_downloads_are_rejected() {
    for corrupt in [false, true] {
        let fx = Fx::new();
        fx.policy();
        let md = skill_md("demo", "", "body v1.1 long enough to truncate meaningfully");
        fx.release("v1.1.0", &md, LICENSE, false);
        let sha = git_blob_sha(md.as_bytes());
        if corrupt {
            fx.up.corrupt_blob(&sha);
        } else {
            fx.up.truncate_blob(&sha);
        }
        let r = ops::check(&fx.ctx(), &[]).unwrap();
        assert_eq!(one(&r).state, "rejected", "corrupt={corrupt}");
        assert!(one(&r).message.as_ref().unwrap().contains("SPX-HPU005"));
        assert_unchanged_active(&fx);
        // Nothing half-extracted is addressable and no stage directory is left.
        assert!(!fx.home.join("updates/scratch/demo").exists());
    }
    // A truncated tree listing is refused as well.
    let fx = Fx::new();
    fx.release("v1.1.0", &skill_md("demo", "", "x"), LICENSE, false);
    fx.up.truncate_trees(true);
    let r = ops::check(&fx.ctx(), &[]).unwrap();
    assert!(one(&r).message.as_ref().unwrap().contains("SPX-HPU005"));
}

fn tree_source(id: &str, kind: Kind, repo: &str, subpath: &str) -> Source {
    Source {
        id: id.into(),
        kind,
        repo: repo.into(),
        subpath: Some(subpath.into()),
        channel: "latest-stable".into(),
        head_branch: "main".into(),
        files: vec![],
        active: None,
        previous: vec![],
        pending: None,
        rejected: None,
        held: None,
        revoked: vec![],
        unavailable: false,
        resolved_head: None,
    }
}

#[test]
fn hp_hn05_escaping_paths_symlinks_and_submodules_are_refused() {
    for (path, mode) in [
        ("pkg/../../evil.md", "100644"),
        ("pkg/link", "120000"),
        ("pkg/sub", "160000"),
    ] {
        let fx = Fx::new();
        let repo = "https://github.com/acme/tree-skill";
        let c = fake_sha(path);
        fx.up
            .add_commit(repo, &c, &[("pkg/SKILL.md", &skill_md("tree", "", "b"))]);
        fx.up.add_raw_file(
            repo,
            &c,
            FixtureFile {
                path: path.into(),
                mode: mode.into(),
                bytes: b"x".to_vec(),
            },
        );
        fx.up.add_release(repo, "v1.0.0", &c, false);
        ops::add_source(&fx.home, tree_source("tree", Kind::Skill, repo, "pkg")).unwrap();
        let r = ops::check(&fx.ctx(), &["tree".into()]).unwrap();
        let s = one(&r);
        assert_eq!(s.state, "rejected", "{path}: {s:?}");
        assert!(
            s.message.as_ref().unwrap().contains("SPX-HPU006"),
            "{path}: {s:?}"
        );
        assert!(fx.state().sources["tree"].active.is_none());
    }
}

#[test]
fn hp_hn05_new_permission_hook_executable_or_license_requires_review() {
    let cases: [(&str, &str, &str, &str); 3] = [
        (
            "tools",
            "allowed-tools: Bash(rm:*)\n",
            LICENSE,
            "widened-permissions:tool:",
        ),
        (
            "hooks",
            "hooks:\n  PreToolUse: run.sh\n",
            LICENSE,
            "widened-permissions:hook:declared",
        ),
        ("license", "", "Proprietary\n", "changed-license"),
    ];
    for (name, extra, lic, reason) in cases {
        let fx = Fx::new();
        fx.policy();
        fx.release("v1.1.0", &skill_md("demo", extra, "v1.1"), lic, false);
        let r = ops::check(&fx.ctx(), &[]).unwrap();
        let s = one(&r);
        assert_eq!(s.state, "pending", "{name}: {s:?}");
        assert!(
            s.reasons.iter().any(|x| x.starts_with(reason)),
            "{name}: {:?}",
            s.reasons
        );
        assert_unchanged_active(&fx);
        let e = ops::apply(&fx.ctx(), "demo", false).unwrap_err();
        assert_eq!(e.code, "SPX-HPU008");
        assert_unchanged_active(&fx);
        // Explicit review activates it.
        ops::apply(&fx.ctx(), "demo", true).unwrap();
        assert_eq!(fx.src().active.unwrap().version, "v1.1.0");
    }
    // A new script in a tree-mode skill is a new executable.
    let fx = Fx::new();
    fx.policy();
    let repo = "https://github.com/acme/tree-skill";
    let c1 = fake_sha("t1");
    fx.up
        .add_commit(repo, &c1, &[("pkg/SKILL.md", &skill_md("tree", "", "b"))]);
    fx.up.add_release(repo, "v1.0.0", &c1, false);
    ops::add_source(&fx.home, tree_source("tree", Kind::Skill, repo, "pkg")).unwrap();
    ops::check(&fx.ctx(), &["tree".into()]).unwrap();
    ops::apply(&fx.ctx(), "tree", true).unwrap();
    let c2 = fake_sha("t2");
    fx.up.add_commit(
        repo,
        &c2,
        &[
            ("pkg/SKILL.md", &skill_md("tree", "", "b2")),
            ("pkg/scripts/run.sh", "echo hi\n"),
        ],
    );
    fx.up.add_release(repo, "v1.1.0", &c2, false);
    let r = ops::check(&fx.ctx(), &["tree".into()]).unwrap();
    assert_eq!(one(&r).state, "pending");
    assert!(one(&r)
        .reasons
        .contains(&"new-executable:scripts/run.sh".to_string()));
}

#[test]
fn hp_hn05_failed_compatibility_gate_cannot_replace_active() {
    let fx = Fx::new();
    fx.policy();
    fx.release("v1.1.0", &skill_md("demo", "", "v1.1"), LICENSE, false);
    let gate =
        |_: &semaprax_harness::updates::stage::Staged| Err("behavior smoke failed".to_string());
    let mut ctx = fx.ctx();
    ctx.gate = Some(&gate);
    let r = ops::check(&ctx, &[]).unwrap();
    assert_eq!(one(&r).state, "rejected");
    assert!(one(&r).message.as_ref().unwrap().contains("SPX-HPU007"));
    assert_unchanged_active(&fx);
    assert!(fx.src().pending.is_none());
    // The incompatible latest stays visible with its reason.
    let st = ops::status(&fx.ctx(), &[]).unwrap();
    assert_eq!(one(&st).state, "rejected");
    assert_eq!(one(&st).candidate.as_deref(), Some("v1.1.0"));
    // Without the failing gate the same release is acceptable.
    let r = ops::check(&fx.ctx(), &[]).unwrap();
    assert_eq!(one(&r).state, "activated");
}

#[test]
fn hp_hn05_frozen_and_offline_make_no_requests_and_keep_locked_artifacts() {
    let fx = Fx::new();
    fx.policy();
    fx.release("v1.1.0", &skill_md("demo", "", "v1.1"), LICENSE, false);
    let pin = session_pin(&fx.ctx(), "demo").unwrap().unwrap();
    fx.up.clear_requests();
    for frozen in [true, false] {
        let mut ctx = fx.ctx();
        ctx.offline = true;
        ctx.frozen = frozen;
        let r = ops::check(&ctx, &[]).unwrap();
        assert!(r.offline);
        assert_eq!(one(&r).state, "cached");
        assert_eq!(one(&r).active.as_deref(), Some("v1.0.0"));
        assert!(!ops::maintenance(&ctx)
            .sources
            .iter()
            .any(|s| s.state == "activated"));
        assert_eq!(fx.up.request_count(), 0, "frozen={frozen}");
    }
    let mut ctx = fx.ctx();
    ctx.frozen = true;
    ctx.offline = true;
    assert_eq!(
        ops::apply(&ctx, "demo", true).unwrap_err().code,
        "SPX-HPU010"
    );
    assert_eq!(ops::rollback(&ctx, "demo").unwrap_err().code, "SPX-HPU010");
    assert_eq!(session_pin(&ctx, "demo").unwrap().unwrap(), pin);
    assert_eq!(fx.up.request_count(), 0);
    // Via the CLI: --frozen needs no fetcher and performs no lookup either.
    let env = Environment {
        harness_home: Some(fx.home.clone()),
        ..Default::default()
    };
    let out = cli_updates(&["check".into(), "--frozen".into(), "--json".into()], &env);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(out.stdout.contains("\"offline\":true"));
}

#[test]
fn hp_hn05_check_failure_is_a_bounded_notice_and_cache_stays_usable() {
    let fx = Fx::new();
    fx.policy();
    fx.release("v1.1.0", &skill_md("demo", "", "v1.1"), LICENSE, false);
    fx.up.set_outage(Some(&"rate limit exceeded ".repeat(40)));
    let r = ops::check(&fx.ctx(), &[]).unwrap();
    assert_eq!(one(&r).state, "error");
    let n = r.notice.as_ref().unwrap();
    assert!(n.len() < 400 && n.contains("using cached revisions"), "{n}");
    // Cached harness remains usable.
    assert_eq!(
        effective_set_from(&fx.home, fx.set.clone())
            .unwrap()
            .find("demo")
            .unwrap()
            .version,
        "v1.0.0"
    );
    assert_eq!(
        session_pin(&fx.ctx(), "demo").unwrap().unwrap().version,
        "v1.0.0"
    );
    let st = ops::status(&fx.ctx(), &[]).unwrap();
    assert!(st.notice.is_some());
    // Maintenance at session start never fails the session and honours the TTL.
    let mut ctx = fx.ctx();
    ctx.now = 1_000_000;
    let m = ops::maintenance(&ctx);
    assert!(m.notice.is_some());
    let used = fx.up.request_count();
    ctx.now = 1_000_010; // inside the TTL
    ops::maintenance(&ctx);
    assert_eq!(fx.up.request_count(), used);
    // Recovery clears the notice.
    fx.up.set_outage(None);
    ctx.now = 2_000_000;
    let m = ops::maintenance(&ctx);
    assert!(m.notice.is_none());
    assert_eq!(fx.src().active.unwrap().version, "v1.1.0");
}

#[test]
fn hp_hn05_maintenance_needs_policy_and_never_runs_unapproved() {
    let fx = Fx::new();
    fx.up.clear_requests();
    let m = ops::maintenance(&fx.ctx());
    assert!(m.sources.is_empty());
    assert_eq!(fx.up.request_count(), 0);
}

fn adapter_files(
    descriptor_edit: impl Fn(String) -> String,
    adapter_py_suffix: &str,
) -> Vec<(String, String)> {
    let dir = repo_root().join("packages/semaprax-harness-adapters/rtk");
    let mut v = Vec::new();
    for f in [
        "adapter.py",
        "harness-provider.json",
        "hook_detect.py",
        "rtk_families.py",
    ] {
        let mut text = std::fs::read_to_string(dir.join(f)).unwrap();
        if f == "harness-provider.json" {
            text = descriptor_edit(text);
        }
        if f == "adapter.py" {
            text.push_str(adapter_py_suffix);
        }
        v.push((format!("rtk/{f}"), text));
    }
    v
}

fn adapter_commit(fx: &Fx, tag: &str, files: &[(String, String)]) {
    let c = fake_sha(&format!("adapter-{tag}"));
    let refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    fx.up.add_commit(AREPO, &c, &refs);
    fx.up.add_release(AREPO, tag, &c, true);
}

#[test]
fn hp_hn05_same_resolver_updates_a_skill_and_an_external_adapter() {
    let fx = Fx::new();
    fx.policy();
    adapter_commit(&fx, "v0.1.0", &adapter_files(|s| s, ""));
    ops::add_source(&fx.home, tree_source("rtk", Kind::Adapter, AREPO, "rtk")).unwrap();
    fx.release("v1.1.0", &skill_md("demo", "", "v1.1"), LICENSE, false);

    // One check handles both kinds; the skill activates, the adapter needs review.
    let r = ops::check(&fx.ctx(), &[]).unwrap();
    let by: BTreeMap<_, _> = r.sources.iter().map(|s| (s.id.as_str(), s)).collect();
    assert_eq!(by["demo"].state, "activated");
    assert_eq!(by["rtk"].state, "pending");
    assert_eq!(by["rtk"].reasons, vec!["initial-install".to_string()]);
    ops::apply(&fx.ctx(), "rtk", true).unwrap();
    let a = fx.state().sources["rtk"].active.clone().unwrap();
    assert_eq!(a.version, "v0.1.0");
    assert_eq!(a.identity, "ai.rtk/rtk-command-view");
    let snap =
        semaprax_harness::skills::snapshot::open(&fx.home.join("artifacts"), &a.digest).unwrap();
    assert!(snap.files_dir.join("adapter.py").is_file());

    // Adapter code change under an auto-content policy still needs review.
    adapter_commit(&fx, "v0.1.1", &adapter_files(|s| s, "\n# changed\n"));
    let r = ops::check(&fx.ctx(), &["rtk".into()]).unwrap();
    assert_eq!(one(&r).state, "pending");
    assert!(one(&r)
        .reasons
        .contains(&"adapter-code-changed".to_string()));
    assert_eq!(
        fx.state().sources["rtk"].active.as_ref().unwrap().version,
        "v0.1.0"
    );

    // Widened permission is named explicitly.
    let widen = |s: String| s.replace("\"network\": []", "\"network\": [\"api.example.com\"]");
    adapter_commit(&fx, "v0.2.0", &adapter_files(widen, ""));
    let r = ops::check(&fx.ctx(), &["rtk".into()]).unwrap();
    assert!(
        one(&r)
            .reasons
            .iter()
            .any(|x| x == "widened-permissions:network:api.example.com"),
        "{:?}",
        one(&r)
    );
    ops::apply(&fx.ctx(), "rtk", true).unwrap();
    assert_eq!(
        fx.state().sources["rtk"].active.as_ref().unwrap().version,
        "v0.2.0"
    );

    // Changed identity (provider id) is rejected outright.
    let rename = |s: String| s.replace("ai.rtk/rtk-command-view", "ai.other/evil");
    adapter_commit(&fx, "v0.3.0", &adapter_files(rename, ""));
    let r = ops::check(&fx.ctx(), &["rtk".into()]).unwrap();
    assert_eq!(one(&r).state, "rejected");
    assert!(one(&r).message.as_ref().unwrap().contains("SPX-HPU004"));
    ops::rollback(&fx.ctx(), "rtk").unwrap();
    assert_eq!(
        fx.state().sources["rtk"].active.as_ref().unwrap().version,
        "v0.1.0"
    );
}

#[test]
fn hp_hn05_head_channel_resolves_to_and_records_a_commit() {
    let fx = Fx::new();
    let head = fake_sha("head-commit");
    commit_files(
        &fx.up,
        REPO,
        &head,
        &skill_md("headskill", "", "from head"),
        LICENSE,
    );
    fx.up.set_branch(REPO, "main", &head);
    let mut src = tree_source("headskill", Kind::Skill, REPO, "skills/demo");
    src.channel = "head:main".into();
    ops::add_source(&fx.home, src).unwrap();
    fx.up.clear_requests();
    let r = ops::check(&fx.ctx(), &["headskill".into()]).unwrap();
    let s = one(&r);
    assert_eq!(s.candidate_commit.as_deref(), Some(head.as_str()), "{s:?}");
    assert_eq!(
        s.candidate.as_deref(),
        Some(format!("commit:{}", &head[..8]).as_str())
    );
    let st = fx.state();
    assert_eq!(
        st.sources["headskill"].resolved_head,
        Some(("main".into(), head.clone()))
    );
    // Exactly one request names the mutable branch; every content read is by sha.
    let reqs = fx.up.requests();
    assert_eq!(
        reqs.iter().filter(|r| r.contains("main")).count(),
        1,
        "{reqs:?}"
    );
    assert!(reqs
        .iter()
        .filter(|r| r.starts_with("tree") || r.starts_with("blob"))
        .all(|r| !r.contains("main")));
    assert!(reqs
        .iter()
        .any(|r| r.starts_with(&format!("tree {REPO} {head}"))));
    // Moving the branch is a new candidate commit, not a mutation of the recorded one.
    let head2 = fake_sha("head-commit-2");
    commit_files(
        &fx.up,
        REPO,
        &head2,
        &skill_md("headskill", "", "from head 2"),
        LICENSE,
    );
    fx.up.set_branch(REPO, "main", &head2);
    ops::apply(&fx.ctx(), "headskill", true).unwrap();
    let r = ops::check(&fx.ctx(), &["headskill".into()]).unwrap();
    assert_eq!(one(&r).candidate_commit.as_deref(), Some(head2.as_str()));
    assert_eq!(
        fx.state().sources["headskill"]
            .active
            .as_ref()
            .unwrap()
            .commit,
        head
    );
}

#[test]
fn hp_hn05_channels_resolve_annotated_tags_ranges_and_commits() {
    let fx = Fx::new();
    let c2 = fx.release("v2.0.0", &skill_md("demo", "", "two"), LICENSE, true);
    let c15 = fx.release("v1.5.0", &skill_md("demo", "", "one-five"), LICENSE, true);
    let cpre = fake_sha("pre");
    commit_files(&fx.up, REPO, &cpre, &skill_md("demo", "", "pre"), LICENSE);
    fx.up.add_prerelease(REPO, "v3.0.0-rc1", &cpre);
    let f: &dyn semaprax_harness::updates::Fetcher = &fx.up;
    let latest = resolve(f, REPO, &Channel::LatestStable, "main").unwrap();
    assert_eq!(
        (latest.tag.as_deref(), latest.commit.as_str()),
        (Some("v2.0.0"), c2.as_str())
    );
    let range = Channel::parse("range:>=1.0.0,<2.0.0").unwrap();
    let r = resolve(f, REPO, &range, "main").unwrap();
    assert_eq!(
        (r.tag.as_deref(), r.commit.as_str()),
        (Some("v1.5.0"), c15.as_str())
    );
    let exact = Channel::parse(&format!("commit:{}", fx.c1)).unwrap();
    assert_eq!(resolve(f, REPO, &exact, "main").unwrap().commit, fx.c1);
    assert!(Channel::parse("range:>=9.0.0")
        .and_then(|c| resolve(f, REPO, &c, "main"))
        .is_err());
    assert!(Channel::parse("main").is_err());
}

#[test]
fn hp_hn05_revoked_artifacts_become_unavailable_and_are_never_reactivated() {
    let fx = Fx::new();
    fx.policy();
    let c = fx.release("v1.1.0", &skill_md("demo", "", "v1.1"), LICENSE, false);
    ops::check(&fx.ctx(), &[]).unwrap();
    assert_eq!(fx.src().active.unwrap().version, "v1.1.0");
    // Origin withdraws v1.1.0: safe fallback to the retained v1.0.0.
    fx.up.revoke(REPO, &c);
    let r = ops::check(&fx.ctx(), &[]).unwrap();
    assert_eq!(fx.src().active.unwrap().version, "v1.0.0");
    assert_eq!(one(&r).state, "rejected");
    assert!(one(&r).message.as_ref().unwrap().contains("SPX-HPU011"));
    assert!(ops::rollback(&fx.ctx(), "demo").is_err());
    // Revoking the last good revision leaves an explicit unavailable state.
    let first = fx.c1.clone();
    let r = ops::revoke(&fx.ctx(), "demo", &first).unwrap();
    assert_eq!(one(&r).state, "unavailable");
    assert!(session_pin(&fx.ctx(), "demo").unwrap().unwrap().unavailable);
    assert!(
        !effective_set_from(&fx.home, fx.set.clone())
            .unwrap()
            .find("demo")
            .unwrap()
            .embedded
    );
    let st = ops::status(&fx.ctx(), &[]).unwrap();
    assert_eq!(one(&st).state, "unavailable");
}

#[test]
fn hp_hn05_propose_reports_conformance_without_touching_state() {
    let fx = Fx::new();
    fx.release("v1.1.0", &skill_md("demo", "", "v1.1"), LICENSE, false);
    let doc = semaprax_harness::updates::propose::propose(&fx.ctx(), &fx.up, &[]).unwrap();
    let p = &doc["proposals"][0];
    assert_eq!(p["status"], "proposed", "{p}");
    assert_eq!(p["proposed"]["tag"], "v1.1.0");
    assert_eq!(p["conformance"]["compatibility_smoke"], "pass");
    assert!(
        p["proposed"]["files"][0]["git_blob_sha"]
            .as_str()
            .unwrap()
            .len()
            == 40
    );
    assert!(!fx.home.join("updates/state.json").exists());
    assert!(!fx.home.join("updates/propose").exists());
    // A release that fails conformance is reported, not proposed.
    fx.release("v1.2.0", &skill_md("renamed", "", "x"), LICENSE, false);
    let doc = semaprax_harness::updates::propose::propose(&fx.ctx(), &fx.up, &[]).unwrap();
    assert_eq!(doc["proposals"][0]["status"], "failed-conformance");
}

fn write_fixture_upstream(dir: &Path, c1: &str, c2: &str) {
    let repo = "acme/cli-skill";
    let doc = serde_json::json!({
        "releases": [{"tag": "v1.1.0"}, {"tag": "v1.0.0"}],
        "tags": {"v1.0.0": {"commit": c1}, "v1.1.0": {"commit": c2, "annotated": true}},
        "branches": {"main": c2}, "revoked": []
    });
    write(dir, &format!("{repo}/repo.json"), &doc.to_string());
    write(
        dir,
        &format!("{repo}/commits/{c1}/pkg/SKILL.md"),
        &skill_md("cli-skill", "", "one"),
    );
    write(
        dir,
        &format!("{repo}/commits/{c2}/pkg/SKILL.md"),
        &skill_md("cli-skill", "", "two"),
    );
}

#[test]
fn hp_hn05_cli_check_apply_status_rollback_with_directory_fixture() {
    let home = fixture_dir("hp-hn05-clihome");
    let up = fixture_dir("hp-hn05-cliup");
    let (c1, c2) = (fake_sha("cli1"), fake_sha("cli2"));
    write_fixture_upstream(&up, &c1, &c2);
    let env = Environment {
        harness_home: Some(home.clone()),
        ..Default::default()
    };
    let run = |a: &[&str]| {
        let v: Vec<String> = a.iter().map(|s| s.to_string()).collect();
        cli_updates(&v, &env)
    };
    let fx = up.to_str().unwrap();
    // Install the older release first through an exact-commit channel.
    let o = run(&[
        "add",
        "cli-skill",
        "--kind",
        "skill",
        "--repo",
        "https://github.com/acme/cli-skill",
        "--subpath",
        "pkg",
        "--channel",
        &format!("commit:{c1}"),
    ]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    let o = run(&["check", "cli-skill", "--fixture-dir", fx]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stdout.contains("pending"), "{}", o.stdout);
    let o = run(&["apply", "cli-skill", "--approve"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    // Without --approve the initial-install style review is enforced.
    let o = run(&["apply", "nope"]);
    assert_eq!(o.code, 1);
    assert!(o.stderr.contains("SPX-HPU001"));
    let o = run(&["status", "--json"]);
    assert!(
        o.stdout.contains("\"active_commit\":\"") && o.stdout.contains(&c1),
        "{}",
        o.stdout
    );
    // `check` without a fetcher is cached/offline, not an error.
    let o = run(&["check"]);
    assert_eq!(o.code, 0);
    assert!(o.stdout.contains("cached"), "{}", o.stdout);
    // Rollback with no previous revision is an explicit refusal.
    assert!(run(&["rollback", "cli-skill"])
        .stderr
        .contains("SPX-HPU014"));
    assert_eq!(cli_updates(&["bogus".into()], &env).code, 2);
    assert_eq!(
        run(&["approve-policy", "--auto-content", "--ttl-secs", "5"]).code,
        0
    );
    assert!(State::load(&home).unwrap().policy.auto_content);
}

#[test]
fn hp_hn05_gh_fetcher_refuses_relative_paths_and_non_github_origins() {
    use semaprax_harness::updates::fetch::parse_repo;
    let env = Environment::default();
    let e = semaprax_harness::updates::GitHubCliFetcher::new(
        "gh".into(),
        &env,
        std::time::Duration::from_secs(1),
    );
    assert_eq!(e.err().unwrap().code, "SPX-HPU015");
    for bad in [
        "http://github.com/a/b",
        "https://evil.example/a/b",
        "https://github.com/a",
        "https://github.com/../b",
    ] {
        assert_eq!(parse_repo(bad).unwrap_err().code, "SPX-HPU016", "{bad}");
    }
    assert_eq!(
        parse_repo("https://github.com/a/b.git").unwrap(),
        ("a".into(), "b".into())
    );
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_GH (absolute path to an authenticated gh)"]
fn hp_hn05_real_gh_resolves_ponytail_and_caveman_latest_stable() {
    let gh = std::env::var("SEMAPRAX_GH").expect("SEMAPRAX_GH");
    let home = fixture_dir("hp-hn05-realhome");
    let env = Environment::from_process();
    let f = semaprax_harness::updates::GitHubCliFetcher::new(
        PathBuf::from(gh),
        &env,
        std::time::Duration::from_secs(60),
    )
    .unwrap();
    let set = semaprax_harness::skills::official::OfficialSet::embedded();
    let ctx = Ctx {
        home: &home,
        fetcher: Some(&f),
        now: 1,
        offline: false,
        frozen: false,
        gate: None,
        catalog: &set,
    };
    let doc = semaprax_harness::updates::propose::propose(&ctx, &f, &[]).unwrap();
    println!("{}", serde_json::to_string_pretty(&doc).unwrap());
    for p in doc["proposals"].as_array().unwrap() {
        assert!(
            matches!(p["status"].as_str(), Some("current") | Some("proposed")),
            "{p}"
        );
    }
    assert_eq!(doc["proposals"].as_array().unwrap().len(), 2);
    // Byte-level check: download the recorded commit's files with real blob/sha
    // verification and reproduce the embedded bundle digest exactly.
    for k in set.embedded_skills() {
        let store = home.join("verify-store");
        let src = ops::catalog_source(&set, k, &store, &home.join("verify-base")).unwrap();
        let res = semaprax_harness::updates::resolve::Resolved {
            commit: k.commit.clone().unwrap(),
            tag: k.tag.clone(),
            version: k.version.clone(),
            head_branch: None,
        };
        let staged = semaprax_harness::updates::stage::stage(
            &f,
            &src,
            &res,
            &store,
            &home.join("verify-scratch"),
            None,
        )
        .unwrap();
        assert_eq!(
            Some(staged.snapshot.digest.clone()),
            k.bundle_digest,
            "{}",
            k.id
        );
        assert!(
            staged.diff.is_empty() && staged.reasons.is_empty(),
            "{} {:?}",
            k.id,
            staged.reasons
        );
        println!(
            "verified {} {} digest {}",
            k.id, k.version, staged.snapshot.digest
        );
    }
}

#[path = "updates/tx.rs"]
mod tx;
