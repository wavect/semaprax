//! `decision.evaluate/v1`: choices and scores for registered decision tasks only.

use super::*;

pub fn validate(dir: Direction, v: &Value) -> HarnessResult<()> {
    match dir {
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

fn options(m: &Map<String, Value>) -> HarnessResult<Vec<&str>> {
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

pub fn check_against_request(request: &Value, result: &Value) -> HarnessResult<()> {
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
