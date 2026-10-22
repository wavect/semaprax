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
