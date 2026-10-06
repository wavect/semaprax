# Mini Jev local decision adapter (experimental)

Local `decision.evaluate` v2 adapter that scores one closed choice with
`r-ms/mini-jev` (pinned `ca612198bfb69f538f029a4615f6d0a18b4f814c`, MIT). Full
contract, provisioning commands, refusal table and what is unverified:
[docs/HARNESS-MINIJEV-V1.md](../../../docs/HARNESS-MINIJEV-V1.md).

| File | Role |
| --- | --- |
| `adapter.py`, `minijev_wire.py` | stdlib-only adapter and shared wire/identity helpers; never installs or downloads |
| `worker.py` | user-started warm loopback worker; imports torch/mini-jev only inside `RealEngine` |
| `fake_engine.py` | counting fake engine (tests/demo only) |
| `decide_once.py` | one decision through the adapter over the SDK frame protocol |
| `harness-provider.json` | descriptor (`org.r-ms/minijev-local`, v2 only, `support.tested` empty) |
| `tests/test_minijev.py` | contract tests, run with `python3 -m unittest discover -s tests` |

Licences: this adapter is part of the repository; Mini Jev is MIT; the model
(for example `Qwen/Qwen3-0.6B`) carries its own licence, and torch/transformers
their own. Dependency pins live in the doc above, not in this directory.
