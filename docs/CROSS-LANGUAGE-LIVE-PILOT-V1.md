# Cross-language live pilot v1

Status: additive local execution route for issue #332. No real provider trial or
independent second-host admission is established by the fixture gates.

## Scope and frozen inventory

This belongs to `benchmarks/cross-language-v1`, the laboratory inherited from
#298/#211. It is separate from the source/graph 36-tuple Agent-task protocol.
The existing v3 fixed-source conformance CLI remains unchanged.

`agent/pilot_protocol.py` freezes two distinct exact requested Claude versions (dated snapshots or the
explicitly admitted `claude-sonnet-5-5` version ID),
two distinct expected provider-reported model identities, one repetition, and
the existing **validation** task `structured-input-error-handling-v1`. There is
no adaptation, prompt tuning, or held-out task selection. The only writable
candidate is `validate.ts`; both fixed public and hidden `index.ts` import it.
All 182 task/adapter inventory rows remain in the plan/report. Each host has
28 pilot dispositions (two models × 14 adapters): two planned TypeScript cells
and 26 unavailable cells with the existing official-support reasons. Other
tasks remain in the complete inventory, outside this explicitly selected pilot.
Unavailable cells and failed/interrupted attempts must never disappear.

A candidate path must be a canonical plain relative file already in the public
snapshot, disjoint from every hidden-overlay path. This is checked before model
dispatch and again before scoring. The original development sequence-digest
port is intentionally not selected: its hidden `index.ts` would replace a
same-path candidate. The new admission does not reinterpret that older route.

The plan binds the independently pinned v3 baseline, approved correction,
current runner revision, implementation/dependency file digests, exact public
prompt/system prompt, per-host CLI executable digests and authentication-home
grants, shared CLI version/limits/models, approval reference, native host
identities, both execution profiles and the fixed controller ledger. Admission regenerates the plan from authenticated inputs and
requires exact canonical bytes and an operator-supplied plan digest. Neither a
plan digest nor an approval string proves an independent review happened.

## Model transport and limits

`agent/claude_subscription.py` uses each frozen host entry's canonical executable,
home and login, plus a private per-trial scratch directory. CLI arguments cannot
replace those grants. The actual OS/architecture and kernel release must match;
Linux additionally requires the exact generation guest boot UUID. The home/login grant permits the installed Claude
subscription authentication route; it does not copy credentials into evidence.
The executable is acquired no-follow, checked against the frozen digest, then
staged privately. A bounded `--version` metadata check must report the same
reviewed CLI 2.1.286 on both hosts before any model request. Mach-O and Linux ELF
artifacts have separate digest/path pins. Known managed-settings routes refuse. The closed environment
contains only the listed login/home, system PATH, scratch TMPDIR and the three
safe-mode/update/traffic switches used by the existing native subscription host.
Native tools, persistence, permission prompts and MCP discovery are disabled.
Claude itself and its authentication/provider transport remain trusted host
components; this is not an OS sandbox proof for the provider client.

One transport object starts at most one CLI process. Requests use private regular
stdin. Concurrent nonblocking stdout/stderr draining has a shared 1 MiB bound
and a maximum 90-second wall deadline; descendants are killed at completion.
The request and decoded result each have a maximum 64 KiB bound. The provider's
`--max-budget-usd` receives the frozen API-equivalent estimate cap (at most $10).
The sum of reported input/output/cache-create/cache-read tokens is checked
against the frozen limit (at most 65,536). This token check is **post-response
admission**, not a claim that the provider cannot consume more tokens. Failed
admission retains the bounded raw receipt, including actual reported usage.
No automatic caller retry is permitted. Provider-internal retries are not
observable and are recorded as such.

Temperature, top-p and seed are null with `provider_default_not_exposed`; the
CLI does not supply those controls. They must not be populated with invented
values. Cost is the provider's estimated API-equivalent USD, with subscription
invoice cost null. No claim of zero spend or known subscription marginal cost
is made. Exact requested snapshots and observed model keys are separate facts:
canonicalization is retained, not falsely presented as an observed dated ID.
Two rows reporting the same model identity cannot form this two-model plan.

The accepted envelope is one successful completed result, one turn, no denied
permissions, no queued turns/subagent spawning, and exactly the expected
first-party reported model. The `result` contains one JSON object with exactly
`validate.ts` string content. Bare JSON or the exact ` ```json\n...\n``` `
transport fence is accepted; other labels, prose, duplicate keys, extra paths,
nonfinite numbers and oversized results refuse. Candidate bytes are never
repaired or line-normalized.

## Candidate execution and host boundary

`agent/pilot_score.py::CandidateSession` is an additive session around the
existing independently authenticated Darwin TypeScript runtime. Before any
model request it admits official archive provenance, exact macOS 26.5.1/25F80
arm64 identity, protected runtime dependencies and real deny-default authority
probes. It uses the pinned existing scorer and exact commands. Separate public
and hidden phase trees receive identical candidate bytes and fixed scaffold;
only the hidden phase receives hidden assertions. Both phase outcomes, command
streams, generated artifacts, candidate bytes/hash and authority receipts are
retained, including failed outcomes.

The compiler-emitted candidate module is wrapped by a trusted numeric-function
bridge before loading it into the assertion harness. It runs in a fresh null-
prototype Node VM context with no injected host objects, no Node `process` or
`require`, disabled string/Wasm code generation, and 100 ms initialization/call
timeouts. Only safe integer arguments/results cross into the fixed assertions.
The frozen validation harnesses are silent. Each must emit exactly its
host-appended completion marker and LF framing, in addition to exiting
successfully; no invented `ok` output is required. Early `process.exit(0)` therefore cannot pass by skipping
assertions. Original emitted candidate, bridge and completed harness bytes are
retained. This is assertion isolation; Node VM is not claimed as a security
sandbox. The existing OS confinement remains mandatory. This bounded numeric
execution profile is explicit in the frozen model prompt. Tests remain empirical correctness checks,
not a proof of arbitrary generated program semantics.

The shared protocol now distinguishes the Darwin profile from the independently
reviewed `apple-container-linux-arm64-typescript-pilot.v1` profile. The latter's
separate `pilot_linux_host.py` owner must authenticate its exact provision digest,
official runtime and real guest authority probes; a profile entry is not an
admission receipt. Linux candidate scoring uses macOS only as the control plane.
Actual provider generation must run the pinned native Linux Claude binary in
the separately identified/authenticated generation guest. The guest-native
`generate` command refuses a Mac process using the Linux host label. Each host
uses the same model IDs, CLI version, prompt and caps.

`score` accepts the exact canonical generation file plus its independently
retained SHA-256. It revalidates plan/model/host identity, CLI binary/version,
arguments, prompt, raw provider envelope, usage, candidate contents and digest.
It then consumes a create-new key for the exact plan/host/model cell in the
frozen private controller ledger before constructing the scorer. A duplicate
receipt, copied receipt or changed outer nonce for that cell cannot score again.
Failures/interruption consume the cell too. Raw receipts remain evidence, not
an attestation or a new runtime grant: each scoring session acquires its ordinary
host authority. Independent review must still establish custody and actual
separate guest execution; no receipt format is a cryptographic provider signature.

## Operator interface and confidential custody

Run from `benchmarks/cross-language-v1` using the Python module entry point.
The configuration has exactly these fields (model values below illustrate the
shape and are not a availability claim or spend approval):

```json
{
  "models": [
    {"id":"a","requested_model":"claude-haiku-4-5-20251001","reported_model":"claude-haiku-4-5"},
    {"id":"b","requested_model":"claude-sonnet-5-5","reported_model":"claude-sonnet-5-5"}
  ],
  "limits": {"deadline_seconds":90,"max_request_bytes":65536,"max_result_bytes":65536,"max_reported_tokens":16384,"max_estimated_usd":1.0},
  "cli_version":"2.1.286",
  "hosts": {
    "first-host": {"native_platform":"darwin-arm64","kernel_release":"ACTUAL_DARWIN_KERNEL","boot_id":null,"executable":"/absolute/resolved/claude","claude_sha256":"ACTUAL_MACHO_SHA256","home":"/absolute/authorized/home","login":"LOGIN"},
    "second-host": {"native_platform":"linux-arm64","kernel_release":"ACTUAL_GUEST_KERNEL","boot_id":"ACTUAL_GENERATION_GUEST_BOOT_UUID","executable":"/absolute/guest/claude","claude_sha256":"ACTUAL_ELF_SHA256","home":"/absolute/private/guest/home","login":"GUEST_LOGIN"}
  },
  "execution_profiles": {
    "darwin-arm64":{"profile":"darwin-arm64-official-typescript-pilot.v1","provision_sha256":null},
    "linux-arm64":{"profile":"apple-container-linux-arm64-typescript-pilot.v1","provision_sha256":"ACTUAL_REVIEWED_LINUX_PROVISION_SHA256"}
  },
  "controller_ledger":"/absolute/private/controller-ledger",
  "approval":"ACTUAL_AUTHORITY_AND_CUSTODY_REFERENCE"
}
```

Provision the private controller ledger (0700) before freezing. Authenticate the
generation guest through native `claude auth login --claudeai`; its private home
and native CLI are separate from the scoring guests. Do not mount subscription
credentials into candidate execution guests. The older single-binary/host-label
configuration refuses instead of silently acquiring new authority.

```sh
# Freeze only after BOTH host identities/artifact pins/profiles are ready.
python3 -m agent.pilot_run freeze --configuration /absolute/config.json --output /absolute/plan.json
# Run on the actual selected generation host, from the same frozen source.
python3 -m agent.pilot_run generate --plan /absolute/plan.json --plan-sha256 APPROVED_DIGEST \
  --host-id second-host --model-id a --directory /absolute/new-trial-a
# Transfer private trial evidence intact, then score from the admitted controller.
python3 -m agent.pilot_run score --plan /absolute/plan.json --plan-sha256 APPROVED_DIGEST \
  --generation /absolute/copied-trial-a/generation.json --generation-sha256 RETAINED_GENERATION_DIGEST \
  --provenance-directory /absolute/reviewed-provenance
python3 -m agent.pilot_run account --plan /absolute/plan.json --plan-sha256 APPROVED_DIGEST \
  --host-id second-host --trial-directory /absolute/copied-trial-a --output /absolute/accounting.json
```

Each new generation directory is private (0700); intent/generation/result/scoring
files use 0600 create-new publication with file and parent-directory fsync. An
existing generation directory is not retried. Successful generation is not yet
a successful scored trial. Missing terminal scoring remains interrupted in the
inventory. Bounded raw stdout/stderr, model metadata and source candidates are
confidential local evidence: no automatic publication or redaction claim is
made. The process-start marker reports native CLI dispatch, not unobservable
internal provider retries. Accounting rejects duplicate/unplanned cells and
mismatched plan/host/invocation receipts; it does not independently attest review
or second-host identity. Preserve guest provider receipts and separate Linux
scoring receipts for external review.

## Focused gate and remaining closure cells

```sh
python3 -m unittest discover -s benchmarks/cross-language-v1/agent/tests -p test_live_pilot.py -v
python3 -m unittest discover -s benchmarks/cross-language-v1 -p test_supported_scope.py -v
```

The new tests exercise all inventory cells, model/plan drift, candidate-overlay
collision, prompt isolation, malformed/provider usage frames, one-use transport,
closed environment, real bounded local process draining/timeouts, assertion-isolated
Node positive/early-exit/wrong-result/code-generation controls, candidate
identity across phases, unavailable-host zero dispatch and interrupted evidence.
They use local fixtures and do not contact Claude or execute the official runtime.
The existing v3 provenance tests remain the preservation gate for frozen inputs.

#332 remains open until the reviewed actual two-model trial inventory is
executed, the independent second host is admitted and executes/scores fresh
trials, and requests/results/cost/latency/correctness/environment/review evidence
is retained. The smallest selected pilot is four real solver/scorer trials
(two models × one TypeScript task × two hosts), with all unavailable dispositions
retained. Failed trials count as observed failures, never successful scores.

Recorded local fixture gates for this implementation batch: new pilot 8/8,
supported-scope preservation 29/29, and existing v3 PureProvenanceTests 11/11.
The bridge fixtures used installed local Node v24.3.0; this is not the official
Node 22.12.0 runtime admission or a real provider result.

### Sonnet 5.5 model pin follow-up

Anthropic's [model overview](https://platform.claude.com/docs/en/models/overview)
lists `claude-sonnet-5-5` as its full API model ID, and the
[model configuration](https://code.claude.com/docs/en/model-config) documentation
distinguishes full version IDs from moving family aliases. Only this exact
undated version ID is added to dated-snapshot admission; `sonnet`, guessed future
versions and suffix variants still refuse. CLI 2.1.286 initialization metadata
listed Sonnet 5.5 and Haiku 4.5 on the signed-in Team account using zero user
messages and zero model turns. That metadata is provider readiness evidence,
not a generated candidate or a real model-usage receipt.

### Native generation and scoring handoff follow-up

The focused pilot gate passed 11/11 in 2.231s after adding per-host native
identity/version admission, exact provider-byte revalidation and one-use
controller cell custody. Tampered candidates, usage, host identity, argv and
receipt digests refuse; changing the outer invocation nonce cannot reuse a
model/host cell. The actual frozen validation harness is silent: completion
requires only the trusted marker after its assertions. Local fixtures cover
that shape and early exit, incorrect results and generated-code negatives.
No provider inference or independent guest generation was executed by this
follow-up; Linux runtime admission has its own evidence and gate.
