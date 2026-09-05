# Guided CLI Help v4

Status: authored and locally exercised; unpublished and unpromoted. The
completion matrix and release evidence own product status.

Audience: CLI users, coding agents, release engineers, and compiler
contributors.

This additive revision replaces the exhaustive global help page with a guided
one-screen overview and moves the exhaustive catalog behind one new admitted
form, `semaprax help all`. It preserves the v1 catalog, the v1 scoped-help
bytes, the v2 typo behavior, and the v3 recovery hint.

## Why

The v1 global page listed every catalog command, one grammar line each, with
no grouping and no purpose. It reached 7 KB, and more than ninety of its lines
were tool-author protocol surfaces. A developer or coding agent reading it
before a first command had to find `check`, `run`, and `fmt` among evidence,
retention, and workspace transaction grammars. Help is the first thing an agent
reads, so its size is a per-task cost.

## Guided global help

The no-argument invocation and the exact one-token `help`, `--help`, and `-h`
forms now print the guided page on stdout. Statuses are unchanged: two for the
empty invocation, zero for the three aliases. The page is rendered from one
static, source-owned guide:

- the unchanged banner line, then `Usage: semaprax <command> [arguments]`;
- six fixed groups in this order, each a heading ending in `:` followed by
  two-space-indented entries: `Write, check, and run` (`check`, `fmt`, `run`,
  `test`, `build`), `Inspect meaning` (`graph`, `context`, `doc`, `query`), `Change by
  meaning` (`patch`, `impact`, `review`, `verify`), `Agents` (`agent inspect`),
  `Start a project` (`new`, `project-scaffold`), and `Toolchain` (`doctor`,
  `version`, `help <command>`, `help all`, `help language [topic]`, `help library`,
  `help shapes`);
- each entry is an abbreviated command shape, padded to one column, followed
  by a one-line purpose;
- a two-line footer naming the first command to run and the `--json`
  diagnostic form.

Every entry names a catalog command by its canonical name, and the capability
filter is the catalog's: since `doctor` moved into the root crate no catalog
command is private any more, the standalone executable omits only the `rust`
build target exactly as its build catalog does, and a group with no visible
entry is omitted. The guided page for either capability class is at
most 2048 bytes; that bound is a contract, enforced by unit and integration
evidence, so the page stays one screen as commands are added.

Guided shapes are summaries, not grammar. The catalog's usage lines remain the
single grammar authority: scoped help renders separate source and project
`build` shapes so their target catalogs do not imply capabilities the input
class lacks. Those shapes also expose `--json` and the `--output` spelling.
A guided shape must not be parsed as an admission rule.

## Exhaustive catalog

`semaprax help all` is one of the admitted forms. For either executable it returns
status zero, empty stderr, and exactly the bytes v1 defined for the global
page: the banner, a blank line, `Usage:`, and every capability-visible global
catalog line in catalog order. Scoped help for `help` lists all admitted
shapes:

```text
Usage:
  semaprax help <command>
  semaprax help all
  semaprax help language
  semaprax help language <topic|topics>
  semaprax help library
  semaprax help library <module|name|stable-id>
  semaprax help shapes
  semaprax help shapes <kind|stable-id|path#stable-id>
```

`all` is not a command. An operand beyond one of the admitted shapes, including
`semaprax help all extra` or `semaprax help language scalars extra`, exits two,
emits no stdout, and names that operand in a precise
`help accepts exactly one operand` diagnostic. `semaprax all` and other
placements retain the ordinary unknown-command behavior. The typo suggestion
and hidden-command refusal are otherwise unchanged in bytes and status.

## Language card

`semaprax help language` is the third `help` shape. For either executable it
returns status zero, empty stderr, and exactly the bytes of the repository's
[agent quick reference](AGENT-QUICK-REFERENCE.md), compiled into the binary.
An agent or developer working from an installed compiler, without the source
checkout, can read the admitted shapes, the diagnostics that habits from other
languages trigger, and their fixes offline. The document's own gate checks its
code blocks against the compiler, so the card cannot describe syntax the
binary rejects.

`semaprax help language <topic|topics>` is the fourth shape. `topics` returns
the closed stable selector list and its card headings. The exact,
case-sensitive topic selectors are `workflow`, `module`, `scalars`,
`control-flow`, `records`, `ownership`, `strings`, `builtins`,
`mistakes-code`, `mistakes-index`, `projects`, and `specifications`. A selector
returns exactly its complete `##` section, including the heading, from the same
compiled card; it cannot drift from or reinterpret the compiler-checked
document. It never includes the next section. No match exits two, emits no
stdout, and reports the literal diagnostic “language card has no exact topic
`<selector>`” on stderr. No fuzzy, prefix, heading, or case-folded matching is
admitted.

The topic inventory is capped at 768 bytes. Every topic is capped at 4,600
bytes and 1,500 repository lexical units and must remain more than five times
smaller than the full card in both measures. The guarded `scalars` section is
also capped at 1,024 bytes and 300 units and must remain more than twenty times
smaller in both measures. The current card is 25,435 bytes and 7,237 units;
`scalars` is 793 bytes and 296 units, while the topic inventory is 569 bytes
and 77 units. Scoped help for `help` lists all eight shapes.

## Standard-library catalog

`semaprax help library` is the fifth `help` shape. For either executable it
returns status zero, empty stderr, and exactly the bytes of the repository's
generated [standard library catalog](STANDARD-LIBRARY-CATALOG.md), compiled
into the binary: every `std.*` declaration with its signature, effects, and
contracts. `tests/project.rs::standard_library` regenerates that document from
`std/` and pins it, so the printed catalog cannot list a function the compiler
does not ship.

`semaprax help library <module|name|stable-id>` is the sixth shape and uses the
generated `std/catalog.json` from that same gate. Matching is exact and
case-sensitive. A module identity returns its declarations in catalog order;
a declaration name or persistent identity returns every exact match in that
order. Each result contains only the persistent identity, exact manifest
dependency row, required project profile, and canonical signature, effects,
and contracts. Results are separated by one blank line. No match exits two,
emits no stdout, and reports
`standard library has no exact match for \`<selector>\`` on stderr. The route
does not admit fuzzy or prefix matching, so an underspecified query cannot
silently expand into the full catalog.

The full catalog is currently 22,076 bytes and 6,662 lexical units. The
`std.core.compare` name and stable-ID lookup outputs are identical: 226 bytes
and 68 lexical units, with ceilings of 512 bytes and 128 units. Both measures
must remain more than 50 times smaller than the full catalog. Integration
evidence pins those bounds while the original full-catalog byte equality
remains unchanged.

## Language shapes catalog

`semaprax help shapes` is the seventh `help` shape. For either executable it
returns status zero, empty stderr, and exactly the bytes of the repository's
generated [language shapes catalog](LANGUAGE-SHAPES-CATALOG.md), compiled into
the binary: every declaration of every committed example, grouped by kind,
with its `@id` and canonical header as the `semaprax doc` model renders it.
`tests/projections.rs::shapes_catalog` regenerates that document from
`examples/` and pins it, so the printed shapes are exactly the ones the
compiler verifies.

`semaprax help shapes <kind|stable-id|path#stable-id>` is the eighth shape and
uses the generated `docs/LANGUAGE-SHAPES-CATALOG.json` companion from the same
gate. Matching is exact and case-sensitive. A declaration kind returns the
canonical exemplar with the fewest repository lexical units, then fewest
bytes, stable identity, and source path; it never expands to the whole kind.
A stable identity returns every exact match in catalog order because example
modules may reuse an identity such as `app.main`; `path#stable-id` selects one
exact example. Each result contains the kind, source path, and canonical
signature. Results are separated by one blank line. No match exits two, emits
no stdout, and reports
`language shapes catalog has no exact match for \`<selector>\`` on stderr.
The route admits no fuzzy or prefix matching.

The full shapes catalog is 22,888 bytes and 7,571 lexical units. The guarded
`calculator.add` lookup is 114 bytes and 33 units; every generated kind
exemplar and that exact lookup must stay within 512 bytes and 128 units, and
the exact lookup must remain at least 40 times smaller than the full catalog
in both measures. The original full-catalog bytes remain unchanged.

## Preservation

Scoped help (`help <command>`, `<command> --help`, `<command> -h`), the
malformed-position rejection, and the recovery hint are unchanged except for
the additive `build` grammar described above. Help still calls no host hook,
reads no path, inspects no environment, and grants no authority.

## Evidence

The standalone and full-toolchain help harnesses prove: the guided page's
banner, byte bound, group headings, capability filtering, and that each guided
entry resolves to a scoped-help command; `help all` byte structure, ordering,
and capability filtering for both executables; that every `help all` line still
has exact scoped help; all eight `help` grammar lines; the full language-card
and both generated catalogs' byte identities; exact topic inventory and
section boundaries; exact name, stable-ID, module, path-disambiguation,
kind-exemplar, missing-selector, and token-economics behavior for scoped
lookups; malformed extra operands; and empty working directories with no
created entries.

## Nonclaims

This surface is still not shell completion, dynamic discovery, a plugin
registry, a machine-readable command schema, or proof that a documented command
is published. A guided shape is not a grammar. Local tests are not hosted,
cross-platform, release, or support evidence.
