# Agent response project

An offline consumer of the bundled JSON slice. It validates an agent-style
JSON response and renders a structured verdict, with no allocation, no
network, and no filesystem authority.

```sh
semaprax check examples/agent-response-project/semaprax.toml
semaprax test  examples/agent-response-project/semaprax.toml
semaprax run   examples/agent-response-project/semaprax.toml
```

## What it demonstrates

- A `[dependencies]` edge on a bundled standard-library package. The manifest
  names `std.data.json.doc = "=0.1.0"`; the compiler adds that package's
  immutable source to the authenticated workspace and links by stable identity.
  Nothing is fetched and no cache is read.
- **Validation an agent actually needs.** `agent.verdict.accepts` requires the
  response to be one whole JSON document with a bounded nesting depth and no
  trailing bytes (`std.data.json.doc.is_document`), with **no repeated member
  name in any object** (`std.data.json.doc.is_unique`), and to carry a
  top-level `"status"` whose value is exactly `"ok"` plus a top-level
  `"answer"` member.
- **Member lookup without a map.** `agent.scan` walks the top-level object's
  members through `std.data.json.doc.next_key`, comparing each name span with
  the wanted name byte for byte. It retains nothing and returns only offsets
  under the family result encoding: `r <= byte_len(input)` is a position,
  `r > byte_len(input)` is "absent".
- **A pull-based structured result.** `agent.report.report_len(response)` is
  the exact length of the JSON verdict and `report_byte(response, index)` is
  its byte at `index`, or `-1` past the end. The verdict is
  `{"accepted":true}` or `{"accepted":false}`. Nothing is buffered, because the
  language has no growable collection to buffer into.

## Limits this example is shaped by

`agent.report.members` counts the top-level object's members and is a web
export, but it is deliberately **not** part of the rendered verdict, and the
count saturates at 99. Both facts come from the same place: the workspace
semantic graph pre-bound (`SPX-G171`) is charged against the whole link
closure, and this project's closure already carries the 10.3 KB document
layer. Measured by padding this project with `accepts`-shaped probe functions
until the bound fires, its own five modules are admitted to **8,305 B** and
rejected at **8,645 B**; they are **7,285 B**, so about **1.0 KB** remains.
Adding a second dependency such as `std.data.json.digits` costs far more than
it saves: the same probe against a consumer holding both packages admits only
about 5.8 KB of its own source, which is why the small decimal helper here is
local rather than borrowed from the sibling that owns exact `i64` rendering. [Bounded JSON Scanner v1](../../docs/BOUNDED-JSON-SCANNER-V1.md)
records the measurement.
