# Calculator Project graph and Web build subjects

Status: local qualification and hosted Linux Chromium passed.

Audience: compiler maintainers and browser CI reviewers.

The Linux ordinary graph captures are from [run 43425577221, job 131886189325](https://github.com/wavect/semaprax/actions/runs/43425577221/job/131886189325) on `3c8b0f35a` with Rust 1.88.0. The same job printed the actual baseline Web manifest subject when its old pin failed. The native ordinary graphs and complete Web manifests were captured from `a5d61b0b` on macOS arm64 with Rust 1.97.1. The intervening production changes are confined to Rustdoc and source custody.

The ordinary Project graph debits 2,025,148 builder bytes on Linux and 2,025,204 on macOS. The reporting entry used by the Web build adds 21,494 bytes on each profile. Recomputing the domain-separated self-digest of the complete baseline graph with that debit reproduces both observed baseline Web manifest subjects. Applying the same debit to the complete renamed graph reproduces the observed macOS subject and predicts the Linux rename pin.

[Run 43430213441, job 131902479220](https://github.com/wavect/semaprax/actions/runs/43430213441/job/131902479220) confirmed both Linux pins on `bb426bb449925a6f5ce19c1811e991b8e9ed0ad0`. The job produced the [baseline manifest](hosted-calculator-baseline-web-manifest.json), [renamed manifest](hosted-calculator-renamed-web-manifest.json), and [build provenance](hosted-web-provenance.json). It type-checked the three generated declaration consumers and ran the three calculator fixtures and the Project v13 HTTPS fixture in Chromium.

The ordinary Rust Project tests retain their own exact compiler-profile graphs and replay the complete preceding graph controls. The browser runner authenticates the Web build subject, project and workspace revisions, scalar ABI, and all artifact bytes before Playwright starts. The source inverse records preserve the complete preceding runner and Rust verification sources.
