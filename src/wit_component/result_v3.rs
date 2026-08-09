//! Exact private WIT result/status Component Model v3 composition.

use sha2::{Digest, Sha256};

use crate::{ast::Program, diagnostic::Diagnostic, wasm};

use super::{
    push_counted_section, push_name, push_section, Cursor, PrivateComponentValidationError,
    COMPONENT_HEADER, WIT,
};

const INTERFACE_EXPORT: &str = "semaprax:private/evaluation@0.1.0";
const FUNCTION_EXPORT: &str = "evaluate";
const PROFILE: &[u8] = b"semaprax.private-result-component.v3\0canonical-abi-memory32-utf8\0status-word-class24-code24\0result-area-256\0";
const PROFILE_DIGEST_DOMAIN: &[u8] = b"semaprax.private-result-component-profile.v3\0";
const COMPONENT_DIGEST_DOMAIN: &[u8] = b"semaprax.private-result-component-artifact.v3\0";

/// Compiler-bound, import-free private Component Model artifact for the exact
/// `result<s64, status>` WIT projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrivateResultComponentArtifactV3 {
    bytes: Vec<u8>,
    digest: [u8; 32],
    generated_core_digest: [u8; 32],
    profile_digest: [u8; 32],
    source_revision: String,
}

impl PrivateResultComponentArtifactV3 {
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[must_use]
    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }

    #[must_use]
    pub const fn generated_core_digest(&self) -> [u8; 32] {
        self.generated_core_digest
    }

    #[must_use]
    pub const fn profile_digest(&self) -> [u8; 32] {
        self.profile_digest
    }

    #[must_use]
    pub fn source_revision(&self) -> &str {
        &self.source_revision
    }

    #[must_use]
    pub const fn wit(&self) -> &'static str {
        WIT
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedPrivateResultComponentV3<'a> {
    generated_core: &'a [u8],
    source_revision: &'a str,
}

impl<'a> ValidatedPrivateResultComponentV3<'a> {
    #[must_use]
    pub const fn generated_core(self) -> &'a [u8] {
        self.generated_core
    }

    #[must_use]
    pub const fn source_revision(self) -> &'a str {
        self.source_revision
    }

    #[must_use]
    pub const fn interface_export_name(self) -> &'static str {
        INTERFACE_EXPORT
    }

    #[must_use]
    pub const fn function_export_name(self) -> &'static str {
        FUNCTION_EXPORT
    }
}

pub fn emit_private_result_component_v3(
    program: &Program,
) -> Result<PrivateResultComponentArtifactV3, Diagnostic> {
    let core = wasm::emit_private_result_core_v3(program)?;
    let generated_core_digest: [u8; 32] = Sha256::digest(&core.bytes).into();
    let profile_digest = profile_digest();
    let bytes = compose(&core.bytes);
    let digest = artifact_digest(
        &core.source_revision,
        &generated_core_digest,
        &profile_digest,
        &bytes,
    );
    Ok(PrivateResultComponentArtifactV3 {
        bytes,
        digest,
        generated_core_digest,
        profile_digest,
        source_revision: core.source_revision,
    })
}

fn compose(core: &[u8]) -> Vec<u8> {
    let mut bytes = COMPONENT_HEADER.to_vec();
    push_section(&mut bytes, 1, core);
    push_counted_section(&mut bytes, 2, 1, &[0x00, 0x00, 0x00]);

    let mut aliases = vec![0x00, 0x00, 0x01, 0x00];
    push_name(&mut aliases, wasm::RESULT_COMPONENT_CANONICAL_EXPORT_V3);
    aliases.extend([0x00, 0x02, 0x01, 0x00]);
    push_name(&mut aliases, "memory");
    push_counted_section(&mut bytes, 6, 2, &aliases);

    push_section(&mut bytes, 7, &component_types());
    // canon lift core-func 0, UTF-8 plus core-memory 0, component-func type 3
    push_counted_section(
        &mut bytes,
        8,
        1,
        &[0x00, 0x00, 0x00, 0x02, 0x00, 0x03, 0x00, 0x03],
    );

    let mut interface = vec![0x01, 0x02]; // from-exports, two exports
    interface.push(0x00);
    push_name(&mut interface, "status");
    interface.extend([0x03, 0x01]); // component type 1
    interface.push(0x00); // component extern name discriminator
    push_name(&mut interface, FUNCTION_EXPORT);
    interface.extend([0x01, 0x00]); // component function 0
    push_counted_section(&mut bytes, 5, 1, &interface);

    let mut export = vec![0x00];
    push_name(&mut export, INTERFACE_EXPORT);
    export.extend([0x05, 0x00, 0x00]); // component instance 0, inferred exact type
    push_counted_section(&mut bytes, 11, 1, &export);
    bytes
}

fn component_types() -> Vec<u8> {
    let mut types = vec![0x04];
    // type 0: option<bool>
    types.extend([0x6b, 0x7f]);
    // type 1: record status
    types.extend([0x72, 0x04]);
    push_name(&mut types, "domain");
    types.push(0x73);
    push_name(&mut types, "code");
    types.push(0x79);
    push_name(&mut types, "class");
    types.push(0x7d);
    push_name(&mut types, "retryable");
    types.push(0x00); // type 0
                      // type 2: result<s64, status>
    types.extend([0x6a, 0x01, 0x78, 0x01, 0x01]);
    // type 3: evaluate(left: s64, right: s64) -> type 2
    types.extend([0x40, 0x02]);
    push_name(&mut types, "left");
    types.push(0x78);
    push_name(&mut types, "right");
    types.extend([0x78, 0x00, 0x02]);
    types
}

pub fn validate_private_result_component_v3<'a>(
    candidate: &'a [u8],
    expected_source_revision: &str,
    expected_generated_core_digest: [u8; 32],
) -> Result<ValidatedPrivateResultComponentV3<'a>, PrivateComponentValidationError> {
    let mut component = Cursor::new(candidate);
    if component.take(8)? != COMPONENT_HEADER {
        return Err(PrivateComponentValidationError::Header);
    }
    let core = component.section(1)?;
    if <[u8; 32]>::from(Sha256::digest(core)) != expected_generated_core_digest {
        return Err(PrivateComponentValidationError::CoreModule);
    }
    let source_revision = validate_core(core, expected_source_revision)?;
    super::validate_exact_counted_section(
        component.section(2)?,
        &[0x00, 0x00, 0x00],
        PrivateComponentValidationError::Profile,
    )?;

    let mut aliases = Cursor::new(component.section(6)?);
    aliases.expect_u32(2, PrivateComponentValidationError::Profile)?;
    aliases.expect_bytes(
        &[0x00, 0x00, 0x01, 0x00],
        PrivateComponentValidationError::Profile,
    )?;
    aliases.expect_name(
        wasm::RESULT_COMPONENT_CANONICAL_EXPORT_V3,
        PrivateComponentValidationError::Profile,
    )?;
    aliases.expect_bytes(
        &[0x00, 0x02, 0x01, 0x00],
        PrivateComponentValidationError::Profile,
    )?;
    aliases.expect_name("memory", PrivateComponentValidationError::Profile)?;
    aliases.finish(PrivateComponentValidationError::Profile)?;

    super::validate_exact_payload(
        component.section(7)?,
        &component_types(),
        PrivateComponentValidationError::Profile,
    )?;
    super::validate_exact_counted_section(
        component.section(8)?,
        &[0x00, 0x00, 0x00, 0x02, 0x00, 0x03, 0x00, 0x03],
        PrivateComponentValidationError::Profile,
    )?;

    let mut interface = Cursor::new(component.section(5)?);
    interface.expect_u32(1, PrivateComponentValidationError::Profile)?;
    interface.expect_bytes(&[0x01], PrivateComponentValidationError::Profile)?;
    interface.expect_u32(2, PrivateComponentValidationError::Profile)?;
    interface.expect_bytes(&[0x00], PrivateComponentValidationError::Profile)?;
    interface.expect_name("status", PrivateComponentValidationError::Profile)?;
    interface.expect_bytes(&[0x03, 0x01], PrivateComponentValidationError::Profile)?;
    interface.expect_bytes(&[0x00], PrivateComponentValidationError::Profile)?;
    interface.expect_name(FUNCTION_EXPORT, PrivateComponentValidationError::Profile)?;
    interface.expect_bytes(&[0x01, 0x00], PrivateComponentValidationError::Profile)?;
    interface.finish(PrivateComponentValidationError::Profile)?;

    let mut export = Cursor::new(component.section(11)?);
    export.expect_u32(1, PrivateComponentValidationError::Profile)?;
    export.expect_bytes(&[0x00], PrivateComponentValidationError::Profile)?;
    export.expect_name(INTERFACE_EXPORT, PrivateComponentValidationError::Profile)?;
    export.expect_bytes(
        &[0x05, 0x00, 0x00],
        PrivateComponentValidationError::Profile,
    )?;
    export.finish(PrivateComponentValidationError::Profile)?;
    component.finish(PrivateComponentValidationError::Profile)?;

    Ok(ValidatedPrivateResultComponentV3 {
        generated_core: core,
        source_revision,
    })
}

fn validate_core<'a>(
    core: &'a [u8],
    expected_source_revision: &str,
) -> Result<&'a str, PrivateComponentValidationError> {
    let mut module = Cursor::new(core);
    if module.take(8)? != b"\0asm\x01\0\0\0" {
        return Err(PrivateComponentValidationError::CoreModule);
    }
    super::validate_exact_payload(
        module.section(1)?,
        &[
            0x02, 0x60, 0x03, 0x7e, 0x7e, 0x7f, 0x01, 0x7f, 0x60, 0x02, 0x7e, 0x7e, 0x01, 0x7f,
        ],
        PrivateComponentValidationError::CoreModule,
    )?;
    super::validate_exact_payload(
        module.section(3)?,
        &[0x02, 0x00, 0x01],
        PrivateComponentValidationError::CoreModule,
    )?;
    super::validate_exact_payload(
        module.section(5)?,
        &[0x01, 0x00, 0x01],
        PrivateComponentValidationError::CoreModule,
    )?;
    let mut exports = Cursor::new(module.section(7)?);
    exports.expect_u32(3, PrivateComponentValidationError::CoreModule)?;
    exports.expect_name("memory", PrivateComponentValidationError::CoreModule)?;
    exports.expect_bytes(&[0x02, 0x00], PrivateComponentValidationError::CoreModule)?;
    exports.expect_name(
        wasm::RESULT_COMPONENT_STATUS_OUT_EXPORT_V3,
        PrivateComponentValidationError::CoreModule,
    )?;
    exports.expect_bytes(&[0x00, 0x00], PrivateComponentValidationError::CoreModule)?;
    exports.expect_name(
        wasm::RESULT_COMPONENT_CANONICAL_EXPORT_V3,
        PrivateComponentValidationError::CoreModule,
    )?;
    exports.expect_bytes(&[0x00, 0x01], PrivateComponentValidationError::CoreModule)?;
    exports.finish(PrivateComponentValidationError::CoreModule)?;
    if module.section(10)?.is_empty() || module.section(11)?.is_empty() {
        return Err(PrivateComponentValidationError::CoreModule);
    }
    let mut custom = Cursor::new(module.section(0)?);
    custom.expect_name(
        "semaprax.component-result-v3",
        PrivateComponentValidationError::CoreModule,
    )?;
    let revision_length =
        usize::try_from(custom.u32()?).map_err(|_| PrivateComponentValidationError::Encoding)?;
    let revision_bytes = custom.take(revision_length)?;
    let revision = std::str::from_utf8(revision_bytes)
        .map_err(|_| PrivateComponentValidationError::CoreModule)?;
    if revision != expected_source_revision {
        return Err(PrivateComponentValidationError::CoreModule);
    }
    custom.finish(PrivateComponentValidationError::CoreModule)?;
    module.finish(PrivateComponentValidationError::CoreModule)?;
    Ok(revision)
}

fn profile_digest() -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(PROFILE_DIGEST_DOMAIN);
    hash.update((WIT.len() as u64).to_le_bytes());
    hash.update(WIT.as_bytes());
    hash.update((PROFILE.len() as u64).to_le_bytes());
    hash.update(PROFILE);
    hash.finalize().into()
}

fn artifact_digest(
    source_revision: &str,
    generated_core_digest: &[u8; 32],
    profile_digest: &[u8; 32],
    bytes: &[u8],
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(COMPONENT_DIGEST_DOMAIN);
    hash.update((source_revision.len() as u64).to_le_bytes());
    hash.update(source_revision.as_bytes());
    hash.update(generated_core_digest);
    hash.update(profile_digest);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}

#[cfg(test)]
mod tests {
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
            "sha256:1252335610de30df759f6e9ac038217853b60d381a0703f4f08518cb20b30cd8",
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
                87, 58, 161, 216, 27, 99, 78, 246, 233, 126, 84, 6, 47, 151, 216, 116, 126, 83,
                153, 79, 217, 248, 64, 54, 227, 208, 141, 103, 231, 171, 226, 186,
            ],
            "generated-core KAT changed"
        );
        assert_eq!(
            first.profile_digest(),
            [
                222, 215, 48, 247, 69, 152, 10, 90, 86, 167, 93, 149, 152, 80, 26, 184, 41, 24, 28,
                36, 66, 136, 84, 206, 88, 224, 108, 189, 68, 18, 50, 98,
            ],
            "profile KAT changed"
        );
        assert_eq!(
            first.digest(),
            [
                50, 186, 5, 139, 50, 203, 180, 108, 209, 87, 28, 38, 121, 87, 59, 85, 213, 41, 161,
                85, 125, 176, 135, 0, 215, 82, 104, 130, 251, 39, 250, 13,
            ],
            "component DAG KAT changed"
        );
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(first.bytes())),
            [
                76, 193, 105, 178, 43, 191, 66, 248, 252, 44, 187, 138, 55, 151, 11, 17, 103, 107,
                195, 125, 215, 227, 26, 159, 128, 213, 167, 220, 118, 41, 127, 177,
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
}
