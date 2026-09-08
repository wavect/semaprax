# Log Writer v1

Status: additive source implementation; focused local verification passes.

Audience: standard-library contributors, compiler maintainers, and backend
implementers.

`std.log` provides a bounded structured JSON-lines writer for one caller-owned
event. It composes the existing `std.io.Writer`, JSON quoting, and decimal
formatting helpers without ambient effects or hidden allocation.

## Event and output

The public source declarations are:

```text
std.log.Event {
    level: u8,
    sequence: usize,
    name: Bytes,
    message: Bytes,
}

std.log.append_event(event: own Event, output: own std.io.Writer)
    -> std.io.Writer
```

Levels are encoded as `trace` (0), `debug` (1), `info` (2), `warn` (3), and
`error` (4); level 5 is `fatal`. The writer emits one complete JSON object and
a trailing line feed:

```json
{"level":"info","sequence":7,"event":"a\"","message":"\n"}
```

`name` and `message` are quoted with the existing JSON writer policy. The
sequence is decimal ASCII. `level_len`, `level_byte`, `fixed_len`, and
`fixed_byte` are source-visible checked helpers for the fixed punctuation and
level words; `event_json_len` computes the exact complete output length.

## Validation and ownership

Before the first output write, `append_event` validates the level, validates
the UTF-8 content of `name` and `message` through `std.data.json.utf8`, and
checks the Writer position and complete remaining capacity. A malformed event,
invalid level, or short/forged output Writer is rejected without partial JSON,
cursor advancement, or published replacement Writer. The event and Writer are
consumed together at the ordinary owned-call boundary; successful output
advances by exactly the emitted bytes and preserves the unwritten suffix.

The package uses the private `useful-data.v2` profile and exactly the bundled
`std.data.json.utf8`, `std.data.json.write`, and `std.io` dependencies. It no
longer depends on `std.format`; `count_into` and `usize_len` come directly from
the JSON writer package. It has no public exports, hidden allocation,
filesystem/process/network effect, or other
ambient authority. This is a bounded JSON-lines event writer, not a general
logging framework or hosted/production logging claim.

## Focused verification

The canonical package checks a complete 59-byte escaped line and all five
unwritten zero suffix bytes. Fifteen expanded cases are kept in
`tests/project/standard_library/log_cases.spx`; each case preserves the exact
local call closure and required type imports. They include all six levels,
valid two-, three-, and four-byte UTF-8, exact capacity, prefix/suffix
preservation, and a fully checked 300-byte message. The unchanged canonical
package also runs. ASCII scanning uses the existing byte-range predicate
directly instead of probing each byte numerically; multibyte validation and
the interpreter fuel limit are unchanged. An explicit inventory requires
every case and direct executed coverage of every public source function.

Focused local gates pass:

```text
logging::log_writer_executes_on_all_three_backends
logging::log_writer_preflight_rejects_invalid_events
logging::utf8_ascii_scan_preserves_package_conformance
```

The shipped package and all fifteen cases pass on the interpreter, C11 at
`-O0` and `-O2`, and four repeated Core Wasm invocations. The Wasm harness
enforces each case’s exact live Bytes bound: zero for pure helpers, one or
two for individual operations, and three for a complete event and Writer.
Every invocation returns with no live Bytes. Nine malformed UTF-8, invalid
level, short-capacity, and forged-cursor cases pass twice through the bundled
consumer with the exact `requires`-false status. The UTF-8 package’s existing
conformance also passes on all three backends. Catalog, metadata, formatting,
document links, and module-size checks pass.

Broader Everyday standard-library completion remains `Partial`; general logging
facilities, sinks, filtering, timestamps, concurrency, and hosted execution
are outside this slice.
