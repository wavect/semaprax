# C++ scalar package v1

Status: additive bounded implementation contract; product promotion is not
claimed.

Audience: C++ integration authors, compiler contributors, and reviewers.

## Purpose and compatibility

`semaprax cxx-package <file.spx> --function <name-or-id>` emits one canonical
JSON envelope containing a C++17 header and a C11 provider translation unit.
The provider embeds the production native projection and adds externally
linked wrappers around the selected internal functions. This closes the
separate-translation-unit defect that the read-only C++ Shim v1 deliberately
does not solve. C++ Shim v1 and its exact bytes remain unchanged.

The command accepts the same one-to-64 unique selections and byte bounds as
[C++ Shim v1](CXX-SHIM-V1.md). Admission is exactly its explicit-identity,
monomorphic, effect-free, by-value Copy-scalar parameter/result subset. An
unknown selection or any selected exclusion rejects the package as a whole;
there is no partially callable package.

## Bounded adapter ABI

The header declares `spx_cxx_status_v1` with exact values `success = 0`,
`semantic_failure = 1`, and `adapter_failure = 2`. Each selected stable ID is
mapped injectively to `spx_cxx_call_` followed by the lowercase hexadecimal ID
bytes. Parameters retain the production C scalar mapping and the final result
is an out pointer. The wrapper:

1. rejects a null result pointer as adapter failure without invoking SEMAPRAX;
2. rejects a `char` argument outside the Unicode scalar range, including the
   surrogate interval, without touching the result slot;
3. constructs a fresh canonical context and one-entry status arena;
4. invokes the exact internal native symbol once, left to right;
5. returns semantic failure for a non-success SEMAPRAX status; and
6. returns success only after the native final result commit.

The selected profile has no imports, effects, owned values, cleanup actions,
callbacks, exceptions, allocator exchange, or reusable context. One status
entry is therefore the complete per-invocation failure capacity, not a general
runtime policy. C++ exceptions never cross the C boundary and the generated C
provider does not call C++.

## Canonical envelope

The compact `semaprax.cxx-package.v1` envelope contains an exact payload byte
count and a domain-separated SHA-256 digest. The configured byte limit bounds
the exact final envelope and all output builders reject before appending bytes
past their active budget. Source input and each native/Canonical-Shim replay
are independently hard-capped at 16 MiB, so a small output limit does not
silently become a claim about parser working memory. Its closed payload
contains:

- the exact canonical source bytes and bytewise-ordered stable-ID selection;
- the exact verified C++ Shim v1 envelope;
- `header`, its byte length, and domain-separated digest;
- `provider_c`, its byte length, and domain-separated digest; and
- fixed nonclaims.

`verify_package_envelope` requires the caller's expected source path and exact
selection; embedded source is proof data and cannot select its own subject.
The verifier first derives the expected package from that retained subject,
then checks closed keys, canonical byte counts and all three digest layers and
parses and verifies the embedded source through the
ordinary compiler, resolves the exact stable-ID selection, re-runs admission
and native code generation, and reconstructs the Shim, header, and provider
bytes exactly. Digests authenticate transport integrity; they are not treated
as provenance and cannot authorize a self-consistent remint. Mutation,
truncation, duplicate/unknown keys, reordered canonical bytes, appended code,
revision or selection substitution, or a forged outer digest fails closed.
Output overflow is rejected, never truncated.

The provider begins with `SPX_NO_ENTRY_WRAPPER`, embeds the exact production
C11 projection, and appends only compiler-derived wrappers. The header carries
no source path and is stable across formatting/path changes that preserve the
verified program revision and selected identities.

## Evidence and nonclaims

The owning gate generates a package, verifies it, compiles the provider as C11
and a separate consumer as C++17, links them, and executes successful,
contract-failure, and checked-arithmetic-failure calls while proving that
failure never commits the result slot. It also covers null outputs, every
scalar mapping, exact and one-byte-short output budgets, digest-reminted
source/revision/selection/artifact attacks, appended provider code,
deterministic generation, and frozen Shim v1 known answers.
The compiler and linker are explicit test inputs; missing tools do not become a
passing skip.

This tranche does not import C++ headers, expose aggregates/resources/strings,
translate exceptions, create a stable general native ABI, write files, choose
tools, build libraries, publish packages, or claim Windows, macOS, or Linux
support from one local Unix compiler execution. In particular, the C
`_Bool`/C++ `bool` boundary is evidenced only for the explicit compiler/target
used by that gate, not asserted as a universal C++ ABI. The JSON command is an
authority-free projection; the caller owns materialization and compilation.
