# Web Application Projection v2

Status: implemented bounded projection, additive to
[Web Application Projection v1](WEBAPP-PROJECTION-V1.md). Local evidence
only. The completion matrix row "First-class application/state/UI dialect"
stays Partial.

Audience: language users, agents, and compiler contributors.

v2 adds the features every larger business application needs: unique keys,
workflows, rollups over referencing rows, accounts with sign-in, role- and
row-level permissions, an audit history, and CSV export. As in v1 there is no
new syntax. Each feature is a naming convention over ordinary records and
pure functions, checked by the ordinary verifier, translated with the v1
expression subset and semantics, and failing closed with `SPX-WA102`. The
agent-facing summary is `semaprax help language web`.

## Conventions

A function named `<entity>_<suffix>` belongs to that entity (v1 prefix
rules). The suffix selects its meaning:

| Suffix | Meaning | Parameters | Result |
| --- | --- | --- | --- |
| `valid` | v1 validation rules | fields | any |
| `key`, `key_<name>` | Unique key: no two rows share `value` | fields | admitted scalar |
| `<field>_step` | Workflow over the variant-typed `<field>` | exactly two values of that variant, bound positionally as current and proposed | `bool` |
| `account` | This entity holds the accounts; the first parameter is the string sign-in field, and the result says whether that row may sign in | fields | `bool` |
| `can_read`, `can_write` | Permission for the signed-in account | fields, `me: i64`, `my_<account field>` | `bool` |
| anything else | v1 computed field, which may also take rollups | fields, rollups | admitted scalar |

Unprefixed `can_read` / `can_write`, and `can_read_<name>` /
`can_write_<name>`, are defaults for entities without a policy of their own.
They take `me`, `my_<field>`, and row fields that some entity declares. A
default naming row fields applies to every entity that has all of them with
the same types. Among applicable defaults, the one naming the most row fields
wins, and declaration order breaks ties. A module may have at most one
`_account` function. Permissions and the defaults require one.

A rollup parameter of a computed field aggregates the rows of a child entity
whose single reference field names this entity:

- `count_<child>: i64`: the number of child rows.
- `count_<child>_<bool>: i64`: the number where the child's stored or
  computed `bool` is true.
- `sum_<child>_<number>`: the sum of the child's stored or computed `i64` or
  `f64`, with the parameter's type. An `i64` sum is checked.

A child computed field used by a rollup must not take rollups itself, so
rollups cannot form cycles. `id` and `password` are reserved field names, and
`Audit` and `Session` are reserved record names.

## Runtime behavior

`schema.js` gains `export const account` and, per entity, `keys`, `steps`,
`rollups`, `canRead`, and `canWrite`. A v1 schema keeps its v1 behavior. The
runtime files implement:

- **Keys:** a duplicate value is a 400 naming the key's fields. The
  account's sign-in field is always unique.
- **Workflows:** rows are created in the variant's first case. An update
  that changes the field must pass the step. The UI offers only allowed next
  states.
- **Rollups:** computed over all rows by the server, independent of what the
  viewer may read. A failing rollup (overflow, or an error in the child's
  computed field) marks only the computed fields that read it as errors.
- **Accounts:** sign-in by login field and password. Passwords are stored
  only as scrypt hashes (N=16384, r=8, p=1, 16-byte salt) in a separate
  `auth.json` (mode 0600) and compared in constant time. An unknown login
  still hashes, so it takes the same time. Sessions use a 32-byte random
  token in an HttpOnly, SameSite=Strict cookie, and the server keeps only
  its SHA-256. A session ends when its row is deleted or can no longer sign
  in. All failures return one generic 401.
- **First run:** `node server.mjs --setup` serves unauthenticated requests
  with every permission until any account has a password. Without `--setup`,
  an empty system returns 401.
- **Permissions:** unreadable rows are absent from lists, CSV, the dashboard,
  and reference selects, and requesting one is 404. A forbidden write is 403.
  A create checks the new row, an update checks the old and the new row, and
  a delete checks the old row. The UI hides actions the account cannot take.
- **Audit:** every create, update, and delete is appended to `audit.jsonl`
  with time, account, entity, id, and the changed fields with old and new
  values. Password changes are logged only as `changed`.
  `GET /api/<entity>/<id>/history` returns one row's entries. `GET /api/audit`
  returns all of them to accounts that may write the account entity.
- **CSV:** `GET /api/<entity>?format=csv` honours `q` and enumeration filters.
  It writes an `id` column, then the fields, then the computed fields, with
  RFC 4180 quoting. A leading `=`, `+`, `-`, `@`, tab, or carriage return
  that is not a plain number is prefixed with `'`, so spreadsheets do not run
  it as a formula.

`node server.mjs --self-test` extends v1. It also checks keys, workflows, the
audit history, CSV, and rollups. When accounts exist it also checks sign-in,
sign-out, and a 401 without a session. For every case of every enumeration
field of the account entity, it compares each entity's list visibility and
PUT result against the schema's own permission predicates.

`semaprax webapp <file> --api` verifies and projects the module like a
normal run but writes nothing. Instead it prints a compact plain-text
listing: the sign-in route when accounts exist, the route shapes, and one
line per entity with its fields (`x_id->entity` for references), computed
fields with their types, keys, workflows, rule count, and whether reads and
writes are open, role rules, or row rules. An agent can confirm the API
without reading generated code.

A passing self-test ends with a `cleanup:` line. It records that the test
server process exited and that the temporary data directory was removed, as
observed after the run rather than assumed.

When a convention function's parameter does not bind, `SPX-WA102` lists
every name and type that is valid in that position: the entity's fields,
`me` and `my_<field>` in policies, and every rollup over the actual child
entities.

## Authority

As in v1, the projection only reads its source and writes the output
directory. The server reads and writes only its data directory and opens no
outbound connection. The self-test is the one exception that starts a child
process: it re-runs the same server file. The session cookie has no `Secure`
flag, because the server speaks plain HTTP. Put it behind TLS for anything
but loopback.

## Non-claims

No password reset, email, multi-factor sign-in, rate limiting, CSRF tokens
beyond SameSite=Strict, cross-row rules other than keys and rollups, schema
migration, browser-automation evidence, hosted evidence, or production
claims.

## Evidence

- **Unit tests:** `src/webapp/tests.rs` covers the v2 conventions (accounts,
  default and row-level permissions, keys, steps, filtered and summed
  rollups) and their `SPX-WA102` failures.
- **Benchmark self-test:** in `benchmarks/webapp-tokens-v2`, the generated
  20-entity TeamDesk Enterprise app passes its self-test across all four
  roles.
- **Negative controls:** while the runtime was developed, removing write,
  read, workflow, or key enforcement each made the self-test fail.
