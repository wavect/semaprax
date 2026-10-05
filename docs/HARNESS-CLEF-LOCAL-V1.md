# Self-hosted Clef decision adapter v1 (MR-06)

Status: **contract-tested, live-unverified.** Opt-in and experimental; no live Clef inference has been run.
Audience: toolchain contributors and harness adapter authors.

Owner: `packages/semaprax-harness-adapters/clef-local/`. Contract: `decision.evaluate` v1 (`model-route/v1`) and
v2 (`model-route/v2`) in [HARNESS-DECISION-V1](HARNESS-DECISION-V1.md); score semantics as in
[HARNESS-LAYA-JEV-V1](HARNESS-LAYA-JEV-V1.md) (MR-02). Provider id `ai.cloudflare/clef-local-decision`.

## What it is

Clef-Flash is a 9B decision model: a Qwen3.5-9B backbone plus a joint schema head that scores every option of every
question in one forward pass. There is no free-form generation. The model card's execution path is
`joint_schema_model.load_release_model(path, device)` then `joint_schema_model.systemone(model, processor, request)`;
this integration uses exactly that path and no other.

Three separate pieces, none of which depends on the others being started by the adapter:

| Piece | File | Role |
|---|---|---|
| Provisioner | `provision.py` | Explicit, separate. Resource preview by default; `--download --accept-resources` fetches the pinned revision and verifies sha256. Never pip-installs. |
| Worker | `worker.py` | Explicitly started warm process on `127.0.0.1`. Verifies every pinned file, imports the verified `joint_schema_model.py`, serves `GET /readyz` and `POST /v1/systemone`. Offline mode forced. |
| Adapter | `adapter.py` | Stdlib-only stdio adapter. Talks to the worker named by `SEMAPRAX_HARNESS_ENDPOINT` (loopback http only). Starts nothing, downloads nothing, installs nothing. |

Route evaluation never pip-installs, downloads, fetches code, or falls back to a hosted Cloudflare endpoint. A
non-loopback endpoint is refused (`SPX-HPK002`).

## Pinned release (clef.lock.json)

Read from the HF API (`/api/models/Cloudflare/<name>?blobs=true`); sha256 of LFS files from the API, of the small
non-LFS files computed over the bytes at the pinned revision.

| Profile | Repo @ revision | Size | Backbone | Status |
|---|---|---|---|---|
| `clef-flash` | `Cloudflare/clef-flash` @ `17f0b0ad64efb65d273590632833508766b2aae6` | 19,083,365,610 B (17.77 GiB) | Qwen/Qwen3.5-9B | supported (first profile) |
| `clef` | `Cloudflare/clef` @ `2f3de3dd85f379784083b0814d997ab627200f0c` | 54,989,882,273 B (51.2 GiB) | Qwen/Qwen3.8-27B | **unavailable unless provisioned and verified**; the adapter returns `unsupported` |

Licence: Apache-2.0 (recorded per release, LICENSE file digest pinned). Requirements (model card): `torch` 2.11,
`transformers` 5.10.2, `safetensors`, `huggingface_hub`, plus `pillow` only for image/video (not used here). Tested
upstream on a single H200 with CUDA; the lock's device list is `["cuda"]`, so any other device (mps, cpu) makes the
worker report `unsupported_device` and the adapter answer `unsupported`.

## Call identity

The lock groups files into backbone (shards, index, config, generation_config), head (`joint_head.safetensors`,
`joint_head_config.json`), code (`joint_schema_model.py`) and tokenizer (`tokenizer.json`, `tokenizer_config.json`,
`chat_template.jinja`, `processor_config.json`). `identity = sha256(canonical{revision, groups: {g: sha256(sorted
"name:sha256" lines)}})`. At start the worker hashes every pinned file before importing any code; `/readyz` then carries
`identity`, `revision`, the four group digests and `digests_verified: true`. The adapter recomputes all of these from its
own lock and refuses (`SPX-HPK008`) on any difference. Every `/v1/systemone` response carries `routing.model = identity`,
and the adapter refuses a response that does not equal the identity verified at readiness.

Receipt (`call`): `checkpoint` = identity, `identity_kind: immutable_checkpoint` only in that verified case (otherwise
`unknown`), `billing: local`, `usage.input_tokens` from the worker (`output_tokens: 0`). Changing the backbone, head, code,
tokenizer or revision changes the identity; the model profile's `checkpoint` must equal it or the profile is refused.
`SEMAPRAX_HARNESS_CLEF_LOCK` overrides the lock path (tests, local forks); that is the host's trust decision, and the
identity then reflects the overriding lock.

## Compatibility matrix

| Case | Result |
|---|---|
| text-only closed choice, v2, <=16 options, state <=4096 B | supported; `option_distribution` scores (per-option softmax), native confidence = chosen probability (`clef.choice_probability`) |
| v1 `model-route/v1` | supported |
| image/video input | `unsupported` before dispatch (profile is text-only; worker also rejects `images`/`videos`) |
| oversize request (`max_wire_bytes`, profile limits) | `refused` `SPX-HPK005` before dispatch; worker body cap 65536 B (HTTP 413) |
| worker not running | `unavailable` `SPX-HPK016` |
| worker cold-starting (`loading`/`verifying`) | `unavailable`, message says cold-starting; retry when `/readyz` is `ready` |
| head/backbone/tokenizer/code file missing or digest mismatch | worker `failed` (`missing_file:` / `digest_mismatch:`), adapter `unavailable`; nothing loaded |
| device not in lock | worker `failed` `unsupported_device:`, adapter `unsupported` |
| worker identity differs from lock | `refused` `SPX-HPK008` |
| profile `clef` (or any unpinned model) | `unsupported` `SPX-HPK014` |
| non-loopback / https endpoint | `refused` `SPX-HPK002` |
| cancellation | adapter closes its connection; the host sees `refused` `SPX-HPK013` (an in-flight forward pass finishes in the worker) |
| generate attempt | the worker replaces every `generate` on the model with a guard that counts and raises; `readyz.calls.generate` must stay 0 |

## Provisioning and local-only smoke path

Sizes above are the reason no live run was made here: the Clef-Flash release is 17.77 GiB before torch, against the
disk rule of at most 1.5 GiB total and 4 GiB free; this host had about 4.3 GiB free. Nothing was downloaded.

On a machine with a CUDA GPU and enough disk:

```sh
cd packages/semaprax-harness-adapters/clef-local
python3 provision.py --dest /data/clef-flash                         # preview only: sizes, deps, disk
python3 -m venv /data/clef-venv && /data/clef-venv/bin/pip install torch==2.11 transformers==5.10.2 safetensors huggingface_hub   # you run this, not the tools
python3 provision.py --dest /data/clef-flash --download --accept-resources
/data/clef-venv/bin/python worker.py --model-dir /data/clef-flash --device cuda --port 8765 &
curl -s http://127.0.0.1:8765/readyz                                  # wait for "state":"ready", digests_verified true
SEMAPRAX_HARNESS_ENDPOINT=http://127.0.0.1:8765 python3 adapter.py     # stdio adapter; host sets the same env
```

The smoke is local-only: no credential, no outbound traffic after provisioning (`HF_HUB_OFFLINE=1`,
`TRANSFORMERS_OFFLINE=1` are forced by the worker).

## Tests

`python3 -m unittest discover -s packages/semaprax-harness-adapters/clef-local/tests` runs the shared SDK conformance
matrix against an instrumented fake worker (a fake model dir whose `joint_schema_model.py` mirrors the real API; no
torch) plus: joint-head path with `load_release_model` once, `systemone` per call and zero `generate`; a generate
attempt is counted and fails; missing and tampered head files, patched code (never imported), identity mismatch,
unsupported device, worker down, non-loopback endpoint, cold start then ready, oversize body, larger Clef unsupported,
and identity changes per component. These prove the adapter and worker contract, not the real model's behavior.
`support.tested` in the descriptor is empty.
