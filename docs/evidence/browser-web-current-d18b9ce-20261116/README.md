# Calculator browser graph funding refresh

The [hosted Rust 1.88 job](https://github.com/wavect/semaprax/actions/runs/43963457945/job/133637397602) captured both graphs and Web manifests at `d18b9ce32`. The native captures use Rust 1.97.1 on macOS arm64 with byte-identical compiler source. Both profiles preserve the exact calculator sources, revisions, scalar ABI and Web artifact checksums.

| Profile | Fixture | Previous builder bytes | Current builder bytes |
| --- | --- | ---: | ---: |
| hosted | baseline | 2,025,148 | 2,020,284 |
| hosted | renamed | 2,025,148 | 2,020,284 |
| native | baseline | 2,025,204 | 2,020,340 |
| native | renamed | 2,025,204 | 2,020,340 |

For each graph, restoring the builder total and graph digest reproduces the complete prior wire. The Web manifest changes only its exact Project graph digest. The source inverse restores both the previous browser runner and the previous graph receiver.

Pinned Playwright 1.62 ran the three Chromium cases against each manifest profile. All six passed, and strict TypeScript checks passed for the three generated consumers. An isolated control with the previous hosted digest was refused before Chromium launched. The Rust Wasm integration harness passed all 246 browser contract tests, including exact prior wire restoration and semantic drift refusal. The source-custody check passed with the regenerated authorities.
