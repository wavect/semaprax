# Caveman command.view adapter (opt-in, contract partly unverified)

Compresses already-captured command output through a user-started local Caveman runtime
(`caveman start`, 127.0.0.1:8787, run with `DO_NOT_TRACK=1` and `CAVEMAN_WORK_TAGS=0`; Caveman
defaults to record mode and this adapter requires the reply to say `mode: compress`).
Nothing installs, starts, logs in to or updates Caveman. `adopt --upstream <caveman>` only
identity-probes `caveman --version` (3.1.0).

Wire contract mirrored from `JuliusBrussee/caveman` at 8af1f1b9b1346bca0722a1556f119b4e6675cc96:
`docs/technical/middleware-protocol.md` (GET `capabilities`, POST `optimize`, plan constraints, loopback),
`packages/sdk/python/caveman_cloud/middleware/{runtime,types,validate,protocol,transport}.py`
(request body, `tool_result` segment kind, scope, policy, recovery binding, response validation).

Read through a summarising fetch tool only, never executed against a real runtime; treat as
UNVERIFIED until the fixture is replaced by a recorded real exchange: the exact `recovery_binding`
shape, whether a loopback runtime requires a bearer token (the adapter sends one only if
`<retention>/caveman-token` exists), the `status` enum (`applied|reused|bypassed` vs
`optimized|bypassed|record`; both accepted), and whether the runtime keeps its own copy of the
raw text in its local SQLite. `caveman shrink` is not used: it compresses MCP tool catalogs and re-runs
commands, not captured output. Caveman's recovery header is stripped; Semaprax retention stays authoritative.
