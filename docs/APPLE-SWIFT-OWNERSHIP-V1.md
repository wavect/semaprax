# Private Apple Swift ownership adapter v1

Status: implemented and CI-configured behind
`unstable-apple-swift-harness`. Local Rust, generator, source-lock, and strict
host gates pass. The Apple device/simulator compilation, XCFramework
inspection, and two installed arm64-Simulator application paths are green in
[run 31333469714, job
93295293995](https://github.com/wavect/semaprax/actions/runs/31333469714/job/93295293995).

This is one bounded Swift projection of the exact callable-v3
`token.discard-two` fixture. It is not a public Swift package, framework API,
or permission to open `SPX-B104`.

## Frozen boundary

- The generated fixture is target-bound separately for arm64 iOS device,
  arm64 iOS Simulator, and x86_64 iOS Simulator.
- A zero-argument generated wrapper is the only application open entry. The
  internal registration bridge cannot accept caller-selected finalizer-evidence
  hooks; reset and snapshot symbols are fixed and hidden in the generated
  target object.
- The Rust host and exact static lease live on one stable Swift-owned `Thread`.
  They remain `!Send` and `!Sync`; a FIFO protected by `NSCondition` is the
  only route into the native runtime.
- `SPXAJH01` generation-tagged values represent wrapper ownership. They are
  never pointers, native owner handles, or capability bytes.
- `OwnedSession.consume()` is the fallible evidence path. ARC `deinit` enqueues
  the same nonthrowing cleanup action. The deterministic test releases the last
  strong reference explicitly; it does not claim arbitrary memory-pressure or
  process-exit cleanup.
- Every rejection leaves output sentinels unchanged. A genuine precommit
  rejection may restore the same wrapper handle; postcommit uncertainty is
  terminal and poisons/drains the private runtime without retry.
- Successful evidence requires exact finalizers `1:13,0:11`, scalar zero,
  no-owned publication, nonzero receipt/candidate/identity facts, a changed
  ledger digest, healthy host state, an empty handle table at close, and zero
  measured Rust allocations across the irreversible interval.

## Executable gate

The hosted script must build strict-C providers and the Rust static host for
all three targets, combine arm64-device and universal-simulator slices into a
private XCFramework, inspect architecture/platform metadata and symbol/image
allowlists, and compile with Swift 6 complete concurrency checking and warnings
as errors. It then signs, installs, and launches two no-UI UIKit applications
on an arm64 Simulator:

- provider `-O0` with explicit `consume()`;
- provider `-O2` with deterministic ARC `deinit` cleanup.

Both applications must publish one exact app-container result. Missing output,
fallback to host execution, an extra dynamic image, an unexpected exported
symbol, or a mismatched result fails the gate.

## Nonclaims

This tranche does not provide a public XCFramework or Swift API, physical
device execution, distribution signing, SwiftUI or UIKit controls,
accessibility, general lifecycle integration, general resources or imported
finalizers, async/cancellation, arbitrary cross-runtime sharing, process-kill
cleanup, malicious-code containment, public native admission, or full Swift or
iOS platform support.
