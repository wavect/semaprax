# Native collection-record pure tests v1

Status: authored source, focused execution pending. This is an explicit test
protocol successor; the frozen [native test v1](PROJECT-NATIVE-TEST-V1.md)
profile set, schema, output and v31 refusal remain unchanged.

Select `semaprax test <project> --target native-collection-record-pure.v1`
with optional `--json`, `--native-timeout-ms` and
`--native-max-output-bytes`. Only exact Project v31
`language-command-io.collection-record.v1` is admitted. Every other Project
profile refuses this selector with SPX-B104. Diagnostic timing and interpreter
limits refuse. `--target native` still selects only the original source-command
profiles, including its original v31 SPX-B104 refusal.

The Rust selector is `ProjectNativeTestProfile::CollectionRecordPureV1` passed
to `native_test_roots_with_profile` and `build_native_test_with_profile`.
Existing methods call the original SourceCommandV1 selector. Admission replays
the complete ordinary v31 typed HIR validator with no command/effect exemption.
The declared test closure must have no permits or function effects. Native
emission is the existing ordinary pure scalar-root C emitter after that replay;
it supplies no argv, filesystem, stdin, stdout command or other provider. The
root list and order remain the manifest-declared main followed by explicit-ID
zero-argument i64 `test_` roots in identity order. An undeclared ID refuses.
Pure test roots print the existing canonical i64 line and pass only on zero.
Typed private helpers keep all ordinary ownership, layout, cleanup and status
validation; this selector cannot bypass a source capacity or backend refusal.

The explicit JSON envelope is
`semaprax.native-test.collection-record-pure.v1`, with target
`native-collection-record-pure.v1`. It has the same four closed envelope fields
(schema, target, passed, cases) and original complete ordered case fields and
canonical result interpretation. Ordinary host diagnostic rendering is unchanged.
This reuses the original bounded child runner, null stdin, Project working
directory, closed process capture, timeout/output/read refusal and settlement,
private scratch authentication/discard and sticky host failure selection. Defaults
remain 10000ms and 65536 combined output bytes; maximums remain 600000ms and
1048576 bytes. Selecting it executes native code only under that explicit host
operation; a starter receipt alone cannot execute it.

Pending gates: `cli_new_project_v1 application_starter::tiny_collection_record_selector_executes_typed_native_roots_and_keeps_legacy_refusal`
independently exercises a small real v31 typed Report helper, native declared
roots, undeclared-root prepublication refusal and old CLI target refusal.
`--lib project::native_test::collection_record_pure_tests::` independently admits
its checked typed closure and refuses checked effectful source, module permits
and hostile function effects. The full starter ShiftSim15 journey and explicit
selector parse controls remain required alongside old native-target v31 refusal, v31 effectful test refusal, wrong profile,
undeclared root refusal, and original source-command result/timeout/output/
settlement receivers. No backend or application qualification is claimed.
