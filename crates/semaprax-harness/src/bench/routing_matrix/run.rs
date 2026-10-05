//! The matched matrix runner (MR-13).
//!
//! Order: the evaluation items are sealed (digest) first; calibration items
//! run next and are the only data the cost-aware arm and score calibration
//! may use; the sealed items run last. Each (item, model) executes once and
//! every arm that chose that model shares the observation, so arms are
//! matched. Qualification per (domain, learned arm) uses the HN-16 record and
//! gate with the MR-13 domain checks; only a `go` registers evidence and
//! installs a session lock for exactly that key.

use super::exec::{
    origin_rank, reconcile, router_charge, CellExecutor, CellRun, Reconciled, RouterCharge, Tokens,
};
use super::registry::{discover, Arm, ArmKind, LearnedProfile, Registry};
use super::{q, Item, Split, Stratum, TaskSet};
use crate::cli::Environment;
use crate::contract::{ProjectBinding, RequestEnvelope};
use crate::decision::call::ScoreKind;
use crate::decision::cost_route::{choose_start, CacheState, ChooseInputs, DEFAULT_MIN_TASKS};
use crate::decision::evidence::{
    Calibration, EvidenceKey, EvidenceRecord, EvidenceRegistry, Origin, Outcome, RetryOwner,
};
use crate::decision::governed::{ProfileStore, SessionLock};
use crate::decision::qualify::{
    calibrate_min_confidence, evaluate_domain, gate_for, DomainEvidence, DomainGateSpec,
    GateDecision, RULES_ARM,
};
use crate::decision::route_v2::{ExecutionDomain, RouteSignals};
use crate::decision::{
    decide, screen, ConfiguredProvider, DecisionCall, DecisionInvoker, DecisionSource,
    EnablementGate, GateStatus, ProviderMode, ProviderProfile, RouteContext, RouteInputs,
    RoutePolicy, RouteRequest,
};
use crate::diag::HarnessResult;
use crate::json;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

/// What a run is given: one executor and the live adapter sessions it may
/// consult, keyed by learned arm id. An arm without a session is unavailable.
pub struct Lane<'a> {
    pub executor: &'a mut dyn CellExecutor,
    pub invokers: BTreeMap<String, &'a mut dyn DecisionInvoker>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    pub item: String,
    pub domain: ExecutionDomain,
    pub partition: String,
    pub stratum: Stratum,
    pub split: Split,
    pub arm: String,
    /// Arm name in HN-16 outcomes (`rules`, the provider id, or the arm id).
    pub arm_label: String,
    pub model: Option<String>,
    pub origin: Origin,
    pub verified_by: String,
    pub completed: bool,
    pub regressions: u32,
    pub attempts: u32,
    pub failed_attempts: u32,
    pub gateway_retries: u32,
    pub cost_micros: Option<u64>,
    pub router: RouterCharge,
    pub router_ms: u64,
    pub latency_ms: Option<u64>,
    /// First execution of the model in this run (`Some(true)`) or a repeat.
    pub cold: Option<bool>,
    pub cache: Vec<&'static str>,
    pub tokens: Tokens,
    pub source: String,
    pub fallback: bool,
    /// The learned provider ran in shadow (automatic routing prohibited).
    pub shadow: bool,
    pub rules_choice: Option<String>,
    pub chosen_score: Option<f64>,
    pub wire_version: u32,
    pub forged: bool,
    pub errors: Vec<String>,
    pub unavailable: Option<String>,
    pub retry_owner: RetryOwner,
}

impl Cell {
    fn new(arm: &Arm, label: &str, item: &Item) -> Self {
        Self {
            item: item.id.clone(),
            domain: item.domain,
            partition: item.partition.clone(),
            stratum: item.stratum,
            split: item.split,
            arm: arm.id.clone(),
            arm_label: label.into(),
            model: None,
            origin: Origin::Unavailable,
            verified_by: String::new(),
            completed: false,
            regressions: 0,
            attempts: 0,
            failed_attempts: 0,
            gateway_retries: 0,
            cost_micros: None,
            router: RouterCharge::none(),
            router_ms: 0,
            latency_ms: None,
            cold: None,
            cache: vec![],
            tokens: Tokens::default(),
            source: String::new(),
            fallback: false,
            shadow: false,
            rules_choice: None,
            chosen_score: None,
            wire_version: 0,
            forged: false,
            errors: vec![],
            unavailable: None,
            retry_owner: RetryOwner::Host,
        }
    }

    pub fn outcome(&self) -> Outcome {
        Outcome {
            item: self.item.clone(),
            arm: self.arm_label.clone(),
            model: self.model.clone().unwrap_or_default(),
            origin: self.origin,
            verified_by: self.verified_by.clone(),
            completed: self.completed,
            regressions: self.regressions,
            attempts: self.attempts,
            cost_micros: self.cost_micros,
            latency_ms: self.latency_ms.map(|l| l + self.router_ms),
            router_cost_micros: self.router.micros,
            context_cost_micros: 0,
            retry_owner: self.retry_owner,
        }
    }
}

/// One gate outcome per (domain, learned arm).
#[derive(Clone, Debug)]
pub struct ArmDecision {
    pub domain: ExecutionDomain,
    pub arm: String,
    /// `go`, `no-go` or `not-evaluated`.
    pub status: &'static str,
    pub reasons: Vec<String>,
    pub decision: Option<GateDecision>,
    pub key: Option<EvidenceKey>,
    pub gate: Option<EnablementGate>,
    pub spec_digest: Option<String>,
}

pub struct MatrixRun {
    pub arms: Vec<Arm>,
    pub cells: Vec<Cell>,
    pub seal_digest: String,
    pub executor: String,
    pub executor_class: Origin,
    pub decisions: Vec<ArmDecision>,
    pub calibration: Vec<Value>,
    pub stores: BTreeMap<ExecutionDomain, ProfileStore>,
    pub registry: EvidenceRegistry,
}

impl MatrixRun {
    /// `go` if any arm qualified, `no-go` if any was evaluated, else
    /// `not-evaluated`.
    pub fn overall(&self) -> &'static str {
        if self.decisions.iter().any(|d| d.status == "go") {
            "go"
        } else if self.decisions.iter().any(|d| d.status == "no-go") {
            "no-go"
        } else {
            "not-evaluated"
        }
    }
}

/// Captures the chosen option's score of the last answered router call.
struct Recording<'b> {
    inner: &'b mut dyn DecisionInvoker,
    last: Option<Value>,
}

impl DecisionInvoker for Recording<'_> {
    fn evaluate(&mut self, request: &RequestEnvelope) -> DecisionCall {
        let c = self.inner.evaluate(request);
        if let DecisionCall::Answered { result, .. } = &c {
            self.last = Some(result.clone());
        }
        c
    }
    fn decision_versions(&self) -> Vec<u32> {
        self.inner.decision_versions()
    }
}

impl Recording<'_> {
    fn chosen_score(&self) -> Option<f64> {
        let r = self.last.as_ref()?;
        r["scores"][r["choice"].as_str()?].as_f64()
    }
}

pub(crate) fn inputs_for(tasks: &TaskSet, item: &Item) -> HarnessResult<RouteInputs> {
    let mut sig = RouteSignals::default();
    sig.execution_domain = item.domain;
    let request = RouteRequest::from_json(&tasks.route_doc(item))?.with_signals(sig);
    Ok(RouteInputs {
        request,
        policy: RoutePolicy::default(),
    })
}

fn ctx_for(item: &Item) -> RouteContext {
    let b = "b".repeat(64);
    RouteContext {
        project: ProjectBinding {
            id: b.clone(),
            worktree: b.clone(),
            revision: b.clone(),
        },
        lock_digest: b,
        invocation_id: format!("mr13-{}", item.id),
        lineage_id: format!("mr13-{}", item.id),
        router_lineage: vec![],
        router_calls_used: 0,
        router_ms_used: 0,
    }
}

fn profile_of(l: &LearnedProfile) -> ProviderProfile {
    ProviderProfile::configured(
        l.adapter.clone(),
        l.model_profile.clone(),
        l.instance.clone(),
    )
}

fn prohibited(i: &RouteInputs) -> bool {
    let f = i.request.features.task_family;
    i.policy.hard_families.contains(&f) || i.policy.rules_only_families.contains(&f)
}

struct Exec {
    run: CellRun,
    rec: Reconciled,
    forged: bool,
    cold: bool,
}

struct Ctx<'r, 'a> {
    reg: &'r Registry,
    tasks: &'r TaskSet,
    arms: &'r [Arm],
    executor: &'r mut dyn CellExecutor,
    invokers: &'r mut BTreeMap<String, &'a mut dyn DecisionInvoker>,
    memo: BTreeMap<(String, String), Exec>,
    calib: BTreeMap<ExecutionDomain, EvidenceRecord>,
}

impl Ctx<'_, '_> {
    fn label(&self, arm: &Arm) -> String {
        match &arm.kind {
            ArmKind::Rules => RULES_ARM.into(),
            ArmKind::Learned(i) => self.reg.learned[*i].adapter.provider_id.clone(),
            _ => arm.id.clone(),
        }
    }

    fn rules(&self, inputs: &RouteInputs, ctx: &RouteContext) -> HarnessResult<String> {
        Ok(decide(inputs, ctx, None, &|| inputs.clone(), None)?.choice)
    }

    fn cell(&mut self, arm: &Arm, item: &Item) -> HarnessResult<Cell> {
        let mut cell = Cell::new(arm, &self.label(arm), item);
        if let Some(why) = &arm.unavailable {
            cell.unavailable = Some(why.clone());
            return Ok(cell);
        }
        let inputs = inputs_for(self.tasks, item)?;
        let ctx = ctx_for(item);
        let choice = match &arm.kind {
            ArmKind::Rules => {
                let d = decide(&inputs, &ctx, None, &|| inputs.clone(), None)?;
                cell.source = format!("{:?}", d.source);
                Some(d.choice)
            }
            ArmKind::Fixed(m) => {
                let scr = screen(&inputs.request, &inputs.policy);
                cell.source = "fixed".into();
                if scr.admissible.iter().any(|p| &p.id == m) {
                    Some(m.clone())
                } else {
                    cell.unavailable =
                        Some("model not admissible under policy for this item".into());
                    None
                }
            }
            ArmKind::CostAware => Some(self.cost_aware(&inputs, &ctx, item, &mut cell)?),
            ArmKind::Learned(i) => {
                let l = &self.reg.learned[*i];
                let Some(inv) = self.invokers.get_mut(&l.arm_id) else {
                    cell.unavailable = Some("no live adapter session is bound for this run".into());
                    return Ok(cell);
                };
                let mut li = inputs.clone();
                if prohibited(&inputs) {
                    // Shadow: the recommendation never routes; it is executed
                    // as a counterfactual and can never qualify.
                    li.policy.hard_families.clear();
                    li.policy.rules_only_families.clear();
                    cell.shadow = true;
                }
                let mut rec = Recording {
                    inner: &mut **inv,
                    last: None,
                };
                let profile = profile_of(l);
                let d = {
                    let mut p = ConfiguredProvider {
                        profile: profile.clone(),
                        invoker: &mut rec,
                        mode: ProviderMode::Explicit,
                        gate: EnablementGate::not_evaluated(
                            &self.tasks.task,
                            &l.adapter.provider_id,
                        ),
                    };
                    decide(&li, &ctx, Some(&mut p), &|| li.clone(), None)?
                };
                cell.source = format!("{:?}", d.source);
                cell.fallback =
                    !matches!(d.source, DecisionSource::Provider | DecisionSource::Cache);
                cell.router = router_charge(&l.router_price, d.router_calls, d.wire.call.as_ref());
                cell.router_ms = d.router_ms;
                cell.wire_version = d.wire.version;
                cell.chosen_score = rec.chosen_score();
                if let Some(c) = &d.wire.call {
                    if let Err(e) = profile.verify_identity(c) {
                        cell.errors
                            .push(format!("answering identity mismatch: {e}"));
                    }
                }
                if cell.shadow {
                    cell.rules_choice = Some(self.rules(&inputs, &ctx)?);
                }
                Some(d.choice)
            }
        };
        let Some(model) = choice else {
            return Ok(cell);
        };
        cell.model = Some(model.clone());
        let backing = self
            .arms
            .iter()
            .find(|a| a.kind == ArmKind::Fixed(model.clone()));
        match backing {
            None => {
                cell.unavailable = Some(format!("no approved generation profile backs `{model}`"));
                return Ok(cell);
            }
            Some(g) if g.unavailable.is_some() => {
                cell.unavailable = Some(format!(
                    "generation profile `{model}` unavailable: {}",
                    g.unavailable.as_deref().unwrap_or("")
                ));
                return Ok(cell);
            }
            Some(g) if !g.domains.contains(&item.domain) => {
                cell.unavailable = Some(format!(
                    "generation profile `{model}` is not approved for {}",
                    item.domain.as_str()
                ));
                return Ok(cell);
            }
            Some(_) => {}
        }
        let key = (item.id.clone(), model.clone());
        if !self.memo.contains_key(&key) {
            let cold = !self.memo.keys().any(|(_, m)| *m == model);
            let class = self.executor.class();
            let mut run = self.executor.execute(item, &model);
            let forged = origin_rank(run.origin) > origin_rank(class);
            if forged {
                run.origin = class;
            }
            let rec = reconcile(&run, &model, &self.tasks.prices, &self.tasks.matched);
            self.memo.insert(
                key.clone(),
                Exec {
                    run,
                    rec,
                    forged,
                    cold,
                },
            );
        }
        let e = &self.memo[&key];
        if let Some(why) = &e.run.unavailable {
            cell.unavailable = Some(why.clone());
            return Ok(cell);
        }
        cell.origin = e.run.origin;
        cell.verified_by = self.tasks.verifier_label(item);
        cell.completed = e.run.completed;
        cell.regressions = e.run.regressions;
        cell.retry_owner = e.run.retry_owner;
        cell.attempts = e.rec.attempts;
        cell.failed_attempts = e.rec.failed_attempts;
        cell.gateway_retries = e.rec.gateway_retries;
        cell.cost_micros = e.rec.cost_micros;
        cell.latency_ms = e.rec.latency_ms;
        cell.cache = e.rec.cache.clone();
        cell.tokens = e.rec.tokens;
        cell.cold = Some(e.cold);
        cell.forged = e.forged;
        cell.errors.extend(e.rec.errors.iter().cloned());
        Ok(cell)
    }

    /// TC-10 total-cost routing over calibration evidence only; hard families
    /// and missing evidence keep the rules choice.
    fn cost_aware(
        &self,
        inputs: &RouteInputs,
        ctx: &RouteContext,
        item: &Item,
        cell: &mut Cell,
    ) -> HarnessResult<String> {
        let f = &inputs.request.features;
        if inputs.policy.hard_families.contains(&f.task_family) {
            cell.source = "rules (hard family)".into();
            return self.rules(inputs, ctx);
        }
        let scr = screen(&inputs.request, &inputs.policy);
        let mut ladder: Vec<&crate::decision::ModelPlan> = inputs.request.catalog.iter().collect();
        ladder.sort_by(|a, b| (a.strength_rank, &a.id).cmp(&(b.strength_rank, &b.id)));
        let ladder: Vec<String> = ladder.into_iter().map(|p| p.id.clone()).collect();
        let c = choose_start(&ChooseInputs {
            ladder: &ladder,
            pool: &scr.admissible,
            features: f,
            input_tokens: f.estimated_context_tokens,
            output_cap: 4096,
            prices: &self.tasks.prices,
            cache: CacheState::Conservative,
            allowance_micros: Some(inputs.request.budget.max_cost_micros),
            evidence: self.calib.get(&item.domain),
            min_tasks: DEFAULT_MIN_TASKS,
            pinned: false,
        });
        match c.model {
            Some(m) => {
                cell.source = "cost_aware".into();
                Ok(m)
            }
            None => {
                cell.source = format!("rules ({})", c.reason);
                self.rules(inputs, ctx)
            }
        }
    }
}

/// Calibration-split fixed-arm outcomes per domain, the only evidence the
/// cost-aware arm may read.
fn calibration_records(
    cells: &[Cell],
    tasks: &TaskSet,
) -> BTreeMap<ExecutionDomain, EvidenceRecord> {
    let mut out = BTreeMap::new();
    for d in ExecutionDomain::ALL {
        let outcomes: Vec<Outcome> = cells
            .iter()
            .filter(|c| {
                c.domain == *d && c.split == Split::Calibration && c.arm.starts_with("fixed:")
            })
            .map(Cell::outcome)
            .collect();
        out.insert(
            *d,
            EvidenceRecord {
                key: EvidenceKey {
                    task: tasks.task.clone(),
                    provider_id: "cost-aware".into(),
                    weights_digest: "calibration".into(),
                    catalog_digest: String::new(),
                    normalization: String::new(),
                    distribution: d.as_str().into(),
                },
                budget: tasks.matched,
                eval_items: BTreeSet::new(),
                trained_on: BTreeSet::new(),
                outcomes,
                calibration: None,
            },
        );
    }
    out
}

/// Run the whole matrix. Every domain spec is checked against the documented
/// floor before anything executes.
pub fn run(
    reg: &Registry,
    tasks: &TaskSet,
    specs: &[DomainGateSpec],
    env: &Environment,
    lane: Lane<'_>,
) -> HarnessResult<MatrixRun> {
    for s in specs {
        s.check_floor().map_err(|w| {
            q(
                "SPX-HPQ002",
                format!(
                    "{} gate spec is weaker than the documented floor: {w}",
                    s.domain.as_str()
                ),
            )
        })?;
    }
    let seal_digest = tasks.seal_digest();
    let Lane {
        executor,
        mut invokers,
    } = lane;
    let bound: BTreeSet<String> = invokers.keys().cloned().collect();
    let (executor_id, executor_class) = (executor.identity(), executor.class());
    let arms = discover(reg, tasks, env, &bound, executor_class != Origin::Real);
    let mut cx = Ctx {
        reg,
        tasks,
        arms: &arms,
        executor,
        invokers: &mut invokers,
        memo: BTreeMap::new(),
        calib: BTreeMap::new(),
    };
    let mut cells = Vec::new();
    for split in [Split::Calibration, Split::Eval] {
        if split == Split::Eval {
            cx.calib = calibration_records(&cells, tasks);
        }
        for arm in &arms {
            if split == Split::Calibration && arm.kind == ArmKind::CostAware {
                continue;
            }
            for item in tasks.items.iter().filter(|i| i.split == split) {
                if arm.domains.contains(&item.domain) {
                    cells.push(cx.cell(arm, item)?);
                }
            }
        }
    }
    let mut out = MatrixRun {
        arms: arms.clone(),
        cells,
        seal_digest,
        executor: executor_id,
        executor_class,
        decisions: vec![],
        calibration: vec![],
        stores: BTreeMap::new(),
        registry: EvidenceRegistry::default(),
    };
    qualify_all(reg, tasks, specs, &mut out)?;
    Ok(out)
}

fn median(mut v: Vec<u64>) -> Option<u64> {
    v.sort_unstable();
    v.get(v.len() / 2).copied().filter(|_| !v.is_empty())
}

pub(crate) fn latency_split(cells: &[&Cell]) -> Value {
    let pick = |cold: bool| {
        median(
            cells
                .iter()
                .filter(|c| c.cold == Some(cold))
                .filter_map(|c| c.latency_ms.map(|l| l + c.router_ms))
                .collect(),
        )
    };
    json!({"cold_median_ms": pick(true), "warm_median_ms": pick(false)})
}

/// Raw option calibration versus downstream task success, from calibration
/// cells only. Option mass is never reported as a success probability.
fn calibration_report(
    l: &LearnedProfile,
    domain: ExecutionDomain,
    cells: &[Cell],
    key: &EvidenceKey,
) -> (Value, Option<Calibration>) {
    let mine = |split: Split| -> Vec<&Cell> {
        cells
            .iter()
            .filter(|c| c.arm == l.arm_id && c.domain == domain && c.split == split)
            .filter(|c| c.origin != Origin::Unavailable)
            .collect()
    };
    let cal = mine(Split::Calibration);
    // Oracle-best option: the cheapest model whose fixed-arm calibration cell
    // was accepted for that item.
    let mut best: BTreeMap<&str, (u64, &str)> = BTreeMap::new();
    for c in cells.iter().filter(|c| {
        c.split == Split::Calibration
            && c.domain == domain
            && c.arm.starts_with("fixed:")
            && c.completed
    }) {
        let cost = c.cost_micros.unwrap_or(u64::MAX);
        let m = c.model.as_deref().unwrap_or("");
        let e = best.entry(c.item.as_str()).or_insert((cost, m));
        if cost < e.0 {
            *e = (cost, m);
        }
    }
    let samples: Vec<(f64, bool)> = cal
        .iter()
        .filter_map(|c| c.chosen_score.map(|s| (s, c.completed)))
        .collect();
    let bins = [(0.0, 0.5), (0.5, 0.8), (0.8, 1.000_001)];
    let bin = |want_best: bool| -> Vec<Value> {
        bins.iter()
            .map(|(lo, hi)| {
                let inb: Vec<&&Cell> = cal
                    .iter()
                    .filter(|c| c.chosen_score.is_some_and(|s| s >= *lo && s < *hi))
                    .collect();
                let hit = inb
                    .iter()
                    .filter(|c| {
                        if want_best {
                            best.get(c.item.as_str()).map(|b| b.1) == c.model.as_deref()
                        } else {
                            c.completed
                        }
                    })
                    .count();
                let n = inb.len();
                json!({"range": [lo, hi.min(1.0)], "n": n,
                       "mean_score": (n > 0).then(|| inb.iter().filter_map(|c| c.chosen_score).sum::<f64>() / n as f64),
                       "rate": (n > 0).then(|| hit as f64 / n as f64)})
            })
            .collect()
    };
    let threshold = calibrate_min_confidence(&samples, 0.8, 10);
    let eval = mine(Split::Eval);
    let eval_scored = eval.iter().filter(|c| !c.shadow).count();
    let coverage = threshold.map(|t| {
        let covered = eval
            .iter()
            .filter(|c| !c.shadow && c.chosen_score.is_some_and(|s| s >= t))
            .count();
        if eval_scored == 0 {
            0.0
        } else {
            covered as f64 / eval_scored as f64
        }
    });
    let calibration = threshold
        .filter(|_| l.model_profile.score_kind != ScoreKind::None)
        .map(|t| {
            let above: Vec<&(f64, bool)> = samples.iter().filter(|s| s.0 >= t).collect();
            Calibration {
                calibration_id: json::digest(
                    "semaprax.harness-routing-calibration.v1",
                    &json!(samples
                        .iter()
                        .map(|s| json!([s.0, s.1]))
                        .collect::<Vec<_>>()),
                ),
                score_kind: l.model_profile.score_kind,
                key_digest: key.digest(),
                success_estimate: above.iter().filter(|s| s.1).count() as f64
                    / above.len().max(1) as f64,
            }
        });
    let report = json!({
        "arm": l.arm_id, "domain": domain.as_str(), "score_kind": l.model_profile.score_kind.as_str(),
        "calibration_samples": samples.len(),
        "raw_option_calibration": {"meaning": "chosen-option score versus whether that option was the cheapest accepted option for the item (calibration split only)", "bins": bin(true)},
        "downstream_success": {"meaning": "chosen-option score versus independently verified task acceptance (calibration split only)", "bins": bin(false)},
        "threshold": threshold, "calibrated_coverage": coverage,
        "note": "option mass is not a probability of downstream task success; neither curve extends beyond the sampled tasks and hardware"});
    (report, calibration)
}

fn qualify_all(
    reg: &Registry,
    tasks: &TaskSet,
    specs: &[DomainGateSpec],
    out: &mut MatrixRun,
) -> HarnessResult<()> {
    let domains: BTreeSet<ExecutionDomain> = tasks.items.iter().map(|i| i.domain).collect();
    for d in domains {
        let spec = specs.iter().find(|s| s.domain == d);
        for arm in out.arms.clone().iter().filter(|a| a.domains.contains(&d)) {
            let ArmKind::Learned(i) = arm.kind else {
                continue;
            };
            let l = &reg.learned[i];
            let mut ad = ArmDecision {
                domain: d,
                arm: arm.id.clone(),
                status: "not-evaluated",
                reasons: vec![],
                decision: None,
                key: None,
                gate: None,
                spec_digest: spec.map(DomainGateSpec::digest),
            };
            let Some(spec) = spec else {
                ad.reasons
                    .push(format!("no reviewed gate spec for {}", d.as_str()));
                out.decisions.push(ad);
                continue;
            };
            if let Some(why) = &arm.unavailable {
                ad.reasons.push(format!("arm unavailable: {why}"));
                out.decisions.push(ad);
                continue;
            }
            let first = tasks
                .items
                .iter()
                .find(|x| x.domain == d)
                .expect("domain has items");
            let catalog = inputs_for(tasks, first)?.request.catalog_digest();
            let mine = |c: &&Cell| c.arm == l.arm_id && c.domain == d;
            let version = out
                .cells
                .iter()
                .filter(mine)
                .map(|c| c.wire_version)
                .max()
                .unwrap_or(0)
                .max(1);
            let key = EvidenceKey::live_versioned(&profile_of(l), &catalog, version).bound(
                d,
                &tasks.candidate_revision,
                &tasks.renderer_revision,
            );
            let eligible: BTreeSet<String> = out
                .cells
                .iter()
                .filter(mine)
                .filter(|c| c.split == Split::Eval && !c.shadow)
                .map(|c| c.item.clone())
                .collect();
            let calib_items: Vec<&Item> = tasks
                .items
                .iter()
                .filter(|x| x.domain == d && x.split == Split::Calibration)
                .collect();
            let mut trained_on: BTreeSet<String> = l.trained_on.clone();
            trained_on.extend(calib_items.iter().map(|x| x.id.clone()));
            let seen: BTreeSet<&str> = calib_items
                .iter()
                .map(|x| x.content_digest.as_str())
                .chain(l.trained_on.iter().map(String::as_str))
                .collect();
            for x in tasks
                .items
                .iter()
                .filter(|x| x.domain == d && x.split == Split::Eval)
            {
                if seen.contains(x.content_digest.as_str()) {
                    trained_on.insert(x.id.clone());
                }
            }
            let (cal_report, calibration) = calibration_report(l, d, &out.cells, &key);
            let counted: Vec<&Cell> = out
                .cells
                .iter()
                .filter(|c| c.domain == d && c.split == Split::Eval && eligible.contains(&c.item))
                .filter(|c| c.arm == "rules" || c.arm == l.arm_id)
                .collect();
            let record = EvidenceRecord {
                key: key.clone(),
                budget: tasks.matched,
                eval_items: eligible.clone(),
                trained_on,
                outcomes: counted.iter().map(|c| c.outcome()).collect(),
                calibration,
            };
            let ev = DomainEvidence {
                domain: d,
                forged_origin: counted.iter().filter(|c| c.forged).count(),
                unreconciled: counted.iter().filter(|c| !c.errors.is_empty()).count(),
                shadow_only: eligible.is_empty(),
                record: record.clone(),
            };
            out.calibration.push(cal_report);
            let dec = evaluate_domain(spec, &ev, &key);
            ad.status = if dec.go { "go" } else { "no-go" };
            ad.reasons = dec.reasons.clone();
            ad.key = Some(key.clone());
            if dec.go {
                out.registry.register(record)?;
                let (gate, _) = gate_for(&out.registry, &key, &spec.spec);
                if matches!(gate.status, GateStatus::Passed { .. }) {
                    out.stores.entry(d).or_default().install(SessionLock {
                        key_digest: key.digest(),
                        record_digest: dec.record_digest.clone(),
                    });
                }
                ad.gate = Some(gate);
            }
            ad.decision = Some(dec);
            out.decisions.push(ad);
        }
    }
    Ok(())
}

/// A new session may use a learned profile only when the domain's active
/// qualified lock is for exactly the live key.
pub fn session_admits(store: Option<&ProfileStore>, live: &EvidenceKey) -> bool {
    store
        .and_then(ProfileStore::lock_session)
        .is_some_and(|l| l.key_digest == live.digest())
}
