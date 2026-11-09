//! Whole-current input views used only by two historical Main8a metadata readers.
//! Live source inventories continue to read complete raw sources directly.
use sha2::{Digest, Sha256};

#[derive(Clone, Copy)]
pub(crate) enum Domain {
    Compact7,
    Receivers12,
}
#[derive(Clone, Copy)]
struct Spec {
    fixture: super::runtime_json::AdditionalFixture,
    length: usize,
    sha256: [u8; 32],
    rows: usize,
}
impl Domain {
    fn spec(self) -> Spec {
        match self {
            // SPECS_BEGIN
            Self::Compact7 => Spec {
                fixture: super::runtime_json::AdditionalFixture::HistoricalCompact7,
                length: 1084990,
                sha256: [
                    0, 149, 173, 86, 200, 253, 74, 28, 73, 247, 87, 156, 250, 159, 42, 179, 31, 58,
                    58, 117, 45, 255, 18, 239, 111, 255, 168, 184, 62, 65, 150, 117,
                ],
                rows: 7,
            },
            Self::Receivers12 => Spec {
                fixture: super::runtime_json::AdditionalFixture::HistoricalReceivers12,
                length: 2125481,
                sha256: [
                    223, 155, 190, 240, 51, 198, 246, 133, 193, 29, 115, 33, 18, 160, 218, 247,
                    198, 191, 36, 101, 22, 191, 240, 124, 254, 242, 172, 36, 136, 208, 145, 167,
                ],
                rows: 12,
            },
            // SPECS_END
        }
    }
}
fn checked(spec: Spec, data: &str, path: &str, current: &str) -> String {
    assert_eq!(
        data.len(),
        spec.length,
        "complete historical input metadata"
    );
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(data.as_bytes())),
        spec.sha256
    );
    let rows: serde_json::Value = serde_json::from_str(data).unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), spec.rows);
    assert_eq!(
        rows.iter().filter(|r| r["source"] == path).count(),
        1,
        "registered historical input domain"
    );
    // This owner admits actual current bytes only. The general historical
    // projector intentionally accepts prior views; those are stale inputs here.
    // Keep the original domain/metadata checks above and authenticate the live
    // physical owner before selecting any historical projection below.
    let compiled = tests::current(path);
    let expected = format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(compiled.as_bytes()))
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let physical = current_file(&root.join(path), compiled.len(), &expected)
        .expect("complete no-follow current Main8a owner");
    assert!(
        physical == compiled,
        "physical/compiled current Main8a owner: {path}"
    );
    assert!(current == compiled, "strict current Main8a input: {path}");
    super::project(data, path, current)
}
pub(crate) fn input(domain: Domain, path: &str, current: &str) -> String {
    let spec = domain.spec();
    let data = super::runtime_json::additional(spec.fixture);
    checked(spec, &data, path, current)
}

// Same bounded no-follow descriptor/identity protocol as the existing raw
// historical reader. The caller supplies a sealed compiled source's exact
// length/hash, after the original fixed-domain metadata has authenticated.
fn same_file(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    if !a.is_file() || !b.is_file() || a.len() != b.len() || a.modified().ok() != b.modified().ok()
    {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if a.dev() != b.dev()
            || a.ino() != b.ino()
            || a.mode() != b.mode()
            || a.mtime() != b.mtime()
            || a.mtime_nsec() != b.mtime_nsec()
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
fn current_file(
    path: &std::path::Path,
    length: usize,
    expected: &str,
) -> Result<String, &'static str> {
    use std::io::Read;
    if length > 1_048_576 {
        return Err("bound");
    }
    let before = std::fs::symlink_metadata(path).map_err(|_| "missing")?;
    if before.file_type().is_symlink() || !before.is_file() {
        return Err("nonregular");
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
    let opened = file.metadata().map_err(|_| "metadata")?;
    if !same_file(&before, &opened) || opened.len() != u64::try_from(length).map_err(|_| "bound")? {
        return Err("length-or-identity");
    }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).map_err(|_| "allocation")?;
    let mut limited = (&mut file).take(length as u64 + 1);
    limited.read_to_end(&mut bytes).map_err(|_| "read")?;
    if bytes.len() != length
        || format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(&bytes))) != expected
    {
        return Err("length-or-digest");
    }
    let after = file.metadata().map_err(|_| "metadata")?;
    let path_after = std::fs::symlink_metadata(path).map_err(|_| "metadata")?;
    if path_after.file_type().is_symlink()
        || !same_file(&opened, &after)
        || !same_file(&after, &path_after)
    {
        return Err("identity-drift");
    }
    String::from_utf8(bytes).map_err(|_| "utf8")
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(super) fn current(path: &str) -> &'static str {
        match path {
            // INPUTS_BEGIN
            "src/cleanup_plan/replay.rs" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/cleanup_plan/replay.rs"
            )),
            "src/cleanup_plan/replay/branch_clone/tests.rs" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/cleanup_plan/replay/branch_clone/tests.rs"
            )),
            "src/cleanup_plan/replay/factored.rs" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/cleanup_plan/replay/factored.rs"
            )),
            "src/cleanup_plan/replay/factored/borrowed_canonical/dictionary/keys.rs" => {
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/cleanup_plan/replay/factored/borrowed_canonical/dictionary/keys.rs"
                ))
            }
            "src/cleanup_plan/replay/factored/borrowed_canonical/tests.rs" => {
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/cleanup_plan/replay/factored/borrowed_canonical/tests.rs"
                ))
            }
            "src/cleanup_plan/replay/factored/oracle_hook.rs" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/cleanup_plan/replay/factored/oracle_hook.rs"
            )),
            "src/hir.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir.rs")),
            "src/hir/expr_nodes.rs" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/hir/expr_nodes.rs"
            )),
            "src/hir/validation.rs" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/hir/validation.rs"
            )),
            "src/source_verify/declaration.rs" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/source_verify/declaration.rs"
            )),
            "tests/hir_module_boundaries.rs" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/hir_module_boundaries.rs"
            )),
            "tests/workspace/semantic_graph/source_inventory.rs" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/workspace/semantic_graph/source_inventory.rs"
            )),
            // INPUTS_END
            _ => panic!("unregistered complete test input"),
        }
    }
    #[test]
    fn complete_historical_domains_refuse_changed_truncated_foreign_and_stale_inputs() {
        for domain in [Domain::Compact7, Domain::Receivers12] {
            let rows: serde_json::Value = serde_json::from_str(
                &super::super::runtime_json::additional(domain.spec().fixture),
            )
            .unwrap();
            for row in rows.as_array().unwrap() {
                let path = row["source"].as_str().unwrap();
                let raw = current(path);
                let historical = input(domain, path, raw);
                let changed = format!("{raw} ");
                let truncated = raw.get(..raw.len() / 2).unwrap_or("");
                for wrong in [changed.as_str(), "", truncated] {
                    assert!(std::panic::catch_unwind(|| input(domain, path, wrong)).is_err());
                }
                if historical != raw {
                    assert!(std::panic::catch_unwind(|| input(domain, path, &historical)).is_err());
                }
            }
            assert!(
                std::panic::catch_unwind(|| input(domain, "src/foreign.rs", "foreign")).is_err()
            );
            let data = super::super::runtime_json::additional(domain.spec().fixture);
            assert!(std::panic::catch_unwind(|| checked(
                domain.spec(),
                &data[..data.len() - 1],
                "src/foreign.rs",
                ""
            ))
            .is_err());
            let changed = data.replacen('s', "t", 1);
            assert!(std::panic::catch_unwind(|| checked(
                domain.spec(),
                &changed,
                "src/foreign.rs",
                ""
            ))
            .is_err());
        }
    }
}
