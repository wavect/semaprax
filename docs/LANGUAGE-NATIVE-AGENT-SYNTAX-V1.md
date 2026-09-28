# Language-native Agent syntax v1

Status: implemented bounded profile; **HOSTED GREEN** under the
[v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md). Historical local,
authoring-time, ignored, device/simulator, or separately provisioned evidence
below retains its narrower scope; public promotion, registry publication and
broader product completion remain separately gated.

Audience: language users, compiler contributors, and Agent-system reviewers.

An admitted declaration has this closed shape:

```semaprax
@id("example.agent")
agent Example {
    types {
        @id("example.agent.type.task")
        type task;
        @id("example.agent.type.state")
        type state;
        @id("example.agent.type.observation")
        type observation;
        @id("example.agent.type.proposal")
        type proposal;
        @id("example.agent.type.outcome")
        type outcome;
        @id("example.agent.type.result")
        type result;
    }
    operations {
        @id("example.agent.fn.initialize")
        fn initialize;
        @id("example.agent.fn.observe")
        fn observe;
        @id("example.agent.fn.propose")
        model fn propose;
        @id("example.agent.fn.authorize")
        fn authorize;
        @id("example.agent.fn.execute")
        effect fn execute;
        @id("example.agent.fn.reduce")
        fn reduce;
    }
    runtime_v1 {
        canonical_json "<exact AgentDefinition v1 runtime_v1 object JSON>";
    }
}
```

All thirteen IDs are explicit and locally unique. Roles occur once in this
order. Plain `fn` is `deterministic`; only `propose` is `model fn` and only
`execute` is `effect fn`. Decoded `canonical_json` is capped at 1,310,720
bytes; formatting escapes the source string without changing decoded bytes.

The parser diagnostic `SPX-P124` owns missing identities, duplicate local
identities, wrong role order/names, a non-string compatibility value, and an
over-bound compatibility value. Ordinary missing-token diagnostics remain
`SPX-P104`/`SPX-P106`.

The AST retains exact IDs, closed role/kind enums, decoded runtime JSON, and
spans. Later semantics must check project-wide ID collisions, build canonical
AgentDefinition v1 JSON, and re-admit it. This syntax slice claims no runtime
JSON validation, HIR/graph integration, execution, authority, or backend support.

## Additive embedded execution metadata (v070 candidate)

The bounded extension admits ordinary function bodies for the four deterministic
roles inside `operations`. Each is one normal `@id` function declaration, with
its complete parameter, return, contract and body syntax. Its operation ID is
that ordinary function ID; no additional body ID or copied executable tree is
created. The `propose` model and `execute` effect roles retain their semicolon
reference forms. Embedded deterministic roles admit no generic parameters,
effects, `yields` or `follows`; ordinary function admission and backend semantics
still apply. Two Agents do not create separate function-name namespaces.

An optional block immediately after `operations` and before `runtime_v1` is:

```semaprax
model_wait_v1 { propose = "example.explicit.helper"; }
```

It associates the model role with one explicit, non-generic ordinary function
in the same original source module. The helper ID must differ from the Agent
and all twelve role identities, use the existing canonical ASCII identifier
rule, and fit 240 bytes. Binding the helper does not execute it, admit its
owned wait profile, grant model authority or widen the existing model-wait API.
That runtime consumer independently checks its exact channel/ownership shape.

Only Agents with embedded bodies or this binding opt in. For those Agents,
the four deterministic roles must resolve to explicit functions in their own
original module. Legacy reference-only Agents keep their existing admission.
An embedded body participates in the ordinary function and stable-ID inventories
exactly once. Canonical formatting suppresses only its exact matched origin
entry; a duplicate top-level declaration remains visible and receives the
ordinary S101/S102 diagnostic after reparse. Source association mismatches are
G559; malformed closed grammar is P124. Independent HIR mismatch is H006.

The AST holds an origin index and optional helper binding, never a second body.
HIR holds checked origin metadata and an immutable original-source association.
Public HIR metadata must match that inventory, and every referenced function
must exist. Private decoded strings alone are inert: authenticated cache
binding and original retained-source association replay must rederive and
compare them before reuse or relinking. The private frontend AST compatibility
is v2 and checked-module HIR compatibility is v4; old compatible-cache payloads
are not interpreted as the new carriers. Public cache-work reports and MAC
envelopes retain their existing versions. Actual carrier capacities and dynamic
strings are charged before clone/allocation; helper boxes and bodies are each
accounted once by their owning carrier.

Graph v50 is selected only when source execution metadata exists. It preserves
the entire selected graph base, including protocol/follows, cleanup and loan
facts, changes the schema header and appends `agent_execution` with the exact
`base_schema` and Agent-ID-sorted `agents` rows. Each tagged row has `agent`,
four ordered deterministic `operations` entries (`role`, `operation_id`,
`function_id`, `origin`), and optional `model_wait` (`model_operation_id`,
`helper_id`). Origin is `embedded` or `reference`; each function ID equals its
operation ID. Exact retained-source replay rejects missing, changed or downgraded
edges with G411. Legacy sources retain their prior graph schema and bytes.

Workspace graph v4 adds module/path-qualified rows for the selected reachable
closure, over its actual v1/v2/v3 base. Package graph v4 adds package/version
qualified rows only through the existing authenticated scalar package route;
its Agent metadata does not admit nominal owned-State lifecycle execution or
change the scalar ABI. Existing protocol/follows bounds and legacy bytes stay
unchanged; the package Agent inventory is bounded to 256 (four selected packages
with at most 64 source Agents each). ProgramRoot already binds raw source/schema
and canonical semantic source through its existing segments. Comments may change
the raw-source root while preserving the semantic-program digest.

The extension retains exact AgentDefinition v1, AgentGraph v1 and Runtime-v1
compatibility products. Embedded placement uses the ordinary interpreter and
backends; the model-wait binding is descriptive until its independently checked
runtime route consumes it. This syntax/metadata packet does not complete the
owned Agent initialize/observe/model/authorize/effect/reduce/Step lifecycle.
Local focused and complete integrated gates own acceptance; no hosted evidence
or completed R20 status is claimed by this candidate specification.
