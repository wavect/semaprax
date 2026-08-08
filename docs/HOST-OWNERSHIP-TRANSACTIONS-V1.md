# Host ownership transactions v1

Status: a private, target-neutral Rust reference model is implemented in
`src/host_ownership.rs`. It fixes the semantic transaction that future native,
Wasm, Swift, Kotlin/JNI, and JavaScript adapters must preserve. It is not a
public FFI, does not execute a finalizer, and does not weaken `SPX-B104`. The
companion [native adapter descriptor](NATIVE-ADAPTER-DESCRIPTOR-V1.md) records
physical compatibility evidence only; it neither serializes this ledger's
runtime authority nor makes the transaction callable. The separate
[native capability-token layer](NATIVE-CAPABILITY-TOKENS-V1.md) fixes future
bearer-token authentication and private OS-entropy/thread-binding mechanics,
but its authority remains disconnected from this ledger; only a synchronized
registry can make authenticated bytes linear.

The private native staging lane also derives ownership contracts in
`src/codegen/native_host_contract.rs`. Derivation accepts a validated program
and exact function ID, rebuilds and compares the resource ABI, and consumes the
exact cleanup/value evidence already admitted by compiler preflight without
classifying or planning again. Its deterministic authority-free template fixes
the complete ordered scalar/resource signature, dense owner ordinals, exact
scalar/owned result mapping, lifecycle identities, module ABI fingerprint, and
function-template fingerprint. Private admission capabilities reject detached
same-ID HIR and mismatched cleanup/value proof objects. Tests cover interleaved
scalar/resource parameters, returning the second of two same-type owners,
cross-ABI template binding rejection, and real-thread observation at binding
and synchronous registry execution. Compiler resource
preflight derives and discards this Stage A template; no runtime binding or
public resource artifact is created there.

## Why this boundary exists

An `own` argument cannot safely cross an ecosystem boundary through a raw
pointer or integer. The adapter must distinguish two outcomes that an ordinary
language status cannot encode:

| Boundary result | Input ownership | Output ownership |
|---|---|---|
| Rejected | Caller retains every input | No output exists |
| Executed success | Callee consumed every committed input | Scalar or exactly one fresh-generation owner is published |
| Executed failure | Callee consumed every committed input | No output exists |

This distinction prevents a host from guessing whether it should retry, drop,
or forget an input after validation or language execution fails.

## Implemented reference invariants

1. Each registry receives a nonzero identity unique within its linked runtime
   instance and is not cloneable. A future cross-library/process ABI must bind
   tokens to a retained module/adapter capability as well.
2. An owner token contains registry identity, slot, and generation. Physical
   payload equality never establishes ownership identity; payload zero and
   `u64::MAX` are both valid.
3. A compiler/adapter-owned `HostCallContract` fixes module, adapter, function,
   thread, ordered resource type/lifecycle requirements, and result mapping.
   Host-supplied tokens cannot redefine those expectations.
4. Preflight checks thread, arity, token provenance, liveness, uniqueness,
   generation, and owned-result publication before changing any owner state.
5. After preflight, all `own` inputs enter one invocation together. Every
   rejection leaves owners, invocation counters, and active state unchanged.
6. A committed call is executed inside a registry-owned scalar or owned scope;
   its completion guard never escapes to adapter code. Safe Rust cannot clone
   it, complete another registry's call, start a nested call, or choose a
   different result shape after commit. Unwinding execution terminally consumes
   the ledger inputs through an allocation-free, non-panicking recovery path,
   clears the active call, and records an adapter failure for later
   materialization.
7. Scalar success and executed failure make every input dead. Owned success
   makes every non-result input dead and republishes exactly one result with a
   new generation, leaving copied input tokens stale.
8. Boundary rejection exists only before a committed capability is created.
   Completion produces only `ExecutedSuccess` or `ExecutedFailure`.

## Executable hostile evidence

The unit corpus proves:

- duplicate-owner rejection with no partial consumption;
- independently allocated registries cannot accept one another's tokens even
  when slot and generation match;
- module, adapter, resource type, lifecycle, thread, and arity mismatches are
  checked against trusted metadata;
- every rejection preserves the complete owner ledger and invocation counter;
- scalar success and contract failure consume every committed input;
- failed owned postconditions publish no owner;
- owned identity rotates generation and preserves zero/max payloads;
- maximum generation is safe for a discarded input and rejects before an
  attempted republication;
- a panicking executor cannot strand the registry or republish an input;
- copied, stale, malformed, and cross-registry tokens fail closed;
- invalid/NUL identities, zero thread identities, and invalid owned-result
  mappings are rejected.

## Deliberate nonclaims and remaining gates

The reference ledger does not yet prove a public adapter. Before any native
resource export can ship, all of the following remain mandatory:

- keep the committed physical-payload slice inside an audited generated
  executor; a copied integer payload is not itself ownership authority, but the
  reference model cannot prevent trusted backend code from retaining it;

- retain the private descriptor discovery header/provider as production
  compatibility evidence, while defining the still-missing callable owner
  token, rejection/completion, argument, result/out-slot, context, and
  module-lifetime layouts;
- turn the private, binding-instance-distinct process-local adapter authority and deterministic
  module/template fingerprints into a versioned physical library capability
  that retains the loaded module and safely rejects hot-reload, unload, and
  cross-library exchange;
- preserve the template's complete ordered scalar/resource metadata through
  generated headers and physical adapters; the ownership ledger intentionally
  consumes only ordered resource requirements and the result owner ordinal;
- carry the compiler-derived template from private resource preflight into the
  generated adapter; deriving and discarding Stage A remains groundwork, not
  public gate evidence;
- runtime-owned context, status arena, trace storage, provisional result, and
  deep materialization—no caller-provided mutable storage;
- a binding-instance/module-lifetime capability retained by every live owner;
- map the private Rust `ThreadId` observation to each physical target runtime
  and define an explicit `Send`/sharing policy;
- exactly-once physical finalizer execution, guard clearing before callbacks,
  and imported-finalizer capability/failure containment;
- concurrency, ABA-safe slot reuse, cancellation, reentrancy, unload/reload,
  allocation-failure, cross-thread race, panic-abort, and process-failure
  evidence;
- exact adapter-to-reference trace comparison across Windows, macOS, Linux,
  sanitizers, and eventually Wasm plus platform hosts.

Until those gates pass, raw payload adoption remains outside the safe contract,
the model remains private, and all public resource-bearing native builds return
the exact generic `SPX-B104` diagnostic.
