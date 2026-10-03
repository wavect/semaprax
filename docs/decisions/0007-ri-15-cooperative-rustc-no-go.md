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

## Reproducible blocker

The fixture pins the only compiler observed for this evaluation:

| field | value |
| --- | --- |
| source revision | `619d3f742ef5992c85c9899e6955acc4f2ca374e` |
| rustc release | `1.98.0` |
| rustc commit | `88d9e12ae178fab0fb5cc050a94da85685d449ea` |
| host target | `aarch64-apple-darwin` |
| required private crates | `rustc_driver`, `rustc_interface` |

The observed Homebrew distribution exposes `librustc_driver` but no
`librustc_interface` artifact under its sysroot.  The fixture's `reproduce.sh`
checks the exact commit and host, then invokes `rustc` directly with a scoped
`RUSTC_BOOTSTRAP=1`; it expects resolution of `rustc_interface` to fail.  It
uses neither Cargo nor rustup, downloads nothing, and writes only to a
temporary directory.  This ADR records the source observation and the
reproduction procedure; no compilation was run for this decision.

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
