# Package Manifest v2: explicit native law sources

Status: bounded LAW-02 table layout. Audience: Project authors and tool authors.
This document extends [Package Manifest v1](PACKAGE-MANIFEST-V1.md). Its table
schema is `semaprax.manifest.v2`; `[package] profile` still selects the same
Project contract, including `semaprax.project.v1` for the scalar profile.
`semaprax.project.v20` remains the public generic Wasm provider profile and is
not a whole-file manifest version.

## Exact layout

The v2 layout has every v1 table and key with the same order and validation,
except `[modules]` requires `law_sources` between `sources` and `tests`:

```toml
schema = "semaprax.manifest.v2"

[package]
name = "calculator"
version = "0.1.0"

[modules]
entry = "calculator.app"
sources = ["src/app.spx", "src/contracts.spx", "src/core.spx", "src/tests.spx"]
law_sources = ["src/contracts.spx"]
tests = ["calculator.tests"]

[exports]
web = ["calculator.add", "calculator.divide", "calculator.is-negative", "calculator.multiply", "calculator.not", "calculator.subtract"]
```

`law_sources` is a strictly byte-sorted, duplicate-free subset of `sources`.
An empty list is allowed. At least two ordinary source files remain required.
All paths obey the ordinary canonical relative `.spx` source path limits.
The exact final filename `LAWS.spx` is an additional portable spelling only
when that path is explicitly selected in `law_sources`; its directory segments
remain lowercase and canonical. A simultaneous lowercase `laws.spx` path in
the same inventory is refused as a case alias. V1 manifests keep their
lowercase-only path rule.
The v1 layout does not admit this key and rejects it with `SPX-J120`. V2
requires the key even when empty. Both layouts retain exact canonical table
order, one-line arrays, blank lines, and one final LF.

Every selected law source is read through the Project's ordinary exact file
inventory. The filename `LAWS.spx` has no discovery authority: any selected
path may be a law module, and an unselected law file has no law meaning.
A selected missing file, a law declaration hidden in an ordinary source, or a
law path absent from `sources` fails admission. The native declaration grammar
and limits are in [Native Law Declarations v1](NATIVE-LAW-DECLARATIONS-V1.md).

## Revision and replay

Law source facts retain the exact authored bytes and source digest. Their
semantic source revision is computed from canonical native-law source text;
comments do not change the proposition. The Project Workspace manifest and
revision include law source facts alongside ordinary source facts in path
order. `semaprax.manifest.v2` Project revisions use the
`semaprax.project-revision.v2\0` SHA-256 domain over length-framed canonical
manifest bytes and Workspace revision. Earlier layouts keep the v1 domain and
their exact revision bytes. Revision-store replay selects the formula from the
independently parsed manifest schema and rechecks the complete retained source
inventory. ProgramRoot's source projection binds law bytes; its semantic
projection binds their canonical propositions.

Native law declarations remain specification data. A selected law has no
runtime execution effect or ambient proof, filesystem, process, publication,
or signing authority. The caller must independently select any protected
baseline, as specified by [Law Set v1](LAW-SET-V1.md).
