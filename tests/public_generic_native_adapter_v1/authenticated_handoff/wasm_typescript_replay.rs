//! The generated TypeScript consumer under Wasm adapter ABI v2 reports the
//! frozen `substituted_leaf_path` recipe with native's stable outcome. Only
//! the generated encoder's input leaf-path table is edited (as the native
//! recipe edits its caller's template); the compiled provider, its binding
//! and the rest of the package are the unmodified generated artifacts.
use super::*;
use semaprax::public_generic_consumer::{
    rust_calling::{OwnedByteField, RecordShape},
    typescript_calling::generate_typescript_calling_consumer,
};

const SCRIPT: &str = r#"import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { Provider, SemapraxPublicGenericException } from "../dist/index.js";
import { TRUSTED_WASM_ADAPTER_ABI_VERSION } from "../dist/descriptor.js";
const [left, right] = JSON.parse(readFileSync("test/names.json", "utf8"));
assert.equal(TRUSTED_WASM_ADAPTER_ABI_VERSION, "v2");
const provider = await Provider.open(readFileSync(process.argv[2]));
assert.throws(
  () => provider.transform({ [left]: Uint8Array.from([1, 7, 13]), [right]: Uint8Array.from([2, 11, 17, 23]) }),
  error => error instanceof SemapraxPublicGenericException
    && error.detail.kind === "carrier-rejected" && error.detail.reason === "carrier-replay",
);
// The refusal left no live handle: close settles to zero exactly once.
provider.close();
provider.close();
process.stdout.write("carrier-replay");
"#;

#[test]
fn generated_typescript_maps_wasm_v2_leaf_path_refusal_like_native() {
    let root = std::env::temp_dir().join(format!(
        "semaprax-r07-ts-replay-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let parsed = semaprax::check(SOURCE, Path::new("r07-ts-replay.spx")).unwrap();
    let revision = semaprax::format::canonical(&parsed);
    let program = semaprax::hir::resolve(&parsed).unwrap();
    let endpoint =
        derive_admitted_public_generic_endpoint_v1(&program, &revision, "auth.identity").unwrap();
    let wasm = semaprax::wasm::emit_public_generic_wasm_provider_v1(&program, &endpoint).unwrap();
    let shape = |paths: &[String]| {
        RecordShape::new(paths.iter().cloned().map(OwnedByteField::new).collect())
    };
    let input = shape(&endpoint.descriptor().input_facts().owned_leaves);
    let output = shape(&endpoint.descriptor().result_facts().owned_leaves);
    let consumer = generate_typescript_calling_consumer(
        wasm.descriptor_bytes(),
        wasm.binding(),
        &input,
        &output,
    )
    .unwrap();
    let package = root.join("package");
    for (name, contents) in consumer.files() {
        let path = package.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }
    // The native recipe flips the case of the path's final byte and remints
    // the frame; the generated encoder remints its own digest.
    let path = &input.fields[1].identity;
    let mut substituted = path.clone().into_bytes();
    *substituted.last_mut().unwrap() ^= 0x20;
    let substituted = String::from_utf8(substituted).unwrap();
    let carrier = package.join("src/carrier.ts");
    let source = fs::read_to_string(&carrier).unwrap();
    let table = source.find("INPUT_LEAF_PATHS = Object.freeze([").unwrap();
    let literal = format!("\"{path}\",");
    let at = table + source[table..].find(&literal).unwrap();
    let mut changed = source.clone();
    changed.replace_range(at..at + literal.len(), &format!("\"{substituted}\","));
    fs::write(&carrier, changed).unwrap();
    let names: Vec<_> = input
        .fields
        .iter()
        .map(|field| {
            format!(
                "field_{}",
                field
                    .identity
                    .bytes()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
            )
        })
        .collect();
    fs::write(
        package.join("test/names.json"),
        serde_json::to_vec(&names).unwrap(),
    )
    .unwrap();
    fs::write(package.join("test/replay.mjs"), SCRIPT).unwrap();
    fs::write(root.join("provider.wasm"), wasm.wasm()).unwrap();
    let tsc =
        super::typescript::locate_tsc().expect("generated consumer requires pinned TypeScript");
    let built = Command::new(&tsc)
        .current_dir(&package)
        .args(["-p", "tsconfig.json"])
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "generated tsc: {}",
        String::from_utf8_lossy(&built.stdout)
    );
    let executed = Command::new("node")
        .current_dir(&package)
        .args(["test/replay.mjs", "../provider.wasm"])
        .output()
        .expect("generated consumer requires Node");
    assert!(
        executed.status.success(),
        "generated Node: {}",
        String::from_utf8_lossy(&executed.stderr)
    );
    assert_eq!(executed.stdout, b"carrier-replay");
    eprintln!("R07 generated TypeScript ABI v2 substituted leaf path: carrier-rejected/carrier-replay, close settles");
    fs::remove_dir_all(root).unwrap();
}
