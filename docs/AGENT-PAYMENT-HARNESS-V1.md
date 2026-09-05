# Agent Payment Harness v1

Status: bounded local implementation; public, hosted, language-syntax, and
production-payment claims remain unsupported.

Audience: compiler contributors, Agent Runtime integrators, and Economic Agent
host authors.

## Purpose

Agent Payment Harness v1 closes the local construction gap between the
canonical [Language-Native Agent Object v1](LANGUAGE-NATIVE-AGENT-OBJECT-V1.md)
compiler, Agent Runtime v1, and [Economic Agent v1](ECONOMIC-AGENT-V1.md).
One compiler-owned object binds an exact AgentDefinition, its derived
AgentGraph, and one independently admitted Economic Agent Policy. A caller can
instantiate that object with two disjoint hosts and run the complete handoff:

```text
AgentDefinition v1 --compile/replay--> AgentGraph v1
        |                                  |
        `-- exact Runtime v1 profile ------+
                         |
                 injected AgentHost
                         |
               completed final message
                         |
              canonical Payment Intent
                         |
     source-bound Economic Policy + injected authorities
                         |
          reserve / simulate / approve / sign / broadcast / reconcile
```

The model output is untrusted data. It cannot supply the Economic Policy,
approve its own payment, obtain signing material, select a host, or mint chain,
custody, journal, or broadcast authority.

## Canonical payment graph

`semaprax.agent-payment-graph.v1` is compact UTF-8 JSON with one terminal LF
and at most 65,536 bytes. Its exact ordered fields are:

1. `schema`;
2. `agent_definition_digest`;
3. `agent_graph_digest`;
4. `economic_policy_digest`;
5. `proposal_schema`;
6. `flow`;
7. `authority_boundary`;
8. `evidence_chain`; and
9. `nonclaims`.

The digest domain is
`semaprax.agent-payment-graph.digest.v1\0`. The policy digest is the canonical
Economic Agent Policy v1 digest, not a caller assertion. Graph compilation
first replays AgentDefinition v1 and independently admits the exact Economic
Policy. Bundle verification recompiles both graphs and exact-compares their
bytes. `SPX-G505` reports a payment-graph replay mismatch.

The graph records the ordered execution stages and these authority facts:

- model output is untrusted data;
- the payment policy is source-bound;
- approval is injected;
- custody is injected and opaque; and
- chain observation and broadcast are injected and limited to the test-network
  profiles admitted by Economic Agent v1.

## Public Rust surface

`CompiledAgentDefinition::instantiate` constructs the existing Runtime v1
kernel from the compiler-owned exact profile. The additive payment composition
is:

```rust
let compiled = semaprax::agent_harness::compile_agent_payment_graph(
    agent_definition,
    economic_policy,
)?;
semaprax::agent_harness::verify_agent_payment_graph_bundle(
    agent_definition,
    economic_policy,
    compiled.agent().graph().canonical_json(),
    compiled.graph().canonical_json(),
)?;
let mut harness = compiled.instantiate(
    agent_host,
    economic_host,
    cancellation,
)?;
let run = harness.run_payment(task)?;
```

`AgentPaymentRun` retains the AgentDefinition, AgentGraph, and payment-graph
digests alongside both opaque replayed run results. Economic Evidence already
binds the exact Agent Evidence digest under the Economic Agent v1 contract.

## Executable evidence

The focused gate is:

```sh
cargo test --locked -p semaprax --test agent_runtime_v1 agent_payment_harness_v1 -- --nocapture
cargo test --locked -p semaprax --lib economic_agent::tests -- --nocapture
```

It proves deterministic graph compilation, exact bundle replay, authority-fact
tamper rejection, runtime generation of a canonical EVM Payment Intent, and the
handoff into the policy/journal/chain state machine. The Economic Agent suite
separately runs successful EVM, Solana, Bitcoin, and x402 authority flows,
hostile policy/intent cases, restart reconciliation, cancellation, and all
document replay gates. Its sealed Agent fixture now originates from a compiled
AgentDefinition and AgentGraph.

## Nonclaims and next gates

This v1 composition does not add `.spx` agent declarations, compiled
`initialize`/`observe`/`authorize`/`reduce` transitions, typed proposal
generation, provider deployment documents, built-in hosts, mainnet authority,
or production payment support. Runtime v1 and Economic Agent v1 remain the
execution kernels. The next language-owned gate must define agent syntax and
derive the proposal grammar and transition bindings from checked source rather
than the compatibility definition.
