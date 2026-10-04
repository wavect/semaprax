//! Human `REPORT.md`, derived only from the machine summary and gates.

use serde_json::Value;
use std::fmt::Write;

fn n(v: &Value) -> String {
    match v {
        Value::Null => "n/a".into(),
        Value::String(s) => s.clone(),
        o => o.to_string(),
    }
}

pub fn render(label: &str, summary: &Value, gates: &Value, pilot: &Value, env: &Value) -> String {
    let mut o = String::new();
    let _ = writeln!(o, "# Harness benchmark report: {label}\n");
    let _ = writeln!(
        o,
        "Contract `semaprax.harness-benchmark.v1`. Corpus digest `{}`. Baseline profile `{}`. Trials per cell: {} cold + {} warm.\n",
        n(&summary["corpus_digest"]), n(&summary["baseline_profile"]), n(&summary["trials"]["cold"]), n(&summary["trials"]["warm"])
    );
    let _ = writeln!(o, "## Labels\n");
    let _ = writeln!(o, "- Measurement unit: `byte-v1` (UTF-8 bytes of the final model-visible envelope). No named tokenizer is available on this machine, so **no token counts are reported** and no byte figure is a token or a billing figure.");
    let _ = writeln!(
        o,
        "- Tested local support: cells marked `ok` below, run on this one machine ({}).",
        n(&env["os"])
    );
    let _ = writeln!(o, "- Protocol-only hosted support (Jev, remote gateways): **not benchmarked**; protocol contract tests only.");
    let _ = writeln!(
        o,
        "- Untested platforms: Linux and Windows. Nothing here supports a claim for them."
    );
    let _ = writeln!(o, "- Historical evidence (earlier HP lane reports, ADR 0001, the #309 case study) is cited for context only and is not mixed into these numbers.");
    let _ = writeln!(
        o,
        "- Zero observed failures is not proof of safety: the corpus is small and seeded.\n"
    );

    let _ = writeln!(o, "## Per-profile results (all applicable cells)\n");
    let _ = writeln!(o, "| profile | cells ok/failed/untested | accepted (n) | Wilson 95% | fact retention | false neg. | visible bytes | incurred | bytes / accepted | warm p50/p95 ms | cold p50/p95 ms |");
    let _ = writeln!(o, "|---|---|---|---|---|---|---|---|---|---|---|");
    for (p, s) in summary["profiles"].as_object().into_iter().flatten() {
        let c = &s["cells"];
        let _ = writeln!(
            o,
            "| {p} | {}/{}/{} | {:.2} ({}) | {}-{} | {} | {} | {} | {} | {} | {}/{} | {}/{} |",
            n(&c["ok"]),
            n(&c["failed"]),
            n(&c["untested"]),
            s["accepted"]["rate"].as_f64().unwrap_or(0.0),
            n(&s["accepted"]["n"]),
            n(&s["accepted"]["wilson95"][0]),
            n(&s["accepted"]["wilson95"][1]),
            n(&s["critical_fact_retention"]),
            n(&s["false_negatives"]),
            n(&s["bytes"]["model_visible"]),
            n(&s["bytes"]["incurred"]),
            n(&s["bytes"]["per_accepted_task"]),
            n(&s["latency_ms"]["warm"]["p50"]),
            n(&s["latency_ms"]["warm"]["p95"]),
            n(&s["latency_ms"]["cold"]["p50"]),
            n(&s["latency_ms"]["cold"]["p95"]),
        );
    }
    let _ = writeln!(o, "\nRows cover different task sets (a profile only runs tasks it can change); compare profiles only through the matched table below.\n");

    let _ = writeln!(
        o,
        "## Matched comparison against the baseline (same task and trial)\n"
    );
    let _ = writeln!(o, "| profile | matched cells | accepted delta / cell | visible bytes delta / cell | latency ms delta / cell |");
    let _ = writeln!(o, "|---|---|---|---|---|");
    for (p, m) in summary["vs_baseline"].as_object().into_iter().flatten() {
        let _ = writeln!(
            o,
            "| {p} | {} | {} | {} | {} |",
            n(&m["matched_cells"]),
            n(&m["accepted_delta_per_cell"]),
            n(&m["visible_bytes_delta_per_cell"]),
            n(&m["latency_ms_delta_per_cell"])
        );
    }
    let _ = writeln!(o, "\nNegative visible-bytes delta = fewer bytes than the baseline. A combined profile is an ablation of its parts: its delta is not the sum of per-plugin savings, and each part is judged on the tasks it touches.\n");

    let _ = writeln!(o, "## Costs, resources and reconciliation\n");
    for (p, s) in summary["profiles"].as_object().into_iter().flatten() {
        let r = &s["resources"];
        let _ = writeln!(
            o,
            "- `{p}`: calls {}, retries {}, detail retrievals {}; child CPU user {} ms / sys {} ms; max RSS {} KiB; disk left behind (max) {} bytes; observation report partial: {}; reconciliation mismatches: {}.",
            n(&s["calls"]), n(&s["retries"]), n(&s["detail_retrievals"]),
            n(&r["user_ms"]["sum"]), n(&r["sys_ms"]["sum"]), n(&r["max_rss_kb"]["max"]), n(&r["disk_bytes"]),
            n(&s["observation_report"]["partial"]), n(&s["reconciliation_mismatches"].as_array().map_or(0, Vec::len).into()),
        );
    }

    let _ = writeln!(o, "\n## Adverse cases and negative savings (retained)\n");
    let mut any = false;
    for (p, tasks) in summary["per_task"].as_object().into_iter().flatten() {
        if p == summary["baseline_profile"].as_str().unwrap_or("") {
            continue;
        }
        for (t, v) in tasks.as_object().into_iter().flatten() {
            let base = &summary["per_task"][summary["baseline_profile"].as_str().unwrap_or("")][t];
            let (a, b) = (
                v["visible_bytes_mean"].as_f64(),
                base["visible_bytes_mean"].as_f64(),
            );
            if let (Some(a), Some(b)) = (a, b) {
                if a > b {
                    any = true;
                    let _ = writeln!(
                        o,
                        "- `{p}` on `{t}`: {a:.0} visible bytes vs baseline {b:.0} (**more**)."
                    );
                }
            }
            if v["accepted"] != v["n"] {
                any = true;
                let _ = writeln!(
                    o,
                    "- `{p}` on `{t}`: accepted {} of {} ({}).",
                    n(&v["accepted"]),
                    n(&v["n"]),
                    n(&v["reason"])
                );
            }
        }
    }
    if !any {
        let _ = writeln!(o, "- none observed in applicable cells");
    }

    let _ = writeln!(o, "\n## Seeded adversarial cases\n");
    for a in summary["adversarial"].as_array().into_iter().flatten() {
        let _ = writeln!(
            o,
            "- `{}` ({}): {} {}",
            n(&a["id"]),
            n(&a["kind"]),
            if a["detected"] == true {
                "DETECTED"
            } else if a["untested"].is_string() {
                "UNTESTED"
            } else {
                "NOT DETECTED"
            },
            n(&a["untested"])
        );
        for e in a["evidence"].as_array().into_iter().flatten() {
            let _ = writeln!(o, "  - {}", n(e));
        }
    }

    let _ = writeln!(
        o,
        "\n## Gates (declared in docs/HARNESS-BENCHMARK-V1.md before measurement)\n"
    );
    for g in gates["gates_declared"].as_array().into_iter().flatten() {
        let _ = writeln!(o, "- **{}**: {}", n(&g["id"]), n(&g["rule"]));
    }
    let mut eligible: Vec<String> = vec![];
    for (p, scopes) in gates["scopes"].as_object().into_iter().flatten() {
        let _ = writeln!(o, "\n### `{p}`\n");
        for (fam, sc) in scopes.as_object().into_iter().flatten() {
            let fails: Vec<String> = sc["gates"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|g| g["pass"] != true)
                .map(|g| format!("{} ({})", n(&g["id"]), n(&g["detail"])))
                .collect();
            let _ = writeln!(
                o,
                "- `{fam}`: matched {}; eligible for auto-enable: **{}**{}",
                n(&sc["matched_cells"]),
                n(&sc["auto_enable_eligible"]),
                if fails.is_empty() {
                    String::new()
                } else {
                    format!("; failed: {}", fails.join("; "))
                }
            );
            if sc["auto_enable_eligible"] == true {
                eligible.push(format!("`{p}` for `{fam}`"));
            }
        }
    }

    let _ = writeln!(o, "\n## Local model pilot\n");
    if pilot.is_null() {
        let _ = writeln!(o, "Not run.");
    } else {
        let _ = writeln!(o, "Status `{}`, label `{}`, model `{}` (digest {}), {} calls, min trials per configuration {}. {}", n(&pilot["status"]), n(&pilot["label"]), n(&pilot["model"]), n(&pilot["model_digest"]), n(&pilot["calls"]), n(&pilot["min_trials_per_configuration"]), n(&pilot["caveat"]));
        for c in pilot["configurations"].as_array().into_iter().flatten() {
            let _ = writeln!(
                o,
                "- {}: {}/{} correct",
                n(&c["config"]),
                n(&c["correct"]),
                n(&c["n"])
            );
        }
        if pilot["status"] != "ran" {
            let _ = writeln!(o, "{}", n(&pilot["reason"]));
        }
    }

    let _ = writeln!(o, "\n## Recommendation\n");
    if eligible.is_empty() {
        let _ = writeln!(o, "**native-only stays the default.** No provider profile passed every gate on any measured task scope, so none is recommended for automatic use. Providers remain opt-in; an integration that failed a gate above should stay disabled for that scope.");
    } else {
        let _ = writeln!(o, "Automatic defaults are justified only for these measured scopes (every other scope stays opt-in): {}.", eligible.join(", "));
    }
    let _ = writeln!(o, "\nEach automatic default above is tied to its task family, matched-cell count and gate verdicts in the Gates section; a scope with fewer than 10 matched cells cannot pass G4-G6.");
    o
}
