# Documentation Projection v1

Status: authored with local executable evidence; unpublished and unpromoted.
The completion matrix and release evidence own product status.

Audience: SEMAPRAX users, coding agents, documentation tooling authors, and
compiler contributors.

## Purpose

Documentation that is written by hand drifts from the compiler that ships.
`semaprax doc` removes the hand: it renders the documentation of one module
from the checked program, carrying the same declaration identities, signatures,
ownership modes, effects, and contracts that `semaprax graph` emits, and the
same graph revision. A reader, an agent, or an editor can therefore match a
page of documentation to the exact semantic graph it describes, and a gate can
prove that the two never name different declarations.

This is the first step of one larger rule: compiler-bundled agent material and
human documentation are generated from the same versioned semantic graph as
the installed binary.

## Command

```sh
semaprax doc <file> [--json]
```

- `<file>` is one `.spx` source file. It is parsed with its comments, then
  verified exactly as `check` verifies it. An unreadable file reports
  `SPX-I001`; parse and verification errors print the ordinary diagnostics and
  exit with status one, and nothing is written to stdout.
- Without `--json`, stdout receives the Markdown projection.
- With `--json`, stdout receives one `semaprax.doc.v1` document on one line.
- The grammar is closed: exactly one file, at most one `--json`, no other
  option. A malformed invocation exits with status two.

The command reads the source file and writes stdout. It creates no file,
resolves no project, and uses no ambient authority. Redirecting the output is
the caller's publication action.

## The model

Both renderings are functions of one model, built by `semaprax::doc::document`:

- `module`, `permits`, and the module's `use` lines;
- `revision`, the value of `semaprax::graph::revision` for the program, which
  is the revision `graph` prints for the same file;
- one entry per declaration in canonical order: types (records, variants,
  classes, resources), class methods, interfaces, protocols, implementations,
  then functions.

Every entry carries its kind, stable identity, display name, whether the
identity is persistent (written with `@id`) or automatic and revision-scoped,
its description, its signature, its facts, and its members.

- The description is the declaration's leading `//` comments, one line each,
  as [Canonical Comments v1](CANONICAL-COMMENTS-V1.md) places them, without
  the comment marker and one leading space.
- The signature is the declaration header in canonical source syntax with
  bodies omitted: the `@id` line when explicit, the `fn`, `record`, `variant`,
  `class`, `resource`, `interface`, `protocol`, or `impl` header, `uses`,
  `requires`, and `ensures` lines, fields, cases, lifecycles, imports, and
  method headers.
- Facts are labelled lists: `Type parameters`, `Parameters` (canonical
  `name: mode Type` text, so ownership modes are visible), `Returns`,
  `Effects`, `Requires`, `Ensures`, `Extends`, `Methods`, `Owner`, `Permits`,
  `Protocol`, and `Receiver`. A fact with no values is omitted, except
  `Returns`.
- Members are the identified parts of a declaration: fields, cases, case
  fields, drop lifecycles, imports, protocol methods, and implementation
  bindings, each with its identity, persistence, and canonical text.

## Markdown rendering

The Markdown page starts with `# Module \`<module>\``, a fixed three-line
paragraph naming the source of the facts, then bullets for the graph revision,
permits, and `use` lines. Entries are grouped under `## Records`, `## Variants`,
`## Classes`, `## Methods`, `## Resources`, `## Interfaces`, `## Protocols`,
`## Implementations`, and `## Functions`, in that order, omitting empty groups.
Each entry is `### \`<name>\``, the description lines, the signature in a
`spx` fenced block, an `Identity` bullet, one bullet per fact, and one bullet
per member kind with the members nested under it, each followed by its
identity.

## JSON rendering

```json
{"schema":"semaprax.doc.v1","module":"...","revision":"sha256:...","permits":[...],
 "uses":[{"kind":"function","id":"...","module":"...","alias":"..."}],
 "declarations":[{"kind":"function","id":"...","name":"...","persistent":true,
   "description":["..."],"signature":"...","facts":[{"label":"Returns","values":["i64"]}],
   "members":[{"kind":"field","id":"...","name":"...","persistent":true,"text":"x: i64"}]}]}
```

Key order is fixed, strings are escaped with the compiler's own JSON quoting,
and the document ends with one newline. It is the same model as the Markdown
page, so a tool that consumes the JSON and a person who reads the page see the
same declarations.

## Determinism and binding

The renderings are deterministic functions of the canonical program and its
comments. Formatting-only changes to the source do not change the revision,
the identities, or the facts; only the descriptions can change when comments
change. Bodies are not read, so a body edit that preserves the canonical
declaration header leaves the entry unchanged except for the revision bullet.

## Executable gate

`tests/projections/doc_projection.rs` in the `projections` harness:

- pins the exact Markdown bytes for `examples/effects.spx` and proves the CLI
  prints the library's bytes on stdout with status zero and empty stderr;
- proves the JSON rendering is one line, parses, names the graph revision, and
  lists the same declarations, signatures, and member counts as the model,
  through both argument orders;
- proves leading comments become descriptions for a class, a method, and a
  function;
- for every `examples/*.spx`, proves that every documented identity of a
  graph-carried kind (functions and methods, records and fields, variants and
  cases and case fields, classes, resources and drops, interfaces and imports)
  is a node of `semaprax graph` with the same kind and persistence, that every
  explicit declaration of those kinds in the graph is documented, and that
  the module, permits, and revision agree;
- proves invalid source, a missing file, and every malformed grammar fail
  closed with no stdout.

Protocols and implementations are documented from source but are not compared
with the graph, because the program graph stays protocol-free
([Static Protocol Conformance v1](STATIC-PROTOCOL-CONFORMANCE-V1.md)).

## Non-claims

`doc` documents one file. It does not resolve a project manifest, follow `use`
lines into other modules, render bodies, execute anything, publish files, or
generate agent skills. It does not replace `graph` or `context` as the exact
machine authority; the JSON projection is a documentation surface whose
identities and revision are proven to agree with the graph.
