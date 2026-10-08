# Local graph timing comparison for OPT #602

Three sequential timed debug CLI `graph` invocations per binary and fixture. Each pair also has one untimed captured graph whose JSON, revision, output bytes and SHA-256 are recorded in `graph-timings.json`. All measured invocations exited successfully. No Cargo build, live-agent workload, or isolated revision-hash benchmark was run.

| Fixture | Source bytes | Baseline median (s) | Current median (s) | Lower wall time | Ratio | Current binary source |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| 1000-if | 43,078 | 19.048941 | 16.078231 | 15.60% | 1.185× | `c1224c4c8aca9deb444b696a1d117382891b1881` |
| 1500-string-if | 75,832 | 44.274092 | 39.578029 | 10.61% | 1.119× | `b58386ab04af2a1f58717f86c0cee0d2e0d526fb` |

These differences apply to the whole CLI graph command across compiler versions. They do not establish an isolated #602 speedup. Intervening compiler changes include statement-if canonical formatting. Both graph revisions and whole stdout hashes differ between baseline and current for both fixtures; no cross-version byte equality or semantic equivalence is claimed. The same-formatter revision oracle regressions are a separate correctness gate.

The baseline executable is identified by its preserved filename and source commit. Current build source commits were confirmed by the parent coordinator. The observed checkout head at the initial capture was `b58386ab04af2a1f58717f86c0cee0d2e0d526fb`; the resume observed `4e06ef56bf9a676295aeb70bfe7114ef853a9f4e`. Current binaries precede later documentation/other commits, so this is prior-head executable evidence, not final-head verification.

The current target executable changed after the first fixture. The runner detected its SHA-256 drift and stopped before measuring the second fixture with mixed provenance. The completed second-fixture baseline was retained, and the new current executable was copied to a private performance file before its three timed runs. No median combines different executables.

| Binary label | Source commit | Executable SHA-256 | Bytes |
| --- | --- | --- | ---: |
| baseline | `1e0e988218b51ad626881b87c83c553fc7cddf37` | `1e5b4b0e5bd1c3e27cd09e72c47e8853ac39d035732c8a087c20ec4d663c183e` | 154,013,688 |
| optimized | `c1224c4c8aca9deb444b696a1d117382891b1881` | `e7609dd625e8014ea94a07be0ad1fcde65e23ae2f6411ba0e9779dafa2aa3c97` | 155,611,840 |
| optimized_b583 | `b58386ab04af2a1f58717f86c0cee0d2e0d526fb` | `6e7e7f89dc0090de86a4619ffd7fa9be60518dadef5c0d8f794d3675e3ce3e39` | 155,131,360 |

Timed repeats in seconds:

- 1000-if / baseline: 18.884509, 19.138390, 19.048941.
- 1000-if / optimized: 16.003404, 16.294179, 16.078231.
- 1500-string-if / baseline: 44.462450, 44.258400, 44.274092.
- 1500-string-if / optimized_b583: 39.249173, 39.578029, 40.311988.

Fixture SHA-256 values:

- 1000-if: `de95640185e8c43c8059afa1422fd6808bd242712231f844958749ac83671cec`.
- 1500-string-if: `102b50f11f7b4b3acc0aa90ba84b1c7b00268212f2633eecf99390fb253f706d`.

Full commands, graph schemas/revisions, stdout lengths/hashes, timestamps, stderr, and executable metadata are preserved in the adjacent JSON report.
