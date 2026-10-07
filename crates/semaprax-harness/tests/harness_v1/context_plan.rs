//! HN-13: task-relevant context planning (fixture prefix `hp-hp05`, shared with
//! the parent module). Deterministic cases use a counting token-search provider
//! and the compiler named by `$SEMAPRAX_COMPILER` (a fake only when unset);
//! `#[ignore]` cases run the real compiler and the shipped source-index adapter
//! (and real Graft when `HARNESS_GRAFT`/`HARNESS_NODE` are given).

use super::*;
use semaprax_harness::context::external::RawEdge;
use semaprax_harness::context::plan::{self, expand_handle, parse_handle, PlanStep};
use semaprax_harness::workflow::lineage::Lineage;
use semaprax_harness::workflow::stages::{
    ContextRequest, ContextStage, ExternalContext, StageFailure, Task,
};
use semaprax_harness::workflow::{BrokerContext, Snapshot as WfSnapshot};

const FAKE_ID: &str = "org.example/fake";
const MARKER: &str = "browser shell that wraps the add export";

#[derive(Clone, Default)]
struct Knobs {
    queries: Arc<AtomicUsize>,
    seen: Arc<Mutex<Vec<ExternalQuery>>>,
    fail: Arc<AtomicBool>,
    partial: Arc<AtomicBool>,
    dup: Arc<AtomicBool>,
}

fn lang_of(rel: &str) -> &'static str {
    match rel.rsplit('.').next().unwrap_or("") {
        "ts" => "typescript",
        "rs" => "rust",
        "spx" => "semaprax",
        "md" => "markdown",
        _ => "text",
    }
}

fn idents(s: &str) -> Vec<String> {
    s.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

/// Identifier-token search over the snapshot, ranked by hit count (top-N only).
struct Src {
    k: Knobs,
}

impl ExternalSource for Src {
    fn identity(&self) -> ProviderIdentity {
        ProviderIdentity {
            provider_id: FAKE_ID.into(),
            provider_version: "1.0.0".into(),
            adapter_version: "0.1.0".into(),
            upstream_version: None,
            descriptor_digest: "sha256:d".into(),
            config_digest: "sha256:c".into(),
            permission_scope: json!({"read": ["project"]}),
        }
    }
    fn scope(&self) -> Vec<String> {
        vec![]
    }
    fn recheck_authority(&self) -> HarnessResult<()> {
        Ok(())
    }
    fn query(
        &self,
        snap: &Snapshot,
        q: &ExternalQuery,
        _max: usize,
    ) -> HarnessResult<ExternalResponse> {
        self.k.queries.fetch_add(1, Ordering::SeqCst);
        self.k.seen.lock().unwrap().push(q.clone());
        if self.k.fail.load(Ordering::SeqCst) {
            return Err(HarnessDiagnostic::new("SPX-HPE030", "provider crashed"));
        }
        let want = idents(
            q.payload["query"]
                .as_str()
                .or(q.payload["symbol"].as_str())
                .unwrap_or(""),
        );
        let cap = q.payload["max_items"].as_u64().unwrap_or(20) as usize;
        let mut hits: Vec<(u64, RawItem)> = Vec::new();
        for rel in snap.files.keys() {
            let Ok(t) = std::fs::read_to_string(snap.root.join(rel)) else {
                continue;
            };
            for (i, l) in t.split('\n').enumerate() {
                let toks = idents(l);
                let n = want.iter().filter(|w| toks.contains(w)).count() as u64;
                if n == 0 {
                    continue;
                }
                let item = RawItem {
                    path: rel.clone(),
                    span: Span {
                        start_line: i as u64 + 1,
                        end_line: i as u64 + 1,
                    },
                    digest: sha256_plain(l.as_bytes()),
                    tier: Tier::Structural,
                    language: lang_of(rel).into(),
                    rank: n as f64,
                    text: Some(l.to_string()),
                    span_kind: Some("start-line".into()),
                    edges: vec![],
                };
                if self.k.dup.load(Ordering::SeqCst) {
                    hits.push((n, item.clone()));
                }
                hits.push((n, item));
            }
        }
        hits.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.path.cmp(&b.1.path)));
        hits.truncate(cap);
        let partial = self.k.partial.load(Ordering::SeqCst);
        let mut cov = complete();
        if partial {
            cov.complete = false;
            cov.exhaustive = false;
            cov.skipped = vec![("docs/diagram.xyzlang".into(), "unsupported".into())];
        }
        Ok(ExternalResponse {
            status: if partial { "partial" } else { "complete" }.into(),
            no_references: hits.is_empty(),
            items: hits.into_iter().map(|h| h.1).collect(),
            coverage: cov,
            upstream_version: None,
            provider_id: FAKE_ID.into(),
            diagnostics: vec![],
        })
    }
}

fn stage(k: &Knobs, cache: Option<&Path>) -> BrokerContext {
    let cache =
        cache.map(|c| ResultCache::new(c.to_path_buf(), CacheConfig::default(), system_clock()));
    BrokerContext::with_sources(
        Some(native()),
        Some(native()),
        Box::new(Src { k: k.clone() }),
        FAKE_ID.into(),
        cache,
    )
    .unwrap()
}

struct Ask {
    lineage: Lineage,
    project: PathBuf,
}

impl Ask {
    fn new(project: &Path) -> Self {
        let snap = WfSnapshot::capture(project).unwrap();
        let t = Task::default();
        Ask {
            lineage: Lineage::new(snap.binding(), "sha256:lock", &t.digest()),
            project: project.to_path_buf(),
        }
    }
    fn req(&self, max: usize) -> ContextRequest<'_> {
        ContextRequest {
            lineage: &self.lineage,
            project: self.project.clone(),
            seed: Some("calculator.add"),
            query: String::new(),
            max_bytes: max,
            external: ExternalContext::WhenNeeded,
        }
    }
}

fn plan_for(p: &Path, goal: &str) -> plan::RetrievalPlan {
    plan::plan(p, goal, Some("calculator.add"), &[])
}

const TS_GOAL: &str = "keep the TypeScript renderAdd wrapper in step with add";

fn texts(p: &semaprax_harness::workflow::stages::ContextPacket, prov: &str) -> String {
    p.items
        .iter()
        .filter(|i| i.provenance.starts_with(prov))
        .map(|i| format!("{} {}", i.label, i.text))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn hp_hn13_foreign_boundary_is_retrieved_although_native_facts_are_complete() {
    let p = fake_world_project();
    let k = Knobs::default();
    let mut s = stage(&k, None);
    let rp = plan_for(&p, TS_GOAL);
    assert!(
        rp.needs.needs_provider() && !rp.needs.spx_local(),
        "{:?}",
        rp.needs
    );
    assert!(rp.needs.foreign.iter().any(|f| f.language == "typescript"));
    let ask = Ask::new(&p);
    let pk = s
        .collect_planned(&ask.req(12_000), rp.initial.as_ref().unwrap())
        .unwrap();
    // Native facts are whole: traversal completeness alone would have stopped here.
    assert!(pk.complete);
    let native = texts(&pk, "compiler-verified");
    assert!(native.contains("calculator.add"), "{native}");
    let ext = texts(&pk, "external:");
    assert!(ext.contains("web/app.ts") && ext.contains(MARKER), "{ext}");
    assert!(!ext.contains(".spx"), "the compiler owns .spx: {ext}");
    assert_eq!(k.queries.load(Ordering::SeqCst), 1, "one smallest query");
    let rep = s.take_plan_report().unwrap();
    assert_eq!(rep["ranked"], true);
    assert_eq!(rep["absence_provable"], false);
    assert_eq!(rep["provider_invocations"], 1);
}

#[test]
fn hp_hn13_goal_participates_but_pure_spx_edit_plans_no_provider_call() {
    let p = fake_world_project();
    let rp = plan_for(&p, "make add commute its operands");
    assert!(
        rp.initial.is_none() && rp.needs.spx_local(),
        "{:?}",
        rp.needs
    );
    assert_eq!(rp.needs.symbols, ["calculator.add"]);
    // The same symbol is a TypeScript export; an interface-changing goal needs the boundary.
    let rp = plan_for(&p, "rename the parameters in the add signature");
    assert!(
        rp.needs
            .foreign
            .iter()
            .any(|f| f.reason == "manifest-export"),
        "{:?}",
        rp.needs
    );
    // Config and explicit foreign paths are needs too; unresolved names come from diagnostics.
    let rp = plan_for(&p, "update web/app.ts and the toml config");
    assert!(rp.needs.explicit_paths.contains(&"web/app.ts".to_string()));
    assert!(rp.needs.config.contains(&"toml".to_string()));
    let rp = plan::plan(
        &p,
        "fix it",
        Some("calculator.add"),
        &[("SPX-X".into(), "unresolved name `hostGlue`".into())],
    );
    assert_eq!(rp.needs.unresolved, ["hostGlue"]);
    assert!(rp.initial.unwrap().query.contains("hostGlue"));
}

#[test]
fn hp_hn13_failed_candidate_triggers_exactly_one_focused_follow_up() {
    let p = fake_world_project();
    let k = Knobs::default();
    let mut s = stage(&k, None);
    let rp = plan_for(&p, TS_GOAL);
    let ask = Ask::new(&p);
    s.collect_planned(&ask.req(12_000), rp.initial.as_ref().unwrap())
        .unwrap();
    assert_eq!(k.queries.load(Ordering::SeqCst), 1);
    // A failure naming nothing new makes no call.
    assert!(s
        .follow_up(&ask.req(12_000), "add failed")
        .unwrap()
        .is_none());
    assert_eq!(k.queries.load(Ordering::SeqCst), 1);
    let f = s
        .follow_up(
            &ask.req(12_000),
            "type error: unknown name `Widget` in renderAdd",
        )
        .unwrap()
        .expect("one focused follow-up");
    assert_eq!(k.queries.load(Ordering::SeqCst), 2);
    let q = k.seen.lock().unwrap()[1].clone();
    assert_eq!(q.op, "search");
    assert!(q.payload["query"].as_str().unwrap().contains("Widget"));
    assert!(
        q.payload["max_items"].as_u64().unwrap() <= 8,
        "not a full dump: {q:?}"
    );
    assert!(f.items.iter().all(|i| i.provenance != "compiler-verified"));
    // The bound is spent: a second failure cannot widen the search.
    assert!(s
        .follow_up(&ask.req(12_000), "boom `Gadget`")
        .unwrap()
        .is_none());
    assert_eq!(k.queries.load(Ordering::SeqCst), 2);
}

#[test]
fn hp_hn13_unmet_need_gets_one_goal_worded_retry_then_an_explicit_unknown() {
    let p = fake_world_project();
    let k = Knobs::default();
    let mut s = stage(&k, None);
    // Language need is Go; the fixture has no Go, so nothing can satisfy it.
    let step = PlanStep {
        label: "initial",
        query: "calculator.add".into(),
        max_items: 3,
        want_languages: vec!["go".into()],
        fallback_query: Some("renderAdd".into()),
        references: false,
        exhaustive: false,
        symbol: Some("calculator.add".into()),
    };
    s.collect_planned(&Ask::new(&p).req(12_000), &step).unwrap();
    assert_eq!(
        k.queries.load(Ordering::SeqCst),
        2,
        "initial + one retry, never more"
    );
    let rep = s.take_plan_report().unwrap();
    let unk = rep["unknowns"].to_string();
    assert!(unk.contains("cannot prove that none exists"), "{unk}");
    assert!(rep["max_provider_calls"].as_u64().unwrap() >= 2);
}

#[test]
fn hp_hn13_ranked_search_never_proves_absence_and_exhaustive_requests_decline_or_cover() {
    let p = fake_world_project();
    let ask = Ask::new(&p);
    let goal = "find all callers of calculator.add";
    let rp = plan_for(&p, goal);
    let step = rp.initial.clone().unwrap();
    assert!(step.references && step.exhaustive);
    // Provider with partial coverage: explicit decline, no absence.
    let k = Knobs::default();
    k.partial.store(true, Ordering::SeqCst);
    let mut s = stage(&k, None);
    s.collect_planned(&ask.req(12_000), &step).unwrap();
    let q = k.seen.lock().unwrap()[0].clone();
    assert_eq!(
        (q.op.as_str(), q.payload["exhaustive"].clone()),
        ("references", json!(true))
    );
    let rep = s.take_plan_report().unwrap();
    assert_eq!(rep["exhaustive"], false);
    assert_eq!(rep["absence_provable"], false);
    assert!(rep["unknowns"].to_string().contains("declined"), "{rep}");
    // Complete, exhaustive, verified coverage: scoped coverage is established.
    let k = Knobs::default();
    let mut s = stage(&k, None);
    s.collect_planned(&ask.req(60_000), &step).unwrap();
    let rep = s.take_plan_report().unwrap();
    assert_eq!(rep["exhaustive"], true, "{rep}");
    assert_eq!(rep["ranked"], false);
    // A ranked search that finds nothing is still not an absence proof.
    let k = Knobs::default();
    let mut s = stage(&k, None);
    let none = PlanStep {
        label: "initial",
        query: "zzzNothingHere".into(),
        max_items: 5,
        want_languages: vec![],
        fallback_query: None,
        references: false,
        exhaustive: false,
        symbol: None,
    };
    let pk = s.collect_planned(&ask.req(12_000), &none).unwrap();
    assert!(pk.items.iter().all(|i| i.provenance == "compiler-verified"));
    let rep = s.take_plan_report().unwrap();
    assert_eq!(
        (rep["ranked"].clone(), rep["absence_provable"].clone()),
        (json!(true), json!(false))
    );
    assert!(rep["unknowns"].to_string().contains("cannot prove"));
}

#[test]
fn hp_hn13_duplicate_slices_are_sent_once_and_task_filtering_is_counted() {
    let p = fake_world_project();
    let k = Knobs::default();
    k.dup.store(true, Ordering::SeqCst);
    let mut s = stage(&k, None);
    let rp = plan_for(&p, TS_GOAL);
    let pk = s
        .collect_planned(&Ask::new(&p).req(30_000), rp.initial.as_ref().unwrap())
        .unwrap();
    let mut labels: Vec<_> = pk
        .items
        .iter()
        .map(|i| (i.label.clone(), i.provenance.clone()))
        .collect();
    let n = labels.len();
    labels.sort();
    labels.dedup();
    assert_eq!(labels.len(), n, "exact slice+provenance appears once");
    let hits = pk
        .items
        .iter()
        .filter(|i| i.label.starts_with("web/app.ts:"))
        .count();
    assert!(hits >= 1);
    // Same input, same output: determinism.
    let k2 = Knobs::default();
    k2.dup.store(true, Ordering::SeqCst);
    let mut s2 = stage(&k2, None);
    let again = s2
        .collect_planned(&Ask::new(&p).req(30_000), rp.initial.as_ref().unwrap())
        .unwrap();
    assert_eq!(pk, again);
}

#[test]
fn hp_hn13_cache_binds_worktree_and_source_content_and_skips_unchanged_queries() {
    let p = fake_world_project();
    let cache = p.parent().unwrap().join("cache");
    let k = Knobs::default();
    let rp = plan_for(&p, TS_GOAL);
    let step = rp.initial.clone().unwrap();
    let mut s = stage(&k, Some(&cache));
    s.collect_planned(&Ask::new(&p).req(12_000), &step).unwrap();
    let mut s = stage(&k, Some(&cache));
    s.collect_planned(&Ask::new(&p).req(12_000), &step).unwrap();
    assert_eq!(
        k.queries.load(Ordering::SeqCst),
        1,
        "same content and worktree: cache hit"
    );
    assert_eq!(s.calls(), 0, "a hit is not a provider invocation");
    assert_eq!(s.take_plan_report().unwrap()["cache_hits"], 1);
    // Source drift: a dirty edit changes the key.
    std::fs::write(p.join("web/app.ts"), "export const x = 1; // add\n").unwrap();
    let mut s = stage(&k, Some(&cache));
    s.collect_planned(&Ask::new(&p).req(12_000), &step).unwrap();
    assert_eq!(k.queries.load(Ordering::SeqCst), 2);
    // A second worktree of identical content is a different binding.
    let other = p.parent().unwrap().join("project2");
    copy_tree(&p, &other);
    let mut s = stage(&k, Some(&cache));
    s.collect_planned(&Ask::new(&other).req(12_000), &step)
        .unwrap();
    assert_eq!(k.queries.load(Ordering::SeqCst), 3);
}

#[test]
fn hp_hn13_provider_failure_keeps_native_facts_and_states_the_unknown() {
    let p = fake_world_project();
    let k = Knobs::default();
    k.fail.store(true, Ordering::SeqCst);
    let mut s = stage(&k, None);
    let rp = plan_for(&p, TS_GOAL);
    let pk = s
        .collect_planned(&Ask::new(&p).req(12_000), rp.initial.as_ref().unwrap())
        .unwrap();
    assert!(texts(&pk, "compiler-verified").contains("calculator.add"));
    assert!(texts(&pk, "external:").is_empty());
    assert!(
        !pk.complete,
        "native facts are valid but never labelled complete"
    );
    assert!(s.take_note().unwrap().contains("SPX-HPE030"));
    let rep = s.take_plan_report().unwrap();
    assert!(rep["unknowns"]
        .to_string()
        .contains("repository provider failed"));
    assert_eq!(
        k.queries.load(Ordering::SeqCst),
        1,
        "no retry of a failed provider"
    );
}

#[test]
fn hp_hn13_exhausted_budget_omits_whole_items_and_handles_expand_without_a_provider_call() {
    let p = fake_world_project();
    let k = Knobs::default();
    let mut s = stage(&k, None);
    let step = PlanStep {
        label: "initial",
        query: "left right".into(),
        max_items: 24,
        want_languages: vec![],
        fallback_query: None,
        references: false,
        exhaustive: false,
        symbol: Some("calculator.add".into()),
    };
    let ask = Ask::new(&p);
    let pk = s.collect_planned(&ask.req(2_600), &step).unwrap();
    let rep = s.take_plan_report().unwrap();
    assert!(rep["omitted_items"].as_u64().unwrap() > 0, "{rep}");
    assert!(rep["unknowns"]
        .to_string()
        .contains("omitted by the byte budget"));
    let handles: Vec<String> = rep["continuation"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h.as_str().unwrap().to_string())
        .collect();
    assert!(!handles.is_empty() && handles.len() <= 8);
    let before = k.queries.load(Ordering::SeqCst);
    let h = handles[0].clone();
    let parsed = parse_handle(&h).unwrap();
    let got = s.expand(&ask.req(2_600), &h).unwrap();
    assert_eq!(
        k.queries.load(Ordering::SeqCst),
        before,
        "expansion is local"
    );
    assert_eq!(got.items.len(), 1);
    assert_eq!(
        got.items[0].label,
        format!("{}:{}-{}", parsed.path, parsed.start, parsed.end)
    );
    assert!(got.items[0].provenance.starts_with("external:"));
    // Asking again adds nothing (deduplicated); unknown handles are refused.
    assert!(s.expand(&ask.req(2_600), &h).unwrap().items.is_empty());
    let StageFailure::Refused(e) = s
        .expand(&ask.req(2_600), "ctx:x:a#1-1@sha256:00")
        .unwrap_err()
    else {
        panic!("refused")
    };
    assert_eq!(e.code, "SPX-HPD130");
    // Drift after issue: the slice no longer hashes to the handle (checked before dedup).
    std::fs::write(p.join(&parsed.path), "changed\n").unwrap();
    let StageFailure::Refused(e) = s.expand(&ask.req(2_600), &h).unwrap_err() else {
        panic!("refused")
    };
    assert_eq!(e.code, "SPX-HPE071");
    // Path safety and size bounds are deterministic.
    let mut bad = parsed.clone();
    bad.path = "../outside.txt".into();
    assert_eq!(
        expand_handle(&p, &bad, 1000).unwrap_err().code,
        "SPX-HPE073"
    );
    let mut gone = parsed.clone();
    gone.path = "no/such/file.ts".into();
    assert_eq!(
        expand_handle(&p, &gone, 1000).unwrap_err().code,
        "SPX-HPE071"
    );
    let _ = pk;
}

#[test]
fn hp_hn13_mandatory_native_facts_beat_optional_ranking_under_a_tight_budget() {
    let p = fake_world_project();
    let k = Knobs::default();
    let mut s = stage(&k, None);
    let rp = plan_for(&p, TS_GOAL);
    let pk = s
        .collect_planned(&Ask::new(&p).req(2_048), rp.initial.as_ref().unwrap())
        .unwrap();
    assert!(
        pk.items.iter().any(|i| i.provenance == "compiler-verified"),
        "native contracts/effects/ownership facts survive any budget"
    );
}

#[test]
fn hp_hn13_edges_and_span_kind_round_trip_through_raw_items_and_cache_json() {
    let it = RawItem {
        path: "web/app.ts".into(),
        span: Span {
            start_line: 1,
            end_line: 1,
        },
        digest: "sha256:aa".into(),
        tier: Tier::Structural,
        language: "typescript".into(),
        rank: 1.0,
        text: None,
        span_kind: Some("definition".into()),
        edges: vec![RawEdge {
            target: "src/host.rs".into(),
            relation: "calls".into(),
            tier: Tier::Inferred,
            resolution: Some("ambiguous".into()),
        }],
    };
    let r = ExternalResponse {
        status: "complete".into(),
        items: vec![it],
        coverage: complete(),
        no_references: false,
        upstream_version: None,
        provider_id: FAKE_ID.into(),
        diagnostics: vec![],
    };
    assert_eq!(ExternalResponse::from_json(&r.to_json()).unwrap(), r);
}

// ---- real compiler + shipped source-index adapter ------------------------------

fn compiler() -> PathBuf {
    real_compiler().expect("provisioned test requires SEMAPRAX_COMPILER")
}

struct Counting {
    inner: HostExternal,
    n: Arc<AtomicUsize>,
}

impl ExternalSource for Counting {
    fn identity(&self) -> ProviderIdentity {
        self.inner.identity()
    }
    fn scope(&self) -> Vec<String> {
        self.inner.scope()
    }
    fn recheck_authority(&self) -> HarnessResult<()> {
        self.inner.recheck_authority()
    }
    fn query(
        &self,
        snap: &Snapshot,
        q: &ExternalQuery,
        max: usize,
    ) -> HarnessResult<ExternalResponse> {
        self.n.fetch_add(1, Ordering::SeqCst);
        self.inner.query(snap, q, max)
    }
}

fn real_stage(w: &World) -> (BrokerContext, Arc<AtomicUsize>) {
    let res = resolve_project(&w.env, &w.project).expect("resolve");
    let cfg = HarnessConfig::load(&w.project).unwrap();
    let l = res
        .launches
        .get(&CapabilityKind::ContextRepository)
        .expect("selected provider")
        .clone();
    let n = Arc::new(AtomicUsize::new(0));
    let ext = Counting {
        inner: HostExternal::new(
            l.clone(),
            w.env.clone(),
            res.profile.lock_digest(),
            res.profile.config_digest.clone(),
            cfg.capability(CapabilityKind::ContextRepository).scope,
        ),
        n: n.clone(),
    };
    let nat = || Some(Box::new(SubprocessNative::new(compiler())) as Box<dyn NativeContextSource>);
    (
        BrokerContext::with_sources(nat(), nat(), Box::new(ext), l.provider_id, None).unwrap(),
        n,
    )
}

mod pipeline {
    use super::*;
    use semaprax_harness::observe::{Observer, ObserverLimits};
    use semaprax_harness::workflow::compiler::SubprocessCompiler;
    use semaprax_harness::workflow::stages::{
        NativeContext, ProposalRequest, ProposalStage, RawCommandView, TaskMode,
    };
    use semaprax_harness::workflow::{run, Composition, RunConfig, Stages};
    use std::cell::Cell;

    /// Proposes the valid body only when the boundary file's text is in the prompt.
    struct NeedsBoundary {
        saw: Cell<bool>,
        calls: Cell<u32>,
    }

    impl ProposalStage for NeedsBoundary {
        fn id(&self) -> String {
            "org.example/needs-boundary".into()
        }
        fn propose(&mut self, r: &ProposalRequest) -> Result<Vec<u8>, StageFailure> {
            self.calls.set(self.calls.get() + 1);
            let seen = r.prompt["context"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|c| {
                    c["provenance"]
                        .as_str()
                        .is_some_and(|p| p.starts_with("external:"))
                        && c["label"]
                            .as_str()
                            .is_some_and(|l| l.starts_with("web/app.ts"))
                        && c["text"].as_str().is_some_and(|t| t.contains(MARKER))
                });
            self.saw.set(seen);
            let (l, r_) = if seen {
                ("right", "left")
            } else {
                ("right", "nothing")
            };
            Ok(json!({"schema": "semaprax.harness-proposal.v1", "intent": {
                "kind": "replace_function_body", "target": "calculator.add",
                "body": {"kind": "binary", "op": "+",
                         "left": {"kind": "place", "name": l},
                         "right": {"kind": "place", "name": r_}}}})
            .to_string()
            .into_bytes())
        }
        fn calls(&self) -> u32 {
            self.calls.get()
        }
        fn side_effecting(&self) -> bool {
            false
        }
    }

    fn drive(
        w: &World,
        goal: &str,
        ext: ExternalContext,
        stage: &mut BrokerContext,
    ) -> (semaprax_harness::workflow::Report, bool) {
        let project = w.project.canonicalize().unwrap();
        let cache = w.home.parent().unwrap().join("run-cache");
        std::fs::create_dir_all(&cache).unwrap();
        let cfg = RunConfig {
            snapshot: WfSnapshot::capture(&project).unwrap(),
            task: Task {
                schema_version: 2,
                mode: TaskMode::Change,
                goal: goal.into(),
                seed: Some("calculator.add".into()),
                external_context: ext,
                ..Task::default()
            },
            context_max_bytes: 16_384,
            cache_dir: cache.clone(),
            lock_digest: "sha256:lock".into(),
            providers: vec![],
            composition: Composition::from_profile(None, true, vec![], &[]).unwrap(),
            apply_policy: None,
            checks: vec![],
            skill_prompt: None,
            endpoint_policy: Default::default(),
            model_plans: None,
            notes: vec![],
            budget: Default::default(),
            cancel: None,
            routing: Default::default(),
            context_target: None,
        };
        let svc = SubprocessCompiler::new(compiler(), cache.join("scratch")).unwrap();
        let mut native = NativeContext::new(&svc);
        let mut prop = NeedsBoundary {
            saw: Cell::new(false),
            calls: Cell::new(0),
        };
        let mut view = RawCommandView;
        let mut obs = Observer::new(None, ObserverLimits::default());
        let r = run(
            &cfg,
            &svc,
            Stages {
                decision: None,
                native: &mut native,
                external: Some(stage),
                proposer: &mut prop,
                command: &mut view,
            },
            &mut obs,
        );
        (r, prop.saw.get())
    }

    #[test]
    #[ignore = "provisioned: needs SEMAPRAX_COMPILER"]
    fn sg15_real_when_needed_table_and_frozen_both_forward_boundary() {
        for table in [false, true] {
            let w = world();
            if table {
                let manifest = semaprax::project::ProjectManifest::parse(
                    &std::fs::read_to_string(w.project.join("semaprax.toml")).unwrap(),
                )
                .unwrap();
                let sources = manifest
                    .sources()
                    .iter()
                    .map(|p| format!("\"{p}\""))
                    .collect::<Vec<_>>()
                    .join(", ");
                let exports = manifest
                    .web_exports()
                    .iter()
                    .map(|p| format!("\"{p}\""))
                    .collect::<Vec<_>>()
                    .join(", ");
                write(&w.project, "semaprax.toml", &format!("schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"calculator\"\nversion = \"0.1.0\"\n\n[modules]\nentry = \"calculator.app\"\nsources = [{sources}]\ntests = [\"calculator.tests\"]\n\n[exports]\nweb = [{exports}]\n"));
            }
            let k = Knobs::default();
            let mut external = stage(&k, None);
            let (report, saw) = drive(
                &w,
                "rename the parameters in the add signature",
                ExternalContext::WhenNeeded,
                &mut external,
            );
            assert!(saw, "proposer needs boundary context: {report:?}");
            assert_eq!(k.queries.load(Ordering::SeqCst), 1);
        }
    }

    #[test]
    #[ignore = "provisioned: needs SEMAPRAX_COMPILER"]
    fn hp_hn13_real_mixed_task_completes_only_because_the_boundary_file_was_retrieved() {
        let w = world();
        install_source_index(&w);
        let (mut s, n) = real_stage(&w);
        let (r, saw) = drive(&w, TS_GOAL, ExternalContext::WhenNeeded, &mut s);
        assert!(saw, "the proposer received web/app.ts content");
        assert_eq!(
            r.status, "candidate-ready",
            "{:?} {:?}",
            r.refusals, r.notes
        );
        assert_eq!(n.load(Ordering::SeqCst), 1, "one provider query");
        assert_eq!(r.context["plan"]["needs"]["spx_local"], false);
        assert_eq!(r.context["plan"]["provider_consulted"], true);
        assert_eq!(r.context["plan"]["retrieval"]["ranked"], true);
        assert_eq!(r.context["plan"]["retrieval"]["absence_provable"], false);
        assert!(r.context["plan"]["planned_steps"][0]["query_digest"].is_string());
        assert!(
            !r.context.to_string().contains("TypeScript renderAdd"),
            "goal text stays out of reports"
        );
        // Control: with external context off the same proposer cannot finish.
        let w2 = world();
        install_source_index(&w2);
        let (mut s2, n2) = real_stage(&w2);
        let (r2, saw2) = drive(&w2, TS_GOAL, ExternalContext::Never, &mut s2);
        assert!(!saw2);
        assert_ne!(r2.status, "candidate-ready", "{:?}", r2.status);
        assert_eq!(n2.load(Ordering::SeqCst), 0);
        assert!(r2.context["plan"]["unknowns"].to_string().contains("never"));
    }

    #[test]
    #[ignore = "provisioned: needs SEMAPRAX_COMPILER"]
    fn hp_hn13_real_pure_local_spx_edit_makes_zero_provider_calls() {
        let w = world();
        install_source_index(&w);
        let (mut s, n) = real_stage(&w);
        let (r, saw) = drive(
            &w,
            "make add commute its operands",
            ExternalContext::WhenNeeded,
            &mut s,
        );
        assert!(!saw);
        assert_eq!(
            n.load(Ordering::SeqCst),
            0,
            "zero repository-provider calls"
        );
        assert_eq!(s.calls(), 0);
        assert_eq!(r.external_calls, 0);
        assert_eq!(r.context["plan"]["needs"]["spx_local"], true);
        assert_eq!(r.context["plan"]["provider_consulted"], false);
    }
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER HARNESS_GRAFT HARNESS_NODE"]
fn hp_hn13_real_graft_retrieves_the_typescript_boundary_for_a_native_complete_task() {
    let w = world();
    let up = crate::support::required_tool("HARNESS_GRAFT");
    crate::support::required_tool("HARNESS_NODE");
    let mut w = w;
    w.env.vars.insert(
        "HARNESS_NODE".into(),
        std::env::var("HARNESS_NODE").unwrap(),
    );
    let d = crate::support::repo_root()
        .join("packages/semaprax-harness-adapters/graft/harness-provider.json");
    let o = sh(
        &w,
        &[
            "adopt",
            d.to_str().unwrap(),
            "--upstream",
            up.to_str().unwrap(),
        ],
    );
    assert_eq!(o.code, 0, "adopt: {}{}", o.stdout, o.stderr);
    let o = sh(&w, &["trust", "org.nanonets/graft-context"]);
    assert_eq!(o.code, 0, "trust: {}{}", o.stdout, o.stderr);
    write(
        &w.project,
        "semaprax.harness.toml",
        "schema = \"semaprax.harness-config.v1\"\n\n[capability.\"context.repository\"]\nmode = \"required\"\nprovider = \"org.nanonets/graft-context\"\n",
    );
    let (mut s, n) = real_stage(&w);
    let rp = plan_for(&w.project, TS_GOAL);
    let pk = s
        .collect_planned(
            &Ask::new(&w.project).req(16_000),
            rp.initial.as_ref().unwrap(),
        )
        .unwrap();
    let ext = texts(&pk, "external:");
    assert!(ext.contains("web/app.ts"), "{ext}");
    assert!(n.load(Ordering::SeqCst) >= 1);
    assert!(texts(&pk, "compiler-verified").contains("calculator.add"));
}

#[test]
fn sg15_table_manifest_and_frozen_inventory_plan_the_same_boundary() {
    let p = fake_world_project();
    let manifest = semaprax::project::ProjectManifest::parse(
        &std::fs::read_to_string(p.join("semaprax.toml")).unwrap(),
    )
    .unwrap();
    let goal = "rename the parameters in the add signature";
    let frozen = plan_for(&p, goal);
    let sources = manifest
        .sources()
        .iter()
        .map(|p| format!("\"{p}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let exports = manifest
        .web_exports()
        .iter()
        .map(|p| format!("\"{p}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let table = format!("schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"calculator\"\nversion = \"0.1.0\"\n\n[modules]\nentry = \"calculator.app\"\nsources = [{sources}]\ntests = [\"calculator.tests\"]\n\n[exports]\nweb = [{exports}]\n");
    write(&p, "semaprax.toml", &table);
    let table_plan = plan_for(&p, goal);
    assert_eq!(table_plan.needs, frozen.needs);
    let k = Knobs::default();
    let mut s = stage(&k, None);
    let ask = Ask::new(&p);
    let packet = s
        .collect_planned(&ask.req(12_000), table_plan.initial.as_ref().unwrap())
        .unwrap();
    assert_eq!(k.queries.load(Ordering::SeqCst), 1);
    assert!(texts(&packet, "external:").contains(MARKER));
    assert!(plan_for(&p, "make add commute its operands")
        .initial
        .is_none());
    write(&p, "semaprax.toml", "schema = \"unknown\"\n");
    assert!(!plan_for(&p, goal).needs.unresolved.is_empty());
}
