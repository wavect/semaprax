# Project Scaffold Service Template v1

Status: implemented bounded profile; local evidence only (`tests/project.rs`,
`tests/project/new_cli.rs`, and the full toolchain's `cli_new_project_v1`).
No hosted or release-archive run has exercised it yet; the calculator and
library templates' HOSTED GREEN evidence in
[Public Project Scaffold Capsule v3](PROJECT-SCAFFOLD-V3.md) does not extend
to this template.

Audience: new SEMAPRAX users and coding agents who want a scaffolded starting
point that already composes a bundled standard-library dependency, and
compiler contributors maintaining the scaffold's frozen inventory/digest
contract.

## Purpose

`--template service` adds a third built-in [Public Project Scaffold Capsule
v3](PROJECT-SCAFFOLD-V3.md) template beside `calculator` and `library`: a
small multi-user task-tracking service that composes ten bundled
standard-library decision layers instead of defining every function locally.
Newcomers can try an end-to-end project with `[dependencies]` without writing
one first. `semaprax new --template service <destination>` and
`semaprax project-scaffold --name <name> --template service --layout tables`
derive it from the same compiled-in bytes as the other templates. Derivation
grants no filesystem, network, or process authority.

## Command

```text
semaprax new <destination> [--name project-name] [--template calculator|library|service]
semaprax project-scaffold --name <name> [--template calculator|library|service] [--layout frozen|tables]
```

The service template requires the table manifest layout because it declares
`[dependencies]`. The frozen `semaprax.project.v1` layout emitted by
`--layout frozen` (the `project-scaffold` default) has no dependency table, so
`derive_project_scaffold_v1_with_layout(name, "service", ScaffoldLayout::Frozen)`
returns `SPX-J115` before rendering anything, rather than silently dropping
the dependencies. `semaprax new` is unaffected: both CLI binaries always
derive through `ScaffoldLayout::Tables` regardless of template, so `new
--template service` always succeeds for a valid destination and name.

## Inventory and contents

The service template's nine-file inventory contains `README.md`, `AGENTS.md`,
`semaprax.toml`, `src/app.spx`, `src/core.spx`, `src/tests.spx`,
`service-config.schema.json`, `service.config.json`, and
`service-host-adapter-request.json`. The six ordinary project files retain the
calculator table layout's shape; the three additional files make the service's
host configuration boundary explicit:

- `semaprax.toml` uses the `useful-data.v1` profile and declares the bundled
  `std.auth`, `std.db`, `std.http`, `std.jobs`, `std.log`, `std.log.redact`,
  `std.metrics`, `std.export.policy`, `std.tracing`, and `std.webhook` decision
  packages under `[dependencies]`, and
  exports `<module>.core.identifier_is_valid` and
  `<module>.core.method_is_rejected` under
  `[exports].web` (`Public Useful Data Export v1` admits no authored aggregate
  in a project that also declares a web export, so the domain record and its
  job are modeled as plain scalar facts, not a `record`).
- `src/core.spx` composes auth/session, database/migration/transaction,
  request-line, durable-job/idempotency, bounded redacted-log, metric/export,
  trace-context, and webhook-admission predicates by stable `@id`. Its webhook step combines the
  existing exact-descriptor idempotency outcome with a bounded signature
  envelope, symmetric replay window, attempt count, and caller-classified
  secret guard; it is a decision before any signing, queueing, retry
  scheduling, or transport.
- `src/app.spx` calls the core module's `run_scenario`, which walks
  register/login/create-or-update/enqueue/complete/query/logout end to end and
  confirms both an unauthorized-access and an invalid-request rejection.
- `src/tests.spx` asserts the same scenario plus focused success, refusal,
  idempotency, redaction, tracing, metric, export, and webhook cases.
- `AGENTS.md` is the same base guide every template ships, with the table
  layout's "Project v1 function boundaries" section (declared aggregates
  cannot cross a scalar-signature function boundary) plus one more section
  naming all ten bundled dependencies and their non-claims.
- `service-config.schema.json` is a closed Draft 2020-12 schema for fixture,
  SQLite/PostgreSQL, native HTTP/TLS, OTLP, and host-owned secret-reference
  selections. Nested database, HTTP, telemetry, and secret objects all refuse
  unknown fields and retain finite string bounds.
- `service.config.json` selects only credential-free fixture adapters, null
  endpoints, and null secret references. It carries no DSN, password, token,
  signing material, environment-variable name, or authority to resolve one.
  `src/project/scaffold/service_config.rs` independently decodes this v1 wire
  under a 16 KiB pre-parse bound, exact closed objects, canonical sorted JSON,
  bounded reference/origin grammar, and paired mode rules: fixture mode admits
  only null/fixture selections, while host mode requires SQLite/PostgreSQL,
  native modern-TLS HTTP, OTLP, HTTPS origins, and nonempty host-owned secret
  references. JSON Schema guidance is therefore not the compiler's sole check.
- `service-host-adapter-request.json` is the compiler-rendered canonical
  handoff for the fixture configuration. It is bounded to 16 KiB and declares
  no capabilities. From a valid host configuration the same decoder renders a
  separate request naming exactly four capabilities — database connect,
  native TLS serve, host-secret resolve, and telemetry emit — plus only
  bounded origins and secret references. A separate closed request-v1 decoder
  replays those bytes for host consumption and retains the telemetry origin as
  an intent only. It cannot construct an outbound policy or capability: the
  host must separately grant one whose exact allowed-origin set contains that
  target before it can bind an adapter. This is an intent declaration, not a
  capability grant or a physical adapter implementation.

This mirrors `examples/task-service-project/`, generalized with the
`{{name}}`/`{{module}}` substitution every template uses; the reference
example is not itself part of the scaffold's compiled-in bytes.

## Capsule

The service template lowers to the same `semaprax.project.v1` contract and
[Public Project Scaffold Capsule v3](PROJECT-SCAFFOLD-V3.md) descriptor shape
as the other two: schema `semaprax.project-scaffold.v3`, digest domain
`semaprax.project-scaffold.digest.v3`, `limits.files` nine, and the rendered
project passes the same in-memory check-and-test validation
(`validate_owned_project_test`) before the capsule is returned to the caller,
using the bundled dependency registry
(`src/project/standard_dependencies.rs`) to resolve all ten decision
packages purely in memory -- no filesystem or network access, exactly like an
ordinary project naming those packages in `[dependencies]`.

## Compatibility

Both CLI binaries accept `--template service` identically: `semaprax new`'s
standalone create-new route
([standalone project creation v1](NEW-PROJECT-STANDALONE-V1.md)) and the full
toolchain's held-parent staged publication route write the same nine checked
files. The held authority gives service its own exact root inventory for the
three configuration files; calculator shares only its source-file names.

## Evidence

`tests/project/scaffold.rs::service_template_composes_bundled_dependencies_and_only_derives_under_tables_layout`
pins the frozen-layout refusal, the exact inventory, the manifest's
`[dependencies]` table and web exports, the `AGENTS.md` additions, deterministic
derivation, self-replay, and cross-template replay rejection.
`tests/project/scaffold_cli.rs` and `tests/project/new_cli.rs` pin the CLI
output and the full end-to-end `check`/`test`/`run`/`fmt --check` loop for a
created project. The full toolchain's
`crates/semaprax-toolchain/tests/cli_new_project_v1.rs::service_template_has_exact_bytes_and_passes_the_developer_loop`
pins the same loop through the held-parent authority and is the regression
test for a prior defect where that authority's `new_project::run` collapsed
every non-library template to the calculator after `parse` had already
accepted `--template service`.
`tests/project/scaffold.rs::service_scaffold_configuration_is_closed_and_credential_free`
pins the nested closed-schema rules, exact database adapter vocabulary, fixture
selection, empty fixture capability request, and absence of endpoints and
secret values. Descriptor replay binds all three configuration/adapter files
byte-for-byte with the other generated assets.
The decoder's own hostile corpus rejects unknown members, mode/adapter drift,
credential-shaped DSNs, insecure origins, noncanonical encoding, and max-plus-
one input before the fixture can enter scaffold derivation.
The independent host-request decoder separately rejects unknown, duplicate,
reordered-capability, noncanonical, and max-plus-one request bytes. Its
private-root loopback integration starts from a checked host configuration,
then proves that only a separately host-granted outbound policy matching the
decoded OTLP origin can bind the fixed telemetry route. Neither the scaffold
fixture (which has no requirements) nor request replay grants network I/O.

## Nonclaims

Unchanged from [Public Project Scaffold Capsule v3](PROJECT-SCAFFOLD-V3.md):
the capsule is checked bytes only, owning no filesystem, process, environment,
current-directory, target-emission, or publication authority, and it makes no
release or host-support claim. The bundled dependencies are pure decision
layers with no hashing, signing, socket, database, job-queue,
retry-scheduling, telemetry-emission, or webhook-delivery host capability of
their own; a real deployment performs all of those outside this scaffold.
