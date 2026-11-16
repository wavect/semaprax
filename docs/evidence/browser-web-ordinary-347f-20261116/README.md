# Calculator Web packages use the authenticated Project graph

Status: Archived Project graph and Web manifest captures with focused browser validation.

Audience: Project build and browser harness contributors.

The [hosted Linux job](https://github.com/wavect/semaprax/actions/runs/43779041463/job/133012112341) captured both calculator Web manifests and ordinary Project graphs at `347f85842084f33b3ac8e578ea4bbf8ae33a1d9b` with Rust 1.88.0. A macOS arm64 build at the same commit captured both fixtures with Rust 1.97.1. The provenance files bind each compiler and executable digest.

Both builds reproduced the complete ordinary [Project graph captures](../browser-capture-6b66-20261113/README.md) byte for byte. The only changed manifest field is `project_graph_digest`. Each manifest now carries the digest of its corresponding ordinary Project graph. Those four digests replace the reporting-route digests in the browser harness. The immediate inverse restores every byte of the previous runner before replaying the earlier source and graph checks.

The updated harness passed all three calculator fixtures in Chromium on macOS with the native manifests. It passed the same three fixtures with the exact hosted manifests. Both profiles authenticated the same emitted artifact bytes. The three generated TypeScript declaration consumers also passed strict type checking.
