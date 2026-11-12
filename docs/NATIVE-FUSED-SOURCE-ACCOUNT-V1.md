# Native fused source account v1

Audience: compiler contributors and reviewers.

Status: authored OPT #768 receiver; current-head executable qualification pending.
This contract applies only to the explicit source-only `fused-construction-v1`
profile. Ordinary source, Project, cache/graph imports and public providers do
not select it. The source façade parses the held raw source once and supplies
its SHA256 alongside the sealed, fully validated original-source selection.
The hash is publication identity, never selection or ownership authority.

## Caller funding

The caller supplies a positive finite managed-byte limit. No amount is inferred
from the source's required allocation, and the old interpreter test-only 64 MiB
reservation is not a runtime policy. Native64 prepays the closed invocation's
maximum requested heap bytes before program effects:

- the actual selected heap frame, including domain header and private carriers;
- the exact lease and borrowed-view tables and ordinary row-Vec backing;
- the one original Bytes payload backing;
- every original literal String request, including its actual length-header
  offset and terminator. Empty literals still allocate.

The emitted C asserts the String header offset against the actual emitted
runtime and the complete category sum against the selected layout mirrors.
The same account and issuer remain held across repeated invocations. Reservations
are cumulative and never refunded, even when a language failure prevents later
allocations. The existing structural ledger retains original allocation-boundary
debits as a subordinate settlement meter; it supplies no additional caller room.
The ordinary 32-event/2-MiB Bytes account remains independently enforced. The
profile's authenticated suffix permits no additional String materialization or
Bytes deep clone. Broadening it requires extending this funding inventory first.

`managed_reserved` reports this monotonic prepayment, **not** measured allocated
bytes, live memory, allocator overhead or RSS. The limit excludes compiler and
source storage, executable/loader memory, fixed caller/context/status/control
stack storage, C allocator bookkeeping, libc and output buffering. It does not
bound total process memory. Allocator failure or an invariant fail-stop remains
an administrative process failure; it does not become an invented language status.

## Receiving and publication

`codegen::emit_fused_source_profile` accepts only the immutable
`ValidatedFusedSelection`, explicit caller funding and the façade's raw-source
identity. It replays the current source/HIR join and emits the complete ordinary
prelude, selected function, private receiver and compiler-owned `main`. There is
no caller-authored granting wrapper, graph selector or public carrier ABI.

The entry leaves the result poisoned until canonical cleanup, postconditions and
selected result checks all succeed. The wrapper emits one deterministic JSON
`semaprax.fused-source.v1` envelope with profile, source SHA256, target `native`,
account limit and reserved bytes. Exactly one outcome is emitted:

- `returned`, with the i64 result as a canonical signed-decimal JSON string
  (including the full i64 range without JSON-number precision loss) and process exit 0;
- `language_failure`, with the actual normalized domain/code semantic pair and
  process exit 1; this pair is not a claim to serialize every status-detail field;
- `administrative_refusal`, with a private reason and process exit 2, without a
  result or fabricated language status.

Initialization and output-transport failures use process exits 70 and 74. A
native fail-stop may terminate without a complete envelope. Native execution has
no interpreter fuel promise. Wrapper stdout is CLI publication, not an ambient
program IO capability.

## Owning qualification

The three native owners in `codegen::selected_fused_row::source_profile::tests`
passed locally on macOS arm64 at `dbd41b589`. They cover the unchanged 4,096-row
original (8,386,560), default-route SPX-T267, deterministic artifact/source
identity, O0/O2 result publication, two paid invocations followed by refusal,
exact and one-short cumulative funding, zero live owners, actual ordinary
allocation counts, and three arithmetic failures on one held account with no
published result. The combined CLI and three-target results are in
[Fused source profile v1](FUSED-SOURCE-PROFILE-V1.md).
