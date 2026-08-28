# Rust-host sanitizer evidence

Audience: maintainers, host integrators, and compiler contributors.

Status: **green public Linux Rust-host ASan evidence**.

SEMAPRAX has two intentionally separate Linux sanitizer lanes:

- `callable-host-sanitizers` uses stable Rust and Clang AddressSanitizer plus UndefinedBehaviorSanitizer on compiler-generated C providers. It proves the provider/ABI boundary, but it does not instrument the Rust host.
- `rust-host-address-sanitizer` instruments `semaprax-native-host` and its Rust standard-library dependencies with AddressSanitizer, links the generated Clang provider into the same sanitizer runtime, and executes the real callable host and authoritative generated-callable corpus.

The Rust lane is pinned to `nightly-2026-07-16`, whose compiler is `1.99.0-nightly` at commit `d0babd8b6b05ef9bb65d42f928cef4129d64cf65`. The exact pin prevents a moving nightly from silently changing this evidence. It uses:

```text
-Zsanitizer=address
-Zexternal-clangrt
-Clinker=clang-18
-Clink-arg=-fsanitize=address
-Cforce-frame-pointers=yes
```

Cargo also receives `-Zbuild-std --target x86_64-unknown-linux-gnu`, so the target standard library is rebuilt with the sanitizer instead of leaving a substantial Rust dependency uninstrumented. Every Rust command runs through `rustup run nightly-2026-07-16`, and the Ubuntu 24.04 job uses the preinstalled `clang-18` for the one mixed Rust/Clang ASan runtime. The runner image still receives security updates, so this is a major-pinned sanitizer environment rather than a bit-reproducible operating-system image.

## Fail-closed activation proof

[`scripts/verify-rust-host-asan.sh`](../scripts/verify-rust-host-asan.sh) rejects the job unless all of the following hold:

1. The audited Linux lane, exact toolchain, exact compiler commit, sanitizer flags, external Clang runtime, frame pointers, and fail-fast/leak-detection options are active.
2. A `cfg(sanitize = "address")` compile-time probe succeeds.
3. That probe's binary has defined or unresolved `__asan_` symbols.
4. An intentional Rust heap use-after-free terminates unsuccessfully with an AddressSanitizer diagnostic. Successful execution or a missing diagnostic fails the gate.
5. A fresh, nonincremental, verbose `semaprax-native-host` test build contains the sanitizer and external-runtime flags on the host crate's actual `rustc` command line.
6. The resulting host test binary has `__asan_` callbacks.
7. The real `runtime_callable_host` suite and authoritative generated callable corpus both execute under that instrumented process. In this lane the generated provider is compiled with Clang ASan and must expose ASan callbacks before it can be loaded.

The stable [`tests/rust_host_asan_contract.rs`](../tests/rust_host_asan_contract.rs) quality gate statically checks the workflow, verifier, executable bit, corpus opt-in, pinned toolchain, activation probes, and absence of permissive skip constructs. This makes accidental removal or silent conversion into an optional lane fail ordinary CI even before the nightly job runs.

## Evidence boundary and nonclaims

The exact lane passed in [public run 31259216533, job 93107277065](https://github.com/wavect/semaprax/actions/runs/31259216533/job/93107277065). The containing [workflow run](https://github.com/wavect/semaprax/actions/runs/31259216533) was also fully green across the current hosted-CI matrix: Linux, macOS, Windows, Rust 1.85, dependency policy, the stable generated-provider ASan+UBSan lane, and the pinned-nightly Rust-host ASan lane. This is not app-platform or mobile evidence. It records runtime evidence for the bounded Linux contract above; configuration and static validation alone would not be sufficient.

Rust's `-Zsanitizer` interface does not provide an UndefinedBehaviorSanitizer mode. Rust-host UBSan is therefore not claimed; the stable generated-provider lane remains the UBSan evidence. This lane also does not prove absence of memory bugs, Miri coverage, macOS or Windows Rust-host instrumentation, mobile sanitizer profiles, malformed-response fallback cleanup, quiescence, or a recovery protocol. It does not weaken or reopen the public `SPX-B104` resource gate.

The implementation follows the official Rust documentation for [sanitizers](https://doc.rust-lang.org/nightly/unstable-book/compiler-flags/sanitizer.html), [Cargo `build-std`](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#build-std), and [`external-clangrt`](https://doc.rust-lang.org/unstable-book/compiler-flags/external-clangrt.html). The pinned compiler is independently recorded in Rust's [2026-07-16 nightly manifest](https://static.rust-lang.org/dist/2026-07-16/channel-rust-nightly.toml).
