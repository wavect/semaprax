## Web applications

`semaprax webapp app.spx -o out` turns one module into a complete web
application: a JSON REST API, file persistence, the same validation in the
browser and the server, computed fields, and a browser UI with a dashboard,
searchable, sortable, filterable, paginated lists, detail pages with
back-references, and forms. Start it with `node out/server.mjs --port 8080`.
The module needs no `main` and no `@id`; the projection names everything
after the source names. The block below is checked by the `webapp` tests.

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

record Order {
    customer_id: i64,
    total: f64,
    paid: bool,
}

fn customer_valid(name: string, seats: i64) -> bool
    requires string_len(name) >= 2 && string_len(name) <= 80
    requires seats >= 1
{
    true
}

fn customer_large(tier: Tier, seats: i64) -> bool
{
    seats >= 100 || match tier { Tier::Pro {} => true, _ => false, }
}

fn order_status(paid: bool) -> string
{
    if paid { "paid" } else { "due" }
}
```

- Each `record` is an entity, served at `/api/<snake_name>` and shown at
  `#/<snake_name>`. Fields are `string`, `i64`, `f64`, `bool`, `char`, or a
  variant whose cases have no payload (shown as a select). The server assigns
  `id`; do not declare it.
- A field `<entity>_id: i64`, such as `customer_id`, references that entity.
  Its form input is a select, a reference to a missing row is rejected, and
  deleting a referenced row returns 409.
- In `fn <entity>_valid(…) -> bool`, each `requires` line is one validation
  rule, reported on its first field. The body is `true`.
- Any other `fn <entity>_<name>(…) -> T` is a read-only computed field
  `<name>`.
- `<entity>` is the snake_case record name (`time_entry` for `TimeEntry`)
  or its lowercase (`timeentry`). Entity function parameters are fields of
  that entity, with the exact name and type, in any order. Functions without
  an entity prefix are helpers; one that no entity function calls is
  `SPX-WA105`.
- Bodies may use `let`, `if`/`else`, `match` with one variant case or scalar
  literal per arm, checked `i64` and IEEE `f64` arithmetic, comparisons,
  `&&`, `||`, `!`, `string_len` (bytes), `string_len_chars`,
  `string_is_empty`, `string_contains`, `string_starts_with`,
  `string_concat`, and `string_from_i64`. Anything else is `SPX-WA103`, and
  an unsupported field type or parameter is `SPX-WA102`.
- `Tier::Free {} | Tier::Pro {}` is `SPX-T254`: write one arm per case.
  There is no `i64` to `f64` conversion: multiply by a count with a
  recursive helper.
- Loop: `semaprax fmt app.spx && semaprax webapp app.spx -o out && node
  out/server.mjs --self-test`. The self-test synthesizes a valid row for every
  entity from its rules and checks create, read, list, update, validation,
  404, 409, persistence across a restart, and delete, printing one line. It
  replaces hand-written request scripts. Regenerating into the same `out` is
  allowed. Rows live in `./data/db.json` unless the server gets `--data DIR`.
