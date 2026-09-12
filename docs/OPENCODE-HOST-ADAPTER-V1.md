# OpenCode Host Adapter v1

## Status and boundary

This is a **LOCAL private-host** contract for issue #112. It binds one
explicitly configured OpenCode command to `live_invocation::ModelHandler` in
`semaprax-toolchain`. The standalone `semaprax` compiler and SDK remain
offline. This contract does not claim hosted evidence, provider availability,
production support, OS isolation, or source/HIR integration; #177 owns the
last of those.

The only admitted profile is `opencode/muse-spark-1.3-contributor-free`. There
is no fallback model, provider, endpoint, retry, or paid route. The host owns
credentials out of band; source, the request, stdout, receipts, and journals
never carry credentials.

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

It writes an `opencode.json` agent policy with `"*":"deny"` before the call.
`--dir` and that policy limit OpenCode's own tools only; they are not an
operating-system sandbox. The process output and session export each have a
1 MiB ceiling. The runner polls both an explicit host cancellation handle and
its deadline, kills and joins/reaps its child and reader on cancellation,
overflow, deadline, and process errors, and captures no stderr. On Unix it starts a dedicated process group and uses same-thread nonblocking
stdout polling; it kills that group after direct-child exit, so a descendant
retaining stdout cannot make the call unbounded. This v1 adapter refuses before
spawning on non-Unix platforms because it does not yet implement an equivalent
bounded nonblocking pipe loop there.

## Settling and replay evidence

The runner requires exactly ordered `step_start`, `text`, and `step_finish`
NDJSON events with one session/message pair and a `stop` finish. It then runs
`opencode export SESSION` and requires the fixed provider/model self-report,
the matching user and assistant `sessionID`, matching user prompt, and matching
assistant text before returning the raw text bytes. The receipt records only the self-reported session and optional
token total; it proves neither identity, billing, provider authorization, nor
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
compiled-schema decoder gate. A coordinated, separately recorded free-model
call is required before any hosted claim.
