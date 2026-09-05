use super::*;

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
