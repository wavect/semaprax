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
- telemetry target: the decoded endpoint origin under the selected fixed
  JSON-event or OTLP/HTTP route.

## Adapter selection (explicit)

| Requirement (decoded intent) | Bound adapter |
| --- | --- |
| `snapshot` | Durable snapshot store under `--state-dir`. `sqlite` and `postgresql` are refused with stable adapter-specific diagnostics during both configuration and independent request decoding; non-null `dsn_secret_ref` is refused rather than reinterpreted as a state path. |
| `native` + `modern` TLS + listen origin | Loopback HTTP/1.1 on `--port`, plaintext by default. The listen origin is intent only and never itself provisions TLS; `--tls-certificate-secret`/`--tls-private-key-secret` opt in (see below). |
| Three secret refs | Exact files under `--secrets-dir`, resolved before serving. |
| `semaprax-json-events` + endpoint origin | Canonical `semaprax.json-event.v1` HTTPS POST to `<origin>/v1/events` through `deliver_http_durable`. The closed completion envelope carries `schema`, `event: "job.completed"`, `job_id`, `owner`, `desc`, and an HMAC commitment; its `x-semaprax-event-schema` header repeats the schema identifier. An operator may separately select one held private provider root with `--telemetry-root-certificate-secret`; absent that flag, the adapter retains its public-root TLS policy. |
| `semaprax-json-events-v2` + endpoint origin | Opt-in signed-timestamp `semaprax.json-event.v2` HTTPS POST to `<origin>/v2/events`. The retained envelope passes checked webhook admission and export admission before the authenticated-intent durable path. [JSON event v2](REFERENCE-SERVICE-JSON-EVENT-V2.md) owns its exact signature, source facts, and restart contract. |
| `otlp-http-json` + endpoint origin | OTLP/HTTP JSON-Protobuf `ExportLogsServiceRequest` HTTPS POST to `<origin>/v1/logs` through the same durable delivery path. The fixed lower-camel request has one `resourceLogs` row, `service.name = semaprax-reference-service`, one `job.completed` INFO log record, and decimal-string OTLP `intValue` job and owner attributes. It uses `Content-Type: application/json`; it has no webhook schema header or HMAC signature. Only a `200` JSON-Protobuf `ExportLogsServiceResponse` without `partialSuccess` settles delivered; malformed, partial, or other-status responses settle failed and are not redispatched under the completion identity. |

## Invocation mapping

Each exchange first passes the scaffold's own `request_is_admitted`
decision. The checked invocation vocabulary admits `i64`, `u8`, `usize`,
`bool`, and borrowed bytes when the selected closure is effect- and
contract-free. The host invokes the scaffold's request-line, registration
name, row-ownership, session-deadline, immediate-enqueue admission, and
idempotent-enqueue decisions. `PATCH /v1/tasks/<id>` also invokes the checked
`update_is_committed` decision with the host's fixed idle transaction fact
before constructing candidate state. Source denial returns
`403 update_not_admitted`; evaluator failure returns `500 decision_failed`.
`DELETE /v1/tasks/<id>` likewise invokes `delete_is_committed` with its fixed
idle transaction fact before constructing candidate state; source denial
returns `403 delete_not_admitted`, and evaluator failure returns
`500 decision_failed`. These refusals leave the committed state unchanged. The
`POST /v1/tasks` route invokes the checked `create_is_committed` decision with
its fixed idle transaction fact before allocating a task ID or constructing
candidate state. Source denial returns `403 create_not_admitted`; evaluator
failure returns `500 decision_failed`. Both leave committed state unchanged.
The enqueue wrapper compares borrowed descriptor bytes in checked source and
returns the 0/1/2 outcome; the host refuses evaluator errors or out-of-range
outcomes. Before creating a new Pending job, `enqueue_is_legal` receives the
Pending source code and fixed `now=0, next=0` immediate-schedule facts. This
does not sample a clock or admit delayed scheduling. Source denial returns
`403 enqueue_not_admitted`; evaluator failure returns `500 decision_failed`,
before candidate state or outbound work. Existing idempotent replay/conflict
paths retain their prior behavior. Direct selection of the contract-bearing standard-library enqueue
helper still refuses at the public-API seam. Before the completion route
prepares telemetry or constructs a candidate snapshot, it invokes
`mark_job_succeeded` with the reference profile's single successful-attempt
facts (`attempt=0`, `max_attempts=3`). Only source status `4` maps to this
host's `Completed` representation; another source-selected status returns
`403 completion_not_admitted`, and evaluator failure returns
`500 decision_failed`. The route then invokes
`completed_job_metric_is_admitted` over the fixed public `job_state=succeeded`
metric facts before delivery. For its fixed counter `0 + 1` and initially empty
series set, the source wrapper calls the equivalent contract-free
`std.metrics.label-admitted-guarded` predicate. The counter and series-registration
helpers retain their contracts and are exercised by a separate normal-project
oracle; placing those helpers in the public decision closure would refuse every
completion. Source denial returns `403 metric_not_admitted`;
both decision refusals preserve the pending job and avoid an outbound attempt.
For `otlp-http-json`, the route next invokes `structured_log_policy_is_admitted` through its
four-scalar `completed_job_log_is_admitted` source adapter before serializing the completion payload. Its facts come from the OTLP encoder:
source INFO level `2`, the host's INFO threshold `2`, and four named attributes
(the resource's `service.name` plus the record's description, job ID and owner).
The six credential-field flags are false because this fixed envelope includes
none of those held credential fields; caller-supplied description bytes remain
public job data under this profile's existing classification, not content-scanned
secret detection. Source denial returns `403 log_not_admitted`; evaluator
failure returns `500 decision_failed`. Both preserve the pending job, state
snapshot and outbound inventory. The `semaprax-json-events` route does not
emit this OTLP log and does not invoke its structured-log policy.
The `mark_job_succeeded` wrapper is contract-free and explicitly selects the
same successful status as `std.jobs.retry.next_state_after_outcome(0, …)`;
the standard helper remains outside this public seam because its declared
postcondition is not admitted there. The only scaffold decision that remains
fixture-only is `migration_is_admitted`: the accepted `snapshot` profile
supplies no SQL migration history, while every SQL adapter label is refused
before host binding. It is therefore outside this host's claimed route
vocabulary. `method_is_rejected` is covered by the checked
`request_is_admitted` decision on every exchange and by its direct
decision-engine unit assertion; it does not need a second host route
invocation.

### Optional inbound trace metadata

After request-line admission and before route selection, clock sampling,
password work or session/state mutation, a supplied `traceparent` header passes
`trace_context_is_admitted` in the retained checked source. Header names are
case-insensitive; duplicates are refused. The host admits only the exact
55-byte version-`00` framing, splits the actual trace ID, parent ID and flags,
and lets source judge their lowercase-hex widths and nonzero IDs. The source
predicate accepts any two lowercase-hex flag digits; it does not negotiate
sampling or apply the separate typed outbound trace adapter's flag mask.

The source receives `carries_secret=false` because these fields are classified
as public trace metadata, not held credential fields. This is not secret-content
scanning. Malformed framing or source denial returns `400 trace_not_admitted`;
a missing decision or failed evaluation returns `500 decision_failed`. These
refusals precede durable snapshots and outbound intents. The identity is optional
for older projects: requests without this header do not invoke the decision.

Admission creates no span, samples no randomness, and stores, echoes or forwards
no trace headers. `tracestate` remains uninterpreted and unforwarded. Existing
JSON-event v1/v2 and OTLP completion bytes and delivery identities are unchanged.
Migration policy still has fixture coverage only: the snapshot adapter supplies
no SQL migration history. The owning regression selector is
`reference_service::mapping::tests::trace_policy` (source success, precise facts,
denial/evaluator nonmutation, framing, missing identity and std.tracing parity).
The focused local selector passed 5/5; it does not establish trace propagation
or a physical provider result.

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

The opt-in [JSON event v2](REFERENCE-SERVICE-JSON-EVENT-V2.md) route uses the
[authenticated HTTP intent primitive](OUTBOUND-HOST-ADAPTER-V1.md#authenticated-service-http-intent-facts)
to recover a retained signing timestamp and actual body commitment under the
same no-redispatch marker name. It invokes the checked webhook decision before
durable work: `403 webhook_not_admitted` on source denial and
`500 decision_failed` on evaluator or clock failure. Legacy/corrupt markers
supply no prior facts and return `503 delivery_unavailable`. An unchanged
fresh-enough prior intent can complete with `Uncertain`; a stale timestamp or
body conflict preserves Pending without redispatch. V1 bytes remain unchanged.
Older v1/OTLP projects may omit the new source adapter; selecting v2 without
that decision fails before any delivery mutation.

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
  [--tls-certificate-secret <ref> --tls-private-key-secret <ref>] \
  [--telemetry-root-certificate-secret <ref>]
```

The server prints `bundle <digest>` then `ready port=... state=... seq=...
tls=on|off` and serves until stopped. There is no graceful shutdown;
crash-safety comes from the store. All directories must already exist; all
secrets must resolve; the port must be explicit and loopback-only.

## Outbound provider TLS trust (optional, operator-held material only)

Outbound delivery uses the platform public-root TLS policy by default. A
private provider or a local integration peer can instead be selected only by
naming `--telemetry-root-certificate-secret <ref>`. The reference resolves one
bounded DER certificate under `--secrets-dir` with the same
hold/read/recheck discipline as other operator-held material, then constructs
a client that trusts exactly that root. An invalid or absent root refuses
startup before the listener binds; the decoded endpoint origin cannot select,
replace, or suppress certificate verification. This option grants no client
credential, proxy, redirect, retry, public deployment, or provider authority.

The issue #329 local real-provider selectors are
`completion_delivers_to_local_tls_provider_and_restart_does_not_duplicate`,
`completion_provider_refusal_persists_across_restart_without_duplicate`, and
`completion_provider_close_after_request_is_uncertain_across_restart_without_duplicate`.
Each starts an independent loopback TLS provider process using the production
provider abstraction, has that process persist every received method, route,
idempotency key, and body, then restarts the actual service from the same state
digest. The provider returns success, an explicit non-2xx refusal, or persists
the request and closes before sending a response. The provider-side receipt
must retain count one and the fixed `/v1/events` completion identity; the
restarted service preserves the corresponding `delivered`, `failed`, or
`uncertain` settlement. This is local test-provider evidence only: it makes no
external-provider, hosted, credential, public-service, or power-loss claim.

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

No SQL adapter label is accepted, no hosted/public/production support, no
graceful shutdown, and no per-request decisions outside the
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
a session-clock dependency. Unknown routes likewise stop before the protected
route boundary, even when they carry a valid bearer. An unavailable clock
preserves committed state
and refuses login with `clock_unavailable`, or authenticated work with
`decision_failed`.

The two login deadlines stay fixed: access does not refresh the idle deadline.
Equality expires a session, absolute expiry wins when both deadlines have
passed, and a persisted terminal state cannot become active even if a later
host observation is earlier. This is not a general wall-clock rollback
protection claim for a still-active session.

The authored `reference_service::mapping::tests::session_policy` corpus covers
just-before/equal/after boundaries, reloading exact expiry snapshot bytes,
terminal replay, unavailable clocks, unknown-route clock refusal, every
protected route's expiry refusal before route mutation, and parity between
actual `std.auth` transition execution and both reference/generated scaffold wrappers. The
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
exact-commit, complete service-suite, full-profile, hosted, or production
receipt. The separate unit assertion establishes absolute-expiry precedence;
the coincident-deadline process result is not isolated absolute-only expiry
evidence. The separately recorded installed-development and OCI execution
receipts appear below.

Focused verification uses the existing harnesses:

```sh
cargo test --locked -p semaprax-native-host --lib reference_service::mapping::tests::session_policy -- --test-threads=1
cargo test --locked -p semaprax-native-host --test runtime_host reference_service_acceptance -- --test-threads=1
```

The repository full profile and generated-scaffold preservation gates remain
required. Within the accepted snapshot host profile, every claimed route uses
its documented checked decision vocabulary; migration remains fixture-only for
the explicitly refused SQL profiles, and method rejection is included through
checked request admission. These five passes establish the session-policy
slice; the later sections record the separate installed-development and OCI
runtime receipts. They do not establish broader reference-application,
deployment, release, or production support.

## Offline native service packaging (#336)

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
below; `Cmd` supplies the default `--port 8080`. Working directory is `/service`
and default user is `65532:65532`. Empty `/state`, `/outbound`,
`/bundle`, and `/secrets` mountpoints carry no data or authority. Writable
state/outbound/bundle mounts must permit the selected UID; secrets should be
mounted read-only. The image has no shell, loader, downloaded base, registry
operation, credential embedding, signing, or publication step.

After explicitly importing the layout into a trusted Linux runtime, the
configured entrypoint already supplies `serve --project /service --config /service/service.config.json
--state-dir /state --outbound-dir /outbound --bundle-dir /bundle
--secrets-dir /secrets`; its `Cmd` supplies `--port 8080`. The operator must
provide the four explicit mounts described above; without them startup refuses
missing secrets or unwritable state. Runtime arguments replace `Cmd`, so append
`--port <port>` and, for restart, `--state <digest>` together. TLS and
session-policy flags can likewise be supplied explicitly. Loopback serving requires clients
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

`reference_service_acceptance::packaged_development_service_runs_from_an_independent_workspace`
adds a separate local installed-development gate. It passes the harness's
actual locally built `semaprax-reference-service` binary as both the explicit
trusted checker and the exact executable whose digest the packager copies. The
Python journey starts only the resulting `package/bin/semaprax-reference-service`
from a fresh temporary workspace, with `package/service` as its project. It
checks missing-secret and unsupported-adapter refusal before runtime state is
created, then register/login/task CRUD/job completion/restart with no duplicate
outbound delivery. The focused local journey passed 1/1 on `wavect/v080`.
It does not establish the binary's provenance, release
status, OCI execution, or publication.

```sh
cargo test --locked -p semaprax-native-host --test runtime_host \
  reference_service_acceptance::packaged_development_service_runs_from_an_independent_workspace \
  -- --exact --test-threads=1
```

### OCI runtime journey

`scripts/tests/reference_service_oci_runtime.py` is the opt-in Linux/Podman
runtime gate. It packages the explicitly supplied static Linux service binary,
archives and imports that exact OCI layout under the deterministic local name
`semaprax-reference-service:local`, and starts only that imported image. It
uses `--network host` because the service deliberately listens on loopback, and
mounts separate host-owned `state`, `outbound`, `secrets`, and `bundle`
directories as its physical adapters. It never mounts the source project.

The imported image must refuse an unsupported database adapter and missing
secrets without writing state, outbound, or bundle inventory. After the three
named secrets are supplied, a pre-existing mismatched run-bundle input must
also refuse without changing state, outbound, or that input. The gate then
proves register/login, task create/update/delete, enqueue, an actual outbound
adapter failure recorded as `uncertain`, and restart from the saved digest with
no duplicate delivery. The image is removed only if the fixed local name did
not exist before the gate; a pre-existing image with that name is refused.

`.github/workflows/reference-service-oci-runtime.yml` is a separate
Ubuntu 24.04 route, scoped to relevant pushes on `wavect/v080` and manual
dispatch after the workflow reaches the default branch. It fetches the locked dependency closure,
builds the current checkout's `x86_64-unknown-linux-musl` service binary,
copies it to a single-link private artifact, records the exact checkout/tree,
selected service/package inputs (including the canonical host-mode
`scripts/tests/reference-service-host.config.json`), and artifact SHA-256,
then calls the same Python OCI journey directly. It installs only `musl-tools`
and Podman, uses no registry credentials or image pulls, and uploads the
bounded identity record whether the journey passes or fails. A workflow
definition alone is not runtime evidence. The exact branch checkout
`c3d5dead84d6d3903410f0b3478d65e6fe6e1b6c` passed the hosted
[OCI runtime run 36922911236](https://github.com/wavect/semaprax/actions/runs/36922911236):
the selected static `ET_EXEC` artifact had SHA-256
`8a0e679a804a48be1466ee74069c306f36d6fc4cb313083be699df0c24fcc015`,
the imported image completed the refusal and physical-adapter journey, and
the run retained [its bound evidence artifact](https://github.com/wavect/semaprax/actions/runs/36922911236/artifacts/11193291279).

```sh
SEMAPRAX_REFERENCE_SERVICE_OCI_EXECUTABLE=/absolute/static-linux/semaprax-reference-service \
cargo test --locked -p semaprax-native-host --test runtime_host \
  reference_service_acceptance::packaged_oci_service_runs_with_physical_adapters \
  -- --ignored --exact --test-threads=1
```

The gate requires an operator-supplied trusted static Linux service executable,
Linux, and a local Podman runtime. It performs no registry access, publication,
signing, or release-provenance verification. This macOS host has `wasmtime` but
no `docker`, `podman`, `nerdctl`, `containerd`, or `runc` on PATH, and no
supplied trusted static Linux service executable, so this selector is prepared
but has not been run on this macOS host. The hosted run establishes only its
selected Linux/Podman profile; it supplies no release provenance, public
deployment, SQL adapter, or full scaffold decision closure.

### Immediate enqueue checked-source parity

The immediate enqueue seam selects the existing checked scaffold
`enqueue_is_legal` decision instead of assuming that every new Pending candidate
is admissible. This uses the already admitted scalar vocabulary and changes no
language, snapshot schema, adapter, or standard-library contract. The host still
supports immediate Pending jobs only. The follow-on completion seam makes
`mark_job_succeeded` contract-free while preserving its successful `4` result,
then binds that result before durable delivery or the `Completed` snapshot
mapping. Migration, trace and webhook policy seams remain as described above.

The new `mapping::tests::enqueue_policy` fixture compares 15 state/due-boundary
rows across actual `std.jobs` calls, the reference DecisionEngine and a freshly
generated service scaffold. A separate alternate checked-source fixture denies
new enqueues and asserts `403 enqueue_not_admitted` with unchanged committed
bytes, digest and outbound inventory; a bounded-fuel refusal asserts
`500 decision_failed` with the same nonmutation requirement.

`job_enqueue_is_idempotent_and_completion_settles_once` covers success, replay
and completion. On this workspace revision, the new selector passed 2/2 and
the existing enqueue/completion regression passed 1/1.

`mapping::tests::completion_policy` compares the successful status across the
reference DecisionEngine and a freshly generated scaffold for representative
attempt facts. Its alternate source returns a different terminal code and
asserts `403 completion_not_admitted` with unchanged committed bytes, digest,
and outbound inventory; a bounded-fuel source loop asserts `500 decision_failed`
at the same pre-delivery boundary.

```sh
cargo test --locked -p semaprax-native-host --lib reference_service::mapping::tests::enqueue_policy -- --test-threads=1
cargo test --locked -p semaprax-native-host --lib reference_service::mapping::tests::job_enqueue_is_idempotent_and_completion_settles_once -- --exact --test-threads=1
cargo test --locked -p semaprax-native-host --lib reference_service::mapping::tests::completion_policy -- --test-threads=1
```

### Checked OTLP structured-log policy (#336)

The source wrapper keeps the guarded `level >= threshold` comparison equivalent
to `std.log.level-enabled`, whose precondition remains outside the frozen public
invocation seam. The unchanged `std.log.redact` predicates still enforce the
32-field budget and all six caller-classified secret flags. The four-scalar
adapter decodes bits 0 through 5 for password, API key, bearer token, session
token, webhook signing secret and SMTP credential; higher bits refuse before
calling the policy. The existing eight-parameter public invocation limit stays
unchanged, and the nine-argument policy is an internal source call. The reference core
source also supplies the generated scaffold, so both projections carry the same
wrapper.

`mapping::tests::log_policy` adds an independent normal-project oracle calling
contract-bearing `std.log.level-enabled` and the redaction predicates, compared
with both reference and generated decision engines across level/threshold,
field-budget and each secret-flag boundary. Unknown-bit cases additionally
require refusal before an intentionally expensive policy can execute. An alternate checked source accepts
only the actual INFO/four-attribute facts; another denies or exhausts only the
log decision and requires unchanged committed bytes, digest, snapshot files and
outbound inventory. The delivery protocol fixture ties the policy field count
and level to the emitted OTLP record and resource attributes.

The focused local selectors passed: `log_policy` 5/5 and the delivery protocol
fixture 1/1. The selectors are:

```sh
cargo test --locked -p semaprax-native-host --lib reference_service::mapping::tests::log_policy -- --test-threads=1
cargo test --locked -p semaprax-native-host --lib reference_service::delivery::tests::otlp_http_json_logs_are_protocol_bound_and_not_a_webhook_alias -- --exact --test-threads=1
```

The completion-policy module also compares the fixed metric wrapper with actual
contract-bearing `std.metrics.counter-increment` and
`std.metrics.try-admit-labeled-series-guarded` execution for valid, secret,
invalid-name, unsafe-value and byte-length-boundary cases. The log-policy module
requires real completion with the unmodified reference decisions under both
telemetry adapters, preserving coverage of the earlier metric admission gate.
The metric oracle passed 1/1. The local TLS-provider acceptance also passed
6/6, including three JSON-event cases and OTLP full-success, partial-response,
and malformed-response cases. Each provider process persisted exactly one
received request across the service restart; the OTLP cases use `/v1/logs`
and retain failed settlement for partial or malformed responses. These are
local provider and file-only store observations, not a power-loss claim.

```sh
cargo test --locked -p semaprax-native-host --lib reference_service::mapping::tests::completion_policy::completion_metric_matches_std_metrics_and_generated_scaffold -- --exact --test-threads=1
```
