# Harness endpoint adoption v1 (HP-12)

Audience: toolchain contributors and harness adapter authors.

Owner: `crates/semaprax-harness/src/endpoint/`. Diagnostics letter `L`.
Machine-local state: `<harness_home>/endpoints.json`
(`semaprax.harness-endpoints.v1`, canonical JSON, atomic write). Nothing is
written to the project; credentials are environment variable NAMES only.

## CLI

```text
endpoints <project> [list] [--json]
endpoints <project> adopt --url http://127.0.0.1:PORT --kind ollama|litellm|openai-compatible
          [--id ID] [--model M] [--disclosure file.json] [--credential-env NAME]
          [--local-only] [--strict-one-attempt]
endpoints <project> bind <logical-id> --endpoint ID --model M --protocol responses|chat|anthropic
          [--rank N] [--local-only] [--strict-one-attempt]
endpoints <project> reprobe [ID]            # per-binding valid / invalid(code)
endpoints <project> litellm-config --logical L --upstream U --api-base URL
```

Adoption is explicit, loopback-only (`http://127.0.0.1|localhost|[::1]:PORT`,
`HPL002`), downloads nothing and refuses an endpoint with no models (`HPL006`).
The credential value is read from `Environment.vars[NAME]` at call time, sent
only as the `Authorization` header and never stored or echoed.

## Probing

Model list: Ollama `/api/tags` + `/api/show` (digest, context length,
capabilities, `remote_host`); others `/v1/models`. On one model, each key gets a
verdict `supported|unsupported|unverified` plus evidence (status line and shape,
no timings): `responses`, `responses_streaming`, `chat_completions`,
`chat_streaming`, `anthropic_messages`, `tool_calls` (schema accepted; whether a
call was emitted is recorded), `usage`, `structured_output` (the reply must
parse against the schema; accepted-but-not-enforced is `unsupported`).
404/405/501/other 4xx and wrong reply shapes are `unsupported`; 401/403/429/5xx
and connection failures are `unverified`. A binding to a protocol that is not
`supported` is refused (`HPL032`): there is no silent downgrade.

## Ownership and disclosure

`AttemptOwnership { semaprax_selects_logical_model: true, gateway_balancing:
none|equivalent_deployments_only|undisclosed, gateway_retries:
disabled|exact(n)|undisclosed, gateway_fallbacks: disabled|disclosed(list)|undisclosed }`.
`max_upstream_attempts() = (1+retries)*(1+fallbacks)`, `None` when undisclosed.
A direct Ollama server has no gateway layer (`disabled`). A gateway is
`undisclosed` until the operator supplies `semaprax.harness-endpoint-disclosure.v1`
(strict; unknown members `HPL005`):

```json
{"schema": "semaprax.harness-endpoint-disclosure.v1",
 "destinations": [{"kind": "local"}], "balancing": "none",
 "retries": "disabled", "fallbacks": "disabled"}
```

Destination: Ollama models without `remote_host` are `Local`; otherwise from the
disclosure; otherwise `Remote{origin:"unknown"}`. A loopback address is never
evidence of local inference. Policy refusals: `HPL010` undisclosed fallback under
local-only, `HPL011` non-local destination or remote fallback, `HPL012` strict
one-attempt unmet or unverifiable. `litellm-config` prints the reviewed snippet
(`num_retries: 0`, no fallback keys, key from `LITELLM_MASTER_KEY`).

## Bindings and outcomes

`LogicalModel { id, endpoint_id, upstream_model, protocol, capabilities,
observed_model_identity, observed_returned_model, observed_catalog_digest,
destination, attempt_owner, max_context, strength_rank }`;
`to_model_plan(est_cost_micros, est_latency_ms) -> decision::ModelPlan`. The plan
does not depend on direct vs gateway transport. Re-probe statuses: `HPL030`
model missing, `HPL031` identity changed, `HPL032` protocol no longer supported,
`HPL033` catalog digest changed (re-bind required), `HPL034` endpoint missing.
Call outcomes (`assess_reply`/`assess_stream`): `HPL020` protocol mismatch,
`HPL021` protocol unsupported, `HPL022` unsupported tool schema, `HPL023`
missing usage (usage JSON says `"unknown"`, never 0), `HPL024` returned identity
changed, `HPL025` identity unreported, `HPL026` other failure. Other codes:
`HPL001` usage, `HPL003` probe I/O, `HPL004` catalog file, `HPL007` credential
must be a NAME.

## Real local evidence (2026-10-04, macOS arm64)

Versions: Ollama 0.35.1 (existing app, `ollama serve`), model `qwen2.5:0.5b`
(pulled by `ollama pull qwen2.5:0.5b`, 379 MB, digest `a8b0c515...`), LiteLLM
1.104.0 (`uv tool install 'litellm[proxy]==1.104.0'`, 499 MB at
`~/.local/share/uv/tools/litellm`, uv cache removed). Config
`/private/tmp/claude-501/hp-tools/litellm/config.yaml` (no global config touched):
`semaprax-local` -> `ollama_chat/qwen2.5:0.5b` and `semaprax-missing` ->
`ollama_chat/does-not-exist:1b`, both `api_base` the test's counting proxy on
127.0.0.1:11435, `num_retries: 0` everywhere, no fallbacks. Started with
`LITELLM_MASTER_KEY=... litellm --config config.yaml --host 127.0.0.1 --port 4000`.

Command (test starts the counting proxy that forwards to Ollama):

```sh
HARNESS_OLLAMA_URL=http://127.0.0.1:11434 HARNESS_LITELLM_URL=http://127.0.0.1:4000 \
HARNESS_LITELLM_KEY=<key> cargo test --offline -p semaprax-harness \
  --test real_tools_v1 endpoints:: -- --ignored --nocapture
```

Result: `1 passed`. Verdicts (identical for Ollama and LiteLLM):
`responses` supported, `responses_streaming` supported (Ollama 9 SSE events,
LiteLLM 10), `chat_completions` supported, `chat_streaming` supported,
`anthropic_messages` supported (`/v1/messages` answered by both),
`tool_calls` supported (schema accepted, no call emitted by the 0.5b model),
`usage` supported (provider-reported on both), `structured_output` supported
(`{"color":"Red"}` / `{"color":"orange"}` parsed against the schema).

- Cancellation: a streaming `/v1/responses` request closed after 3 events on both
  endpoints (`cancelled=true`, status 200). Upstream cancellation is not proven.
- Failure: LiteLLM `semaprax-missing` returned 404 and the counting proxy saw
  exactly one upstream request (`POST /api/chat`), stable over 1.5 s; a direct
  nonexistent-model call also counted 1. No retry storm.
- Usage: Ollama reports cached input tokens (24); LiteLLM's `cached_input_tokens`
  is `unknown`. Returned model: Ollama `qwen2.5:0.5b`; LiteLLM chat returns the
  logical `semaprax-local`, so its identity is only `Reported`, while the
  streaming Responses `response.created` leaks the upstream `qwen2.5:0.5b`.
- Policy: the disclosed local one-attempt gateway satisfied `--local-only
  --strict-one-attempt` (`attempt_owner = semaprax`); the same gateway without a
  disclosure was refused (`HPL011`, destination remote-unknown).
- Gateway `max_context` is 0 (not exposed by `/v1/models`); Ollama's is 32768.

## Bridge guidance

Both Ollama 0.35.1 and LiteLLM 1.104.0 serve `/v1/responses` non-streaming and
streaming SSE with usage. The toolchain bridge should use the existing
`OpenAiResponsesAdapter` (path `/v1/responses`); no new protocol adapter is
missing. What is missing is a plain-HTTP loopback `HostHttpStreamTransport`
(the existing buffered transport is HTTPS-origin based) with an `Authorization`
header from the named variable; `endpoint::probe::ProbeClient::{send,post_stream}`
is the std reference for it. Verify per endpoint with `endpoints ... bind
--protocol responses`, which refuses unless `responses` was observed supported.

## Project model policy

`[model]` in `semaprax.harness.toml`: `local_only`, `strict_one_attempt`, `logical`. The workflow loads the logical
binding from the machine-local catalog and applies `check_policy` with the endpoint's real ownership; endpoint
adoption itself is never configured by a project.
