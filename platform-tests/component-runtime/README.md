# Private Component Model result runtime v3

This unpublished, standalone Rust crate is hosted evidence for one exact
SEMAPRAX `result<s64, status>` component. It is intentionally excluded from
the root workspace so Wasmtime cannot widen the compiler's MSRV or publish
dependency graph.

The runner authenticates immutable compiler-produced component bytes, compiles
them with Wasmtime's Component Model API, requires zero component imports,
instantiates with an empty component linker, and invokes the checked-in WIT
through generated typed bindings. It supplies no WASI context or host callback
and grants no filesystem, network, environment, clock, randomness, process,
logging, or mutable ambient authority.

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
remains private scalar evidence: it adds no resources, imports, futures/streams,
public backend, or `SPX-B104` admission.
