# SEMAPRAX harness adapters

Out-of-tree adapter programs for the harness provider host
([HARNESS-PROVIDER-V1](../../docs/HARNESS-PROVIDER-V1.md)). Each directory holds
one provider: a `harness-provider.json` descriptor, the adapter program, and
its own tests. None of these runtimes enter Cargo's dependency graph or the
compiler build.

`sdk/python` and `sdk/node` are the small shared wire helpers (framing,
handshake, envelope echo); adapters may vendor-copy or import them by relative
path.
