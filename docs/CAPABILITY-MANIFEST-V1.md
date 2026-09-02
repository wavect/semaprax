# Build Capability Manifest v1

Status: versioned bounded reference; the completion matrix owns product status.

Audience: integration tool authors and compiler contributors.

`semaprax capability-manifest <file.spx>` is a deterministic, read-only
projection that declares the EXACT build capabilities one verified module
requires. It is the first executable slice of the completion-matrix row
"Sandboxed builds and dependencies": before any sandbox can be enforced,
builds need a machine-checkable statement of what to enforce against. The
command imports no package, resolves no dependency, writes no lockfile,
contacts no registry, enforces nothing at build time, executes nothing, and
changes no source.

## Command

```sh
semaprax capability-manifest <file> [--max-bytes N]
```

- `--max-bytes` (default 64 KiB, bounds follow the Agent Context byte limits)
  bounds the whole envelope. Overflow fails closed with `SPX-K203`; output is
  never truncated or repaired. Invalid options fail with `SPX-K201`.
- The default output is one canonical compact JSON envelope plus one trailing
  newline. The module must verify cleanly first; any verifier diagnostic fails
  the whole command closed.

## Closed capability vocabulary

The manifest admits exactly five ambient-authority domains, in canonical
bytewise order: `filesystem`, `home`, `network`, `process`, `secrets`. A
capability token names a domain when it equals the domain (`process`) or
starts with `<domain>.` (`network.read`). Every capability token anywhere in
the module — module permits, interface permits, declared function effects,
and declared interface-import effects — must sit inside this closed
vocabulary. Any token outside it (for example `audit.write`) aborts the whole
command with the dedicated diagnostic `SPX-K202`; a manifest is never emitted
for a partially understood module.

Admission mirrors the established scalar projection profile in spirit but not
in shape: signatures are irrelevant here because the manifest speaks only
about capabilities, so generic functions, aggregates, resources, and imports
are all projected without exclusion. There are no partial manifests.

## Manifest content

The payload inventories, each deterministic and bytewise sorted:

- `module_permits`: the deduplicated module-level `permit { ... }`
  inventory;
- `functions`: every function declaration ordered by stable identity, each
  with its declared effect set (`uses { ... }`) deduplicated and sorted — the
  same facts the verifier already checked for permit containment and call-edge
  propagation; they are reused verbatim, never recomputed;
- `imports`: every interface-import declaration ordered by stable identity,
  each with its interface name and declared effect set. Import effects are
  required host capabilities, so omitting them would make the manifest
  inexact;
- interface permits that no import consumes are still fail-closed-checked
  against the vocabulary but do not appear as separate entries and do not by
  themselves mark an ambient domain as declared.

The `ambient_authority` object asserts, for each of the five domains in fixed
order, `"declared"` when any required inventory token names that domain and
`"none"` otherwise. A completely effect-free module therefore emits the
explicit empty-by-default assertion with all five domains set to `"none"`:
absence of evidence becomes explicit evidence of absence.

## Envelope and verification

`capability_manifest::generate` returns canonical compact JSON with fixed key
order:

- outer wrapper `{"schema","digest","bytes","payload"}` where `digest` is the
  domain-separated SHA-256 of the exact payload bytes
  (`semaprax.capability-manifest.payload.v1`) and `bytes` is their length;
- payload members in order: `schema`, `source` (`path`, graph `revision`,
  domain-separated source digest under `semaprax.capability-manifest.source.v1`),
  `limits`, `module` accounting (`name`, `permits_total`, `functions_total`,
  `imports_total`), `module_permits`, `functions`, `imports`,
  `ambient_authority`, and fixed `nonclaims`.

`capability_manifest::verify_envelope` independently replays one envelope: it
recomputes the outer payload digest over the exact serialized payload bytes,
re-checks the declared byte count, re-checks the closed vocabulary over every
listed token, and re-derives the ambient authority section from the listed
inventories, failing with `SPX-K204` on any mismatch. Any mutation anywhere in
the envelope invalidates verification, and even a consistently re-minted
digest cannot smuggle in an undeclared capability or an out-of-vocabulary
token without tripping the derivation replay.
`capability_manifest::verify_envelope_against_source` additionally rebinds the
current source bytes to the embedded source digest and fails closed on drift.

Source bytes are snapshotted before parsing and re-checked after rendering;
drift during generation fails the whole command closed. All diagnostics use
the previously unused `SPX-K2xx` family: `SPX-K201` options, `SPX-K202`
closed-vocabulary violation, `SPX-K203` budget exhaustion, `SPX-K204`
envelope consistency or replay failure.

## Honest boundary

This tranche is a read-only declaration, not enforcement. It performs no
sandbox enforcement at build time, provides no network/home/secrets/
filesystem/process enforcement machinery, resolves no dependencies, writes no
lockfile, implements no resolver or version negotiation, hosts no package
registry, executes no target, and claims no hostile-package dynamic behavior
beyond the static fail-closed checks above. Project Manifest v1 continues to
have no resolver, lockfile, dependency graph, or registry. Enforcement
against a declared manifest remains future work.

## Evidence

Executable evidence lives in `tests/projections/capability_manifest.rs` plus module
tests in `src/capability_manifest.rs`: a pinned golden envelope SHA-256 KAT
over `examples/meaning.spx` whose effect-free program must assert all-five-
`"none"` ambient authority, a declared-effects module whose manifest lists
exactly the declared permits/effects and flips exactly those ambient domains,
byte-identical double runs (library and CLI), undeclared-capability injection
rejected both by digest authentication and by the derivation replay after a
consistent re-mint, out-of-vocabulary tokens rejected at generation
(`SPX-K202`) and at replay (`SPX-K204`), tampered-digest and foreign-schema
rejection, source-drift detection through the embedded source binding, budget
exhaustion, and CLI exit-code contracts. No sandbox is enforced and no target
execution is claimed.
