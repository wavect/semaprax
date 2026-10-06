//! MN-05: provider spans are verified against the captured revision, never a
//! later file version, and only fully verified answers are cached.

use super::*;
use semaprax_harness::context::identity::unbound;

const FAKE: &str = "org.example/fake";

fn project() -> PathBuf {
    fake_world_project()
}

fn line_item(snap: &Snapshot, rel: &str, line: u64) -> RawItem {
    let t = std::fs::read_to_string(snap.root.join(rel)).unwrap();
    let l = t.split('\n').nth(line as usize - 1).unwrap();
    RawItem {
        path: rel.into(),
        span: Span {
            start_line: line,
            end_line: line,
        },
        digest: sha256_plain(l.as_bytes()),
        tier: Tier::Structural,
        language: "typescript".into(),
        rank: 1.0,
        text: Some(l.to_string()),
        span_kind: None,
        edges: vec![],
    }
}

fn response(items: Vec<RawItem>) -> ExternalResponse {
    ExternalResponse {
        status: "complete".into(),
        no_references: items.is_empty(),
        items,
        coverage: complete(),
        upstream_version: None,
        provider_id: FAKE.into(),
        diagnostics: vec![],
    }
}

/// A provider that edits `web/app.ts` to `after` while answering with the span of
/// line 2 as it reads it *after* the edit, exactly once.
fn racing_broker(p: &Path, after: &'static str, cache: bool) -> (Broker, Arc<AtomicUsize>) {
    let root = p.to_path_buf();
    let answer: Answer = Box::new(move |snap, _| {
        let f = root.join("web/app.ts");
        assert!(
            snap.files.contains_key("web/app.ts"),
            "snapshot carries the captured digest"
        );
        let before = std::fs::read_to_string(&f).unwrap();
        if before != after {
            std::fs::write(&f, after).unwrap();
        }
        response(vec![line_item(snap, "web/app.ts", 2)])
    });
    let s = FakeSource::new(FAKE, answer);
    let q = s.queries.clone();
    let dir = p.parent().unwrap().join("cache");
    let mut b = if cache {
        fake_broker(s, &dir)
    } else {
        let mut b = Broker::new(Some(native()), None);
        b.add_provider(Box::new(s)).unwrap();
        b
    };
    b.set_cache(
        cache.then(|| ResultCache::new(dir.clone(), CacheConfig::default(), system_clock())),
    );
    (b, q)
}

fn only_request() -> BrokerRequest {
    let mut r = req("browser", 12_000);
    r.native_targets = Some(vec![]);
    r
}

const B_SAME_SIZE: &str = "export function renderAdd(left: number, right: number): number {\n  // browser shell that wraps the sub export\n  return left + right;\n}\n";

#[test]
fn mn05_item_from_changed_file_is_never_verified_under_the_captured_revision() {
    for cache in [true, false] {
        let p = project();
        let (b, q) = racing_broker(&p, B_SAME_SIZE, cache);
        let o = b.context(&p, &only_request()).unwrap();
        assert_eq!(q.load(Ordering::SeqCst), 1);
        assert_eq!(o.external.len(), 1);
        let it = &o.external[0];
        assert!(!it.verified && !it.complete, "cache={cache}");
        assert_eq!(it.omission_reason.as_deref(), Some(unbound::CHANGED));
        assert_eq!(it.provenance, Tier::Inferred);
        assert_eq!(
            it.revision, o.snapshot.revision,
            "revision stays A, unverified"
        );
        assert!(!it.authorizes_edits());
        if let Some(c) = b.cache() {
            assert_eq!(c.len(), 0, "a failed binding is not cached");
        }
        // A fresh capture sees B; the same answer is now verified under B's revision.
        let o2 = b.context(&p, &only_request()).unwrap();
        assert_ne!(o2.snapshot.revision, o.snapshot.revision);
        assert!(o2.external[0].verified);
        assert!(!o2.cache_hits[FAKE]);
    }
}

#[test]
fn mn05_change_outside_the_returned_span_is_detected() {
    // Line 2 (the returned span) is untouched; line 3 changes.
    let p = project();
    let (b, _) = racing_broker(
        &p,
        "export function renderAdd(left: number, right: number): number {\n  // browser shell that wraps the add export\n  return left - right;\n}\n",
        true,
    );
    let o = b.context(&p, &only_request()).unwrap();
    let it = &o.external[0];
    assert!(!it.verified);
    assert_eq!(it.omission_reason.as_deref(), Some(unbound::CHANGED));
    assert_eq!(b.cache().unwrap().len(), 0);
    assert!(!o.exhaustive);
}

#[test]
fn mn05_unchanged_source_verifies_and_is_cached_only_then() {
    let p = project();
    let original = std::fs::read_to_string(p.join("web/app.ts")).unwrap();
    let leaked: &'static str = Box::leak(original.into_boxed_str());
    let (b, q) = racing_broker(&p, leaked, true);
    let o = b.context(&p, &only_request()).unwrap();
    assert!(o.external[0].verified && o.external[0].complete);
    assert_eq!(b.cache().unwrap().len(), 1);
    let o2 = b.context(&p, &only_request()).unwrap();
    assert!(o2.cache_hits[FAKE]);
    assert!(o2.external[0].verified);
    assert_eq!(q.load(Ordering::SeqCst), 1);
}

#[test]
fn mn05_cached_answer_is_rebound_when_the_file_moves_after_caching() {
    let p = project();
    let original = std::fs::read_to_string(p.join("web/app.ts")).unwrap();
    let leaked: &'static str = Box::leak(original.into_boxed_str());
    let (b, _) = racing_broker(&p, leaked, true);
    b.context(&p, &only_request()).unwrap();
    // Same revision key cannot survive an edit: the next capture misses the cache.
    std::fs::write(p.join("web/app.ts"), B_SAME_SIZE).unwrap();
    let o = b.context(&p, &only_request()).unwrap();
    assert!(!o.cache_hits[FAKE]);
}

#[test]
fn mn05_read_bound_has_explicit_outcomes() {
    let p = project();
    let snap = Snapshot::capture(&p).unwrap();
    assert!(snap.read_bound("web/app.ts").unwrap().contains("renderAdd"));
    assert_eq!(snap.read_bound("nope.ts"), Err(unbound::MISSING));

    let f = p.join("web/app.ts");
    let a = std::fs::read_to_string(&f).unwrap();
    std::fs::write(&f, a.replace("add", "sub")).unwrap(); // same size
    assert_eq!(snap.read_bound("web/app.ts"), Err(unbound::CHANGED));
    std::fs::write(&f, format!("{a}// more\n")).unwrap(); // different size
    assert_eq!(snap.read_bound("web/app.ts"), Err(unbound::CHANGED));
    std::fs::remove_file(&f).unwrap(); // deletion
    assert_eq!(snap.read_bound("web/app.ts"), Err(unbound::MISSING));
    std::fs::create_dir(&f).unwrap(); // replacement by another kind
    assert_eq!(snap.read_bound("web/app.ts"), Err(unbound::CHANGED));
    std::fs::remove_dir(&f).unwrap();
    std::fs::write(&f, &a).unwrap(); // restored bytes bind again
    assert!(snap.read_bound("web/app.ts").is_ok());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o000)).unwrap();
        let denied = std::fs::read(&f).is_err(); // root can still read
        let got = snap.read_bound("web/app.ts");
        std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o644)).unwrap();
        if denied {
            assert_eq!(got, Err(unbound::UNREADABLE));
        }
    }
}

#[test]
fn mn05_deleted_file_item_is_unverified_with_a_distinct_reason() {
    let p = project();
    let root = p.clone();
    let answer: Answer = Box::new(move |snap, _| {
        let it = line_item(snap, "web/app.ts", 2);
        std::fs::remove_file(root.join("web/app.ts")).unwrap();
        response(vec![it])
    });
    let b = fake_broker(
        FakeSource::new(FAKE, answer),
        &p.parent().unwrap().join("c"),
    );
    let o = b.context(&p, &only_request()).unwrap();
    assert_eq!(
        o.external[0].omission_reason.as_deref(),
        Some(unbound::MISSING)
    );
    assert_eq!(b.cache().unwrap().len(), 0);
}

/// Compiler stand-in that edits the declaring file while "answering".
struct RacingNative(PathBuf);

impl NativeContextSource for RacingNative {
    fn identity(&self) -> String {
        "racing-compiler".into()
    }
    fn facts(&self, p: &Path, target: &str, q: &NativeQuery) -> HarnessResult<NativeFacts> {
        let f = self.0.join("src/core.spx");
        let t = std::fs::read_to_string(&f).unwrap();
        std::fs::write(&f, t.replace("left + right", "left - right")).unwrap();
        FakeNative { pad: 0 }.facts(p, target, q)
    }
}

#[test]
fn mn05_native_item_for_a_moved_file_is_unverified_and_not_compiler_authority() {
    let p = project();
    let mut b = Broker::new(Some(Box::new(RacingNative(p.clone()))), None);
    b.add_provider(Box::new(FakeSource::new(FAKE, grep_answer(FAKE))))
        .unwrap();
    let mut r = req("zzz", 12_000);
    r.native_targets = Some(vec!["calculator.add".into()]);
    let o = b.context(&p, &r).unwrap();
    let n = &o.native[0];
    assert!(!n.verified && !n.complete);
    assert_eq!(n.omission_reason.as_deref(), Some(unbound::CHANGED));
    assert_ne!(n.provenance, Tier::CompilerVerified);
    assert!(!n.authorizes_edits());
    assert_eq!(n.provider_id, "semaprax.compiler", "native identity kept");

    // Unchanged tree: native keeps compiler authority and external stays a hint.
    let p2 = project();
    let b2 = fake_broker(
        FakeSource::new(FAKE, grep_answer(FAKE)),
        &p2.parent().unwrap().join("c"),
    );
    let o2 = b2.context(&p2, &r).unwrap();
    assert!(o2.native[0].verified);
    assert_eq!(o2.native[0].provenance, Tier::CompilerVerified);
    assert!(o2.external.iter().all(|i| !i.authorizes_edits()));
}
