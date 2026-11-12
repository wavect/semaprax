# Invocation-selected file resource input v1

Status: focused native profile and LogLens qualification passed on macOS arm64.
Audience: compiler maintainers and bounded native CLI application authors.

The closed profile `source-command.selected-file.resource-output.v1` selects
Project v34 and input `argv-utf8-256+selected-file.follow.v1`, native64 Unix.
It combines the exact lazy held-file authority and named follow policy of
[Selected File Input v1](INVOCATION-SELECTED-FILE-INPUT-V1.md) with the existing
[Project v28](PROJECT-MANIFEST-V28.md) String and staged output resources.

## Exact bounds and authority

Application argv is forwarded unchanged: at most 256 arguments and 65536
aggregate UTF-8 bytes. The optional `--spx-selected-file-v1 INDEX --` host
prefix selects one of these arguments with canonical decimal INDEX 0..255.
The prefix consumes no application argument. No prefix gives no file grant.
Checked source alone validates complete application usage before the first
`file_read_text`; the launcher performs no option normalization, input
`dirname`, `cd` or `readlink` traversal, or application grammar parsing.

The selected spelling remains bounded at 4096 bytes. Acquisition/read/UTF-8,
regular-file, metadata drift, sticky failure, held descriptor, symlink policy,
no-reopen, reservation and settlement rules are identical to v33. File text
remains 65536 bytes, attempts at most 64 and reserved reads at most 1048576 bytes.
The source String and authenticated String-derived byte-view limit is 1048576
bytes. Combined staged stdout/stderr is 1048576 bytes, allocated as the existing
v28 two-channel heap envelope; owning Bytes and ordinary Slice bounds remain
frozen. Staging is erased and released after settlement and publication or
failure. Provider close failure cannot replace a selected root status. Output
allocation failure exits without root execution, file acquisition or publication.
Physical ordinary String allocation recovery remains separately incomplete.

The new native output classifier uses the exact same resource-provenance and
checked output helpers as v28. No new borrowed representation, public provider
ABI, persistent grant, filesystem write capability or optimistic metadata is
introduced. Interpreter, Wasm, Web, npm and non-native64 Unix hosts refuse.
V33 keeps its 16 argv/65536 output bounds; v26/v28 keep their 16 argv and existing
relative-root no-follow provider bytes. Profile selection is explicit in the
canonical manifest, graph, Project revision, lock and retained workspace route.

## Application and owning gates

`experiments/selected-file-input/loglens/` is a new adaptation of the preserved
qualified LogLens source. Its wrapper only locates its executable and grants
application argv 0. Source takes over the prior shell's filename-flag and ASCII
positive-top validation. Decimal arithmetic, parsing, reporting and every
retained assertion remain intact, including the 33000-digit report and 30
repeated options. The finite physical invocation bounds do not impose a numeric
byte-count ceiling inside the admitted file. Historical acceptance is retained
provenance, never acceptance for this adaptation or evidence of savings.

`project::source_command::selected_file_resource` passed at native O0 and O2
on compiler source `7d27641abb9a80b357beaf602b96ee3ffc4fd752`. It covered
the v34 graph and lock selectors, raw argv order, inclusive argv bounds,
66,000-byte output and the v33 refusals. At each optimization level,
`loglens_selected_file` passed all 33 historical checks and all 16 independent
SPEC boundary checks, the retained assertions and additive selected-path
controls. The public CLI independently checked the canonical manifest, ran the
declared native Project test and built the checked-in launcher fixture, which
also passed CLI49.

The optional Codex setup flag `--selected-file-resource-v1` authenticates
compiler help and snapshots the setup source and note hashes. Its tests cover
exact capability binding, drift refusal and preservation of the default setup
and TypeScript prompt. Fresh matched campaigns and physical allocation
recovery remain pending.
