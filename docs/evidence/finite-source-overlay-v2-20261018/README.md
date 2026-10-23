# Finite source overlay v2

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

The page is source-bound: 680 logical rows and 469 parser-derived owners. The complete original 519-row predecessor, prior generations, archive channels and original controls remain retained.

The source checks recorded under `qualification/` cover the dc369 generation: strict 67 refusals, FINAL6 source 75 controls, six role controls and 36 Python protocol fixtures. The original source producers bound 174 subjects and six FINAL6 sources at the exact heads in their receipts; these are source-only producer results, not executed Rust tests.

The subsequent incoming short-plan and selected-compilation merges has current source pins resealed in the same bounded page. Its focused Rust build, the nine new overlay Rust tests, runtime checks and current boundary requalification remain pending. Prior-head checks are not current-head runtime evidence. See `../merge5695-bounded-overlay-20261018/` for both complete histories and the merge pin check.

The version-4 page encoding retains the same single-page and source bounds. Only
`src/source_reflows_merge.rs` and
`src/source_reflows/incoming67107_semantic_receivers.rs` may use
`{"old519":[index,length,sha256,additional_views]}` (literal indices 0 and 1).
The decoder verifies each complete immutable old519 row against its fixed digest,
clones every field and old view, restores the required original-current view,
and appends the supplied view suffix. The current archive is exactly
`e/<full sha256>`. Both references must occur once, in replacement positions only.
The complete rows expand before the unchanged v3 fragment expansion and v2
inverse/current-admission checks. Prior v2/v3 pages and controls remain supported;
this encoding supplies no source admission or runtime qualification.

V5 adds exactly two fixed original519 row references: retained arena tests and replay. Each full immutable original row is authenticated by its literal canonical digest and cloned with every field and prior view before new current/view channels are appended. V5 expands these two records before the complete unchanged V4, V3, and V2 readers; the one-page cap remains 262144 bytes. No referenced path is opened or admitted by this metadata codec.

V6 inherits any unique original519 row by its integer position in the same authenticated predecessor. A record carries the full current pin, exact appended views, and either no fragment override or an array for the original three V3 roots. All source, compiled, unknown-field and original-view data are cloned; non-root fragment changes remain full literal rows. Expanded rows pass the unchanged V3 reference rules and V2 whole inverse, policy, edge, and current-authentication checks. V2–V5 readers and controls remain independent; no new path or page is read and all caps remain unchanged.

V7 retains V6's three-root override domain and additionally encodes addition-row references by their authenticated canonical positions. V8 adds only two literal old519 carrier/leaf pairs: index 38 `main67107_inputs.rs` with child 366 `main67107_inputs_tail.rs`, and index 314 `hir/resolve_program.rs` with child 316 `hir/resolve_program_methods.rs`. Each complete immutable carrier row is authenticated by its fixed canonical digest. Its existing four-field `old519_index` tuple carries a current pin, appended views and exactly one reference to that literal compiled leaf. The complete updated leaf is acquired from exactly one replacement record; all old fields, unknown metadata and view prefixes are restored before the unchanged V7, V6, V3 and V2 readers. Other carriers, alternate children, duplicate or missing target records and non-leaf targets are refused. The single page, ordering, physical-source authentication and 262144-byte cap are unchanged; this private fixture encoding grants no source admission or runtime qualification.

V9 transports pins only in replacement/addition row `current` and `views` slots,
the corresponding `old519_index` current/appended-view slots, and the `current`
and `views` of their one-level literal fragment rows. Canonical references and
all other metadata are untouched. A current pin is `[length, full_sha256]`; a
named view is `[name, length, full_sha256]`. Either tuple may append the exact
literal archive path; omission means precisely `e/<full_sha256>`. Only objects
with exactly the documented pin fields are encoded. Extra or unknown fields
keep the entire original object, including absent versus null values.

The decoder validates tuple arity, types, full lowercase digests, individual
source length bounds and archive paths with the existing pin validator. It
visits only those fixed slots, without recursive reference expansion. V9
materializes the complete canonical V8 page before the unchanged V8 through V2
readers. The generator requires exact whole-V8 equality after its round trip;
composition, numeric positions, archive authentication, single-page 262144-byte
cap and source admissions retain their existing meanings. This private
transport grants no authority or runtime qualification.

V10 is a structural transport for a complete V9 page. It adds one
`archive_prefixes` array of at most 32 distinct, nonempty, normal directory
prefixes. A prefix ends in `/`. Only an explicit archive member in a documented
V9 pin tuple may replace its literal string with `[prefix_index, suffix]`.
Implicit `e/<full_sha256>` archives, full literal pin objects, canonical
references, row and fragment order, numeric positions, unknown fields and every
other value remain unchanged.

The decoder validates the prefix table, reference arity and types, expanded
normal archive path, longest matching prefix, and at least two uses of every
table entry. It refuses a matching archive left literal. It then removes the
table, restores the V9 schema and every exact archive string, and enters the
complete unchanged V9 through V2 reader chain. The generator derives candidate
directory prefixes only from explicit archives, selects the size-reducing
candidate with the smallest canonical page size (lexical prefix breaks ties),
and stops within the unchanged 262144-byte page cap. A caller may request
additional reserve; the codec imposes none by default. It requires exact
whole-V9 equality after decoding. The frozen fixture retains its explicit
10 KiB reserve assertion as a codec test oracle.

V10 adds no page, source read, archive payload compression, source admission or
runtime qualification. The source cap, physical-source authentication and all
V9 and older formats keep their existing meanings and implementations. The
frozen `v10-unqualified-full-union-v9.json` and
`v10-unqualified-full-union.json` values are logical codec fixtures. They bind
the complete 707-row parent-metadata union and its exact transport inverse for
tests; they are not admitted current physical pages or source qualification.

V11 is an explicitly versioned digest transport over a complete V10 page.
Only the SHA256 member of each documented positional V9 current or named-view
pin changes: 64 lowercase hexadecimal characters become 43 canonical unpadded
base64url characters encoding exactly 32 bytes. Full object pins, including
objects carrying unknown fields, remain literal. Source and view names, archive
references, unknown metadata, ordering, and the existing prefix table stay
unchanged. There is no recursive matching of digest-like strings.

The V11 decoder checks tuple arity, named-view types, integer source lengths,
the exact alphabet and length, decoded byte length, and canonical re-encoding.
It restores the complete V10 value before entering the unchanged V10 through V2
reader chain. All older schemas retain their readers and controls. The page cap
remains 262144 bytes, the source cap remains 9753094 bytes, and the prefix limit
remains 32. The owning Python and Rust controls round-trip the complete frozen
V10 logical fixture and reject malformed and noncanonical inputs; these codec
controls make no runtime qualification claim.

V12 is an optional view-label dictionary transport over a complete V11 value.
Its closed envelope adds only `view_labels`, an ordered array of
`[index, label]` declarations. Indices must equal their contiguous positions,
labels must be distinct strings, every declaration must be used, and first
uses must follow declaration order. Repeated references are required to remain
legal: they preserve distinct occurrences of the same historical view. Only
the first member of documented positional named-view tuples becomes an integer
reference. Full object pins and their unknown fields, canonical references,
current pins, literal row metadata, order and multiplicity remain unchanged.

The decoder first validates every reference and checked-sums the expanded
quoted JSON label bytes against the existing 9753094-byte source bound, before
publishing expanded label strings. The physical page still has its unchanged
262144-byte cap. Expansion restores the entire V11 canonical value, including
the unchanged archive-prefix table, before the original V11 through V2 chain.
Duplicate declaration indices or labels, unknown indices, noncanonical first
use, unused declarations, invalid arities and extra envelope fields refuse.
The V11 encoder and all older readers remain unchanged and independent; V12
encoding is explicitly requested and does not change default serialization.

The V12 Python and Rust controls are authored, not executed. Shared fixed vectors
cover complete canonical equality and opaque metadata; the full immutable a966
V11 page and whole old controller sources remain independent controls. A static
prototype on the separate frozen 797-row V11 page counted 1490 positional views
and 193 labels: 254079 bytes became 227431 bytes. That is source-capacity evidence
for that input, not current-page adoption, runtime, RSS or agent-token evidence.
