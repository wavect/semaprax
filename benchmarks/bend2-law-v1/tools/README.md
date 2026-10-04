# LAW-16 source to SMT regeneration

`law16_render_app_negate_smt.rs` reconstructs the rendered SMT-LIB for
`app.negate`, postcondition index `0`, from the Boolean-negation fixture in
the source tree pinned at `cdfc0cdd27aa248951134f70a71bf7d36b4798de`.

The helper was added after that source snapshot and is not a tracked file in
`cdfc0cdd`. Its standalone SHA-256 is
`35185d41143938999b5f9da04cb8c970dd879a4817e502fa67ee3c235dd0383e`.
The previously captured host helper binary was built from bytes with that
digest; this is a declared local association, not a source-tree attestation.

Copy the helper source from the current repository into the pinned source
checkout. The helper file did not exist at the pinned source commit, so it
cannot be copied from inside that checkout. Then run the build offline there:

```sh
cp /absolute/current/repo/benchmarks/bend2-law-v1/tools/law16_render_app_negate_smt.rs \
  /absolute/pinned/cdfc-checkout/examples/law16_render_app_negate_smt.rs
cd /absolute/pinned/cdfc-checkout
CARGO_NET_OFFLINE=true CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 \
CARGO_TARGET_DIR="$PWD/target/law16-host-smt-helper" \
cargo run --locked --offline -j1 --example law16_render_app_negate_smt -- \
  benchmarks/bend2-law-v1/fixtures/boolean-negation-project-v1/candidate/src/app.spx
```

The helper uses public `semaprax::parse`,
`smt_discharge::translate_function`, and
`smt_discharge::render_postcondition_script`, with the installed project proof
limit of 10,000 ms. Its 307-byte output ends with `(get-model)\n`, matching the
already-built capture helper. The installed project proof caller, and the LAW-16
capture script, remove that final 12-byte suffix before passing the 295-byte
input to Z3, as `src/proof_export/installed_project.rs` does.

For the captured source, the input is 295 bytes, with raw SHA-256
`f0938ea89248af8556a3a623759eeba7193213301b2d3bc4f1c478c113490e88` and
`smt_discharge::script_digest`
`e642131dd3d766d2ddfeb9d4f7342914ad800f8fdc9b2d20dd9b5f298c3925a5`.

The historical project `proof_ref` is not a script hash. The installed route
derives it from the complete receipt JSON under the
`semaprax.installed-smt-project-proof.v1\0` domain. Do not compare that value
to either digest above.
