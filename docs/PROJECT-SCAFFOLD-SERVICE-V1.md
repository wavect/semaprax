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
small multi-user task-tracking service that composes two bundled
standard-library decision layers instead of defining every function locally.
It exists so a newcomer who wants to see a `[dependencies]`-carrying project
work end to end does not have to hand-write one; `semaprax new --template
service <destination>` and `semaprax project-scaffold --name <name> --template
service --layout tables` both derive it from the same compiled-in bytes as the
other two templates, with no filesystem, network, or process authority.

## Command

```text
semaprax new <destination> [--name project-name] [--template calculator|library|service]
semaprax project-scaffold --name <name> [--template calculator|library|service] [--layout frozen|tables]
```

The service template only derives under the table manifest layout. Its
`semaprax.toml` declares a `[dependencies]` table, and the frozen
`semaprax.project.v1` layout that `--layout frozen` (the default for
`project-scaffold`) emits has no such table to put it in, so
`derive_project_scaffold_v1_with_layout(name, "service", ScaffoldLayout::Frozen)`
returns `SPX-J115` before rendering anything, rather than silently dropping
the dependencies. `semaprax new` is unaffected: both CLI binaries always
derive through `ScaffoldLayout::Tables` regardless of template, so `new
--template service` always succeeds for a valid destination and name.

## Inventory and contents

The service template's six-file inventory is byte-identical in shape to the
calculator's table-layout inventory: `README.md`, `AGENTS.md`,
`semaprax.toml`, `src/app.spx`, `src/core.spx`, `src/tests.spx`. Only the file
*contents* differ:

- `semaprax.toml` uses the `useful-data.v1` profile, declares
  `std.auth = "=0.1.0"` and `std.jobs = "=0.1.0"` under `[dependencies]`, and
  exports `<name>.identifier_is_valid` and `<name>.method_is_rejected` under
  `[exports].web` (`Public Useful Data Export v1` admits no authored aggregate
  in a project that also declares a web export, so the domain record and its
  job are modeled as plain scalar facts, not a `record`).
- `src/core.spx` imports four `std.auth` functions (session lifecycle,
  password-hash policy bounds) and five `std.jobs` functions
  (claim/lease/retry/idempotency state machines) by stable `@id`, plus a
  locally reimplemented identifier and HTTP-method grammar (mirroring, not
  depending on, `std.db.identifier.is_valid` and `std.http.method_is_valid`;
  see `examples/task-service-project/README.md` for why a third dependency
  does not fit).
- `src/app.spx` calls the core module's `run_scenario`, which walks
  register/login/create-or-update/enqueue/complete/query/logout end to end and
  confirms both an unauthorized-access and an invalid-request rejection.
- `src/tests.spx` asserts the same scenario plus four narrower cases: the
  success path, unauthorized rejection, invalid-input rejection, and
  idempotent duplicate enqueue.
- `AGENTS.md` is the same base guide every template ships, with the table
  layout's "Project v1 function boundaries" section (declared aggregates
  cannot cross a scalar-signature function boundary) plus one more section
  naming the two bundled dependencies and their non-claims.

This mirrors `examples/task-service-project/`, generalized with the
`{{name}}`/`{{module}}` substitution every template uses; the reference
example is not itself part of the scaffold's compiled-in bytes.

## Capsule

The service template lowers to the same `semaprax.project.v1` contract and
[Public Project Scaffold Capsule v3](PROJECT-SCAFFOLD-V3.md) descriptor shape
as the other two: schema `semaprax.project-scaffold.v3`, digest domain
`semaprax.project-scaffold.digest.v3`, `limits.files` six, and the rendered
project passes the same in-memory check-and-test validation
(`validate_owned_project_test`) before the capsule is returned to the caller,
using the bundled dependency registry
(`src/project/standard_dependencies.rs`) to resolve `std.auth`/`std.jobs`
purely in memory -- no filesystem or network access, exactly like an ordinary
project naming those packages in `[dependencies]`.

## Compatibility

Both CLI binaries accept `--template service` identically: `semaprax new`'s
standalone create-new route
([standalone project creation v1](NEW-PROJECT-STANDALONE-V1.md)) and the full
toolchain's held-parent staged publication route share one authority path for
the calculator and service templates (both name their sources `app.spx`,
`core.spx`, `tests.spx`), and a separate one for the library template's
different file names.

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

## Nonclaims

Unchanged from [Public Project Scaffold Capsule v3](PROJECT-SCAFFOLD-V3.md):
the capsule is checked bytes only, owning no filesystem, process, environment,
current-directory, target-emission, or publication authority, and it makes no
release or host-support claim. `std.auth` and `std.jobs` are pure decision
layers with no hashing, signing, socket, database, or job-queue host
capability of their own; a real deployment performs all of those outside this
scaffold.
