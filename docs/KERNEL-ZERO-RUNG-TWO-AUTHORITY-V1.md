# Kernel-0 Rung-2 Formatter Authority v1

Audience: compiler and self-hosting contributors.

Status: implemented local compiler boundary for issue #188. This is neither a
self-hosting-rung promotion nor a public ABI, hosted result, target-runtime
claim, authority transfer, or owned Kernel-0 buffer. The private
[Owned Handoff v1](KERNEL-ZERO-RUNG-TWO-OWNED-HANDOFF-V1.md) adds an ordinary
checked owned-Bytes transfer around the assembled scalar candidate without
changing the scalar proof boundary.

## Closed subject

`src/kernel_zero/rung_two_authority.rs` is the one production adapter for five
embedded-source lanes, in bootstrap-v2 order: `char`, `bool`, `int`,
`operator`, and `string-scalar`. Its only formatter caller is
`src/format/kernel_zero_tokens.rs`; each renderer replays its exact source
before evaluating a candidate.

The adapter accepts a caller-owned 20-byte token. The longest supported value,
canonical `i64::MIN`, is `-9223372036854775808` (20 ASCII bytes); all other
lanes are shorter. A new lane or longer reference requires review and a new
versioned contract, never a silent array enlargement.

## Selection protocol

For each ordinary formatter token, the adapter evaluates the candidate first.
That evaluation replays the component's embedded source through its retained
binding before it can return text. The candidate then traverses the private
exact-bound synchronous owned handoff; actual last-owner release and successful
result settlement must be observed before comparison. The adapter copies it into
its fixed private token only if it fits, then byte-compares the entire token with
the caller-provided Rust reference bytes. Only exact equality selects the
candidate token for copying to the caller's output.

Candidate refusal, invalid/oversize length, UTF-8 failure in a component, or
one-byte disagreement selects the original Rust byte borrow. The fallback is
pointer-identical to the supplied Rust slice before the adapter performs its
single bounded copy; candidate storage is never returned. Rust therefore
remains the sole authority for formatter bytes even on a candidate match.

Exact-source replay may canonicalize internal compiler data. A thread-local,
panic-safe scoped guard surrounds candidate evaluation and owned handoff. Any
formatter entry nested under that scope bypasses candidates and copies its authoritative
Rust bytes directly; the outer call resumes normal byte comparison after the
scope drops. This prevents recursive candidate derivation without disabling
ordinary top-level lane traversal or leaking state across calls/threads.

Any active `bounded_output` limit also selects the Rust-only path before
candidate evaluation. Evidence replay can allocate or reserve compiler work,
so it must never spend a caller's bounded canonical-output budget. This rule
leaves bounded callers Rust-authoritative and resumes ordinary candidate
comparison after their scope exits.

The adapter does not run C, Node, Wasm, a compiler subprocess, or any external
target. It opens no files, performs no network/process/persistence action, and
adds no capability. Bootstrap-v2 target execution and its recovery model stay
test-only evidence under `rung_two_bootstrap/` and are governed separately by
[Rung-2 Target and Recovery Evidence v1](KERNEL-ZERO-RUNG-TWO-TARGET-RECOVERY-V1.md).

The integer, character, and string-scalar byte lanes stage their one or two
Kernel-0 arguments in fixed stack slots. The closed one- or two-byte operator
lane also assembles its intermediate token in fixed storage; test-only byte
projection still copies it for the independent oracle. Each renderer
authenticates its fixed source before reuse. Evaluation, owned handoff and
the final Rust-byte comparison still run for each ordinary token.

## Required evidence

The authority module tests refusal, mismatch, and oversized candidate recovery
with pointer-exact Rust fallback, plus a matching candidate copied to a
caller-owned `[u8; 20]`. The formatter integration test formats ordinary AST
nodes containing every lane and pins the deterministic counter vector
`[char, bool, int, operator, string-scalar] = [2, 4, 6, 8, 2]` for one
canonical formatting operation: the formatter's measured pass and emitted
pass each traverse the authored `[1, 2, 3, 4, 1]` token inventory. This
pins both phases of one canonical operation. A nested-canonicalization test
proves the guard bypasses the inner candidate and restores the outer thread
state; unwind panic or refused handoff keeps Rust bytes and restores re-entry.
Package-report generation
from `examples/meaning.spx` pins the former recursive path. A bounded-output
test pins evidence bypass, zero candidate count inside the scope, and normal
candidate use after scope restoration. Existing broad
per-lane component and shadow tests remain separate regression evidence.

This finite comparison route does not prove source/HIR correspondence, backend
equivalence, an owned-buffer interface, or universal formatter equivalence.
The self-hosting ladder remains at rung 1 until the independent requirements
in [Semantic Kernel v1](SEMANTIC-KERNEL-V1.md) are accepted.

## Borrowed entry discovery (OPT708)

All five scalar renderers discover each length/byte entry by exact borrowed
identity text in their already-held KernelProgram. This preserves the linear
first-match order and actual current function pointer; persistent declaration
IDs and exact-source replay are unchanged. It removes the temporary owned ID
allocation per query, without caching another program's lookup or changing
evaluation, fuel, settlement or Rust byte authority.

Two focused owning controls compare a saved linear lookup, duplicate and foreign
program pointers, 1,000 observed owned-ID versus borrowed queries, and actual
authenticated integer-renderer evaluation including both i64 extremes. Their
allocation scope is lookup only; compiler construction, evaluation and handoff
are outside that observer. Both controls passed on the compiler sources at
`99fdfac49`.

## Borrowed evaluator bindings (OPT708)

The reference evaluator's call-local environment stores borrowed ValueId
references into the immutable current entry/term. It retains the same fresh
callee environment, newest-first shadow lookup, left-to-right argument
evaluation and pop on selected failure. No reference escapes evaluation.
ValueId clones already share Arc backing; the removed work is reference-count
traffic and the larger inline ValueId in each environment slot, not an ID
payload allocation. Environment and argument vectors remain real allocations.

The saved owned-environment evaluator is a test-only independent storage and
traversal oracle. Two controls cover literal requested allocation totals for
1,000 one-parameter calls through Drop, nested shadowing, fresh callee scopes,
lazy and first-fault order, failed-let restoration, reentry, and authenticated
integer renderer bytes. Construction and source replay are outside the
allocation observer. Both controls passed on the compiler sources at
`99fdfac49`.

## Fixed owned handoff measurement (OPT708)

The predecessor serialized and authenticated the same 169,036-byte binding
on every warm handoff. The current implementation authenticates that exact
binding before publishing the private immutable boundary. Both paths execute
the checked ownership wrapper and compare the settled bytes on every call.

An unoptimized macOS aarch64 Rust 1.97.1 run measured seven alternating rounds of
128 calls with the same eight token inputs. The median warm round fell from
10.629 ms to 0.562 ms. In a separate allocation-observer round of 128 calls,
requested bytes fell from 66,696,496 to 171,312. Both paths released every
observed allocation and returned identical bytes. The single current
boundary initialization took 931 ms and was measured separately. The focused
binding, substitution, fallback, reentry, and native/Wasm settlement controls
also passed.

Run the bounded comparison with:

```sh
cargo test --locked -p semaprax --lib \
  kernel_zero::rung_two_owned_handoff::tests::cached_handoff_measures_the_previous_per_token_binding_work \
  -- --exact --ignored --nocapture
```

The existing Criterion benchmark of the first greedy-capture witness measured
111 ms for source checking, 43.0 ms for canonical formatting and 9.68 ms for
retained execution in the unoptimized build. The last stage includes its
binding recheck and worker creation. The ordinary source invocation and the
retained call both returned 1 using 25,184 of their 1,000,000 steps. A separate
single sample of graph generation and verification took 2.70 s. These phase
measurements describe the current witness. The paired measurement above
isolates the handoff change.

```sh
cargo bench --locked -p semaprax --profile dev --bench interpreter -- ascii-pattern
```

[OPT #708](https://github.com/wavect/semaprax/issues/708)
