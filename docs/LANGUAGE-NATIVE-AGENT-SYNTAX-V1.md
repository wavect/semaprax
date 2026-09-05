# Language-native Agent syntax v1

Status: locally exercised additive parser, AST, and canonical-formatter tranche.
Semantic execution remains a separate gate.

Audience: language users, compiler contributors, and Agent-system reviewers.

One declaration has this closed shape:

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

All thirteen identities are explicit and locally unique. Type roles and
operations occur exactly once in the displayed order. Plain `fn` means
`deterministic`; only `propose` is `model fn`, and only `execute` is `effect
fn`. The decoded `canonical_json` string is bounded by the AgentDefinition v1
maximum of 1,310,720 bytes. The canonical formatter uses ordinary SEMAPRAX
string escaping and never changes the decoded bytes.

The parser diagnostic `SPX-P124` owns missing identities, duplicate local
identities, wrong role order/names, a non-string compatibility value, and an
over-bound compatibility value. Ordinary missing-token diagnostics remain
`SPX-P104`/`SPX-P106`.

The AST retains exact stable IDs, closed role/kind enums, decoded runtime JSON,
and source spans. The following semantic tranche must validate stable-ID
collisions project-wide, construct canonical AgentDefinition v1 JSON, and
admit it through the existing AgentDefinition compiler. This frontend tranche
does not claim valid runtime JSON, HIR integration, graph population,
execution, provider/tool authority, or backend support.
