# Harness adapter host v1 (HP-03)

Status: additive development-harness specification (HP-00); local macOS aarch64 evidence; see HARNESS-PLATFORMS-V1.md for the per-platform matrix.

Audience: toolchain contributors and harness adapter authors.

Implements the host side of `docs/HARNESS-PROVIDER-V1.md` (wire protocol,
lifecycle) in `crates/semaprax-harness/src/host/`. Diagnostics are
`SPX-HPC001..`.

## Not part of compile/check

`semaprax-harness` is a separate crate that the compiler does not link. Ordinary
`semaprax check`/`build` never starts an adapter and needs none of an adapter's
runtime dependencies (node, python, upstream tools). Adapters start only when a
host caller invokes `AdapterHandle::invoke`, after a trust `Grant`.

## Public API

- `LaunchSpec { descriptor, descriptor_dir, runtime_executable, upstream_executable, grant, project_root, cache_dir, retention_dir, isolation: IsolationRequest, forward_env }`.
  `prepare` refuses when the grant does not name this descriptor digest
  (`HPC001`), when the entry file or upstream executable differs from the
  granted digest (`HPC002`), or when requested isolation is not enforceable
  (`HPC003`). The digest is re-checked on every (re)start. Hashing and exec are
  separate steps (no held-fd exec in `std`), so a swap between them is not
  excluded; the window is the same as for any path-based launch.
  A second `prepare` for the same `(project, provider)` reuses the cached
  handle only when the complete launch identity is equal: descriptor digest,
  isolation request, grant, runtime and upstream executables, descriptor, project,
  cache and retention paths, and `forward_env`. Any difference is refused with
  `HPC001` naming the differing input; a live handle is never mutated in place
  and replacement requires an explicit close first (MA-01).
- `AdapterManager::new(HostConfig)`, `prepare(project_id, LaunchSpec) -> HarnessResult<Arc<AdapterHandle>>`,
  `handle(project, provider)`, `reap_idle(now: Instant)`, `shutdown_all()`, `budget_used()`.
- `AdapterHandle::invoke(&RequestEnvelope, InvocationClass, &CancelToken) -> Outcome`,
  `state()`, `isolation_mode()`, `stderr_tail()`, `shutdown()`, `reap_idle(now)`.
- `InvocationClass { SafeRead, Decision, SideEffecting }`.
- `Outcome { Completed(ResultEnvelope), Refused(diag), Quarantined(diag), Unavailable { reason, request_sent, fallback_allowed }, Cancelled, Uncertain(diag) }`.
- `IsolationRequest { None, Restricted { allow_read, allow_write, network: NetworkPolicy::Deny } }`,
  `IsolationMode { Subprocess, OsEnforced { mechanism } }`, `IsolationBackend::detect()`.
- `ApprovedEndpoint::from_host_config(url)`, `Credential::new(header, value)`,
  `HttpClient::new(endpoint, credential, HttpLimits).request(method, path, body)`.
- `HostBudget { max_jobs, max_total_ms, max_output_bytes }`, separate from the
  root crate's `process_provider` limits (not reused, not enlarged).

## Process model

- `env_clear()` plus `PATH=/usr/bin:/bin`, `HOME`/`TMPDIR` below the cache dir,
  `SEMAPRAX_HARNESS_{PROJECT_ROOT,CACHE_DIR,RETENTION_DIR,UPSTREAM}`, python
  `PYTHONDONTWRITEBYTECODE`/`PYTHONNOUSERSITE`, and the explicit `forward_env`
  (reserved keys refused). Working directory is the cache dir.
- Own process group; deadline, cancel-after-grace, violation, shutdown and drop
  all send group `SIGKILL` and reap. A descendant that calls `setsid` leaves the
  group and is not reached.
- stdout: one frame at a time, capped at `min(max_frame_bytes, 4 MiB)`; stderr:
  64 KiB ring with a dropped-byte count, drained by its own thread. A waiter
  polls with `recv_timeout`, so cancellation never waits on a read.
- Protocol violations (non-JSON or malformed line `HPC009`, unknown/duplicate
  response id `HPC010` -- which also catches a flood on its first frame --,
  adapter-initiated request or notification incl. `sampling/*`, `roots/*`,
  `resources/*` `HPC011`, oversized frame `HPC012`, bad initialize `HPC006`,
  spoofed result binding/authority member `HPC015`) kill the group and
  quarantine. Payload-level refusals (`HPA040..046`, e.g. path escape, choice
  outside options) discard the result and keep the adapter.
- A violation the reader records while no request is pending (an unsolicited
  frame between invocations) has the same effect. Before any replacement the
  handle reconciles the cached generation's exact close reason under the `core`
  lock: `Violation` quarantines (`state()` reports it at once, and the next
  invocation returns the original diagnostic with no new start and no
  `harness/invoke` frame); an idle `Exited`/`Transport` close is charged to the
  crash breaker once per generation; host-initiated closes (idle reap,
  shutdown) stay freely restartable. A stale generation never overwrites a
  newer one.

## Diagnostics

001 grant/spec mismatch or reserved env key; 002 entry/upstream digest;
003 isolation not enforceable; 004 start/path error; 005 handshake timeout;
006 handshake protocol; 007 adapter exited; 008 invoke deadline;
009 malformed frame; 010 unknown response id; 011 adapter-initiated message;
012 oversized frame; 015 envelope binding breach; 016 crash breaker;
017 queue full; 018 cancelled side-effecting call; 020 request not accepted /
wrong project; 021 handle closed; 022 host budget exhausted; 023 nothing
negotiable; 024 adapter error response; 030 remote/TLS endpoint refused;
032 redirect refused; 033 response too large; 034 HTTP timeout; 035 HTTP I/O;
036 malformed HTTP response; 037 invalid endpoint/credential/request.

## Isolation

`Restricted` uses `/usr/bin/sandbox-exec -p <profile>` on macOS (deny default,
deny `network*`, reads only below system paths and allowed roots, writes only
below write roots) and `/usr/bin/bwrap` on Linux (`--unshare-net`, read-only
and read-write binds). With neither tool the request is refused. Host-added
roots: descriptor dir, interpreter prefix, cache dir (read-write), project root
when `read` grants `project`, retention dir when `write` grants `retention`.

## HTTP facility

Plain HTTP/1.1 over loopback only. A non-loopback host or any `https` URL is
refused with "remote transport requires TLS, unsupported in v1". Redirects
are refused, responses are size-bounded, calls are timeout-bounded, and the
credential header is injected by the host and redacted in `Debug`. The only way
to name a destination is `ApprovedEndpoint::from_host_config`.

Chunked responses are decoded strictly: the chunk size is hex digits only, a size
that does not fit the remaining response budget (including one beyond `usize`) is
`SPX-HPC033`, never a panic; each data chunk must be followed by exactly CRLF; the
last chunk must be followed by a bounded (8 KiB) trailer section of `name: value`
lines and the terminating empty line. Truncated or malformed framing is
`SPX-HPC036`, so a body that is not complete valid chunked framing is never a success.

## Cancellation across the bridge (HN-18)

`CancelToken` is the only cancellation input; the bridge (`bridge/invoke`/`bridge/cancel`,
`docs/HARNESS-BRIDGE-V1.md`) trips it from a second protocol frame. Outcome mapping: `Cancelled` (safe class) is a
confirmed termination (group killed and reaped); `Uncertain` (`SPX-HPC018`, side-effecting request already sent) is an
uncertain external effect and is never retried. Admission and the queue (`SPX-HPC017`) apply unchanged.

## Pre-dispatch checks and post-dispatch uncertainty (MA-02, MA-04)

One absolute deadline (`min(request deadline, invoke_timeout_ms)`) bounds admission, waiting for start ownership,
the handshake (`min(handshake_timeout_ms, remaining deadline)`) and dispatch. Cancellation and expiry are checked on
the free-slot admit path, while waiting for the start gate, after the handshake and immediately before
`harness/invoke` is queued. A stop at any of these points sends nothing: it returns `Cancelled`, or `Unavailable`
(`SPX-HPC008`, `request_sent: false`), releases the gate slot and job counter once, and does not count as an adapter
crash. A handshake cut short by the invocation deadline kills that process and returns the handle to `Prepared`.
The race after the final host check is not an atomic remote-cancellation guarantee.

After `harness/invoke` was queued, a JSON-RPC error (`SPX-HPC024`) or a result that fails payload validation is not
proof of non-execution: for `SideEffecting` it returns `Uncertain` (never `Refused`), so the journal records it as
non-replayable. `SafeRead` and `Decision` keep `Refused`. Pre-dispatch rejections (validation, undeclared operation)
remain `Refused` and retryable.

## Platform evidence

The authoritative per-platform pass/fail/untested matrix is `docs/HARNESS-PLATFORMS-V1.md`.


| Platform | Executed evidence |
| --- | --- |
| macOS aarch64 | Full lifecycle, hostile modes, grandchild settlement, `sandbox-exec` secret and loopback-network blocks, loopback HTTP (`cargo test -p semaprax-harness --lib host::`). |
| Linux | Not evidenced. `bwrap` argument generation is unit-tested; nothing was executed. |
| Windows | Not evidenced; the host is Unix-only (`process_group`, `rustix`). |

## Known gaps

Outbound MCP client bridge is HP-14. TLS and remote HTTP are not implemented.
Concurrent invocations share one adapter process and are demultiplexed by
JSON-RPC id; an adapter that serialises internally gains no parallelism.

The bridge `--stdio` session drives `reap_idle` from a 250 ms maintenance tick while waiting for input
(`docs/HARNESS-BRIDGE-V1.md`); embedders that do not serve a bridge session still call it themselves.
