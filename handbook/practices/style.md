# Style guide

`fmt` and `check` enforce most style. This page covers what tools cannot decide:
identities, naming and layout. After this page you can name things so that
agents, queries and patches keep working across renames.

## Give every declaration an ID

An `@id` is a declaration's permanent address. Renames do not change it, and
every query, patch and graph edge uses it.

```semaprax
module calculator.core;

@id("calculator.add")
fn add(left: i64, right: i64) -> i64
{
    left + right
}
```

Without an `@id`, `check` warns `SPX-S103`: the automatic identity changes when
the function is renamed. Assign one by hand, or use
`semaprax fix --plan` (see [Debugging](debugging.md#apply-a-one-step-repair)).

- **Namespace by module:** `calculator.add`, `calculator.tests.test_add`.
  Avoid flat global names and trees like `a.b.c.d.e.f`.
- **Name the meaning, not today's name:** `math.add` survives a rename to `plus`.
- **ID fields, cases and methods too.** Queries and patches address them by ID.
- **Never reuse an ID** for a different declaration. A stale ID with new meaning
  is worse than none, and nothing can warn about it.

## Names

| Thing | Style |
| --- | --- |
| Modules | `dotted.lowercase`, matching the file's role |
| Functions, bindings | `snake_case` |
| Types | `PascalCase` |
| Tests | `test_<what>() -> i64`, no parameters, `0` is pass |
| `main` | Returns `i64`; `0` is success |
| Project names | `[a-z][a-z0-9-]*` |

## Layout: let `fmt` do it

```sh
semaprax fmt .            # rewrite every file in canonical form
semaprax fmt . --check    # report drift, write nothing
```

Canonical form: the body's `{` on its own line; one statement per line; compact
`if`, `match` and record literals; a trailing `,` after every field, case and
arm including the last. `fmt` parses all files before rewriting any, and keeps
`//` comments ([placement rules](https://github.com/wavect/semaprax/blob/main/docs/CANONICAL-COMMENTS-V1.md)).
Workspace transactions do not promise to keep comments, so put durable intent in
`@id` names, contracts and tests. Run `fmt` before `check`. A formatting diff is
never the interesting part of a review.

## Organize files

- One module per file, one concern per module.
- The entry module holds `main` and wiring. Logic lives in sibling modules
  imported by ID, right after the `module` line:
  `use function @id("calculator.add") from calculator.core as add;`
- Tests live in their own module, listed under `tests` in the manifest.
- Keep bodies shallow. Nesting deeper than 128 levels fails with `SPX-P207`, but
  a few levels is already a sign to extract a named helper with its own `@id`
  and contracts. Then it is queryable and testable.

## Put intent in contracts

`requires` and `ensures` state what callers may pass and what they get back, and
run on every call, including under `test`. Prefer them to comments. Write the
effect list (`permit` and `uses`) as narrowly as the code allows. See
[Contracts and effects](../language/contracts-effects.md).

## Keep the manifest canonical

Keep `semaprax.toml` byte-canonical: table order, one blank line between tables,
one-line arrays, no comments. `SPX-J100` names the first differing line. See
[Manifests](../projects/manifests.md). Pin what you ship:
`semaprax lock . --write`, then `--verify`, and `--compare <base.lock>` in CI
([Shipping](../projects/shipping.md#lock-the-interface)).

## Generate docs from the source

`semaprax doc <file>` renders declarations, signatures, contracts, effects and
comments from the checked graph, so your docs cannot drift from the code
(`--json` for tools). A file given to `doc` needs a `fn main() -> i64`.
