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
projection still copies it for the independent oracle. Exact-source replay,
evaluation, owned handoff, and final Rust-byte comparison remain required on
every ordinary token. These source changes have no measured speedup claim until
a bounded before/after run and the focused correctness selectors pass.

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
