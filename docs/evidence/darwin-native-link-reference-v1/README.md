# Darwin native tool reference

[Public SDK job 116714584363](https://github.com/wavect/semaprax/actions/runs/38819916075/job/116714584363)
failed two calculator consumers with I231. The builder deliberately keeps that
diagnostic static and suppresses child stderr. The new blocking macOS SDK step
runs a small independent Rust/C reference with the configured compiler images,
empty child environments, and the current prepared Rust staticlib and Clang
link arguments. It prints ordinary tool diagnostics before the full SDK gate.

The reference calls an exported Rust function using the unwind runtime, links
the static library from C and executes the result. Its local Rust 1.88 run
passed with a sampled aggregate child-tree peak of 97,232 KiB under a 200 MiB
stop guard. This does not reproduce or resolve the hosted calculator failure.

A separate local component probe compiled the complete current Unix quarantine
and its original common carriers, excluding unrelated doctor routes. It used
the actual discovery, direct-compiler and sysroot checks, prepared C and Rust
compilation, captured object backing, prepared link, executable admission and
execution on the same small input. It passed with sampled compiler/runtime
peaks of 195,872 and 84,848 KiB under the same stop guard. It does not qualify
the full SDK, held production inputs or the hosted macOS runner.

The ordinary reference is diagnostic evidence only. The existing exact archive
admission and complete package, consumer, callback and backend-equivalence
gates remain blocking and unchanged. The production tool environment, process
settlement, diagnostics and publication authority are unchanged.

## Hosted failure and paired tool selection

The reference in [job 116728654423](https://github.com/wavect/semaprax/actions/runs/38824367721/job/116728654423)
reproduced the hosted failure before the SDK build. Rust staticlib and C object
compilation passed. The `/usr/bin/clang` link rejected
`CommandLineTools/SDKs/MacOSX27.0.sdk/usr/lib/libSystem.B.tbd` with a TAPI
unknown-architecture error for `arm64e.x1-macos`.

The macOS native CI jobs now select
`/Library/Developer/CommandLineTools/usr/bin/clang`, whose companion linker
comes from the same installation as the explicit SDK. The public generic
consumer job also selects that installation's C++ driver and linker path.
Linux resolution and every Windows binding remain unchanged. This changes CI
tool configuration; it does not widen or bypass production authority checks.

The ordinary reference passed locally with the selected CLT driver at a sampled
aggregate peak of 103,472 KiB. The complete current Unix quarantine probe also
passed with that driver, including held-image admission and generated executable
execution; compiler/runtime peaks were 203,264 and 59,888 KiB under the 200 MiB
guard. The complete Product Acceptance and Public SDK CI contract harnesses
passed all 12 tests, including the new eight-job tool/SDK pairing regression.
The hosted CLT 27 configuration and full SDK consumers still need CI evidence.

## Ordinary native dependency headers

[Job 116733021038](https://github.com/wavect/semaprax/actions/runs/38825665695/job/116733021038)
then exposed a separate ordinary Cargo dependency-build failure: the direct
CLT driver could not find `TargetConditionals.h`, `stdio.h` or `stdlib.h`.
An independent local syntax-only check reproduced the missing `stdio.h` with
an empty environment and passed with the explicit CLT `SDKROOT`.

Each affected macOS setup now exports the matching `DEVELOPER_DIR` and
`SDKROOT` in its current shell and into later CI steps. This configures ordinary
Cargo C dependencies and their tool discovery. It changes no machine-wide
developer selection. Held production tools retain their empty environment and
explicit SDK arguments. The CI reference now checks standard-header discovery
with only those two configured values and independently compiles the held-style
C objects with explicit SDK arguments and an empty environment.

The new Python/YAML and all eight shell routes passed lightweight checks. A
subsequent compiler-guarded reference/contract rerun was deferred when another
user compiler exceeded the local RAM precondition; full Cargo and hosted
qualification remain CI work.
