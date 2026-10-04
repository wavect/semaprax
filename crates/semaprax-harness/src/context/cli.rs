//! `semaprax harness context <project> <query> [--max-bytes N] [--symbol S]
//! [--references] [--purge-cache] [--json]`.

use super::broker::{Broker, BrokerRequest};
use super::cache::{system_clock, CacheConfig, ResultCache};
use super::external::HostExternal;
use super::native::SubprocessNative;
use crate::cli::{Environment, Outcome};
use crate::contract::CapabilityKind;
use crate::profile::{resolve_project, HarnessConfig};
use serde_json::Value;

pub fn run(args: &[String], env: &Environment) -> Outcome {
    let mut pos: Vec<&String> = Vec::new();
    let (mut max_bytes, mut symbol, mut refs, mut json, mut purge) =
        (None::<usize>, None::<String>, false, false, false);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--max-bytes" => match it.next().and_then(|v| v.parse().ok()) {
                Some(n) => max_bytes = Some(n),
                None => return Outcome::usage("--max-bytes needs an integer"),
            },
            "--symbol" => match it.next() {
                Some(s) => symbol = Some(s.clone()),
                None => return Outcome::usage("--symbol needs a value"),
            },
            "--references" => refs = true,
            "--json" => json = true,
            "--purge-cache" => purge = true,
            f if f.starts_with("--") => return Outcome::usage(format!("unknown flag `{f}`")),
            _ => pos.push(a),
        }
    }
    let Some(project) = pos.first() else {
        return Outcome::usage("context needs <project> <query>");
    };
    if pos.len() > 2 || (pos.len() < 2 && !purge) {
        return Outcome::usage("context needs <project> <query>");
    }
    let project = env.cwd.join(project);
    let cache_root = env
        .harness_home
        .as_ref()
        .map(|h| h.join("cache").join("context"));
    let cache = cache_root.map(|r| ResultCache::new(r, CacheConfig::default(), system_clock()));
    if purge {
        let n = cache.as_ref().map_or(0, ResultCache::purge_all);
        return Outcome::ok(format!("purged {n} context cache entries\n"));
    }
    let res = match resolve_project(env, &project) {
        Ok(r) => r,
        Err(e) => return Outcome::refused(&e),
    };
    let cfg = match HarnessConfig::load(&project) {
        Ok(c) => c,
        Err(e) => return Outcome::refused(&e),
    };
    let native = env
        .compiler
        .clone()
        .map(|c| Box::new(SubprocessNative::new(c)) as Box<dyn super::native::NativeContextSource>);
    let mut broker = Broker::new(native, cache);
    if let Some(l) = res.launches.get(&CapabilityKind::ContextRepository) {
        let scope = cfg.capability(CapabilityKind::ContextRepository).scope;
        let src = HostExternal::new(
            l.clone(),
            env.clone(),
            res.profile.lock_digest(),
            res.profile.config_digest.clone(),
            scope,
        );
        if let Err(e) = broker.add_provider(Box::new(src)) {
            return Outcome::refused(&e);
        }
    }
    let mut req = BrokerRequest::new(
        pos[1],
        max_bytes.unwrap_or(cfg.budget.context_max_bytes as usize),
    );
    req.symbol = symbol;
    req.references = refs;
    match broker.context(&project, &req) {
        Ok(o) if json => Outcome::ok(format!("{}\n", o.rendered)),
        Ok(o) => Outcome::ok(human(&o.rendered)),
        Err(e) => Outcome::refused(&e),
    }
}

/// Human view derived from the same document the JSON mode prints.
fn human(rendered: &str) -> String {
    let v: Value = serde_json::from_str(rendered).unwrap_or(Value::Null);
    let mut out = format!(
        "context {} ({} {})\n",
        v["snapshot"]["revision"].as_str().unwrap_or(""),
        v["budget"]["unit"].as_str().unwrap_or(""),
        v["budget"]["max_bytes"]
    );
    for k in ["native", "external"] {
        for i in v[k].as_array().into_iter().flatten() {
            out.push_str(&format!(
                "{k} {} {}:{}-{} [{}]\n",
                i["provider_id"].as_str().unwrap_or(""),
                i["path"].as_str().unwrap_or(""),
                i["span"]["start_line"],
                i["span"]["end_line"],
                i["provenance"].as_str().unwrap_or("")
            ));
        }
    }
    out.push_str(&format!("omitted {}\n", v["omitted"]["count"]));
    if !v["references"].is_null() {
        out.push_str(&format!(
            "references exhaustive={} definitive_absence={}\n",
            v["references"]["exhaustive"], v["references"]["definitive_absence"]
        ));
    }
    out
}
