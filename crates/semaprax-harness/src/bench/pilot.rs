//! Local model pilot. A small loopback model answers a fact question given the
//! context each profile produced; correctness is scored by string presence, so
//! no model judges another model. Fewer than ten trials per configuration is
//! labelled `pilot-only`. Only a loopback HTTP endpoint is ever contacted.

use super::arena::Arena;
use super::corpus::Corpus;
use crate::cli::run;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

pub struct PilotConfig {
    /// `host:port`, must be loopback.
    pub addr: String,
    pub model: String,
    pub reps: u32,
    pub max_calls: u32,
}

fn http(addr: &str, method: &str, path: &str, body: &str) -> Result<String, String> {
    http_with(addr, method, path, body, Duration::from_secs(120))
}

/// Same request with an explicit read timeout (application trials wait longer).
pub(crate) fn http_with(addr: &str, method: &str, path: &str, body: &str, timeout: Duration) -> Result<String, String> {
    let mut s = TcpStream::connect(addr).map_err(|e| format!("connect {addr}: {e}"))?;
    s.set_read_timeout(Some(timeout)).ok();
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    s.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&raw).into_owned();
    let (head, rest) = text
        .split_once("\r\n\r\n")
        .ok_or("malformed http response")?;
    if !head.starts_with("HTTP/1.1 200") {
        return Err(format!(
            "http status: {}",
            head.lines().next().unwrap_or("")
        ));
    }
    if head
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        let (mut out, mut r) = (String::new(), rest);
        while let Some((len, tail)) = r.split_once("\r\n") {
            let n = usize::from_str_radix(len.trim(), 16).map_err(|_| "bad chunk")?;
            if n == 0 {
                break;
            }
            out.push_str(tail.get(..n).ok_or("short chunk")?);
            r = tail.get(n + 2..).ok_or("short chunk")?;
        }
        return Ok(out);
    }
    Ok(rest.to_string())
}

pub fn run_pilot(corpus: &Corpus, arenas: &[&Arena], cfg: &PilotConfig) -> Value {
    if !(cfg.addr.starts_with("127.0.0.1:") || cfg.addr.starts_with("localhost:")) {
        return json!({"status": "refused", "reason": "pilot endpoint must be loopback"});
    }
    let tags = match http(&cfg.addr, "GET", "/api/tags", "") {
        Ok(t) => serde_json::from_str::<Value>(&t).unwrap_or(Value::Null),
        Err(e) => {
            return json!({"status": "untested", "reason": format!("local model endpoint unreachable: {e}")})
        }
    };
    let digest = tags["models"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|m| m["name"] == cfg.model.as_str())
        .and_then(|m| m["digest"].as_str())
        .unwrap_or("unknown")
        .to_string();
    let mut rows = vec![];
    let mut calls = 0;
    'outer: for t in corpus
        .tasks
        .iter()
        .filter(|t| t.question.is_some() && t.query.is_some())
    {
        let (ask, expect) = t.question.clone().expect("filtered");
        for a in arenas.iter().filter(|a| a.untested.is_none()) {
            let project = a.projects[&t.project].clone();
            let o = run(
                &[
                    "context",
                    project.to_str().unwrap_or(""),
                    t.query.as_deref().unwrap_or(""),
                    "--max-bytes",
                    &t.max_bytes.to_string(),
                    "--json",
                ]
                .map(String::from),
                &a.env,
            );
            let ctx: String = o.stdout.chars().take(12_000).collect();
            for rep in 0..cfg.reps {
                if calls >= cfg.max_calls {
                    break 'outer;
                }
                calls += 1;
                let seed = corpus.seed + rep as u64;
                let prompt = format!("Context:\n{ctx}\n\nQuestion: {ask}\nAnswer in one short sentence using only the context.");
                let body = json!({"model": cfg.model, "prompt": prompt, "stream": false,
                                  "options": {"temperature": 0.2, "seed": seed, "num_predict": 64, "num_ctx": 8192}});
                let started = Instant::now();
                let reply = http(&cfg.addr, "POST", "/api/generate", &body.to_string());
                let ms = started.elapsed().as_millis() as u64;
                let (answer, err) = match reply {
                    Ok(r) => (
                        serde_json::from_str::<Value>(&r)
                            .ok()
                            .and_then(|v| v["response"].as_str().map(str::to_string))
                            .unwrap_or_default(),
                        None,
                    ),
                    Err(e) => (String::new(), Some(e)),
                };
                let low = answer.to_lowercase();
                let correct =
                    err.is_none() && expect.iter().all(|e| low.contains(&e.to_lowercase()));
                rows.push(json!({"task": t.id, "profile": a.profile.id, "rep": rep, "seed": seed, "correct": correct,
                    "prompt_bytes": prompt.len(), "latency_ms": ms, "error": err}));
            }
        }
    }
    let mut cfgs = std::collections::BTreeMap::<String, (u64, u64)>::new();
    for r in &rows {
        let e = cfgs
            .entry(format!(
                "{}|{}",
                r["task"].as_str().unwrap_or(""),
                r["profile"].as_str().unwrap_or("")
            ))
            .or_default();
        e.1 += 1;
        e.0 += (r["correct"] == true) as u64;
    }
    let min_n = cfgs.values().map(|v| v.1).min().unwrap_or(0);
    json!({
        "status": "ran", "label": if min_n >= 10 { "matched-trials" } else { "pilot-only" },
        "model": cfg.model, "model_digest": digest, "endpoint": "loopback", "calls": calls,
        "min_trials_per_configuration": min_n, "temperature": 0.2, "base_seed": corpus.seed,
        "scoring": "all expected strings present in the answer (case-insensitive); deterministic",
        "configurations": cfgs.iter().map(|(k, v)| json!({"config": k, "correct": v.0, "n": v.1})).collect::<Vec<_>>(),
        "rows": rows,
        "caveat": "small local model, fewer than 10 trials per configuration: not evidence for any default",
    })
}

fn generate(cfg: &PilotConfig, prompt: &str, seed: u64) -> (String, Option<String>, u64) {
    let body = json!({"model": cfg.model, "prompt": prompt, "stream": false,
                      "options": {"temperature": 0.2, "seed": seed, "num_predict": 64, "num_ctx": 8192}});
    let started = Instant::now();
    let reply = http(&cfg.addr, "POST", "/api/generate", &body.to_string());
    let ms = started.elapsed().as_millis() as u64;
    match reply {
        Ok(r) => (
            serde_json::from_str::<Value>(&r)
                .ok()
                .and_then(|v| v["response"].as_str().map(str::to_string))
                .unwrap_or_default(),
            None,
            ms,
        ),
        Err(e) => (String::new(), Some(e), ms),
    }
}

/// Skill pilot: does the proposal reuse the existing API with and without the
/// adopted skill? Same context in both arms; the only difference is the skill
/// prompt block exactly as `skills --json` renders it. Scoring is by string
/// presence; fewer than ten trials per arm is `pilot-only`.
pub fn run_skill_pilot(
    corpus: &Corpus,
    base: &Arena,
    skill_arena: &Arena,
    cfg: &PilotConfig,
    reps: u32,
) -> Value {
    let Some(task) = corpus.tasks.iter().find(|t| t.reuse_probe.is_some()) else {
        return json!({"status": "untested", "reason": "corpus has no reuse_probe task"});
    };
    if base.untested.is_some() || skill_arena.untested.is_some() {
        return json!({"status": "untested", "reason": "baseline or skill arena unavailable"});
    }
    let (ask, reuse) = task.reuse_probe.clone().expect("checked");
    let project = base.projects[&task.project].clone();
    let ctx = run(
        &[
            "context",
            project.to_str().unwrap_or(""),
            task.query.as_deref().unwrap_or(""),
            "--max-bytes",
            "4096",
            "--json",
        ]
        .map(String::from),
        &base.env,
    )
    .stdout;
    let mut args = vec!["skills".to_string()];
    for r in skill_arena.skill_roots() {
        args.push("--root".into());
        args.push(r.display().to_string());
    }
    args.extend(["--tags", "api-reuse", "--json"].map(String::from));
    let sk: Value =
        serde_json::from_str(run(&args, &skill_arena.env).stdout.trim()).unwrap_or(Value::Null);
    let skill_text = sk["prompt"]["text"].as_str().unwrap_or("").to_string();
    if skill_text.is_empty() {
        return json!({"status": "untested", "reason": "skill prompt unavailable"});
    }
    let mut rows = vec![];
    let mut tally = [(0u64, 0u64); 2];
    for rep in 0..reps {
        for (arm, with_skill) in [("without-skill", false), ("with-skill", true)] {
            let prompt = format!(
                "Context:\n{ctx}\n\n{}Task: {}\n{ask}",
                if with_skill {
                    format!("{skill_text}\n")
                } else {
                    String::new()
                },
                task.request
            );
            let seed = corpus.seed + rep as u64;
            let (answer, err, ms) = generate(cfg, &prompt, seed);
            let reused = err.is_none() && reuse.iter().all(|r| answer.contains(r.as_str()));
            let slot = &mut tally[with_skill as usize];
            slot.1 += 1;
            slot.0 += reused as u64;
            rows.push(json!({"arm": arm, "rep": rep, "seed": seed, "reused_existing_api": reused, "prompt_bytes": prompt.len(),
                             "latency_ms": ms, "error": err, "answer_digest": crate::json::sha256_plain(answer.as_bytes())}));
        }
    }
    json!({
        "status": "ran", "label": if reps >= 10 { "matched-trials" } else { "pilot-only" }, "task": task.id,
        "calls": reps * 2, "reuse_check": format!("answer contains {reuse:?}"),
        "without_skill": {"reused": tally[0].0, "n": tally[0].1}, "with_skill": {"reused": tally[1].0, "n": tally[1].1},
        "skill_prompt_bytes": skill_text.len(),
        "compiler_gate": "unchanged: proposals are still judged only by the workflow's compiler checks (see the native+skill workflow cells)",
        "rows": rows, "caveat": "small local model, few trials: no evidence for any default",
    })
}
