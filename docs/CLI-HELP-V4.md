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
  `version`, `help <command>`, `help all`, `help language`, `help library`);
- each entry is an abbreviated command shape, padded to one column, followed
  by a one-line purpose;
- a two-line footer naming the first command to run and the `--json`
  diagnostic form.

Every entry names a catalog command by its canonical name, and the capability
filter is the catalog's: the standalone executable omits `doctor` and the
`rust` build target exactly as its catalog does, and a group with no
visible entry is omitted. The guided page for either capability class is at
most 2048 bytes; that bound is a contract, enforced by unit and integration
evidence, so the page stays one screen as commands are added.

Guided shapes are summaries, not grammar. The catalog's usage lines remain the
single grammar authority: scoped help renders separate source and project
`build` shapes so their target catalogs do not imply capabilities the input
class lacks. Those shapes also expose `--json` and the `--output` spelling.
A guided shape must not be parsed as an admission rule.

## Exhaustive catalog

`semaprax help all` is one of two new admitted forms. For either executable it returns
status zero, empty stderr, and exactly the bytes v1 defined for the global
page: the banner, a blank line, `Usage:`, and every capability-visible global
catalog line in catalog order. Scoped help for `help` lists both of its
shapes:

```text
Usage:
  semaprax help <command>
  semaprax help all
  semaprax help language
  semaprax help library
```

`all` is not a command. A third token in any help form, including
`semaprax help all extra`, is an extra operand: it exits two, emits no stdout,
and names that operand in a precise `help accepts exactly one operand`
diagnostic. `semaprax all` and other placements retain the ordinary
unknown-command behavior. The typo suggestion and hidden-command refusal are
otherwise unchanged in bytes and status.

## Language card

`semaprax help language` is the third `help` shape. For either executable it
returns status zero, empty stderr, and exactly the bytes of the repository's
[agent quick reference](AGENT-QUICK-REFERENCE.md), compiled into the binary.
An agent or developer working from an installed compiler, without the source
checkout, can read the admitted shapes, the diagnostics that habits from other
languages trigger, and their fixes offline. The document's own gate checks its
code blocks against the compiler, so the card cannot describe syntax the
binary rejects. Scoped help for `help` lists all four shapes.

## Standard-library catalog

`semaprax help library` is the fourth `help` shape. For either executable it
returns status zero, empty stderr, and exactly the bytes of the repository's
generated [standard library catalog](STANDARD-LIBRARY-CATALOG.md), compiled
into the binary: every `std.*` declaration with its signature, effects, and
contracts. `tests/project.rs::standard_library` regenerates that document from
`std/` and pins it, so the printed catalog cannot list a function the compiler
does not ship.

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
has exact scoped help; the two-line `help help`; the malformed `help all extra`
case; and empty working directories with no created entries.

## Nonclaims

This surface is still not shell completion, dynamic discovery, a plugin
registry, a machine-readable command schema, or proof that a documented command
is published. A guided shape is not a grammar. Local tests are not hosted,
cross-platform, release, or support evidence.
