# Agent Observation Schema v1

Status: compiler-owned bounded same-module slice.

Audience: compiler, Agent-runtime, semantic-service, and generated-consumer maintainers.

This contract derives one closed Observation/context grammar from a
language-native source Agent and the verified HIR type bound to its
`observation` role. The role identity must resolve in the same checked module
to one persistent, monomorphic record or non-empty variant. This slice does not
perform cross-module role resolution.

## Canonical derivation

`compile_source_agent_observation_schema` selects exactly one Agent by stable
identity, lowers it through the unchanged AgentDefinition v1 compiler, checks
that the frozen definition and retained HIR agree on the Observation role, and
derives the shape through the same role-neutral scanner used by Agent Proposal
Schema v1. Display names are excluded. Record fields and variant cases remain
in verified declaration order and carry their persistent identities.

The admitted scalar set is `bool`, `i32`, `i64`, `u8`, `usize` as exact `u64`,
and bounded UTF-8 `string`. Integers use canonical decimal strings. Nested,
generic, borrowed, resource-bearing, byte-owning, floating-point, class, and
otherwise unsupported shapes fail closed. The canonical schema is bounded to
262,144 bytes; an Observation document is bounded to 65,536 bytes; each string
field retains the shared 4,096-byte bound.

Schema and type-revision digests use distinct domain-separated SHA-256
domains. `verify_source_agent_observation_schema_bundle` independently
rederives and byte-compares a supplied schema. The decoder requires exact
closed keys, stable field/case identities, exact schema digest, canonical key
order, canonical scalar encoding, and a terminal LF.

## Authority and compatibility

An Observation is data. Derivation and decoding grant no authorization,
publication token, capability, provider or model trust, tool access, or host
authority and perform no effects. The slice changes no AgentDefinition v1,
AgentGraph v1, Runtime Profile v1, Proposal Schema v1, or agent-free Project
bytes. Type and operation role identities are bindings and may name compatible
persistent declarations; only the Agent declaration identity must be fresh
against ordinary Project declarations.
