# Web Application Projection v1

Status: implemented bounded projection. It is the second executable slice of
the "First-class application/state/UI dialect" row in the
[completion matrix](COMPLETION-MATRIX.md), after
[UI Schema](UI-SCHEMA-V1.md). Local evidence only.

Audience: language users, agents, and compiler contributors.

[Web Application Projection v2](WEBAPP-PROJECTION-V2.md) adds unique keys,
workflows, rollups, accounts, permissions, an audit history, and CSV export
on top of this contract; the v1 non-claims below that v2 covers no longer
apply to a module that uses those conventions.

`semaprax webapp <file> [-o|--output dir] [--title text] [--api]` projects one verified module into
a complete, dependency-free full-stack web application. `--title` supplies
explicit presentation metadata for the browser, sign-in heading, and server
startup line. It is a nonempty, control-character-free UTF-8 string of at most
256 bytes; invalid metadata is `SPX-WA106`. Without it, the title remains the
capitalised final module segment. The agent-facing
summary is the `web` topic of the [agent quick reference](AGENT-QUICK-REFERENCE.md#web-applications)
(`semaprax help language web`). The token benchmark that motivated it is
[benchmarks/webapp-tokens-v1](../benchmarks/webapp-tokens-v1/README.md).

## Input

The module must verify under the ordinary verifier. `SPX-T105` (no `main`) is
the one exception, because a web application has no entry point. No new
syntax exists: the projection reads only existing declarations.

- Every `record` is an entity. Its fields are `i64`, `f64`, `bool`, `char`,
  `string`, or a variant whose cases have no payload. A field named `id` is
  reserved. A field `<entity>_id: i64` whose prefix names another entity's
  snake_case name is a reference. Generic records and other field types are
  `SPX-WA102`.
- Entity names must have distinct snake_case routes; route collisions are `SPX-WA102`.
- A function named `<entity>_<suffix>` is an entity function. `<entity>` is
  the snake_case record name or its lowercase spelling, and the longest
  matching prefix wins. Each parameter must be a field of that entity, passed
  by value, with the same name and type, or the projection reports
  `SPX-WA102`. Entity functions are pure and monomorphic.
  - The suffix `valid` contributes one validation rule per `requires` clause.
    A rule's text is the clause's source text with whitespace collapsed, and
    its fields are the parameters the clause reads.
  - Any other suffix is a read-only computed field of that name, returning an
    admitted scalar. Its own `requires` clauses are preconditions.
- Other functions are helpers. A helper is translated only when an entity
  function reaches it. A function other than `main` that is neither an entity
  function nor reached from one is `SPX-WA105`, because silently ignoring it
  usually hides a misspelled prefix.

## Expression subset and semantics

Admitted: literals of the admitted types, parameters, `let` bindings, `if`,
`match` with payload-free variant cases, `i64`/`bool`/`char` literal
patterns, wildcards, bindings, guards, unary `-` and `!`, the arithmetic and
comparison operators, `&&`, `||`, payload-free variant construction, calls to
`string_len`, `string_len_chars`, `string_is_empty`, `string_contains`,
`string_starts_with`, `string_concat`, `string_from_i64`, `string_from_char`,
and calls to other admitted functions. Everything else is `SPX-WA103`, which
names the construct and lists the subset.

The translation keeps SEMAPRAX semantics:

- `i64` is a JavaScript BigInt. `+ - * / %` go through `rt.add`/`sub`/`mul`/
  `div`/`rem`, which trap with `overflow` outside the signed 64-bit range,
  with `division_by_zero`, and on `MIN % -1`. Division truncates toward zero
  and the remainder takes the dividend's sign.
- Executed functions evaluate their body once, then check every `ensures` in
  declaration order before returning the provisional result. Body and precondition
  failures take precedence. `_valid` remains a rule-only convention.
- `f64` is an IEEE double, as in SEMAPRAX (`1.0 / 0.0` is infinity).
- `&&` and `||` stay lazy, and evaluation is left to right.
- `string_len` counts UTF-8 bytes and `string_len_chars` Unicode scalars.
  `char` ordering compares scalar values.
- A failing rule rejects the row. A computed field that traps or fails a
  precondition or postcondition is reported as `{"error": "<code>"}` and never crashes the
  server.

## Output

The `-o` directory (default `webapp`) receives `schema.js`, the only
generated file, and five fixed runtime files embedded in the compiler:
`runtime.js`, `server.mjs`, `index.html`, `app.js`, and `style.css`. The
output is deterministic. The first line of `schema.js` is a fixed header,
and the second line binds the source file name and SHA-256 digest. A
non-empty output directory is replaced only when its `schema.js` starts with
that header; otherwise the command fails with `SPX-WA104` and writes
nothing.

`node server.mjs [--port N] [--host H] [--data DIR]` serves:

- `GET/POST /api/<entity>` and `GET/PUT/DELETE /api/<entity>/<id>`. Bodies
  are JSON, at most 1 MiB. JSON `i64`, `f64`, and reference fields require
  number tokens; quoted numeric strings are rejected. Browser form controls
  still accept decimal text and encode it as a number token. `i64` values
  round-trip exactly.
- 400 with `{"errors":[{field, message}]}` for type errors, failed rules, and
  missing reference targets. 404 for unknown rows. 409 when deleting a row
  that another row references.
- Persistence publishes one fsynced atomic `DIR/state.json` snapshot containing
  rows, auth, and audit, then refreshes the `db.json`, `auth.json`, and
  `audit.jsonl` mirrors. Startup repairs mirrors from the snapshot. IDs are
  monotonic and never reused. Before publication, write failure returns 500
  and preserves the previous state. After publication, a mirror failure returns
  503 with `committed:true`; restart recovers that mutation and its one audit fact.
- The canonical data directory has one exclusive `.writer-lock` claim, acquired
  before loading state. A second writer, including through a symlink alias,
  fails startup. SIGINT/SIGTERM and normal exit release the claim. A crash keeps
  it: inspect `owner.json`, confirm its PID is no longer running, and only then
  remove `.writer-lock` before restarting. Automatic stale-claim takeover is
  deliberately unsupported.
- A browser UI with hash routing: a dashboard of row counts per entity and
  per enumeration value; list pages with search, sorting, enumeration
  filters, and 25-row pagination; detail pages with reference links and
  back-references; and forms with typed inputs and the same validation as
  the server.

`node server.mjs --self-test` checks the generated application against its
own schema. It synthesizes one valid row per entity in reference order, using
candidate values seeded with the string literals of that entity's rules and a
bounded search of 20,000 rule evaluations. It then checks create, read,
list, update, validation, 404, 409 for each reference, persistence across a
restart, and delete. On success it prints one line; on failure it prints one
`FAIL` line per check and exits 1. A row it cannot synthesize is reported
with the rule that blocks it, which also exposes unsatisfiable rules.

## Authority

The projection only reads the source and writes the output directory. The
generated server listens where its operator tells it to and reads and writes
only its data directory. It has no outbound network access, child processes,
or other authority, and the compiler grants none. The one exception is
`--self-test`. It runs the same server file as a child process on an
ephemeral loopback port with a fresh temporary data directory, or a given
empty one. It drives that child with loopback requests and removes the
temporary directory afterwards.

## Non-claims

No authentication, styling system, localisation, file uploads, pagination on
the server, transactions across rows, or schema migration of an existing
`db.json` after fields change. Validation rules see one row; cross-row rules
are not expressible. No hosted, browser-automation, or production evidence is
claimed. The browser UI is exercised by its own module code only.

## Evidence

`src/webapp/tests.rs` covers the projection: entities, references, rules,
computed fields and helpers in `schema.js`; determinism; `SPX-WA102`,
`SPX-WA103`, `SPX-WA104`, and `SPX-WA105`; verifier errors still rejecting the
module; and the quick-reference example projecting and staying canonical.
`benchmarks/webapp-tokens-v1` records an end-to-end smoke run of the
generated TeamDesk server and a native-versus-JavaScript agreement check on
its computed fields.
