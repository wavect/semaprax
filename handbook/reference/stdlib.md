# Standard library

The standard library ships inside the compiler: no checkout, cache or network.
You add a dependency, import by stable id, and set the project profile the
package requires.

## Find a function

Do not memorize names. Ask the installed compiler:

```sh
semaprax help library                    # the whole catalog, offline
semaprax help library compare            # one entry, about 200 bytes
semaprax help library std.core.compare   # by stable id
```

An entry shows the stable id, dependency row, required profile, signature,
effects and contracts. The generated catalog is
[Standard library catalog](https://github.com/wavect/semaprax/blob/main/docs/STANDARD-LIBRARY-CATALOG.md);
the machine-readable form is `std/catalog.json`.

## Use a package in three steps

```toml
# 1. semaprax.toml: depend on the package
[dependencies]
std.core = "^0.1.0"

# 2. set the profile the entry requires (omit it for `scalar`)
[package]
profile = "owned-data-api.v1"
```

```semaprax
// 3. import by stable id, right after the module line
module app.using_std;

use function @id("std.core.min") from std.core as min;
```

Every bundled package is at version `0.1.0`. An unknown package or an
unsatisfied range fails with `SPX-J121`. If you call a standard-library name
without importing it, the error prints the exact dependency and `use` lines to
add (for example for `min`). Spell the type argument of generic calls
(`vec_push<i64>(...)`). Pick the profile with
[Choose a project profile](../projects/profiles.md).

## Every package

47 packages, all marked `partial` in the catalog: each works on the interpreter,
native C11 and Core Wasm for its listed profile, and each has a conformance
module. "Functions" counts declarations in the catalog entry.

| Package | Profile | Functions | What it gives you |
| --- | --- | ---: | --- |
| `std.agent` | `owned-data-api.v1` | 17 | Task, Context, Observation and Outcome records, plus stage-transition and retry helpers for agent loops. |
| `std.async` | `useful-data.v1` | 6 | Wait and retry arithmetic: clamp a wait, next timeout, remaining time, stream end. |
| `std.auth` | `owned-data-api.v1` | 48 | Secret, Identity and Authorization records, constant-time byte compare, session-state rules (expiry, rotation). |
| `std.bytes` | `useful-data.v1` | 20 | Byte-slice helpers: `get_or`, `index_of`, `count`, prefix and suffix tests, u16/u32 reads, trimming, fields. |
| `std.collections` | `owned-data-api.v1` | 8 | Bounded `Vec` operations over Copy scalars: `with_capacity`, `push`, `len`, `capacity`, `get`, `set`, `clear`, `reserve_exact`. |
| `std.core` | none (`scalar`) | 12 | `compare`, `min`, `max`, `clamp`, `in_range`, bool and i64 conversion, `xor`, `implies`. |
| `std.data.csv` | `useful-data.v1` | 8 | CSV field scanning, quote balance and record well-formedness. |
| `std.data.json` | `useful-data.v1` | 12 | JSON scanning primitives: whitespace, hex, escapes, string ends, failure offsets. |
| `std.data.json.dec` | `owned-data-api.v1` | 27 | Decode JSON strings (escapes, UTF-8) into an owned buffer and compare decoded tokens. |
| `std.data.json.digits` | none (`scalar`) | 5 | Decimal digit helpers for JSON numbers. |
| `std.data.json.doc` | `useful-data.v1` | 19 | Whole-document JSON structure: document end, key iteration, key uniqueness. |
| `std.data.json.token` | `useful-data.v1` | 13 | JSON number and literal tokens: integer, fraction and exponent ends, `true`/`false`/`null`. |
| `std.data.json.utf8` | `useful-data.v1` | 11 | UTF-8 validation for JSON text. |
| `std.data.json.write` | `useful-data.v2` | 16 | Write JSON: quoted strings, escapes and decimal numbers into a buffer. |
| `std.data.toml` | `useful-data.v1` | 16 | TOML scanning: bare and quoted keys, values, comments, failures. |
| `std.db` | `useful-data.v1` | 18 | Database-access rules: descriptor matching, safe identifiers, transaction state machine. |
| `std.email` | `useful-data.v2` | 32 | Email address, header and envelope validation. |
| `std.encoding` | none (`scalar`) | 10 | Hex and Base64 digit encode and decode helpers. |
| `std.encoding.base64` | `owned-data-api.v1` | 3 | Base64 length and byte access for an owned buffer. |
| `std.env` | `environment-io.v1` | 7 | Read process environment entries: count, name, value. |
| `std.env.policy` | `owned-data-api.v1` | 12 | Validity rules for environment variable names and assignments. |
| `std.export.policy` | `useful-data.v2` | 9 | Admission rules for export batches: sizes, target ids, queue depth, backoff. |
| `std.format` | `useful-data.v2` | 14 | Build text in a buffer: append str, usize, i64 and bool, with padding. |
| `std.fs` | `filesystem-io.v3` | 22 | Typed `Path`, `FileInfo` and `WriteOutcome` over the `fs.*` effects: read, write, metadata, list, create, remove, atomic write. |
| `std.http` | `useful-data.v1` | 58 | HTTP/1.1 message parsing: status, headers, `Content-Length`, method and token validity. |
| `std.io` | `owned-data-api.v1` | 13 | `Reader` and `Writer` cursors over byte buffers. |
| `std.io.lines` | `owned-data-api.v1` | 7 | Line-oriented reading over a `Reader`. |
| `std.jobs` | `useful-data.v1` | 29 | Durable-job state machine: states, leases, claim, heartbeat, retry and dead-letter rules. |
| `std.log` | `useful-data.v2` | 27 | Structured JSON log events with levels and guarded append. |
| `std.log.redact` | `useful-data.v2` | 13 | Log redaction policy: protected field names and safe events. |
| `std.mem` | `owned-data-api.v1` | 3 | Owned `Box`: `new`, `get`, `into_inner`. |
| `std.metrics` | `useful-data.v2` | 44 | Counters, gauges, histogram observation, label and cardinality rules. |
| `std.net` | `useful-data.v1` | 24 | Network value checks: ports, hosts, IPv4 classes (loopback, private, link-local), wait results. |
| `std.num` | none (`scalar`) | 15 | `abs`, `sign`, `gcd`, `pow`, `isqrt`, `div_euclid`, `rem_euclid`, `digit_count`, `log2_floor`, `log10_floor`. |
| `std.num.overflow` | none (`scalar`) | 13 | Overflow detection plus wrapping and saturating add, sub, neg, mul. |
| `std.path` | `useful-data.v1` | 6 | Path text queries: absolute, segments, file name, parent, extension. |
| `std.path.normalize` | `owned-data-api.v1` | 17 | Normalize a path (resolve `.` and `..`) into an owned buffer. |
| `std.path.value` | `owned-data-api.v1` | 16 | Owned `Path` value: validation, join, parent. |
| `std.process` | `process-io.v1` | 29 | `Argv` and `Output` records and `run` for subprocesses. |
| `std.random` | none (`scalar`) | 4 | Deterministic seeded generator: `next_seed`, `sample_below`. |
| `std.test` | none (`scalar`) | 10 | Assertion helpers: `equal_i64`, `equal_bool`, failure bit sets. |
| `std.test.bytes` | `useful-data.v2` | 8 | Byte assertions and snapshot comparison. |
| `std.text` | `useful-text-consumer.v1` | 5 | Byte length, `contains`, `equals`, `is_empty`, `starts_with` over borrowed text. |
| `std.time` | none (`scalar`) | 8 | Millisecond and second arithmetic: deadlines, remaining and elapsed time. |
| `std.tracing` | `useful-data.v2` | 42 | W3C `traceparent` and `tracestate` validation. |
| `std.url` | none (`scalar`) | 5 | URL scheme and percent-encoding byte predicates. |
| `std.webhook` | `useful-data.v2` | 23 | Webhook admission: replay window, signature shape, delivery, backoff and idempotency rules. |

`scalar` packages work in any project. Other profiles gate what a project
may contain, for example `std.fs` needs `filesystem-io.v3` and `std.process`
needs `process-io.v1`, and those also need the matching effect and a provider
that grants it ([Input and output](../language/io.md)).

## Traps

- Mutators thread the owner. Write `let next = vec_push<i64>(values, x);`; there
  is no in-place mutation.
- Most `std.*` packages are rule and parsing helpers over byte slices and
  scalars, not high-level clients. `std.http` parses messages; it does not open
  connections. For network access use the compiler-owned `net_*` and `https_*`
  functions ([Built-in functions](builtins.md)).
- Compiler-owned functions (`string_len`, `byte_get`, `stdout_write`, `box_new`)
  are not in the standard library. They are reserved names in every file.
- A stable id is `<package>.<name>`, for example `std.core.min`.

Contract: [Standard Library v1](https://github.com/wavect/semaprax/blob/main/docs/STANDARD-LIBRARY-V1.md).
