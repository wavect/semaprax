//! `endpoints <project> [list|adopt|bind|reprobe|litellm-config]` (see spec).

use super::adopt::{adopt, AdoptRequest};
use super::catalog::{Catalog, LogicalModel};
use super::ownership::{litellm_config_snippet, Disclosure, EndpointPolicy};
use super::types::{err, EndpointKind, Protocol};
use crate::cli::{Environment, Outcome};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json::{self, JsonLimits};
use serde_json::{json, Value};
use std::collections::BTreeMap;

struct Flags {
    positional: Vec<String>,
    values: BTreeMap<String, String>,
    switches: Vec<String>,
}

const VALUE_FLAGS: &[&str] = &[
    "--url",
    "--kind",
    "--id",
    "--disclosure",
    "--credential-env",
    "--model",
    "--endpoint",
    "--protocol",
    "--rank",
    "--logical",
    "--upstream",
    "--api-base",
];
const SWITCHES: &[&str] = &["--json", "--local-only", "--strict-one-attempt"];

fn parse(args: &[String]) -> HarnessResult<Flags> {
    let mut f = Flags {
        positional: Vec::new(),
        values: BTreeMap::new(),
        switches: Vec::new(),
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if VALUE_FLAGS.contains(&a.as_str()) {
            let v = it
                .next()
                .ok_or_else(|| err("SPX-HPL001", format!("{a} needs a value")))?;
            f.values.insert(a.clone(), v.clone());
        } else if SWITCHES.contains(&a.as_str()) {
            f.switches.push(a.clone());
        } else if a.starts_with("--") {
            return Err(err("SPX-HPL001", format!("unknown flag `{a}`")));
        } else {
            f.positional.push(a.clone());
        }
    }
    Ok(f)
}

pub fn run(args: &[String], env: &Environment) -> Outcome {
    match inner(args, env) {
        Ok(out) => Outcome::ok(out),
        Err(d) if d.code == "SPX-HPL001" => Outcome::usage(d.message),
        Err(d) => Outcome::refused(&d),
    }
}

fn inner(args: &[String], env: &Environment) -> HarnessResult<String> {
    let f = parse(args)?;
    let project = f
        .positional
        .first()
        .ok_or_else(|| err("SPX-HPL001", "endpoints needs <project>"))?;
    if !env.cwd.join(project).is_dir() {
        return Err(err(
            "SPX-HPL001",
            format!("project `{project}` is not a directory"),
        ));
    }
    let sub = f.positional.get(1).map_or("list", String::as_str);
    let as_json = f.switches.iter().any(|s| s == "--json");
    let policy = EndpointPolicy {
        local_only: f.switches.iter().any(|s| s == "--local-only"),
        strict_one_attempt: f.switches.iter().any(|s| s == "--strict-one-attempt"),
    };
    let home = env
        .harness_home
        .clone()
        .ok_or_else(|| err("SPX-HPL004", "no harness home (set SEMAPRAX_HARNESS_HOME)"))?;
    let need = |k: &str| {
        f.values
            .get(k)
            .cloned()
            .ok_or_else(|| err("SPX-HPL001", format!("{k} is required")))
    };
    let credential = |name: &Option<String>| -> HarnessResult<Option<String>> {
        match name {
            None => Ok(None),
            Some(n) => env.vars.get(n).cloned().map(Some).ok_or_else(|| {
                err(
                    "SPX-HPL007",
                    format!(
                        "credential variable `{n}` is not forwarded to the harness environment"
                    ),
                )
            }),
        }
    };
    match sub {
        "list" => {
            let c = Catalog::load(&home)?;
            if as_json {
                return Ok(format!("{}\n", json::canonical(&c.to_json())));
            }
            let mut out = String::new();
            for e in c.endpoints.values() {
                out.push_str(&format!(
                    "endpoint {} {} {} models={}\n",
                    e.id,
                    e.kind.as_str(),
                    e.url,
                    e.models.len()
                ));
                for (k, p) in &e.probes {
                    out.push_str(&format!("  {k}: {} ({})\n", p.verdict.as_str(), p.evidence));
                }
            }
            for b in c.bindings.values() {
                out.push_str(&format!(
                    "binding {} -> {}:{} via {}\n",
                    b.id,
                    b.endpoint_id,
                    b.upstream_model,
                    b.protocol.as_str()
                ));
            }
            Ok(out)
        }
        "adopt" => {
            let kind = EndpointKind::parse(&need("--kind")?)?;
            let disclosure = match f.values.get("--disclosure") {
                None => None,
                Some(p) => {
                    let bytes = std::fs::read(env.cwd.join(p))
                        .map_err(|e| err("SPX-HPL005", format!("cannot read disclosure: {e}")))?;
                    let v = json::parse_strict(
                        &bytes,
                        &JsonLimits {
                            max_bytes: 256 * 1024,
                            max_depth: 16,
                            max_nodes: 8192,
                        },
                    )
                    .map_err(|d| err("SPX-HPL005", d.message))?;
                    Some(Disclosure::from_json(&v)?)
                }
            };
            let credential_env = f.values.get("--credential-env").cloned();
            let record = adopt(&AdoptRequest {
                id: f.values.get("--id").cloned(),
                url: need("--url")?,
                kind,
                credential_value: credential(&credential_env)?,
                credential_env,
                disclosure,
                probe_model: f.values.get("--model").cloned(),
            })?;
            // Fail closed on policy flags before anything is stored.
            super::ownership::check_policy(
                policy,
                &record.ownership,
                &record.destination_for(&record.probe_model),
            )?;
            let mut c = Catalog::load(&home)?;
            c.endpoints.insert(record.id.clone(), record.clone());
            c.save(&home)?;
            if as_json {
                Ok(format!("{}\n", json::canonical(&record.to_json())))
            } else {
                Ok(format!(
                    "adopted {} ({}) models={} catalog={}\n",
                    record.id,
                    record.kind.as_str(),
                    record.models.len(),
                    record.catalog_digest()
                ))
            }
        }
        "bind" => {
            let id = f
                .positional
                .get(2)
                .ok_or_else(|| err("SPX-HPL001", "bind needs <logical-id>"))?;
            let mut c = Catalog::load(&home)?;
            let ep = need("--endpoint")?;
            let rec = c
                .endpoints
                .get(&ep)
                .ok_or_else(|| err("SPX-HPL006", format!("endpoint `{ep}` is not adopted")))?;
            let rank = f
                .values
                .get("--rank")
                .map_or(Ok(0), |r| r.parse::<u32>())
                .map_err(|_| err("SPX-HPL001", "--rank must be an integer"))?;
            let lm = LogicalModel::bind(
                id,
                rec,
                &need("--model")?,
                Protocol::parse(&need("--protocol")?)?,
                policy,
                rank,
            )?;
            c.bindings.insert(lm.id.clone(), lm.clone());
            c.save(&home)?;
            Ok(format!("{}\n", json::canonical(&lm.to_json())))
        }
        "reprobe" => {
            let c = Catalog::load(&home)?;
            let ids: Vec<String> = match f.positional.get(2) {
                Some(i) => vec![i.clone()],
                None => c.endpoints.keys().cloned().collect(),
            };
            let mut fresh = BTreeMap::new();
            for id in &ids {
                let old = c
                    .endpoints
                    .get(id)
                    .ok_or_else(|| err("SPX-HPL006", format!("endpoint `{id}` is not adopted")))?;
                let request = |probe_model: Option<String>| -> HarnessResult<AdoptRequest> {
                    Ok(AdoptRequest {
                        id: Some(old.id.clone()),
                        url: old.url.clone(),
                        kind: old.kind,
                        credential_value: credential(&old.credential_env)?,
                        credential_env: old.credential_env.clone(),
                        disclosure: old.disclosed.then(|| Disclosure {
                            destinations: old.destinations.clone(),
                            ownership: old.ownership.clone(),
                        }),
                        probe_model,
                    })
                };
                // A vanished probe model must surface as a binding status, not a failure.
                let new = match adopt(&request(Some(old.probe_model.clone()))?) {
                    Err(d) if d.code == "SPX-HPL006" => adopt(&request(None)?)?,
                    other => other?,
                };
                fresh.insert(id.clone(), new);
            }
            let statuses = c.revalidate(&fresh);
            let report: Value = statuses
                .iter()
                .filter(|(k, _)| {
                    c.bindings
                        .get(*k)
                        .is_some_and(|b| fresh.contains_key(&b.endpoint_id))
                })
                .map(|(k, s)| (k.clone(), s.to_json()))
                .collect::<serde_json::Map<_, _>>()
                .into();
            // The catalog keeps the approved record; a re-probe never rebinds silently.
            Ok(format!(
                "{}\n",
                json::canonical(&json!({"bindings": report}))
            ))
        }
        "litellm-config" => Ok(litellm_config_snippet(
            &need("--logical")?,
            &need("--upstream")?,
            &need("--api-base")?,
        )),
        other => Err(HarnessDiagnostic::new(
            "SPX-HPL001",
            format!("unknown endpoints subcommand `{other}`"),
        )),
    }
}
