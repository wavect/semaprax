# Explore a project's meaning

The semantic explorer turns a checked project into a view you can read, share
or compare. After this page you can export one declaration's neighborhood as
HTML, Markdown, JSON or SVG, and answer smaller questions from the terminal.

## Export a focused view

Run this in a project directory (the output file must not exist):

```sh
semaprax explore semaprax.toml --target calculator.add --depth 1 --format html --output calculator-explorer.html
```

Open the file in a browser. The view is centered on `calculator.add`.
`--depth` limits how far the neighborhood extends. Omit `--target` and
`--depth` for the whole project.

| `--format` | Use it for |
| --- | --- |
| `html` | Browsing. |
| `markdown` | A written review. The page is source-free and may still contain names and paths. |
| `json` | A tool. |
| `svg` | A diagram. |

An existing output path fails with `SPX-G328`. Use one new filename per format
and subject. Keep generated reports out of commits unless you version them on
purpose, and keep the revision lines when you share one.

## Ask smaller questions first

```sh
semaprax query . --id calculator.add                  # where is it?
semaprax query . --calls calculator.add               # who calls it?
semaprax query . --called-by calculator.app.main      # what does it call?
semaprax query . available-operations calculator.add  # which typed changes are allowed?
semaprax context . calculator.add --direction both --depth 1 --max-bytes 4096
semaprax doc <file.spx>                               # declarations, contracts, effects
```

A **caller** uses the function. A **callee** is a function it calls. Keep the
two directions straight when you estimate the effect of a change.

## Preview a rename

```sh
semaprax change preview . rename-display-name calculator.add sum
```

The preview renames the display name and keeps the stable ID. It writes
nothing. Next steps are in [Shipping](../projects/shipping.md#change-with-review).

## Review a candidate

Pass a candidate capsule and the exact digest you intend to review:

```sh
semaprax explore semaprax.toml --candidate-capsule capsule.json --expect-candidate sha256:<digest> --format markdown --output review.md
```

If the digest differs, the export fails: you never review a candidate you did
not mean to. A **capsule** is revision-bound candidate data. Make one with
`project-candidate-export` ([Shipping](../projects/shipping.md#keep-candidates-and-images)).
`semaprax compact candidate-diff <project> <capsule>` gives a compact diff.

## Fill a typed hole or take a compiler repair

Use these when a change is half done or was rejected. Both work on a
candidate, which is in memory, so source stays unchanged until you commit.

A **typed hole** marks one expression in a function body that you will fill
later. The compiler gives you the hole's expected type and ownership, the
effects allowed there, the names in scope and the calls you may use. A fill
becomes a normal typed change and goes through the full checks. There are no
placeholder source text, no invalid code and no run. You can open up to 16
holes, and they must not overlap.

```sh
semaprax serve-diagnostics semaprax.toml     # JSON-RPC over stdin and stdout
```

A client sends `hole/open-expression`, then `hole/query` (context),
`hole/fill-suggestions`, `hole/fill`, and finally `hole/complete`.
`hole/discard` drops the draft. `complete` refuses while any hole is open
(`SPX-G232` for a stale or unresolved selection). In VS Code use
**Open Typed Hole** and **Fill Selected Hole from Active Scratch**
([Editor setup](../getting-started/editor.md)).

A **repair route** is a fix the compiler derived and already admitted. When
`candidate/attempt` rejects a change, the same server keeps the attempt:
`attempt/repair-catalog` lists only proposals that pass every check (or says
there are none), and `attempt/repair-apply` turns one into a new candidate.
The compiler never picks or ranks for you. For files rather than candidates,
the one repair today is the missing `@id` fix (`SPX-S103`): `semaprax fix
--plan`, then `repairs` and `repair`
([Debugging](debugging.md#apply-a-one-step-repair)).

Specs:
[Expression Holes v1](https://github.com/wavect/semaprax/blob/main/docs/PROJECT-CANDIDATE-EXPRESSION-HOLES-V1.md),
[Image Candidate Diagnostic Protocol v4](https://github.com/wavect/semaprax/blob/main/docs/IMAGE-CANDIDATE-DIAGNOSTIC-PROTOCOL-V4.md).

## What to ask before you accept a change

Which declaration changed? Which callers are involved? Did contracts or effects
change? Which tests cover it? Refresh the report after every edit: an old
diagram stays readable long after it stops describing the project.

**Next:** [Use the same workflow from an AI coding agent](agents.md).
References: [Semantic Explorer v1](https://github.com/wavect/semaprax/blob/main/docs/SEMANTIC-EXPLORER-V1.md),
[installed command catalog](https://github.com/wavect/semaprax/blob/main/src/cli/help.rs).
