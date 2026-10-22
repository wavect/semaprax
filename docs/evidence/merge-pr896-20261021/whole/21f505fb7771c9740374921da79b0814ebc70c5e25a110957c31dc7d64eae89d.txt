//! Exact custody for later receiver changes, separate from CI reformatting.
//! These snapshots are used only by historical source assertions. Live source
//! inventories continue to include the current modules and their new children.
use super::*;

const LIVE_AUDITS: &str =
    include_str!("../../docs/evidence/live-source-audit-custody-v1/inverse.json");
static PROOF_RECORDS: OnceLock<Vec<serde_json::Value>> = OnceLock::new();

// A later test receiver has its own semantic inverse before the unchanged
// formatting registry. Keep all original twelve plus two custody proofs intact.
fn followups() -> &'static [serde_json::Value] {
    static RECORDS: OnceLock<Vec<serde_json::Value>> = OnceLock::new();
    RECORDS.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../docs/evidence/source-reader-followups-20261016/allocation.json"
        ))
        .unwrap()
    })
}

// Physical setup successors have their own complete-input custody before the
// frozen formatting input. They do not edit or replace any formatting repair,
// original twelve-plus-two receiver proof, or allocation followup.
fn ordinary_setup() -> &'static [serde_json::Value] {
    static RECORDS: OnceLock<Vec<serde_json::Value>> = OnceLock::new();
    RECORDS.get_or_init(|| {
        serde_json::from_str(
            &std::fs::read_to_string(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("docs/evidence/ordinary-replay-setup-v1/format-inputs.json"),
            )
            .unwrap(),
        )
        .unwrap()
    })
}

// Other promoted helper inputs also retain their complete historical view.
// Read evidence at runtime so these archived modules are not embedded in each
// compiler test binary.
fn ordinary_helpers() -> &'static [serde_json::Value] {
    static RECORDS: OnceLock<Vec<serde_json::Value>> = OnceLock::new();
    RECORDS.get_or_init(|| {
        serde_json::from_str(
            &std::fs::read_to_string(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("docs/evidence/ordinary-replay-setup-v1/helper-inputs.json"),
            )
            .unwrap(),
        )
        .unwrap()
    })
}

fn proofs() -> &'static [serde_json::Value] {
    // Preserve the old parsed metadata cache, but authenticate the runtime
    // dependency on every reached lookup, including an already warm cache.
    let payload = crate::source_reflows_merge::receiver_fixture(
        crate::source_reflows_merge::ReceiverFixture::Main67107,
    );
    PROOF_RECORDS.get_or_init(|| {
        let mut original: Vec<serde_json::Value> = serde_json::from_str(&payload).unwrap();
        let additional: Vec<serde_json::Value> = serde_json::from_str(LIVE_AUDITS).unwrap();
        assert_eq!(original.len(), 12);
        assert_eq!(additional.len(), 2);
        original.extend(additional);
        original
    })
}

fn digest(source: &str) -> String {
    format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))
    )
}

pub(super) fn invert_ordinary(proof: &serde_json::Value, current: &str) -> String {
    let current_view = crate::source_reflows_merge::main_e177_final_inputs::value(
        proof["path"].as_str().unwrap(),
        proof["after_sha256"].as_str().unwrap(),
        current,
    );
    let current = current_view.as_str();
    // Only these nine exact Main67107 entry tuples changed after this packet.
    // The full existing MAIN21 acquisition and every inverse below still run.
    let path = proof["path"].as_str().unwrap();
    let expected = proof["after_sha256"].as_str().unwrap();
    let entry = match (path, expected) {
        (
            "src/cleanup_plan/replay.rs",
            "30616d71694d393a730c0c8e49a25bbad7539b07ff96ed901b9d07be29765b9c",
        )
        | (
            "tests/hir_module_boundaries.rs",
            "f25b784eba1a2ea26a07b260b2ddff67b6dd90c0a6402fd143dc42ca68847d4e",
        )
        | (
            "tests/workspace/semantic_graph/source_inventory.rs",
            "59e5b98593c2c2d443d1479e7c89564551d515ca921f7abab2cc8f5d579c8c21",
        )
        | (
            "src/source_verify/declaration.rs",
            "c93ac4c883fe6b10c831a2603c994ea03880986fd0b7a394bfa41ec8a06d88cb",
        )
        | (
            "src/cleanup_plan/replay/branch_clone/tests.rs",
            "46bf0451ef390225f540d031f326100c7f4288cf7e9d50e30ce7ddb68a448e0f",
        )
        | (
            "src/cleanup_plan/replay/factored.rs",
            "343eda3712d0c3b224742d72c34838a327e0c69449c8a94fd7eb81dc5e0134a6",
        )
        | (
            "src/cleanup_plan/replay/factored/oracle_hook.rs",
            "0126d25150ca4b96f11995293fc9043d23fc8fff2008485e9560abe5c85cb7ae",
        )
        | (
            "src/hir/validation.rs",
            "f89e882a3d8ed796b8b8b864247e4e7eb19631d6299cd0dd94ddcc6b5ca17eba",
        )
        | (
            "src/hir/expr_nodes.rs",
            "e2291267786bdec4371b969031209b4e77aa6876f4564048ad1c7561b004b73f",
        ) => Some(
            crate::source_reflows_merge::main_e177_final_inputs::value_direct(
                path, expected, current,
            ),
        ),
        _ => None,
    };
    let acquired = super::semantic_receivers::incoming_main21::ours_expected_input(
        proof,
        entry.as_deref().unwrap_or(current),
    );
    let current = acquired.as_str();
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

fn receiver_input(path: &str, current: &str) -> String {
    let domain = match path {
        "src/cleanup_plan/replay/selected_entry/tests/session_errors/source.rs" => {
            "liveSessionMAIN"
        }
        "src/hir/nominal_payload/legacy/source_tests.rs" => "liveNominalMAIN",
        _ => return current.to_owned(),
    };
    crate::source_reflows_merge::main67107_inputs::project(domain, current)
}
fn invert(proof: &serde_json::Value, current: &str) -> String {
    let view = receiver_input(proof["path"].as_str().unwrap(), current);
    invert_ordinary(proof, &view)
}

pub(super) fn predecessor(path: &str, current: &str) -> Option<String> {
    if let Some(proof) = ordinary_setup()
        .iter()
        .chain(ordinary_helpers())
        .find(|proof| proof["path"] == path)
    {
        return Some(invert(proof, current));
    }
    if let Some(proof) = followups().iter().find(|proof| proof["path"] == path) {
        return Some(invert(proof, current));
    }
    let proof = proofs().iter().find(|proof| proof["path"] == path)?;
    Some(invert(proof, current))
}

pub(super) fn contains(path: &str) -> bool {
    proofs()
        .iter()
        .chain(followups())
        .chain(ordinary_setup())
        .chain(ordinary_helpers())
        .any(|proof| proof["path"] == path)
}

#[test]
fn ordinary_helper_views_authenticate_every_complete_input() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(ordinary_helpers().len(), 20);
    let mut paths = std::collections::BTreeSet::new();
    for proof in ordinary_helpers() {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        assert!(!proofs()
            .iter()
            .chain(followups())
            .chain(ordinary_setup())
            .any(|old| old["path"] == path));
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            std::fs::read_to_string(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(contains(path));
        assert_eq!(predecessor(path, &current), Some(before));
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
    }
}

#[test]
fn ordinary_setup_complete_sources_and_original_legacy_constructor_remain_exact() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let evidence = root.join("docs/evidence/ordinary-replay-setup-v1");
    let rows: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(evidence.join("sources.json")).unwrap())
            .unwrap();
    assert_eq!(rows.len(), 34);
    let mut paths = std::collections::BTreeSet::new();
    for row in &rows {
        let path = row["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let before = invert(row, &current);
        assert_eq!(
            before,
            std::fs::read_to_string(evidence.join(row["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert(row, &(current.clone() + "\n"))).is_err());
        if path == "src/cleanup_plan/replay/skeleton_work.rs" {
            let start = before.find("        Ok(Self {\n").unwrap();
            let end = before[start..]
                .find("\n    pub(super) fn checked_with_index(")
                .unwrap()
                + start;
            let legacy = &before[start..end];
            assert_eq!(current.matches(legacy).count(), 1);
        }
    }
}

#[test]
fn ordinary_setup_bridges_preserve_every_complete_original_format_input() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(ordinary_setup().len(), 6);
    let mut paths = std::collections::BTreeSet::new();
    for proof in ordinary_setup() {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        assert!(!proofs()
            .iter()
            .chain(followups())
            .any(|old| old["path"] == path));
        let original_format = records()
            .iter()
            .find(|record| record["path"] == path)
            .unwrap();
        assert_eq!(proof["before_sha256"], original_format["after_sha256"]);
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            std::fs::read_to_string(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert_eq!(
            digest(&restore(original_format, &before)),
            original_format["before_sha256"]
        );
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
    }
}

#[test]
fn every_receiver_inverse_preserves_the_entire_reflow_input_and_archive() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut paths = std::collections::BTreeSet::new();
    assert_eq!(proofs().len(), 14);
    for proof in proofs() {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let repair = records().iter().find(|repair| repair["path"] == path);
        if let Some(repair) = repair {
            assert_eq!(proof["before_sha256"], repair["after_sha256"]);
        }
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let restored = invert(proof, &current);
        let source_audits: Vec<serde_json::Value> = serde_json::from_str(LIVE_AUDITS).unwrap();
        let archive_root = if source_audits.iter().any(|audit| audit["path"] == path) {
            "docs/evidence/live-source-audit-custody-v1"
        } else {
            "docs/evidence/receiver-source-custody-v1"
        };
        let archive = root
            .join(archive_root)
            .join(proof["archive"].as_str().unwrap());
        assert_eq!(restored, std::fs::read_to_string(archive).unwrap());
        let current = receiver_input(path, &current);
        assert_ne!(current, restored);
        // Execute the unchanged formatting inverse and its whole-file digest.
        if let Some(repair) = repair {
            assert_ne!(restore(repair, &restored), restored);
        }
    }
    assert!(predecessor("src/source_reflows.rs", "unregistered source").is_none());
}

#[test]
fn receiver_mutation_and_forged_predecessor_fail_closed() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for proof in proofs() {
        let current = std::fs::read_to_string(root.join(proof["path"].as_str().unwrap())).unwrap();
        let changed = format!("{current} ");
        assert!(std::panic::catch_unwind(|| invert(proof, &changed)).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
    }
}

#[test]
fn allocation_followup_preserves_the_complete_original_format_input() {
    assert_eq!(followups().len(), 1);
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for proof in followups() {
        let path = proof["path"].as_str().unwrap();
        assert!(!proofs().iter().any(|original| original["path"] == path));
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let restored = invert(proof, &current);
        let archive = root
            .join("docs/evidence/source-reader-followups-20261016")
            .join(proof["archive"].as_str().unwrap());
        assert_eq!(restored, std::fs::read_to_string(archive).unwrap());
        let format = records()
            .iter()
            .find(|record| record["path"] == path)
            .unwrap();
        assert_eq!(proof["before_sha256"], format["after_sha256"]);
        assert_ne!(restore(format, &restored), restored);
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
    }
}

fn followup_input(path: &str, current: &str) -> String {
    let domain = match path {
        "src/source_reflows/semantic_receivers.rs" => "followupReceiverMAIN",
        "src/workspace_graph/identity_resolution.rs" => "followupIdentityMAIN",
        "src/workspace_graph/expected_projection/import_stub/core_proofs/tests.rs" => {
            "followupCoreMAIN"
        }
        _ => panic!("foreign MAIN followup source"),
    };
    let registered =
        if path == "src/workspace_graph/expected_projection/import_stub/core_proofs/tests.rs" {
            crate::source_reflows_merge::main_e177_final_inputs::value(
                path,
                "bf59d79e6bcd513cf071c52112ded222e4561c785389f2723684d072c15d0fc2",
                current,
            )
        } else {
            current.to_owned()
        };
    let incoming = crate::source_reflows_merge::main67107_inputs::project(domain, &registered);
    if path == "src/source_reflows/semantic_receivers.rs" {
        crate::source_reflows_merge::complete_receiver_followup(&incoming)
    } else {
        incoming
    }
}

#[test]
fn complete_followup_receivers_keep_all_original_source_bytes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let evidence = root.join("docs/evidence/source-reader-followups-20261016");
    let records: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(evidence.join("sources.json")).unwrap())
            .unwrap();
    for record in records.as_array().unwrap() {
        let original =
            std::fs::read_to_string(evidence.join(record["archive"].as_str().unwrap())).unwrap();
        assert_eq!(digest(&original), record["sha256"]);
        let current = std::fs::read_to_string(root.join(record["path"].as_str().unwrap())).unwrap();
        let current = followup_input(record["path"].as_str().unwrap(), &current);
        let restore = |source: &str| -> Option<String> {
            let mut restored = source.to_owned();
            for edit in record["edits"].as_array().unwrap().iter().rev() {
                let new = edit["new"].as_str().unwrap();
                if restored.matches(new).count() != 1 {
                    return None;
                }
                restored = restored.replacen(new, edit["old"].as_str().unwrap(), 1);
            }
            Some(restored)
        };
        assert_eq!(restore(&current).as_deref(), Some(original.as_str()));
        assert_ne!(
            restore(&(current.clone() + "\n")).as_deref(),
            Some(original.as_str())
        );
        assert_ne!(
            restore(&current.replacen(&current[..1], "!", 1)).as_deref(),
            Some(original.as_str())
        );
    }
}

// Three incoming MAIN source-only domains. This small runtime manifest, sealed by compiled length and SHA, binds
// complete current and MAIN inputs; no source, authority, or production text is
// changed. Whole source payloads remain bounded runtime files, not large embeds.
const MAIN34D_MANIFEST_LEN: usize = 51371;
const MAIN34D_MANIFEST_SHA: &str =
    "d9d9e7168cea74f3f860e02ae631fd3bec90bb9dbf88b121e0edcf21f582288d";
// Separate NEWMAIN-only admitted source-proof input: held-status sources.json.
// The unrelated runtime_json335 cap remains exactly5,081,115 and is untouched.
const MAIN34D_CAP: usize = 9_317_546;
fn main34d_manifest() -> serde_json::Value {
    let text = main34d_file(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("docs/evidence/main34d-three-source-domains-20261017/input-manifest.json"),
        MAIN34D_MANIFEST_LEN,
        MAIN34D_MANIFEST_SHA,
    )
    .expect("complete closed MAIN input manifest");
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["schema"], "semaprax.main34d-three-source-domains.v1");
    assert_eq!(
        value["main_head"],
        "34d53510a3374537874c89a0e46be4d68ad9fe61"
    );
    let domains = value["domains"].as_array().unwrap();
    assert_eq!(domains.len(), 3);
    for (row, name) in domains
        .iter()
        .zip(["held-status", "held-continuation", "receiving-lookup"])
    {
        assert_eq!(row["name"], name);
        let mut unique = std::collections::BTreeSet::new();
        for path in row["reads"].as_array().unwrap() {
            assert!(unique.insert(path.as_str().unwrap()));
        }
    }
    let entries = value["inputs"].as_array().unwrap();
    let mut unique = std::collections::BTreeSet::new();
    for entry in entries {
        assert!(unique.insert(entry["path"].as_str().unwrap()));
    }
    value
}
fn main34d_body(
    reader: &mut impl std::io::Read,
    length: usize,
    expected: &str,
) -> Result<String, &'static str> {
    if length > MAIN34D_CAP {
        return Err("bound");
    }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).map_err(|_| "allocation")?;
    bytes.resize(length, 0);
    reader.read_exact(&mut bytes).map_err(|_| "truncated")?;
    let mut extra = [0u8; 1];
    if reader.read(&mut extra).map_err(|_| "read")? != 0 {
        return Err("extra");
    }
    if digest_bytes(&bytes) != expected {
        return Err("digest");
    }
    String::from_utf8(bytes).map_err(|_| "utf8")
}
fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)))
}
fn main34d_same_file(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    if !a.is_file() || !b.is_file() || a.len() != b.len() || a.modified().ok() != b.modified().ok()
    {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if a.dev() != b.dev()
            || a.ino() != b.ino()
            || a.ctime() != b.ctime()
            || a.ctime_nsec() != b.ctime_nsec()
        {
            return false;
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if a.file_attributes() & 0x400 != 0
            || b.file_attributes() & 0x400 != 0
            || a.creation_time() != b.creation_time()
            || a.last_write_time() != b.last_write_time()
        {
            return false;
        }
    }
    true
}
fn main34d_file(path: &Path, length: usize, expected: &str) -> Result<String, &'static str> {
    if length > MAIN34D_CAP {
        return Err("bound");
    }
    let path_before = std::fs::symlink_metadata(path).map_err(|_| "missing")?;
    if path_before.file_type().is_symlink() || !path_before.is_file() {
        return Err("length-or-nonregular");
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0020_0000);
    }
    #[cfg(not(any(unix, windows)))]
    {
        return Err("platform");
    }
    let mut file = options.open(path).map_err(|_| "open")?;
    let metadata = file.metadata().map_err(|_| "metadata")?;
    if !main34d_same_file(&path_before, &metadata)
        || metadata.len() != u64::try_from(length).map_err(|_| "bound")?
    {
        return Err("length-or-nonregular");
    }
    // The one opened no-follow regular handle is checked before any buffer.
    let value = main34d_body(&mut file, length, expected)?;
    let after = file.metadata().map_err(|_| "metadata")?;
    let path_after = std::fs::symlink_metadata(path).map_err(|_| "metadata")?;
    if path_after.file_type().is_symlink()
        || !main34d_same_file(&metadata, &after)
        || !main34d_same_file(&after, &path_after)
    {
        return Err("metadata-drift");
    }
    Ok(value)
}
fn main34d_entry<'a>(
    manifest: &'a serde_json::Value,
    domain: &str,
    path: &str,
) -> &'a serde_json::Value {
    let domains: Vec<_> = manifest["domains"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["name"] == domain)
        .collect();
    assert_eq!(domains.len(), 1, "foreign MAIN domain");
    assert!(
        domains[0]["reads"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == path),
        "foreign MAIN source"
    );
    let entries: Vec<_> = manifest["inputs"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["path"] == path)
        .collect();
    assert_eq!(entries.len(), 1, "missing or duplicate MAIN input");
    entries[0]
}
fn main34d_project_entry(entry: &serde_json::Value, current: &str) -> String {
    let current_view = crate::source_reflows_merge::main_e177_final_inputs::value(
        entry["path"].as_str().unwrap(),
        entry["current_sha256"].as_str().unwrap(),
        current,
    );
    let current = current_view.as_str();
    assert_eq!(current.len() as u64, entry["current_len"].as_u64().unwrap());
    assert_eq!(digest(current), entry["current_sha256"]);
    let inverse = entry["whole_source_inverse"].as_array().unwrap();
    assert_eq!(inverse.len(), 1);
    assert_eq!(inverse[0]["at"], 0);
    assert_eq!(inverse[0]["remove_len"], entry["current_len"]);
    assert_eq!(inverse[0]["remove_sha256"], entry["current_sha256"]);
    let archive = entry["main_archive"].as_str().unwrap();
    assert!(archive.starts_with("docs/evidence/") && !archive.split('/').any(|part| part == ".."));
    assert_eq!(inverse[0]["insert_archive"], archive);
    let result = main34d_file(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join(archive),
        usize::try_from(entry["main_len"].as_u64().unwrap()).unwrap(),
        entry["main_sha256"].as_str().unwrap(),
    )
    .expect("whole immutable MAIN input");
    assert_eq!(digest(&result), entry["main_sha256"]);
    result
}
pub(super) fn main34d_project(domain: &str, path: &str, current: &str) -> String {
    let manifest = main34d_manifest();
    main34d_project_entry(main34d_entry(&manifest, domain, path), current)
}
pub(super) fn main34d_read(domain: &str, path: &Path) -> std::io::Result<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let relative = path
        .strip_prefix(root)
        .expect("MAIN source must be repository local");
    let relative = relative.to_str().unwrap();
    let manifest = main34d_manifest();
    let entry = main34d_entry(&manifest, domain, relative);
    let length = usize::try_from(entry["current_len"].as_u64().unwrap()).unwrap();
    let expected = entry["current_sha256"].as_str().unwrap();
    assert!(length <= MAIN34D_CAP);
    let current = crate::source_reflows_merge::main_e177_final_inputs::file(path, length, expected)
        .map(Ok)
        .unwrap_or_else(|| main34d_file(path, length, expected))
        .expect("complete current raw MAIN input");
    Ok(main34d_project_entry(entry, &current))
}
pub(super) fn main34d_proofs() -> Vec<serde_json::Value> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = main34d_manifest();
    let mut result = Vec::new();
    for (name, count) in [("main_receiver_proofs", 12), ("main_live_audits", 2)] {
        let row = &manifest[name];
        let text = main34d_file(
            &root.join(row["archive"].as_str().unwrap()),
            usize::try_from(row["len"].as_u64().unwrap()).unwrap(),
            row["sha256"].as_str().unwrap(),
        )
        .unwrap();
        let values: Vec<serde_json::Value> = serde_json::from_str(&text).unwrap();
        assert_eq!(values.len(), count);
        result.extend(values);
    }
    result
}
pub(super) fn main34d_predecessor(path: &str, current: &str) -> Option<String> {
    for directory in [
        "held-status-prefix-v1",
        "held-continuation-growth-v1",
        "paid-expression-lookup-state-v1",
    ] {
        let relative = format!("docs/evidence/{directory}/views.json");
        let manifest = main34d_manifest();
        let domain = match directory {
            "held-status-prefix-v1" => "held-status",
            "held-continuation-growth-v1" => "held-continuation",
            _ => "receiving-lookup",
        };
        let entry = main34d_entry(&manifest, domain, &relative);
        let text = main34d_file(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join(entry["main_archive"].as_str().unwrap()),
            usize::try_from(entry["main_len"].as_u64().unwrap()).unwrap(),
            entry["main_sha256"].as_str().unwrap(),
        )
        .unwrap();
        let rows: Vec<serde_json::Value> = serde_json::from_str(&text).unwrap();
        if let Some(proof) = rows.iter().find(|row| row["path"] == path) {
            return Some(invert_ordinary(proof, current));
        }
    }
    None
}

#[test]
fn complete_main34d_inputs_keep_all_domains_and_refuse_wrong_source_and_bounded_io() {
    let manifest = main34d_manifest();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for domain in manifest["domains"].as_array().unwrap() {
        for path in domain["reads"].as_array().unwrap() {
            let path = path.as_str().unwrap();
            let value = main34d_read(domain["name"].as_str().unwrap(), &root.join(path)).unwrap();
            assert_eq!(
                digest(&value),
                main34d_entry(&manifest, domain["name"].as_str().unwrap(), path)["main_sha256"]
            );
        }
    }
    assert!(std::panic::catch_unwind(|| main34d_entry(
        &manifest,
        "foreign",
        "src/cleanup_plan/replay.rs"
    ))
    .is_err());
    assert!(
        std::panic::catch_unwind(|| main34d_entry(&manifest, "held-status", "src/foreign.rs"))
            .is_err()
    );
    let domains = manifest["domains"].as_array().unwrap();
    let exclusive = domains[0]["reads"]
        .as_array()
        .unwrap()
        .iter()
        .find(|path| !domains[1]["reads"].as_array().unwrap().contains(path))
        .unwrap()
        .as_str()
        .unwrap();
    assert!(
        std::panic::catch_unwind(|| main34d_entry(&manifest, "held-continuation", exclusive))
            .is_err()
    );
    let mut duplicate = manifest.clone();
    let first = duplicate["inputs"][0].clone();
    duplicate["inputs"].as_array_mut().unwrap().push(first);
    let path = manifest["inputs"][0]["path"].as_str().unwrap();
    let domain = manifest["domains"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| {
            row["reads"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == path)
        })
        .unwrap()["name"]
        .as_str()
        .unwrap();
    assert!(std::panic::catch_unwind(|| main34d_entry(&duplicate, domain, path)).is_err());
    let entry = main34d_entry(&manifest, domain, path);
    let raw = std::fs::read_to_string(root.join(path)).unwrap();
    for changed in [raw.clone() + "\n", raw[..raw.len() - 1].to_owned()] {
        assert!(std::panic::catch_unwind(|| main34d_project_entry(entry, &changed)).is_err());
    }
    for entry in manifest["inputs"].as_array().unwrap() {
        let raw = std::fs::read(root.join(entry["path"].as_str().unwrap())).unwrap();
        let raw = String::from_utf8(raw).unwrap();
        if entry["current_sha256"] != entry["main_sha256"] {
            let old = main34d_project_entry(entry, &raw);
            assert!(std::panic::catch_unwind(|| main34d_project_entry(entry, &old)).is_err());
        }
        for field in ["current_sha256", "main_sha256"] {
            let mut forged = entry.clone();
            forged[field] = serde_json::json!("0".repeat(64));
            assert!(std::panic::catch_unwind(|| main34d_project_entry(&forged, &raw)).is_err());
        }
        let mut forged = entry.clone();
        forged["whole_source_inverse"][0]["at"] = serde_json::json!(1);
        assert!(std::panic::catch_unwind(|| main34d_project_entry(&forged, &raw)).is_err());
    }
    let bytes = b"complete\n";
    let hash = digest_bytes(bytes);
    assert_eq!(
        main34d_body(&mut std::io::Cursor::new(bytes), bytes.len(), &hash),
        Ok("complete\n".to_owned())
    );
    assert_eq!(
        main34d_body(&mut std::io::Cursor::new(bytes), bytes.len() + 1, &hash),
        Err("truncated")
    );
    assert_eq!(
        main34d_body(&mut std::io::Cursor::new(bytes), bytes.len() - 1, &hash),
        Err("extra")
    );
    assert_eq!(
        main34d_body(
            &mut std::io::Cursor::new(bytes),
            bytes.len(),
            &"0".repeat(64)
        ),
        Err("digest")
    );
    assert_eq!(
        main34d_body(&mut std::io::Cursor::new([255u8]), 1, &digest_bytes(&[255])),
        Err("utf8")
    );
    assert_eq!(
        main34d_body(&mut std::io::Cursor::new(bytes), MAIN34D_CAP + 1, &hash),
        Err("bound")
    );
    let directory = std::env::temp_dir().join(format!(
        "main34d-reader-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    struct Remove(std::path::PathBuf);
    impl Drop for Remove {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _remove = Remove(directory.clone());
    let file = directory.join("regular");
    std::fs::write(&file, bytes).unwrap();
    assert_eq!(
        main34d_file(&file, bytes.len(), &hash),
        Ok("complete\n".to_owned())
    );
    for length in [bytes.len() - 1, bytes.len() + 1] {
        assert_eq!(
            main34d_file(&file, length, &hash),
            Err("length-or-nonregular")
        );
    }
    assert_eq!(main34d_file(&file, MAIN34D_CAP + 1, &hash), Err("bound"));
    std::fs::write(&file, b"Complete\n").unwrap();
    assert_eq!(main34d_file(&file, bytes.len(), &hash), Err("digest"));
    std::fs::write(&file, b"complete\r\n").unwrap();
    assert_eq!(
        main34d_file(&file, bytes.len(), &hash),
        Err("length-or-nonregular")
    );
    #[cfg(unix)]
    {
        let link = directory.join("link");
        std::os::unix::fs::symlink(&file, &link).unwrap();
        assert_eq!(
            main34d_file(&link, bytes.len() + 1, &digest_bytes(b"complete\r\n")),
            Err("length-or-nonregular")
        );
    }
    assert_eq!(
        main34d_file(root, 0, &digest_bytes(&[])),
        Err("length-or-nonregular")
    );
    assert_eq!(
        main34d_file(
            &root.join("docs/evidence/main34d-three-source-domains-20261017/NEVER_EXISTS"),
            0,
            &digest_bytes(&[])
        ),
        Err("missing")
    );
}

// Independent latest MAIN ordinary-routes domain, never a retarget of MAIN34d.
const MAINBCC_MANIFEST_LEN: usize = 12232;
const MAINBCC_MANIFEST_SHA: &str =
    "3b66d32488e373f260b8f37b4046c6929e3f22a7c6c01834e66951a1b29141ee";
const MAINBCC_CAP: usize = 1_048_576;
#[allow(
    clippy::assertions_on_constants,
    reason = "These assertions pin the exact control layout and resource limits."
)]
fn mainbcc_manifest() -> serde_json::Value {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(MAINBCC_MANIFEST_LEN <= MAINBCC_CAP);
    let text = main34d_file(
        &root.join("docs/evidence/mainbcc-ordinary-source-domain-20261017/input-manifest.json"),
        MAINBCC_MANIFEST_LEN,
        MAINBCC_MANIFEST_SHA,
    )
    .unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        value["schema"],
        "semaprax.mainbcc-ordinary-source-domain.v1"
    );
    assert_eq!(
        value["main_head"],
        "bccbe9696eca892045b0603ad663234062df15f4"
    );
    let entries = value["inputs"].as_array().unwrap();
    assert_eq!(entries.len(), 13);
    let mut paths = std::collections::BTreeSet::new();
    for entry in entries {
        assert!(paths.insert(entry["path"].as_str().unwrap()));
        for field in ["current_len", "main_len"] {
            assert!(entry[field].as_u64().unwrap() <= MAINBCC_CAP as u64);
        }
    }
    value
}
fn mainbcc_entry<'a>(manifest: &'a serde_json::Value, path: &str) -> &'a serde_json::Value {
    let entries: Vec<_> = manifest["inputs"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["path"] == path)
        .collect();
    assert_eq!(entries.len(), 1, "foreign, missing or duplicate BCC source");
    entries[0]
}
pub(super) fn mainbcc_read(path: &Path) -> std::io::Result<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let relative = path.strip_prefix(root).unwrap().to_str().unwrap();
    let manifest = mainbcc_manifest();
    let entry = mainbcc_entry(&manifest, relative);
    let length = usize::try_from(entry["current_len"].as_u64().unwrap()).unwrap();
    let expected = entry["current_sha256"].as_str().unwrap();
    assert!(length <= MAINBCC_CAP);
    let raw = crate::source_reflows_merge::main_e177_final_inputs::file(path, length, expected)
        .map(Ok)
        .unwrap_or_else(|| main34d_file(path, length, expected))
        .unwrap();
    Ok(main34d_project_entry(entry, &raw))
}
pub(super) fn mainbcc_invert(proof: &serde_json::Value, current: &str) -> String {
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

#[test]
fn complete_mainbcc_routes_keep_all_six_rows_and_refuse_foreign_inputs() {
    let manifest = mainbcc_manifest();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let table =
        mainbcc_read(&root.join("docs/evidence/ordinary-replay-routes-v1/sources.json")).unwrap();
    let rows: Vec<serde_json::Value> = serde_json::from_str(&table).unwrap();
    assert_eq!(rows.len(), 6);
    for entry in manifest["inputs"].as_array().unwrap() {
        let path = entry["path"].as_str().unwrap();
        let value = mainbcc_read(&root.join(path)).unwrap();
        assert_eq!(digest(&value), entry["main_sha256"]);
        let raw = std::fs::read(root.join(path)).unwrap();
        let raw = String::from_utf8(raw).unwrap();
        for changed in [raw.clone() + "\n", "!".to_owned() + &raw] {
            assert!(std::panic::catch_unwind(|| main34d_project_entry(entry, &changed)).is_err());
        }
        if entry["current_sha256"] != entry["main_sha256"] {
            assert!(std::panic::catch_unwind(|| main34d_project_entry(entry, &value)).is_err());
        }
    }
    assert!(std::panic::catch_unwind(|| mainbcc_entry(&manifest, "src/foreign.rs")).is_err());
    let mut duplicate = manifest.clone();
    let first = duplicate["inputs"][0].clone();
    duplicate["inputs"].as_array_mut().unwrap().push(first);
    let path = manifest["inputs"][0]["path"].as_str().unwrap();
    assert!(std::panic::catch_unwind(|| mainbcc_entry(&duplicate, path)).is_err());
}

// Semantic inverse owners pass complete, authenticated formatting AFTERimages.
// Keep raw-current acquisition at source_reflows::restore for its live owners;
// this namespace's archived-stage calls retain the full original formatter.
fn restore(record: &serde_json::Value, current: &str) -> String {
    super::restore_formatting(record, current)
}
