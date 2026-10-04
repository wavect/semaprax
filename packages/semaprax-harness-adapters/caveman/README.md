# Caveman command.view adapter (opt-in)

Compresses already-captured command output through a user-started local Caveman runtime
(`caveman-proxy`, 127.0.0.1:8787, started by the user with `DO_NOT_TRACK=1` and `CAVEMAN_WORK_TAGS=0`).
Nothing installs, starts, logs in to or updates Caveman; `adopt --upstream <caveman>` only identity-probes
`caveman --version` (3.1.0). Host provisioning, in the provider retention directory: `caveman-token`
(runtime credential, required) and optionally `caveman-endpoint` (`host:port`, loopback only).

Verified against source at JuliusBrussee/caveman 8af1f1b9b1346bca0722a1556f119b4e6675cc96
(`docs/technical/middleware-protocol.md` = mw.md, and `packages/sdk/python/caveman_cloud/middleware/`):

- Auth (mw.md §2, lines 77-83): every route, `capabilities` included, needs `Authorization: Bearer <runtime
  credential>`; no loopback exemption. Without `caveman-token` the adapter makes no request and plan bypasses
  with "caveman runtime credential not provisioned". It never sends a provider API key and never an `Origin`
  header (§2, lines 72-74).
- Features header (§3, lines 121-130; types.py `CLIENT_FEATURES_HEADER_VALUE`): `Caveman-Middleware-Features:
  http_status_v2, revision_tolerant` on every request, plus `Caveman-Middleware-Client`.
- Plan status enum: validate.py `plan()` accepts `optimized | bypassed | record`; the `applied|reused|skipped|
  recorded|disabled` set in mw.md §14 (line 718) is the client decision-event enum, not the wire plan. A decision
  about the content is a 200 plan with `bypassed`, `replacements: []` (§6, lines 314-318); the adapter delivers raw.
  `record` (capabilities `mode`, protocol.py parse) is the default and changes no payload: no optimize is sent.
- `recovery_binding` (runtime.py 501-505, validate.py): required for any replacement; shape `{id, kind:"host_tool",
  tool_name:"caveman_retrieve", overhead_text}`. It is declared only to satisfy that precondition; the Caveman
  marker `[caveman: shortened; exact original via caveman_retrieve handle=cmw_...]` (§7 item 2) is validated and
  then stripped, so the model is never pointed at a tool Semaprax does not expose. Semaprax retention is the only
  authoritative raw recovery.
- Plan validation is a port of validate.py `plan()` / §7 (request_id echo, input_digest, measurement, recovery,
  per-replacement digest, strictly smaller UTF-8 size, marker/handle). Any violation is a fallback to raw.
- Originals (§12, lines 613-675): stored upstream per authority until `expires_at` (default 604800 s), encrypted
  at rest only if the runtime has a key. The adapter uses a fresh `session_id` per call and calls
  `POST sessions/delete` afterwards (200 `{status:"revoked", originals_deleted}`; later use of that scope is 410
  `deleted`); `originals_deleted:false` would mean a 1.0 runtime kept them in its process-wide CCR. Deletion is
  best effort and never changes the view. No receipts are posted.
- `caveman shrink` compresses MCP tool catalogs and is not used.

Verified against a real 3.1.0 runtime on 2026-10-04. Pin: GitHub tag `v3.1.0` / commit 8af1f1b9 built from source
(`go build ./proxy/cmd/caveman-proxy`, Go 1.27.1); npm has no 3.1.0 (`@caveman-ai/cli` ends at 2.0.1 and is the agent
wrapper). The binary ran on 127.0.0.1:8787 with `CAVEMAN_MODE=compress DO_NOT_TRACK=1 CAVEMAN_WORK_TAGS=0` and a local
`CAVEMAN_AUTH_TOKEN`; its only socket was the loopback listener, before and during the exchange. Exactly verified: this
adapter's `view()` ran `GET capabilities`, `POST optimize` and `POST sessions/delete` against it on a synthetic noisy log
with two planted ERROR lines; every response passed the adapter's validation unchanged, the `log` transform kept both
ERROR lines, `sessions/delete` returned `revoked` with `originals_deleted:true`, and the view was far smaller than raw.
The exchange (token redacted) is `crates/semaprax-harness/tests/fixtures/caveman/recorded/`, replayed by
`caveman_replays_a_recorded_real_runtime_exchange_to_the_same_view`. Not verified: other transforms and content types,
record mode, error statuses and timeouts of the real runtime, and token measurements (TC-12).
