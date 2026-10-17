//! Same admitted nested byte program on every existing byte-data engine.
use super::*;
const NESTED: &str = include_str!("../../fixtures/nested-byte-slice-traversal.spx");
#[test]
fn nested_byte_traversal_matches_manual_loop_on_interpreter_native_and_wasm() {
    // Rust independently enumerates the ordered Cartesian product.
    let ordinary: i64 = [1u8, 2]
        .iter()
        .flat_map(|outer| {
            [3u8, 4, 5]
                .iter()
                .map(move |inner| i64::from(*outer) * 10 + i64::from(*inner))
        })
        .sum();
    let inner_exits = 14 + 24;
    let while_once = 13 + 14 + 15;
    let expected = ordinary + inner_exits + while_once;
    assert_eq!(expected, 194);
    let ast = semaprax::check(NESTED, "nested-bytes.spx").unwrap();
    hir::validate(&hir::resolve(&ast).unwrap()).unwrap();
    let failure = NESTED.replace(
        "for inner in right { total",
        "for inner in right { let checked = reject(inner); total",
    )
        + "\n@id(\"app.nested.reject\") fn reject(value:u8)->u8 requires value != 3u8 {value}\n";
    let failed_ast = semaprax::check(&failure, "nested-byte-failure.spx").unwrap();
    hir::validate(&hir::resolve(&failed_ast).unwrap()).unwrap();
    let root =
        std::env::temp_dir().join(format!("semaprax-nested-byte-for-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    for (name, source, program, value) in [
        ("nested", NESTED, &ast, Some(expected)),
        ("nested-failure", failure.as_str(), &failed_ast, None),
    ] {
        let path = root.join(format!("{name}.spx"));
        std::fs::write(&path, source).unwrap();
        let outcome = interpreter::interpret(
            &path,
            "app.nested.main",
            &[],
            &interpreter::InterpreterOptions::default(),
        )
        .unwrap();
        if let Some(value) = value {
            assert!(outcome.returned, "{}", outcome.envelope);
            assert!(
                outcome.envelope.contains(&format!("\"value\":\"{value}\"")),
                "{}",
                outcome.envelope
            );
        } else {
            assert!(!outcome.returned);
            let report: serde_json::Value = serde_json::from_str(&outcome.envelope).unwrap();
            assert_eq!(
                report["payload"]["outcome"]["status"]["domain_id"],
                "semaprax.contract.v1"
            );
            assert_eq!(report["payload"]["outcome"]["status"]["code"], 1);
        }
        let c = root.join(format!("{name}.c"));
        std::fs::write(&c, codegen::emit_c(program).unwrap()).unwrap();
        for optimization in ["-O0", "-O2"] {
            let binary = root.join(format!("{name}-{optimization}"));
            let compiled = Command::new("clang")
                .args(["-std=c11", "-Wall", "-Wextra", "-Werror", optimization])
                .arg(&c)
                .arg("-o")
                .arg(&binary)
                .output()
                .unwrap();
            assert!(
                compiled.status.success(),
                "{}",
                String::from_utf8_lossy(&compiled.stderr)
            );
            let outcome = Command::new(&binary).output().unwrap();
            if let Some(value) = value {
                assert!(outcome.status.success());
                assert_eq!(
                    String::from_utf8_lossy(&outcome.stdout).trim(),
                    value.to_string()
                );
            } else {
                assert_eq!(outcome.status.code(), Some(70));
                assert!(String::from_utf8_lossy(&outcome.stderr)
                    .contains("requires value != 3u8 in app.nested.reject"));
            }
        }
        let module = semaprax::wasm::emit_module(program).unwrap();
        assert_eq!(module, semaprax::wasm::emit_module(program).unwrap());
        raw_wasm(&root, name, &module, value.unwrap_or(0));
    }
    std::fs::remove_dir_all(root).unwrap();
}
