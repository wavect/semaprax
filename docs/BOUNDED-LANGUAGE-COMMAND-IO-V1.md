# Bounded Language Command I/O v1

Status: locally evidenced implementation tranche. This document freezes the
reviewed contract; exact-head hosted promotion, registry publication, and
release completion remain pending.

## Objective

This tranche makes command input and stderr visible to checked SEMAPRAX code
without granting ambient process authority. The exact compiler-owned source
operations are:

| Operation | Stable identity | Effect | Result |
| --- | --- | --- | --- |
| `args_len()` | `core.host.args-len` | `process.args.read` | infallible `usize` |
| `arg_utf8(index)` | `core.host.arg-utf8` | `process.args.read` | fallible `borrow str` |
| `stdin_read()` | `core.host.stdin-read` | `process.stdin.read` | fallible `own Bytes` |
| `stderr_write(value)` | `core.host.stderr-write` | `process.stderr.write` | infallible `usize` |

The existing `stdout_write` operation and its success-only transcript remain
authoritative. No authored interface import, Native Rust import, callback,
WASI import, libc read, or ambient JavaScript process access implements these
operations.

The public command profile selects one explicit stable-ID function with exact
signature `() -> bool`. `true` and `false` are both successful semantic
results. They seal the staged stdout and stderr transcripts; normalized
operation failure, contract failure, target invariant failure, or cleanup
failure discards both.

## Invocation input

An adapter snapshots the complete input before semantic execution:

- arguments exclude `argv[0]` and number at most 16;
- every argument is strict UTF-8 and contains no NUL;
- stdin is an arbitrary byte sequence;
- the checked cumulative argument-plus-stdin size is at most 65,536 bytes;
- the snapshot is immutable until invocation settlement.

That capacity is one invocation budget, not one allowance per source or per
read. The snapshot admits exactly one CommandArguments source and one Stdin
source and rejects a duplicate of either source; HIR
admission rejects more than one reachable `stdin_read`, including loop and
call-cycle reachability. Consequently neither repeated argument lookup nor a
second representation of the same provider bytes can recharge the budget.

Failure to construct this snapshot is an adapter failure before the language
entry runs. Native Unix validates and copies raw argument bytes. Native
Windows converts `wmain` UTF-16 strictly. Node rejects lone surrogates and NUL
before UTF-8 encoding. Browser consumers inject ordinary immutable snapshots;
they receive no process authority.

All `arg_utf8` results borrow one invocation-owned argument arena. Repeated or
dynamic reads do not mint or recharge roots. The view can be forwarded and
projected through `str_as_bytes`, but cannot escape, be stored, enter an
aggregate, cross an import/callback/async boundary, or outlive settlement.

Each successful `stdin_read` creates a fresh ordinary owned `Bytes` value.
The admitted v1 closure executes at most one read on a path and does not reach
it from a loop or call cycle. Its allocation is governed by the existing
owned-byte allocation, transfer, sticky-failure, result-publication, and
exact-once-drop rules.

## Failure and settlement

Fallible operations use the closed normalized domain
`semaprax.command-input.v1`:

| Code | Meaning |
| --- | --- |
| 1 | argument index is out of range |
| 2 | argument bytes are not valid UTF-8 |
| 3 | stdin read failed |
| 4 | the command-input capacity contract was exceeded |

Adapters normally reject codes 2 and 4 before entry because they authenticate
the complete snapshot first. The raw provider ABI nevertheless remains closed
over all four codes so a forged or independently supplied provider cannot
widen the status vocabulary. OS errors and JavaScript exceptions are never
smuggled into this domain.

The HIR uses a distinct host-command call, not an ordinary infallible call and
not `NativeRustImportCall`. `arg_utf8` produces a success-only borrowed view;
`stdin_read` initializes its owned result slot only after status zero. Existing
CleanupPlan v2/v3 `CallCommit`, propagated-call, initialize, transfer,
finalize, sticky-failure, stage-result, and publish transitions are sufficient
only when the canonical builder and independent replay derive those exact
facts. This tranche does not reinterpret an old transition or add CleanupPlan
v4.

## Output

`stdout_write` and `stderr_write` append to distinct invocation-owned semantic
transcripts. At most one write per channel executes on a path, and their
combined staged bytes are at most 65,536. Writes are observable only after the
root result and all cleanup settle successfully. Consumers retrieve both
channels atomically as one semantic envelope and clear both after retrieval.

A process adapter may then flush the sealed channels. Physical writes are
fallible and are neither atomic across descriptors nor durable; a prefix may
already be visible when a later physical write fails. That adapter failure
cannot retroactively change semantic success.

## Targets and Project v6

Native generated functions receive an explicit invocation context. They never
read or write process descriptors directly. Wasm uses synchronous, closed
`env` imports for argument length, argument lookup, owned stdin creation, and
exact owned-arena membership validation; there is no WASI, console, process,
arbitrary callback, or async authority.
Provider out-slots remain poisoned on failure, owned-byte arena tokens settle
exactly once, and both transcript ranges are cleared on every failed path.
Because an owned stdin value is represented by an authenticated arena token,
the Wasm wrapper copies its bytes into private transcript staging while the
owner is live; it never treats the tagged token as a linear-memory address.
The wrapper publishes only the two lengths after semantic result and cleanup
settle. The generated command package's semantic carrier is independently
replayed before publication.

The `arg_utf8` provider status sub-domain is exactly 0/1/2; any other value is
a target-invariant failure. A zero-status `stdin_read` carrier is not trusted
from its tag and length alone. Before CleanupPlan initializes the owned slot,
the compiler calls the closed recoverable membership contract (0 = exact live
member, 1 = not a member), which authenticates the arena key and recorded
length even when the claimed length is zero. Nonexistent and wrong-length
tokens fail before initialization; provider-owned rejected entries are settled
by that provider boundary. After successful transcript publication, the
private Wasm staging pages are zeroed while the public transcript bytes remain.
The command wrapper additionally exports mutable
`__spx_command_input_status_v1`. It resets to zero for every invocation and is
set only after authenticating an `arg_utf8` failure code 1/2 or `stdin_read`
failure code 3/4. The ordinary `__spx_data_status_v1` remains the status-code
lane for all language failures. Consumers may attach domain
`semaprax.command-input.v1` only when the separate marker is nonzero and
exactly equals that ordinary code; arithmetic, contract, and internal
fail-stop failures leave the marker zero.

Project Manifest v6 is additive. It selects profile
`language-command-io.v1`, input `argv-utf8+stdin-bytes.v1`, one identical
`command`/`web_exports` stable ID with `() -> bool`, and exactly the sorted
capabilities `process.args.read`, `process.stderr.write`,
`process.stdin.read`, and `process.stdout.write`. Earlier manifests, Graphs,
Wasm, packages, carriers, and command adapters remain byte-frozen.

Reachable command-input or stderr meaning selects additive
`semaprax.graph.v19` above v18. Graph v19 records the closed operation table,
status domain, immutable invocation-root provenance, input/output bounds, and
success-only publication policy.

Windows npm publication remains deliberately fail-closed: Project v6 can
build and independently replay the carrier, but it does not claim a weaker
path-based publication primitive as equivalent to the Unix handle-relative
no-clobber route.

## Local evidence

The focused local gates are:

```sh
cargo test --locked -p semaprax --test bounded_language_command_io_v1
cargo test --locked -p semaprax --test language_command_io_native_v1
cargo test --locked -p semaprax --test project_manifest_v6
cargo test --locked -p semaprax --lib wasm::command_io::tests
cargo test --locked -p semaprax --lib project::npm::command_v3::tests
```

They cover source/HIR hostility, shared invocation-root provenance, cumulative
input and dual-output bounds, one-read/one-write path restrictions, exact
CleanupPlan builder/replay facts, Graph v19 projection, interpreter and native
C11 O0/O2 settlement, Core-Wasm/Node command execution, Project v6 canonical
manifest and carrier replay, and preservation of earlier schema/package
bytes. These are local artifact and execution facts only; they do not promote
the affected completion rows.

## Nonclaims

This tranche does not add files, directories, environment variables,
networking, child processes, terminals, interactive or streaming I/O,
multiple reads or writes, mutable input views, callbacks, async, threads,
WASI, Component Model I/O, arbitrary host imports, cross-descriptor atomicity,
physical write durability, dependency resolution, registry publication,
signing, provenance, or release promotion.
