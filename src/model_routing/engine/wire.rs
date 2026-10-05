//! `decision.evaluate/v1` and `/v2`: choices and scores for registered
//! decision tasks only. The v2 request is recognized by its task id and the
//! v2 result by its `score_kind` member; v1 shapes are unchanged.

use super::call::ResultV2;
use super::diag::DecisionResult;
use super::render::{RenderedRequest, MAX_CANDIDATES_V2, MAX_STATE_BYTES, RENDERER_V2};
use super::route_v2::{TaskFeaturesV2, MAX_EXCERPT};
use super::shape::{array_of, bool_of, e, shape, str_of, uint_of};

use serde_json::{Map, Value};

/// Which side of the envelope a payload travels on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Request,
    Result,
}

const V2_TASK: &str = "model-route/v2";

fn is_v2_request(v: &Value) -> bool {
    v.get("task").and_then(Value::as_str) == Some(V2_TASK)
}

fn is_v2_result(v: &Value) -> bool {
    v.get("score_kind").is_some()
}

fn validate_v2_request(v: &Value) -> DecisionResult<()> {
    let m = shape(
        v,
        "decision request",
        &[
            "task",
            "features",
            "candidates",
            "options",
            "disclosure",
            "rendered",
            "max_wire_bytes",
        ],
        &["excerpt"],
    )?;
    TaskFeaturesV2::from_json(&m["features"])?;
    let ids = options(m)?;
    if ids.is_empty() || ids.len() > MAX_CANDIDATES_V2 {
        return Err(e(
            "SPX-HPA040",
            "v2 `options` must hold 1..=16 selection ids",
        ));
    }
    if ids.iter().enumerate().any(|(i, s)| *s != format!("m{i}")) {
        return Err(e(
            "SPX-HPA040",
            "v2 selection ids must be `m0..m{n-1}` in order",
        ));
    }
    let cands = array_of(m, "candidates", MAX_CANDIDATES_V2)?;
    let cids: Vec<&str> = cands.iter().filter_map(|c| c["id"].as_str()).collect();
    if cids != ids {
        return Err(e("SPX-HPA040", "`candidates[*].id` must equal `options`"));
    }
    let disclosure = str_of(m, "disclosure", 32)?;
    match (disclosure, m.get("excerpt")) {
        ("metadata_only", None) => {}
        ("excerpt", Some(Value::String(x))) if !x.is_empty() && x.len() <= MAX_EXCERPT => {}
        _ => {
            return Err(e(
                "SPX-HPA040",
                "`excerpt` must be present (1..=1024 bytes) exactly when disclosure is `excerpt`",
            ))
        }
    }
    let r = shape(
        &m["rendered"],
        "rendered",
        &[
            "renderer",
            "instructions",
            "state",
            "option_labels",
            "digest",
        ],
        &[],
    )?;
    if str_of(r, "renderer", 64)? != RENDERER_V2 {
        return Err(e("SPX-HPA040", "unknown renderer"));
    }
    str_of(r, "state", MAX_STATE_BYTES)?;
    str_of(r, "instructions", 1024)?;
    let labels = r["option_labels"]
        .as_object()
        .ok_or_else(|| e("SPX-HPA040", "`option_labels` must be an object"))?;
    let mut lk: Vec<&str> = labels.keys().map(String::as_str).collect();
    let mut want = ids.clone();
    lk.sort_unstable();
    want.sort_unstable();
    if lk != want
        || labels
            .values()
            .any(|l| !l.as_str().is_some_and(|s| s.len() <= 80))
    {
        return Err(e(
            "SPX-HPA040",
            "`option_labels` must label exactly the options",
        ));
    }
    let mut body = m["rendered"].clone();
    if let Some(o) = body.as_object_mut() {
        o.remove("digest");
    }
    if r["digest"].as_str() != Some(RenderedRequest::digest_of(&body).as_str()) {
        return Err(e(
            "SPX-HPA040",
            "`rendered.digest` does not match the rendered content",
        ));
    }
    uint_of(m, "max_wire_bytes")?;
    Ok(())
}

pub fn validate(dir: Direction, v: &Value) -> DecisionResult<()> {
    match dir {
        Direction::Request if is_v2_request(v) => validate_v2_request(v)?,
        Direction::Result if is_v2_result(v) => {
            ResultV2::from_json(v)?;
        }
        Direction::Request => {
            let m = shape(v, "decision request", &["task", "features", "options"], &[])?;
            if str_of(m, "task", 64)? != "model-route/v1" {
                return Err(e("SPX-HPA040", "unregistered decision task"));
            }
            if !m["features"].is_object() {
                return Err(e("SPX-HPA040", "`features` must be an object"));
            }
            let ids = options(m)?;
            if ids.is_empty() {
                return Err(e("SPX-HPA040", "`options` must not be empty"));
            }
        }
        Direction::Result => {
            let m = shape(v, "decision result", &["choice", "scores", "abstain"], &[])?;
            let abstain = bool_of(m, "abstain")?;
            match &m["choice"] {
                Value::Null if abstain => {}
                Value::String(s) if !abstain && !s.is_empty() => {}
                _ => {
                    return Err(e(
                        "SPX-HPA040",
                        "`choice` must be null exactly when `abstain` is true",
                    ))
                }
            }
            let sc = m["scores"]
                .as_object()
                .ok_or_else(|| e("SPX-HPA040", "`scores` must be an object"))?;
            for (k, x) in sc {
                match x.as_f64() {
                    Some(f) if f.is_finite() && (0.0..=1.0).contains(&f) => {}
                    _ => {
                        return Err(e(
                            "SPX-HPA044",
                            format!("score for `{k}` must be finite in [0,1]"),
                        ))
                    }
                }
            }
        }
    }
    Ok(())
}

fn options(m: &Map<String, Value>) -> DecisionResult<Vec<&str>> {
    let mut ids: Vec<&str> = Vec::new();
    for o in array_of(m, "options", 1024)? {
        let s = o
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 128)
            .ok_or_else(|| e("SPX-HPA040", "option ids must be non-empty strings"))?;
        if ids.contains(&s) {
            return Err(e("SPX-HPA040", format!("duplicate option `{s}`")));
        }
        ids.push(s);
    }
    Ok(ids)
}

pub fn check_against_request(request: &Value, result: &Value) -> DecisionResult<()> {
    // A v2 request takes only a v2 result, and a v1 request only a v1 result.
    if is_v2_request(request) != is_v2_result(result) {
        return Err(e(
            "SPX-HPA040",
            "result payload version does not match the request task",
        ));
    }
    if is_v2_request(request) {
        let m = request
            .as_object()
            .ok_or_else(|| e("SPX-HPA040", "request payload is not an object"))?;
        let ids = options(m)?;
        return ResultV2::from_json(result)?.check_against(
            &ids,
            request["rendered"]["digest"].as_str().unwrap_or(""),
            request["max_wire_bytes"].as_u64().unwrap_or(0),
        );
    }
    let ids = options(
        request
            .as_object()
            .ok_or_else(|| e("SPX-HPA040", "request payload is not an object"))?,
    )?;
    if let Some(c) = result["choice"].as_str() {
        if !ids.contains(&c) {
            return Err(e(
                "SPX-HPA043",
                format!("choice `{c}` is not one of the request options"),
            ));
        }
    }
    for k in result["scores"]
        .as_object()
        .into_iter()
        .flat_map(|s| s.keys())
    {
        if !ids.contains(&k.as_str()) {
            return Err(e(
                "SPX-HPA043",
                format!("score for `{k}` which is not a request option"),
            ));
        }
    }
    Ok(())
}
