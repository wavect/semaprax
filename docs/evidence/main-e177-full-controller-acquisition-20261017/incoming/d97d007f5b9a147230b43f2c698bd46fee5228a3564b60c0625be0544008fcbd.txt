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
        ("view-failure", failure.as_str(), &failed_ast, None),
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

/// Exercise the host oracle itself with tiny ABI modules. The owning test above
/// remains responsible for checking compiler output on all three engines.
#[test]
fn borrowed_failure_host_keeps_owned_zero_byte_cleanup_strict() {
    fn leb(mut value: u32, bytes: &mut Vec<u8>) {
        loop {
            let byte = (value & 127) as u8;
            value >>= 7;
            bytes.push(byte | if value == 0 { 0 } else { 128 });
            if value == 0 {
                break;
            }
        }
    }
    fn section(module: &mut Vec<u8>, tag: u8, bytes: &[u8]) {
        module.push(tag);
        leb(u32::try_from(bytes.len()).unwrap(), module);
        module.extend_from_slice(bytes);
    }
    fn name(bytes: &mut Vec<u8>, text: &str) {
        leb(u32::try_from(text.len()).unwrap(), bytes);
        bytes.extend_from_slice(text.as_bytes());
    }
    fn module(copy: bool, drop: bool, fail: bool) -> Vec<u8> {
        let mut module = b"\0asm\x01\0\0\0".to_vec();
        section(
            &mut module,
            1,
            &[
                5, 0x60, 2, 0x7e, 0x7e, 1, 0x7e, 0x60, 1, 0x7e, 1, 0x7e, 0x60, 1, 0x7f, 0, 0x60, 1,
                0x7e, 0, 0x60, 0, 1, 0x7e,
            ],
        );
        let mut imports = vec![11];
        for (symbol, ty) in [
            ("spx_add", 0),
            ("spx_sub", 0),
            ("spx_mul", 0),
            ("spx_div", 0),
            ("spx_rem", 0),
            ("spx_neg", 1),
            ("spx_contract_fail", 2),
            ("spx_bytes_copy", 1),
            ("spx_bytes_get", 0),
            ("spx_bytes_drop", 3),
            ("spx_bytes_as_slice", 1),
        ] {
            name(&mut imports, "env");
            name(&mut imports, symbol);
            imports.extend_from_slice(&[0, ty]);
        }
        section(&mut module, 2, &imports);
        section(&mut module, 3, &[1, 4]);
        section(&mut module, 5, &[1, 0, 2]);
        let mut exports = vec![2];
        name(&mut exports, "semaprax_main");
        exports.extend_from_slice(&[0, 11]);
        name(&mut exports, "__spx_byte_memory");
        exports.extend_from_slice(&[2, 0]);
        section(&mut module, 7, &exports);
        let mut body = if copy { vec![1, 1, 0x7e] } else { vec![0] };
        if copy {
            // Copy the zero-byte view and retain the actual owner token.
            body.extend_from_slice(&[0x42, 0, 0x10, 7, 0x21, 0]);
        }
        if drop {
            body.extend_from_slice(&[0x20, 0, 0x10, 9]);
        }
        if fail {
            body.extend_from_slice(&[0x41, 9, 0x10, 6]);
        }
        body.extend_from_slice(&[0x42, 0, 0x0b]);
        let mut code = vec![1];
        leb(u32::try_from(body.len()).unwrap(), &mut code);
        code.extend_from_slice(&body);
        section(&mut module, 10, &code);
        module
    }

    let root = std::env::temp_dir().join(format!(
        "semaprax-borrowed-failure-host-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let host = root.join("host.mjs");
    std::fs::write(&host, include_str!("host.mjs")).unwrap();
    for (label, copy, drop, fail, profile, accepted) in [
        ("borrowed", false, false, true, "view-failure", true),
        ("owned-empty", true, true, true, "catalog-failure-0", true),
        (
            "missing-owner",
            false,
            false,
            true,
            "catalog-failure-0",
            false,
        ),
        ("forbidden-owner", true, true, true, "view-failure", false),
        (
            "missing-drop",
            true,
            false,
            true,
            "catalog-failure-0",
            false,
        ),
        (
            "missing-failure",
            false,
            false,
            false,
            "view-failure",
            false,
        ),
        ("borrowed-success", false, false, false, "nested", true),
    ] {
        let wasm = root.join(format!("{label}.wasm"));
        std::fs::write(&wasm, module(copy, drop, fail)).unwrap();
        let outcome = Command::new("node")
            .arg(&host)
            .arg(&wasm)
            .args([profile, "0"])
            .output()
            .unwrap();
        assert_eq!(
            outcome.status.success(),
            accepted,
            "{label}: {}",
            String::from_utf8_lossy(&outcome.stderr)
        );
        if accepted {
            assert_eq!(
                String::from_utf8_lossy(&outcome.stdout).trim(),
                "byte-slice-for-ok"
            );
        } else {
            let diagnostic = match label {
                "missing-owner" => "0 !== 1",
                "forbidden-owner" => "view traversal called copy",
                "missing-drop" => "report follows complete non-result cleanup",
                "missing-failure" => "body failure returned",
                _ => unreachable!(),
            };
            assert!(
                String::from_utf8_lossy(&outcome.stderr).contains(diagnostic),
                "{label}: {}",
                String::from_utf8_lossy(&outcome.stderr)
            );
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}
