# Bounded Process I/O v1

Status: bounded local verification passes for the selected `std.process` and
Darwin provider paths; the complete profile and cross-platform gates remain pending.

Audience: compiler contributors, standard-library authors, host-adapter
implementers, and reviewers of capability boundaries.

Bounded Process I/O v1 defines one explicit process request and one complete
result. It does not grant checked code ambient process authority, shell access,
or arbitrary executable lookup.

## Request

The source-level operation has this exact eight-argument signature:

```semaprax
process_run(tool: usize, argv: borrow Slice<u8>, argv_length: usize,
            stdin: borrow Slice<u8>, stdin_length: usize, timeout_ms: usize,
            stdout_max: usize, stderr_max: usize) -> own Bytes
```

Its stable operation identity is `core.host.process-run`. The executable is
selected by an explicit numeric registry tool identity; it is never a path,
`PATH` lookup, shell command, or shell script.
The registry entry supplies the executable, explicit working directory,
environment policy, and argv policy.

The argv wire starts with one little-endian `u32` argument count. Each item
then contains a little-endian `u32` byte length and that many non-NUL bytes.
Empty arguments and non-UTF-8 bytes are admitted. `argv_length` selects the
exact wire prefix; `stdin_length` selects the stdin prefix. Trailing staging
bytes outside these prefixes are ignored, but trailing bytes inside the argv
wire are rejected. The request carries the
combined argv wire and stdin input within 65,536 bytes, a timeout between 1
and 30,000 milliseconds, and the existing maximum of 16 arguments.

One invocation admits at most 16 process runs and at most 1 MiB of cumulative
input plus reserved output-wire bytes. The complete request is validated before
its reservation is admitted. Once valid capacity is reserved, it is consumed
even when launch, I/O, or settlement later fails; there are no refunds. All
arithmetic and wire lengths are checked before launch.

## Result wire

The result is exactly one version-1 little-endian wire:

1. `u64` version `1`;
2. `u64` termination, encoded as `(code << 2) | kind`, where kind `0` is a
   normal exit with a `u32` exit code and kind `1` is a nonzero `u8` signal;
3. `u64` stdout length;
4. `u64` stderr length;
5. the exact stdout bytes; and
6. the exact stderr bytes.

The reservation satisfies `32 + stdout_max + stderr_max <= 65,536`. Each
returned stream must fit its requested maximum, and the complete wire must
have exactly `32 + stdout_length + stderr_length` bytes, without trailing data.
The caller receives a result only after both pipes and the child have settled.
The provider's fallible settlement returns success `0` or failure status `7`
before result publication. Failure publishes no partial output. A nonzero
normal exit is still a normal settled result and is not itself a process-I/O
failure.

## Closed failures and authority

Failures use `semaprax.process.v1`:

| Code | Meaning |
| --- | --- |
| 1 | invalid request input or wire |
| 2 | process authority denied |
| 3 | process launch failed |
| 4 | timeout |
| 5 | capacity exceeded |
| 6 | pipe or process I/O failure |
| 7 | child or pipe settlement failed |

The provider is explicit and trusted for its registered tools. It enforces the
registry's cwd, environment, and argv policy and grants no arbitrary path,
shell, network, filesystem, or ambient environment authority. The contract
does not claim a hard OS-level reap deadline, sandbox confinement, or hosted
production support.

The registered physical provider is owned by
`src/process_provider/registered.rs` and
`src/process_provider/registered/platform.rs`. Registration holds an already
opened executable and cwd, an explicit `argv0`, a canonical private
environment, and a numeric tool policy; it never resolves paths, consults
`PATH`, inherits the host environment, or discovers the current directory.
The platform adapter uses Linux `fexecve` and Darwin suspended-launch
attestation of the held cwd vnode and executable mapping. It creates separate
stdin, stdout, and stderr pipes, applies the request deadline, kills the
process group on timeout or failure, and reaps the child before publication.
Failed settlement is quarantined and blocks subsequent launches until the
leader is reaped and its process group disappears. Quarantined reaped
identities are never signalled again. The embedding must preserve exclusive
reaping of this provider’s children. An ignored, custom, or `SA_NOCLDWAIT`
SIGCHLD policy is rejected before launch; detected loss of child ownership
permanently quarantines the identity without further signals. An uncertain
raw descriptor close fails by terminating the host, rather than silently
relinquishing descriptor authority. This low-level provider is a separate implementation
from Git process authority and reuses none of that authority.

## Project and graph admission

`ProcessV1` is the additive Graph v44 / Project v18
`process-io.v1` profile. It permits `process.execute` together with the
existing `process.environment.read`, `process.args.read`,
`process.stdin.read`, `process.stdout.write`, and `process.stderr.write`
capabilities: six permits in total. It rejects filesystem and network effects,
legacy single writes, and every older profile. Existing manifest, graph,
descriptor, carrier, and target bytes remain frozen.

The profile's graph facts identify the numeric registry tool, request bounds,
timeout, cumulative reservation, complete-result settlement, termination
encoding, output-wire limits, and the closed failure domain. The named local
package gate covers the interpreter, native C11 `-O0`/`-O2`,
and Core Wasm example, conformance, and bundled-consumer commands. Five focused
physical Darwin provider cases also pass locally. Linux physical-provider, hosted,
public, and broader process gates remain open; this evidence does not complete
the profile or claim production support.
