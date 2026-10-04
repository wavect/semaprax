# Caveman command.view adapter (opt-in)

Compresses already-captured command output through a user-started local Caveman runtime
(`caveman start`, 127.0.0.1:8787, started by the user with `DO_NOT_TRACK=1` and `CAVEMAN_WORK_TAGS=0`).
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

Still unverified: nothing here has run against a real Caveman runtime; the test fixture
(`crates/semaprax-harness/tests/fixtures/caveman/fake_runtime.py`) is written from the files above. The real
runtime's compress-mode eligibility and token measurements are TC-12's to measure.
