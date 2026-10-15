# Composed source-domain and receiver registration repair

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Source-only. No Cargo/build/test execution. Source base a1edab67b.

Both current and independently compiled saved SourceIndex receivers now explicitly register the same complete source_index/tests.rs child. Current implicit registration was valid; the saved receiver's changed filename otherwise selected a nonexistent source_index_original/tests.rs directory. All three complete test bodies remain untouched.

The workspace projection inventory now includes the complete new default field-directory, its tests, and the full saved recursive default; the workspace root inventory also joins the new borrowed-help test child. Existing members and the pure authority/effect guard are unchanged. The three newly joined default files use exact explicit imports, preserving their bodies. The one new default source-guard digest renderer uses the existing digest_hex adapter required by sha2 0.11.

Defaults field-directory modules already have genuine valid Rust registrations. The external source parser's custom_path default-directory bug is separately repaired by root; no production registration workaround was added. Actual frozen binary --list/--exact confirmation remains mandatory.

Current source hashes and full inverse composition are a separate follow-up so root can integrate this source change and update registration promptly. Original archives remain immutable; new before snapshots preserve every changed source's full bytes.
