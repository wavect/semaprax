# Cloudflare-hosted Clef and Clef-Flash decision adapter v1 (MR-05)

Status: experimental, opt-in. **Contract-tested, live-unverified**: no Cloudflare
credentials were available, so no live inference has been run and the descriptor
records `support.tested: []`.
Audience: toolchain contributors, harness adapter authors and operators.

Owner: `packages/semaprax-harness-adapters/systemone/` (`cloudflare_backend.py`,
`cloudflare-clef-hosted/`). Contract: `decision.evaluate` v1 (`model-route/v1`)
and v2 (`model-route/v2`); see [HARNESS-DECISION-V1](HARNESS-DECISION-V1.md) and
[HARNESS-LAYA-JEV-V1](HARNESS-LAYA-JEV-V1.md). Provider id
`com.cloudflare/clef-decision`.

## Transport

One `Backend` subclass, two explicit profiles. Cloudflare Workers AI is not the
Jev API, so nothing is shared except the codec for the SystemOne
`state`/`questions`/`answers` fields.

| Profile | `SEMAPRAX_HARNESS_MODEL` | Path | Body `model` |
| --- | --- | --- | --- |
| `cf-clef` | `@cf/cloudflare/clef` | `/client/v4/accounts/{account_id}/ai/run/@cf/cloudflare/clef` | `clef` |
| `cf-clef-flash` | `@cf/cloudflare/clef-flash` | `/client/v4/accounts/{account_id}/ai/run/@cf/cloudflare/clef-flash` | `clef-flash` |

There is no default model: an unset or foreign model is refused (`SPX-HPK014`).

| Setting | Source |
| --- | --- |
| `SEMAPRAX_HARNESS_CLOUDFLARE_ACCOUNT_ID` | host env; must be 32 hex characters, else `SPX-HPK002` |
| `SEMAPRAX_HARNESS_SECRET_CLOUDFLARE` | host secret; sent only as `Authorization: Bearer`, scrubbed from output |
| `SEMAPRAX_HARNESS_REMOTE_APPROVED=1` | independent consent for router data egress (`SPX-HPK001`) |

The host is fixed to `https://api.cloudflare.com:443`. `SEMAPRAX_HARNESS_ENDPOINT`
may only restate it. Redirects are never followed, the response is bounded by
the invocation budget (at most 64 KiB), the deadline is the host's, and there is
**no retry** and **no `/v1/models` discovery**.

## Response handling

The body must be exactly the Workers AI envelope `{success, errors, messages,
result}` with `success: true`, empty `errors` and an object `result`.
`result` is then validated by the shared SystemOne codec: the answer must bind
to the request question id (foreign or extra ids are refused), cover exactly the
options, sum to about 1 and name the argmax. Native abstention is authoritative.
The answering `model` must be present and equal the requested variant
(`clef` or `clef-flash`, optionally `@cf/cloudflare/`-prefixed), else `SPX-HPK008`.

| Failure | Status / code |
| --- | --- |
| HTTP 401/403 | refused, `SPX-HPK012` (authentication) |
| HTTP 429 | failed, `SPX-HPK012` ("rate limited") |
| other HTTP, 3xx, connection/timeout | failed, `SPX-HPK012`/`011` |
| `success: false` | failed, `SPX-HPK012`, numeric error codes only |
| bad envelope, wrong variant, foreign question id | refused, `SPX-HPK008` |
| malformed distribution | refused, `SPX-HPK009`/`010` |

Error and echoed bodies are never copied into diagnostics.

## Request bounds and modality

The service truncates long state to a token limit, so the adapter refuses state
above 4096 bytes (`SPX-HPK005`) and a serialized body above `max_wire_bytes`
before any dispatch. Only `text` is admitted; an `image` modality is refused
(`SPX-HPK004`) and a profile listing `image` is refused (`SPX-HPK006`), although
the upstream accepts images. Profiles must be `mutable_service`, carry no
checkpoint and score as `option_distribution`.

## Identity, usage, billing

`call.identity_kind` is `mutable_service` with `checkpoint: null`; the two
profiles have distinct `profile_id` for calibration and qualification keys.
`usage.input_tokens`/`output_tokens` pass through when the service reports them
(`basis: provider_reported`), `billing` is `api`. No prices are embedded; use the
host price book.

## Fixtures and provenance

`systemone/tests/fixtures/` holds documentation-derived fixtures, labelled in
each file. The request bodies are the documented curl examples. The Clef pages
show no response example, so response and error envelopes are built from the
documented envelope members and output schema (`model`, `answers`, `usage`); the
exact `usage` member names and the choice answer layout are **assumptions taken
from the SystemOne/Jev shape** and must be confirmed by a live call.

## Commands

```sh
python3 -m unittest discover -s packages/semaprax-harness-adapters/systemone/tests

# Opt-in live smoke (one billable request; refuses without credentials):
export SEMAPRAX_HARNESS_CLOUDFLARE_ACCOUNT_ID=... SEMAPRAX_HARNESS_SECRET_CLOUDFLARE=...
export SEMAPRAX_HARNESS_REMOTE_APPROVED=1
python3 packages/semaprax-harness-adapters/systemone/cloudflare-clef-hosted/live_smoke.py clef
python3 packages/semaprax-harness-adapters/systemone/cloudflare-clef-hosted/live_smoke.py clef-flash
```

Upstream: [Clef](https://developers.cloudflare.com/workers-ai/models/clef/),
[Clef-Flash](https://developers.cloudflare.com/workers-ai/models/clef-flash/),
[REST API](https://developers.cloudflare.com/workers-ai/get-started/rest-api/).
