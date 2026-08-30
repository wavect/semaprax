# Image Candidate Test Protocol v3

Audience: embedding hosts, agent builders, and compiler contributors.

Status: authored implementation and regression cases, unrun by user request.
No current passing gate, native/Wasm execution, or complete programme claim.

V3 adds explicit interpreted-test authority to the retained candidate session.
The host selects `ImageHostCapability::TestEnabled` before requests arrive, or
calls `ImageSession::open_test_enabled(manifest, policy)` with a bounded
`CandidateTestPolicy`. Read-only v1 and candidate-only v2 retain their existing
method sets, envelopes and authority. An agent cannot select the profile or
alter the policy through a request.

```text
semaprax serve-test-candidates <manifest>
```

This explicit CLI startup selects 100,000 interpreter steps, 65,536 execution
envelope bytes and 262,144 report bytes. The ordinary `serve-candidates` command
still cannot execute tests. There is no trace, native compiler, subprocess,
network, source-write, artifact or managed-publication capability in v3.

## Discovery and requests

The envelope schemas are `semaprax.image-agent-protocol.v3` and
`semaprax.image-agent-result.v3`. Discovery, instructions and generated client
helpers use v3 identities. Capabilities include `semantic_read`,
`candidate_prepare` and `candidate_test`, plus exact fixed policy limits.
`target_execution` remains false: interpreted tests do not execute a native or
Wasm target. Complete bundled response schemas and executed client compatibility
remain separate programme requirements.

V3 retains the candidate lifecycle and adds two methods. Both require exact
`image_revision` and `candidate_revision` digest fields, and no others:

| Method | Result and authority |
| --- | --- |
| `candidate/test-plan` | Static test relevance bound to the candidate, including conservative fallback reasons. It executes nothing and is not runtime coverage. |
| `candidate/test` | Independently replays the complete candidate and executes its complete declared test closure under host policy. A returned test value passes only when it is zero; failures and fuel exhaustion cannot become passing reports. |

`protocol/schemas` labels execution as `candidate_test`. `validation/catalog`
uses `semaprax.image-validation-catalog.v2` in this profile to advertise the
explicit execution route while retaining native/Wasm conformance and full
quality checks as external gates. Merely discovering the route runs no test.

## Replay and limits

Execution uses the [candidate test API](PROJECT-CANDIDATE-TESTS-V1.md).
Even when the relevance plan selects no affected root, an explicit test request
runs the complete declared closure. Reports bind candidate/source/diff identity,
test origin, options and execution outcome. Candidate preview/validation reports
retain their original `not_run` meaning; an execution report is a separate
artifact, never an approval or commit token.

The session authenticates its held disk inputs before preparing the response
and again before returning it. Observed drift fails closed; no successful test
report is returned. Pure in-memory execution may already have occurred before
a final drift or output-bound rejection. No exactly-once or zero-work-on-error
claim is made, and no external effects are performed by this route.

Existing 64 KiB request and 1 MiB response limits remain. Policy bounds are set
only by the host; over-limit requests and unknown fields reject before dispatch.
Registry lookup rejects stale or unknown candidate handles. Draft handles cannot
select execution, so unresolved holes never become executable candidates.
Test responses do not mutate or retain registry entries, and no test result can
publish source. Explicit managed publication remains a separate host API.

Focused evidence is authored in
[image_candidate_test_transport_v3.rs](../tests/image_candidate_test_transport_v3.rs):
old-profile rejection, host policy disclosure, no request overrides, candidate
binding, replay-bound success, fuel exhaustion, no source writes and held-input
drift. These cases have not been run.
