# OPT831 empty-signature receiver, next source-only batch

Base: `d96247b3cb69282c3d1b835b2428779e8bde82c7`.
This is a bounded actual-constructor improvement, not a weighted layout rewrite.
The preserved a82 row17 failed at width0 (paid10829 ->11970, allocated1517 ->861).
That is historical evidence: d962 contains later #854 arena changes, so no
current failure or gain is inferred from those figures.

The original signature-work debit stays before the new paid branch. Empty
successors construct only the real `(accepted, Vec::new())` signature, retaining
the original zero-vector authentication, lookup/hit accounting, signature-before-
state insertion, and original insertion ordinals. All preflight, cold activation,
function/budget/current-ledger ownership, failure publication, scratch teardown,
route arithmetic and general successor bodies are physically unchanged. The
nonempty borrowed query path is complete and unchanged. It now also pays the
real new branch predicate/payment receivers, so wide/unique unfavorable costs
remain visible. No refunds, caps, work reductions or fee exceptions exist.

`empty_signature.rs` inventories every newly introduced owned/borrowed/Option/
Result carrier using its actual Rust type. The full d962 receiver body is
compiled verbatim in `before_empty_signature.rs`, with only imports and read-only
getters outside that entire original body. It never calls the new empty branch.
`current-to-d962.json` has exact SHA-gated whole-source inverses and full Git
preimages for every changed preexisting file. Three new helpers are bound in all
three raw inventories; the native inventory remains1500 lines. Original guards,
encoders, tests, strict paid inequalities, and old wide/unique assertions remain
untouched; no old fixture is padded or weakened.

## NEXT integration and qualification

This private component is not part of the parent's frozen current batch. At
future integration, preserve all older fixed source tables and algorithms, and
prepend the finite current-to-d962 bridge for these exact preexisting paths
before older source readers. The final joined-source endpoint generation must
incorporate these3 new helper inputs in each full authority inventory and its
exact Main043 allowlist. Do not feed fresh source directly to old immutable
current endpoints, or overwrite their complete predecessor recipes. Refresh all
current metadata/bindings under the parent joined-source protocol, preserving
all protected originals. This source-only component does not claim those final
joined endpoints already match.

`pending-selectors.json` lists4 new existing-lib owners plus original required
owners; retain the complete original singleton, foreign/stale/restored/sticky,
all-byte-cut, wide/unique and application retry/normalizer qualification scope.
The new physical owner reports actual old/current paid/allocated/peak/Drop and
checks zero/exact/short against each independently measured total. Its strict
paid comparison is scoped only to the real empty case. No Cargo, Rust build,
test, Clippy, interpreter, backend or application execution occurred. No actual
paid/heap gain, application fit or issue closure is claimed.

Source-only proof command:

```sh
TMPDIR=/Volumes/Semaprax/runtime-tmp PYTHONDONTWRITEBYTECODE=1 /opt/homebrew/bin/python3 docs/evidence/empty-signature-receiving-831-20261016/verify-source.py --repo /Volumes/Semaprax/worktrees/opt831-compact-successors-20261016/AI-Lang
```
