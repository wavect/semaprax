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
| `sqlite` / `postgresql` + DSN ref | Durable snapshot store under `--state-dir`. No SQL wire protocol is implemented; the DSN value is held but never connected to. |
| `native` + `modern` TLS + listen origin | Loopback plaintext HTTP/1.1 on `--port`. The listen origin is intent only; TLS provisioning is open (see below). |
| Three secret refs | Exact files under `--secrets-dir`, resolved before serving. |
| `otlp` + endpoint origin | HTTPS POST to `<origin>/v1/events` through `deliver_http_durable`. No OTLP protobuf is emitted. |

## Invocation mapping

Each exchange first passes the scaffold's own `request_is_admitted`
decision; the four invocable decisions (frozen `i64`/`bool`/borrowed-bytes
vocabulary plus an effect- and contract-free closure) gate request lines,
registration names, and row ownership. Idempotent enqueue mirrors the
checked decision's 0/1/2 truth table host-side because its closure reaches
a contract-bearing callee. The remaining scaffold decisions take
`u8`/`usize` parameters the vocabulary does not carry; they keep their
fixture-mode coverage and are documented as open in
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
  --secrets-dir <dir> --bundle-dir <dir> --port <1-65535> [--state <digest>]
```

The server prints `bundle <digest>` then `ready port=... state=... seq=...`
and serves until stopped. There is no graceful shutdown; crash-safety
comes from the store. All directories must already exist; all secrets must
resolve; the port must be explicit and loopback-only.

## Non-claims

No SQLite/PostgreSQL protocol, no TLS server provisioning (loopback
plaintext only; `accept_tls` needs operator-supplied certificate material
this host does not mint), no OTLP protobuf, no hosted/public/production
support, no graceful shutdown, and no per-request decisions outside the
frozen invocation vocabulary. The `.invalid` origins in tests exist so no
real peer can be contacted; delivery attempts there fail closed by design.
