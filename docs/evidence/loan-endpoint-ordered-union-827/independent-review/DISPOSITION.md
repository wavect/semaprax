# Bounded ordered-union source review disposition

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

One tool-disabled review completed in 40.686135 seconds, exit 0, peak process
RSS 319,963,136 bytes (limit 805,306,368), no resource refusal. Payload 9,990
bytes; raw verdict/result/usage and process receipt retained unchanged. Provider
list cost USD 0.069498, actual billing unknown. No compilation or Rust execution.
No definite blocker was reported; this is not runtime/funding/fit approval.

The executed runner was copied with a stale historical source_commit display
label. The raw receipt is preserved exactly, not rewritten. Actual reviewed
source is integrated 6f8c5be48327a3f8e96c40307888d8527692e1fd, bound by
payload SHA and dispatch-source-binding.json. run_review.executed.py.txt preserves
the exact executed runner. There was one dispatch, no retry or replacement.

1. successor_edges in src/loan_plan/adjacency.rs returns Range<u16> indexing the
   canonical cfg.edges. Its existing successors method maps precisely that
   range through cfg.edges[index].1. The explicit mapping supplies the Clone
   iterator bound; the existing binary_search is retained to preserve behavior
   even for duplicated internal edge tuples rather than quietly selecting a
   different duplicate row.
2. src/loan_plan/storage.rs::marker is the same empty static H006 Diagnostic
   used by existing cfg_directory::Map::new/insert/vector. Physical identity is
   retained separately by CfgReservation::first and storage::classify, which
   consults the affine owner, never Diagnostic text. The new early zero call
   authenticates that same owner before traversal and preserves selected E.
3. Both old AVL and new heap debit the same cumulative payer; neither refunds.
   New paid backing is count * Layout<&[LoanId]> plus exact unique output.
   Old paid backing is one node per union ID plus exact output. High-degree,
   few-unique shapes can cost more in the new algorithm: there is no claim of
   universal reduction or application fit. It removes the actual temporary
   per-ID trees, without inventing cheaper layout units or credit. Both are
   bounded O(total row IDs log degree/union), with no accumulated-vector copies.
4. LoanProgramPoint contains ExpressionId plus a heap-free phase. ExpressionId
   is Option<Arc<String>> (src/hir/ids.rs), so cloning shares its existing text;
   LoanCause contains only enum/u16 payload. All cited clones predate this
   change and remain verbatim in the compiled before receiver. Origin place
   cloning continues through the original paid storage::clone_place.
5. Empty unions intentionally pay the new actual Vec carrier only. Old union
   paid both Map and Vec carriers. Numeric debit equality is not promised for
   an allocation-removal change. Existing exact/short/zero/sticky physical
   controls derive the new actual full debit; old semantic parity is separate.
6. cfg_directory::vector<T> has no trait bound and delegates to the exact paid
   storage::vector<T>. &[LoanId] therefore satisfies the existing API directly.
7. Both union constructions remain before endpoint starts/kills projection.
   The prior complete endpoint block is retained independently. Concrete
   reservation requests/frontiers change with the real backing being removed;
   first refusal identity remains sticky and no allocation follows selected E.
   No claim of matching old byte requests is made. Missing-end semantic error
   is still selected before endpoint work and compared in all six fields.
8. The unique-count bound follows directly from the u16 LoanId domain, even
   for malformed IDs; it does not depend on the producer's 256-loan admission.
   Existing draft-index casts are unchanged. Tests include u16::MAX, repeated
   IDs, empty/single/high-degree rows and unordered-row old fallback.

Five new registered selectors remain authored, not executed in this lane.
Full old/current materializer fields/errors, unchanged work counter, and actual
new union requested backing/Drop/refusal controls await root qualification.
