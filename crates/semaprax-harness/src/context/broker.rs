//! Native-first context broker. The compiler supplies the mandatory `.spx`
//! facts; at most one external provider per scope (or explicitly federated
//! disjoint scopes) supplies surrounding discovery. External relationships
//! stay hints: nothing here promotes them to compiler facts.

use super::budget::{fit, Parts, RefInputs};
use super::cache::{CacheKey, ResultCache};
use super::external::{
    Coverage, ExternalQuery, ExternalResponse, ExternalSource, ProviderIdentity,
};
use super::identity::{scan_spx, span_digest, Snapshot, SpxDecl};
use super::item::{ContextItem, Link, Span, Tier};
use super::native::{NativeContextSource, NativeQuery, COMPILER_MIN_BYTES};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;

const NATIVE_PROVIDER: &str = "semaprax.compiler";
const MAX_TARGETS: usize = 4;
const MAX_RESOLUTIONS: usize = 8;
const MAX_CANDIDATES: usize = 200;

#[derive(Clone, Debug)]
pub struct BrokerRequest {
    pub query: String,
    pub symbol: Option<String>,
    pub references: bool,
    pub max_bytes: usize,
    /// Explicit compiler targets (symbol or stable id); derived from the query when `None`.
    pub native_targets: Option<Vec<String>>,
    pub native_depth: u32,
    pub native_filters: Vec<String>,
    pub max_items: u64,
    /// Ask the provider for scoped exhaustive coverage (references only).
    pub exhaustive: bool,
    /// Project-relative path prefix the provider search is limited to.
    pub within: Option<String>,
    /// File extensions (no dot) the task does not want from the provider, for
    /// example `spx` when the compiler already owns those facts. Counted, never silent.
    pub exclude_extensions: Vec<String>,
}

impl BrokerRequest {
    pub fn new(query: &str, max_bytes: usize) -> Self {
        Self {
            query: query.to_string(),
            symbol: None,
            references: false,
            max_bytes,
            native_targets: None,
            native_depth: 1,
            native_filters: ["contracts", "ownership", "effects", "types"]
                .map(String::from)
                .to_vec(),
            max_items: 20,
            exhaustive: false,
            within: None,
            exclude_extensions: vec![],
        }
    }
}

#[derive(Clone, Debug)]
pub struct BrokerOutput {
    pub rendered: String,
    pub native: Vec<ContextItem>,
    pub external: Vec<ContextItem>,
    pub omitted: usize,
    pub exhaustive: bool,
    pub definitive_absence: bool,
    /// Per provider: was the response served from cache (side channel, not in the document).
    pub cache_hits: BTreeMap<String, bool>,
    pub snapshot: Snapshot,
}

pub struct Broker {
    native: Option<Box<dyn NativeContextSource>>,
    providers: Vec<Box<dyn ExternalSource>>,
    federated: bool,
    cache: Option<ResultCache>,
}

fn overlaps(a: &str, b: &str) -> bool {
    let (a, b) = (a.trim_end_matches('/'), b.trim_end_matches('/'));
    a == b || b.starts_with(&format!("{a}/")) || a.starts_with(&format!("{b}/"))
}

fn under(path: &str, scope: &[String]) -> bool {
    scope.is_empty()
        || scope.iter().any(|s| {
            path == s.trim_end_matches('/')
                || path.starts_with(&format!("{}/", s.trim_end_matches('/')))
        })
}

impl Broker {
    pub fn new(native: Option<Box<dyn NativeContextSource>>, cache: Option<ResultCache>) -> Self {
        Self {
            native,
            providers: Vec::new(),
            federated: false,
            cache,
        }
    }

    /// Opt in to several providers; each must then own a disjoint, explicit scope.
    pub fn enable_federation(&mut self) {
        self.federated = true;
    }

    pub fn add_provider(&mut self, p: Box<dyn ExternalSource>) -> HarnessResult<()> {
        if !self.providers.is_empty() {
            if !self.federated {
                return Err(HarnessDiagnostic::new(
                    "SPX-HPE040",
                    "one external repository provider per scope; enable federation with disjoint scopes to add another",
                ));
            }
            let scope = p.scope();
            if scope.is_empty() || self.providers.iter().any(|q| q.scope().is_empty()) {
                return Err(HarnessDiagnostic::new(
                    "SPX-HPE041",
                    "federated providers each need an explicit scope",
                ));
            }
            for q in &self.providers {
                if q.scope()
                    .iter()
                    .any(|a| scope.iter().any(|b| overlaps(a, b)))
                {
                    return Err(HarnessDiagnostic::new(
                        "SPX-HPE041",
                        format!(
                            "scope of `{}` overlaps `{}`; federation requires disjoint scopes",
                            p.identity().provider_id,
                            q.identity().provider_id
                        ),
                    ));
                }
            }
        }
        self.providers.push(p);
        Ok(())
    }

    /// Attach (or replace) the provider-result cache after construction.
    pub fn set_cache(&mut self, cache: Option<ResultCache>) {
        self.cache = cache;
    }

    pub fn cache(&self) -> Option<&ResultCache> {
        self.cache.as_ref()
    }

    fn derive_targets(&self, req: &BrokerRequest, decls: &[SpxDecl]) -> Vec<String> {
        if let Some(t) = &req.native_targets {
            return t.clone();
        }
        let mut toks: Vec<String> = Vec::new();
        for s in req
            .query
            .split(|c: char| c.is_whitespace() || c == ',')
            .chain(req.symbol.as_deref())
        {
            let s = s
                .trim_matches(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.' || c == '-'));
            if !s.is_empty() {
                toks.push(s.to_string());
            }
        }
        let mut out: Vec<String> = Vec::new();
        for d in decls {
            let last = d.id.rsplit('.').next().unwrap_or("");
            if toks.iter().any(|t| *t == d.id || *t == d.name || t == last) && !out.contains(&d.id)
            {
                out.push(d.id.clone());
            }
        }
        out.truncate(MAX_TARGETS);
        out
    }

    pub fn context(&self, project: &Path, req: &BrokerRequest) -> HarnessResult<BrokerOutput> {
        let snap = Snapshot::capture(project)?;
        let decls = scan_spx(&snap);
        let targets = self.derive_targets(req, &decls);

        // 1. Mandatory native facts, verbatim from the compiler.
        let mut native: Vec<ContextItem> = Vec::new();
        if !targets.is_empty() {
            let Some(src) = &self.native else {
                return Err(HarnessDiagnostic::new("SPX-HPE002", "native `.spx` facts are required but no compiler path was provided (--compiler or SEMAPRAX_COMPILER)"));
            };
            if req.max_bytes < COMPILER_MIN_BYTES {
                return Err(HarnessDiagnostic::new(
                    "SPX-HPE001",
                    format!("mandatory native facts need at least {COMPILER_MIN_BYTES} byte-v1 but the budget is {}", req.max_bytes),
                ));
            }
            let q = NativeQuery {
                depth: req.native_depth,
                max_bytes: req.max_bytes.min(16 * 1024),
                filters: req.native_filters.clone(),
            };
            for t in &targets {
                let scope = if snap.root.join("semaprax.toml").is_file() {
                    snap.root.clone()
                } else {
                    let d = decls
                        .iter()
                        .find(|d| d.id == *t || d.name == *t)
                        .ok_or_else(|| {
                            HarnessDiagnostic::new(
                                "SPX-HPE023",
                                format!(
                                    "`{t}` is not a declaration of any `.spx` file in the project"
                                ),
                            )
                        })?;
                    snap.root.join(&d.path)
                };
                let f = src.facts(&scope, t, &q)?;
                if native
                    .iter()
                    .any(|n| n.stable_id.as_deref() == Some(f.root.as_str()))
                {
                    continue;
                }
                native.push(native_item(
                    &snap,
                    &decls,
                    &f.root,
                    &f.raw,
                    f.truncated,
                    req,
                )?);
            }
        }

        // 2. External providers (cache + authority recheck on every read).
        let mut resolutions: BTreeMap<String, bool> = BTreeMap::new();
        let mut reports: Vec<Value> = Vec::new();
        let mut candidates: Vec<(String, ContextItem)> = Vec::new();
        let mut hits: BTreeMap<String, bool> = BTreeMap::new();
        let (mut all_exhaustive, mut all_none, mut all_complete) =
            (!self.providers.is_empty(), !self.providers.is_empty(), true);
        for src in &self.providers {
            let ident = src.identity();
            let query = external_query(req);
            let mut diags: Vec<Value> = Vec::new();
            let resp = self.fetch(src.as_ref(), &ident, &snap, &query, &mut hits, &mut diags);
            let Some(resp) = resp else {
                all_exhaustive = false;
                all_none = false;
                all_complete = false;
                reports.push(provider_report(
                    &ident.provider_id,
                    "unavailable",
                    None,
                    &Coverage::default(),
                    0,
                    0,
                    diags,
                ));
                continue;
            };
            let scope = src.scope();
            let mut normalized = normalize(&ident, resp, &mut diags);
            let offered = normalized.items.len();
            normalized.items.retain(|i| under(&i.path, &scope));
            let dropped = offered - normalized.items.len();
            let before = normalized.items.len();
            normalized
                .items
                .retain(|i| !excluded_by_task(&i.path, &req.exclude_extensions));
            let by_task = before - normalized.items.len();
            let before = normalized.items.len();
            dedup_exact(&mut normalized.items);
            let duplicates = before - normalized.items.len();
            if by_task + duplicates > 0 {
                diags.push(json!({"code": "SPX-HPE070", "message": format!("{by_task} item(s) outside the task's language needs and {duplicates} exact duplicate slice(s) not forwarded")}));
            }
            // Task-filtered items mean the answer is not the whole truth.
            let mut verified_all = by_task == 0;
            for raw in &normalized.items {
                let mut it = make_item(&snap, &ident.provider_id, raw);
                verified_all &= it.verified;
                link_spx(
                    &mut it,
                    &snap,
                    &decls,
                    self.native.as_deref(),
                    &mut resolutions,
                );
                if let Link::Resolved(id) = &it.link {
                    if native
                        .iter()
                        .any(|n| n.stable_id.as_deref() == Some(id.as_str()))
                    {
                        continue; // the compiler item already carries it: no duplicate
                    }
                }
                candidates.push((ident.provider_id.clone(), it));
            }
            let cov = &normalized.coverage;
            let exhaustive = normalized.status == "complete"
                && cov.complete
                && cov.exhaustive
                && verified_all
                && scope.is_empty();
            all_exhaustive &= exhaustive;
            all_none &= exhaustive && normalized.no_references && offered == 0;
            all_complete &= normalized.status == "complete" && cov.complete;
            reports.push(provider_report(
                &ident.provider_id,
                &normalized.status,
                normalized.upstream_version.as_deref(),
                cov,
                offered,
                dropped,
                diags,
            ));
        }
        candidates.truncate(MAX_CANDIDATES);
        dedup_across_providers(&mut candidates);

        // 3. Fit under the single byte budget: native first, then whole external items.
        let native_bytes: usize = native.iter().map(ContextItem::rendered_len).sum();
        let remaining = req.max_bytes.saturating_sub(native_bytes);
        let n = self.providers.len().max(1);
        let caps = self
            .providers
            .iter()
            .map(|p| (p.identity().provider_id, remaining / n))
            .collect();
        let parts = Parts {
            query: json!({"text": req.query, "symbol": req.symbol, "references": req.references}),
            ids: json!({"project_id": snap.project_id, "worktree_id": snap.worktree_id, "revision": snap.revision}),
            native_complete: native.iter().all(|n| n.complete),
            native: native.clone(),
            providers: reports,
            coverage_complete: all_complete,
            references: req.references.then(|| RefInputs {
                symbol: req.symbol.clone().unwrap_or_else(|| req.query.clone()),
                providers_exhaustive: all_exhaustive,
                providers_say_none: all_none,
            }),
            candidates,
            caps,
        };
        let fitted = fit(&parts, req.max_bytes)?;
        let value: Value = serde_json::from_str(&fitted.rendered).unwrap_or(Value::Null);
        let definitive_absence = value["references"]["definitive_absence"]
            .as_bool()
            .unwrap_or(false);
        Ok(BrokerOutput {
            rendered: fitted.rendered,
            native,
            external: fitted.selected,
            omitted: fitted.omitted,
            exhaustive: fitted.exhaustive,
            definitive_absence,
            cache_hits: hits,
            snapshot: snap,
        })
    }

    /// Authority first, then cache, then the provider. `None` = provider unusable.
    fn fetch(
        &self,
        src: &dyn ExternalSource,
        ident: &ProviderIdentity,
        snap: &Snapshot,
        q: &ExternalQuery,
        hits: &mut BTreeMap<String, bool>,
        diags: &mut Vec<Value>,
    ) -> Option<ExternalResponse> {
        if let Err(e) = src.recheck_authority() {
            if let Some(c) = &self.cache {
                c.purge_provider(&ident.provider_id);
            }
            diags.push(json!({"code": e.code, "message": e.message}));
            return None;
        }
        let key = CacheKey::new(snap, ident, q);
        if let Some(r) = self.cache.as_ref().and_then(|c| c.get(&key)) {
            hits.insert(ident.provider_id.clone(), true);
            return Some(r);
        }
        hits.insert(ident.provider_id.clone(), false);
        match src.query(snap, q, 1 << 20) {
            Ok(r) => {
                if matches!(r.status.as_str(), "complete" | "partial") {
                    if let Some(c) = &self.cache {
                        c.put(&key, &r);
                    }
                }
                Some(r)
            }
            Err(e) => {
                diags.push(json!({"code": e.code, "message": e.message}));
                None
            }
        }
    }
}

fn external_query(req: &BrokerRequest) -> ExternalQuery {
    let mut q = match (&req.symbol, req.references) {
        (Some(s), true) => ExternalQuery {
            op: "references".into(),
            payload: json!({"symbol": s, "max_items": req.max_items}),
        },
        (None, true) => ExternalQuery {
            op: "references".into(),
            payload: json!({"symbol": req.query, "max_items": req.max_items}),
        },
        _ if req.query.trim().is_empty() => ExternalQuery {
            op: "orient".into(),
            payload: json!({"max_items": req.max_items}),
        },
        _ => ExternalQuery {
            op: "search".into(),
            payload: json!({"query": req.query, "max_items": req.max_items}),
        },
    };
    if q.op == "references" && req.exhaustive {
        q.payload["exhaustive"] = json!(true);
    }
    if let (Some(w), "search" | "references") = (&req.within, q.op.as_str()) {
        q.payload["in"] = json!(w);
    }
    q
}

fn excluded_by_task(path: &str, exts: &[String]) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, e)| exts.iter().any(|x| x == e))
}

/// Same provider reporting the same slice twice: keep the first.
fn dedup_exact(items: &mut Vec<super::external::RawItem>) {
    let mut seen = std::collections::BTreeSet::new();
    items.retain(|i| {
        seen.insert((
            i.path.clone(),
            i.span.start_line,
            i.span.end_line,
            i.digest.clone(),
            i.tier,
        ))
    });
}

fn native_item(
    snap: &Snapshot,
    decls: &[SpxDecl],
    root: &str,
    raw: &str,
    truncated: bool,
    req: &BrokerRequest,
) -> HarnessResult<ContextItem> {
    let d = decls.iter().find(|d| d.id == root).ok_or_else(|| {
        HarnessDiagnostic::new(
            "SPX-HPE023",
            format!("compiler root `{root}` was not located in the project's `.spx` sources"),
        )
    })?;
    let text = std::fs::read_to_string(snap.root.join(&d.path)).unwrap_or_default();
    let digest = span_digest(&text, d.start_line, d.end_line).unwrap_or_default();
    Ok(ContextItem {
        project_id: snap.project_id.clone(),
        worktree_id: snap.worktree_id.clone(),
        revision: snap.revision.clone(),
        path: d.path.clone(),
        span: Span {
            start_line: d.start_line,
            end_line: d.end_line,
        },
        span_kind: Some("definition".into()),
        digest,
        provenance: Tier::CompilerVerified,
        language: "semaprax".into(),
        provider_id: NATIVE_PROVIDER.into(),
        provider_rank: None,
        complete: !truncated,
        omission_reason: truncated.then(|| "compiler-truncated".to_string()),
        edges: vec![],
        text: Some(raw.to_string()),
        stable_id: Some(root.to_string()),
        link: Link::None,
        verified: true,
        retrieval_handle: truncated.then(|| {
            format!(
                "semaprax context <project> {root} --depth {} --max-bytes <larger>",
                req.native_depth
            )
        }),
        also_reported: vec![],
    })
}

/// Reject or downgrade anything an index report claims that the snapshot or the
/// provider's own identity contradicts.
fn normalize(
    ident: &ProviderIdentity,
    mut r: ExternalResponse,
    diags: &mut Vec<Value>,
) -> ExternalResponse {
    let mut note = |code: &str, msg: String| diags.push(json!({"code": code, "message": msg}));
    if r.provider_id != ident.provider_id {
        note(
            "SPX-HPE060",
            format!(
                "report names provider `{}`, expected `{}`: discarded",
                r.provider_id, ident.provider_id
            ),
        );
        r.items.clear();
        r.status = "stale".into();
        r.coverage = Coverage::default();
    }
    if let (Some(want), Some(got)) = (&ident.upstream_version, &r.upstream_version) {
        if want != got {
            note(
                "SPX-HPE061",
                format!("index built by upstream {got}, trusted version is {want}: discarded"),
            );
            r.items.clear();
            r.status = "stale".into();
            r.coverage = Coverage::default();
        }
    }
    if !r.coverage.skipped.is_empty() {
        r.coverage.complete = false;
    }
    if !r.coverage.complete || r.status != "complete" {
        r.coverage.exhaustive = false;
    }
    for it in &mut r.items {
        if it.tier == Tier::CompilerVerified {
            it.tier = Tier::Structural;
            note("SPX-HPE050", format!("provider claimed compiler-verified for {}; external items are never compiler facts", it.path));
        }
    }
    r
}

fn make_item(snap: &Snapshot, provider: &str, raw: &super::external::RawItem) -> ContextItem {
    let (verified, why) = match std::fs::read_to_string(snap.root.join(&raw.path)) {
        _ if !snap.files.contains_key(&raw.path) => (false, Some("deleted-or-renamed")),
        Ok(t) => match span_digest(&t, raw.span.start_line, raw.span.end_line) {
            Some(d) if d == raw.digest => (true, None),
            _ => (false, Some("stale-digest")),
        },
        Err(_) => (false, Some("unreadable")),
    };
    ContextItem {
        project_id: snap.project_id.clone(),
        worktree_id: snap.worktree_id.clone(),
        revision: snap.revision.clone(),
        path: raw.path.clone(),
        span: raw.span,
        span_kind: raw.span_kind.clone(),
        digest: raw.digest.clone(),
        provenance: if verified { raw.tier } else { Tier::Inferred },
        language: raw.language.clone(),
        provider_id: provider.to_string(),
        provider_rank: Some(raw.rank),
        complete: verified,
        omission_reason: why.map(str::to_string),
        edges: raw
            .edges
            .iter()
            .map(|e| super::item::Edge {
                relation: e.relation.clone(),
                target_path: e.target.clone(),
                provenance: e.tier,
                resolution: e.resolution.clone(),
            })
            .collect(),
        text: raw.text.clone(),
        stable_id: None,
        link: Link::None,
        verified,
        retrieval_handle: None,
        also_reported: vec![],
    }
}

fn link_spx(
    it: &mut ContextItem,
    snap: &Snapshot,
    decls: &[SpxDecl],
    native: Option<&dyn NativeContextSource>,
    cache: &mut BTreeMap<String, bool>,
) {
    if !it.path.ends_with(".spx") {
        return;
    }
    if !it.verified {
        it.link = Link::Stale(it.omission_reason.clone().unwrap_or_default());
        return;
    }
    let Some(d) = decls.iter().find(|d| {
        d.path == it.path && d.start_line <= it.span.start_line && it.span.start_line <= d.end_line
    }) else {
        it.link = Link::Unmappable("no `@id` declaration encloses the span".into());
        return;
    };
    let ok = match cache.get(&d.id) {
        Some(b) => *b,
        None => {
            let b = cache.len() < MAX_RESOLUTIONS
                && native.is_some_and(|n| n.confirm_identity(&scope_for(snap, d), &d.id));
            cache.insert(d.id.clone(), b);
            b
        }
    };
    it.link = if ok {
        Link::Resolved(d.id.clone())
    } else {
        Link::Unmappable("compiler did not confirm the identity".into())
    };
    if ok {
        it.stable_id = Some(d.id.clone());
    }
}

/// Same path+span+digest from two providers: keep the first, record the others'
/// own rank and provenance without comparing scores.
fn dedup_across_providers(c: &mut Vec<(String, ContextItem)>) {
    let mut out: Vec<(String, ContextItem)> = Vec::new();
    for (p, it) in c.drain(..) {
        match out
            .iter_mut()
            .find(|(q, o)| *q != p && o.dedup_key() == it.dedup_key())
        {
            Some((_, o)) => o.also_reported.push((p, it.provider_rank, it.provenance)),
            None => out.push((p, it)),
        }
    }
    *c = out;
}

fn provider_report(
    id: &str,
    status: &str,
    upstream: Option<&str>,
    cov: &Coverage,
    offered: usize,
    dropped: usize,
    diags: Vec<Value>,
) -> Value {
    let skipped: Vec<Value> = cov
        .skipped
        .iter()
        .take(8)
        .map(|(p, r)| json!({"path": p, "reason": r}))
        .collect();
    json!({"provider_id": id, "status": status, "upstream_version": upstream, "rank_scale": "provider-local",
           "offered_items": offered, "out_of_scope_items": dropped,
           "coverage": {"complete": cov.complete, "exhaustive": cov.exhaustive, "indexed_files": cov.indexed_files,
                        "skipped": skipped, "skipped_total": cov.skipped.len()},
           "diagnostics": diags.into_iter().take(4).collect::<Vec<_>>()})
}

/// Compiler scope for one declaration: the project when it has a manifest
/// (project mode), else the declaring file.
fn scope_for(snap: &Snapshot, d: &SpxDecl) -> std::path::PathBuf {
    if snap.root.join("semaprax.toml").is_file() {
        snap.root.clone()
    } else {
        snap.root.join(&d.path)
    }
}
