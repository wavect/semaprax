# Style guide

Semaprax style is mostly enforced by `fmt` and `check`. This page covers the
decisions tools can't make for you: identities, naming, and layout.

## Identities first

Every public declaration needs an explicit `@id`. Think of it as the
declaration's permanent address — renames don't change it, and every agent
query, patch, and graph edge keys off it.

```semaprax
module calculator.core;

@id("calculator.add")
fn add(left: i64, right: i64) -> i64
{
    left + right
}
```

Rules of thumb:

- **Namespace by module**: `calculator.add`, `calculator.tests.test_add`.
  Flat global names collide; deep trees (`a.b.c.d.e.f`) are noise.
- **Name what it means, not what it's called today**: `@id("math.add")`
  survives a rename from `add` to `plus`. The display name can evolve; the id
  shouldn't need to.
- **Id every field, case, and method too** — not just functions. Queries and
  patches address fields by id.
- **Never reuse an id** for a different declaration. A stale id pointing at
  new meaning is worse than a missing one (`SPX-S103` warns on the missing
  case; nothing can warn on the misleading one).

## Naming

- Modules: `dotted.lowercase`, matching the file's role (`calculator.core`,
  `app.bytes`).
- Functions and bindings: `snake_case`. Types: `PascalCase`.
- Test functions: `test_<what>` with zero parameters, returning `i64`
  (`0` = pass). The test runner reports failures by stable id.
- `main` returns `i64`; return `0` for success, nonzero for failure.

## Layout (let `fmt` do it)

Canonical form, enforced by `semaprax fmt`:

- The function body's `{` goes on its own line; each statement on its own
  line; `if`, `match`, and record literals stay compact.
- Every field, parameter-adjacent declaration, and match arm ends with `,`
  — including the last one.
- `//` comments are preserved by `fmt`; workspace transactions don't promise
  that, so put durable intent in `@id` names, contracts, and tests instead
  of prose comments.

Run `fmt` before `check`, always. A formatting diff is never the interesting
part of a review.

## File and project organization

- **One module per file**, one concern per module. Split when a file holds
  more than a handful of declarations.
- **Entry module holds `main` and wiring only**; logic lives in sibling
  modules imported by stable id:
  `use function @id("calculator.add") from calculator.core as add;`
  directly after the `module` line.
- **Tests live in their own module** listed in the manifest's `tests` array,
  not interleaved with implementation.
- **Keep function bodies shallow.** Nesting past a few levels (hard limit:
  128, `SPX-P207`) means extract a named helper with its own `@id` and
  contracts — which also makes the helper queryable and testable.

## Manifest hygiene

- Keep `semaprax.toml` byte-canonical: table order, blank lines, one-line
  arrays, no comments. `SPX-J100` names the first differing line.
- Pin what you ship: `semaprax lock semaprax.toml --write` records a
  deterministic `semaprax.lock`; `--verify` re-checks it, and
  `--compare <base.lock>` reports breaking interface changes for CI.
