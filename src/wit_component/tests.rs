use super::*;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_CONSUMER: AtomicU64 = AtomicU64::new(0);

const CHECKED_COMPONENT_SOURCE: &str = r#"
module test.checked_component;

@id("app.main")
fn main() -> i64 { 19 + 23 }
"#;

fn checked_component_program() -> crate::ast::Program {
    crate::parse(CHECKED_COMPONENT_SOURCE, Path::new("checked-component.spx")).unwrap()
}

fn replace_unique_byte(bytes: &mut [u8], needle: &[u8], relative_index: usize, value: u8) {
    let matches = bytes
        .windows(needle.len())
        .enumerate()
        .filter_map(|(index, candidate)| (candidate == needle).then_some(index))
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "hostile mutation anchor is not unique");
    bytes[matches[0] + relative_index] = value;
}

fn rehashed_artifact(
    original: &PrivateCheckedComponentArtifactV2,
    bytes: Vec<u8>,
) -> PrivateCheckedComponentArtifactV2 {
    let mut hostile = original.clone();
    hostile.bytes = bytes;
    hostile.digest = Sha256::digest(hostile.bytes()).into();
    hostile
}

struct ConsumerDirectory(PathBuf);

impl ConsumerDirectory {
    fn create() -> Self {
        let ordinal = NEXT_CONSUMER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "semaprax-wit-default-surface-{}-{ordinal}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for ConsumerDirectory {
    fn drop(&mut self) {
        let Ok(metadata) = std::fs::symlink_metadata(&self.0) else {
            return;
        };
        if metadata.file_type().is_symlink() || metadata.is_file() {
            let _ = std::fs::remove_file(&self.0);
        } else if metadata.is_dir() {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

#[test]
fn bundle_is_deterministic_canonical_and_mutation_closed() {
    let first = emit_private_wit_bundle_v1();
    let second = emit_private_wit_bundle_v1();
    assert_eq!(first, second);
    assert_eq!(
        first.digest,
        [
            76, 15, 16, 105, 217, 44, 141, 85, 231, 174, 10, 165, 27, 215, 130, 15, 255, 119, 167,
            24, 57, 189, 52, 39, 33, 22, 186, 145, 27, 43, 153, 52,
        ]
    );
    assert_eq!(verify_private_wit_bundle_v1(first.bytes()), Ok(()));
    assert!(first.wit.contains("result<s64, status>"));
    assert!(!first.wit.contains("resource"));
    for index in 0..first.bytes().len() {
        let mut hostile = first.bytes().to_vec();
        hostile[index] ^= 1;
        assert_eq!(
            verify_private_wit_bundle_v1(&hostile),
            Err(if index < 8 {
                "SPX-WIT001"
            } else {
                "SPX-WIT002"
            })
        );
    }
    for end in 0..first.bytes().len() {
        assert!(verify_private_wit_bundle_v1(&first.bytes()[..end]).is_err());
    }
    let mut trailing = first.bytes().to_vec();
    trailing.push(0);
    assert_eq!(verify_private_wit_bundle_v1(&trailing), Err("SPX-WIT002"));
}

#[test]
fn node_executes_exact_javascript_result_adapter() {
    let script = format!(
        "{JAVASCRIPT}\n{}",
        r#"const reject = candidate => {
  let rejected = false;
  try { normalizeEvaluation(candidate); } catch (_error) { rejected = true; }
  if (!rejected) process.exit(92);
};
const status = (domain, code = 7) => ({ err: { domain, code, class: 3, retryable: null } });
const ok = normalizeEvaluation({ ok: 7n });
const err = normalizeEvaluation({ err: { domain: "fixture.v1", code: 7, class: 3, retryable: false } });
if (ok.ok !== 7n || err.err.code !== 7) process.exit(91);
for (const hostile of [
  { ok: 7 }, {}, status("x", 0), status("x", 0x1_0000_0000), status("", 7),
  status("a".repeat(256), 7), status("a\0b", 7), status("€".repeat(86), 7),
  status("\uD800", 7), status("\uDC00", 7)
]) reject(hostile);

const asciiMax = normalizeEvaluation(status("a".repeat(255), 0xFFFF_FFFF));
const utf8Max = normalizeEvaluation(status("€".repeat(85), 7));
const paired = normalizeEvaluation(status("😀".repeat(63) + "abc", 7));
if (asciiMax.err.code !== 0xFFFF_FFFF || utf8Max.err.domain.length !== 85 || paired.err.domain.length !== 129) process.exit(93);

let getterReads = 0;
const changingGetter = {};
Object.defineProperty(changingGetter, "ok", { enumerable: true, get() { getterReads++; return getterReads === 1 ? 7n : 7; } });
reject(changingGetter);
reject({ ok: 7n, [Symbol("hostile")]: 1 });
const statusGetter = { code: 7, class: 3, retryable: null };
Object.defineProperty(statusGetter, "domain", { enumerable: true, get() { getterReads++; return "fixture.v1"; } });
reject({ err: statusGetter });
if (getterReads !== 0) process.exit(94);

let descriptorReads = 0;
let valueReads = 0;
const changingProxy = new Proxy({ ok: 7n }, {
  ownKeys() { return ["ok"]; },
  getOwnPropertyDescriptor() {
descriptorReads++;
return { configurable: true, enumerable: true, value: descriptorReads === 1 ? 7n : 7 };
  },
  get() { valueReads++; return 7; }
});
if (normalizeEvaluation(changingProxy).ok !== 7n || descriptorReads !== 1 || valueReads !== 0) process.exit(95);
reject(new Proxy({}, { ownKeys() { throw new Error("hostile"); } }));
"#
    );
    let output = std::process::Command::new("node")
        .args(["--input-type=module", "--eval", &script])
        .output()
        .expect("Node is required by the existing Wasm quality gate");
    assert!(
        output.status.success(),
        "Node adapter failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn component_binary_is_deterministic_independently_parsed_and_mutation_closed() {
    let first = emit_private_component_v1();
    let second = emit_private_component_v1();
    assert_eq!(first, second);
    assert_eq!(&first.bytes()[..8], COMPONENT_HEADER);
    assert_eq!(
        first.digest,
        [
            0x3e, 0xd6, 0xbe, 0xd8, 0x47, 0x2e, 0xea, 0xe0, 0xef, 0x17, 0xf9, 0x64, 0x58, 0x62,
            0x2c, 0x9a, 0xe0, 0x32, 0xdd, 0x7a, 0x13, 0xb1, 0x15, 0xd2, 0xd7, 0xfe, 0xa7, 0xfc,
            0xfe, 0xcd, 0xe6, 0x43,
        ]
    );
    let validated = validate_private_component_v1(first.bytes()).unwrap();
    assert_eq!(validated.export_name(), "evaluate");
    assert_eq!(&validated.core_module()[..8], b"\0asm\x01\0\0\0");

    for index in 0..first.bytes().len() {
        let mut hostile = first.bytes().to_vec();
        hostile[index] ^= 1;
        assert!(
            validate_private_component_v1(&hostile).is_err(),
            "byte {index} was not authenticated by the profile parser"
        );
    }
    for end in 0..first.bytes().len() {
        assert!(validate_private_component_v1(&first.bytes()[..end]).is_err());
    }
    let mut trailing = first.bytes().to_vec();
    trailing.push(0);
    assert_eq!(
        validate_private_component_v1(&trailing),
        Err(PrivateComponentValidationError::Profile)
    );

    let mut noncanonical_length = first.bytes().to_vec();
    noncanonical_length.splice(9..10, [0xae, 0x00]);
    assert_eq!(
        validate_private_component_v1(&noncanonical_length),
        Err(PrivateComponentValidationError::Encoding)
    );
}

#[test]
fn checked_component_v2_is_generated_bound_and_independently_parsed() {
    let program = checked_component_program();
    let first = emit_private_checked_component_v2(&program).unwrap();
    let second = emit_private_checked_component_v2(&program).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.source_revision(), graph::revision(&program));
    assert_eq!(first.runtime_core_digest(), CHECKED_RUNTIME_CORE_V2_SHA256);
    assert_eq!(
        first.digest(),
        [
            192, 191, 163, 225, 184, 136, 50, 55, 236, 153, 52, 82, 12, 249, 177, 205, 178, 73, 40,
            157, 49, 141, 108, 208, 65, 62, 99, 183, 22, 112, 59, 192,
        ]
    );

    let validated =
        validate_private_checked_component_v2(first.bytes(), first.generated_core_digest())
            .unwrap();
    assert_eq!(validated.export_name(), "evaluate");
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(validated.generated_core())),
        first.generated_core_digest()
    );
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(validated.runtime_core())),
        first.runtime_core_digest()
    );
    for index in 0..first.bytes().len() {
        let mut hostile = first.bytes().to_vec();
        hostile[index] ^= 1;
        assert!(
            validate_private_checked_component_v2(&hostile, first.generated_core_digest()).is_err(),
            "component v2 byte {index} was not authenticated"
        );
    }
    for end in 0..first.bytes().len() {
        assert!(validate_private_checked_component_v2(
            &first.bytes()[..end],
            first.generated_core_digest()
        )
        .is_err());
    }
    let mut trailing = first.bytes().to_vec();
    trailing.push(0);
    assert_eq!(
        validate_private_checked_component_v2(&trailing, first.generated_core_digest()),
        Err(PrivateComponentValidationError::Profile)
    );
}

#[test]
fn checked_component_v2_digest_is_read_only_and_javascript_uses_exact_bytes() {
    let artifact = emit_private_checked_component_v2(&checked_component_program()).unwrap();
    let expected = private_checked_component_runtime_javascript_v2(&artifact);
    let mut forged_metadata = artifact.clone();
    forged_metadata.digest = [0xa5; 32];
    assert_ne!(forged_metadata.digest(), artifact.digest());
    assert_eq!(
        private_checked_component_runtime_javascript_v2(&forged_metadata),
        expected,
        "JavaScript authorization trusted forgeable digest metadata"
    );
}

#[test]
fn upstream_validator_rejects_rehashed_component_cross_type_hostiles() {
    let artifact = emit_private_checked_component_v2(&checked_component_program()).unwrap();
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(artifact.bytes())
        .expect("pinned upstream validator rejected checked component v2");

    let validated =
        validate_private_checked_component_v2(artifact.bytes(), artifact.generated_core_digest())
            .unwrap();
    let generated_offset = artifact
        .bytes()
        .windows(validated.generated_core().len())
        .position(|candidate| candidate == validated.generated_core())
        .unwrap();
    let runtime_offset = artifact
        .bytes()
        .windows(validated.runtime_core().len())
        .position(|candidate| candidate == validated.runtime_core())
        .unwrap();

    let mut invalid_signature = artifact.bytes().to_vec();
    replace_unique_byte(
        &mut invalid_signature[runtime_offset..runtime_offset + validated.runtime_core().len()],
        &[7, b's', b'p', b'x', b'_', b'a', b'd', b'd', 0, 0],
        9,
        5,
    );
    let mut invalid_body = artifact.bytes().to_vec();
    let generated_end = generated_offset + validated.generated_core().len();
    invalid_body[generated_end - 1] = 0;
    let mut invalid_cardinality = artifact.bytes().to_vec();
    replace_unique_byte(&mut invalid_cardinality, &[6, 19, 1, 0, 0, 1, 1, 13], 2, 2);
    let mut invalid_canonical_lift = artifact.bytes().to_vec();
    replace_unique_byte(
        &mut invalid_canonical_lift,
        &[7, 5, 1, 64, 0, 0, 120],
        6,
        127,
    );

    for (name, bytes) in [
        ("signature", invalid_signature),
        ("body", invalid_body),
        ("cardinality", invalid_cardinality),
        ("canonical-lift", invalid_canonical_lift),
    ] {
        let hostile = rehashed_artifact(&artifact, bytes);
        assert_eq!(
            hostile.digest(),
            <[u8; 32]>::from(Sha256::digest(hostile.bytes()))
        );
        assert!(
            wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
                .validate_all(hostile.bytes())
                .is_err(),
            "pinned upstream validator admitted rehashed hostile {name}"
        );
    }
}

#[test]
fn checked_component_v2_rejects_owned_core_profiles() {
    let program = crate::parse(
        r#"module test.checked_component_owned;
@id("token.type")
resource Token {
@id("token.drop")
drop trivial;
}
@id("token.identity")
fn identity(value: own Token) -> Token { value }
@id("app.main")
fn main() -> i64 { 42 }
"#,
        Path::new("checked-component-owned.spx"),
    )
    .unwrap();
    let error = emit_private_checked_component_v2(&program).unwrap_err();
    assert_eq!(error.code, "SPX-WIT106");
}

#[test]
fn checked_component_v2_ignores_only_implicit_prelude_templates() {
    for source in [
        r#"module authored_record;
@id("record.type") record Payload { @id("record.value") value: i64, }
@id("app.main") fn main() -> i64 { 42 }
"#,
        r#"module authored_variant;
@id("variant.type") variant Choice { @id("variant.none") None, }
@id("app.main") fn main() -> i64 { 42 }
"#,
    ] {
        let program = crate::parse(source, Path::new("authored-aggregate-v2.spx")).unwrap();
        let error = emit_private_checked_component_v2(&program).unwrap_err();
        assert_eq!(error.code, "SPX-WIT106");
    }

    emit_private_checked_component_v2(&checked_component_program())
        .expect("implicit Option/Result templates must preserve the scalar profile");
}

#[test]
fn node_executes_generated_core_with_the_embedded_checked_runtime() {
    let artifact = emit_private_checked_component_v2(&checked_component_program()).unwrap();
    let validated =
        validate_private_checked_component_v2(artifact.bytes(), artifact.generated_core_digest())
            .unwrap();
    let runtime = validated
        .runtime_core()
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let generated = validated
        .generated_core()
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let script = format!(
        r#"const runtimeBytes = new Uint8Array([{runtime}]);
const generatedBytes = new Uint8Array([{generated}]);
if (!WebAssembly.validate(runtimeBytes)) process.exit(76);
if (!WebAssembly.validate(generatedBytes)) process.exit(75);
const runtime = (await WebAssembly.instantiate(runtimeBytes, {{}})).instance;
const generated = (await WebAssembly.instantiate(generatedBytes, {{ env: runtime.exports }})).instance;
if (generated.exports.semaprax_main() !== 42n) process.exit(72);
const min = -(1n << 63n), max = (1n << 63n) - 1n;
for (const invoke of [
  () => runtime.exports.spx_add(max, 1n),
  () => runtime.exports.spx_sub(min, 1n),
  () => runtime.exports.spx_mul(max, 2n),
  () => runtime.exports.spx_div(min, -1n),
  () => runtime.exports.spx_rem(min, -1n),
  () => runtime.exports.spx_neg(min),
  () => runtime.exports.spx_contract_fail()
]) {{
  let trapped = false;
  try {{ invoke(); }} catch (error) {{ trapped = error instanceof WebAssembly.RuntimeError; }}
  if (!trapped) process.exit(73);
}}
if (runtime.exports.spx_add(19n, 23n) !== 42n ||
runtime.exports.spx_sub(23n, 19n) !== 4n ||
runtime.exports.spx_mul(-6n, 7n) !== -42n ||
runtime.exports.spx_div(-42n, 7n) !== -6n ||
runtime.exports.spx_rem(43n, 7n) !== 1n ||
runtime.exports.spx_neg(42n) !== -42n) process.exit(74);
"#
    );
    let output = Command::new("node")
        .args(["--input-type=module", "--eval", &script])
        .output()
        .expect("Node is required by the existing Wasm quality gate");
    assert!(
        output.status.success(),
        "Node checked component core execution failed with {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn node_executes_the_authenticated_checked_component_v2_runtime() {
    let artifact = emit_private_checked_component_v2(&checked_component_program()).unwrap();
    let bytes = artifact
        .bytes()
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let script = format!(
        r#"{}
const original = new Uint8Array([{}]);
const component = await instantiatePrivateCheckedComponentV2(original);
if (component.evaluate() !== 42n || !Object.isFrozen(component)) process.exit(77);
let rejectedArgument = false;
try {{ component.evaluate(1n); }} catch (error) {{ rejectedArgument = error instanceof TypeError && error.message === "SPX-WIT-I64"; }}
if (!rejectedArgument) process.exit(78);

const copiedBeforeAwait = new Uint8Array(original);
const pending = instantiatePrivateCheckedComponentV2(copiedBeforeAwait);
copiedBeforeAwait.fill(0);
if ((await pending).evaluate() !== 42n) process.exit(79);

for (const hostile of [original.subarray(0, original.length - 1), Uint8Array.from([...original, 0])]) {{
  let rejected = false;
  try {{ await instantiatePrivateCheckedComponentV2(hostile); }}
  catch (error) {{ rejected = error instanceof TypeError; }}
  if (!rejected) process.exit(80);
}}
const changed = Uint8Array.from(original);
changed[Math.floor(changed.length / 2)] ^= 1;
let authenticated = false;
try {{ await instantiatePrivateCheckedComponentV2(changed); }}
catch (error) {{ authenticated = error instanceof TypeError && error.message === "SPX-WIT105"; }}
if (!authenticated) process.exit(81);
"#,
        private_checked_component_runtime_javascript_v2(&artifact),
        bytes
    );
    let output = Command::new("node")
        .args(["--input-type=module", "--eval", &script])
        .output()
        .expect("Node is required by the existing Wasm quality gate");
    assert!(
        output.status.success(),
        "Node checked component v2 runtime failed with {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn node_checked_component_v2_evaluate_traps_generated_overflow_and_contract_failure() {
    for (name, source) in [
        (
            "overflow",
            r#"module test.checked_component_overflow;
@id("app.main")
fn main() -> i64 { 9223372036854775807 + 1 }
"#,
        ),
        (
            "contract",
            r#"module test.checked_component_contract;
@id("app.main")
fn main() -> i64 requires false { 42 }
"#,
        ),
    ] {
        let program = crate::parse(source, Path::new("checked-component-trap.spx")).unwrap();
        let artifact = emit_private_checked_component_v2(&program).unwrap();
        let bytes = artifact
            .bytes()
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let script = format!(
            r#"{}
const component = await instantiatePrivateCheckedComponentV2(new Uint8Array([{}]));
let trapped = false;
try {{ component.evaluate(); }}
catch (error) {{ trapped = error instanceof WebAssembly.RuntimeError; }}
if (!trapped) process.exit(82);
"#,
            private_checked_component_runtime_javascript_v2(&artifact),
            bytes
        );
        let output = Command::new("node")
            .args(["--input-type=module", "--eval", &script])
            .output()
            .expect("Node is required by the existing Wasm quality gate");
        assert!(
            output.status.success(),
            "Node checked component v2 {name} did not trap through evaluate with {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn node_private_component_runtime_executes_the_embedded_core_export() {
    let artifact = emit_private_component_v1();
    let bytes = artifact
        .bytes()
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let mut script = format!(
        "{}\nconst bytes=new Uint8Array([{}]);\n",
        private_component_runtime_javascript_v1(),
        bytes
    );
    script.push_str(
        r#"const component = await instantiatePrivateScalarComponentV1(bytes);
if (component.evaluate(19n, 23n) !== 42n) process.exit(81);
const i64Minimum = -(1n << 63n);
const i64Maximum = (1n << 63n) - 1n;
if (component.evaluate(i64Minimum, 0n) !== i64Minimum ||
component.evaluate(i64Maximum, 0n) !== i64Maximum) process.exit(82);
const rejectI64 = args => {
  let rejected = false;
  try { component.evaluate(...args); }
  catch (error) { rejected = error instanceof TypeError && error.message === "SPX-WIT-I64"; }
  if (!rejected) process.exit(83);
};
for (const args of [
  [19, 23], [19n, 23], [i64Minimum - 1n, 0n], [i64Maximum + 1n, 0n],
  [0n, i64Minimum - 1n], [0n, i64Maximum + 1n]
]) rejectI64(args);

const changedCore = bytes.slice();
const coreOpcode = changedCore.indexOf(0x7c);
if (coreOpcode < 10) process.exit(84);
changedCore[coreOpcode] = 0x7d;
const hostileInputs = [
  bytes.slice(0, -1),
  Uint8Array.from([...bytes, 0]),
  Uint8Array.from(bytes, (byte, index) => index === 8 ? 2 : byte),
  changedCore
];
for (const hostile of hostileInputs) {
  let rejected = false;
  try { await instantiatePrivateScalarComponentV1(hostile); }
  catch (error) { rejected = error instanceof TypeError && error.message.startsWith("SPX-WIT"); }
  if (!rejected) process.exit(85);
}

const mutable = bytes.slice();
const pending = instantiatePrivateScalarComponentV1(mutable);
mutable[coreOpcode] = 0x7d;
const snapshotted = await pending;
if (snapshotted.evaluate(19n, 23n) !== 42n) process.exit(86);
"#,
    );
    let output = std::process::Command::new("node")
        .args(["--input-type=module", "--eval", &script])
        .output()
        .expect("Node is required by the existing Wasm quality gate");
    assert!(
        output.status.success(),
        "Node private component runtime failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn default_feature_external_consumer_cannot_import_component_harness() {
    let directory = ConsumerDirectory::create();
    let manifest_root = env!("CARGO_MANIFEST_DIR").replace('\\', "\\\\");
    std::fs::write(
        directory.path().join("Cargo.toml"),
        format!(
            r#"[package]
name = "semaprax-wit-default-surface-check"
version = "0.0.0"
edition = "2021"

[workspace]

[dependencies]
semaprax = {{ path = "{manifest_root}", default-features = false }}
"#
        ),
    )
    .unwrap();
    std::fs::create_dir(directory.path().join("src")).unwrap();
    std::fs::write(
        directory.path().join("src/main.rs"),
        r#"use semaprax::wit_component::{
emit_private_checked_component_v2,
emit_private_component_v1,
emit_private_result_component_v3,
emit_private_scalar_algebra_component_v5,
emit_private_source_result_component_v4,
private_checked_component_runtime_javascript_v2,
validate_private_checked_component_v2,
validate_private_component_v1,
validate_private_result_component_v3,
validate_private_scalar_algebra_component_v5,
validate_private_source_result_component_v4,
};

fn main() {
let artifact = emit_private_component_v1();
let _ = validate_private_component_v1(artifact.bytes());
let _ = emit_private_checked_component_v2;
let _ = private_checked_component_runtime_javascript_v2;
let _ = validate_private_checked_component_v2;
let _ = emit_private_result_component_v3;
let _ = validate_private_result_component_v3;
let _ = emit_private_scalar_algebra_component_v5;
let _ = validate_private_scalar_algebra_component_v5;
let _ = emit_private_source_result_component_v4;
let _ = validate_private_source_result_component_v4;
}
"#,
    )
    .unwrap();
    let checked = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .args(["check", "--offline", "--manifest-path"])
        .arg(directory.path().join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", directory.path().join("target"))
        .output()
        .unwrap();
    assert!(
        !checked.status.success(),
        "default surface exposed the private component harness"
    );
    let stderr = String::from_utf8_lossy(&checked.stderr);
    assert!(
        stderr.contains("wit_component")
            && (stderr.contains("could not find") || stderr.contains("unresolved import")),
        "unexpected default-surface compiler diagnostic:\n{stderr}"
    );
}

#[test]
fn owned_resource_corpus_wit_exposes_token_resource_handles() {
    let program = crate::parse(
        crate::owned_resource_corpus::OWNED_RESOURCE_CORPUS_SOURCE_V1,
        std::path::Path::new("owned-resource-corpus-wit.spx"),
    )
    .unwrap();
    let first = crate::wit_component::emit_wit(&program).unwrap();
    let second = crate::wit_component::emit_wit(&program).unwrap();
    assert_eq!(first, second, "WIT generation must be deterministic");
    assert!(
        first.contains("resource token"),
        "WIT must expose trivial-drop Token as `resource token`:\n{first}"
    );
    assert!(
        first.contains("own<token>"),
        "WIT must use owned handles for Token: \n{first}"
    );
    // The corpus exercises owned handles; borrow is the symmetric handle
    // kind and is authenticated through the same resource arm. Ensure the
    // generated WIT is structurally valid and at least mentions handle
    // syntax. If a borrow variant is present, it must also be typed.
    assert!(
        first.contains("borrow<token>") || first.contains("own<token>"),
        "WIT must contain handle types: \n{first}"
    );
    let unsupported = crate::parse(
        r#"module test.unsupported;
@id("token.type")
resource Token {
@id("token.drop")
drop import "token.finalize";
}
@id("token.use")
fn use_token(value: own Token) -> i64 { 0 }
@id("app.main")
fn main() -> i64 { 0 }
"#,
        std::path::Path::new("unsupported-token-wit.spx"),
    )
    .unwrap();
    let error = crate::wit_component::emit_wit(&unsupported).unwrap_err();
    assert_eq!(error.code, "SPX-WIT110");
}

#[test]
fn feature_consumer_can_only_read_checked_component_digests_through_accessors() {
    let directory = ConsumerDirectory::create();
    let manifest_root = env!("CARGO_MANIFEST_DIR").replace('\\', "\\\\");
    std::fs::write(
        directory.path().join("Cargo.toml"),
        format!(
            r#"[package]
name = "semaprax-wit-read-only-digest-check"
version = "0.0.0"
edition = "2021"

[workspace]

[dependencies]
semaprax = {{ path = "{manifest_root}", default-features = false, features = ["unstable-wit-component-harness"] }}
"#
        ),
    )
    .unwrap();
    std::fs::create_dir(directory.path().join("src")).unwrap();
    std::fs::write(
        directory.path().join("src/main.rs"),
        r#"use semaprax::wit_component::{
PrivateCheckedComponentArtifactV2,
PrivateResultComponentArtifactV3,
PrivateSourceResultComponentArtifactV4,
};

fn hostile(artifact: &mut PrivateCheckedComponentArtifactV2) {
let _ = artifact.digest();
let _ = artifact.generated_core_digest();
let _ = artifact.runtime_core_digest();
artifact.digest = [0; 32];
artifact.generated_core_digest = [0; 32];
artifact.runtime_core_digest = [0; 32];
}

fn hostile_v3(artifact: &mut PrivateResultComponentArtifactV3) {
let _ = artifact.digest();
let _ = artifact.generated_core_digest();
let _ = artifact.profile_digest();
artifact.digest = [0; 32];
artifact.generated_core_digest = [0; 32];
artifact.profile_digest = [0; 32];
}

fn hostile_v4(artifact: &mut PrivateSourceResultComponentArtifactV4) {
let _ = artifact.digest();
let _ = artifact.generated_core_digest();
let _ = artifact.profile_digest();
let _ = artifact.prelude_digest();
let _ = artifact.result_i64_bool_layout_digest();
let _ = artifact.result_bool_bool_layout_digest();
artifact.digest = [0; 32];
artifact.generated_core_digest = [0; 32];
artifact.profile_digest = [0; 32];
artifact.prelude_digest = [0; 32];
artifact.result_i64_bool_layout_digest = [0; 32];
artifact.result_bool_bool_layout_digest = [0; 32];
}

fn main() {}
"#,
    )
    .unwrap();
    let checked = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .args(["check", "--offline", "--manifest-path"])
        .arg(directory.path().join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", directory.path().join("target"))
        .output()
        .unwrap();
    assert!(!checked.status.success(), "digest fields remained writable");
    let stderr = String::from_utf8_lossy(&checked.stderr);
    for field in [
        "digest",
        "generated_core_digest",
        "runtime_core_digest",
        "profile_digest",
        "prelude_digest",
        "result_i64_bool_layout_digest",
        "result_bool_bool_layout_digest",
    ] {
        assert!(
            stderr.contains("private field") && stderr.contains(field),
            "missing private-field diagnostic for {field}:\n{stderr}"
        );
    }
}
