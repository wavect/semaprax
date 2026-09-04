# Public Project Scaffold Capsule v2

Status: authored implementation with focused local evidence; unpublished and
unpromoted. Required-host and release-artifact gates remain open.

Audience: new SEMAPRAX users, coding agents, tool integrators, and compiler
contributors.

## Purpose

Version 2 of the authority-free scaffold adds one file, `AGENTS.md`, to every
template of [version 1](PROJECT-SCAFFOLD-V1.md): the calculator application
and the library package. A project created by either `new` route or
materialized from the capsule now carries, next to its source, the commands
that check, test, run, format, and build it and the language rules that differ
from other languages. Coding agents read such a
file before their first edit; people benefit from the same page. Everything
else v1 promised is preserved: the artifact owns no path, handle, process, or
publication authority, and materialization remains entirely caller-owned.

## Rust API and CLI

```text
derive_project_scaffold_v1(project_name, template)
replay_project_scaffold_v1(project_name, template, canonical_capsule_bytes, digest)
```

The API names keep their `_v1` suffix; only the schema string and digest domain
are v2, and the returned `ProjectScaffoldV1` and its `ProjectScaffoldFileV1`
entries expose the same facts as before. The standalone `semaprax
project-scaffold --name <name> [--template calculator|library]` command prints
the v2 capsule bytes to stdout exactly as v1 described. The standalone
`semaprax new` writes either template; the full toolchain's `new` writes the
five calculator files.

## Closed capsule

The capsule schema is exactly `semaprax.project-scaffold.v2`. It selects only:

- template `calculator` or `library`;
- Project schema `semaprax.project.v1`;
- one valid canonical project name; and
- the template's exact ordered inventory. The calculator template holds five
  files: `README.md`, `AGENTS.md`, `semaprax.toml`, `src/app.spx`,
  `src/tests.spx`. The library template holds six: `README.md`, `AGENTS.md`,
  `semaprax.toml`, `src/examples.spx`, `src/lib.spx`, `src/tests.spx`.
  `AGENTS.md` is always the second file and is byte-identical across
  templates for one project name.

The digest domain is `semaprax.project-scaffold.digest.v2`; every other
digest, canonicality, and replay rule of v1 applies unchanged, including the
requirement that replay equal a fresh derivation. A v1 capsule does not replay
against v2 and a v2 capsule does not replay against v1; the schema field and
the digest domain both differ.

## File contents

The manifests and source modules of both templates are byte-identical to v1.
Each template's `README.md` now shows the directory-operand forms
(`semaprax check .`) and points the reader at `AGENTS.md`. `AGENTS.md` substitutes the project name and
module name like the other files and states, in this order: what the project
is and where to read the language card (`semaprax help language`); the five
commands; and the rules that differ from other languages, namely the `module`
header and `@id` identities, the tail-expression body rule, `if`/`else` and
`while` shape, contracts and effects, whole-project checking, manifest
registration of new modules, and the diagnostic code and `help:` convention.
It states no rule of its own; every sentence restates a rule the compiler
enforces and the [agent quick reference](AGENT-QUICK-REFERENCE.md) documents.

## Compatibility

The private held-parent authority behind the full toolchain's `new` now
authenticates and publishes a three-file root inventory (`README.md`,
`AGENTS.md`, `semaprax.toml`) plus `src`; its staging, failure precedence,
and no-replace guarantees are unchanged. The standalone `new` of
[standalone project creation v1](NEW-PROJECT-STANDALONE-V1.md) writes the same
five calculator files by default and the six library files for `--template
library`; the full toolchain's `new` still admits only the calculator template.
Project v1 manifest semantics, source semantics, and every other command are
unchanged.

## Evidence

`tests/project/scaffold.rs` pins the v2 schema, digest domain, both ordered
inventories, the calculator's per-file digests, top-level digest, and canonical
byte length, the shared `AGENTS.md` bytes, and the same rejection classes v1
required. `tests/project/scaffold_cli.rs`,
`tests/project/new_cli.rs`, the full toolchain's `cli_new_project_v1`, the
private authority's binding tests, and the quickstart harness pin the five
files at the CLI and publication boundaries.

## Nonclaims

Unchanged from v1: this is not a filesystem API, archive, package manager,
template registry, dependency installer, Git initializer, or publication
mechanism, and it grants no filesystem, process, network, environment, home,
secret, signing, or target-execution authority. `AGENTS.md` is documentation
inside the generated project; the compiler does not read it.
