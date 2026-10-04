//! One trial: one task, one arm, one model, one repetition. Every model call is
//! recorded, including failed attempts; the grader (never the model) decides
//! success. Records hold counts, digests and verdicts, never prompt or answer text.

use super::arms::{self, Arm, ContextPack, ContextSpec, ViewSpec};
use super::model::{Generation, ModelClient, ModelError};
use super::task::{self, Files, Task, TaskSet, Tools};
use super::tokens::TokenCounter;
use crate::json::sha256_plain;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, Instant};

pub const TRIAL_SCHEMA: &str = "semaprax.harness-apptrial.v1";

#[derive(Clone, Debug)]
pub struct ModelSpec {
    pub id: String,
    /// `small`, `large`.
    pub size: String,
    /// Provider-billed (ledger applies) or local.
    pub billed: bool,
}

pub struct TrialEnv<'a> {
    pub tasks: &'a TaskSet,
    pub tools: &'a Tools,
    pub work: &'a Path,
    pub counter: &'a dyn TokenCounter,
    /// Cold index/retrieval results per (task, context kind), built once.
    pub packs: &'a BTreeMap<(String, String), ContextPack>,
    /// Retrieval tool definitions (by id) for live re-indexing in later steps.
    pub arm_set: &'a arms::ArmSet,
    /// Rendered skill blocks per kind (see `arms::skill_blocks`).
    pub skills: &'a BTreeMap<String, arms::SkillBlock>,
}

#[derive(Clone, Debug)]
pub struct TrialKey {
    pub task: String,
    pub arm: String,
    pub model: String,
    pub rep: u32,
}

impl TrialKey {
    pub fn id(&self) -> String {
        format!("{}|{}|{}|{}", self.task, self.arm, self.model, self.rep)
    }
}

fn status_record(
    key: &TrialKey,
    task: &Task,
    arm: &Arm,
    model: &ModelSpec,
    status: &str,
    reason: &str,
) -> Value {
    json!({"schema": TRIAL_SCHEMA, "trial": key.id(), "task": task.id, "class": task.class, "arm": arm.id, "arm_role": format!("{:?}", arm.role),
           "model": model.id, "size": model.size, "rep": key.rep, "cold": key.rep == 0, "status": status, "reason": reason, "passed": false})
}

/// Context shown for a step: full tree or retrieval, from the live state.
fn context_for(
    env: &TrialEnv,
    task: &Task,
    arm: &Arm,
    step: usize,
    state: &Files,
    request: &str,
    slot: &str,
) -> ContextPack {
    match &arm.context {
        ContextSpec::Native => arms::native_pack(state),
        ContextSpec::Retrieval { tool } if step == 0 => env
            .packs
            .get(&(task.id.clone(), tool.clone()))
            .cloned()
            .unwrap_or_else(|| ContextPack {
                unavailable: Some("pack not built".into()),
                ..ContextPack::default()
            }),
        ContextSpec::Retrieval { tool } => match env.arm_set.retrieval.get(tool) {
            Some(t) => arms::retrieval_pack(t, task, state, request, env.tools, env.work, slot),
            None => ContextPack {
                unavailable: Some(format!("unknown retrieval tool {tool}")),
                ..ContextPack::default()
            },
        },
    }
}

fn sha(s: &str) -> String {
    sha256_plain(s.as_bytes())
}

/// Run one trial. Never panics on a model or tool failure: those are recorded.
pub fn run_trial(
    env: &TrialEnv,
    key: &TrialKey,
    arm: &Arm,
    model: &ModelSpec,
    client: &dyn ModelClient,
) -> Value {
    let Some(task) = env.tasks.task(&key.task) else {
        return json!({"schema": TRIAL_SCHEMA, "trial": key.id(), "status": "error", "reason": "unknown task", "passed": false});
    };
    let wall = Instant::now();
    let Some(skill) = env.skills.get(&arm.id).cloned() else {
        return status_record(
            key,
            task,
            arm,
            model,
            "untested",
            "skill block not rendered",
        );
    };
    if arm.skill != arms::SkillSpec::None && !skill.delivered {
        return status_record(
            key,
            task,
            arm,
            model,
            "untested",
            &format!("skill not delivered: {}", skill.note.unwrap_or_default()),
        );
    }
    let skill_tokens = env.counter.count(&skill.text).unwrap_or(0);
    let home = env.work.join("home");
    let slot = format!(
        "{}-{}-{}-{}",
        sanitize(&key.arm),
        sanitize(&key.model),
        key.rep,
        std::process::id()
    );
    let sandbox = env.work.join("sb").join(&slot);
    let mut state: Files = task.project.clone();
    let seed = 1000 + key.rep as u64;
    let (mut prompt_tok, mut answer_tok, mut prov_in, mut prov_out) = (0u64, 0u64, 0u64, 0u64);
    let (mut cost, mut cost_known, mut model_ms, mut harness_ms, mut attempts_n, mut tamper) =
        (0f64, false, 0u64, 0u64, 0u32, 0u32);
    let mut prov_known = true;
    let mut steps_json = vec![];
    let mut context_json = Value::Null;
    let (mut all_passed, mut first_attempt_ok, mut first_valid) = (true, false, false);
    let mut status = "ok";
    let mut reason = String::new();

    'steps: for (si, step) in task.steps.iter().enumerate() {
        let pack = context_for(env, task, arm, si, &state, &step.request, &slot);
        if let Some(why) = &pack.unavailable {
            status = "untested";
            reason = format!("context tool unavailable: {why}");
            all_passed = false;
            break 'steps;
        }
        // Cold trial pays index construction; every trial pays its own retrieval.
        let (build, retrieval) = if si == 0 {
            (
                if key.rep == 0 { pack.build_ms } else { 0 },
                pack.retrieval_ms,
            )
        } else {
            (pack.build_ms, pack.retrieval_ms)
        };
        harness_ms += build + retrieval;
        // The failing-run output a developer would show the model.
        let mut view_ms = 0;
        let mut view_text: Option<String> = None;
        let wants_view = step.initial_command.is_some() || task.class == "compile_repair";
        if wants_view && task::prepare_sandbox(&sandbox, &state).is_ok() {
            let g0;
            let raw = if let Some(ic) = &step.initial_command {
                match env
                    .tools
                    .expand(&ic.cmd)
                    .and_then(|a| arms::view_argv(&arm.view, &a, env.tools))
                {
                    Ok(argv) => {
                        let r =
                            task::run_cmd(&argv, &sandbox, &home, &[], Duration::from_secs(120));
                        view_ms = r.ms;
                        Some(arms::render_view(&arm.view, &r.combined()))
                    }
                    Err(e) => {
                        status = "untested";
                        reason = e;
                        None
                    }
                }
            } else {
                g0 = task::grade(&sandbox, &home, task, si, env.tools);
                view_ms = g0.ms;
                Some(arms::render_view(&arm.view, &g0.output_tail))
            };
            match raw {
                Some(t) => view_text = Some(t),
                None => {
                    all_passed = false;
                    break 'steps;
                }
            }
        }
        harness_ms += view_ms;
        let first_prompt =
            arms::build_prompt(&skill.text, &pack.text, &step.request, view_text.as_deref());
        let ctx_tokens = env.counter.count(&pack.text).unwrap_or(0);
        if si == 0 {
            let refs: Vec<&String> = step
                .reference_files
                .iter()
                .filter(|f| task.project.contains_key(*f))
                .collect();
            let recall = (!refs.is_empty()).then(|| {
                refs.iter()
                    .filter(|f| pack.files_in_full.contains(**f))
                    .count() as f64
                    / refs.len() as f64
            });
            context_json = json!({"kind": arm.retrieval_tool().unwrap_or("native"), "bytes": pack.text.len(), "tokens_o200k": ctx_tokens,
                "files_in_full": pack.files_in_full, "reference_recall": recall, "index_build_ms": pack.build_ms, "retrieval_ms": pack.retrieval_ms,
                "index_bytes": pack.index_bytes, "identity": pack.identity, "view": view_label(&arm.view),
                "view_bytes": view_text.as_ref().map(|t| t.len())});
        }
        let mut prompt = first_prompt.clone();
        let mut acc: Vec<(String, String)> = vec![];
        let mut attempts_json = vec![];
        let mut step_passed = false;
        for n in 1..=step.max_attempts {
            attempts_n += 1;
            let ptok = env.counter.count(&prompt).unwrap_or(0);
            prompt_tok += ptok;
            let g: Result<Generation, ModelError> = client.generate(&prompt, seed + n as u64);
            let g = match g {
                Ok(g) => g,
                Err(ModelError::Budget(m)) => {
                    status = "budget";
                    reason = m;
                    all_passed = false;
                    attempts_json.push(json!({"n": n, "prompt_bytes": prompt.len(), "prompt_tokens_o200k": ptok, "error": "budget-refused"}));
                    steps_json
                        .push(json!({"id": step.id, "attempts": attempts_json, "passed": false}));
                    break 'steps;
                }
                Err(ModelError::Failed(m)) => {
                    attempts_json.push(json!({"n": n, "prompt_bytes": prompt.len(), "prompt_tokens_o200k": ptok, "error": m}));
                    prov_known = false;
                    continue;
                }
            };
            model_ms += g.latency_ms;
            let atok = env.counter.count(&g.text).unwrap_or(0);
            answer_tok += atok;
            match (g.provider_in, g.provider_out) {
                (Some(i), Some(o)) => {
                    prov_in += i + g.cache_read.unwrap_or(0) + g.cache_write.unwrap_or(0);
                    prov_out += o;
                }
                _ => prov_known = false,
            }
            if let Some(c) = g.cost_usd {
                cost += c;
                cost_known = true;
            }
            let mut parsed = task::parse_answer(&g.text);
            let produced = parsed.edits.len();
            if arm.answer_filter.as_deref() == Some("keep_first_file") {
                parsed.edits.truncate(1);
            }
            let dropped = produced - parsed.edits.len();
            let (ok, refused) = task::split_edits(task, &parsed.edits);
            tamper += refused.len() as u32;
            for (p, c) in &ok {
                acc.retain(|(q, _)| q != p);
                acc.push((p.clone(), c.clone()));
            }
            let valid = !ok.is_empty()
                && refused.is_empty()
                && parsed.unsafe_paths.is_empty()
                && parsed.unterminated.is_empty();
            if si == 0 && n == 1 {
                first_valid = valid;
            }
            let mut trial_files = state.clone();
            trial_files.extend(acc.iter().cloned());
            let grade = if task::prepare_sandbox(&sandbox, &trial_files).is_ok() {
                task::grade(&sandbox, &home, task, si, env.tools)
            } else {
                task::Grade {
                    output_tail: "sandbox error".into(),
                    ..Default::default()
                }
            };
            harness_ms += grade.ms;
            if let Some(why) = &grade.untested {
                status = "untested";
                reason = why.clone();
                all_passed = false;
                attempts_json.push(json!({"n": n, "error": why}));
                steps_json.push(json!({"id": step.id, "attempts": attempts_json, "passed": false}));
                break 'steps;
            }
            attempts_json.push(json!({"n": n, "prompt_bytes": prompt.len(), "prompt_tokens_o200k": ptok, "answer_bytes": g.text.len(), "answer_tokens_o200k": atok,
                "provider_in": g.provider_in, "provider_out": g.provider_out, "cache_read": g.cache_read, "cache_write": g.cache_write,
                "cost_usd": g.cost_usd, "latency_ms": g.latency_ms,
                "blocks": {"applied": ok.iter().map(|(p, _)| p).collect::<Vec<_>>(), "rejected_protected": refused, "unsafe": parsed.unsafe_paths, "unterminated": parsed.unterminated},
                "files_dropped_by_filter": dropped, "structurally_valid": valid, "grade_passed": grade.passed, "grade_failed_cmd": grade.failed_cmd, "grade_ms": grade.ms,
                "answer_sha256": sha(&g.text)}));
            if grade.passed {
                step_passed = true;
                if si == 0 && n == 1 {
                    first_attempt_ok = true;
                }
                break;
            }
            let feedback = feedback_view(
                arm,
                task,
                si,
                &grade,
                &sandbox,
                &home,
                env.tools,
                &mut harness_ms,
            );
            prompt = arms::retry_prompt(&first_prompt, &g.text, &feedback);
        }
        steps_json.push(json!({"id": step.id, "attempts": attempts_json, "passed": step_passed}));
        if !step_passed {
            all_passed = false;
            break 'steps;
        }
        state.extend(acc);
    }
    let _ = std::fs::remove_dir_all(&sandbox);
    let calls: u32 = attempts_n;
    let mut rec = json!({
        "schema": TRIAL_SCHEMA, "trial": key.id(), "task": task.id, "class": task.class, "arm": arm.id, "arm_role": format!("{:?}", arm.role),
        "model": model.id, "size": model.size, "rep": key.rep, "cold": key.rep == 0, "seed": seed, "status": status, "reason": reason,
        "skill": {"ids": skill.ids, "delivered": skill.delivered, "bytes": skill.text.len(), "tokens_o200k": skill_tokens},
        "context": context_json, "steps": steps_json,
        "passed": all_passed && status == "ok", "passed_first_attempt": first_attempt_ok && task.steps.len() == 1, "structurally_valid_first": first_valid,
        "tamper_attempts": tamper,
        "totals": {"prompt_tokens_o200k": prompt_tok, "answer_tokens_o200k": answer_tok, "tokens_o200k": prompt_tok + answer_tok,
                   "provider_in": prov_known.then_some(prov_in), "provider_out": prov_known.then_some(prov_out),
                   "cost_usd": cost_known.then_some((cost * 1e6).round() / 1e6), "model_ms": model_ms, "harness_ms": harness_ms,
                   "completion_ms": model_ms + harness_ms, "wall_ms": wall.elapsed().as_millis() as u64, "attempts": attempts_n, "calls": calls},
    });
    if status != "ok" {
        rec["passed"] = json!(false);
    }
    rec
}

#[allow(clippy::too_many_arguments)]
fn feedback_view(
    arm: &Arm,
    task: &Task,
    step: usize,
    grade: &task::Grade,
    sandbox: &Path,
    home: &Path,
    tools: &Tools,
    harness_ms: &mut u64,
) -> String {
    let wrapped = matches!(arm.view, ViewSpec::Wrap { .. });
    if wrapped {
        if let Some(i) = grade.failed_cmd {
            if let Ok(argv) = tools
                .expand(&task.steps[step].grade[i].cmd)
                .and_then(|a| arms::view_argv(&arm.view, &a, tools))
            {
                let r = task::run_cmd(&argv, sandbox, home, &[], Duration::from_secs(120));
                *harness_ms += r.ms;
                return task::tail_bytes(&r.combined(), 3000);
            }
        }
    }
    arms::render_view(&arm.view, &grade.output_tail)
}

fn view_label(v: &ViewSpec) -> String {
    match v {
        ViewSpec::Raw => "raw".into(),
        ViewSpec::Stripped => "stripped".into(),
        ViewSpec::Wrap { env, args } => format!("wrap:{env}:{}", args.join(" ")),
    }
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}
