use super::*;

#[test]
fn browser_runtime_normalizes_internal_contract_and_byte_range_selectors() {
    if std::process::Command::new("node")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let root = std::env::temp_dir().join(format!(
        "semaprax-browser-status-selectors-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let runtime = browser_runtime()
        .replace("__SEMAPRAX_OWNED_EXPORTS__", "Object.freeze({})")
        .replace("__SEMAPRAX_WASM_SHA256__", &"0".repeat(64));
    std::fs::write(root.join("runtime.mjs"), runtime).unwrap();
    std::fs::write(
        root.join("probe.mjs"),
        r#"import {imports,semanticStatus} from './runtime.mjs';
for (const [selector,domain_id,code] of [
  [9,'semaprax.contract.v1',1],
  [10,'semaprax.contract.v1',2],
  [11,'semaprax.byte-range.v1',1],
  [12,'semaprax.byte-range.v1',2],
  [16,'semaprax.byte-buffer.v1',1],
]) {
  let actual=null;
  try { imports.env.spx_contract_fail(selector); } catch (error) { actual=semanticStatus(error); }
  if (actual===null||actual.domain_id!==domain_id||actual.code!==code) throw Error(JSON.stringify({selector,actual}));
}
"#,
    )
    .unwrap();
    let output = std::process::Command::new("node")
        .arg("probe.mjs")
        .current_dir(&root)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(root.join("probe.mjs"));
    let _ = std::fs::remove_file(root.join("runtime.mjs"));
    let _ = std::fs::remove_dir(root);
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn embedded_manifest(project: &str, artifacts: &[Vec<u8>]) -> String {
    let digest = |index: usize| {
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(&artifacts[index]))
        )
    };
    format!(
            "{{\"schema\":\"semaprax.web-project.v1\",\"project_schema\":\"semaprax.project.v1\",\"project\":{project:?},\"project_revision\":\"sha256:project\",\"workspace_revision\":\"sha256:workspace\",\"project_graph_digest\":\"sha256:graph\",\"entry_module\":\"calculator.app\",\"capabilities\":[],\"artifacts\":[{{\"path\":\"app.wasm\",\"sha256\":\"{}\"}},{{\"path\":\"index.html\",\"sha256\":\"{}\"}},{{\"path\":\"package.json\",\"sha256\":\"{}\"}},{{\"path\":\"semaprax.bindings.d.ts\",\"sha256\":\"{}\"}},{{\"path\":\"semaprax.bindings.js\",\"sha256\":\"{}\"}},{{\"path\":\"semaprax.js\",\"sha256\":\"{}\"}}],\"scalar_abi\":{{\"schema\":\"semaprax.wasm-scalar.v1\",\"functions\":[{{\"stable_id\":\"calculator.add\",\"wasm_export\":{},\"parameters\":[\"i64\",\"i64\"],\"result\":\"i64\"}}]}}}}\n",
            digest(0),
            digest(6),
            digest(5),
            digest(3),
            digest(2),
            digest(1),
            quote_json(&scalar_exports::raw_symbol("calculator.add")),
        )
}

#[test]
fn independently_replayed_inner_manifest_rejects_self_resigned_identity_forgery() {
    let mut bytes = vec![
        b"wasm".to_vec(),
        b"runtime".to_vec(),
        b"bindings".to_vec(),
        b"declarations".to_vec(),
        Vec::new(),
        b"package".to_vec(),
        b"index".to_vec(),
    ];
    bytes[4] = embedded_manifest("calculator", &bytes).into_bytes();
    let refs = PROJECT_WEB_ARTIFACT_PATHS
        .iter()
        .copied()
        .zip(bytes.iter().map(Vec::as_slice))
        .collect::<Vec<_>>();
    build_project_web_carrier(
        ProjectWebIdentity {
            project_name: "calculator",
            project_revision: "sha256:project",
            workspace_revision: "sha256:workspace",
            project_graph_digest: "sha256:graph",
            entry_module: "calculator.app",
        },
        64 * 1024,
        &refs,
    )
    .unwrap();

    bytes[4] = embedded_manifest("calculat0r", &bytes).into_bytes();
    let forged_refs = PROJECT_WEB_ARTIFACT_PATHS
        .iter()
        .copied()
        .zip(bytes.iter().map(Vec::as_slice))
        .collect::<Vec<_>>();
    let error = build_project_web_carrier(
        ProjectWebIdentity {
            project_name: "calculator",
            project_revision: "sha256:project",
            workspace_revision: "sha256:workspace",
            project_graph_digest: "sha256:graph",
            entry_module: "calculator.app",
        },
        64 * 1024,
        &forged_refs,
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-W117");
    assert!(error.message.contains("embedded manifest disagrees"));

    bytes[4] = embedded_manifest("calculator", &bytes)
        .replacen('{', "{ ", 1)
        .into_bytes();
    let noncanonical_refs = PROJECT_WEB_ARTIFACT_PATHS
        .iter()
        .copied()
        .zip(bytes.iter().map(Vec::as_slice))
        .collect::<Vec<_>>();
    let error = build_project_web_carrier(
        ProjectWebIdentity {
            project_name: "calculator",
            project_revision: "sha256:project",
            workspace_revision: "sha256:workspace",
            project_graph_digest: "sha256:graph",
            entry_module: "calculator.app",
        },
        64 * 1024,
        &noncanonical_refs,
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-W117");
    assert!(error.message.contains("not canonical exact replay"));
}
