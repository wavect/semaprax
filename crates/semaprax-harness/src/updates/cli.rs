//! `semaprax harness updates <check|apply|status|rollback|revoke|approve-policy|add|propose|maintain>`.
//!
//! Only these explicit verbs and the session-start `maintain` entry ever reach
//! the network; nothing on the compile/check path calls into this module.

use super::fetch::Fetcher;
use super::ops::{self, Ctx, Report};
use super::state::{Kind, Source, State};
use super::{d, DirectoryFetcher, GitHubCliFetcher};
use crate::cli::{Environment, Outcome};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::skills::official::embedded_cached;
use std::path::{Path, PathBuf};
use std::time::Duration;

const USAGE: &str = "usage: semaprax harness updates <verb> [args]\n\
  check [id...] | status [id...] | apply <id> [--approve] | rollback <id> | revoke <id> <commit|digest>\n\
  approve-policy [--auto-content] [--ttl-secs N] [--timeout-ms N] [--gh ABS_PATH]\n\
  add <id> --kind skill|adapter --repo URL [--subpath P] [--channel C] [--branch B] [--file rel=upstream]...\n\
  propose [id...] | maintain\n\
flags: --json --frozen --offline --gh ABS_PATH --fixture-dir ABS_DIR\n";

#[derive(Default)]
struct Opts {
    verb: String,
    pos: Vec<String>,
    json: bool,
    frozen: bool,
    offline: bool,
    approve: bool,
    auto_content: bool,
    gh: Option<String>,
    fixture: Option<String>,
    ttl: Option<u64>,
    timeout: Option<u64>,
    kind: Option<String>,
    repo: Option<String>,
    subpath: Option<String>,
    channel: Option<String>,
    branch: Option<String>,
    files: Vec<String>,
}

fn parse(args: &[String]) -> Result<Opts, String> {
    let mut o = Opts::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = |f: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{f} needs a value"))
        };
        let num = |v: String, f: &str| v.parse::<u64>().map_err(|_| format!("invalid {f} `{v}`"));
        match a.as_str() {
            "--json" => o.json = true,
            "--frozen" => o.frozen = true,
            "--offline" => o.offline = true,
            "--approve" => o.approve = true,
            "--auto-content" => o.auto_content = true,
            "--gh" => o.gh = Some(val("--gh")?),
            "--fixture-dir" => o.fixture = Some(val("--fixture-dir")?),
            "--ttl-secs" => o.ttl = Some(num(val("--ttl-secs")?, "--ttl-secs")?),
            "--timeout-ms" => o.timeout = Some(num(val("--timeout-ms")?, "--timeout-ms")?),
            "--kind" => o.kind = Some(val("--kind")?),
            "--repo" => o.repo = Some(val("--repo")?),
            "--subpath" => o.subpath = Some(val("--subpath")?),
            "--channel" => o.channel = Some(val("--channel")?),
            "--branch" => o.branch = Some(val("--branch")?),
            "--file" => o.files.push(val("--file")?),
            f if f.starts_with("--") => return Err(format!("unexpected argument `{f}`")),
            w if o.verb.is_empty() => o.verb = w.to_string(),
            w => o.pos.push(w.to_string()),
        }
    }
    Ok(o)
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn fetcher(o: &Opts, home: &Path, env: &Environment) -> HarnessResult<Option<Box<dyn Fetcher>>> {
    if o.frozen || o.offline {
        return Ok(None);
    }
    if let Some(dir) = &o.fixture {
        return Ok(Some(Box::new(DirectoryFetcher::load(&PathBuf::from(dir))?)));
    }
    let policy = State::load(home)?.policy;
    let Some(gh) = o.gh.clone().or(policy.gh) else {
        return Ok(None);
    };
    let t = Duration::from_millis(policy.timeout_ms);
    Ok(Some(Box::new(GitHubCliFetcher::new(
        PathBuf::from(gh),
        env,
        t,
    )?)))
}

fn emit(o: &Opts, r: &Report) -> Outcome {
    if o.json {
        Outcome::ok(format!("{}\n", crate::json::canonical(&r.to_json())))
    } else {
        Outcome::ok(r.render())
    }
}

fn run(args: &[String], env: &Environment) -> HarnessResult<Outcome> {
    let o = parse(args).map_err(|m| d("SPX-HPU001", m))?;
    if o.verb.is_empty() {
        return Ok(Outcome::usage("missing updates verb").with_usage(USAGE));
    }
    let home = env
        .harness_home
        .clone()
        .ok_or_else(|| d("SPX-HPU013", "no harness home is configured"))?;
    let id = || {
        o.pos.first().cloned().ok_or_else(|| {
            d(
                "SPX-HPU001",
                format!("`updates {}` needs a source id", o.verb),
            )
        })
    };
    match o.verb.as_str() {
        "approve-policy" => {
            ops::approve_policy(&home, o.auto_content, o.ttl, o.timeout, o.gh.clone())?;
            Ok(Outcome::ok("update policy approved\n"))
        }
        "add" => {
            let kind = o
                .kind
                .as_deref()
                .and_then(Kind::parse)
                .ok_or_else(|| d("SPX-HPU001", "`updates add` needs --kind skill|adapter"))?;
            let files = o
                .files
                .iter()
                .map(|f| {
                    f.split_once('=')
                        .map(|(a, b)| (a.to_string(), b.to_string()))
                        .ok_or_else(|| d("SPX-HPU001", "--file needs rel=upstream-path"))
                })
                .collect::<HarnessResult<Vec<_>>>()?;
            let src = Source {
                id: id()?,
                kind,
                repo: o
                    .repo
                    .clone()
                    .ok_or_else(|| d("SPX-HPU001", "--repo is required"))?,
                subpath: o.subpath.clone(),
                channel: o.channel.clone().unwrap_or_else(|| "latest-stable".into()),
                head_branch: o.branch.clone().unwrap_or_else(|| "main".into()),
                files,
                active: None,
                previous: vec![],
                pending: None,
                rejected: None,
                held: None,
                revoked: vec![],
                unavailable: false,
                resolved_head: None,
            };
            ops::add_source(&home, src)?;
            Ok(Outcome::ok("update source added\n"))
        }
        v @ ("check" | "status" | "apply" | "rollback" | "revoke" | "propose" | "maintain") => {
            let f = if matches!(v, "check" | "propose" | "maintain") {
                fetcher(&o, &home, env)?
            } else {
                None
            };
            let ctx = Ctx {
                home: &home,
                fetcher: f.as_deref(),
                now: now(),
                offline: o.offline || o.frozen || f.is_none(),
                frozen: o.frozen,
                gate: None,
                catalog: embedded_cached(),
            };
            match v {
                "check" => Ok(emit(&o, &ops::check(&ctx, &o.pos)?)),
                "status" => Ok(emit(&o, &ops::status(&ctx, &o.pos)?)),
                "maintain" => Ok(emit(&o, &ops::maintenance(&ctx))),
                "apply" => Ok(emit(&o, &ops::apply(&ctx, &id()?, o.approve)?)),
                "rollback" => Ok(emit(&o, &ops::rollback(&ctx, &id()?)?)),
                "revoke" => {
                    let t = o.pos.get(1).cloned().ok_or_else(|| {
                        d("SPX-HPU001", "`updates revoke` needs <id> <commit|digest>")
                    })?;
                    Ok(emit(&o, &ops::revoke(&ctx, &id()?, &t)?))
                }
                _ => {
                    let Some(f) = ctx.fetcher else {
                        return Err(d("SPX-HPU010", "`updates propose` needs a fetcher (--gh or --fixture-dir) and no --offline"));
                    };
                    let doc = super::propose::propose(&ctx, f, &o.pos)?;
                    Ok(Outcome::ok(format!("{}\n", crate::json::canonical(&doc))))
                }
            }
        }
        other => Ok(Outcome::usage(format!("unknown updates verb `{other}`")).with_usage(USAGE)),
    }
}

trait WithUsage {
    fn with_usage(self, u: &str) -> Self;
}
impl WithUsage for Outcome {
    fn with_usage(mut self, u: &str) -> Self {
        self.stderr.push_str(u);
        self
    }
}

pub fn cli_updates(args: &[String], env: &Environment) -> Outcome {
    run(args, env).unwrap_or_else(|e: HarnessDiagnostic| Outcome::refused(&e))
}
