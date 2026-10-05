# Mini Jev local closed-choice decision adapter v1 (MR-04)

Audience: toolchain contributors and harness adapter authors.

Status: **experimental, contract-tested, live-unverified.** No real Mini Jev
inference was run for this adapter (reason below), `support.tested` in the
descriptor is empty, and the explicit `experimental` routing mode is the only
supported way to use it. Nothing here is routing-accuracy evidence.

Owner: `packages/semaprax-harness-adapters/minijev-local/`. Contract:
`decision.evaluate` **v2**, task `model-route/v2` (see
[HARNESS-DECISION-V1](HARNESS-DECISION-V1.md) and
[HARNESS-PROVIDER-V1](HARNESS-PROVIDER-V1.md)). It is a decision adapter, not a
generation provider, and it grants no authority.

## What it is

Mini Jev (`r-ms/mini-jev`) reads a multiple-choice answer from the model's last
hidden state: one prefill, fp32 logits of the letter tokens `A..`, softmax over
those letters. This adapter exposes exactly that mechanism for the routing
question "which candidate model handles this step". It is **not** a TypeSafe
hosted model, not Mini Jev's `demo/server.py` bench, and runs no sequence
generation, JSON-constrained decoding or label-likelihood baseline.

| Pin | Value |
| --- | --- |
| Upstream | `https://github.com/r-ms/mini-jev` @ `ca612198bfb69f538f029a4615f6d0a18b4f814c` (MIT). Files read: `minijev/engine.py`, `minijev/letters.py`, `minijev/prompts.py`, `minijev/config.py`, `demo/server.py`, `pyproject.toml` |
| Upstream dependency pins (its `pyproject.toml`) | Python 3.11, `torch==2.5.1`, `transformers==4.57.6`, `accelerate==1.14.0`, `numpy>=1.24,<2.0` (it also lists `datasets`, `xgrammar`; the worker path imports neither) |
| Upstream default model | `Qwen/Qwen3-4B-Instruct-2507` rev `cdbee75f17c01a7cc42f958dc650907174af0554` (about 8 GB); smoke model `Qwen/Qwen3-0.6B` |
| Reused upstream functions | `Engine.last_hidden`, `letters.build_tables`, `letters.candidate_logits_fp32`, `letters.score` (bare- and space-token conventions and tie flag preserved), `prompts.render` with `prompts.SYSTEM_B` |
| Not used | `Engine.generate`, `run_json`, `run_split`, `run_labels`, `sequence_logprobs`, `full_logits_from_hidden` (bf16 full-vocabulary logits), `demo/server.py` |

## Architecture and lifecycle

```
host --stdio frames--> adapter.py --loopback TCP--> worker.py (warm model) --> mini-jev engine
```

- `adapter.py` is stdlib-only. It never starts, installs, restarts or downloads
  anything. A missing worker is `unavailable` (`SPX-HPK016`) and the decision
  layer falls back under its normal policy.
- `worker.py` is **started and owned by the user** (documented command below),
  loads one model once, binds loopback only, authenticates with a token from
  `SEMAPRAX_HARNESS_SECRET_MINIJEV`, and stops on SIGINT/SIGTERM. One forward
  pass runs at a time; at most `--max-queue` (default 4) further requests wait;
  beyond that the answer is `busy`, which the adapter maps to `unavailable`.
  A request whose client disconnected while queued is dropped without spending
  a forward pass. A forward pass already running cannot be interrupted; the
  adapter returns at once on cancel or deadline and the bounded pass finishes
  unobserved.
- The worker sets `HF_HUB_OFFLINE=1` before importing the model libraries, so
  evaluating a route can never download weights. It refuses to start unless the
  mini-jev checkout is exactly at the pinned commit with no local modification,
  the model revision is a full 40-hex commit, and all 26 letters are single
  tokens in both the bare and the space-prefixed form (upstream
  `build_tables`). A tokenizer that encodes a letter in several tokens is
  refused; there is no multi-token heuristic.

Worker wire (newline-delimited JSON, loopback): line 1 `{"auth": token}`; then
`{"v":1,"op":"ping"}` returns the identity block and
`{"v":1,"op":"score","k":N,"user":text}` returns `logits`, `p_cand`, `pred_pos`,
`tie`, `gap`, `prompt_tokens`, `identity`, `warm`, `latency_ms`, `rss_mb`.
Request lines are bounded at 16 KiB, prompts at 8 KiB before tokenization and
`--max-prompt-tokens` (default 2048) after it; responses the adapter reads are
bounded at 8 KiB.

## Mapping a `model-route/v2` decision

Selection ids `m0..m{n-1}` map to letters `A..` by position and back. The
host-rendered `rendered.state`, `rendered.instructions` and `rendered.option_labels`
are used verbatim inside the fixed template `minijev-letters.v1` (shaped like
upstream's `user_mcq`):

```
TEXT:
<rendered.state>

QUESTION: <rendered.instructions>
A = m0: <label>
B = m1: <label>
...

ANSWER:
```

The chat template and system prompt are upstream's (`SYSTEM_B`,
`enable_thinking=False`).

| Result member | Value |
| --- | --- |
| `scores` | softmax over the fp32 candidate-letter logits, recomputed by the adapter from the logits (worker `p_cand` must agree within 1e-3) |
| `score_kind` | `candidate_relative` (relative preference among the offered letters, not a calibrated probability of success) |
| `choice` | the highest-logit letter mapped back to its `m<i>` |
| abstention | `native` when upstream reports a tie (fp32 equal maxima; upstream would pick the lowest position), or when `choice_confidence` (upstream's own formula) is below `SEMAPRAX_HARNESS_NATIVE_MIN_CONFIDENCE` |
| `native_confidence` | `choice_confidence`, kind `minijev.choice_confidence` |
| `call.identity_kind` | `local_declared` |
| `call.checkpoint` | `rev:<40-hex model revision>+tok:<12 hex of tokenizer digest>+code:<12 hex of mini-jev commit>+rnd:<8 hex renderer pin>` |
| `call.usage` | `input_tokens` measured by the worker, `output_tokens` 0, basis `local_measured` |
| `call.billing` | `local` |
| `call.wire_bytes` | length of the serialized score line sent to the worker |

The `tok` digest is sha256 over the sorted vocabulary plus the chat template;
the `rnd` pin is sha256 of the template id and the upstream system prompt. Any
change to model revision, tokenizer, mini-jev code or option renderer changes
`call.checkpoint`, and therefore the cache, evidence and qualification keys that
bind it (HARNESS-DECISION-V1 "Evidence key"): evidence never transfers.
Additionally the adapter itself checks, **before any scoring work**, that the
worker's code commit equals the pinned commit and (when a profile is supplied)
that model and checkpoint equal the profile; a mismatch is `refused`
(`SPX-HPK009`), not silently accepted under a new identity. A worker engine
other than `minijev` is refused unless `SEMAPRAX_HARNESS_MINIJEV_ALLOW_FAKE=1`
(tests only).

## Bounds

Intersection of host and upstream: 2 to 16 options (upstream demo admits 2..26;
a single admissible option never reaches an adapter because the host bypasses
inference, and an adapter that is handed one answers `unsupported`);
`rendered.state` at most 4096 bytes (profile `max_state_bytes`); whole prompt
at most 8 KiB before tokenization and 2048 tokens after; serialized score line
at most the request's `max_wire_bytes`; text input only. Oversized input is
refused, never truncated.

## Refusals (none produces an accepted route)

| Code | Status | Cause |
| --- | --- | --- |
| HPK002 | refused | worker address missing or not loopback |
| HPK003 | refused | worker token not provided by the host |
| HPK004 | unsupported | not `model-route/v2`; single option; image input; foreign renderer; unsupported tokenizer/encoding reported by the worker; non-Mini-Jev engine |
| HPK005 | refused | more than 16 options, state/prompt/token bound, `max_wire_bytes` |
| HPK006 | refused | malformed profile or threshold |
| HPK007 | refused | worker response over the bound |
| HPK009 | refused | non-finite, short/long, out-of-range or mutually inconsistent scores (probabilities vs logits, prediction vs argmax, tie flag); identity incomplete, changed during the call, or different from the pinned profile; code outside the pinned commit |
| HPK010 | refused | duplicate or foreign options (candidate ids must be exactly `m0..m{n-1}`, labels exactly the options) |
| HPK011 | failed | deadline exhausted |
| HPK012 | refused/failed | worker auth failure, engine fault |
| HPK013 | refused | cancelled |
| HPK016 | unavailable | worker not running, died mid-call, or busy |

## Adoption

```sh
semaprax harness adopt packages/semaprax-harness-adapters/minijev-local/harness-provider.json
semaprax harness trust org.r-ms/minijev-local
```

`semaprax.harness.toml`:

```toml
schema = "semaprax.harness-config.v1"

[capability."decision.evaluate"]
mode = "auto"
provider = "org.r-ms/minijev-local"

[routing]
mode = "experimental"     # the only mode this profile may use until matched task evidence qualifies it
allow_remote = false
```

Host environment for the adapter process (the host passes these; the adapter reads
nothing else): `SEMAPRAX_HARNESS_MINIJEV_ADDR=127.0.0.1:<port>`,
`SEMAPRAX_HARNESS_SECRET_MINIJEV=<token>`, optionally
`SEMAPRAX_HARNESS_MODEL_PROFILE` and `SEMAPRAX_HARNESS_NATIVE_MIN_CONFIDENCE`.

An optional pinning profile (the value of `SEMAPRAX_HARNESS_MODEL_PROFILE`; take
`checkpoint` from the worker's identity as shown by `decide_once.py` output
`call.checkpoint`):

```json
{"profile_id": "minijev-qwen3-0.6b", "model": "Qwen/Qwen3-0.6B",
 "checkpoint": "rev:<40-hex>+tok:<12>+code:ca612198bfb6+rnd:<8>",
 "identity_kind": "local_declared", "score_kind": "candidate_relative", "scoreless": false,
 "max_options": 16, "max_state_bytes": 4096, "modalities": ["text"]}
```

## Reproducible local inference (not run here; see "What is unverified")

Provisioning is explicit and separate from the adapter; the adapter and the
worker never do it. Python 3.11, about 1.5 GiB of model plus a few hundred MiB
of libraries for the 0.6B smoke model.

```sh
# 1. pinned upstream code
git clone https://github.com/r-ms/mini-jev target/minijev/mini-jev
git -C target/minijev/mini-jev checkout ca612198bfb69f538f029a4615f6d0a18b4f814c
# 2. private venv with upstream's pins (the worker path imports only torch + transformers)
uv venv --python 3.11 target/minijev-venv
uv pip install --python target/minijev-venv/bin/python torch==2.5.1 transformers==4.57.6 accelerate==1.14.0 'numpy>=1.24,<2.0'
# 3. one explicit model download into a private cache (Qwen/Qwen3-0.6B, revision read from the Hub on 2026-10-05)
HF_HOME=$PWD/target/minijev-hf target/minijev-venv/bin/python -c \
  "from huggingface_hub import snapshot_download as d; d('Qwen/Qwen3-0.6B', revision='c1899de289a04d12100db370d81485cdf75e47ca')"
# 4. start the warm worker (offline; prints LISTENING host:port, load seconds and RSS)
export SEMAPRAX_HARNESS_SECRET_MINIJEV=$(python3 -c 'import secrets;print(secrets.token_hex(16))')
HF_HOME=$PWD/target/minijev-hf target/minijev-venv/bin/python \
  packages/semaprax-harness-adapters/minijev-local/worker.py serve \
  --minijev-dir target/minijev/mini-jev --model Qwen/Qwen3-0.6B \
  --revision c1899de289a04d12100db370d81485cdf75e47ca --device cpu --dtype float32 --port 18431 &
# 5. one decision between two models through the adapter, over the SDK frame protocol
SEMAPRAX_HARNESS_MINIJEV_ADDR=127.0.0.1:18431 \
  python3 packages/semaprax-harness-adapters/minijev-local/decide_once.py
```

Step 5 prints the full result envelope: choice, candidate-relative scores,
`call.checkpoint` with the pinned identities, and an `SPX-HPK100` diagnostic with
warm/cold, `latency_ms` and `rss_mb`. The first request after start is
`warm=false`; the worker line printed at startup gives load time and resident
memory. Stop the worker with `kill %1`. To remove everything:
`rm -rf target/minijev target/minijev-venv target/minijev-hf`.

Without torch the same path runs against the counting fake engine for demos:
`SEMAPRAX_HARNESS_SECRET_MINIJEV=... python3 worker.py serve --engine fake` with
`SEMAPRAX_HARNESS_MINIJEV_ALLOW_FAKE=1` on the adapter. Its identity says
`engine: fake`; it is never a model result.

## Tests

`python3 -m unittest discover -s packages/semaprax-harness-adapters/minijev-local/tests`
(54 tests, about 20 s, stdlib only): the shared SDK conformance matrix plus
a counting fake engine proving **exactly one scoring execution and zero
generate/decode/baseline calls per decision**; duplicate/foreign options,
unsupported encodings, over-limit options/context, malformed scores (NaN, inf,
wrong length, range, probability vs logit, wrong argmax, lying tie); dead worker,
mid-call worker death, busy worker, cancellation (including a queued request that
is dropped before it reaches the engine), exhausted deadline; no network,
subprocess or download code in the adapter; identity changes (revision,
tokenizer, system prompt/renderer, code commit, model) visible in
`call.checkpoint` and refused against a pinned profile before scoring.

## What is unverified

- **No live Mini Jev inference was run.** The smallest supported model
  (`Qwen/Qwen3-0.6B`) has a 1,503,300,328 byte `model.safetensors` plus about
  15 MB of tokenizer files (1.42 GiB) before any library: torch 2.5.1 alone is a
  64 MB wheel that installs to several hundred MiB, with transformers and its
  dependencies on top. Together they exceed the 1.5 GiB total budget this lane
  was given, so the run was not started (free disk was 4.3 GiB). The upstream
  default model is about 8 GB. Acceptance item "a real provisioned worker
  selects one of at least two models" is therefore open; the command above
  reproduces it on a host with the disk.
- `RealEngine` (worker.py) is read-checked against the pinned upstream source but
  never executed: its torch calls (`last_hidden`, `candidate_logits_fp32`,
  `score`, tokenizer identity hash) are untested. Latency, memory and the
  warm/cold gap on real hardware are unmeasured; no hardware/backend
  combination is supported yet (the intended first target is CPU float32 on
  macOS arm64 and Linux).
- Single-row `Engine.last_hidden` with left padding is used rather than the
  upstream broadcast-prefix optimisation; the demo's per-field path equals this
  for one question, but numeric parity with the upstream run has not been checked.
- Qualification and any accuracy claim: none. The profile is uncalibrated and
  unevaluated on matched tasks; use only the experimental mode.
