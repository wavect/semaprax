# Public Project Scaffold Capsule v3

Status: implemented bounded profile; **HOSTED GREEN** under the
[v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md). Historical local,
authoring-time, ignored, device/simulator, or separately provisioned evidence
below retains its narrower scope; public promotion, registry publication and
broader product completion remain separately gated.

Audience: people and coding agents scaffolding SEMAPRAX projects, and compiler
contributors.

`semaprax project-scaffold` prepares the calculator, library, or service
template as checked bytes. [Public Project Scaffold Capsule
v2](PROJECT-SCAFFOLD-V2.md) emits the frozen `semaprax.project.v1` manifest
layout and is pinned to those exact bytes. Capsule v3 adds one axis: the
`--layout` flag chooses whether the scaffold's `semaprax.toml` uses that
frozen layout or the extensible `semaprax.manifest.v1` table layout
([Package Manifest v1](PACKAGE-MANIFEST-V1.md)), so a new project can start in
the format the ecosystem tooling reads.

## Command

```text
semaprax project-scaffold --name <name> [--template calculator|library|service] [--layout frozen|tables]
```

`--layout frozen` (the default) emits the v2 capsule, byte-for-byte identical
to what shipped, under schema `semaprax.project-scaffold.v2`. `--layout tables`
emits the v3 capsule under schema `semaprax.project-scaffold.v3`. For the
calculator, v3 also separates `add` into `src/core.spx` and imports it by
stable identity from `src/app.spx`; the library inventory is unchanged. A
`--layout` value other than `frozen` or `tables` exits with status 2 before any
output. The service template only derives under `--layout tables`: its
manifest carries a `[dependencies]` table, which the frozen `semaprax.project.v1`
layout has no room for, so `--template service` under the default `frozen`
layout is refused (`SPX-J115`) before anything is rendered, not silently
downgraded to a dependency-free project.

## Capsule

The v3 capsule is identical to v2 except:

- the descriptor `schema` is `semaprax.project-scaffold.v3`;
- the digest is framed under the domain `semaprax.project-scaffold.digest.v3`,
  so a v2 and a v3 capsule of the same project never share a digest;
- the `semaprax.toml` file is the extensible table layout;
- the calculator inventory adds `src/core.spx`, and `src/app.spx` contains
  `use function @id("<name>.add") from <module>.core as add;`.

The service template shares the calculator's table-layout inventory shape
(`app.spx` importing from a separate `core.spx`, plus `tests.spx`), but its
`core.spx` composes two bundled standard-library dependencies instead of
defining `add` locally: `std.auth` (session lifecycle, password-hash policy
bounds) and `std.jobs` (claim/lease/retry/idempotency state machines), pinned
in `[dependencies]` as `std.auth = "=0.1.0"` and `std.jobs = "=0.1.0"`. Its
`AGENTS.md` carries an extra section naming both. See
[Project Scaffold Service Template v1](PROJECT-SCAFFOLD-SERVICE-V1.md) for the
full template.

For the calculator template the manifest is:

```toml
schema = "semaprax.manifest.v1"

[package]
name = "<name>"
version = "0.1.0"

[modules]
entry = "<module>.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
tests = ["<module>.tests"]

[exports]
web = ["<name>.add"]
```

where `<module>` is `<name>` with `-` replaced by `_`. The library template is
the analogous table manifest over the library inventory. The table manifest
lowers to the same `semaprax.project.v1` contract as the frozen one, so the
capsule's `project_schema` stays `semaprax.project.v1` and the rendered project
passes the same check-and-test validation before the capsule is returned.

The frozen v1 layout carries no version; the table layout requires
`[package] version`, so the scaffold sets `0.1.0`, matching the version the
lock and manifest examples use.

## Replay

`replay_project_scaffold_v1` reads the capsule's `schema`, maps it to the
frozen or table layout, and re-derives that exact layout, requiring byte and
digest equality. A v2 digest cannot validate v3 bytes and vice versa. The
descriptor field set and non-claims are unchanged from v2; v3's `limits.files`
is six, admitting the calculator's added core module and the existing six-file
library inventory.

## Evidence and nonclaims

`tests/project.rs::scaffold::tables_layout_derives_a_v3_capsule_and_replays_only_as_itself`
pins the v3 schema, the exact table manifest bytes for both templates, the
calculator import and added core module, the byte and digest distinction from
the frozen capsule, deterministic derivation, self-replay, and cross-digest
rejection; the unchanged
`derivation_is_literal_ordered_deterministic_and_self_replaying` test is the
byte guard that the frozen default did not move.
`tests/project.rs::scaffold_cli` pins the `--layout tables` CLI output and the
`--layout bogus` rejection.

The capsule is checked bytes only. It owns no filesystem, process, environment,
current-directory, target-emission, or publication authority, and makes no
release or host-support claim.
