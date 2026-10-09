# Catalog restock command v1

This independent manual application contract is frozen before qualification. It
is not a replacement for ShiftSim, LogLens or their acceptance requirements.

Read one complete UTF-8 JSON object from stdin with exactly `tags` and `items`.
`tags` is an array of 0..8 unique identifiers. `items` is an array of 0..256
objects with exactly `id`, `department`, `priority`, `stock`, `topup`, and `mark`.
All identifiers are unique within their own array and match ASCII
`[A-Za-z0-9_-]{1,16}` after JSON escape decoding. Item `department` and `priority`
are JSON integers in 0..9; `stock` and `topup` are integers in 0..1000; `mark` is
an integer in 0..255. Booleans, fractions, exponents and null are not integers.
Unknown, repeated or missing keys, invalid UTF-8/JSON and invalid values refuse.

With empty tags, select every item. Otherwise select items whose decoded id
starts with at least one decoded tag. Validate all items before selection,
including those a tag would exclude. Replace each selected item's stock with
stock + topup. Sort selected items by department ascending, priority ascending,
and unsigned decoded identifier bytes ascending. Identifier uniqueness makes
the remaining values irrelevant to ordering. Never use raw JSON token spelling
or input position as a tie-breaker.

Emit exactly `{"items":[...],"metrics":{...}}` plus one newline. Each item has,
in order, `id`, `department`, `priority`, `stock`, `mark`. Metrics have, in order,
`selected` (count), `stock` (sum after restock), and `checksum` (sum of mark
bytes). Strings use ordinary JSON quoting; values are canonical decimal.
Empty selection emits empty items and three zero metrics. Successful status is
0 with no stderr. Invalid requests return 2, emit exactly
`invalid catalog request\n` to stderr and no stdout. Ordinary provider/runtime
failures are distinct from application-invalid input.

Whitespace is allowed anywhere JSON permits it, with no raw-input byte limit.
The existing stream provider, grammar/fuel, collection, String/Bytes and output
limits remain authoritative. The largest valid report stays below 65536 bytes.
Runtime items own a String id and scalar mark. Ordered publication fragments
own a String label and Bytes payload, with allocation/clone/replacement outside
loops; native source must use
the independently checked explicit `language-command-io.owned-data.v1` profile,
the same four command grants and external `fn() -> i64` ABI. No schema or source
generator identity grants runtime authority.
