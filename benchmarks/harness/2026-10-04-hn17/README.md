# HN-17 application-task campaign, 2026-10-04

Status: retained local evidence (macOS aarch64), explicitly authorized remote spend (USD 15 hard cap).

Audience: harness maintainers deciding default skills and ablations.

`REPORT.md` is generated from `trials.jsonl` (`semaprax-harness bench app report .`); `summary.json`,
`recommendations.json` and `outcomes.json` are its machine forms. Records are metadata only.

- Large model: `claude-haiku-4-5` through the Claude Code CLI 2.1.289 behind `shim.py` (loopback,
  Ollama-shaped, own USD ledger `shim-ledger.json`, per-call log `shim-calls.jsonl` without text).
  Settings: system prompt replaced, no tools, `MAX_THINKING_TOKENS=0`, isolated empty cwd. 10 repetitions
  per (task, arm) cell: non-pilot. Total ledger spend USD 10.52 (harness `ledger.json`), of which USD 1.24
  was a discarded calibration pass with thinking enabled (`calibration-thinking-on/`, not in any figure).
- Small model: local Ollama `qwen2.5:0.5b`, 1 to 2 repetitions on the core arms plus one control: **pilot**.
  It was accepted 0 times by the graders in every arm (floor, mostly invalid file blocks); no comparison is
  informative.
- `discarded-ponytail-undelivered.jsonl`: 122 `ponytail` records marked untested by a harness bug (a `stop`
  persisted in the scratch skills home by another arm made the skill undelivered). Fixed (one fresh home per
  arm), the arm was re-run in full. They were never counted.
- Honest limits: haiku accepts 120/120 native trials (ceiling), so no arm can show a quality gain on this task
  set; skills only cost tokens here. Controls: the harness-level lossy control is detected in five of six
  classes and not in `failing_tests`, where the retry session recovers the dropped file; the prompt-level
  control is obeyed only partly by haiku. Graphify cannot index `.spx` (compile-repair-spx cells untested).
