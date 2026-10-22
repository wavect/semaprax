# Three cold-construction opportunities — discovery only

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Frozen inspected source commit: `940fcf220e47d1975f757e76f0b3401691db9358`. Locally verified published-link target: `4f8f598c7935d0a5cdd4a5d5ba5b49d164ae2cfc`. No benchmark, allocation census, implementation, or fresh gain claim. Source reachability establishes opportunities, not application frequency.

## 1. Reuse registry-owned temporary scratch across distinct cold callers

Early real reachability: [src/workspace_graph.rs:4056–4080](https://github.com/wavect/semaprax/blob/4f8f598c7935d0a5cdd4a5d5ba5b49d164ae2cfc/src/workspace_graph.rs#L4056-L4080) constructs the registry and invokes import validation and initial forecasts before core construction. [src/workspace_graph/expected_projection/private_layout/registry.rs:170–211](https://github.com/wavect/semaprax/blob/4f8f598c7935d0a5cdd4a5d5ba5b49d164ae2cfc/src/workspace_graph/expected_projection/private_layout/registry.rs#L170-L211) lazily constructs each caller closure inside its existing preparation scope ([src/workspace_graph/expected_projection/private_layout/registry.rs:79–109](https://github.com/wavect/semaprax/blob/4f8f598c7935d0a5cdd4a5d5ba5b49d164ae2cfc/src/workspace_graph/expected_projection/private_layout/registry.rs#L79-L109)).

Actual temporary backing: [src/workspace_graph/expected_projection/private_layout/names.rs:120–143](https://github.com/wavect/semaprax/blob/4f8f598c7935d0a5cdd4a5d5ba5b49d164ae2cfc/src/workspace_graph/expected_projection/private_layout/names.rs#L120-L143) creates three capacity-513 vectors per identifier census; [src/workspace_graph/expected_projection/private_layout.rs:83–108](https://github.com/wavect/semaprax/blob/4f8f598c7935d0a5cdd4a5d5ba5b49d164ae2cfc/src/workspace_graph/expected_projection/private_layout.rs#L83-L108) creates a traversal-frame vector; [src/workspace_graph/expected_projection/private_layout/owner_index.rs:49–65](https://github.com/wavect/semaprax/blob/4f8f598c7935d0a5cdd4a5d5ba5b49d164ae2cfc/src/workspace_graph/expected_projection/private_layout/owner_index.rs#L49-L65) creates nominal-fact and ownership-frame vectors. The returned owner retains sites, not those facts/frames ([src/workspace_graph/expected_projection/private_layout/owner_index.rs:266–275](https://github.com/wavect/semaprax/blob/4f8f598c7935d0a5cdd4a5d5ba5b49d164ae2cfc/src/workspace_graph/expected_projection/private_layout/owner_index.rs#L266-L275)).

Candidate: lazy scratch owned by this early registry, paid inside the existing preparation scope, cleared between successful distinct caller derivations. Reset per-caller census counters, preserve every work debit, poison/drop on failure, and release scratch before core construction; a later unexpected cold derivation must honestly reacquire it. No parent/child budget fusion or authority donation. Expected elimination: repeated actual allocations and reservations after the first eligible caller. Added cost: scratch owner headers/reset bookkeeping and possible preparation overlap; measure peak and drop behavior rather than claiming they improve.

## 2. Use the existing callback count to tighten entry backing

[src/workspace_graph/expected_projection/private_layout.rs:90–101](https://github.com/wavect/semaprax/blob/4f8f598c7935d0a5cdd4a5d5ba5b49d164ae2cfc/src/workspace_graph/expected_projection/private_layout.rs#L90-L101) counts every successful first-pass callback. The old code then counts every type across the whole workspace and reserves that many entries ([src/workspace_graph/expected_projection/private_layout.rs:108–121](https://github.com/wavect/semaprax/blob/4f8f598c7935d0a5cdd4a5d5ba5b49d164ae2cfc/src/workspace_graph/expected_projection/private_layout.rs#L108-L121)). In the second-pass callback, a duplicate returns without insertion at lines142–147; a nonduplicate performs exactly one push at lines153–157, after the capacity guard at148–150 ([src/workspace_graph/expected_projection/private_layout.rs:123–159](https://github.com/wavect/semaprax/blob/4f8f598c7935d0a5cdd4a5d5ba5b49d164ae2cfc/src/workspace_graph/expected_projection/private_layout.rs#L123-L159)). Thus unique inserted entries are bounded by the first-pass occurrence count, assuming unchanged traversal on immutable inputs.

Candidate capacity: `min(count, total_workspace_types)`. Preserve the existing `names::first` call, `debit(programs.len())`, checked total-type fold and its overflow/error order before applying the min. Do not substitute an unchecked/saturating total or skip that old work. Keep both traversals, all semantic checks, aliases, deduplication, sorting and work debits. Expected elimination: unused entry backing and its real reservation; allocation-call count is unchanged. Do not apply this bound to ownership facts, whose traversal can include other descendants.

## 3. Stream canonical commented comparison into held source

[src/workspace_graph.rs:3991–4015](https://github.com/wavect/semaprax/blob/4f8f598c7935d0a5cdd4a5d5ba5b49d164ae2cfc/src/workspace_graph.rs#L3991-L4015) creates canonical output solely to compare and read its length. [src/format/comments.rs:192–199](https://github.com/wavect/semaprax/blob/4f8f598c7935d0a5cdd4a5d5ba5b49d164ae2cfc/src/format/comments.rs#L192-L199) builds a growing ordinary String; [src/format.rs:260–265](https://github.com/wavect/semaprax/blob/4f8f598c7935d0a5cdd4a5d5ba5b49d164ae2cfc/src/format.rs#L260-L265) already accepts `fmt::Write`.

Candidate: a comparison sink with offset, checked total length and sticky mismatch; execute the complete unchanged commented formatter and keep accepting writes after mismatch. Preserve placement construction, full traversal, bounded scope, and overflow-before-noncanonical diagnostic order. Expected elimination: the complete temporary output String and growth allocations. No direct reservation reduction is proven because this output is an ordinary String; placement/formatter scratch remains.

## Required owning qualification

Use complete frozen old/current cold production routes with identical source/cache state; record total reservation, allocations/reallocations, allocated bytes, peak live bytes, attempts and outcomes. Distinguish distinct registry caller derivations from held hits. Include zero/one/multiple eligible callers, scratch release/reacquisition, exact/one-short builder and work bounds, interleaved callers, nested floors, sticky failure, Drop and foreign-pointer controls. Compare all six diagnostic fields/order, canonical/source/graph bytes and declared-work traces. Explain new capacity-bound successes exclusively by real backing removed. No baseline adjustment, refunds, scope fusion or cap changes.

## Source parity proof

- `src/workspace_graph/expected_projection/private_layout.rs`: full-file identical; SHA-256 `292d6b24b974e6ee3c4575a389f67faa48dcf61e95783a70ed467b9acaa1c5f2`.
- `src/workspace_graph/expected_projection/private_layout/registry.rs`: full-file identical; SHA-256 `92280049b79e12704d73359c34bebaadbcc0ebac37f435b710f88487c0cb8a44`.
- `src/workspace_graph/expected_projection/private_layout/names.rs`: full-file identical; SHA-256 `3f434af06f500aacfccf578d871ef67845e597b0fa9a6834c3790c8346ffda59`.
- `src/workspace_graph/expected_projection/private_layout/owner_index.rs`: full-file identical; SHA-256 `8aca443af867d3b213fbbabfc526e92a2e045a4447ab0a1cf16d8bb0b44b8cee`.
- `src/format/comments.rs`: full-file identical; SHA-256 `248eacbafd3a87fa5ebcd2813b7251e97aa8c33be5ff1ed41b8dd4c01a32bcd7`.
- `src/format.rs`: full-file identical; SHA-256 `a0afc1d0c8a137742ae524f57e1e2fc585c2cbff808c932e1bd1e0081b358a6e`.
- `src/workspace_graph.rs:3991–4015`: byte-identical fragment at identical lines.
- `src/workspace_graph.rs:4056–4080`: byte-identical fragment at identical lines.

`src/workspace_graph.rs` as a whole is NOT identical: the published target lacks frozen940's later attempt/typed-receiver plumbing (diff begins at4122; Receiver acquisition at old4403 is absent). None of the links above claim that plumbing exists at the published commit. The early construction/canonical fragments and all six complete implementation files cited above are identical. Parity was checked using local `git show`, with no network access.
