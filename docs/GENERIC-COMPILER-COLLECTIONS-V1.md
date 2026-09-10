# Generic Compiler Collections v1

Status: implemented private profile; **HOSTED GREEN** under the
[accepted v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md).
Source/HIR/graph, ProgramRoot and all-engine runtime checks also have retained
historical local witnesses. Public generic ABI and support remain separately gated.

Audience: compiler contributors and reviewers.

Private effect-free functions may declare one explicit type parameter and move
compiler-owned `Box<T>` and `Vec<T>` carriers through parameters and results.
The parameter is instantiated with exactly `i64`, `i32`, `u8`, `usize`, `char`,
`f32`, `f64`, or `bool`. Local Box/Vec intrinsic composition behind scalar
signatures uses the same profile. Owned elements, nested collections, inference,
and public aggregate ABI declarations remain outside this profile.

Source verification checks all eight substitutions. HIR independently checks
exact compiler-owned nominal identities, scoped parameter identities, intrinsic
calls, materialized function bodies, and canonical cleanup plans. Existing
Box v1 and Vec v1 allocation, borrowing, generation transfer, failure, and
lexical settlement rules apply without a new runtime ABI or prelude contract.

Focused evidence belongs to `language::generic_collections_next` and
`owned_data::generic_owned_function_runtime::collections`. The latter exercises
all eight scalar substitutions across interpreter, native C11 O0/O2, and
Core-Wasm with repeated execution, lexical drop, consuming calls, and all Vec
operations. Its Wasm fixture host rejects stale and duplicate owner settlement and requires
zero live allocations after each invocation. Native probes count every
Box/Vec malloc, calloc, realloc, and free; success and precondition/capacity
failure preserve result sentinels, sticky status, and zero live allocations.
Current release evidence follows the accepted baseline above; historical local
execution is not independently relabeled as a hosted run.

The runtime corpus additionally exercises nested intrinsic producer staging, generic
contract failure and Vec capacity failure. Native allocation accounting and
result sentinels, and Wasm generation-aware handle accounting, remain checked
across four repeated invocations per profile. Existing Box/Vec runtime checks
also passed in the historical local corpus. Release acceptance does not add
public support, broader physical-host coverage, or evidence for later code.

The private Project HIR linker also retains exact bounded collection signatures.
An attempted selected public Box result now reaches the unchanged Public Scalar
Export Profile v1 and rejects with SPX-W115 (generic templates or instances are
not admitted). The former SPX-H006 was an earlier private-linker rejection, not
the public signature contract.

[Generic Iterator Helpers v1](GENERIC-ITERATORS-V1.md) extends this private
profile to the existing `Iter<T>` and `IterStep<T>` carriers and consuming
protocol, including callbacks and explicit step reconstruction. It preserves
the earlier Box/Vec contract and keeps public signatures separately closed.
