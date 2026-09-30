# Reference Service Host v1

Status: implemented bounded profile; local evidence only
(`semaprax-native-host` unit tests plus
`tests/runtime_host/reference_service_acceptance.rs`). No hosted, release,
or production run is claimed.

Audience: operators running the existing service scaffold
(`PROJECT-SCAFFOLD-SERVICE-V1`, `examples/task-service-project`) as a local
process, and contributors maintaining the host wiring.

## Purpose

The scaffold's `.spx` sources are checked decisions with no sockets, files,
clocks, or delivery authority. This host runs them as a real local process:
loopback HTTP serving, physical persistence, held secrets, and durable
outbound delivery, all under host-created grants. It reuses the
repository's existing machinery and adds no new authority:

- configuration intent: the existing closed service-config and
  adapter-request decoders (`derive_service_host_adapter_request_v1`);
- decisions: per-request evaluation from the operator-retained
  authenticated revision (`ProjectRevision::evaluate_service_decision_v1`);
- persistence: canonical snapshots through the existing durable checkpoint
  store (`OutboundDeliveryStore`, `ServiceState` kind);
- outbound delivery: R17's `deliver_http_durable` over the existing TLS
  client path (`TcpNetworkProvider::connect_tls`);
- serving: loopback-only HTTP/1.1 over `TcpNetworkProvider`;
- passwords: the existing Argon2id password host;
- telemetry target: the decoded endpoint origin under the existing fixed
  `/v1/events` route.

## Adapter selection (explicit)

| Requirement (decoded intent) | Bound adapter |
| --- | --- |
| `snapshot` | Durable snapshot store under `--state-dir`. SQL adapter labels and DSN references are refused during configuration decoding. |
| `native` + `modern` TLS + listen origin | Loopback HTTP/1.1 on `--port`, plaintext by default. The listen origin is intent only and never itself provisions TLS; `--tls-certificate-secret`/`--tls-private-key-secret` opt in (see below). |
| Three secret refs | Exact files under `--secrets-dir`, resolved before serving. |
| `semaprax-json-events` + endpoint origin | Canonical JSON event HTTPS POST to `<origin>/v1/events` through `deliver_http_durable`. OTLP labels are refused during configuration decoding. |

## Invocation mapping

Each exchange first passes the scaffold's own `request_is_admitted`
decision. The checked invocation vocabulary admits `i64`, `u8`, `usize`,
`bool`, and borrowed bytes when the selected closure is effect- and
contract-free. The host invokes the scaffold's request-line, registration
name, row-ownership, and session-deadline decisions. Idempotent enqueue mirrors the
checked decision's 0/1/2 truth table host-side because its closure reaches
a contract-bearing callee. The remaining scaffold decisions not required by
these routes retain their fixture-mode coverage and are documented as open in
`reference_service::decisions`.

Routes: `GET /v1/health`, `POST /v1/register`, `POST /v1/login`,
`POST /v1/logout`, `POST /v1/tasks`, `GET/PATCH/DELETE /v1/tasks/<id>`,
`POST /v1/jobs/enqueue`, `POST /v1/jobs/<id>/complete`, `GET /v1/jobs/<id>`.
Mutating routes answer with the new state digest; completing a job attempts
one durable webhook delivery whose stable identity prevents redispatch.

## Persistence and restart

Snapshots are content-addressed and write-once; the store never selects a
"latest". The operator retains each accepted digest and passes it as
`--state` after a restart. A lost digest is unrecoverable by design. An
`Uncertain` commit advances no in-memory state, so a client retry replays
byte-identical content to `Committed`. Job completion attempts delivery
before committing; a crash between the two leaves a pending job whose
durable marker already exists, so the retry settles `Uncertain` instead of
redispatching. Fixture mode stays separate: fixture-mode configuration is
refused here and keeps running on `semaprax run` / `semaprax test`.

## Run bundle

`serve` (and the standalone `bundle` subcommand) writes a digest-bound
local bundle: the exact config bytes, the derived canonical adapter-request
bytes, and a manifest binding both digests. The bundle is verified at
startup. It names no base image, registry, signature, or publication; OCI
runtime execution remains open.

## Operating

```sh
semaprax-reference-service serve --project examples/task-service-project \
  --config service.config.json --state-dir <dir> --outbound-dir <dir> \
  --secrets-dir <dir> --bundle-dir <dir> --port <1-65535> [--state <digest>] \
  [--session-idle-seconds <n> --session-absolute-seconds <n>] \
  [--tls-certificate-secret <ref> --tls-private-key-secret <ref>]
```

The server prints `bundle <digest>` then `ready port=... state=... seq=...
tls=on|off` and serves until stopped. There is no graceful shutdown;
crash-safety comes from the store. All directories must already exist; all
secrets must resolve; the port must be explicit and loopback-only.

## TLS serving (optional, operator-held material only)

Serving is plaintext by default. An operator opts into TLS by naming both
`--tls-certificate-secret` and `--tls-private-key-secret` -- exact
references resolved against `--secrets-dir` through the same
hold/read/recheck discipline as the three password/session/webhook
secrets, holding one leaf certificate (DER) and its PKCS#8 private key
(DER). Naming only one of the pair is a usage error; naming both without a
held file under either name refuses before any listener binds, so
configuration intent alone never mints TLS authority. When TLS is
configured, the host serves *only* under `TcpNetworkProvider::accept_tls`
(TLS 1.2/1.3 via Rustls) for the whole process lifetime; there is no
plaintext fallback, and a plaintext client dialing a TLS-configured
listener gets at most a raw TLS alert record, never a downgraded HTTP
exchange. Restarting a TLS-configured deployment requires passing the
same two flags again.

Local evidence: `tests/runtime_host/reference_service_acceptance.rs` runs
the full login/CRUD/job/restart flow over TLS with a checked-in test CA,
plus hostile cases -- a client that does not trust the certificate's
issuer, a client dialing a name the certificate does not cover, an
already-expired test certificate, and a plaintext client against a
TLS-configured listener -- and the certificate-config decoder builders
(`server_tls_config_from_der`, `client_tls_config_trusting` in
`semaprax::network_provider`) carry their own unit coverage. No hosted,
public, or production TLS deployment is claimed; this is loopback and test
material only.

## Non-claims

No SQL or OTLP adapter label is accepted, no hosted/public/production
support, no graceful shutdown, and no per-request decisions outside the
frozen invocation vocabulary. TLS server provisioning is now available but
only from operator-held certificate/key material named on the command
line, not from configuration intent, and not chained beyond the one leaf
certificate this host holds. The `.invalid` origins in tests exist so no
real peer can be contacted; delivery attempts there fail closed by design.
Sessions carry persisted Unix-second idle and absolute deadline facts plus the
checked `std.auth.session` state code. At each authenticated request, the host
invokes the scaffold's `session_is_usable` and `session_next_state_on_access`
decisions with those facts and the current host tick. The latter scaffold
wrapper mirrors the `std.auth.session.next_state_on_access` branches using
contract-free `std.auth` predicates: the host's closed public-API evaluator
does not admit the standard transition's `ensures` clause in its call closure.
The logout wrapper follows the same rule. Their answers must agree:
an active result remains usable, while expiry is committed as its selected
terminal state before the ordinary unauthorized response. Explicit logout
similarly persists the source-selected logout state. The default fixed
deadlines are 15 minutes and 8 hours from login, with CLI values bounded to
`idle <= absolute <= 7 days`. Snapshot schema v3 adds the state code; v1 and
v2 snapshots are deliberately refused rather than guessed or silently
migrated.
