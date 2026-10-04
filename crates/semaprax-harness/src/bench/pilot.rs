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
    let mut s = TcpStream::connect(addr).map_err(|e| format!("connect {addr}: {e}"))?;
    s.set_read_timeout(Some(Duration::from_secs(120))).ok();
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
