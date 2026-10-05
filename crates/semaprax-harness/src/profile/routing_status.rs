//! MR-14: ready-to-review decision-provider routing profiles on the existing
//! `status` path, a non-billable configuration/negotiation check and a
//! separate, explicit, metered probe.
//!
//! `status --routing` and `status --routing --check <profile>` make zero
//! inference calls: they read the bundled descriptors, the machine-local
//! adoption/trust/evidence state, the project configuration and the presence
//! (never the value) of declared secret variables, and at most open one TCP
//! connection to a configured loopback endpoint. Only `--probe <profile>
//! --yes` starts the adapter and sends one decision request over a synthetic
//! two-candidate catalog (no task text, no project content); it announces
//! that before it runs and appends a metered record to
//! `<home>/routing/probes.jsonl`.
//!
//! Diagnostics: `SPX-HPB070` declared secret absent, `071` runtime/worker
//! endpoint unset or unreachable, `072` task/version not declared by the
//! adapter, `073` stale evidence, `074` probe abstained, `075` probe not
//! confirmed, `076` profile not adopted, `077` unknown routing profile.

use super::config::HarnessConfig;
use super::installations::LocalState;
use crate::cli::Environment;
use crate::contract::{CapabilityKind, Descriptor};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json;
use serde_json::{json, Value};
use std::path::Path;

pub const ROUTING_PROFILES_SCHEMA: &str = "semaprax.harness-routing-profiles.v1";
pub const ROUTING_CHECK_SCHEMA: &str = "semaprax.harness-routing-check.v1";
pub const ROUTING_PROBE_SCHEMA: &str = "semaprax.harness-routing-probe.v1";
/// The matched routing gate of record (MR-13); see that directory.
pub const MR13_GATE: &str = "benchmarks/harness/2026-10-05-routing-matrix/gate-decision.json";

/// Who owns the endpoint a profile's decision requests reach.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndpointOwner {
    /// In-process rules: no endpoint.
    None,
    /// A vendor-hosted API fixed by the descriptor's network permission.
    VendorHosted,
    /// A loopback server/worker the user starts and selects.
    UserLoopback,
}

impl EndpointOwner {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none (in-process rules)",
            Self::VendorHosted => "vendor-hosted",
            Self::UserLoopback => "user-selected loopback (you start and own it)",
        }
    }
}

/// One reviewable profile. The model text is a selection, not a claim the
/// model is reachable or qualified.
pub struct RoutingProfile {
    pub id: &'static str,
    pub label: &'static str,
    /// Bundled adapter directory; `None` for built-in rules.
    pub bundle: Option<&'static str>,
    pub model: &'static str,
    pub endpoint: EndpointOwner,
    /// Shown as unavailable until the user provisions it.
    pub provision_required: bool,
}

pub const PROFILES: &[RoutingProfile] = &[
    RoutingProfile {
        id: "rules",
        label: "Rules (default)",
        bundle: None,
        model: "semaprax/rules-decision (policy rules, zero router calls)",
        endpoint: EndpointOwner::None,
        provision_required: false,
    },
    RoutingProfile {
        id: "jev",
        label: "Jev (hosted)",
        bundle: Some("systemone/jev-hosted"),
        model: "entitled Jev model named by SEMAPRAX_HARNESS_MODEL or a model_profile (no default)",
        endpoint: EndpointOwner::VendorHosted,
        provision_required: false,
    },
    RoutingProfile {
        id: "laya",
        label: "Laya (local server)",
        bundle: Some("systemone/laya-local"),
        model: "multilingual (adapter default) on laya-0.3.26",
        endpoint: EndpointOwner::UserLoopback,
        provision_required: false,
    },
    RoutingProfile {
        id: "minijev",
        label: "Mini Jev (local worker)",
        bundle: Some("minijev-local"),
        model: "worker identity pinned by the model_profile checkpoint (mini-jev ca612198)",
        endpoint: EndpointOwner::UserLoopback,
        provision_required: false,
    },
    RoutingProfile {
        id: "clef-hosted",
        label: "Clef (Cloudflare Workers AI)",
        bundle: Some("systemone/cloudflare-clef-hosted"),
        model: "@cf/cloudflare/clef",
        endpoint: EndpointOwner::VendorHosted,
        provision_required: false,
    },
    RoutingProfile {
        id: "clef-flash-hosted",
        label: "Clef-Flash (Cloudflare Workers AI)",
        bundle: Some("systemone/cloudflare-clef-hosted"),
        model: "@cf/cloudflare/clef-flash",
        endpoint: EndpointOwner::VendorHosted,
        provision_required: false,
    },
    RoutingProfile {
        id: "clef-local",
        label: "Clef-Flash (local worker)",
        bundle: Some("clef-local"),
        model: "clef-flash (Cloudflare/clef-flash, clef-flash-17f0b0ad)",
        endpoint: EndpointOwner::UserLoopback,
        provision_required: true,
    },
];

fn d(code: &'static str, m: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, m)
}

pub fn profile(id: &str) -> HarnessResult<&'static RoutingProfile> {
    PROFILES.iter().find(|p| p.id == id).ok_or_else(|| {
        let ids: Vec<&str> = PROFILES.iter().map(|p| p.id).collect();
        d(
            "SPX-HPB077",
            format!("unknown routing profile `{id}`; one of: {}", ids.join(", ")),
        )
    })
}

/// The bundled descriptor of a profile (from the binary, not a checkout).
pub fn bundled_descriptor(p: &RoutingProfile) -> HarnessResult<Option<Descriptor>> {
    let Some(dir) = p.bundle else { return Ok(None) };
    let want = format!("{dir}/harness-provider.json");
    let bytes = crate::assets::files()
        .iter()
        .find(|(path, _)| *path == want)
        .map(|(_, b)| *b)
        .ok_or_else(|| {
            d(
                "SPX-HPB063",
                format!("bundled descriptor `{want}` is missing"),
            )
        })?;
    Descriptor::parse(bytes).map(Some)
}

/// Tasks a descriptor's declared `decision.evaluate` versions carry.
pub fn declared_tasks(desc: Option<&Descriptor>) -> Vec<&'static str> {
    let Some(desc) = desc else {
        return vec!["model-route/v1", "model-route/v2", "choice-select/v1"];
    };
    let mut out = Vec::new();
    for c in desc
        .capabilities
        .iter()
        .filter(|c| c.kind_name == "decision.evaluate")
    {
        out.push(match c.version {
            1 => "model-route/v1",
            2 => "model-route/v2",
            3 => "choice-select/v1",
            _ => continue,
        });
    }
    out
}

fn task_version(task: &str) -> HarnessResult<u32> {
    match task {
        "model-route/v1" => Ok(1),
        "model-route/v2" => Ok(2),
        "choice-select/v1" => Ok(3),
        other => Err(d("SPX-HPB072", format!("unknown decision task `{other}`; one of model-route/v1, model-route/v2, choice-select/v1"))),
    }
}

/// Presence only: a value is never read into any output.
fn secret_present(env: &Environment, name: &str) -> bool {
    env.vars.contains_key(name) || std::env::var_os(name).is_some_and(|v| !v.is_empty())
}

fn configured_endpoint(config: &HarnessConfig, provider: &str) -> Option<String> {
    let c = config.capability(CapabilityKind::DecisionEvaluate);
    (c.provider.as_deref() == Some(provider))
        .then(|| {
            c.config
                .get("endpoint")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .flatten()
        .filter(|e| !e.is_empty())
}

/// `host:port` of a loopback `http(s)://` endpoint, else `None`.
fn loopback_addr(endpoint: &str) -> Option<String> {
    let rest = endpoint.split_once("://")?.1;
    let hostport = rest.split('/').next()?;
    let host = hostport.rsplit_once(':').map_or(hostport, |(h, _)| h);
    matches!(host, "127.0.0.1" | "localhost" | "[::1]").then(|| hostport.to_string())
}

fn reachable(hostport: &str) -> bool {
    use std::net::{TcpStream, ToSocketAddrs};
    let Ok(mut addrs) = hostport.to_socket_addrs() else {
        return false;
    };
    addrs.any(|a| TcpStream::connect_timeout(&a, std::time::Duration::from_millis(300)).is_ok())
}

/// Readiness and qualification of one profile, with actionable findings.
/// Zero inference calls.
pub fn review(
    p: &RoutingProfile,
    env: &Environment,
    state: &LocalState,
    config: &HarnessConfig,
    task: Option<&str>,
) -> HarnessResult<Value> {
    let desc = bundled_descriptor(p)?;
    let tasks = declared_tasks(desc.as_ref());
    let mut findings: Vec<Value> = Vec::new();
    let mut find =
        |e: HarnessDiagnostic| findings.push(json!({"code": e.code, "message": e.message}));
    if let Some(t) = task {
        let v = task_version(t)?;
        if !tasks.contains(&t) {
            find(d("SPX-HPB072", format!("`{}` does not declare decision.evaluate v{v}, so it cannot serve `{t}`; choose a profile that lists it or use rules", p.id)));
        }
    }
    let Some(desc) = desc else {
        return Ok(
            json!({"id": p.id, "label": p.label, "provider_id": "semaprax/rules-decision",
            "model": p.model, "tasks": tasks, "endpoint_owner": p.endpoint.as_str(), "secrets": [],
            "adopted": true, "trusted": true, "readiness": if findings.is_empty() { "ready" } else { "not-ready" },
            "qualification": "default (rules decide unless a learned profile qualifies)", "findings": findings}),
        );
    };
    let id = desc.provider_id.clone();
    let secrets: Vec<Value> = desc
        .permissions
        .secrets
        .iter()
        .map(|s| json!({"name": s, "present": secret_present(env, s)}))
        .collect();
    for s in &desc.permissions.secrets {
        if !secret_present(env, s) {
            find(d("SPX-HPB070", format!("`{s}` is not set; export it in the shell that runs semaprax-harness (only its presence is checked, never its value)")));
        }
    }
    let inst = state.installations.get(&id);
    let mut trusted = false;
    match inst {
        None => find(d("SPX-HPB076", format!("`{id}` is not adopted; run `semaprax harness setup` (bundled adapters) or `semaprax harness adopt <dir>/{}/harness-provider.json --runtime <python>` then `semaprax harness trust {id}`", p.bundle.unwrap_or("")))),
        Some(i) => match i.inspect().and_then(|x| super::trust::grant_for(state, &id, &x.current)) {
            Ok(_) => trusted = true,
            Err(e) => find(e),
        },
    }
    let endpoint = match p.endpoint {
        EndpointOwner::UserLoopback => {
            let ep = configured_endpoint(config, &id);
            match &ep {
                None => find(d("SPX-HPB071", format!("no endpoint configured for `{id}`; start your {} and set `[capability.\"decision.evaluate\"] provider = \"{id}\"` with `[capability.\"decision.evaluate\".config] endpoint = \"http://127.0.0.1:<port>\"`", if p.provision_required { "provisioned local worker (see docs/HARNESS-CLEF-LOCAL-V1.md)" } else { "local server/worker" }))),
                Some(e) => match loopback_addr(e) {
                    Some(hp) if reachable(&hp) => {}
                    Some(hp) => find(d("SPX-HPB071", format!("the configured endpoint {hp} is not accepting connections; start the worker/server, then re-run the check"))),
                    None => find(d("SPX-HPB071", "the configured endpoint is not a loopback http(s) URL; this profile only reaches a user-selected loopback endpoint")),
                },
            }
            json!({"owner": p.endpoint.as_str(), "configured": ep.is_some()})
        }
        o => json!({"owner": o.as_str(), "fixed_by_descriptor": desc.permissions.network}),
    };
    // Evidence: records for this provider that no longer match its current
    // checkpoint identity are stale and qualify nothing.
    let mut qualification = "not-evaluated (no passed gate on this machine; MR-13 gate of 2026-10-05: not-evaluated, rules stay active)".to_string();
    if let Some(home) = env.harness_home.as_deref() {
        if let Some(reg) = crate::workflow::routing::load_registry(home)? {
            let current = crate::workflow::decision_open::decision_profile(
                &desc,
                &config.capability(CapabilityKind::DecisionEvaluate),
                env,
            )
            .map(|pp| pp.checkpoint)
            .ok();
            let mine: Vec<_> = reg.keys().filter(|k| k.provider_id == id).collect();
            if !mine.is_empty() {
                if mine
                    .iter()
                    .any(|k| Some(&k.weights_digest) == current.as_ref())
                {
                    qualification = "evidence recorded for the current identity (qualifies only through a passed gate; see `harness bench`)".into();
                } else {
                    qualification = "stale evidence (recorded for another checkpoint)".into();
                    find(d("SPX-HPB073", format!("evidence for `{id}` was recorded for checkpoint(s) {} but the current profile is {}; it qualifies nothing until re-evaluated (`semaprax-harness bench routing-matrix`)",
                        mine.iter().map(|k| k.weights_digest.as_str()).collect::<Vec<_>>().join(", "),
                        current.as_deref().unwrap_or("unknown"))));
                }
            }
        }
    }
    let ready = findings.is_empty();
    let readiness = match (ready, p.provision_required) {
        (true, _) => "ready",
        (false, true) => "unavailable unless provisioned",
        (false, false) => "not-ready",
    };
    Ok(
        json!({"id": p.id, "label": p.label, "provider_id": id, "model": p.model, "tasks": tasks,
        "endpoint_owner": p.endpoint.as_str(), "endpoint": endpoint, "secrets": secrets,
        "adopted": inst.is_some(), "trusted": trusted, "readiness": readiness,
        "qualification": qualification, "findings": findings}),
    )
}

/// `status --routing`: every profile, zero inference calls.
pub fn profiles(env: &Environment, project: &Path, as_json: bool) -> HarnessResult<(String, bool)> {
    let config = HarnessConfig::load(project)?;
    let state = LocalState::load(env)?;
    let rows: Vec<Value> = PROFILES
        .iter()
        .map(|p| review(p, env, &state, &config, None))
        .collect::<HarnessResult<_>>()?;
    let doc = json!({"schema": ROUTING_PROFILES_SCHEMA, "inference_calls": 0, "gate_of_record": MR13_GATE,
        "note": "decision providers choose among logical generation models; the generation model that answered is reported per run in route.explain.generation_model",
        "profiles": rows});
    if as_json {
        return Ok((format!("{}\n", json::canonical(&doc)), true));
    }
    let mut out = String::from("routing profiles (zero inference calls; review before enabling)\n");
    for r in doc["profiles"].as_array().into_iter().flatten() {
        let secrets: Vec<String> = r["secrets"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|s| {
                format!(
                    "{}={}",
                    s["name"].as_str().unwrap_or(""),
                    if s["present"] == true {
                        "set"
                    } else {
                        "missing"
                    }
                )
            })
            .collect();
        out.push_str(&format!(
            "- {} [{}] provider={} model={}\n    tasks={} endpoint={} secrets=[{}] adopted={} trusted={}\n    readiness={} qualification={}\n",
            r["id"].as_str().unwrap_or(""), r["label"].as_str().unwrap_or(""),
            r["provider_id"].as_str().unwrap_or(""), r["model"].as_str().unwrap_or(""),
            r["tasks"].as_array().into_iter().flatten().filter_map(Value::as_str).collect::<Vec<_>>().join(","),
            r["endpoint_owner"].as_str().unwrap_or(""), secrets.join(","), r["adopted"], r["trusted"],
            r["readiness"].as_str().unwrap_or(""), r["qualification"].as_str().unwrap_or("")));
        for f in r["findings"].as_array().into_iter().flatten() {
            out.push_str(&format!(
                "    {}: {}\n",
                f["code"].as_str().unwrap_or(""),
                f["message"].as_str().unwrap_or("")
            ));
        }
    }
    out.push_str(&format!("gate of record: {MR13_GATE}\n"));
    Ok((out, true))
}

/// `status --routing --check <profile> [--task <t>]`: non-billable; exits
/// non-zero with the first finding when the profile is not ready.
pub fn check(
    env: &Environment,
    project: &Path,
    id: &str,
    task: Option<&str>,
    as_json: bool,
) -> HarnessResult<(String, bool)> {
    let p = profile(id)?;
    let config = HarnessConfig::load(project)?;
    let state = LocalState::load(env)?;
    let mut r = review(p, env, &state, &config, task)?;
    let ok = r["findings"].as_array().is_none_or(Vec::is_empty);
    r["schema"] = json!(ROUTING_CHECK_SCHEMA);
    r["inference_calls"] = json!(0);
    r["billable"] = json!(false);
    if as_json {
        return Ok((format!("{}\n", json::canonical(&r)), ok));
    }
    let mut out = format!(
        "check {} (non-billable: zero inference calls)\nreadiness: {}\n",
        p.id,
        r["readiness"].as_str().unwrap_or("")
    );
    for f in r["findings"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "{}: {}\n",
            f["code"].as_str().unwrap_or(""),
            f["message"].as_str().unwrap_or("")
        ));
    }
    Ok((out, ok))
}

/// The announcement printed before any probe runs.
pub fn probe_notice(p: &RoutingProfile) -> String {
    format!("probe {}: this sends ONE decision request to the adapter ({}); it may incur a billable provider call. The request is a synthetic two-candidate catalog with no task text or project content.\n", p.id, p.endpoint.as_str())
}

/// `status --routing --probe <profile> --yes`: one metered decision call.
pub fn probe(
    env: &Environment,
    project: &Path,
    id: &str,
    confirmed: bool,
    python: Option<&Path>,
) -> HarnessResult<String> {
    use crate::decision::*;
    let p = profile(id)?;
    let mut out = probe_notice(p);
    if !confirmed {
        return Err(d(
            "SPX-HPB075",
            format!("{out}not run: pass --yes to confirm the probe"),
        ));
    }
    if p.bundle.is_none() {
        out.push_str("rules make no provider call; nothing to probe\n");
        return Ok(out);
    }
    let home = LocalState::load(env)?.home()?.to_path_buf();
    let mut o = crate::workflow::decision_open::open_decision(
        env,
        project,
        python,
        &home.join("routing/probe-cache"),
    )?;
    let plan = |id: &str, tier, cost| ModelPlan {
        id: id.into(),
        destination: Destination::Local,
        structured_output: true,
        tools: true,
        max_context: 32_000,
        est_cost_micros: cost,
        est_latency_ms: 500,
        strength_rank: 1,
        descriptor: crate::decision::route_v2::PlanDescriptor {
            quality_tier: Some(tier),
            ..Default::default()
        },
    };
    let features = TaskFeatures {
        task_family: TaskFamily::LocalizedDebug,
        estimated_context_tokens: 1000,
        requires_structured_output: true,
        requires_tools: false,
        confidentiality: Confidentiality::Project,
        latency_class: LatencyClass::Interactive,
    };
    let budget = Budget {
        max_cost_micros: 10_000,
        max_latency_ms: 30_000,
        max_router_calls: 1,
    };
    let catalog = vec![
        plan("probe-economy", QualityTier::Economy, 100),
        plan("probe-frontier", QualityTier::Frontier, 1200),
    ];
    let inputs = RouteInputs {
        request: RouteRequest::new(features, catalog, budget)
            .map_err(|e| d("SPX-HPB050", e.message))?,
        policy: RoutePolicy {
            router_max_calls: 1,
            ..RoutePolicy::default()
        },
    };
    let rctx = RouteContext {
        project: o.binding.clone(),
        lock_digest: o.lock_digest.clone(),
        invocation_id: "routing-probe".into(),
        lineage_id: "routing-probe".into(),
        router_lineage: vec![],
        router_calls_used: 0,
        router_ms_used: 0,
    };
    let profile = o.profile.clone();
    let mut cp = ConfiguredProvider {
        gate: EnablementGate::not_evaluated("model-route/v2", &profile.provider_id),
        profile: profile.clone(),
        invoker: &mut o.invoker,
        mode: ProviderMode::Explicit,
    };
    let dec = decide(&inputs, &rctx, Some(&mut cp), &|| inputs.clone(), None)
        .map_err(|e| d(e.code, e.message))?;
    let call = dec.wire.call.as_ref();
    let rec = json!({"schema": ROUTING_PROBE_SCHEMA, "profile": p.id, "provider_id": profile.provider_id,
        "router_calls": dec.router_calls, "router_ms": dec.router_ms, "source": format!("{:?}", dec.source),
        "wire_version": dec.wire.version, "answering_model": call.and_then(|c| c.answering_model.clone()),
        "usage": call.map(|c| json!({"input_tokens": c.usage.input_tokens, "output_tokens": c.usage.output_tokens, "basis": c.usage.basis.as_str()})),
        "billing": call.map(|c| c.billing.as_str()), "billable": true});
    let log = home.join("routing/probes.jsonl");
    std::fs::create_dir_all(home.join("routing"))
        .map_err(|e| d("SPX-HPB020", format!("cannot record the probe: {e}")))?;
    use std::io::Write;
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log)
        .and_then(|mut f| writeln!(f, "{}", json::canonical(&rec)))
        .map_err(|e| d("SPX-HPB020", format!("cannot record the probe: {e}")))?;
    if dec.source != DecisionSource::Provider {
        return Err(d("SPX-HPB074", format!("{out}the provider did not answer the probe ({:?}{}); rules decide, and this profile cannot qualify until it answers. Metered: {} router call(s) recorded in {}",
            dec.source, dec.wire.abstention.map(|a| format!(", abstention {}", a.as_str())).unwrap_or_default(), dec.router_calls, log.display())));
    }
    out.push_str(&format!("{}\n", json::canonical(&rec)));
    Ok(out)
}
