use super::*;

#[test]
fn inline_status_receipts_keep_all_complete_inputs_and_original_validation_loops() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let proofs: Vec<serde_json::Value> = serde_json::from_str(
        &std::fs::read_to_string(root.join("docs/evidence/status-inline-receipts-v1/sources.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(proofs.len(), 7);
    let mut paths = std::collections::BTreeSet::new();
    for proof in &proofs {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            std::fs::read_to_string(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
    }
}

// Distinct closed Main6d acquisition; the complete incoming controller above is unchanged.
const MAIN6D_CAP: usize = 262_144;
fn main6d_error(message: &'static str) -> ::std::io::Error {
    ::std::io::Error::new(::std::io::ErrorKind::InvalidData, message)
}
fn main6d_held(
    path: &::std::path::Path,
    length: usize,
    expected: &str,
) -> ::std::io::Result<String> {
    if length > MAIN6D_CAP {
        return Err(main6d_error("Main6d exact source bound"));
    }
    crate::source_reflows_merge::main_e177_inputs::main_e177_file_raw(path, length, expected)
        .map_err(main6d_error)
}
fn main6d_compiled(source: &str) -> Option<&'static str> {
    match source {
        "src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs" => Some(::std::include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs"))),
        "src/cleanup_plan/replay/expression_facts/receiving/retained.rs" => Some(::std::include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/expression_facts/receiving/retained.rs"))),
        "tests/hir_module_boundaries.rs" => Some(::std::include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/tests/hir_module_boundaries.rs"))),
        "tests/workspace/semantic_graph/source_inventory.rs" => Some(::std::include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/tests/workspace/semantic_graph/source_inventory.rs"))),
        "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs" => Some(::std::include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs"))),
        "src/source_reflows/semantic_receivers.rs" => Some(::std::include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/source_reflows/semantic_receivers.rs"))),
        "docs/evidence/projection-audit-scope-v1/legacy-inputs.tsv" => Some(::std::include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/docs/evidence/projection-audit-scope-v1/legacy-inputs.tsv"))),
        _ => None,
    }
}
fn main6d_packet() -> ::std::io::Result<serde_json::Value> {
    let data = ::std::include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/main6d-closed-controller-acquisition-20261017/inputs.json"
    ));
    assert_eq!(data.len(), 6867);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(<sha2::Sha256 as sha2::Digest>::digest(data.as_bytes()))
        ),
        "7003275c8ccc1f6ebdb64ef96ccdc6297aac4f85e76f12d080f73b7dfef03050"
    );
    let root = ::std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        main6d_held(
            &root.join("docs/evidence/main6d-closed-controller-acquisition-20261017/inputs.json"),
            6867,
            "7003275c8ccc1f6ebdb64ef96ccdc6297aac4f85e76f12d080f73b7dfef03050"
        )?,
        data,
        "whole physical and compiled Main6d packet"
    );
    let packet: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(
        packet["schema"],
        "semaprax.main6d-closed-controller-acquisition.v1"
    );
    assert_eq!(
        packet["main_head"],
        "6d101efd8c255330a3fe5fb0c06e058c6f8c085f"
    );
    assert_eq!(packet["rows"].as_array().unwrap().len(), 7);
    Ok(packet)
}
fn main6d_read(path: &::std::path::Path) -> ::std::io::Result<String> {
    let root = ::std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let relative = path
        .strip_prefix(root)
        .map_err(|_| main6d_error("foreign Main6d root"))?;
    if !relative
        .components()
        .all(|part| matches!(part, ::std::path::Component::Normal(_)))
    {
        return Err(main6d_error("foreign Main6d spelling"));
    }
    let source = relative
        .to_str()
        .ok_or_else(|| main6d_error("nonUTF8 Main6d path"))?;
    if source == "docs/evidence/status-inline-receipts-v1/sources.json" {
        let text = main6d_held(
            path,
            7249,
            "5534b323fbebfaa7db577853f154231fba164dc18572645a1eaf25d0be23cdeb",
        )?;
        assert_eq!(
            text,
            ::std::include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/status-inline-receipts-v1/sources.json"
            )),
            "whole immutable Main6d control table"
        );
        return Ok(text);
    }
    let packet = main6d_packet()?;
    let rows = packet["rows"].as_array().unwrap();
    let before: Vec<_> = rows
        .iter()
        .filter(|row| row["before"]["path"] == source)
        .collect();
    if before.len() == 1 {
        let before = &before[0]["before"];
        return main6d_held(
            path,
            before["length"].as_u64().unwrap() as usize,
            before["sha256"].as_str().unwrap(),
        );
    }
    let found: Vec<_> = rows.iter().filter(|row| row["source"] == source).collect();
    if found.len() != 1 {
        return Err(main6d_error("unknown or duplicate Main6d source"));
    }
    let row = found[0];
    let current = &row["current"];
    let actual = main6d_held(
        path,
        current["length"].as_u64().unwrap() as usize,
        current["sha256"].as_str().unwrap(),
    )?;
    let compiled = main6d_compiled(source).unwrap();
    let compiled = crate::source_reflows_merge::main_e177_final_inputs::value_direct(
        source,
        current["sha256"].as_str().unwrap(),
        compiled,
    );
    assert_eq!(actual, compiled, "whole actual and compiled Main6d source");
    assert_eq!(
        actual,
        main6d_held(
            &root.join(current["path"].as_str().unwrap()),
            current["length"].as_u64().unwrap() as usize,
            current["sha256"].as_str().unwrap()
        )?,
        "whole current source archive"
    );
    let incoming = &row["incoming"];
    main6d_held(
        &root.join(incoming["path"].as_str().unwrap()),
        incoming["length"].as_u64().unwrap() as usize,
        incoming["sha256"].as_str().unwrap(),
    )
}
mod std {
    pub use ::std::{collections, panic};
    pub mod fs {
        pub fn read_to_string(path: impl AsRef<::std::path::Path>) -> ::std::io::Result<String> {
            super::super::main6d_read(path.as_ref())
        }
    }
}

// Resolve the unchanged incoming controller against its complete incoming
// inverse, rather than the unrelated current-parent normalization gateway.
fn invert(proof: &serde_json::Value, current: &str) -> String {
    let path = proof["path"].as_str().unwrap();
    assert_eq!(
        digest(current),
        proof["after_sha256"],
        "receiver input drifted: {path}"
    );
    let mut restored = current.to_owned();
    for edit in proof["inverse"].as_array().unwrap() {
        let after = edit["after"].as_str().unwrap();
        assert_eq!(
            restored.matches(after).count(),
            1,
            "nonunique receiver inverse: {path}"
        );
        restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
    }
    assert_eq!(
        digest(&restored),
        proof["before_sha256"],
        "receiver predecessor drifted: {path}"
    );
    restored
}
