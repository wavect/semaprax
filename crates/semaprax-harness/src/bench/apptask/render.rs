//! REPORT.md for the application-task campaign. Every table is per model and
//! task class; nothing is pooled across classes, and losses stay in.

use serde_json::Value;
use std::fmt::Write;

fn f(v: &Value, d: usize) -> String {
    v.as_f64()
        .map(|x| format!("{x:.d$}"))
        .unwrap_or_else(|| "n/a".into())
}

/// Change against native from a reduction figure: `-` = fewer tokens or cheaper.
fn chg(v: &Value) -> String {
    v.as_f64()
        .map(|x| format!("{:+.1}%", -x * 100.0))
        .unwrap_or_else(|| "n/a".into())
}

fn acc(v: &Value) -> String {
    let (k, n) = (v["k"].as_u64().unwrap_or(0), v["n"].as_u64().unwrap_or(0));
    format!(
        "{k}/{n} ({}-{})",
        f(&v["wilson95"][0], 2),
        f(&v["wilson95"][1], 2)
    )
}

fn mean_sd(v: &Value) -> String {
    format!("{} +/- {}", f(&v["mean"], 0), f(&v["sd"], 0))
}

pub fn render(summary: &Value, recs: &Value, meta: &Value, ledger: &Value, label: &str) -> String {
    let mut o = String::new();
    let _ = writeln!(
        o,
        "# Harness application-task benchmark report (HN-17): {label}\n"
    );
    let cs = &summary["cell_size_check"];
    let _ = writeln!(o, "Contract `semaprax.harness-apptask-summary.v1`. Task-set digest `{}`. Trials recorded: {}. Repetitions per (task, arm, model) cell: {} to {} over {} cells: **{}** (non-pilot needs at least {}).\n",
        meta["taskset_digest"].as_str().unwrap_or("?"), summary["trials_total"], cs["min_trials_per_cell"], cs["max_trials_per_cell"], cs["cells"], cs["label"].as_str().unwrap_or("?").to_uppercase(), cs["required_for_non_pilot"]);
    for (m, v) in cs["by_model"].as_object().into_iter().flatten() {
        let _ = writeln!(
            o,
            "- Model `{m}`: {} to {} repetitions per cell: **{}**.",
            v["min"],
            v["max"],
            v["label"].as_str().unwrap_or("?").to_uppercase()
        );
    }
    o.push_str("\n## Labels and identities\n\n");
    let _ = writeln!(o, "- Tokens: `{}` counts of the exact text sent (all attempts, skill prompt, retrieval pack and command view included) and of the exact answers. These are not billing units for any provider.", meta["tokenizer"].as_str().unwrap_or("?"));
    o.push_str("- Provider usage: the provider's own reported input/output tokens (Claude Code CLI JSON `usage`, Ollama `prompt_eval_count`/`eval_count`). Claude figures include the CLI's own system overhead; they are kept separate from the named-tokenizer counts and never merged.\n");
    o.push_str("- Cost: USD from the CLI's `total_cost_usd` for the larger model; the local model has no billing (`unavailable`, never zero). Cost is not estimated from tokens.\n");
    o.push_str("- Success: decided only by the immutable grader (compiler checks and test oracles the model cannot edit; protected paths are restored before grading). Structural validity (parsable file blocks, no protected edit) is reported separately and is never success.\n");
    o.push_str("- Delivery versus influence: `skill delivered` means the host framed and sent the byte-exact upstream skill; influence is only what the matched comparison shows in output tokens and accepted rate.\n");
    o.push_str("- Local macOS aarch64 evidence only. No Linux, Windows or hosted run.\n\n");
    if let Some(ids) = meta["identities"].as_object() {
        o.push_str("| identity | value |\n| --- | --- |\n");
        for (k, v) in ids {
            let text = match (v["path"].as_str(), v["version"].as_str()) {
                (Some(p), ver) => format!("`{p}` {}", ver.unwrap_or("(no version output)")),
                _ => v
                    .as_str()
                    .map(String::from)
                    .unwrap_or_else(|| v.to_string()),
            };
            let text: String = text
                .chars()
                .take(400)
                .collect::<String>()
                .replace(['|', '\n'], " ");
            let _ = writeln!(o, "| {k} | {text} |");
        }
        o.push('\n');
    }
    o.push_str("## Spend and ledger\n\n");
    let _ = writeln!(o, "Cap USD {} (user-authorized), spent USD {}, ledger calls {}, calls refused {}. Models: {}\n",
        f(&ledger["cap_usd"], 2), f(&ledger["spent_usd"], 4), ledger["calls"], ledger["refused_calls"], meta["models_note"].as_str().unwrap_or(""));
    o.push_str("## Arms\n\n| arm | role | description |\n| --- | --- | --- |\n");
    for a in meta["arms"].as_array().into_iter().flatten() {
        let _ = writeln!(
            o,
            "| `{}` | {} | {} |",
            a["id"].as_str().unwrap_or(""),
            a["role"].as_str().unwrap_or(""),
            a["label"].as_str().unwrap_or("")
        );
    }
    for (id, why) in meta["untested_arms"].as_object().into_iter().flatten() {
        let _ = writeln!(o, "| `{id}` | untested | {} |", why.as_str().unwrap_or(""));
    }
    o.push_str("\n## Results per model, task class and arm\n\n");
    for (model, classes) in summary["by_model_class_arm"]
        .as_object()
        .into_iter()
        .flatten()
    {
        let _ = writeln!(o, "### Model `{model}`\n");
        for (class, arms) in classes.as_object().into_iter().flatten() {
            let _ = writeln!(o, "#### {class}\n");
            o.push_str("| arm | trials ok/other | accepted (Wilson 95%) | first attempt (single-step) | valid blocks first | tokens o200k total mean+/-sd | answer tokens | provider in/out (mean) | USD/trial | USD/accepted | completion ms mean | attempts | cold acc. | warm acc. | per-task accepted |\n");
            o.push_str("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n");
            for (arm, s) in arms.as_object().into_iter().flatten() {
                let cost = &s["cost_usd"];
                let per_task: Vec<String> = s["per_task_accepted"]
                    .as_object()
                    .into_iter()
                    .flatten()
                    .map(|(k, v)| {
                        format!(
                            "{}:{}",
                            k.split('-').take(3).collect::<Vec<_>>().join("-"),
                            v.as_str().unwrap_or("")
                        )
                    })
                    .collect();
                let _ = writeln!(o, "| `{arm}` | {}/{} | {} | {}/{} | {}/{} | {} | {} | {}/{} | {} | {} | {} | {} | {} | {} | {} |",
                    s["ok"], s["not_ok"], acc(&s["accepted"]), s["accepted_first_attempt_single_step"]["k"], s["accepted_first_attempt_single_step"]["n"],
                    s["structurally_valid_first_attempt"]["k"], s["structurally_valid_first_attempt"]["n"], mean_sd(&s["tokens_o200k"]["total"]), f(&s["tokens_o200k"]["output"]["mean"], 0),
                    f(&s["provider_usage"]["input"]["mean"], 0), f(&s["provider_usage"]["output"]["mean"], 0),
                    if cost["per_trial"].is_null() { "n/a (local)".to_string() } else { f(&cost["per_trial"]["mean"], 5) },
                    if cost["per_accepted"].is_null() { "n/a".to_string() } else { f(&cost["per_accepted"], 5) },
                    f(&s["completion_ms"]["mean"], 0), f(&s["attempts"]["mean"], 2),
                    format!("{}/{}", s["cold"]["accepted"]["k"], s["cold"]["accepted"]["n"]), format!("{}/{}", s["warm"]["accepted"]["k"], s["warm"]["accepted"]["n"]),
                    per_task.join(" "));
            }
            o.push('\n');
        }
    }
    o.push_str("## Matched comparison against native, with predeclared gates\n\nMatched = same task, repetition and model. Gates (declared in `docs/HARNESS-BENCHMARK-V1.md` before measurement): N at least 10 matched cells; Q accepted delta >= 0 (no loss); C total o200k tokens (skill, retrieval and failed attempts included) down at least 20% and billed cost down at least 20% where billed; T added completion time <= 2000 ms per cell. Negative controls are judged by whether the oracle caught them.\n\n");
    o.push_str("| model | class | arm | matched | accepted delta/cell | total tokens | output tokens | billed cost | added ms/cell | N | Q | C | T | verdict |\n| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n");
    let yn = |b: &Value| {
        if b.as_bool() == Some(true) {
            "pass"
        } else {
            "FAIL"
        }
    };
    for c in summary["comparisons"].as_array().into_iter().flatten() {
        let p = &c["paired_vs_native"];
        let g = &p["gates"];
        let _ = writeln!(
            o,
            "| {} | {} | `{}` | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | **{}** |",
            c["model"].as_str().unwrap_or(""),
            c["class"].as_str().unwrap_or(""),
            c["arm"].as_str().unwrap_or(""),
            p["matched_cells"],
            f(&p["accepted_delta_per_cell"], 3),
            chg(&p["total_tokens_o200k_reduction"]),
            chg(&p["output_tokens_o200k_reduction"]),
            if p["billed_cost_reduction"].is_null() {
                "n/a".into()
            } else {
                chg(&p["billed_cost_reduction"])
            },
            f(&p["added_completion_ms_per_cell"], 0),
            yn(&g["N_matched_at_least_10"]),
            yn(&g["Q_quality_no_loss"]),
            yn(&g["C_cost_reduction_20pct"]),
            yn(&g["T_latency_within_2000ms"]),
            c["verdict"].as_str().unwrap_or("")
        );
    }
    o.push_str("\nToken and cost columns show the change against native: `-` = fewer tokens or cheaper, `+` = more.\n\n");
    o.push_str("## Mixed small-then-large cascade (derived)\n\nThe large model is asked only when the small model's answer failed the grader; computed from paired trials of the two sizes, not an independent run.\n\n| class | arm | pairs | accepted | escalation rate | mean tokens | mean USD |\n| --- | --- | --- | --- | --- | --- | --- |\n");
    for c in summary["cascade_mixed"].as_array().into_iter().flatten() {
        let _ = writeln!(
            o,
            "| {} | `{}` | {} | {} | {} | {} | {} |",
            c["class"].as_str().unwrap_or(""),
            c["arm"].as_str().unwrap_or(""),
            c["pairs"],
            acc(&c["accepted"]),
            f(&c["escalation_rate"], 2),
            f(&c["mean_tokens_o200k"], 0),
            f(&c["mean_billed_usd"], 5)
        );
    }
    o.push_str("\n## Scoped recommendations\n\nAdvisory only. No entry changes configuration, routing or skill state. A `qualified-scoped` entry is an input for the HN-05 lock, HN-06 skill preset and HN-16 evidence contracts (`recommendations.json`, `outcomes.json`); anything else keeps its tool available without an automatic-performance claim.\n\n| arm | class | model | verdict | matched | reason |\n| --- | --- | --- | --- | --- | --- |\n");
    for e in recs["entries"].as_array().into_iter().flatten() {
        let _ = writeln!(
            o,
            "| `{}` | {} | {} | **{}** | {} | {} |",
            e["arm"].as_str().unwrap_or(""),
            e["task_class"].as_str().unwrap_or(""),
            e["model"].as_str().unwrap_or(""),
            e["verdict"].as_str().unwrap_or(""),
            e["matched_cells"],
            e["reason"].as_str().unwrap_or("")
        );
    }
    o.push('\n');
    o
}
