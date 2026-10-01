# Reference service JSON event v2

Status: implemented opt-in route; focused regressions authored, pending execution.
This is local host behavior, not hosted or production-provider evidence.

## Selection and envelope

`semaprax-json-events-v2` is an additive telemetry adapter label in both the
closed service-config and independently decoded service-host-adapter-request
v1 wires. The scaffold JSON Schema admits the same label. Existing
`semaprax-json-events` still emits its byte-identical v1 envelope on
`/v1/events`; `otlp-http-json` retains its distinct `/v1/logs` encoding and
response interpretation.

The v2 route POSTs canonical JSON to the configured HTTPS origin's fixed
`/v2/events` path with `Content-Type: application/json`. Its closed object has
exactly these members:

| Member | Value |
| --- | --- |
| `schema` | `semaprax.json-event.v2` |
| `event` | `job.completed` |
| `job_id` | Positive signed 64-bit job ID |
| `owner` | Positive signed 64-bit owner ID |
| `desc` | Existing public job-description text |
| `signed_at` | Nonnegative signed 64-bit Unix seconds |
| `signature` | 64 lowercase hexadecimal HMAC-SHA-256 characters |

The signature uses the held webhook signing key and the exact canonical UTF-8
object **without** `signature`, including `schema` and `signed_at`. There is
no trailing newline. `x-webhook-signature` repeats the signature;
`x-semaprax-event-schema` is `semaprax.json-event.v2`. A receiver verifies the
HMAC and timestamp under its own clock and authority. Sender admission alone
is not evidence of receiver verification or remote exactly-once delivery.

## Actual source-policy facts

After existing request/session/ownership/completion/metric decisions, the host
samples its injected Unix-seconds clock for v2. An unavailable clock or a value
outside nonnegative `i64` refuses with `500 decision_failed`. The legacy
JSON-event and OTLP completion paths acquire no new clock dependency.

The new seven-argument checked source adapter
`completed_job_webhook_is_admitted` calls the existing nine-argument
`webhook_delivery_policy_is_admitted` with the profile's reserved attempt `1`
and public-payload classification `carries_secret=false`. The existing internal
policy and `std.webhook`/`std.jobs` predicates retain their semantics. The
remaining arguments come from one retained, prepared v2 envelope:

- the actual signature and full transmitted body byte count;
- the signing timestamp in that authenticated body and the sampled current time;
- whether an authenticated prior intent exists;
- the authenticated prior body commitment, or empty bytes for observed absence;
- the candidate body commitment computed from the actual retained body.

The descriptor is the domain-separated body commitment specified by the
[authenticated HTTP intent format](OUTBOUND-HOST-ADAPTER-V1.md#authenticated-service-http-intent-facts).
It is not a full endpoint/header/policy commitment. The independent HTTP
capability and durable session checks retain those bindings. The fixed payload
contains public job data and no held credential fields; the host does not scan
arbitrary description text for secrets.

Source denial returns `403 webhook_not_admitted`. An evaluator failure returns
`500 decision_failed`. Older source projects without the new adapter still
bind and serve v1/OTLP; selecting v2 without it returns `500 decision_failed`.
Both policy failures precede any outbound marker/checkpoint, candidate
state snapshot, or physical delivery. The existing checked export policy then
receives the exact retained body's size before durable delivery. Serialization
and pure preparation occur before these gates; no policy-approved envelope is
regenerated with a new timestamp afterward.

## Prior intents and cross-version behavior

The held outbound store uses the existing deployment / `job-<id>` /
`completion` identity and unchanged marker filename. Read-only lookup has three
relevant outcomes:

- Explicit absence supplies first-attempt facts and uses the sampled time.
- An authenticated intent supplies its retained timestamp and body commitment.
  Reconstructing the candidate at that timestamp preserves exact body identity
  on a restart. Attempt `1` describes the original reservation; no second
  physical attempt is authorized.
- A legacy, corrupt, oversized, wrong-key, or otherwise unauthenticated marker
  supplies no invented facts and returns `503 delivery_unavailable`.

A prior timestamp outside the existing 300-second source window or a changed
body commitment is a source denial. At the exact window boundary an unchanged
candidate can pass source admission, but the existing marker still prevents
redispatch; the host records `Completed` with `Uncertain` settlement. Denied
retries preserve the pending state and stored intent. A physical delivery that
preceded a lost state commit never becomes another physical delivery merely
because the state still says Pending.

The final atomic create-new remains the dispatch arbiter after a read observed
absence. Concurrent preparations may both pass source admission; only one can
commit the shared marker and enter the adapter. V1 and v2 therefore block each
other across configuration switches. No marker is upgraded, replaced, or
removed. The held-directory, namespace-sync, rollback, and operator-retained
state-digest limits of the existing store continue to apply.

## Owning regression selectors

Native-host library selectors:

- `reference_service::delivery::webhook_v2::tests`
- `reference_service::mapping::tests::webhook_policy`
- `reference_service::delivery::tests` (v1 and OTLP preservation)
- `outbound_delivery_store::service_invocation::tests::authenticated_intent`

The mapping corpus covers actual authenticated completion, default and custom
source decisions, denial/evaluator nonmutation, legacy and tampered records,
clock failures, restart boundaries/conflicts, and reference/generated parity
against independently executed `std.webhook` and `std.jobs` predicates. The
encoder corpus binds captured request bytes, HMAC, headers, timestamp and prior
facts, and exercises competing preparations and cross-version no-redispatch.
The closed config and independent handoff decoder unit tests cover explicit v2
selection and unknown protocol refusal. Focused local execution passed the
delivery selector (4/4), mapping policy selector (6/6), and the two owning
config-decoder selectors (1/1 each). The authenticated-intent selector passed
earlier (8 new cases within 19/19 service-invocation cases). These are local
source-tree results, not provider, OCI, or installed-artifact acceptance.
