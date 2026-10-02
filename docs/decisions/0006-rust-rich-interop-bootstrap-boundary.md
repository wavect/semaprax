# ADR 0006: Bootstrap rich Rust interop through generated C-compatible shims

Audience: maintainers and contributors implementing [RI-01](https://github.com/wavect/semaprax/issues/359).

Status: accepted design direction; no callable rich-interop implementation or
support claim follows from this decision.

## Decision

RI-01 uses a compiler-owned, checked-in BindingPlan to generate two artifacts
from one selected binding: a Rust adapter that makes the monomorphized Rust
call and a C-compatible thunk/header pair that forms the physical boundary.
The existing native C11 path remains the initial SEMAPRAX side of that
boundary. Rust source stays ordinary and contains no SEMAPRAX annotation or
handwritten FFI export.

The plan, adapter, declaration projection, descriptor and bundle are separate
domain-bound inputs. They must be authenticated and replayed before build or
foreign execution. The plan grants no authority.

## Why this boundary

Rust item symbol names, layouts, unwinding behavior and `repr(Rust)` are not a
stable ABI. Linking a guessed symbol would make a compiler version and
implementation detail part of the language contract without a checked
compatibility rule. A generated thunk can instead expose exactly the scalar
arguments, status and cleanup protocol that the selected Semaprax declaration
records.

Generating the Semaprax declaration and Rust adapter from the same plan keeps
stable IDs, parameter order, effects, substitutions, failure mapping and
target selection from drifting. The checked-in fixture is intentionally small:
it validates the path before RI-03 supplies automatic package indexing.

## Deferred alternatives

RI-14 may lower an admitted Semaprax subset to stable Rust source. That would
move more code generation into Rust, but it does not discover or safely invoke
arbitrary external Rust APIs and therefore cannot replace RI-01's selected
binding boundary.

RI-15 may evaluate cooperative patched-rustc monomorphization. It might expose
more precise compiler facts, but it changes toolchain trust, reproducibility,
maintenance and compatibility obligations. It is not required to make the
bootstrap fixture executable and must not silently replace it.

Both routes remain experiments with their own acceptance criteria. Neither
permits direct linkage to guessed Rust symbols, `repr(Rust)` values, or an
unreviewed unwind across C.

## Consequences

- The initial profile is native-only and static. Interpreter and ordinary Wasm
  reject before foreign execution.
- The first value boundary stays Copy-scalar. Ownership, aggregates, resources,
  borrowed references, traits and async require later contracts and executable
  cleanup evidence.
- Rust `Result::Err`, Semaprax semantic failure and caught Rust panic retain
  separate selected statuses and cannot publish a success result.
- Cargo preparation, exact dependency locking and automatic index discovery
  belong to later work; no ambient Cargo configuration becomes authority here.

The exact schema, diagnostic reservations and evidence gates are owned by
[Native Rust Rich Interoperability v1](../NATIVE-RUST-RICH-INTEROP-V1.md).
