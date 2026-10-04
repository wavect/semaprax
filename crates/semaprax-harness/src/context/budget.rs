//! Byte budget. The unit is `byte-v1`: canonical output bytes, never model
//! tokens. Native facts are mandatory and budgeted first; external items are
//! included whole or not at all; everything omitted keeps a retrieval handle
//! as long as the handle itself fits.

use super::item::ContextItem;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json::canonical;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub const UNIT: &str = "byte-v1";
const MAX_RESERVED_HANDLES: usize = 8;
pub const SCHEMA: &str = "semaprax.harness-context.v1";

/// What the references verdict needs to know; the broker fills it in.
#[derive(Clone, Debug)]
pub struct RefInputs {
    pub symbol: String,
    /// Every provider answered completely and exhaustively with no absence claim contradicted.
    pub providers_exhaustive: bool,
    /// Every consulted provider asserted `no_references` and offered zero items.
    pub providers_say_none: bool,
}

pub struct Parts {
    pub query: Value,
    pub ids: Value,
    pub native: Vec<ContextItem>,
    pub providers: Vec<Value>,
    pub coverage_complete: bool,
    pub native_complete: bool,
    pub references: Option<RefInputs>,
    /// Ordered candidates `(provider_id, item)`; order is the only priority.
    pub candidates: Vec<(String, ContextItem)>,
    pub caps: BTreeMap<String, usize>,
}

pub struct Fitted {
    pub rendered: String,
    pub selected: Vec<ContextItem>,
    pub omitted: usize,
    pub exhaustive: bool,
}

fn handle_of(provider: &str, i: &ContextItem) -> String {
    format!(
        "ctx:{provider}:{}#{}-{}@{}",
        i.path, i.span.start_line, i.span.end_line, i.digest
    )
}

fn refs_block(r: &RefInputs, omitted: usize, pessimistic: bool) -> Value {
    let exhaustive = !pessimistic && r.providers_exhaustive && omitted == 0;
    let absence = exhaustive && r.providers_say_none;
    let mut m =
        json!({"symbol": r.symbol, "exhaustive": exhaustive, "definitive_absence": absence});
    if !exhaustive {
        m["recommendation"] = json!("index coverage or budget is incomplete: run a source search over the project before concluding anything about other usages");
    }
    m
}

fn build(
    p: &Parts,
    selected: &[usize],
    omitted_idx: &[usize],
    handles: usize,
    pessimistic: bool,
) -> Value {
    let omitted = omitted_idx.len();
    let hs: Vec<Value> = omitted_idx
        .iter()
        .take(handles)
        .map(|&i| {
            let (prov, it) = &p.candidates[i];
            json!({"handle": handle_of(prov, it), "provider_id": prov, "path": it.path,
                   "span": {"start_line": it.span.start_line, "end_line": it.span.end_line}, "digest": it.digest,
                   "reason": "byte-budget"})
        })
        .collect();
    let mut doc = json!({
        "schema": SCHEMA,
        "budget": {"unit": UNIT, "max_bytes": 0},
        "query": p.query,
        "snapshot": p.ids,
        "native": p.native.iter().map(ContextItem::to_json).collect::<Vec<_>>(),
        "external": selected.iter().map(|&i| p.candidates[i].1.to_json()).collect::<Vec<_>>(),
        "providers": p.providers,
        "coverage": {"complete": p.coverage_complete && omitted == 0, "native_complete": p.native_complete, "omitted_items": omitted},
        "omitted": {"count": omitted, "handles": hs, "handles_dropped": omitted.saturating_sub(handles)},
    });
    if let Some(r) = &p.references {
        doc["references"] = refs_block(r, omitted, pessimistic);
    }
    doc
}

fn size(v: &Value, max: usize) -> usize {
    let mut v = v.clone();
    v["budget"]["max_bytes"] = json!(max);
    canonical(&v).len()
}

pub fn fit(p: &Parts, max: usize) -> HarnessResult<Fitted> {
    let n = p.candidates.len();
    let base = size(&build(p, &[], &(0..n).collect::<Vec<_>>(), 0, true), max);
    if base > max {
        // Mandatory facts (and the metadata that makes them interpretable) alone overflow.
        let floor = size(&build(p, &[], &[], 0, true), max);
        return Err(HarnessDiagnostic::new(
            "SPX-HPE001",
            format!("mandatory native facts and metadata need {floor} {UNIT} but the budget is {max}; raise --max-bytes or narrow the query"),
        ));
    }
    // Interleave providers round-robin; never compare their raw scores.
    let mut order: Vec<usize> = Vec::new();
    let mut lanes: Vec<Vec<usize>> = Vec::new();
    for (i, (prov, _)) in p.candidates.iter().enumerate() {
        match lanes.iter_mut().find(|l| &p.candidates[l[0]].0 == prov) {
            Some(l) => l.push(i),
            None => lanes.push(vec![i]),
        }
    }
    let mut round = 0;
    while order.len() < n {
        for l in &lanes {
            if let Some(&i) = l.get(round) {
                order.push(i);
            }
        }
        round += 1;
    }
    let mut used: BTreeMap<&str, usize> = BTreeMap::new();
    let mut selected: Vec<usize> = Vec::new();
    for &i in &order {
        let (prov, it) = &p.candidates[i];
        let len = it.rendered_len();
        let cap = p.caps.get(prov).copied().unwrap_or(usize::MAX);
        if used.get(prov.as_str()).copied().unwrap_or(0) + len > cap {
            continue;
        }
        let mut trial = selected.clone();
        trial.push(i);
        let rest: Vec<usize> = (0..n).filter(|x| !trial.contains(x)).collect();
        // Reserve room for retrieval handles of what stays omitted (bounded).
        if size(
            &build(p, &trial, &rest, rest.len().min(MAX_RESERVED_HANDLES), true),
            max,
        ) <= max
        {
            *used.entry(prov.as_str()).or_default() += len;
            selected = trial;
        }
    }
    selected.sort_by_key(|i| order.iter().position(|o| o == i));
    let omitted_idx: Vec<usize> = order
        .iter()
        .copied()
        .filter(|i| !selected.contains(i))
        .collect();
    let mut handles = 0;
    while handles < omitted_idx.len()
        && size(&build(p, &selected, &omitted_idx, handles + 1, false), max) <= max
    {
        handles += 1;
    }
    let mut doc = build(p, &selected, &omitted_idx, handles, false);
    while size(&doc, max) > max && handles > 0 {
        handles -= 1;
        doc = build(p, &selected, &omitted_idx, handles, false);
    }
    doc["budget"]["max_bytes"] = json!(max);
    let rendered = canonical(&doc);
    debug_assert!(rendered.len() <= max);
    Ok(Fitted {
        rendered,
        selected: selected
            .iter()
            .map(|&i| p.candidates[i].1.clone())
            .collect(),
        omitted: omitted_idx.len(),
        exhaustive: p
            .references
            .as_ref()
            .is_some_and(|r| r.providers_exhaustive)
            && omitted_idx.is_empty(),
    })
}
