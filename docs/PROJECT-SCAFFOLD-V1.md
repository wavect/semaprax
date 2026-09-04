# Public Project Scaffold Capsule v1

Status: superseded by [Public Project Scaffold Capsule v2](PROJECT-SCAFFOLD-V2.md),
which adds `AGENTS.md` and renders schema `semaprax.project-scaffold.v2`; this
document records the v1 contract. Unpublished and unpromoted.

Audience: new SEMAPRAX users, tool integrators, and compiler contributors.

## Purpose

The Public Project Scaffold Capsule v1 is an authority-free, replayable form of
the built-in calculator Project. It lets a caller obtain the exact four files
needed to create a canonical Project v1 without granting the compiler a
filesystem location or publication authority.

The capsule is not the private `new` workflow. `new` retains its held-parent,
create-new staging, authentication, and no-replace publication contract. A
capsule consumer chooses whether, where, and how to materialize files and must
provide its own safe publication policy.

## Rust API and CLI

The library exposes two authority-free operations:

```text
derive_project_scaffold_v1(project_name, template)
replay_project_scaffold_v1(project_name, template, canonical_capsule_bytes, digest)
```

The returned artifact exposes its schema, template, Project schema, project
name, ordered files, canonical bytes, and digest. Each file exposes only its
relative path, exact bytes, and SHA-256 fact.

The standalone public CLI exposes the same derivation as one stdout-only
command. Success writes exactly the artifact's canonical bytes to stdout and
writes nothing to stderr. Invocation and derivation failures write no capsule
bytes. The command does not create a directory, write a file, inspect a
destination, or call the private `new` hook.

## Closed capsule

The capsule schema is exactly `semaprax.project-scaffold.v1`. It selects only:

- template `calculator` or, additively, `library`;
- Project schema `semaprax.project.v1`;
- one valid canonical project name; and
- the template's exact ordered inventory. The calculator template holds four
  files:
  1. `README.md`;
  2. `semaprax.toml`;
  3. `src/app.spx`;
  4. `src/tests.spx`.

  The library template holds five, in the standard-library package shape of
  [Standard Library v1](STANDARD-LIBRARY-V1.md#library-architecture): a
  library module whose one function carries `requires` and `ensures`
  contracts, an examples module as the entry, and a conformance test module:
  1. `README.md`;
  2. `semaprax.toml`;
  3. `src/examples.spx`;
  4. `src/lib.spx`;
  5. `src/tests.spx`.

The `template` field of the descriptor names the selected template, and replay
is bound to the caller's expected template: a library capsule does not replay
as a calculator, nor the reverse.

Every file entry binds its literal relative path, exact bytes, and lowercase
SHA-256. The top-level digest binds a scaffold-specific domain, a checked `u64`
length, and the canonical document rendered with its `digest` field omitted;
the digest cannot recursively hash a final document containing itself. Replay
then requires exact equality with the fully rederived canonical bytes. File
hashes do not replace complete canonical replay.

The file bytes are the existing calculator template bytes. Project-name and
module substitutions retain the private generator's exact spelling, including
hyphen-to-underscore module conversion. The manifest remains canonical Project
v1 and the sources retain their existing canonical SEMAPRAX formatting.

## Independent replay

Replay is explicitly bound to the caller's expected project name, template,
and submitted digest, then closed and byte-exact. It rejects malformed UTF-8 or JSON, duplicate,
missing, reordered, or unknown fields, a wrong schema/template/Project schema,
an invalid project name, a wrong or reordered inventory, unsafe paths, changed
file bytes or hashes, noncanonical escaping or whitespace, and any capsule that
does not equal a fresh derivation for its expected project name and template.

`SPX-J115` reports invalid names and any semantic, closed-root, framing,
parsing, inventory, hash, canonicality, or replay disagreement. `SPX-J116` is
reserved for exact byte/capacity overflow. Diagnostics are bounded and must not
echo attacker-controlled capsule contents.

Successful replay returns the same closed artifact. It grants no permission to
write the files and does not treat the supplied hashes as authority.

## Compatibility and preservation

The capsule is additive. It does not change:

- Project v1 manifest or source bytes;
- the private `semaprax-full new` grammar, template inventory, staged
  publication, failure precedence, or successful output bytes;
- standalone Project check, test, run, graph, or build behavior; or
- CLI help for any existing command except the additive public scaffold entry.

Private `new` and public scaffold derivation must converge on the same four
calculator template files for the same name. That equality is evidence about
bytes, not a shared publication capability. The library template is available
only through the public stdout capsule: the private `new` staging authority
admits exactly the four calculator files, so `semaprax new --template library`
is rejected until that authority is widened.

## Required evidence

Focused evidence must prove:

- literal schema/template/Project-schema facts and exact ordered inventory;
- deterministic derivation, distinct project-name substitution, per-file
  hashes, top-level digest, and derive/replay equality;
- representative rejection from each closed-root, ordering, path, byte, hash,
  name, framing, canonicality, and capacity mutation class with `SPX-J115` or
  `SPX-J116`, including a self-consistent reminted capsule;
- exact public CLI stdout, empty stderr, malformed invocation behavior, and an
  unchanged empty working directory;
- equality with private `new` output while preserving its prior byte known
  answers and filesystem/publication tests; and
- a capsule-derived in-memory Project that passes ordinary check/test meaning,
  without treating a test-owned temporary materialization as product
  publication authority.

Focused library, CLI, private-byte-preservation, and quickstart cases passed
locally. Required-host, release-artifact, and hosted gates remain open.

## Nonclaims

This is not a filesystem API, archive, package manager, template registry,
remote template protocol, dependency installer, Git initializer, managed
workspace transaction, atomic publication mechanism, or crash-recovery format.
It grants no filesystem, process, network, environment, home, secret, signing,
or target-execution authority. It does not make the private full toolchain or
its `new` command public.
