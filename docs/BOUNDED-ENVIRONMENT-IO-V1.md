# Bounded Environment I/O v1

Status: implemented bounded snapshot profile; **HOSTED GREEN** under the
[v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md), including the admitted
interpreter, native C11, Core Wasm, and provider-constructor checks.

Audience: compiler contributors, standard-library authors, host-adapter
implementers, and reviewers of capability boundaries.

Bounded Environment I/O v1 defines the `EnvironmentV1` input snapshot and the
Project `environment-io.v1` profile (Project v17). It exposes a
caller-supplied immutable environment to checked code through explicit host
operations. It never reads ambient process state or calls `getenv`.

## Operations

The compiler-owned operations are:

| Operation | Stable identity | Result | Effect |
| --- | --- | --- | --- |
| `env_len()` | `core.host.env-len` | fallible `usize` | `process.environment.read` |
| `env_name_utf8(index)` | `core.host.env-name-utf8` | fallible borrowed `str` | `process.environment.read` |
| `env_value_utf8(index)` | `core.host.env-value-utf8` | fallible borrowed `str` | `process.environment.read` |

The source-authored `std.env` package provides the wrappers in the private
`environment-io.v1` profile. Its focused conformance and backend checks are
part of the hosted-green interpreter, native C11, and Core Wasm release corpus.

The Core Wasm provider imports are exact and private to this profile:

| Import | Wasm signature | Success output |
| --- | --- | --- |
| `spx_environment_len_v1` | `(i32 out) -> i32` | writes a `u32` length |
| `spx_environment_name_utf8_v1` | `(i64 index, i32 out) -> i32` | writes an `i64` borrowed carrier |
| `spx_environment_value_utf8_v1` | `(i64 index, i32 out) -> i32` | writes an `i64` borrowed carrier |

The generated command uses one fixed 64 KiB environment arena. A provider
status of zero is accepted only after the corresponding output has passed the
root, bounds, NUL/`=` name, and strict UTF-8 checks; the output slot is
initialized to `-1` poison before each import. A failed lookup must leave the
carrier at `-1`; nonzero status leaves poison in place and follows the closed
status contract below. Root zero is valid for an empty borrowed value. These
imports are read-only and widen no authority.

## Explicit Wasm provider construction

`ProjectRevision::environment_provider_source()` exposes the standalone ES module
`wasm/environment_provider.mjs` only for an admitted environment Project. Its
`createEnvironmentProvider({environment, arguments, stdin})` factory validates the
complete combined snapshot before returning imports: environment entries are
`[name, value]` pairs, `null` means absent authority, arguments are explicit text
values, and stdin is an explicit `Uint8Array`. Text accepts strings or strict
UTF-8 byte arrays. Construction rejects isolated UTF-16 surrogates, malformed
UTF-8, duplicate names, invalid name/value bytes, and count or combined-byte
overflow. It sorts names by raw bytes and retains private copies; subsequent
caller mutation cannot change the snapshot.

The owned literal arena remains a separate 64 KiB region at byte offset
393216 (the seventh Wasm page), disjoint from the six command-input pages; this
does not raise the command input capacity.

The result supplies the three environment imports and four command-input
imports. Call `attach(memory, {allocateOwned, validateOwned})` before each
invocation. Attachment copies the immutable text snapshot into the reserved
first 64 KiB, resets stdin consumption, and binds output slots to writable
scratch outside that input arena. The optional owned-byte adapter is needed
only when stdin is read: allocation receives a fresh copy and must return a
validated tagged owned carrier. It must share the same owner inventory as the
host's other byte imports. Absent allocation authority returns stdin failure.
Repeated environment lookups reuse existing carriers without recharging input.

This construction boundary enforces complete snapshot ordering, uniqueness and
combined capacity. Generated Wasm separately checks each raw import's status,
poisoned output, borrowed carrier range and UTF-8. Those local checks alone do
not authenticate a complete snapshot. A caller that replaces the imports,
mutates the reserved arena during execution, or reattaches during an active
invocation violates the provider contract; the module does not claim to make
arbitrary malicious host imports cooperate. The provider itself reads no
ambient process state and grants no filesystem or network access.

## Snapshot contract

The host constructs one private immutable `EnvironmentSnapshot` and passes it
through `HostedEnvironmentCommandInput`, which composes the unchanged
`HostedCommandInput` with `Option<EnvironmentSnapshot>`. `Some(empty)` is an
explicit empty environment and makes `env_len()` return zero. `None` denies
all environment operations, including `env_len()`, with status code 4.

The snapshot admits at most 256 entries and at most 65,536 combined name and
value bytes. Names are nonempty strict UTF-8, contain no NUL and no `=`; values
are strict UTF-8, contain no NUL, and may be empty. Construction canonicalizes
entries by UTF-8 bytewise key order and rejects duplicate keys. Raw callback
inputs must satisfy this complete contract before any result or snapshot is
published.

The one invocation-owned environment arena is shared by repeated name/value
lookups. A borrowed view cannot escape the invocation, be stored in an
aggregate, cross a host or public boundary, or outlive settlement. Checked
internal calls, including `std.format`'s borrowed-`str` helpers, may consume
the view within the invocation.
Repeated lookups do not recharge the budget. The existing combined argv plus
stdin limit remains 65,536 bytes, and the environment snapshot is included in
that same total; the existing maximum of 16 arguments is unchanged.

Failures use the closed `semaprax.environment-input.v1` domain:

| Code | Meaning |
| --- | --- |
| 1 | environment index is out of bounds |
| 2 | input name or value is invalid UTF-8 or violates its byte rules |
| 3 | snapshot capacity is exceeded |
| 4 | environment authority is absent or denied |

## Profile and authority

The `environment-io.v1` profile permits environment reads together
with the existing argv/stdin inputs, stdout/stderr append operations, and
byte-range meaning. It excludes legacy single writes, network operations, and
filesystem operations. Existing language-command, line-command, network,
filesystem, and effect-free profiles reject environment operations. Their
manifest, graph, descriptor, and carrier bytes remain frozen.

Environment data is user-provided invocation input, not ambient authority. No
compiler, generated program, interpreter, native adapter, or Wasm adapter may
read the host environment, home directory, process table, or inherited
variables directly. The host must inject an explicit snapshot, and an absent
snapshot must fail closed.

<a id="planned-graph-and-verification"></a>

## Implemented graph and verification

The Project profile selects Graph v43. Its graph facts identify
the three operation IDs, `process.environment.read`, the entry/byte bounds,
canonical key ordering, UTF-8/NUL/`=` rules, shared arena lifetime, and the
status domain. Graph v19/v20 command facts and all earlier graph projections
remain unchanged.

The maintained corpus covers source/HIR admission, snapshot constructor
hostility, interpreter/native C11/Core Wasm execution, repeated lookup and
lifetime rules, strict combined capacity, absent-versus-empty authority, and
legacy-profile rejection. The implementation has hosted-green release evidence;
its earlier local observations retain their original scope. Production,
secret-input handling and general ambient process-environment support are not
provided by this bounded snapshot profile.

[Bounded Process I/O v1](BOUNDED-PROCESS-IO-V1.md) separately composes these
inputs with registered-tool execution under Project v18. That implemented
extension does not grant this environment-only profile process authority.
