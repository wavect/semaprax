## Web applications

`semaprax webapp app.spx -o out` turns one module into a full-stack web app:
REST API, persistence, browser and server validation, computed fields, and a
UI (dashboard, searchable/sortable/filterable paginated lists, detail pages,
forms). No `main` or `@id` is needed. This example is checked by the tests:

```spx webapp
module shop;

variant Tier {
    Free,
    Pro,
}

record Customer {
    name: string,
    tier: Tier,
    seats: i64,
}
    requires string_len(name) >= 2 && string_len(name) <= 80
    requires seats >= 1

record Order {
    customer_id: i64,
    total: f64,
    paid: bool,
}

fn customer_large(tier: Tier, seats: i64) -> bool
{
    seats >= 100 || tier == Tier::Pro {}
}

fn order_status(paid: bool) -> string
{
    if paid { "paid" } else { "due" }
}
```

- Each `record` is an entity at `/api/<snake_name>`. Field types: `string`,
  `i64`, `f64`, `bool`, `char`, payload-free variant. The server assigns `id`.
- `customer_id: i64` references `Customer`: select input, missing target
  rejected, deleting a referenced row is 409.
- Each `requires` line after a record is a validation rule (a record
  invariant). Any `fn <entity>_<name>(…)` is a computed field `<name>`.
  `<entity>` is snake_case (`time_entry`) or lowercase (`timeentry`).
  Parameters are that entity's fields, same name and type.
- Bodies: `let`, `if`/`else`, `match` (`A {} | B {}` joins payload-free
  cases), `==` on enums, arithmetic, comparisons, `&&` `||` `!`, `string_len` (bytes),
  `string_len_chars`, `string_is_empty`, `string_contains`,
  `string_starts_with`, `string_concat`, `string_from_i64`, and helper
  functions. No `i64` to `f64` cast: use a recursive helper. Errors:
  `SPX-WA102` type, `SPX-WA103` expression, `SPX-WA105` uncalled function.
- Unique key: `fn customer_key(email: string) -> string { email }` (any
  scalar; join fields with `string_concat`).
- Workflow on variant field `state`: `fn order_state_step(from: State, to:
  State) -> bool`. Rows start in the first case; updates must pass the step.
- Rollups: a computed field may take `count_<child>`, `count_<child>_<bool
  field>` (both `i64`), or `sum_<child>_<number field>` over the child rows
  that reference this row: `fn customer_spent(sum_order_total: f64) -> f64`.
- Accounts: `fn member_account(email: string, active: bool) -> bool {
  active }` makes `Member` the sign-in entity (login field first; the server
  keeps a write-only `password`). First run: `node out/server.mjs --setup`.
- Permissions: `fn <entity>_can_read` / `_can_write(…) -> bool` take row
  fields plus `me: i64` and `my_<account field>`. Unprefixed `can_read` /
  `can_write` (or `can_write_<name>`) are defaults; one taking row fields such
  as `member_id: i64` covers every entity with those fields, the most
  specific default winning. Audit history and CSV export are automatic.
- Cross-account permission evidence: run
  `checks/permission-api-self-test.mjs --arm semaprax --base-url URL` against
  a fresh server started with `--setup` and an empty disposable `--data` dir.
  This is an external API check, separate from the built-in `--self-test`: it
  signs in two Agent accounts, checks that each can read and update their own
  Task, Expense, and Leave, confirms the other Agent's Expense is hidden and
  not writable, and confirms the other Agent's Task and Leave remain readable
  but not writable. It does not exercise browser UI behavior. The same script
  supports the TypeScript reference arm with `--arm typescript`.
- Run `semaprax fmt app.spx && semaprax webapp app.spx -o out && node
  out/server.mjs --self-test` as one command. The self-test exercises every
  feature for every entity and role, prints the observed evidence, and ends
  with its own cleanup line, so no hand-written requests or process checks are
  needed. Fix a diagnostic with a targeted edit at its line, not by rewriting
  the file; `semaprax webapp app.spx --api` lists the generated API.
- API: `GET`/`POST /api/<entity>`, `GET`/`PUT`/`DELETE /api/<entity>/<id>`,
  `GET /api/<entity>/<id>/history`, `?format=csv`, `GET /api/audit`; with
  accounts `POST /api/session {"login", "password"}` and `DELETE
  /api/session`. `node out/server.mjs [--port N] [--data DIR] [--setup]`
  serves until killed, so start it in the background.
