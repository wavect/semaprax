# Private Swift/iOS executable evidence

This dependency-free project is a private executable gate for the callable-v3
ownership host. It is not a public Swift API, application framework, or claim of
general iOS support. `scripts/ios-swift-app-v3.sh` generates target-bound C,
builds and inspects a private XCFramework, and runs separate O0 explicit-consume
and O2 deterministic-ARC-deinit applications on an arm64 iOS Simulator.

The Swift wrapper keeps every native operation on one stable `Thread` FIFO.
`deinit` is a nonthrowing fallback; fallible exact evidence comes from
`OwnedSession.consume()`. The deterministic deinit gate releases the last strong
reference explicitly and drains the identical cleanup action path. It does not
claim observation of nondeterministic memory pressure or process-exit cleanup.
