# SystemOne decision adapters (HP-11)

`laya-local/` and `jev-hosted/` speak `semaprax.harness-rpc.v1` for
`decision.evaluate/v1` (task `model-route/v1`). `systemone_codec.py` and
`systemone_runtime.py` are the shared codec, transport and frame loop; Python 3
standard library only. Contract and gate: [HARNESS-LAYA-JEV-V1](../../../docs/HARNESS-LAYA-JEV-V1.md).

Tests (fake loopback servers, no network, no credentials):
`python3 -m unittest discover -s packages/semaprax-harness-adapters/systemone/tests`
