//! DV-03 / DV-15: update-state transactions and strict range parsing.
//! Fixture prefix `hp-hn05` (shared with the parent module).

use super::*;
use semaprax_harness::diag::{HarnessDiagnostic, HarnessResult};
use semaprax_harness::updates::fetch::{Release, TagRef, Tree};
use semaprax_harness::updates::state::{lock, lock_within, state_path, Policy};
use semaprax_harness::updates::Fetcher;
use std::cell::RefCell;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

type Hook<'a> = RefCell<Option<Box<dyn FnOnce() + 'a>>>;

/// Runs `hook` once, inside the first `releases` call (the fetch seam of a
/// slow check), then optionally fails it like an outage.
struct Seam<'a> {
    inner: &'a MemoryFetcher,
    hook: Hook<'a>,
    fail: bool,
}

impl Fetcher for Seam<'_> {
    fn releases(&self, repo: &str) -> HarnessResult<Vec<Release>> {
        if let Some(h) = self.hook.borrow_mut().take() {
            h();
            if self.fail {
                return Err(HarnessDiagnostic::new("SPX-HPU009", "simulated outage"));
            }
        }
        self.inner.releases(repo)
    }
    fn tag_ref(&self, r: &str, t: &str) -> HarnessResult<Option<TagRef>> {
        self.inner.tag_ref(r, t)
    }
    fn peel_tag(&self, r: &str, o: &str) -> HarnessResult<String> {
        self.inner.peel_tag(r, o)
    }
    fn branch_head(&self, r: &str, b: &str) -> HarnessResult<String> {
        self.inner.branch_head(r, b)
    }
    fn tree(&self, r: &str, c: &str) -> HarnessResult<Tree> {
        self.inner.tree(r, c)
    }
    fn blob(&self, r: &str, s: &str, m: u64) -> HarnessResult<Vec<u8>> {
        self.inner.blob(r, s, m)
    }
    fn revoked(&self, r: &str) -> HarnessResult<Vec<String>> {
        self.inner.revoked(r)
    }
}

fn seeded() -> Fx {
    let fx = Fx::new();
    // Persist the seeded catalog source with a harmless revocation.
    ops::revoke(&fx.ctx(), "demo", "unrelated-seed").unwrap();
    fx
}

fn check_with(fx: &Fx, fail: bool, hook: impl FnOnce() + 'static) -> HarnessResult<Report> {
    let seam = Seam {
        inner: &fx.up,
        hook: RefCell::new(Some(Box::new(hook))),
        fail,
    };
    let mut ctx = fx.ctx();
    ctx.fetcher = Some(&seam);
    ops::check(&ctx, &[])
}

fn revoke_in_hook(fx: &Fx, target: String) -> impl FnOnce() + 'static {
    let home = fx.home.clone();
    let set = fx.set.clone();
    move || {
        let ctx = Ctx {
            home: &home,
            fetcher: None,
            now: 1_001,
            offline: true,
            frozen: false,
            gate: None,
            catalog: &set,
        };
        ops::revoke(&ctx, "demo", &target).unwrap();
        let src = State::load(&home).unwrap().sources.remove("demo").unwrap();
        assert!(src.revoked.contains(&target) && src.unavailable);
    }
}

#[test]
fn hp_dv03_check_paused_after_load_cannot_erase_a_concurrent_revocation() {
    let fx = seeded();
    let c1 = fx.c1.clone();
    // The fetch fails after the revoke committed: the old code saved its stale
    // snapshot over the revocation.
    let r = check_with(&fx, true, revoke_in_hook(&fx, c1.clone())).unwrap();
    assert_eq!(one(&r).state, "error");
    let src = fx.src();
    assert!(
        src.revoked.contains(&c1),
        "revocation lost: {:?}",
        src.revoked
    );
    assert!(src.unavailable, "revoked active revision was reactivated");
    assert!(session_pin(&fx.ctx(), "demo").unwrap().unwrap().unavailable);
    assert!(
        !effective_set_from(&fx.home, fx.set.clone())
            .unwrap()
            .find("demo")
            .unwrap()
            .embedded
    );
}

#[test]
fn hp_dv03_successful_check_does_not_publish_over_a_changed_source() {
    let fx = seeded();
    fx.policy();
    fx.release("v1.1.0", &skill_md("demo", "", "v1.1"), LICENSE, false);
    let c1 = fx.c1.clone();
    let r = check_with(&fx, false, revoke_in_hook(&fx, c1.clone())).unwrap();
    // Controlled conflict: the newer state wins and the report says so.
    let m = one(&r).message.clone().unwrap();
    assert!(m.contains("SPX-HPU018"), "{m}");
    assert_eq!(one(&r).state, "error");
    let src = fx.src();
    assert!(src.revoked.contains(&c1) && src.unavailable);
    assert_eq!(src.active.unwrap().version, "v1.0.0");
    // A re-run starts from the new state: recovery from a revocation needs review.
    let r = ops::check(&fx.ctx(), &[]).unwrap();
    assert_eq!(one(&r).state, "pending");
    ops::apply(&fx.ctx(), "demo", true).unwrap();
    let src = fx.src();
    assert!(!src.unavailable && src.revoked.contains(&c1));
    assert_eq!(src.active.unwrap().version, "v1.1.0");
}

#[test]
fn hp_dv03_policy_and_source_registration_survive_a_concurrent_check() {
    let fx = seeded();
    let home = fx.home.clone();
    let hook = move || {
        ops::approve_policy(&home, true, Some(7), Some(3000), None).unwrap();
        let s = tree_source("extra", Kind::Skill, REPO, "skills/demo");
        ops::add_source(&home, s).unwrap();
    };
    check_with(&fx, true, hook).unwrap();
    let st = fx.state();
    assert!(st.policy.approved && st.policy.auto_content);
    assert_eq!((st.policy.ttl_secs, st.policy.timeout_ms), (7, 3000));
    assert!(st.sources.contains_key("extra") && st.sources.contains_key("demo"));
    assert_eq!(st.last_check, 1_000);
    assert_ne!(st.policy, Policy::default());
}

#[test]
fn hp_dv03_state_lock_is_bounded_and_released_on_drop() {
    let fx = seeded();
    let held = lock(&fx.home).unwrap();
    let t = Instant::now();
    let e = lock_within(&fx.home, Duration::from_millis(60)).unwrap_err();
    assert_eq!(e.code, "SPX-HPU017");
    assert!(t.elapsed() >= Duration::from_millis(60) && t.elapsed() < Duration::from_secs(4));
    let before = std::fs::read(state_path(&fx.home)).unwrap();
    drop(held);
    lock_within(&fx.home, Duration::from_millis(60)).unwrap();
    assert_eq!(before, std::fs::read(state_path(&fx.home)).unwrap());
}

const CHILD: &str = "SPX_DV03_CHILD";

fn wait_for(p: &Path) {
    let end = Instant::now() + Duration::from_secs(20);
    while !p.exists() {
        assert!(
            Instant::now() < end,
            "barrier {} never appeared",
            p.display()
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Re-invoked by the multi-process tests below; a no-op in a normal run.
#[test]
fn hp_dv03_child_worker() {
    let Ok(mode) = std::env::var(CHILD) else {
        return;
    };
    let home = PathBuf::from(std::env::var("SPX_DV03_HOME").unwrap());
    let idx = std::env::var("SPX_DV03_IDX").unwrap();
    let c1 = fake_sha("demo-c1");
    let set = catalog(&c1, &skill_md("demo", "", "body v1")).0;
    let ctx = Ctx {
        home: &home,
        fetcher: None,
        now: 5,
        offline: true,
        frozen: false,
        gate: None,
        catalog: &set,
    };
    wait_for(&home.join("go"));
    if mode == "revoke" {
        for j in 0..6 {
            ops::revoke(&ctx, "demo", &format!("rev-{idx}-{j}")).unwrap();
        }
    } else {
        let _lock = lock(&home).unwrap();
        std::fs::write(home.join(format!("held-{idx}")), "x").unwrap();
        // The parent kills this process while it holds the lock.
        std::thread::sleep(Duration::from_secs(60));
    }
}

fn spawn_child(mode: &str, home: &Path, idx: usize) -> std::process::Child {
    Command::new(std::env::current_exe().unwrap())
        .args(["hp_dv03_child_worker", "--nocapture", "--test-threads=1"])
        .env(CHILD, mode)
        .env("SPX_DV03_HOME", home)
        .env("SPX_DV03_IDX", idx.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
}

#[test]
fn hp_dv03_concurrent_processes_never_lose_a_revocation() {
    let fx = seeded();
    let kids: Vec<_> = (0..4).map(|i| spawn_child("revoke", &fx.home, i)).collect();
    std::fs::write(fx.home.join("go"), "go").unwrap(); // barrier: all start together
    let end = Instant::now() + Duration::from_secs(120);
    for mut k in kids {
        loop {
            if let Some(s) = k.try_wait().unwrap() {
                assert!(s.success(), "child failed: {s}");
                break;
            }
            assert!(Instant::now() < end, "child hung");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    let revoked = fx.src().revoked;
    for i in 0..4 {
        for j in 0..6 {
            assert!(
                revoked.contains(&format!("rev-{i}-{j}")),
                "lost rev-{i}-{j}"
            );
        }
    }
}

#[test]
fn hp_dv03_lock_is_released_when_the_owner_process_dies() {
    let fx = seeded();
    let mut kid = spawn_child("hold", &fx.home, 0);
    std::fs::write(fx.home.join("go"), "go").unwrap();
    wait_for(&fx.home.join("held-0"));
    let e = lock_within(&fx.home, Duration::from_millis(50)).unwrap_err();
    assert_eq!(e.code, "SPX-HPU017");
    kid.kill().unwrap();
    kid.wait().unwrap();
    lock_within(&fx.home, Duration::from_secs(5)).expect("lock must die with its owner");
    ops::revoke(&fx.ctx(), "demo", "after-crash").unwrap();
    assert!(fx.src().revoked.contains(&"after-crash".to_string()));
}

#[test]
fn hp_dv15_caret_zero_major_and_strict_grammar_resolve_the_highest_satisfying_release() {
    let up = MemoryFetcher::new();
    for (tag, n) in [
        ("v0.0.1", "1"),
        ("v0.0.9", "9"),
        ("v0.2.3", "a"),
        ("v0.2.9", "b"),
        ("v0.3.0", "c"),
        ("v1.2.3", "d"),
        ("v1.9.0", "e"),
        ("v2.0.0", "f"),
    ] {
        up.add_release(REPO, tag, &n.repeat(40), false);
    }
    let f: &dyn Fetcher = &up;
    let pick = |req: &str| {
        let c = Channel::parse(&format!("range:{req}")).unwrap();
        resolve(f, REPO, &c, "main").unwrap().version
    };
    assert_eq!(pick("^0.0.1"), "v0.0.1");
    assert_eq!(pick("^0.2.3"), "v0.2.9");
    assert_eq!(pick("^1.2.3"), "v1.9.0");
    assert_eq!(pick("~1.2"), "v1.2.3");
    assert_eq!(pick("^0.0"), "v0.0.9");
    assert_eq!(pick(">=0.0.1, <0.0.9"), "v0.0.1");
    let before = up.request_count();
    for bad in [
        ">=1.2.3.4",
        ">=1.bad.3",
        ">=1.2.bad",
        ">=",
        "^1..2",
        "^99999999999999999999",
    ] {
        let e = Channel::parse(&format!("range:{bad}")).unwrap_err();
        assert_eq!(e.code, "SPX-HPU001", "{bad}");
    }
    assert_eq!(up.request_count(), before, "parse must not touch upstream");
}

#[test]
fn hp_dv15_a_malformed_range_never_reaches_state_or_upstream() {
    let fx = Fx::new();
    let mut s = tree_source("bad", Kind::Skill, REPO, "skills/demo");
    s.channel = "range:>=1.2.bad".into();
    let e = ops::add_source(&fx.home, s).unwrap_err();
    assert_eq!(e.code, "SPX-HPU001");
    assert!(!fx.state().sources.contains_key("bad"));
    assert_eq!(fx.up.request_count(), 0);
}
