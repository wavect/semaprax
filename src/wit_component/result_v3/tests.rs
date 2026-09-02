use std::path::Path;
use std::process::Command;

use super::*;

const SOURCE: &str = r#"module test.component_result_v3;

@id("component.evaluate")
fn evaluate(left: i64, right: i64) -> i64
    requires right != 7
    ensures result != 9
{
    (left + 1) / right
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn artifact() -> PrivateResultComponentArtifactV3 {
    let program = crate::parse(SOURCE, Path::new("component-result-v3.spx")).unwrap();
    emit_private_result_component_v3(&program).unwrap()
}

#[test]
fn deterministic_result_component_is_exactly_parsed_and_upstream_valid() {
    let first = artifact();
    let second = artifact();
    assert_eq!(first, second);
    assert_eq!(first.wit(), WIT);
    assert_eq!(
        first.source_revision(),
        "sha256:c911665ec227ca17a23964578fa0911a98d64049362a03f9dd86287902891317",
        "source revision KAT changed"
    );
    let validated = validate_private_result_component_v3(
        first.bytes(),
        first.source_revision(),
        first.generated_core_digest(),
    )
    .unwrap();
    assert_eq!(validated.source_revision(), first.source_revision());
    assert_eq!(validated.interface_export_name(), INTERFACE_EXPORT);
    assert_eq!(validated.function_export_name(), FUNCTION_EXPORT);
    assert_eq!(
        Sha256::digest(validated.generated_core()).as_slice(),
        first.generated_core_digest()
    );
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(first.bytes())
        .expect("pinned upstream validator rejected result component v3");

    assert_eq!(
        first.generated_core_digest(),
        [
            213, 95, 118, 160, 230, 151, 71, 119, 92, 50, 147, 253, 97, 110, 47, 240, 58, 146, 28,
            101, 12, 190, 169, 5, 94, 200, 202, 151, 119, 184, 115, 108,
        ],
        "generated-core KAT changed"
    );
    assert_eq!(
        first.profile_digest(),
        [
            222, 215, 48, 247, 69, 152, 10, 90, 86, 167, 93, 149, 152, 80, 26, 184, 41, 24, 28, 36,
            66, 136, 84, 206, 88, 224, 108, 189, 68, 18, 50, 98,
        ],
        "profile KAT changed"
    );
    assert_eq!(
        first.digest(),
        [
            200, 43, 157, 77, 32, 150, 242, 75, 185, 143, 204, 106, 147, 125, 158, 51, 172, 39,
            106, 223, 217, 130, 219, 212, 83, 17, 7, 156, 163, 222, 26, 12,
        ],
        "component DAG KAT changed"
    );
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(first.bytes())),
        [
            125, 134, 68, 56, 73, 72, 245, 145, 214, 254, 121, 41, 208, 207, 237, 158, 171, 121,
            209, 117, 40, 202, 86, 16, 7, 238, 69, 88, 226, 147, 48, 39,
        ],
        "exact component-byte SHA-256 KAT changed"
    );
}

#[test]
fn every_component_byte_prefix_trailing_and_noncanonical_length_reject() {
    let artifact = artifact();
    for index in 0..artifact.bytes().len() {
        let mut hostile = artifact.bytes().to_vec();
        hostile[index] ^= 1;
        assert!(
            validate_private_result_component_v3(
                &hostile,
                artifact.source_revision(),
                artifact.generated_core_digest(),
            )
            .is_err(),
            "result component byte {index} escaped authentication"
        );
    }
    for end in 0..artifact.bytes().len() {
        assert!(validate_private_result_component_v3(
            &artifact.bytes()[..end],
            artifact.source_revision(),
            artifact.generated_core_digest(),
        )
        .is_err());
    }
    let mut trailing = artifact.bytes().to_vec();
    trailing.push(0);
    assert!(validate_private_result_component_v3(
        &trailing,
        artifact.source_revision(),
        artifact.generated_core_digest(),
    )
    .is_err());
    let mut noncanonical = artifact.bytes().to_vec();
    noncanonical.splice(9..10, [noncanonical[9] | 0x80, 0x00]);
    assert_eq!(
        validate_private_result_component_v3(
            &noncanonical,
            artifact.source_revision(),
            artifact.generated_core_digest(),
        ),
        Err(PrivateComponentValidationError::Encoding)
    );
}

#[test]
fn v1_v2_v3_profiles_are_not_confused() {
    let v1 = super::super::emit_private_component_v1();
    let program = crate::parse(
        "module v2; @id(\"app.main\") fn main() -> i64 { 19 + 23 }",
        Path::new("v2.spx"),
    )
    .unwrap();
    let v2 = super::super::emit_private_checked_component_v2(&program).unwrap();
    let v3 = artifact();
    for candidate in [v1.bytes(), v2.bytes()] {
        assert!(validate_private_result_component_v3(
            candidate,
            v3.source_revision(),
            v3.generated_core_digest(),
        )
        .is_err());
    }
    assert!(super::super::validate_private_component_v1(v3.bytes()).is_err());
    assert!(super::super::validate_private_checked_component_v2(
        v3.bytes(),
        v2.generated_core_digest(),
    )
    .is_err());
}

#[test]
fn rehashed_signature_type_order_index_and_lift_hostiles_reject() {
    let artifact = artifact();
    let hostiles = [
        (
            &[0x60, 0x02, 0x7e, 0x7e, 0x01, 0x7f][..],
            5,
            0x7e,
            "core-signature",
        ),
        (&[0x6b, 0x7f, 0x72, 0x04][..], 0, 0x72, "type-order"),
        (
            &[0x00, 0x00, 0x00, 0x02, 0x00, 0x03, 0x00, 0x03][..],
            7,
            0x02,
            "lift-type",
        ),
        (&[0x05, 0x00, 0x00][..], 0, 0x01, "interface-kind"),
    ];
    for (needle, relative, replacement, name) in hostiles {
        let mut hostile = artifact.bytes().to_vec();
        let offsets = hostile
            .windows(needle.len())
            .enumerate()
            .filter_map(|(index, window)| (window == needle).then_some(index))
            .collect::<Vec<_>>();
        assert_eq!(offsets.len(), 1, "hostile anchor {name} must be unique");
        hostile[offsets[0] + relative] = replacement;
        assert!(
            wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
                .validate_all(&hostile)
                .is_err(),
            "pinned upstream validator admitted rehashed {name} hostile"
        );
        let core = extract_first_core(&hostile);
        assert!(validate_private_result_component_v3(
            &hostile,
            artifact.source_revision(),
            Sha256::digest(core).into(),
        )
        .is_err());
    }
}

fn extract_first_core(candidate: &[u8]) -> &[u8] {
    let mut cursor = Cursor::new(candidate);
    cursor.take(8).unwrap();
    cursor.section(1).unwrap()
}

#[test]
fn node_executes_status_out_with_poison_preservation_and_exact_statuses() {
    let artifact = artifact();
    let validated = validate_private_result_component_v3(
        artifact.bytes(),
        artifact.source_revision(),
        artifact.generated_core_digest(),
    )
    .unwrap();
    let core = validated
        .generated_core()
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let script = format!(
        r#"const bytes = new Uint8Array([{core}]);
const instance = (await WebAssembly.instantiate(bytes, {{}})).instance;
const call = instance.exports.semaprax_evaluate_status_out;
const view = new DataView(instance.exports.memory.buffer);
const poison = 0x5a5a5a5a5a5a5a5an;
const cases = [
  [83n, 2n, 0, 42n],
  [(1n << 63n) - 1n, 1n, 0x02000001, poison],
  [(1n << 63n) - 1n, 0n, 0x02000001, poison],
  [1n, 0n, 0x02000004, poison],
  [1n, 7n, 0x01000001, poison],
  [17n, 2n, 0x01000002, poison],
];
for (const [left, right, status, expected] of cases) {{
  view.setBigInt64(128, poison, true);
  if (call(left, right, 128) !== status) process.exit(91);
  if (view.getBigInt64(128, true) !== expected) process.exit(92);
}}
const adapter = instance.exports.cabi_evaluate;
for (const [left, right, status, expected] of cases) {{
  if (adapter(left, right) !== 256) process.exit(93);
  const tag = view.getUint8(256);
  if (status === 0) {{
    if (tag !== 0 || view.getBigInt64(264, true) !== expected) process.exit(94);
  }} else {{
    const class_ = status >>> 24, code = status & 0xffffff;
    const pointer = view.getUint32(264, true), length = view.getUint32(268, true);
    const domain = new TextDecoder().decode(new Uint8Array(instance.exports.memory.buffer, pointer, length));
    const expectedDomain = class_ === 1 ? "semaprax.contract.v1" : "semaprax.arithmetic.v1";
    if (tag !== 1 || domain !== expectedDomain || view.getUint32(272, true) !== code ||
        view.getUint8(276) !== class_ || view.getUint8(277) !== 1 || view.getUint8(278) !== 0) process.exit(95);
  }}
}}
"#
    );
    let output = Command::new("node")
        .args(["--input-type=module", "--eval", &script])
        .output()
        .expect("Node is required by the established Wasm gate");
    assert!(
        output.status.success(),
        "Node result/status core failed with {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn node_specialized_arithmetic_paths_return_typed_status_without_trapping() {
    let cases = [
        (
            "negation",
            r#"module test.neg;
@id("component.evaluate") fn evaluate(left: i64, right: i64) -> i64 { -left }
@id("app.main") fn main() -> i64 { 0 }
"#,
            "[[42n,0n,0,-42n],[-(1n<<63n),0n,0x02000008,poison]]",
        ),
        (
            "multiplication",
            r#"module test.mul;
@id("component.evaluate") fn evaluate(left: i64, right: i64) -> i64 { left * right }
@id("app.main") fn main() -> i64 { 0 }
"#,
            "[[6n,7n,0,42n],[-(1n<<63n),-1n,0x02000003,poison],[-1n,-(1n<<63n),0x02000003,poison]]",
        ),
        (
            "subtraction",
            r#"module test.sub;
@id("component.evaluate") fn evaluate(left: i64, right: i64) -> i64 { left - right }
@id("app.main") fn main() -> i64 { 0 }
"#,
            "[[42n,2n,0,40n],[-(1n<<63n),1n,0x02000002,poison]]",
        ),
        (
            "division",
            r#"module test.div;
@id("component.evaluate") fn evaluate(left: i64, right: i64) -> i64 { left / right }
@id("app.main") fn main() -> i64 { 0 }
"#,
            "[[84n,2n,0,42n],[1n,0n,0x02000004,poison],[-(1n<<63n),-1n,0x02000005,poison]]",
        ),
        (
            "remainder",
            r#"module test.rem;
@id("component.evaluate") fn evaluate(left: i64, right: i64) -> i64 { left % right }
@id("app.main") fn main() -> i64 { 0 }
"#,
            "[[43n,7n,0,1n],[1n,0n,0x02000006,poison],[-(1n<<63n),-1n,0x02000007,poison]]",
        ),
    ];
    for (name, source, js_cases) in cases {
        let program =
            crate::parse(source, Path::new("component-result-arithmetic-v3.spx")).unwrap();
        let artifact = emit_private_result_component_v3(&program).unwrap();
        wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
            .validate_all(artifact.bytes())
            .unwrap();
        let core = validate_private_result_component_v3(
            artifact.bytes(),
            artifact.source_revision(),
            artifact.generated_core_digest(),
        )
        .unwrap()
        .generated_core()
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(",");
        let script = format!(
            r#"const instance=(await WebAssembly.instantiate(new Uint8Array([{core}]),{{}})).instance;
const view=new DataView(instance.exports.memory.buffer), poison=0x5a5a5a5a5a5a5a5an;
for (const [left,right,status,value] of {js_cases}) {{
  view.setBigInt64(128,poison,true);
  let actual;
  try {{ actual=instance.exports.semaprax_evaluate_status_out(left,right,128); }} catch (_) {{ process.exit(96); }}
  if (actual!==status || view.getBigInt64(128,true)!==value) process.exit(97);
}}
"#
        );
        let output = Command::new("node")
            .args(["--input-type=module", "--eval", &script])
            .output()
            .expect("Node is required by the established Wasm gate");
        assert!(
            output.status.success(),
            "Node {name} status/out regression failed with {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn excluded_profiles_fail_closed_without_changing_the_public_backend_gate() {
    for source in [
        r#"module bad; permit { clock.read } @id("component.evaluate") fn evaluate(left: i64, right: i64) -> i64 uses { clock.read } { left } @id("app.main") fn main() -> i64 { 0 }"#,
        r#"module bad; @id("component.evaluate") fn evaluate(left: i64) -> i64 { left } @id("app.main") fn main() -> i64 { 0 }"#,
        r#"module bad; @id("component.evaluate") fn evaluate(left: i64, right: i64) -> i64 { if true { left } else { right } } @id("app.main") fn main() -> i64 { 0 }"#,
        r#"module bad; @id("payload.type") record Payload { @id("payload.value") value: i64, } @id("component.evaluate") fn evaluate(left: i64, right: i64) -> i64 { left } @id("app.main") fn main() -> i64 { 0 }"#,
        r#"module bad; @id("choice.type") variant Choice { @id("choice.none") None, } @id("component.evaluate") fn evaluate(left: i64, right: i64) -> i64 { left } @id("app.main") fn main() -> i64 { 0 }"#,
    ] {
        let program = crate::parse(source, Path::new("excluded-result-v3.spx")).unwrap();
        let error = emit_private_result_component_v3(&program).unwrap_err();
        assert_eq!(error.code, "SPX-WIT107");
    }

    let resource = crate::parse(
        r#"module resource;
@id("token.type") resource Token { @id("token.drop") drop trivial; }
@id("component.evaluate") fn evaluate(left: i64, right: i64) -> i64 { left }
@id("app.main") fn main() -> i64 { 0 }
"#,
        Path::new("resource-result-v3.spx"),
    )
    .unwrap();
    assert_eq!(
        emit_private_result_component_v3(&resource)
            .unwrap_err()
            .code,
        "SPX-WIT107"
    );
    assert_eq!(
        crate::codegen::emit_c(&resource).unwrap_err().code,
        "SPX-B104"
    );
}
