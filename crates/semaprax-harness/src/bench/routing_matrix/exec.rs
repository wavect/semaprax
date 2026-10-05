//! Cell execution and receipt reconciliation (MR-13, reconciled with MR-03).
//!
//! An executor runs one item on one generation model and reports attempts with
//! their usage receipts and the verdict of the item's independent verifier.
//! The executor declares the most its observations can be (`class`): a
//! fixture executor can never produce a `real` cell, and a row that claims
//! more is recorded as forged. Costs are never taken from the executor: the
//! runner prices every attempt (failures and retries included) from the task
//! set's price book, and router overhead from the router call's typed
//! `CallMetadata` usage or, when that is not provider-reported, its reserved
//! ceiling (never zero).

use super::registry::{ExecutorDecl, RouterPrice};
use super::{q, Item};
use crate::decision::call::{CallMetadata, UsageBasis};
use crate::decision::evidence::{MatchedBudget, Origin, RetryOwner};
use crate::diag::HarnessDiagnostic;
use crate::json;
use crate::receipt::{PriceBook, Usage};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

pub const FIXTURE_SCHEMA: &str = "semaprax.harness-routing-fixture-outcomes.v1";
pub const CELL_SCHEMA: &str = "semaprax.harness-routing-cell.v1";

pub fn origin_rank(o: Origin) -> u8 {
    match o {
        Origin::Unavailable => 0,
        Origin::Fixture => 1,
        Origin::Real => 2,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttemptResult {
    Accepted,
    Failed,
    TransportError,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AttemptRun {
    pub usage: Usage,
    pub result: AttemptResult,
    pub latency_ms: Option<u64>,
    /// Transport retries the gateway performed inside this attempt.
    pub gateway_retries: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CellRun {
    /// The origin the executor claims for this observation.
    pub origin: Origin,
    pub completed: bool,
    pub regressions: u32,
    pub attempts: Vec<AttemptRun>,
    pub retry_owner: RetryOwner,
    pub unavailable: Option<String>,
}

impl CellRun {
    pub fn unavailable(why: impl Into<String>) -> Self {
        Self {
            origin: Origin::Unavailable,
            completed: false,
            regressions: 0,
            attempts: vec![],
            retry_owner: RetryOwner::Host,
            unavailable: Some(why.into()),
        }
    }

    /// Parse a fixture row or an executor reply (`semaprax.harness-routing-cell.v1`).
    pub fn from_json(v: &Value, default_origin: Origin) -> Result<Self, String> {
        let origin = match v.get("origin").and_then(Value::as_str) {
            None => default_origin,
            Some("real") => Origin::Real,
            Some("fixture") => Origin::Fixture,
            Some("unavailable") => {
                return Ok(Self::unavailable(
                    v["reason"]
                        .as_str()
                        .unwrap_or("executor reported unavailable"),
                ))
            }
            Some(o) => return Err(format!("unknown origin `{o}`")),
        };
        let n = |x: &Value, k: &str| x.get(k).and_then(Value::as_u64);
        let mut attempts = Vec::new();
        for a in v["attempts"]
            .as_array()
            .ok_or("`attempts` must be an array")?
        {
            let u = &a["usage"];
            attempts.push(AttemptRun {
                usage: Usage {
                    input_total: n(u, "input_total"),
                    uncached_input: n(u, "uncached_input"),
                    cache_read: n(u, "cache_read"),
                    cache_write: n(u, "cache_write"),
                    cache_write_1h: None,
                    output: n(u, "output"),
                    reasoning: n(u, "reasoning"),
                },
                result: match a["result"].as_str() {
                    Some("accepted") => AttemptResult::Accepted,
                    Some("failed") => AttemptResult::Failed,
                    Some("transport_error") => AttemptResult::TransportError,
                    _ => {
                        return Err(
                            "attempt `result` must be accepted|failed|transport_error".into()
                        )
                    }
                },
                latency_ms: n(a, "latency_ms"),
                gateway_retries: n(a, "gateway_retries").unwrap_or(0) as u32,
            });
        }
        Ok(Self {
            origin,
            completed: v["completed"]
                .as_bool()
                .ok_or("`completed` must be a boolean")?,
            regressions: n(v, "regressions").unwrap_or(0) as u32,
            attempts,
            retry_owner: match v.get("retry_owner").and_then(Value::as_str) {
                None | Some("host") => RetryOwner::Host,
                Some("gateway") => RetryOwner::Gateway,
                Some(o) => return Err(format!("unknown retry owner `{o}`")),
            },
            unavailable: None,
        })
    }
}

/// Runs one (item, model) cell. Implementations live in tree (fixture,
/// command) or out of tree (a test or operator harness).
pub trait CellExecutor {
    /// The strongest origin this executor can produce.
    fn class(&self) -> Origin;
    fn identity(&self) -> String;
    fn execute(&mut self, item: &Item, model: &str) -> CellRun;
}

/// Contract-fixture outcomes: every cell is at most `fixture`; a missing row
/// is `unavailable`, never a success.
pub struct FixtureExecutor {
    rows: BTreeMap<(String, String), Value>,
    digest: String,
}

impl FixtureExecutor {
    pub fn from_json(v: &Value) -> Result<Self, HarnessDiagnostic> {
        if v["schema"] != FIXTURE_SCHEMA {
            return Err(q(
                "SPX-HPQ002",
                format!("fixture outcomes schema must be `{FIXTURE_SCHEMA}`"),
            ));
        }
        let mut rows = BTreeMap::new();
        for r in v["rows"].as_array().into_iter().flatten() {
            let (Some(i), Some(m)) = (r["item"].as_str(), r["model"].as_str()) else {
                return Err(q("SPX-HPQ002", "fixture row needs `item` and `model`"));
            };
            if rows
                .insert((i.to_string(), m.to_string()), r.clone())
                .is_some()
            {
                return Err(q(
                    "SPX-HPQ005",
                    format!("duplicate fixture row `{i}`/`{m}`"),
                ));
            }
        }
        Ok(Self {
            rows,
            digest: json::digest(FIXTURE_SCHEMA, v),
        })
    }
}

impl CellExecutor for FixtureExecutor {
    fn class(&self) -> Origin {
        Origin::Fixture
    }
    fn identity(&self) -> String {
        format!("fixture-table:{}", self.digest)
    }
    fn execute(&mut self, item: &Item, model: &str) -> CellRun {
        match self.rows.get(&(item.id.clone(), model.to_string())) {
            None => CellRun::unavailable("no fixture row for this item and model"),
            Some(r) => CellRun::from_json(r, Origin::Fixture)
                .unwrap_or_else(|e| CellRun::unavailable(format!("malformed fixture row: {e}"))),
        }
    }
}

/// Operator-declared real executor: one process per cell, request JSON on
/// stdin, one `semaprax.harness-routing-cell.v1` reply on stdout. Its replies
/// are `real` only because the operator declared it in the approved registry.
pub struct CommandExecutor {
    pub decl: ExecutorDecl,
    pub cwd: std::path::PathBuf,
    pub vars: BTreeMap<String, String>,
}

impl CellExecutor for CommandExecutor {
    fn class(&self) -> Origin {
        Origin::Real
    }
    fn identity(&self) -> String {
        format!("command:{}", self.decl.identity)
    }
    fn execute(&mut self, item: &Item, model: &str) -> CellRun {
        let req = json!({"schema": CELL_SCHEMA, "item": item.id, "domain": item.domain.as_str(),
                         "partition": item.partition, "model": model, "verifier": item.verifier});
        let child = Command::new(&self.decl.argv[0])
            .args(&self.decl.argv[1..])
            .current_dir(&self.cwd)
            .env_clear()
            .envs(&self.vars)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let Ok(mut child) = child else {
            return CellRun::unavailable("executor could not be started");
        };
        if let Some(mut si) = child.stdin.take() {
            let _ = writeln!(si, "{req}");
        }
        let Ok(out) = child.wait_with_output() else {
            return CellRun::unavailable("executor did not finish");
        };
        match serde_json::from_slice::<Value>(&out.stdout) {
            Ok(v) if out.status.success() && v["schema"] == CELL_SCHEMA => {
                CellRun::from_json(&v, Origin::Real).unwrap_or_else(|e| {
                    CellRun::unavailable(format!("malformed executor reply: {e}"))
                })
            }
            _ => CellRun::unavailable("executor failed or replied off-contract"),
        }
    }
}

pub fn load_fixture(path: &Path) -> Result<FixtureExecutor, HarnessDiagnostic> {
    let b = std::fs::read(path).map_err(|e| q("SPX-HPQ001", format!("{}: {e}", path.display())))?;
    let v: Value = serde_json::from_slice(&b)
        .map_err(|e| q("SPX-HPQ001", format!("{}: {e}", path.display())))?;
    FixtureExecutor::from_json(&v)
}

/// Token totals of one cell (all attempts).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tokens {
    pub input: u64,
    pub cache_read: u64,
    pub output: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Reconciled {
    /// Every attempt priced, failures and retries included; `None` if any
    /// attempt's price is unknown.
    pub cost_micros: Option<u64>,
    pub attempts: u32,
    pub failed_attempts: u32,
    pub gateway_retries: u32,
    /// `cold` (no confirmed cache read) or `warm` per attempt.
    pub cache: Vec<&'static str>,
    pub tokens: Tokens,
    pub latency_ms: Option<u64>,
    pub errors: Vec<String>,
}

/// Reconcile a cell's attempts against its receipts and the retry owner.
pub fn reconcile(
    run: &CellRun,
    model: &str,
    prices: &PriceBook,
    budget: &MatchedBudget,
) -> Reconciled {
    let mut errors = Vec::new();
    let mut cost = Some(0u64);
    let mut tokens = Tokens::default();
    let mut latency = Some(0u64);
    let mut cache = Vec::new();
    let last = run.attempts.len().saturating_sub(1);
    for (i, a) in run.attempts.iter().enumerate() {
        let mut u = a.usage;
        // The uncached share follows from a complete split; nothing else is inferred.
        if u.uncached_input.is_none() {
            if let (Some(t), Some(r), Some(w)) = (u.input_total, u.cache_read, u.cache_write) {
                u.uncached_input = t.checked_sub(r).and_then(|x| x.checked_sub(w));
            }
        }
        let u = &u;
        if let (Some(r), Some(t)) = (u.cache_read, u.input_total) {
            if r > t {
                errors.push(format!("attempt {i}: cache read {r} exceeds input {t}"));
            }
        }
        cache.push(if u.cache_read.unwrap_or(0) > 0 {
            "warm"
        } else {
            "cold"
        });
        tokens.input += u.input_total.unwrap_or(0);
        tokens.cache_read += u.cache_read.unwrap_or(0);
        tokens.output += u.output.unwrap_or(0);
        cost = match (cost, prices.estimate(model, u).micros) {
            (Some(c), Some(x)) => c.checked_add(x),
            _ => None,
        };
        latency = latency.zip(a.latency_ms).map(|(x, y)| x + y);
        if a.result == AttemptResult::Accepted && i != last {
            errors.push(format!(
                "attempt {i} was accepted but the cell kept retrying"
            ));
        }
        if a.result == AttemptResult::TransportError
            && i != last
            && run.retry_owner == RetryOwner::Gateway
        {
            errors.push(format!(
                "attempt {i}: the gateway owns transport retries but the host retried too (double count)"
            ));
        }
        if a.gateway_retries > 0 && run.retry_owner == RetryOwner::Host {
            errors.push(format!(
                "attempt {i}: gateway retries reported while the host owns retries"
            ));
        }
    }
    let accepted_last = run.attempts.last().map(|a| a.result) == Some(AttemptResult::Accepted);
    if run.origin != Origin::Unavailable {
        if run.attempts.is_empty() {
            errors.push("an executed cell carries no attempt receipt".into());
        }
        if run.completed != accepted_last {
            errors.push("completion does not match the final attempt's verdict".into());
        }
    }
    if run.attempts.len() as u32 > budget.max_attempts {
        errors.push(format!(
            "{} attempts exceed the matched budget of {}",
            run.attempts.len(),
            budget.max_attempts
        ));
    }
    Reconciled {
        cost_micros: if run.attempts.is_empty() { None } else { cost },
        attempts: run.attempts.len() as u32,
        failed_attempts: run
            .attempts
            .iter()
            .filter(|a| a.result != AttemptResult::Accepted)
            .count() as u32,
        gateway_retries: run.attempts.iter().map(|a| a.gateway_retries).sum(),
        cache,
        tokens,
        latency_ms: if run.attempts.is_empty() {
            None
        } else {
            latency
        },
        errors,
    }
}

/// Router overhead charged to one task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouterCharge {
    pub calls: u32,
    pub micros: u64,
    /// `none`, `non_billed`, `provider_reported` or `reserved_uncertain`.
    pub basis: &'static str,
    pub input_tokens: Option<u64>,
}

impl RouterCharge {
    pub fn none() -> Self {
        Self {
            calls: 0,
            micros: 0,
            basis: "none",
            input_tokens: None,
        }
    }

    pub fn to_json(&self) -> Value {
        json!({"calls": self.calls, "micros": self.micros, "basis": self.basis,
               "input_tokens": self.input_tokens})
    }
}

/// Settle router spend like MR-03: provider-reported usage of a single priced
/// call settles exactly; otherwise every call keeps its reserved ceiling. The
/// adapter's own `billing` claim does not change a host-priced router.
pub fn router_charge(price: &RouterPrice, calls: u32, call: Option<&CallMetadata>) -> RouterCharge {
    if calls == 0 {
        return RouterCharge::none();
    }
    let input = call.and_then(|c| c.usage.authoritative_input());
    match *price {
        RouterPrice::NonBilled => RouterCharge {
            calls,
            micros: 0,
            basis: "non_billed",
            input_tokens: input,
        },
        RouterPrice::Priced {
            input: pi,
            output: po,
            max_call_micros,
        } => {
            let out = call
                .filter(|c| c.usage.basis == UsageBasis::ProviderReported)
                .and_then(|c| c.usage.output_tokens);
            match (calls, input, out) {
                (1, Some(i), Some(o)) => RouterCharge {
                    calls,
                    micros: (i * pi + o * po).div_ceil(1_000_000),
                    basis: "provider_reported",
                    input_tokens: Some(i),
                },
                _ => RouterCharge {
                    calls,
                    micros: max_call_micros.saturating_mul(u64::from(calls)),
                    basis: "reserved_uncertain",
                    input_tokens: input,
                },
            }
        }
    }
}
