# Task equivalence: `owned-byte-sentinel-balance-v1`

This held-out greenfield task measures a bounded byte transformation that
consumes an owned buffer, maps its two sentinel values, and reduces the
transformed bytes to a positional checksum.

## Problem and oracle

`sentinel_checksum(input)` accepts a finite byte sequence of at most eight
bytes and returns a nonnegative machine-sized integer. The transformation
maps `0xff` to `0x00`, `0x00` to `0xff`, and every other byte to `0x01`. The
required result is the one-based positional checksum of the transformed
sequence:

```
(1 * transformed[0]) + (2 * transformed[1]) + ...
```

The public vectors contain no zero bytes, so a plausible implementation that
maps `0xff` but forgets to map `0x00` passes every public assertion. Hidden
vectors include both sentinel values and must fail that implementation. The
hidden assertion modules contain the expected scalar answers directly; they
do not import or execute a second reference implementation, and they are
overlaid only after the public phase.

SEMAPRAX's owning operation is private to the application module because
imported functions with owned parameters are outside the admitted public
Project boundary. The cross-module `evaluate` wrapper takes a borrowed
`Slice<u8>`,
copies it to `Bytes`, and calls the private
`sentinel_checksum(input: own Bytes)` function. That function allocates an
owned output buffer, writes the mapped bytes through the admitted `bytes_set`
replacement chain, and computes the checksum from the frozen result. Rust
clones into a `Vec<u8>` before mapping; TypeScript copies into a `Uint8Array`.
All three ports therefore perform the same owned-byte transformation and
return the same scalar values.

## Ports and overlays

Rust, TypeScript, and SEMAPRAX Project ports keep the candidate implementation
in a public source file and overlay only the assertion entry point for hidden
scoring. The hidden entry imports the copied public candidate, so changing or
removing visible assertions cannot change hidden acceptance.

| Language | Invocation | Success signal |
| --- | --- | --- |
| Rust | `rustc --edition 2021 --test main.rs -o test_bin`, then `./test_bin` | exit code `0` |
| TypeScript | `tsc --strict --target ES2020 --module commonjs index.ts`, then `node index.js` | exit code `0` |
| SEMAPRAX Project | `semaprax run .` | stdout `0` |

The adapters execute the candidate and fixed assertions through each language's
ordinary test entry. Runner timings include the configured process and tool
steps; this task does not claim isolated candidate-function timings.
Implementations may use ordinary loops, standard iterator primitives and
byte-slice operations. They must implement the stated sentinel mapping and
positional checksum rather than print a fixed answer.

This fixture is held out. Its vectors and repair transcript must not be used
to tune a language adapter or admitted feature set. This commit supplies
corpus and oracle evidence only; no coding-agent trial was run or claimed.

## C, Python, Swift, and Java ports

Added under the `runnable_adapter_v2` extension. C, Python, and Java keep
the same candidate/entry split the Rust and TypeScript ports use
(`candidate.c`/`candidate.py`/`Candidate.java` hold the unchanged
`sentinel_checksum` function, copying its input into a private owned
buffer before mapping in place, mirroring the Rust reference's
`input.to_vec()`; the hidden overlay replaces only
`main.c`/`digest.py`/`Main.java`, via `#include`, `import`, and javac's
same-directory auto-discovery respectively). Java represents each byte as
an `int` constrained to `0..255` rather than a signed `byte[]`, avoiding
sign-extension pitfalls for values at or above `0x80`; this is a
language-idiom choice, not a difference in what is computed. Swift's fixed
single-file `swiftc main.swift` invocation admits no candidate/entry split,
so `sentinelChecksum` is repeated verbatim in both the public and hidden
`main.swift`.

Each port was authored independently against this file's sentinel-mapping
and positional-checksum contract and the Rust/TypeScript references, not
transliterated line-by-line, and was independently compiled/run against
the public and hidden vectors above. None of the three public vectors
contains a `0x00` byte, so each port was then checked against a
deliberately mutated candidate that drops the `0x00 -> 0xff` mapping
(treating `0x00` the same as any other non-`0xff` byte, i.e. `0x01`): the
mutant passes every public vector unchanged and fails both hidden vectors
(`8`/`12` instead of the correct `1024`/`1536`) in all four languages,
confirming the hidden vectors are non-vacuous.
