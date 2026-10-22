# OPT835: cold canonical-comment comparison — source authored, unexecuted

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Immutable basis `940fcf220e47d1975f757e76f0b3401691db9358`. The private cold workspace preflight now sends the **complete unchanged commented formatter** into a borrowed byte-comparison sink, avoiding its temporary final output String. No current runtime, allocation/RSS, reservation, application-fit or token/cost gain has been measured. No Cargo, Rust compilation, Clippy, tests, backend execution, Claude, or review was run by this lane. ROOT, #834 private-layout and #825 type-rewrite files were not edited.

## Exact behavior and authority

`Compare` borrows the held source bytes and retains only fixed stack controls. Its offset and total UTF-8 length use checked addition. Mismatch and length overflow are sticky observations; every `fmt::Write` write returns success, so the entire original placement walk, formatter traversal and later writes continue after mismatch/overflow. The caller checks bounded/length overflow before noncanonical-source reporting, then retains the original full Diagnostic, frontend counters and checked canonical-byte accumulation. Nested `with_limit` scope and every other early/cold and later constructor operation remain in order. No output capacity, reserved debit, cap, work allowance, authority or lifetime is invented.

The actual API is [`std::fmt::Write`](https://doc.rust-lang.org/std/fmt/trait.Write.html): write failure aborts formatting. This sink therefore records comparison state rather than returning `fmt::Error`. UTF-8 chunks compare through byte slices, preserving arbitrary multibyte comment boundaries. The current formatter constructs `Placement` directly; it does not need a base canonical String for placement. Placement vectors/maps/comment clones and formatter scratch all remain. The removed final output was an ordinary unpaid String, so no reservation reduction is claimed or encoded.

## Independent old receivers and exact custody

- Original `canonical_with_comments` and all placement code remain byte-exact; the full original `src/format/comments.rs` is archived.
- Full `src/format.rs`, including the complete commented writer, remains byte-exact and independently shared by both routes, with its own full archive.
- All441 lines of the complete old actual cold `build_owned_inner`, plus the full28-line limit wrapper, remain independently compiled in `cold_canonical/before.rs`. They still call the old String formatter directly. Only test visibility is added; literal bytes and complete function bodies remain unchanged.
- Three complete changed source/audit owners have full exact inverses; two documentation originals are separately archived. Five new source children have current hashes and are joined into the existing root source audit. No old audit join or exemption is removed.
- `source-inverses-checkpoint-417332e.before.json` preserves the earlier paused packet. It is historical source-only metadata, not execution evidence.

`source-only.py` verifies the complete inverses, original functions/wrapper and literal bytes, unchanged placement/String/writer, current child hashes/joins, unchanged authoring-basis constructor region and exact source registrations. `static-integration-checks.json` records91 unchanged private-layout purity inputs, all6251 Rust files in the owning module-budget roots, zero violations, the three existing session mirrors and all four prior typed projection joins without new exemptions. These are static source proofs only.

Existing global Source36/other inverse packets retain their prior heads. The integrator must compose their additive full-source preinverses after adoption; this lane does not relabel an old global proof current. This packet's full authoring-source hooks and SHA bindings are complete checkpoints for that composition. The compiled whole-original test follows composed hooks before comparing its immutable archives.

## Owning source-authored qualification

`pending-selectors.json` binds8 new and14 exact existing controls to source declarations with no ignored body. The new controls cover equal/noncanonical output, empty/one/multiple comments, Unicode, CRLF, trailing and nested placement, continued writes after mismatch/overflow, full formatter scratch traversal, exact/short/zero/sticky/nested-floor reservation behavior, complete old/current cold success and malformed/duplicate outcomes, all six Diagnostic fields/order, full source/HIR graph bytes and iterative whole Drop. Paired allocator observations include the same cold setup and complete lifetime; they separately report actual allocated/reallocated/cumulative/peak bytes and total reserved bytes. Reservation equality is asserted rather than discounted. Physical differences remain unmeasured until execution.

The original comment-placement/idempotence, canonical/normalization, malformed-project/no-write and semantic-graph controls are retained as exact selectors, along with the existing pure-source audit, cold/warm cache and unchanged retry/normalizer owners. Those application owners must run and disclose failures; their recovery is not inferred from removal of one formatter output allocation. Qualification belongs in the parent's next consolidated implementation batch, not a per-ticket Cargo job. #835 stays open pending that execution and any actual-producer pin regeneration.
