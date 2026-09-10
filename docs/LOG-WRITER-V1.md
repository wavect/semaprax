# Log Writer v1

Status: implemented additive source profile; **HOSTED GREEN** under the
[v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md).

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
filesystem/process/network effect, or other ambient authority. This is a
bounded JSON-lines event writer, not a general logging framework, hosted log
service or production logging facility.

## Level filtering

Filtering is the additive half of the same profile, and it is explicit rather
than ambient:

```text
std.log.level_enabled(level: u8, threshold: u8) -> bool
std.log.event_admitted(event: borrow Event, threshold: u8,
    output: borrow Writer) -> bool
std.log.discard_event(event: own Event, output: own Writer)
    -> std.io.Writer
std.log.append_event_if(event: own Event, threshold: u8,
    output: own Writer) -> std.io.Writer
```

Levels run `0` (trace) to `5` (fatal), and an event is enabled when its level
is at or above the threshold, so a threshold of `0` admits everything and `5`
admits only fatal events. `event_admitted` is the borrowed observer a caller
checks before committing: it is true only when the event both passes the
threshold and fits the writer's live capacity.

`append_event_if` writes the event exactly as `append_event` does when it
passes, and otherwise consumes the event and returns the caller's Writer
untouched, with no byte written and the cursor unchanged. Capacity is therefore
required only for an event that is actually written: a filtered event needs no
room at all, which is what makes a small buffer plus a high threshold a valid
composition rather than a contract failure. `discard_event` is that drop path
named on its own, for a caller that decides policy itself; both transitions
consume the event, so a dropped event releases its `name` and `message` bytes
through ordinary lexical cleanup rather than leaking them.

A filtered event is dropped, not buffered: the profile adds no queue, no sink,
no timestamp source, no redaction, and no concurrency. Nothing here observes a
clock or a process, and no filtering decision is taken outside the caller's
own call.

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

The maintained focused selectors are:

```text
logging::log_writer_executes_on_all_three_backends
logging::log_writer_preflight_rejects_invalid_events
logging::utf8_ascii_scan_preserves_package_conformance
```

The original local witness exercised the shipped package and all fifteen cases
on the interpreter, C11 at `-O0` and `-O2`, and four repeated Core Wasm
invocations. The Wasm harness enforces each case's exact live Bytes bound:
zero for pure helpers, one or two for individual operations, and three for a
complete event and Writer. Every invocation returns with no live Bytes.
Nine malformed UTF-8, invalid level, short-capacity, and forged-cursor cases
passed twice through the bundled consumer with the exact `requires`-false
status. The UTF-8 package's existing conformance also passed on all three
backends, together with catalog, metadata, formatting, document-link and
module-size checks.

The released implementation is hosted green; the earlier local counts remain
historical observations. Broader Everyday standard-library completion remains
`Partial`: general logging facilities, sinks, filtering, timestamps and
concurrency are outside this source-writer slice.
