# Project Agent Transport v6 SDK/discovery v1

Status: additive implementation and owning local tests; not transport or package promotion.

Audience: SDK consumers, transport integrators, compiler contributors, and reviewers.

The compiler exposes an authority-free, out-of-band discovery document with
schema `semaprax.project-agent-transport-v6-discovery.v1`. It describes the
unchanged `semaprax.agent-transport.v6` protocol, exactly
`project/api-describe` and `project/npm-build-inline`, and the closed Project,
descriptor, and carrier schema triples for Project v8 through v11. It records
the transport byte ceilings and explicitly identifies TypeScript, Python, and
Rust codecs.

Generated codecs only construct LF-delimited JSON-RPC request bytes and decode
caller-supplied response bytes. They never locate or launch `semapraxd`, read a
manifest, select a path or tool, inspect an environment, open a socket, or
perform filesystem, source, workspace, package, or publication effects. The
caller owns process and byte transport plumbing.

The decoders reject surplus wrapper/result fields, mismatched request IDs,
unknown profile triples, invalid descriptor digests, descriptor/profile
disagreement, and carrier/profile disagreement. Nested descriptor semantic
replay and carrier authentication remain server responsibilities; the SDK does
not relabel structural outer validation as authentication.

This artifact adds no protocol method and does not alter v2-v6 wire bytes. Its
owning gate is the `agent_transport_v6_sdk` module in the Project integration
harness. Generated-client compilation or execution is evidence only for this
codec contract, not registry publication, installed-product support, daemon
peer authentication, or Project profile promotion.
