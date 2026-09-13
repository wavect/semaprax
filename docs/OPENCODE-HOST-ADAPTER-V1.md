# OpenCode Host Adapter v1

Status: **LOCAL private-host** contract; no hosted provider evidence.
Audience: local adapter maintainers and compiler contributors.

## Status and boundary

This is a **LOCAL private-host** contract for issue #112. It binds one
explicitly configured OpenCode command to `live_invocation::ModelHandler` in
`semaprax-toolchain`. The standalone `semaprax` compiler and SDK remain
offline. A local source-driver smoke reached Complete with OpenCode 1.18.27;
this contract does not claim hosted evidence, production support or OS isolation. The source adapter implements the existing
Agent lifecycle v2 feedback callback; broader live-kernel source/HIR integration
remains tracked separately in #177.

The only admitted profile is `opencode/muse-spark-1.3-contributor-free`. There
is no fallback model, provider, endpoint or paid route. The adapter adds no
automatic transport retry; OpenCode subprocess work remains deadline-bounded.
Host credentials are not added to model context or runtime journals. Provider
error bodies and headers are discarded from the closed diagnostic categories.

## Authority and route

A deployment supplies both an `OpenCodeHostConfig` (absolute executable,
empty non-symlink absolute workspace, positive deadline, and an
`OpenCodeGrammar` derived from the exact `CompiledInteractionSchema`) and the
existing per-call `ModelInvokeCapability`. `OpenCodeGrammar` carries the
canonical interaction schema and its existing provider JSON Schema projection;
the handler rejects a request whose grammar digest differs, then passes those
compiler-derived guidance bytes to the configured process. It does not decode
or admit the response. The existing `SourceInteractionProposalDecoder` still
decodes the raw response after this handler returns it. Neither source data nor a response can construct
those values. The production `ProcessOpenCodeRunner` uses no shell or inherited
stdin and invokes exactly:

```
opencode run --pure --agent semaprax-live \
  --model opencode/muse-spark-1.3-contributor-free --format json --dir WORKSPACE PROMPT
```

It writes an `opencode.json` agent policy with `"*":"deny"` and
`"snapshot":false` before the call. Filesystem snapshot tracking is disabled.
The child receives a cleared environment with private home, configuration,
cache, temporary and database paths. Project/Claude instructions, external
skills, default plugins and automatic updates are disabled. The explicit
policy is the sole admitted local configuration. Existing host credentials
remain in the original XDG data directory; their bytes never enter arguments,
context or receipts. Remote-configuration auth entries and managed settings
cause a generic pre-spawn refusal. Run and export share this private context.
These controls are required because `--pure` alone only disables plugins.
`--dir` and that policy limit OpenCode's own tools only; they are not an
operating-system sandbox. The process output and session export each have a
1 MiB ceiling. The runner polls both an explicit host cancellation handle and
its deadline, kills and reaps its child on cancellation,
overflow, deadline, and process errors, and captures no stderr. On Unix it starts a dedicated process group and uses same-thread nonblocking
stdout polling; it kills that group after direct-child exit, so a descendant
retaining stdout cannot make the call unbounded. This v1 adapter refuses before
spawning on non-Unix platforms because it does not yet implement an equivalent
bounded nonblocking pipe loop there.

## Settling and replay evidence

The runner requires exactly ordered `step_start`, `text`, and `step_finish`
NDJSON events with one session/message pair and a `stop` finish. It then runs
`opencode export SESSION --pure` and requires the fixed provider/model self-report,
the matching user and assistant `sessionID`, matching user prompt, and matching
assistant text before returning the raw text bytes. The receipt records the bound session/message, fixed model, supplied token
counters and reported cost; it proves neither identity, billing, provider authorization, nor
exclusive execution.

The handler returns `Settled(raw_bytes)` only after that transport validation.
It deliberately performs no proposal/schema decoding. The existing
`SourceInteractionProposalDecoder` runs in the live driver after the handler,
so compiler-derived grammar admission remains authoritative.

Before process start, a cancellation returns `Cancelled`. After start, any
transport/export uncertainty is `ProviderError` unless the injected runner has
observed cancellation; timeout, capacity, malformed output, and policy refusal
remain their respective closed `ModelFailure` cases. A cancelled call does not
prove that the provider stopped work or billing.

## Regression obligations

Offline tests cover ordered-event/export binding including hostile session
substitution, receipt usage preservation, cancellation and overflow reaping of
local stubs, and the rule that malformed post-start output is not cancellation. A local
stub executable is the required integration seam for process command,
deadline, bounded-output, and export coverage; it must never call a provider.
The existing `agent_interaction_schema::live_bridge` kernel test remains the
compiled-schema decoder gate. A separately recorded free-model call is required for live-provider evidence;
it does not establish hosted CI or production support.

## Source-feedback smoke embedding

`crates/semaprax-toolchain/examples/opencode_live_smoke.rs` compiles a frozen,
pure one-turn source lifecycle and calls its existing #111 `run_live` route.
With no arguments it uses one canonical offline proposal. A real invocation is
opt-in and requires all of:

```sh
cargo run -p semaprax-toolchain --example opencode_live_smoke -- \
  --live --opencode /absolute/path/to/opencode --scratch /absolute/empty/dir \
  --evidence /absolute/new/evidence-dir
```

The `--live` branch keeps the source lifecycle's own proposal decode and
bounded retry loop, but its host runner permits only one actual provider run.
It archives the prompt, raw events and session export in the new explicit
evidence directory. It swaps only the `ProposalSource` callback for the
explicit OpenCode bridge, uses the free configured profile, and has no paid
fallback. This example is local host evidence only; it does not establish a
hosted support claim.


Implementation references checked 2026-09-13: [OpenCode CLI](https://opencode.ai/docs/cli/)
(`run`, model/agent/JSON flags and session export) and [permissions](https://opencode.ai/docs/permissions/)
(the deny-all agent policy). The observed wire profile is OpenCode 1.18.27;
future CLI/export changes must be admitted deliberately rather than silently
accepted. The receipt validator compares all streamed parts to their exported
counterparts, admits the observed empty reasoning marker, and rejects model,
finish, prompt, session, part-order, and typed-usage drift. Missing token usage
remains unknown. It resets the prior receipt before every attempted request.


## Local execution evidence (2026-09-13)

The explicit source smoke returned `live status=Complete` using the frozen
source fixture, real `run_live`, compiler-derived proposal grammar and unchanged
canonical decoder. Its in-memory read callback returned a fixed fixture value;
no external effect mutation or deployment occurred. OpenCode 1.18.27 reported
session `ses_f67f10b32ffeSsQKpC96WHphgi`, assistant
`msg_0980ef5b2001CpH4AZG9d5JL7N`, model
`opencode/muse-spark-1.3-contributor-free`, and a `stop` finish.
Reported counters: total 3188, input 1780, output 110, reasoning 1298, cache read
and write 0; reported cost 0. These are provider self-reports, not billing proof.
The smoke runner allowed one actual OpenCode run and archived raw prompt,
events and exported session under local `.agent-logs/0913-opencode-source-live-v6/`.

The exercised binary SHA-256 was
`c1ab0401fd926d9250fc8d446bdf47bd6a8f71a98807d9c6538d9f8a832a0a24`.
Raw event SHA-256:
`039b8955d5aaa6bebe5bc20173631d70ac43d693eaf7fc7e4068a5fc58dc32e1`.
Export SHA-256:
`e14ef1b0cef6f7ef95370f9aad9cc3d123af3e1f88e299b7173c59e3a9640aac`.
Earlier unsuccessful attempts were rejected for multiple steps, the CLI's
positional-prompt quoting, absent/literal-escaped terminal LF, and a snapshot
patch record. The final call used scratch outside Git with snapshots disabled. The final prompt requests the LF explicitly;
the host neither appends it nor repairs the proposal. Those failures remain
separate local observations, not successful executions.

OpenCode's tagged [CLI emitter](https://github.com/anomalyco/opencode/blob/v1.18.27/packages/opencode/src/cli/cmd/run.ts)
also owns the positional-prompt quoting and error envelopes used here.
`last_provider_failure` distinguishes rate limiting, authentication, refusal,
server failure and incomplete output without retaining provider error text.
The source driver still owns proposal decoding, bounded malformed retries and
all checked authorization/reduction stages. Local focused gate:
`cargo test --locked -p semaprax-toolchain --lib opencode_host -- --test-threads=1`
passed all 29 tests. The canonical-context regression (1), existing source-driver
tests (5), bundle pin (1), module-size (1), and source-contract coverage (1) also
passed. The offline smoke returned Complete. These are focused local checks;
the full/hosted quality profile was not rerun.

The environment profile was checked against OpenCode v1.18.27's tagged
[instruction loader](https://github.com/anomalyco/opencode/blob/v1.18.27/packages/opencode/src/session/instruction.ts)
and [configuration loader](https://github.com/anomalyco/opencode/blob/v1.18.27/packages/opencode/src/config/config.ts).
Offline environment tests use synthetic auth/settings paths; normal tests
neither read the developer's credentials nor contact a provider.

OpenCode may install runtime dependencies in its private configuration/cache
directories even with `--pure`; those writes are not compiler build-time work.
The smoke archives its receipts before removing disposable scratch storage.

## Source attempt accounting

The source adapter requires an explicit `OpenCodeSourceAccounting` wrapper
around the existing `InvocationBudgetHook`. The host supplies a positive fixed
reservation in the deployment's units and a bounded attempt capacity. Each
attempt reserves before dispatch and retains that reservation through malformed
proposals, uncertain provider failure and cancellation in flight. Validated
reported token counters are observations; they never refund or determine the
reservation. A cancellation observed before reservation makes no provider call.

The wrapper retains bounded host-only receipts with turn/attempt identity,
reserved units, measured request/response bytes and optional reported counters.
The source grammar decoder still owns admission, so a transport-settled but
noncanonical document remains charged when the driver retries. Receipt capacity
is checked before a new reservation. The shared absolute deadline is checked
before dispatch and again before settled proposal bytes reach the decoder.

The existing source driver checks the same policy through
`ProposalSource::check_deadline` at stage, proposal/decode, effect and result
publication boundaries. The OpenCode source delegates that read-only check to
its accounting wrapper; fixture sources retain an accepting default. Checks do
not reserve again. An expired successful read blocks reduction and publication;
an already selected effect failure or reducer `Fail` remains selected.

These receipts and the source wrapper are in-memory. A deadline diagnostic from
`run_live` does not return its partial run or durably retain an observed effect.
Durable source recovery, migration, restart-stable deadline binding and complete
failure evidence remain open #113 requirements. The smoke clock is process-local.
[Source Live Journal v1](SOURCE-LIVE-JOURNAL-V1.md) records the proposed
source durability contract and required recovery tests; it is unimplemented.
The earlier local provider evidence above is bound to its recorded executable;
these extensions have separate offline regression evidence.

Focused accounting validation (2026-09-13): 39 OpenCode host tests and 103
live-invocation library tests passed, including exact-deadline rejection,
just-before-deadline admission, nonrefundable malformed retries, prior-failure
selection and causal journal replay. Bundle pin and both structural checks
also passed. No additional provider call was used for this extension.

Source-boundary extension validation (2026-09-13): 8 source-driver tests and
41 OpenCode host tests passed, including expiry inside stage/effect/transition
callbacks, actual adapter reads finishing late, cancellation before Complete
publication and sticky reducer Fail. These are local offline regressions.
