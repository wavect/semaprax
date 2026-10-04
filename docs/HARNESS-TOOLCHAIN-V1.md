# Harness toolchain integration v1

Status: implemented in source; build and test evidence pending (see Evidence).

## Entry point

`semaprax-full harness <verb> [args]` routes to `semaprax_harness::cli::run`
(verbs per [HARNESS-PROVIDER-V1](HARNESS-PROVIDER-V1.md)). The toolchain builds
`Environment::from_process()`; `compiler` defaults to the full toolchain's own
executable (`std::env::current_exe()`) unless `SEMAPRAX_COMPILER` names another.
The command is `CommandId::Harness`, `Availability::Private`; the host hook is
`PrivateHost::harness: fn(&[String]) -> u8`, which prints its own stdout and
stderr (the `bridge` verb streams stdio).

## Private availability

The standalone crates.io `semaprax` package must not carry private crates in its
dependency closure. `semaprax-harness` is therefore a dependency of the
unpublished `semaprax-toolchain` only. The standalone binary answers
`harness is unavailable in the standalone crates.io package` with exit 2 and
does not list the command in its catalog.

## Bridges (`semaprax_toolchain::harness_bridge`)

| Module | Bridge |
| --- | --- |
| `policy` | `FrozenRoutePlan::to_provider_slots()` to `ProviderPolicy`, exact order and authorized flags. `admit_failover` stays the single owner of forward-only, authorized-only admission. |
| `transport` | Plain-HTTP, loopback-only `HostHttpStreamTransport` over `std::net::TcpStream`: bearer secret from the host (`Secret`, redacted `Debug`), bounded response, chunked/length/close framing, SSE chunks passed through, `cancel` closes the socket (`CancelledAfterDispatch`, classed `Uncertain` by the adapter). Non-loopback or non-`http://` origins are refused (`SPX-HPL002`). |
| `model` | A catalog `LogicalModel` with protocol `responses` becomes an `OpenAiResponsesAdapter` over that transport; `LogicalModelFactory` is a `ProviderAdapterFactory` keyed by logical model id. Other protocols are refused (`SPX-HPL032`), never downgraded. |

HP-16 `model.generate` conformance is delegated to the SDK's
`run_conformance_suite` run against the bridged adapter.

## Evidence

Tests: `crates/semaprax-toolchain/tests/harness_bridge_v1.rs` (fixture loopback
SSE server, cancel, refusal, policy, conformance, CLI, one `#[ignore]` real
Ollama test). Results are recorded in the lane report, not here.
