# Doctor offline bundle v1

Status: authored, unrun private inventory parsing and preparation. No CLI
activation, production profile provisioning, executable isolation, or WP-05
promotion.

Audience: CLI/platform contributors and reviewers.

## Boundary

`DoctorOfflineBundle::parse` consumes one [sealed-input snapshot](DOCTOR-SEALED-INPUT-V1.md)
and an explicit profile selector. It binds the encoded selector to that argument
and the encoded architecture to the compiled native Linux host. The returned
opaque bundle owns the input and bounded range indexes. Read-only file/tool views
borrow path and payload slices from that retained input; they cannot outlive it.
No second payload copy or serialized HIR is involved. The unsafe-free parser
module lives in the sys quarantine so internal root preparation consumes its
opaque result directly; the safe platform facade delegates through a wrapper.
There is one validator, not separately compiled production copies. File-view
accessors retain the bundle borrow even after the temporary view is dropped.

This is structural admission of untrusted content, not trusted provenance or
permission to execute. The parser performs no filesystem access, process launch,
environment discovery, publication, or fallback. It does not convert a bundle
into an admitted `DoctorHost`. The [real doctor CLI](DOCTOR-PROBE-V1.md) continues
to report unavailable production profiles.

## Pure provisioner preparation

`encode_doctor_offline_bundle(architecture, selector, entries, roles, max_bytes)`
prepares the existing binary format from explicitly supplied borrowed inputs.
Each `DoctorOfflineBundleEntry` contains a relative `path`, a borrowed `bytes`
slice and an `executable` intent. `DoctorOfflineBundleRoles` explicitly supplies
optional `clang`, `node` and `rustc` indices into that same entry slice. The
encoder does not sort entries, infer roles, follow paths, discover dependencies,
or repair invalid input. The existing exact basename and interpreter rules apply.

The caller may lower but never widen the 512 MiB carrier ceiling. A zero
`max_bytes` is `Invalid`; a value above the ceiling is `Limit`, before examining
input records. File count, selector/path lengths, cumulative path bytes and
every header/record/content contribution are bounded before reserving the output.
The encoder requests one fallible reservation for the complete measured carrier,
then passes its complete bytes through the existing full inventory validator
before returning them. Invalid roles, ordering, paths, minimum ELF or interpreter
closure can therefore fail after a bounded buffer has been allocated and filled;
this is not a claim of complete validation before payload copying. No partial
output is returned. Existing decoder acceptance and diagnostic precedence remain
unchanged.

The return value is only `Vec<u8>`, not `DoctorOfflineInput`, an opaque admitted
bundle, a filesystem object or execution authority. Callers retain their original
input buffers unchanged. Either named Linux architecture may be serialized on
any host where this private API is available; that does not widen native-host
sealed acquisition or bundle admission. The provisioner must still arrange
immutable storage, reacquire through the sealed-input boundary and parse against
the actual native host and selected profile. It must separately authenticate
provenance, loader closure and the complete launch context.

An already parsed bundle can prepare a matching request using
`encode_worker_request(target, nonce)`, as specified by the
[worker request contract](DOCTOR-OFFLINE-WORKER-V1.md#canonical-request-preparation).
There is no arbitrary-byte constructor that skips sealed-input acquisition.

## Closed binary format

All multibyte integers are unsigned little-endian. There is no compression,
padding, footer, checksum field, alternative encoding, or trailing data. The
input's complete length, including framing, is bounded by the sealed-input
512 MiB ceiling. The fixed header is exactly 28 bytes:

| Offset | Bytes | Meaning |
| --- | --- | --- |
| 0 | 8 | Literal `SPXDOC1` followed by NUL |
| 8 | 1 | Architecture: 1 = native64 little-endian Linux x86-64; 2 = Linux AArch64 |
| 9 | 1 | Nonzero role mask, only bits 0 (Clang), 1 (Node), 2 (Rust) admitted |
| 10 | 2 | Selector byte length, 1–64 |
| 12 | 4 | File count, 1–4,096 |
| 16 | 4 | Clang file index |
| 20 | 4 | Node file index |
| 24 | 4 | Rust file index |

The exact selector bytes follow the header and match
`[a-z][a-z0-9-]{0,63}`. A role absent from the mask must have index `0xffffffff`;
a present role must name an in-range file. Role order is fixed, not inferred
from inventory names. Its selected file must be declared executable and have
the exact final path component `clang`, `node`, or `rustc`, respectively.
These distinct required basenames also prevent two roles selecting one entry.

Exactly `file_count` records follow, in strictly increasing raw ASCII path order:

| Bytes | Meaning |
| --- | --- |
| 2 | Path byte length, 1–1,024 |
| 1 | Kind: 0 = data; 1 = executable intent |
| 1 | Reserved, exactly zero |
| 8 | Content byte length |
| path length | Exact relative ASCII path |
| content length | Opaque file content |

An executable intent is not an OS permission grant. Data files may be empty;
executable entries must satisfy the minimum ELF checks below. There are no
directory records, symlinks, hardlinks, devices, ownership/mode metadata,
timestamps, arbitrary arguments, or environment assignments. A provisioner must
materialize any needed alias as an ordinary file under the exact canonical name;
the parser never follows a host symlink to obtain bytes.

## Paths, bounds and binding

Paths are relative slash-separated components using only ASCII letters, digits,
dot, underscore, plus and hyphen. Components must be nonempty, neither `.` nor
`..`, at most 255 bytes, and at most 32 components deep. Leading/trailing or
repeated slashes, backslashes, colons, controls and non-ASCII bytes reject.
Case is significant; these are Linux inventory names, not Windows host paths.
The sum of all encoded file-path bytes is at most 1,048,576.

Duplicate and unsorted paths reject. No file may also be a directory prefix of
another file. Every slash prefix is looked up in the sorted inventory; an
adjacent-only comparison is insufficient for names such as `a`, `a-`, `a/x`.
Missing directories are only implied inventory structure, never host directories.

Every integer conversion, cursor advance, program-table range and payload range
is checked before slicing. Declared counts and byte lengths are bounded before
allocations or content access. Two metadata vectors reserve fallibly, each with
at most 4,096 entries: the retained file-range index and temporary borrowed ELF
interpreter references. The latter is discarded after validation. No path or
payload strings are cloned into the index.

Caller selector grammar is checked before host support or wire parsing. The
public parser supports the same native64 Linux x86-64/AArch64 configurations as
the sealed-input reader. Private structural tests inject an expected architecture
to cover both encodings without claiming execution on either target.

## Minimum ELF and interpreter closure

Every executable entry is checked, including entries not selected by a tool
role. It must contain an ELF64 little-endian header with both version fields 1,
type ET_EXEC or ET_DYN, the declared machine (62 or 183), header size 64, program
entry size 56, and 1–128 bounded program headers.

At most one PT_INTERP is allowed. Its bounded content is 3–1,026 bytes: an
absolute UTF-8 path with exactly one terminal NUL and no embedded NUL. Strip
exactly one leading slash, then apply the same relative path grammar and limits.
That exact path must name another executable inventory entry. The interpreter
entry must itself have no PT_INTERP. Self-reference, cycles, chains, missing
interpreters and data-only interpreters reject without recursive traversal.

This minimum inspection is deliberately not complete ELF validation: section
tables, OSABI, entry addresses, load-segment semantics and dynamic dependencies
are not attested. It does not prove kernel loadability, compatible libraries,
DT_NEEDED/configuration closure, successful version execution, or sandboxing.
It also does not make file names safe to use outside a separately admitted
private root. Future execution must use an independently reviewed contained
native loader route, never an ambient shell, interpreter search, or binfmt
fallback to compensate for rejected inputs.

## Errors and evidence

Malformed framing/grammar/roles/minimum ELF or interpreter structure is
`Invalid`. Exceeded carrier/count/path/component/depth bounds are `Limit`;
zero selector/file/path lengths are `Invalid`. An encoded selector length over
64 is `Limit`; malformed caller selectors are always `Invalid`. Minimum ELF
bound violations remain `Invalid`, not a second wire resource-error family.
Known but mismatched selector/architecture bindings have distinct
`SelectorMismatch`/`ArchitectureMismatch` results. Unknown architecture tags
are `Invalid`, unsupported compiled hosts return `Unsupported`, and failed
metadata reservations return `Allocation`. No error exposes a partial bundle.

Authored structural fixtures cover both architectures, all role bindings, exact
header bytes, every truncated prefix, unknown fields/trailing data, extreme
lengths, 4,096/4,097 files, exact/plus-one path/component/depth/cumulative limits,
nonadjacent prefix collisions, and malformed or chained interpreters. Literal
ELF fixtures independently exercise both version fields, type/machine/header
and program-table bounds, truncation/overflow, and interpreter framing. These
are structural byte samples, not runnable distributions.

A sys-crate fixture passes real sealed memory-file snapshots into the production
parser, checks zero-copy view addresses and retained bytes after dropping the
original file, and rejects wrong selector/architecture bindings. The former
cross-crate source include is removed. Separate safe-facade type checks cover
the exported API and file-view lifetimes. No test-only public input constructor
or new dependency is introduced.

Preparation regressions compare complete encoded carriers against independent
literal framing for both architecture tags and all role masks. They cover exact
caller-lowered bounds, malformed inventory, ordering and interpreter rejection.
Near-ceiling length-only arithmetic cases do not establish successful physical
allocation of a 512 MiB carrier. Request regressions acquire real sealed inputs,
derive every target/mask combination, preserve source offsets and seals, retain
bindings after the original file is dropped, and reject missing roles or zero
nonces. Same-length payload, path and selector changes exercise exact-byte
binding; nonzero nonce coverage is not evidence of freshness.

The targeted structural and sealed-input gates are authored but unrun:

```sh
cargo test --locked -p semaprax-native-rust-interop-platform-sys --lib doctor::offline_bundle::encode
cargo test --locked -p semaprax-native-rust-interop-platform-sys --lib doctor::offline_input::request_handoff
```

The collector's `tests/support/prepared_handoff.rs` adds an ignored physical
handoff gate: independently literal-checked preparation, sealed acquisition,
worker execution and exact report delivery for native/all targets, plus
unrepaired request-digest and bundle-payload drift rejection. It requires the
same external trusted context, executable paths and serial invocation as the
[provisioned collector fixtures](DOCTOR-OFFLINE-COLLECTOR-V1.md#evidence-and-non-claims).
Prepared bytes alone do not prove immutable storage or successful execution.

The separate internal [detached root materializer](DOCTOR-OFFLINE-ROOT-V1.md)
consumes this opaque inventory inside an already controlled child context.
It remains unconnected to production launch or profile admission.

All fixtures remain unrun. Existing sealed-input, CLI report, profile-selection
and lower-level probe fixtures remain unchanged and required. Completion still
needs real provisioning, immutable executable/library/configuration inputs,
OS filesystem/IPC/network containment, descendant settlement, selector-to-host
admission, and physical selected-tool compatibility at the release head.
