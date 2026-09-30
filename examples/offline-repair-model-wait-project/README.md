# Offline repair through a model wait

Run the fixed, credential-free demonstration from the repository:

```sh
cargo run --locked -p semaprax-toolchain -- source-live offline-repair-model-wait
```

FixtureAgent supplies a checked `await_proposal` function that yields its Copy
Observation and receives its Copy Proposal. The public runtime uses the real
scripted model adapter, existing grants and typed repair effects. The first
candidate fails checked validation; the second receives that diagnostic feedback
before producing a replacement preview. Original source is not modified.

The report retains the exact wait evidence and root, v7 journal, two model calls,
two effects and 4000 wait fuel. The key and store are in-memory fixtures. This
profile supports interpreter model waits; owned State/Bytes suspension and
native/Wasm yields remain open. See
[Source Model Wait v1](../../docs/SOURCE-MODEL-WAIT-V1.md).
