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
| `snapshot` | Durable snapshot store under `--state-dir`. `sqlite` and `postgresql` are refused with stable adapter-specific diagnostics during both configuration and independent request decoding; non-null `dsn_secret_ref` is refused rather than reinterpreted as a state path. |
| `native` + `modern` TLS + listen origin | Loopback HTTP/1.1 on `--port`, plaintext by default. The listen origin is intent only and never itself provisions TLS; `--tls-certificate-secret`/`--tls-private-key-secret` opt in (see below). |
| Three secret refs | Exact files under `--secrets-dir`, resolved before serving. |
| `semaprax-json-events` + endpoint origin | Canonical `semaprax.json-event.v1` HTTPS POST to `<origin>/v1/events` through `deliver_http_durable`. The closed completion envelope carries `schema`, `event: "job.completed"`, `job_id`, `owner`, `desc`, and an HMAC commitment; its `x-semaprax-event-schema` header repeats the schema identifier. OTLP labels are refused during configuration decoding. |

## Invocation mapping

Each exchange first passes the scaffold's own `request_is_admitted`
decision. The checked invocation vocabulary admits `i64`, `u8`, `usize`,
`bool`, and borrowed bytes when the selected closure is effect- and
contract-free. The host invokes the scaffold's request-line, registration
name, row-ownership, session-deadline, and idempotent-enqueue decisions. The
enqueue wrapper compares borrowed descriptor bytes in checked source and
returns the 0/1/2 outcome; the host refuses evaluator errors or out-of-range
outcomes. Direct selection of the contract-bearing standard-library enqueue
helper still refuses at the public-API seam. The remaining scaffold decisions not required by
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


## Deterministic session-policy boundary (#336)

The native host observes Unix seconds from its own `SystemTime` clock, after
credential admission, and passes one tick to the checked session predicates.
The private `handle_with_clock` seam supports deterministic host tests; HTTP
requests, configuration bytes and scaffold code cannot choose a clock or gain
time authority. Health, registration and malformed credentials do not acquire
a session-clock dependency. An unavailable clock preserves committed state
and refuses login with `clock_unavailable`, or authenticated work with
`decision_failed`.

The two login deadlines stay fixed: access does not refresh the idle deadline.
Equality expires a session, absolute expiry wins when both deadlines have
passed, and a persisted terminal state cannot become active even if a later
host observation is earlier. This is not a general wall-clock rollback
protection claim for a still-active session.

The authored `reference_service::mapping::tests::session_policy` corpus covers
just-before/equal/after boundaries, reloading exact expiry snapshot bytes,
terminal replay, unavailable clocks, and parity between actual `std.auth`
transition execution and both reference/generated scaffold wrappers. The
normal project interpreter supplies the contract-bearing standard-library
oracle; the host's public decision seam remains contract-free. The existing
real-process expiry case now restarts from its retained state digest, alongside
a coincident idle/absolute-deadline case. That process case establishes
refusal across restart; the deterministic mapping test separately asserts
absolute-expiry precedence through the persisted state code.

Local focused execution on 1 October 2026 passed all five selected tests:

- `exact_session_deadlines_persist_and_remain_terminal_after_reload`: 1/1.
- `unavailable_session_clock_refuses_without_state_changes`: 1/1.
- `session_transitions_match_std_auth_and_the_generated_scaffold`: 1/1,
  comparing all 64 oracle rows against both decision engines (286.10 seconds).
- `expired_session_is_refused_by_the_checked_source_policy`: 1/1,
  including the real-process idle-expiry restart (23.55 seconds).
- `coincident_session_deadlines_refuse_after_restart`: 1/1,
  including real-process coincident-deadline refusal after restart
  (21.57 seconds).

These are local working-tree results for the session-policy batch, not an
exact-commit, complete service-suite, full-profile, hosted, installed-container
or production receipt. The separate unit assertion establishes absolute-expiry
precedence; the coincident-deadline process result is not isolated
absolute-only expiry evidence.

Focused verification uses the existing harnesses:

```sh
cargo test --locked -p semaprax-native-host --lib reference_service::mapping::tests::session_policy -- --test-threads=1
cargo test --locked -p semaprax-native-host --test runtime_host reference_service_acceptance -- --test-threads=1
```

The repository full profile and generated-scaffold preservation gates remain
required. The remaining scaffold decision routes described above, runnable OCI
packaging and its installed runtime journey remain open; these five passes do
not establish complete #336 acceptance.

## Offline native service packaging (#336, pending execution gate)

`scripts/package-reference-service.py` adds a separate operator-invoked package
route. The frozen `semaprax build --target oci` route remains a Wasm artifact;
it cannot start this native service. This new route consumes an explicitly
selected executable, its expected `sha256:<hex>` digest, a trusted host-native
checker, the service project, and host-mode configuration. It copies only
`semaprax.toml` and `src/{app,core,tests}.spx` plus the separate configuration;
projects requiring other source paths refuse during the copied-project check.
Secrets, snapshots, delivery records, and incidental project files are excluded.

The checker is `semaprax-reference-service check-package --project <dir>
--config <file>`. It authenticates the staged project and binds its service
decision set, then validates host-mode configuration without resolving secrets,
writing a bundle, or binding a listener. The packager executes the explicitly
trusted checker, never the supplied runtime executable. The digest binds the
copied executable bytes; it does not prove that they implement this service.
The operator must supply a trusted build from the intended revision. Startup
repeats project/configuration validation and acquires ordinary runtime grants.

Example development package, after building/installing a trusted host binary:

```sh
python3 scripts/package-reference-service.py --format development \
  --checker /absolute/path/semaprax-reference-service \
  --executable /absolute/path/semaprax-reference-service \
  --executable-sha256 sha256:<expected-hex> \
  --project examples/task-service-project --config /absolute/path/host.config.json \
  --output /absolute/path/new-service-package
```

The destination must not exist; its parent must be operator-controlled. The
packager checks bounded copied input bytes before creating it. On publication
I/O failure a partial directory may remain; only the final
`service-package.json` receipt indicates completion. Receipts and digests are
unsigned evidence and grant no authority. Development format admits only a
host-architecture thin macOS executable or host-architecture static Linux ELF;
Windows, cross-host development executables, and Linux dynamic executables
refuse. The macOS package relies on the operator's compatible system libraries.

Run the development executable as `new-service-package/bin/semaprax-reference-service
serve`, using `--project new-service-package/service` and
`--config new-service-package/service/service.config.json`, plus the existing
explicit port and four held-directory flags. Installation can independently use
`cargo install --locked --offline --path crates/semaprax-native-host
--bin semaprax-reference-service --root <private-install-root>` when all locked
dependencies are available locally; installation is not performed by packaging.

For `--format oci`, supply a trusted static Linux executable instead. The
bounded admission accepts little-endian ELF64 `ET_EXEC` for amd64 or arm64,
requires an executable load segment, and rejects `PT_INTERP`, `PT_DYNAMIC`,
truncation, and unsupported headers. Static PIE is deliberately outside this
first format. This is structural admission, not an executable correctness or
provenance proof. Packaging may happen on macOS; the resulting image requires
a matching Linux execution environment.

OCI output contains a deterministic uncompressed rootfs tar layer, standard
[OCI image configuration](https://github.com/opencontainers/image-spec/blob/v1.1.0/config.md),
content-addressed config/manifest/layer blobs, index, and layout marker. Its
entrypoint invokes `/bin/semaprax-reference-service serve` with the fixed paths
below and port `8080`; working directory is `/service` and default user is
`65532:65532`. Empty `/state`, `/outbound`,
`/bundle`, and `/secrets` mountpoints carry no data or authority. Writable
state/outbound/bundle mounts must permit the selected UID; secrets should be
mounted read-only. The image has no shell, loader, downloaded base, registry
operation, credential embedding, signing, or publication step.

After explicitly importing the layout into a trusted Linux runtime, the
configured entrypoint already supplies `serve --project /service --config /service/service.config.json
--state-dir /state --outbound-dir /outbound --bundle-dir /bundle
--secrets-dir /secrets --port 8080`. The operator must provide the four explicit
mounts described above; without them startup refuses missing secrets or
unwritable state. Preserve the returned state digest and append `--state <digest>`
to the entrypoint for restart. An appended `--port <port>` overrides the default
through the existing CLI parser; TLS and session-policy flags can likewise be
supplied explicitly. Loopback serving requires clients
in the same network namespace or an explicitly granted Linux host network;
ordinary published-port forwarding to a container interface does not make the
loopback listener reachable. Outbound delivery also needs explicitly permitted
DNS/network access to the configured HTTPS peer. Runtime OCI import and mount
syntax is tool-specific and has not been exercised here.

Focused gates on the current `wavect/v080` workspace:

```sh
python3 -m unittest discover -s scripts/tests -p 'test_package_reference_service.py'
cargo test --locked -p semaprax-native-host --test runtime_host reference_service_acceptance::package_preflight_checks_service_without_runtime_grants -- --exact --test-threads=1
```

The Python fixtures passed 3/3 using synthetic ELF bytes for format,
content-addressing, mode, determinism, and refusal checks; they are not runnable
service evidence. The Rust preflight fixture passed 1/1 against the real service
project, refusing missing projects and fixture-mode intent without runtime grants.
This macOS host has `wasmtime` but no `docker`,
`podman`, `nerdctl`, `containerd`, or `runc` on PATH, and no supplied trusted
static Linux service executable. Installed-development execution and the Linux
container register/login/CRUD/job/restart journey remain open; packaging alone
does not close #336 or the broader scaffold decision gaps above.
