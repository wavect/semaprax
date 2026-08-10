# Private Component Model result runtimes v3/v4

This unpublished, standalone Rust crate is hosted evidence for one exact
SEMAPRAX v3 `result<s64, status>` component and one exact v4
`result<result<bool, bool>, status>` component. It is intentionally excluded
from the root workspace so Wasmtime cannot widen the compiler's MSRV or
publish dependency graph.

The runner authenticates immutable compiler-produced component bytes, compiles
them with Wasmtime's Component Model API, requires zero component imports,
instantiates with an empty component linker, and invokes the checked-in WIT
through generated typed bindings. The v4 gate covers both inner result arms,
both boolean payloads, a rejected call that skips a would-be division by zero,
arithmetic and contract statuses, repeated calls on one instance, fresh
instances, and an out-of-band fuel failure. It supplies no WASI context or host
callback and grants no filesystem, network, environment, clock, randomness,
process, logging, or mutable ambient authority.

Run the complete gate from the repository root:

```sh
scripts/component-runtime-v3.sh
```

The CI dependency-policy step may refresh its advisory database. Separately,
the runtime script has two explicit dependency-acquisition commands: one for
the root compiler tests and one for this isolated runner. It then forces every
root profile/source-lock test and isolated formatting, lint, test, build, and
execution command offline, with every dependency-resolving command locked. The
complete script is an exact x86_64 Ubuntu toolchain gate; non-Linux contributors
can run its Cargo commands individually with the pinned Rust release. This
remains private copy-result evidence: it adds no resources, imports,
futures/streams, public backend, or `SPX-B104` admission.
