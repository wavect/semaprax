# Guided CLI Help v4

Status: implemented bounded profile; **HOSTED GREEN** under the
[v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md). Historical local,
authoring-time, ignored, device/simulator, or separately provisioned evidence
below retains its narrower scope; public promotion, registry publication and
broader product completion remain separately gated.

Audience: CLI users, coding agents, release engineers, and compiler
contributors.

V4 makes global help a guided one-screen overview. Use `semaprax help all`
for the exhaustive command catalog. Existing command catalog entries and
order, v2 typo behavior, and the v3 recovery hint stay unchanged; scoped help
adds an explicit full standard-library catalog and bounded language-shape kind
selectors. The `fmt` catalog also exposes an explicit manifest-layout mode.

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
- a two-line footer naming the first command to run, exact diagnostic-code
  help, and the `--json` diagnostic form.

Every entry names a catalog command by its canonical name, and the capability
filter is the catalog's: `source-live` is private to `semaprax-full` and
appears only in its exhaustive catalog and scoped help. The standalone
executable also omits the `rust` build target. Groups with no visible entries are omitted. For either capability class, the
guided page is limited to 2048 bytes. Unit and integration evidence enforce
that contract so the page stays one screen as commands are added.

Guided shapes summarize commands; they do not define accepted grammar. The
catalog's usage lines remain authoritative. Scoped help shows separate source
and project `build` shapes so their targets do not imply unsupported input
capabilities. Source build shapes separately show native commands, native-callable
functions, and Wasm/web exports. Only the Wasm/web form names `--profile`;
native text support is selected from source. Those shapes also show `--json`
and the `--output` spelling. Scoped `test` help separates interpreter limits
from the native Project route and its per-process timeout and output limits.
Do not parse a guided shape as an admission rule.

## Formatter

`semaprax fmt <file>|<dir>|semaprax.toml [--check]` formats source files. A
directory or manifest selects the `.spx` files listed by that manifest; the
manifest itself must already be canonical and is never rewritten by this
shape. `--check` checks only those source files.

`semaprax fmt --manifest <semaprax.toml> [--check]` is the explicit table
manifest layout formatter. It accepts only `semaprax.toml` paths and applies
the canonical table/key/spacing order after the shared Project Manifest parser
has validated the complete manifest. It does not repair missing or invalid
semantic fields, source cardinality or ordering, test-module declarations,
profiles, or unknown tables and keys. `--check` reports layout drift without
writing; write mode changes only that manifest file. Frozen assignment layouts
retain their existing canonical requirement.

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
  semaprax help diagnostic <SPX-code|codes>
  semaprax help language
  semaprax help language <topic|topics>
  semaprax help library
  semaprax help library all
  semaprax help library <module|name|stable-id>
  semaprax help shapes
  semaprax help shapes kinds
  semaprax help shapes <kind|stable-id|path#stable-id>
```

`all` is not a command. An operand beyond one of the admitted shapes, including
`semaprax help all extra`, `semaprax help diagnostic SPX-T208 extra`, or
`semaprax help language scalars extra`, exits two,
emits no stdout, and names that operand in a precise
`help accepts exactly one operand` diagnostic. `semaprax all` and other
placements retain the ordinary unknown-command behavior. The typo suggestion
and hidden-command refusal are otherwise unchanged in bytes and status.

## Diagnostic help

`semaprax help diagnostic <SPX-code|codes>` is the third `help` shape. An exact,
case-sensitive `SPX-*` code returns only the common failed form and correction
rows indexed for that code. `codes` and bare `help diagnostic` return a
common-code shortlist with commands for exact-code advice and the complete
`help language mistakes-index` table. The shortlist ranks codes by the number
of indexed failed forms, descending, then by exact code; this is advice
coverage, not measured diagnostic frequency. It includes at most twelve whole
codes, reserving space for both navigation commands within the byte bound.
Adding an indexed code does not require it to fit this shortlist: every code
remains available through exact lookup and the complete table.
The response is derived from the diagnostic-index table in the compiler-checked
[agent quick reference](AGENT-QUICK-REFERENCE.md) through the pinned
`semaprax.agent-diagnostic-help.v1`
[JSON companion](AGENT-DIAGNOSTIC-HELP.json); the CLI does not maintain a second
copy of the advice. The documentation gate also requires every marked failing
example in the card to have an indexed correction.

No match exits two, emits no stdout, and reports the literal diagnostic
“diagnostic help has no exact match for `<SPX-code>`” on stderr. Prefix, fuzzy,
and case-folded matching are not admitted. The default shortlist is capped at
256 bytes and 100 repository lexical units. Every exact response is capped at
1,024 bytes and 300 units. The
guarded `SPX-T208` response is 111 bytes and 32 units, more than twenty times
smaller in both measures than the 2,513-byte, 916-unit complete diagnostic
index. Even the nine-row `SPX-P106` response is only 773 bytes.

## Language card

`semaprax help language` is the fourth `help` shape. For either executable it
returns status zero, empty stderr, and exactly the bytes of the repository's
[agent quick reference](AGENT-QUICK-REFERENCE.md), compiled into the binary.
An agent or developer working from an installed compiler, without the source
checkout, can read the admitted shapes, the diagnostics that habits from other
languages trigger, and their fixes offline. The document's own gate checks its
code blocks against the compiler, so the card cannot describe syntax the
binary rejects.

`semaprax help language <topic|topics>` is the fifth shape. `topics` returns
the closed stable selector list and its card headings. The exact,
case-sensitive topic selectors are `workflow`, `module`, `scalars`,
`control-flow`, `records`, `ownership`, `strings`, `builtins`, `cli`, `maps`, `lists`,
`mistakes-code`, `mistakes-index`, `web`, `projects`, and `specifications`. A selector
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
smaller in both measures. The current card is 52,372 bytes and 14,673 units;
`scalars` is 788 bytes and 293 units, while the topic inventory is 718 bytes
and 94 units. Scoped help for `help` lists all eleven shapes.

## Standard-library catalog

`semaprax help library` is the sixth `help` shape. For either executable it
returns status zero, empty stderr, and a compact index of all 50 bundled module
identities with exact-lookup and full-catalog syntax. The index comes from the
same generated `std/catalog.json` as declaration lookup, so each listed module
is shipped by the compiler. The current index is 872 bytes and 237 lexical
units; the executable gate caps it at 2,048 bytes and 256 units.

`semaprax help library all` is the seventh shape. It returns status zero, empty
stderr, and exactly the bytes of the repository's generated [standard library
catalog](STANDARD-LIBRARY-CATALOG.md), compiled into the binary: every `std.*`
declaration with its signature, effects, and contracts. `tests/project.rs::standard_library`
regenerates and pins that document from `std/`.

`semaprax help library <module|name|stable-id>` is the eighth shape and uses the
generated `std/catalog.json` from that same gate. Matching is exact and
case-sensitive. A module identity returns its declarations in catalog order;
a declaration name or persistent identity returns every exact match in that
order. Each result contains only the persistent identity, exact manifest
dependency row, required project profile, and canonical signature, effects,
and contracts. Results are separated by one blank line. No match exits two,
emits no stdout, and reports
`` standard library has no exact match for `<selector>` `` on stderr. The route
does not admit fuzzy or prefix matching, so an underspecified query cannot
silently expand into the full catalog.

Qualified `std.core.compare` and `std.int.decimal.compare` lookups each keep
ceilings of 512 bytes and 128 lexical units, and remain more than 50 times
smaller than the full catalog. The bare name `compare` now returns both exact
matches, separated by a blank line, in catalog order. Its combined result is
pinned to 512 bytes and twice the per-result lexical ceiling; each qualified
result still satisfies the original per-result bounds. Integration evidence
pins both signatures and the combined output, while full-catalog byte equality
remains unchanged.

## Language shapes catalog

`semaprax help shapes` is the ninth `help` shape. For either executable it
returns status zero, empty stderr, and exactly the bytes of the repository's
generated [language shapes catalog](LANGUAGE-SHAPES-CATALOG.md), compiled into
the binary: every declaration of every committed example, grouped by kind,
with its `@id` and canonical header as the `semaprax doc` model renders it.
`tests/projections.rs::shapes_catalog` regenerates that document from
`examples/` and pins it, so the printed shapes are exactly the ones the
compiler verifies.

`semaprax help shapes kinds` is the tenth shape. It returns a complete compact
index of every exact kind selector in `docs/LANGUAGE-SHAPES-CATALOG.json`, in
lexical order, followed by the exact-exemplar and full-catalog commands. The
index is derived from the generated companion at runtime and is not truncated.
It is capped at 2,048 bytes and 256 repository lexical units. The generated
index must contain neither a kind nor a stable identity named `kinds`, so this
exact selector cannot shadow an existing lookup. Case variants and prefixes
remain ordinary exact lookups and return the existing no-match diagnostic.

`semaprax help shapes <kind|stable-id|path#stable-id>` is the eleventh shape and
uses the generated `docs/LANGUAGE-SHAPES-CATALOG.json` companion from the same
gate. Matching is exact and case-sensitive. A declaration kind returns the
canonical exemplar with the fewest repository lexical units, then fewest
bytes, stable identity, and source path; it never expands to the whole kind.
A stable identity returns every exact match in catalog order because example
modules may reuse an identity such as `app.main`; `path#stable-id` selects one
exact example. Each result contains the kind, source path, and canonical
signature. Results are separated by one blank line. No match exits two, emits
no stdout, and reports
`` language shapes catalog has no exact match for `<selector>` `` on stderr.
The route admits no fuzzy or prefix matching.

The full shapes catalog is 22,888 bytes and 7,571 lexical units. The guarded
`calculator.add` lookup is 114 bytes and 33 units; every generated kind
exemplar and that exact lookup must stay within 512 bytes and 128 units, and
the exact lookup must remain at least 40 times smaller than the full catalog
in both measures. The original full-catalog bytes remain unchanged.

## Preservation

Scoped help (`help <command>`, `<command> --help`, `<command> -h`), the
malformed-position rejection, and the recovery hint are unchanged except for
the additive `build` grammar, library catalog selector, shapes kind index
selector, and `fmt --manifest` grammar. Help still calls no host hook, reads no
path, inspects no environment, and grants no authority.

## Evidence

The standalone and full-toolchain help harnesses prove: the guided page's
banner, byte bound, group headings, capability filtering, and that each guided
entry resolves to a scoped-help command; `help all` byte structure, ordering,
and capability filtering for both executables; that every `help all` line still
has exact scoped help; all eleven `help` grammar lines; the full language-card
and both generated catalogs' byte identities; bounded diagnostic navigation,
complete-table reachability and exact lookup through catalogue growth,
generated-companion pin, compiler-example coverage, topic inventory, and section
boundaries; exact diagnostic, name, stable-ID, module, path-disambiguation,
complete kind-index, kind-exemplar, missing-selector, and token-economics
behavior for scoped lookups; malformed extra operands; and empty working
directories with no created entries.

## Nonclaims

This surface is still not shell completion, dynamic discovery, a plugin
registry, a machine-readable command schema, or proof that a documented command
is published. A guided shape is not a grammar. Local tests are not hosted,
cross-platform, release, or support evidence.
