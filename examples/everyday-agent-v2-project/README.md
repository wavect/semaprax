# Everyday Agent V2 Project readiness

This sibling preserves `examples/everyday-agent-project` as the Lifecycle V1
durable-checkpoint product. It supplies an independently authenticated,
multi-module Project whose source-declared Agent has the checked V2 `Step`
shape and uses the exact OpenCode provider/model identity admitted by the
private source-live repair route:

```
opencode/muse-spark-1.3-contributor-free
```

The Project includes the Agent source, a bound target declaration, and a test
module. Its integration regression authenticates the Project and compiles the
linked Agent lifecycle, checking the v3 Project-associated lifecycle document,
its V2 `Step` identity, and the compiler-derived proposal schema.

This is readiness work for #323, not repair evidence. The committed target is
passing. #323 requires an operator-authorized real-provider run against a
genuinely failing Project, an interrupt after a retained settlement, a resumed
run with zero duplicate dispatch for that settled attempt, and independently
observed passing checks after the repaired candidate. No credentials, provider
execution, candidate-test authority, source mutation, or publication authority
is stored in this project.

When an authorized operator selects a real failing Project, they must use the
private `source-live repair-tested` route and its absolute config, checkpoint,
OpenCode executable, and empty scratch operands as specified in
[`SOURCE-LIVE-CLI-V1`](../../docs/SOURCE-LIVE-CLI-V1.md). This example does not
provide a reusable repair configuration because the route binds operator-owned
absolute paths and a restart-stable deadline.
