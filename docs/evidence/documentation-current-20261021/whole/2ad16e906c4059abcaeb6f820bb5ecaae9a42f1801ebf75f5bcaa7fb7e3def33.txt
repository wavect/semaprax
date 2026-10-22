# Current344 direct-reader input composition

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Base: `a82e00cc83f99b16249181c45710d962598577db`. This is a source-only repair of five actual owning-test failures from the current344 run. It grants no compiler admission, executable qualification, or production authority.

The live failures identified missing composition at direct raw-input sites:

| Owner | Missing current-to-historical stage |
| --- | --- |
| Source custody a | `ReplayPhysicalRefusal` moved from the test reexport into the production reexport in the complete cleanup-plan root |
| Source custody b | the later `ids/instance_match` module registration remained in the complete HIR ids root |
| Runtime JSON hostile-I/O owner | raw latest runtime source bypassed Maina3 → Main8a and reached the older release normalizer directly |
| Projection purity/facade owner | the historical facade reader received the later complete joined alias source, including owned-directory growth controls |
| Codec808 preparation witness | the complete original string-vectors assertion received the later root-canonicalization insertion |

`bridges.json` records four unique full-input SHA256 pairs and exact replacement preimages. Its complete bytes are independently length/digest bound in the shared test-only `current344.rs` helper. The runtime owner instead uses the existing normalizers and immutable Maina3/Main8a inverses; it reaches the unchanged release common SHA256 `048584a387e19ce44d46428072db42fedd4a4f94f67f3fac0622a2ce3fc0961f`.

Every historical expected file, table, reader guard, and assertion remains intact. The projection purity scan still receives the entire actual source join before its separate historical facade comparison. New authored controls refuse appended drift, empty/truncated input, and the historical input supplied as current. The three complete raw-source inventories include the shared helper.

Read-only source verification:

```sh
TMPDIR=/Volumes/Semaprax/runtime-tmp PYTHONDONTWRITEBYTECODE=1 python3 docs/evidence/current344-input-composition-20261016/verify-source.py
```

This checks all four bridges, reconstructs their existing original owner inputs, checks twelve wrong-input SHA preimages, follows the runtime chain, preserves historical tables, and checks the three new raw joins and changed module sizes. It does not execute Rust tests.

The five existing exact selectors remain:

- `cleanup_plan::replay::live_oracle::source_custody::a::complete_original_sources_and_compiled_owning_receivers_remain_exact`
- `cleanup_plan::replay::live_oracle::source_custody::b::complete_original_sources_and_compiled_owning_receivers_remain_exact`
- `source_reflows_merge::runtime_json::tests::fixed_projection_json_transport_refuses_hostile_io_before_parsing`
- `semantic_graph::expected_projection_source_boundary_is_pure_and_keeps_shared_helpers_in_root`
- `workspace_graph::tests::codec808_frontier::primitive_string_codec_actual_preparation_reports_existing_first_operation_without_replacing_failure`

New authored regression selector: `source_reflows_merge::current344::tests::complete_direct_reader_preimages_refuse_stale_truncated_and_historical_inputs`. The same helper compiles in the workspace harness as `semantic_graph::current344_input::tests::complete_direct_reader_preimages_refuse_stale_truncated_and_historical_inputs`; the projection owner exercises its virtual-root refusal inputs too.

Pending: final joined endpoint/authority refresh after integration with other lanes, ordinary owning-test execution, Clippy, full qualification, and graph refresh. No Cargo, build, or Rust test was run in this private lane. Keep the a82 postpin receipt and earlier joined receipt unchanged; use a separately labeled refresh phase.
