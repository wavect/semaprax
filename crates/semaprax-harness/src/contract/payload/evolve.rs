//! `skill.evolve/v1` payload validators (HN-15). Closed shapes; the request
//! to `solve` never carries an expected answer.

use super::*;

/// Validate one `skill.evolve/v1` payload.
pub fn validate_payload(operation: &str, direction: Direction, v: &Value) -> HarnessResult<()> {
    if !crate::contract::kind::EvolveCapability::OPERATIONS.contains(&operation) {
        return Err(e(
            "SPX-HPA046",
            format!("`{operation}` is not an operation of skill.evolve"),
        ));
    }
    match (operation, direction) {
        ("evolve", Direction::Request) => evolve_request(v),
        ("evolve", Direction::Result) => evolve_result(v),
        (_, Direction::Request) => solve_request(v),
        (_, Direction::Result) => {
            let m = shape(v, "solve result", &["answer", "model_calls"], &[])?;
            str_of(m, "answer", 64 * 1024)?;
            uint_of(m, "model_calls").map(|_| ())
        }
    }
}

fn caps(v: &Value) -> HarnessResult<()> {
    let m = shape(
        v,
        "caps",
        &["max_iterations", "max_model_calls", "max_seconds"],
        &[],
    )?;
    for k in ["max_iterations", "max_model_calls", "max_seconds"] {
        uint_of(m, k)?;
    }
    Ok(())
}

fn evolve_request(v: &Value) -> HarnessResult<()> {
    let m = shape(
        v,
        "evolve request",
        &[
            "experiment",
            "family",
            "trace_path",
            "trace_digest",
            "parent",
            "caps",
            "train_tasks",
        ],
        &[],
    )?;
    str_of(m, "experiment", 128)?;
    str_of(m, "family", 256)?;
    path_of(m, "trace_path")?;
    digest_of(m, "trace_digest")?;
    let p = shape(&m["parent"], "parent", &["name", "digest"], &[])?;
    str_of(p, "name", 128)?;
    digest_of(p, "digest")?;
    caps(&m["caps"])?;
    for t in array_of(m, "train_tasks", 256)? {
        let t = shape(t, "train task", &["id", "prompt", "expected"], &[])?;
        str_of(t, "id", 128)?;
        str_of(t, "prompt", 16 * 1024)?;
        str_of(t, "expected", 16 * 1024)?;
    }
    Ok(())
}

fn evolve_result(v: &Value) -> HarnessResult<()> {
    let m = shape(
        v,
        "evolve result",
        &["wiki", "model_calls", "iterations"],
        &["candidate", "no_action_reason"],
    )?;
    uint_of(m, "model_calls")?;
    uint_of(m, "iterations")?;
    for w in array_of(m, "wiki", 1024)? {
        let w = shape(w, "wiki entry", &["path", "digest"], &[])?;
        path_of(w, "path")?;
        digest_of(w, "digest")?;
    }
    match (m.get("candidate"), m.get("no_action_reason")) {
        (Some(c), None) => {
            let c = shape(c, "candidate", &["name", "description", "body"], &[])?;
            str_of(c, "name", 64)?;
            str_of(c, "description", 512)?;
            str_of(c, "body", 64 * 1024)?;
            Ok(())
        }
        (None, Some(_)) => str_of(m, "no_action_reason", 512).map(|_| ()),
        _ => Err(e(
            "SPX-HPA040",
            "an evolve result needs exactly one of `candidate` or `no_action_reason`",
        )),
    }
}

fn solve_request(v: &Value) -> HarnessResult<()> {
    let m = shape(v, "solve request", &["task"], &["skill"])?;
    let t = shape(&m["task"], "solve task", &["id", "prompt"], &[])?;
    str_of(t, "id", 128)?;
    str_of(t, "prompt", 16 * 1024)?;
    if let Some(s) = m.get("skill") {
        let s = shape(s, "solve skill", &["name", "body"], &[])?;
        str_of(s, "name", 128)?;
        str_of(s, "body", 64 * 1024)?;
    }
    Ok(())
}
