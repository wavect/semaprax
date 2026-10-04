//! `decide <task.json> [--catalog <catalog.json>] [--json]` (rules only; an
//! external router is wired in later through the host).

use super::policy::RoutePolicy;
use super::route::RouteRequest;
use super::router::{decide, RouteContext, RouteInputs};
use crate::cli::Environment;
use crate::contract::ProjectBinding;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json::{self, JsonLimits};
use serde_json::{json, Value};

fn read(env: &Environment, path: &str) -> HarnessResult<Value> {
    let p = env.cwd.join(path);
    let bytes = std::fs::read(&p)
        .map_err(|e| HarnessDiagnostic::new("SPX-HPJ015", format!("cannot read `{path}`: {e}")))?;
    json::parse_strict(
        &bytes,
        &JsonLimits {
            max_bytes: 4 * 1024 * 1024,
            max_depth: 32,
            max_nodes: 65_536,
        },
    )
}

pub fn run(args: &[String], env: &Environment) -> HarnessResult<Option<String>> {
    let (mut task, mut catalog, mut as_json) = (None, None, false);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--json" => as_json = true,
            "--catalog" => {
                catalog = Some(
                    it.next()
                        .ok_or_else(|| {
                            HarnessDiagnostic::new("SPX-HPJ015", "--catalog needs a path")
                        })?
                        .clone(),
                )
            }
            s if s.starts_with("--") => {
                return Err(HarnessDiagnostic::new(
                    "SPX-HPJ015",
                    format!("unknown flag `{s}`"),
                ))
            }
            s if task.is_none() => task = Some(s.to_string()),
            _ => {
                return Err(HarnessDiagnostic::new(
                    "SPX-HPJ015",
                    "unexpected extra operand",
                ))
            }
        }
    }
    let Some(task) = task else { return Ok(None) };
    let doc = read(env, &task)?;
    let mut request = RouteRequest::from_json(&doc)?;
    if let Some(path) = catalog {
        let c = read(env, &path)?;
        let c = c.get("catalog").cloned().unwrap_or(c);
        request = RouteRequest::new(
            request.features,
            RouteRequest::catalog_from_json(&c)?,
            request.budget,
        )?;
    }
    let policy = match doc.get("policy") {
        Some(p) => RoutePolicy::from_json(p)?,
        None => RoutePolicy::default(),
    };
    let lineage = doc
        .get("lineage_id")
        .and_then(Value::as_str)
        .unwrap_or("cli-lineage")
        .to_string();
    let inputs = RouteInputs { request, policy };
    let ctx = RouteContext {
        project: ProjectBinding {
            id: "cli".into(),
            worktree: "cli".into(),
            revision: "cli".into(),
        },
        lock_digest: "cli".into(),
        invocation_id: "cli".into(),
        lineage_id: lineage,
        router_lineage: vec![],
        router_calls_used: 0,
        router_ms_used: 0,
    };
    let d = decide(&inputs, &ctx, None, &|| inputs.clone(), None)?;
    let out = json!({
        "task": "model-route/v1", "choice": d.choice, "source": format!("{:?}", d.source),
        "provider": d.provider_id, "status": d.provider_status, "router_calls": d.router_calls,
        "digests": d.digests.to_json(), "plan": d.plan.to_json(),
    });
    Ok(Some(if as_json {
        format!("{}\n", json::canonical(&out))
    } else {
        let order: Vec<&str> = d.plan.ordered.iter().map(|s| s.model_id.as_str()).collect();
        format!(
            "choice: {}\nsource: {:?}\nprovider: {} ({})\nrouter calls: {}\nplan: {}\n",
            d.choice,
            d.source,
            d.provider_id,
            d.provider_status,
            d.router_calls,
            order.join(" > ")
        )
    }))
}
