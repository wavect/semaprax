# Reuse-skill matched trial: claude-haiku-4-5 (HP-13)

Status: retained evidence, 2026-10-04, explicitly authorized remote trial.

Audience: harness maintainers.

The `pilot.json` here records `endpoint: loopback` because the benchmark only
talks to a loopback Ollama-shaped endpoint. That endpoint was `shim.py` (copied
here), which forwarded each prompt to **Anthropic-hosted `claude-haiku-4-5`**
through the local Claude Code CLI 2.1.289 (`claude -p --model haiku --tools ""
--setting-sources project`, empty cwd, fixed system prompt). This is remote
inference, not local support. `shim-calls.jsonl` holds per-call return code,
latency, cost and prompt/answer digests (no text): 20 calls, USD 0.0648.

Result (10 matched trials per arm, deterministic `multiply(` check): with the
adopted `reuse-before-generation` skill 10/10 reused the existing API; without
it also 10/10. The skill adds 1,730 prompt bytes and shows **no measurable lift**
on this task (ceiling effect: the compiler context already names
`calculator.multiply`). Compiler gating and the unchanged `semaprax.toml` are
shown by the `native+skill` workflow cells, not by the model.

Two earlier attempts through the same shim were discarded as invalid: the shim
answered with an `HTTP/1.0` status line, which the pilot client rejects, so
every row recorded a transport error (about USD 0.12 spent). The local
`qwen2.5-coder:1.5b` run (`../2026-10-04-skill-coder`) is valid: 0/10 vs 0/10.
