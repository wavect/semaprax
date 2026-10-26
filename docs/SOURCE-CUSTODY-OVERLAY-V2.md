# Finite source custody overlay V2

Status: private source contract; executable qualification is pending.

Audience: compiler contributors and reviewers.

This specification owns OPT#893's proof-only successor of the complete 519-row
source-custody loader. It changes no compiler, target, ledger or application
admission. Each source batch requires its own binding and executable qualification. No Rust
qualification or inherited PASS follows from this document.

There is exactly one literal policy and one literal page, each at most 262144
bytes. Rust authenticates policy and page as compiled text, held physical bytes,
and full length/SHA256 pins. Python authenticates those same complete pinned
bytes. JSON has unique object keys and the pinned canonical spelling. There is no
next link, directory discovery, caller-chosen page, compression or additional
quota. Object keys have a canonical spelling; source vectors and inventory order
are never sorted or repaired.

The entire previous 519 loader remains executable. It first reads **metadata**:
the immutable 433 packet, complete 133 loader, original 444 and all intervening
stages and their original inverse checks. It does not require modified physical
sources to impersonate that previous generation. The page composes immediately
after that metadata replay, before current source validation or historical view
selection. Replacing every old row and removing finite additions must reconstruct
the entire previous object, including unknown fields and anchors.

Replacement paths are literal in both source implementations and in the pinned
policy, in original source order. Replacements retain their source position,
compiled flag, unknown fields, and every old view as an exact ordered prefix.
A changed current descriptor is retained whole, with the exact archive channel,
as `whole-pre-overlay-v2-current`. Restoring current/views and the permitted
canonical leaf references must reproduce the whole predecessor row. Additions
are a separate literal ordered list with independently acquired original-parser
classification; a page does not mint roles. Existing files newly brought under
this logical audit retain genuine whole before images; they are not new files.
`errors-existing-owner-witness.json` records such an existing production owner.
Logical admission does not automatically expand the separate raw-module scope.

All original fragment edges remain in order. New edges must be explicitly named
by the literal policy. Every child is compiled, leaf-only, nonself, unique and
exactly equal to the complete same-scope canonical row. Leaf-only references rule
out cycles; a module tree is not recursively expanded as a fragment graph.

Current source acquisition reads the real new physical bytes first, compares the
same-scope compiled text and the whole current archive, and only then selects an
old view. The older Clippy stage's current/requested/normalize fronts explicitly
perform this new admission before reading their separately authenticated old
archive. Their whole old bodies remain for unaffected paths. Python's
`_stage_source` uses the same ordering. Unlisted digests never become authority.
No current byte normalization or fabricated old physical file is used.

The per-held-source Rust bound remains 9753094. Python's original bounded reader
keeps its 262144 per-read limit and its no-follow, regular-file, inode and read
identity checks. These are separate domains, not a new aggregate sum. A source
outside a reader's existing domain remains refused; a page cannot split it to
reset a bound. All original strict174, FINAL6, parser and IO hostile controls
remain owning gates. The new raw child is joined into all three existing source
unions without growing their budgeted root line counts.

Control inputs are excluded from their own page: the policy/page, metadata
extension, guard/config and loader implementation. They are authenticated by
literal compiled pins, complete before/inverses, actual owning source unions and
separate model/guard source bindings. This prevents a policy/page/helper hash
cycle; it does not exempt a control source from those existing audits.

Final binding must occur once after the combined source batch settles. It derives
all replacement/addition paths and classifications from actual source and the
original parser, records whole baseline/current images, fills both literal source
lists and policy pins, updates actual guard/module/carrier/table bindings, and
retains the existing 519 extension byte-for-byte. No producer or role gate may
claim the authored unbound page as qualified current compiler evidence. Rust
runtime controls and the original source gates run on the eventual frozen head.

## Canonical fragment references (page V3)

V3 uses the same one literal page, policy, paths, and unchanged 262144-byte
physical bounds. It changes only redundant fragment metadata for the three
literal audit roots: `tests/hir_module_boundaries.rs`,
`tests/workspace/semantic_graph/source_inventory.rs`, and the existing native
interop `ledger_capacity/source_inventory.rs`.

Within those roots' `fragments`, the exact object
`{"canonical_source":"<source>"}` may replace a complete duplicate canonical
row. The source must already occur in the complete predecessor or this same
page, be compiled and leaf-only, and have that exact root/child edge in the old
edge set or literal policy. References cannot open paths, select archives,
recurse, add roots, change a scope, or confer Rust namespace ownership. Duplicate,
foreign, missing, noncompiled, nested, and extra-field references refuse.

Both readers reconstruct the complete V2 page before the unchanged whole-row
inverse, ordering, edge checks, and current-source authentication. Unknown fields
and every original image/view remain exact. Binding mechanically proves expanded
V3 equals the entire uncompressed V2 candidate. The original V2 composer remains
executable and its complete body is checked; frozen full V2 page/policy fixtures
exercise the old branch and exact canonical expansion. All original I/O,
physical/compiled/archive, old-view, and hostile controls remain in force.

## Archive-prefix transport (page V10)

V10 is a structural transport for one complete V9 page. It preserves all V2
through V9 composition and admission semantics. Its `archive_prefixes` table has
at most 32 distinct, nonempty, normal directory prefixes, each ending in `/`.
Only archive members in the explicit V9 pin slots may replace their literal path
with `[prefix_index, suffix]`; objects, unknown fields, source order, fragment
order and every other scalar remain unchanged.

Expansion refuses an invalid index or suffix, an abnormal reconstructed path, a
literal archive that matches a table prefix, and a reference that does not use
the longest matching prefix. Every table prefix must be referenced at least
twice. Canonical JSON spelling fixes table and reference order. Both readers
expand the entire V10 value to a V9 value before entering the unchanged V9-to-V2
reader chain. Encoding must prove that this expansion equals the complete input
V9 value in canonical bytes.

V10 adds no path discovery, archive read, compression or admission authority.
The single-page 262144-byte bound and the 9753094-byte per-held-source bound stay
unchanged, as do every physical, compiled, archive and historical-view check.

## V13 repeated positional digest transport

V13 is an explicit opt-in transport of the complete logical V12 object. Its
closed envelope adds `digest_table` and changes only the schema to
`semaprax.finite-source-overlay-page.v13`. The table is a unique array of
canonical V11 digest strings (43 unpadded URL-safe base64 characters, including
canonical zero pad bits). Only current/named-view positional pin digest slots
in literal rows, indexed old-519 rows and literal fragments may contain integer
references to that table. Object pins, canonical-source references, unknown
metadata, names, archive references, ordered arrays and multiplicity are opaque
and retained in full.

Each declaration must be referenced at least twice. Declaration order is exact
first-use order across replacements then additions and their existing slot
order. Every repeated positional digest must be interned; singleton digests
remain literal. Duplicate or unused declarations, single-use declarations,
literal/reference aliases, repeated uninterned literals, noninteger or invalid
indices, malformed digests and malformed slot arities refuse. No recursive
metadata substitution, dictionary discovery or additional page is admitted.

The physical V13 page remains at most 262,144 canonical bytes. Before publishing
any expanded digest, the decoder checks the complete logical V12 byte length
against the existing 9,753,094-byte source bound. That unpublished logical value
uses an explicit internal V12 entry bounded by the same source limit; the public
V12 entry retains its original 262,144-byte limit and complete implementation.
The entire canonical V12 value must reconstruct exactly. All subsequent V12,
V11 and older validation, source/compiled/archive authority and limits remain in
force. Python/Rust parity, every original codec control and a complete independent
V12 source inverse are required; wire reduction alone establishes no source
admission or application performance claim.

## V15 literal source-path transport

V15 preserves the entire logical V14 object. Only `source` in literal
replacement/addition rows and literal fragments may use a token. Indexed-row
fragment slots are included; canonical-source references and all unknown
metadata are opaque. The fixed 44 directory prefixes and their one-character
ASCII codes are versioned identically in the Python and Rust codecs. A token
is `code:suffix`. It must reconstruct a normal path and use its longest matching
fixed prefix. A literal normal path with no matching prefix remains literal.
Uninterned matching literals, malformed/unknown codes, abnormal paths and extra
envelope fields refuse. No page-selected dictionary, path discovery or source
authority is introduced. Unused fixed registry entries grant no authority.

The public V15 wire remains at most 262144 canonical bytes. Before publishing
expanded paths, checked complete-length accounting enforces the existing
9753094 source bound. The entire V14 value is reconstructed and validated by an
explicit internal source-bounded copy of the complete V14 reader, followed by
the unchanged older decoder chain. Public V14 functions and their wire cap
remain independently executable and unchanged. Complete historical V14/policy
controls, actual over-cap V14/V13/V12 inverse fixtures, current whole-page
Python/Rust parity, path hostiles and old amplification controls remain owning
gates. Prediction and source-only codec checks establish no compiler or
application admission.


## V16 known literal-row transport

V16 reconstructs the entire canonical V15 value before any unchanged V15 path
validation, earlier codec validation or source admission. In the existing literal
replacement/addition and literal-fragment slots, an object with exactly `source`,
`compiled`, `current`, `views` and optionally `fragments` is represented as
`{"source_row":[source,compiled,current,views]}` or the five-item form carrying
`fragments`. The source is the unchanged V15 literal/token spelling, `compiled`
is an explicit boolean, and every payload occurs exactly once. Indexed rows and
canonical-source references retain their old shapes. A literal row with any
additional metadata remains literal, preserving every unknown value. Only the exact one-key object is an envelope; old literal rows containing an
unknown `source_row` key remain literal. Wrong envelope arities/types and an
unwrapped eligible row are refused as noncanonical aliases.
The fragment-row domain stays exactly one level deep, matching V15. A top-level
fifth payload must be null or an array; a fragment row's fifth payload retains
its original opaque semantics and is never traversed. Unknown
nested metadata is never searched for tags or source paths.

The single canonical wire page remains bounded by 262144 bytes. A first pass
validates all borrowed row envelopes and checks the complete expanded byte count
using only the fixed field-name/punctuation delta before allocating reconstructed
rows. Checked arithmetic enforces the unchanged 9753094 source bound. Expansion
then proves the exact complete V15 canonical preimage and enters an internal
source-bounded copy of the entire V15 reader. Public V15 functions, their
262144-byte cap, old controls and all older readers remain independently
executable and unchanged. There is no new page, dictionary, archive path,
source authority or logical source admission. Source-only size predictions and
authored controls establish no executable qualification or application gain.

## V17 positional source-row transport

V17 uses the same closed row domain and payloads as V16, with each exact
`{"source_row":[...]}` envelope replaced by its array payload. The four- and
five-item arities retain the source, an explicit compiled-flag code, current pin,
ordered views, and optional fragments. Integer `0` decodes to `false`, and integer
`1` to `true`; booleans, floats, and other integers in this wire slot are refused.
Indexed rows, canonical-source references,
and objects carrying unknown metadata retain their entire original values.
An eligible literal row or a sole `source_row` envelope is a noncanonical alias
in V17. Fragment rows remain one level deep; their own fifth payload is opaque.

Before cloning any payload, the decoder validates every borrowed row and counts
the exact fixed field-name, punctuation, and flag-spelling expansion against the unchanged
9753094-byte source limit. It reconstructs the entire V15 page directly and
enters the existing source-bounded V15 decoder chain. An intermediate V16 wire
page is not constructed: its redundant envelopes can exceed the unchanged
262144-byte wire limit. All public V16 functions, limits, and hostile controls
remain independently executable. V17 adds no source, path, page, or authority.

## Closed prior-source path transport

The authenticated guard configuration may encode only `closed_prior_sources`
as `{"schema":"semaprax.closed-prior-sources.prefixes.v1","sources":[...]}`.
A legacy list retains its entire original reader behavior. This field is a
named source-acquisition allowlist, distinct from the canonical overlay row
inventory. Equal lengths do not imply equal membership. New canonical sources
are identified against the authenticated prior canonical inventory; extending
the allowlist preserves its complete existing order and historical members.
The fixed tokens are:

```text
R: src/cleanup_plan/replay/
W: src/workspace_graph/
H: src/hir/
D: docs/evidence/
T: tests/
N: crates/semaprax-native-rust-interop-builder/
S: src/source_reflows/
M: src/source_reflows_merge/
```

A token replaces only that directory prefix. The suffix is a nonempty normal
relative path; absolute paths, empty or dot segments, backslashes and NUL are
refused. Unknown tokens, multiple colons, extra envelope fields, and an eligible
unencoded literal are refused. Unmatched normal paths remain literal. Source
order and duplicates survive exactly; all other configuration values are opaque
and unchanged. Whole physical configuration authentication precedes decoding.

The wire configuration remains bounded by 262144 bytes. Before joining any
expanded path, the reader accounts for the complete reconstructed canonical
configuration, including every opaque field, against the unchanged 9753094-byte
source bound. The decoded list grants exactly the original named-source scope.
Entire legacy configurations and their independent readers remain executable.

## Invocation-local checker compilation reuse

The optional FINAL checker reuse layer may retain at most two successfully
compiled Python `exec` code objects, each with at most 65536 UTF-8 source bytes.
Its key retains the complete source value and exact source type, filename and
mode. Unsupported call shapes and compile failures use the original compiler
without publishing an entry. The authenticated guard must have no inherited
future flags, matching the cache callable's compilation context.

Only compilation is reused. Each invocation still executes the code in its
fresh original namespace. All original module initialization, configuration,
source/archive reads, late physical reauthentication, mutations, symlink
refusals, six roots and hostile controls remain live and unchanged. The complete
original FINAL checker remains independently executable through its original
selector. Source-only checks do not establish runtime gain or FINAL acceptance.

## Source-front edit-string pool V1

OPT#931 adds an opt-in physical transport for the existing complete
`semaprax.next-custody-four-fronts.v1` DATA object. Its closed outer object has
exactly `schema: "semaprax.source-edit-string-pool.v1"`, `strings` and `body`.
`body` retains the entire original object and all unknown metadata. The old
unenveloped parser remains independently executable. This is separate from
the page codecs above: it changes no policy, source role, archive authority,
logical source admission, compiler semantics or application resource limit.

Only exact four-key edit objects (`start`, `end`, `after`, `before`) in these
existing domains may pool their `after` and `before` string payloads:

- top-level `rows`, in original order, visiting `target.edits` then non-null
  `middle.edits`;
- `constructor_retention_903.records`, in lexical source-key order;
- `main8_raw_current_fronts.rows`, in original order, visiting `inverse`.

The complete new body requires the original top-level schema, both existing
nested schema strings, and their existing arrays/records. A top-level row
requires `target` and `middle`; every visited stage/record requires its edit
array. Edit objects with additional or missing fields are opaque. Other nested
metadata is never searched for references. The traversal changes no source
vector, row, inverse, target, multiplicity or unknown field.

A payload reference is exactly `{"$source_edit_string_v1": index}` with a
nonnegative integer index. Booleans, floating representations and negative
zero cannot act as indices. The pool consists of unique Unicode scalar strings
of at least 64 UTF-8 bytes. Every declaration must occur at least twice, in
exact first-use order across the domains above, visiting `after` before
`before`. All repeated eligible strings must be interned; singleton strings
remain literal. Unused, duplicate or single-use declarations, literal/reference
aliases, wrong tags, extra reference fields and invalid indices refuse. Pool
entries are strings, so references cannot nest or form cycles. Reference-shaped
values in opaque metadata remain untouched. New envelopes reject duplicate
JSON keys, nonfinite numbers and non-scalar text without changing the old
parser branch.

The physical wire remains at most 2,097,152 bytes. Before copying any expanded
payload, both readers validate every reference and check a common wire-derived
expansion bound against the existing 9,753,094-byte held-source limit. Start
with the byte length of the complete raw `body` JSON span plus one newline;
for each reference add the canonical UTF-8 JSON string length and subtract the
canonical ASCII reference-object length. Checked arithmetic is required. This
conservatively bounds textual reference expansion while preserving all other
raw JSON bytes. Whitespace or escaped reference-key spellings may increase the
bound; they never reduce it. Opaque numeric spelling is counted from the actual
wire, avoiding differences between Python and Rust float serializers. The
bound is not a claim about reserialization size in either language.

Only after this complete preflight may readers substitute payloads and enter
all original source-front checks. Physical authentication precedes decoding;
the original final source/DATA rereads remain live. Encoding must prove the
complete original logical object is recovered, including opaque fields. The
owning Python controls and test-only Rust module cover inverse equality,
canonical declarations, malformed references, opaque fallback, raw number
spelling and exact/short/amplification limits. Source-only code and size
predictions establish no executable qualification or application improvement.


## Historical V11 envelope compatibility

OPT#933 preserves two independently executable encodings whose historical
schema string is `semaprax.finite-source-overlay-page.v11`. The digest form
is the exact seven-key page (`schema`, `index`, `total_pages`, `predecessor`,
`replacements`, `additions`, `archive_prefixes`). The incoming named-view form
is exactly `schema`, `names`, `page`, carrying its complete V10 predecessor.
Public dispatch recognizes only those disjoint key sets and refuses mixed,
missing and unknown outer fields before entering either reader. Unknown values
inside their original opaque domains retain each original reader's rules.

The complete digest reader remains the internal V12--V17 predecessor. The
complete incoming V10/name reader, producer and hostile controls remain
independently executable. Both public routes restore the same complete logical
packet through their original older readers; their historical schema is not
renamed. Existing page/source caps and source/archive authority are unchanged.
Source-only compatibility and size probes establish no executable qualification.

The direct public V10 entry likewise selects only its complete six-key legacy
pin page or seven-key archive-prefix page. A named V11 envelope always enters
the original legacy V10 decoder after restoring names, even if hostile inner
metadata resembles an archive-prefix page. It never switches codec provenance
using field presence. All original V10 readers remain independently callable.

## V18 typed fragment-reference transport

OPT#934 adds `semaprax.finite-source-overlay-page.v18`, with the same exact
outer fields and positional rows as V17. Only immediate top-level row
`fragments` and replacement `old519_index[3]` arrays admit an integer shorthand
for an exact single-key `{"canonical_source": integer}` object. The integer
must designate an addition: at least 519 and less than 519 plus the complete
ordered additions count. Booleans, floating values, negative zero, negative
values and out-of-range indices refuse. The original object spelling with an
eligible integer is a noncanonical alias in V18. String references, objects
with additional keys and unknown fields retain their original values and old
validation rules. Reference-shaped objects in other fields and a fragment
row's own fifth payload are opaque. No recursive search, sorting, deduplication,
new source admission or archive authority is introduced.

Before cloning payloads, the borrowed decoder checks the unchanged 262144-byte
wire cap, all row/reference slots and the complete expanded V15 size against
9753094 bytes. Each integer-reference expansion adds exactly 21 bytes; V17's
existing row/compiled-flag expansion remains fully counted with checked Rust
arithmetic. The decoder constructs V15 directly, because a redundant V17
intermediate may exceed the wire cap, and enters the original V15 logical
reader chain. The producer proves the entire V15 preimage and the public
composition preserves complete predecessor and unknown metadata. Existing
V10/V11 forms and every V12--V17 function, limit and hostile control remain
independently executable. The two existing Python program members retain their
65536-byte individual limits. Authored controls and pure round-trip/size
measurements establish no compiler, runtime or application qualification.

## Compiled-registration successor (page V19, issue #942)

V19 explicitly records one current compiled-registration successor without
changing the independent V2--V18 readers or their immutable-compiled-field
refusals. Its exact envelope is `schema`, `compiled_successor`, and `page`; the
schema is `semaprax.finite-source-overlay-page.v19`, and `page` is the complete
V18 page, including all unknown fields and ordering. The whole envelope retains
the 262144-byte bound, and both Python program members retain their individual
65536-byte bounds. No decoded payload exceeds the existing 9753094-byte bound.

The singleton successor has exactly `source`, `prior_row_sha256`, `current`,
and `registration`. This admitted packet selects original row 389,
`src/workspace_graph/expected_projection/borrowed_imports.rs`, whose full
original canonical row must have `compiled: false` and the declared full-row
SHA-256. The successor's `current` is the complete ordinary pin. Its exact
registration fields are `owner`, `current`, `start`, and `text`; the owner is
`src/workspace_graph/expected_projection.rs`, and its declaration is exactly:

```rust
#[path = "expected_projection/borrowed_imports.rs"]
pub(in crate::workspace_graph) mod borrowed_imports;
```

The trailing newline belongs to the declaration. `start` is its non-boolean
byte offset after a blank line; the complete literal must occur once in the
actual authenticated owner. An adjacent attribute, another source, a missing
or repeated declaration, a stale pin, an unknown field, multiple successors,
and any already-true predecessor refuse. Both current descriptors must name
`e/<their full SHA-256>`; a wire-supplied alternate normal path is not admitted.

Before any successor source or archive I/O, the ordinary loader compares both
source and owner pins against its independently held current registration
bindings. Rust uses its compiled-source registry; Python uses the sealed
current module bindings supplied by its caller. It then performs the existing
complete physical/compiled/archive current checks for both paths and checks
the exact declaration span. Missing bindings and physical, compiled or archive
drift fail before any historical selection. A metadata-only decoder performs
no I/O and supplies neither registration nor publication authority; the normal
publisher still requires original-parser current classification and source
admission for the exact source cut.

Composition first authenticates the entire original predecessor and old false
row. A private clone changes only that row's compiled field to true. Restoring
false must reproduce the entire original predecessor byte-for-byte. Clones of
the policy and inner page rebind only their internal predecessor digest to
that private clone, and enter the complete unchanged V18 through V2 pipeline.
This permits the explicitly named new compiled fragment while preserving all
old unknown-field, view-prefix, source-order, fragment, whole-inverse and cap
checks. The resulting canonical source and owner must exactly match the two
successor pins. The private clone is proof data, never a new physical
predecessor, historical checkpoint, source admission or execution claim.

Replacement declarations remain in original predecessor source order. Inserting
the newly replaced row 389 at replacement position 140 preserves the complete
relative order of the earlier 201 declarations; additions retain their complete
existing order. Python's `CompiledSuccessorV19` controls and the Rust V19 child
controls exercise the successor, old V2 refusal, new compiled fragment, unknown
and duplicate record rejection, stale/foreign/current witness rejection,
pre-I/O binding, physical drift, declaration attributes, and unchanged caps.
These authored controls are pending the joint owning verification batch and
establish no current normal, runtime, hosted or production qualification.
