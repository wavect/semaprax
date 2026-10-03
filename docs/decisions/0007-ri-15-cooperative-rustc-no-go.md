# ADR 0007: Do not adopt cooperative `rustc_private` monomorphization for RI-15

Audience: maintainers evaluating [RI-15](https://github.com/wavect/semaprax/issues/373).

- Status: no-go for the `wavect/v080` RI-15 experiment, recorded 2026-10-03.
- Scope: this closes the optional cooperative-compiler experiment only.  It
  does not close RI-01, RI-14, or the Rust interop epic.
- Evidence: [`experiments/ri-15-rustc-private-blocker`](../../experiments/ri-15-rustc-private-blocker/)
  pins the observed compiler identity and carries a source-only reproduction.

## Proposed capability

The specific capability under evaluation was a Rust generic consumer calling a
safe trait implementation backed by a non-zero-sized Semaprax value, then
returning through the same callback cycle.  That could remove hand-maintained
monomorphic adapter coverage for a narrow class of generic callbacks.

The stable path already uses a checked BindingPlan plus generated Rust and
C-compatible shims (ADR 0006).  Its generated Rust surface forbids unsafe code
and its private FFI owns the explicit physical boundary.  It does not claim to
implement arbitrary Rust traits, preserve Rust borrows, or treat matching
layouts as interchangeable.

The [Valen article](https://verdagon.dev/blog/golden-spike-reviving-vale-valen)
reports a compiler-cooperation prototype: Valen structs implementing Rust
traits work there only when zero-sized; filled structs were in a separate
prototype, while closure borrowing and generational references remained
limited or disabled. Those observations are not Semaprax features. This
experiment has not produced a Semaprax trait implementation, cross-language
borrow proof, or cooperative monomorphization.

## Reproducible blocker

The fixture pins the only compiler observed for this evaluation:

| field | value |
| --- | --- |
| source revision | `619d3f742ef5992c85c9899e6955acc4f2ca374e` |
| rustc release | `1.98.0` |
| rustc commit | `88d9e12ae178fab0fb5cc050a94da85685d449ea` |
| host target | `aarch64-apple-darwin` |
| required private crates | `rustc_driver`, `rustc_interface` |

The pinned Homebrew distribution cannot resolve `rustc_driver` or
`rustc_interface` from its sysroot. The fixture's `reproduce.sh`
checks the exact commit and host, then invokes `rustc` directly with a scoped
`RUSTC_BOOTSTRAP=1`; it expects resolution of `rustc_interface` to fail.  It
uses neither Cargo nor rustup, downloads nothing, and writes only to a
temporary directory. At `10bf59b6e` on the named host, the command exited
zero after observing `E0463` for both private crates. This is a reproduced
toolchain blocker, not a successful compiler-integration fixture.
A mismatched `rustc -Vv` shim was rejected before the fixture invoked a
compiler, so a different commit cannot silently reuse this observation.

An independent default-toolchain check at the same revision passed:
`CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
CARGO_PROFILE_TEST_DEBUG=0 CARGO_TARGET_DIR=$PWD/target/ri03 cargo check
--offline --locked -p semaprax` (exit zero, 55 seconds). The experimental
fixture has no Cargo manifest and is absent from that build. The two focused
native interop compiler projections also passed 1/1 each before this
documentation-only commit. No patched compiler, rustc-private component, or
generated binding artifact was installed.

The unavailable compiler component is an honest precondition failure.  A
patched compiler checkout could remove this immediate blocker, but would add a
new product dependency that must be rebuilt, pinned, distributed, and audited
before it establishes any language capability.

## Decision

Do not create or ship a patched compiler integration.  The proposed benefit
does not justify an unowned compiler fork, rustc-private API rebase work, and
new cross-toolchain trust boundary while stable generated adapters remain the
supported route.  RI-14 remains separately responsible for evaluating an
admitted Semaprax-to-stable-Rust lowering; it is not evidence for arbitrary
Rust trait or borrowing support.

No maintainer owns a patched rustc distribution or its recurring private-API
rebases. That work would require a pinned build, artifact publication and
audit for each compiler update, in addition to the fixed-point and ownership
proofs below. This cost is incurred before any supported signature beyond the
stable generated-adapter path is demonstrated.

No SEMAPRAX compiler, backend, generated package, default toolchain, or
release requirement changes under this decision.  The fixture is intentionally
outside normal test selection because an absent private compiler component is
the asserted condition.

## Conditions to reopen

A future proposal may reopen RI-15 only with all of the following before
implementation work is claimed complete:

1. A maintained owner and published patch series, each bound to an exact rustc
   commit, private API inventory, target triple, and artifact digest.
2. An executable fixed-point fixture that deduplicates canonical
   package/type/const identities, has a finite request bound, and covers cycle
   and reentrancy rejection.
3. A stateful, non-zero-sized Semaprax value implementing one safe Rust trait
   through Rust-to-Semaprax-to-Rust callback execution, plus ownership,
   lifetime, aliasing, cleanup, panic, and unsupported-shape negatives.
4. A measured comparison with generated monomorphic adapters and RI-14, naming
   removed maintenance, added signature shapes, compile/runtime cost,
   backend coupling, and rustc rebase cost.
5. Exact mismatch rejection for compiler, patch, target, and binding artifacts;
   an independent stable Semaprax build must remain unaffected when the
   experiment is disabled.

Until those conditions have executable evidence, this fixture is a blocker
record only.  It does not support a claim of upstream Rust support, a hosted
run, a physical-device result, or a production interop surface.
