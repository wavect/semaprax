//! Real public-library publication bounds; long identities are not passed
//! through a host command-line argument limit or synthetic module constructor.
use super::*;
use crate::wasm::internal_strings::InternalStringModule;

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn source(id: &str) -> String {
    assert!(!id.is_empty() && id.bytes().all(|byte| byte == b'a'));
    format!("module web.descriptor_bound;\n@id(\"{id}\") fn main() -> i64 {{ 42 }}\n")
}

fn compile(text: &str, id: &str) -> InternalStringModule {
    assert!(text.len() < SOURCE_LIMIT);
    let program = crate::check(text, "input.spx").unwrap();
    let resolved = crate::hir::resolve(&program).unwrap();
    crate::hir::validate(&resolved).unwrap();
    assert_eq!(resolved.functions.len(), 1);
    let body = &resolved.functions[0].body;
    let crate::hir::ResolvedExprKind::Block { statements, tail } = &body.kind else {
        panic!("fixture must retain its single function block");
    };
    assert!(statements.is_empty());
    assert!(matches!(tail.kind, crate::hir::ResolvedExprKind::Int(42)));
    // Exactly two expression nodes and no String cells or call frames: the
    // long identity, not semantic complexity, supplies descriptor bytes.
    emit_module(&program, &[id.to_owned()], InternalStringOptions::default()).unwrap()
}

fn descriptor(id: &str, wasm: &[u8]) -> String {
    // Independent literal wire oracle. Only the digest and decimal size of
    // actual Wasm bytes vary; zero stack/one owner are facts of this fixture.
    format!(
        "{{\"schema\":\"semaprax.wasm-internal-strings.v1\",\"runtime_schema\":\"semaprax.wasm-internal-strings.runtime.v1\",\"wasm_sha256\":\"{}\",\"wasm_bytes\":{},\"memory_pages\":4,\"result_offset\":65536,\"literal_offset\":196608,\"stack_bytes\":0,\"derived_owner_capacity\":1,\"limits\":{{\"max_string_bytes\":65536,\"max_live_bytes\":1048576,\"max_cumulative_bytes\":16777216,\"max_live_owners\":1}},\"exports\":[{{\"stable_id\":\"{id}\",\"wasm_export\":\"__spx_call_0\",\"parameters\":[],\"result\":\"i64\"}}]}}",
        hex(wasm), wasm.len()
    )
}

fn manifest(files: &BTreeMap<&str, Vec<u8>>, text: &str) -> String {
    let rows = INVENTORY
        .iter()
        .filter(|name| **name != "semaprax.manifest.json")
        .map(|name| {
            let bytes = &files[*name];
            format!(
                "{{\"path\":\"{name}\",\"bytes\":{},\"sha256\":\"sha256:{}\"}}",
                bytes.len(),
                hex(bytes)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let program = crate::check(text, "input.spx").unwrap();
    format!(
        "{{\"schema\":\"semaprax.web-internal-strings.v1\",\"module\":\"web.descriptor_bound\",\"source_digest\":\"sha256:{}\",\"graph_revision\":\"{}\",\"compiler_schema\":\"semaprax.wasm-internal-strings.v1\",\"runtime_schema\":\"semaprax.wasm-internal-strings.runtime.v1\",\"capabilities\":[],\"artifacts\":[{rows}]}}\n",
        hex(text.as_bytes()), crate::graph::revision(&program)
    )
}

#[test]
fn real_descriptor_exact_bound_publishes_and_plus_one_rejects_before_effects() {
    let baseline = compile(&source("a"), "a");
    assert_eq!(
        baseline.descriptor(),
        descriptor("a", baseline.wasm_bytes())
    );
    // Every ASCII identity byte occupies exactly one descriptor byte. This
    // uses a literal independent empty-ID envelope, not measured compiler
    // descriptor size or an adaptive search for a passing boundary.
    let overhead = descriptor("", baseline.wasm_bytes()).len();
    let identity_length = DESCRIPTOR_LIMIT.checked_sub(overhead).unwrap();
    assert!(identity_length > 0);
    let exact_id = "a".repeat(identity_length);
    let exact_source = source(&exact_id);
    let exact = compile(&exact_source, &exact_id);
    assert_eq!(exact.wasm_bytes(), baseline.wasm_bytes());
    assert_eq!(
        exact.descriptor(),
        descriptor(&exact_id, baseline.wasm_bytes())
    );
    assert_eq!(exact.descriptor().len(), DESCRIPTOR_LIMIT);

    let root = directory();
    let source_path = root.join("input.spx");
    let output = root.join("output");
    std::fs::write(&source_path, &exact_source).unwrap();
    build_web_from_source(&source_path, &output, &[exact_id]).unwrap();
    let files = reopened_package(&output);
    assert_eq!(files["app.wasm"], exact.wasm_bytes());
    assert_eq!(files["semaprax.js"], exact.runtime_source().as_bytes());
    assert_eq!(
        files["semaprax.internal-strings.json"],
        exact.descriptor().as_bytes()
    );
    assert_eq!(
        files["semaprax.manifest.json"],
        manifest(&files, &exact_source).as_bytes()
    );
    plain(&source_path, false);
    assert_eq!(
        std::fs::read(&source_path).unwrap(),
        exact_source.as_bytes()
    );

    let excess_id = "a".repeat(identity_length + 1);
    let excess_source = source(&excess_id);
    let excess = compile(&excess_source, &excess_id);
    assert_eq!(excess.wasm_bytes(), baseline.wasm_bytes());
    assert_eq!(
        excess.descriptor(),
        descriptor(&excess_id, baseline.wasm_bytes())
    );
    assert_eq!(excess.descriptor().len(), DESCRIPTOR_LIMIT + 1);
    let excess_path = root.join("excess.spx");
    let rejected = root.join("rejected");
    std::fs::write(&excess_path, &excess_source).unwrap();
    let errors = build_web_from_source(&excess_path, &rejected, &[excess_id]).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, "SPX-W111");
    assert_eq!(
        errors[0].message,
        "internal String Web descriptor exceeds 1048576 bytes"
    );
    assert_eq!(
        std::fs::symlink_metadata(&rejected).unwrap_err().kind(),
        std::io::ErrorKind::NotFound
    );

    // Validate all retained evidence before explicit nonrecursive cleanup.
    exact_entries(&root, &["input.spx", "excess.spx", "output"]);
    assert_eq!(reopened_package(&output), files);
    for (path, expected) in [
        (&source_path, &exact_source),
        (&excess_path, &excess_source),
    ] {
        plain(path, false);
        assert_eq!(std::fs::read(path).unwrap(), expected.as_bytes());
    }
    for name in INVENTORY {
        std::fs::remove_file(output.join(name)).unwrap();
    }
    std::fs::remove_dir(output).unwrap();
    std::fs::remove_file(source_path).unwrap();
    std::fs::remove_file(excess_path).unwrap();
    std::fs::remove_dir(root).unwrap();
}
