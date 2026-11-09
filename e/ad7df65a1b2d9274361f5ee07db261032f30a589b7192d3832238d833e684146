//! Fixed complete incoming Main44 metadata and exact source views; test authority only.
use sha2::{Digest, Sha256};
#[derive(Clone, Copy)]
pub(crate) enum Fixture {
    Compact7,
    Receivers12,
}
pub(super) struct Spec {
    pub(super) path: &'static str,
    pub(super) length: usize,
    pub(super) sha256: [u8; 32],
}
impl Fixture {
    pub(super) fn spec(self) -> Spec {
        match self {
            Self::Compact7 => Spec {
                path: concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/main44-independent-custody-20261016/compact-inverse7.json"
                ),
                length: 64689,
                sha256: [
                    153, 168, 166, 192, 186, 118, 148, 200, 38, 143, 84, 161, 14, 105, 173, 157,
                    112, 255, 173, 144, 219, 53, 74, 198, 11, 128, 210, 106, 69, 174, 111, 128,
                ],
            },
            Self::Receivers12 => Spec {
                path: concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/main44-independent-custody-20261016/receiver-inverse12.json"
                ),
                length: 91294,
                sha256: [
                    117, 72, 249, 231, 212, 41, 231, 239, 143, 178, 92, 66, 45, 155, 38, 20, 208,
                    136, 104, 26, 99, 66, 163, 225, 25, 87, 73, 56, 29, 75, 222, 18,
                ],
            },
        }
    }
}
pub(super) fn input_spec() -> Spec {
    // MAIN44_INPUT_SPEC
    Spec {
        path: concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/main44-independent-custody-20261016/current-to-main44.json"
        ),
        length: 2475967,
        sha256: [
            17, 166, 197, 8, 165, 188, 199, 145, 238, 70, 80, 56, 250, 161, 135, 87, 222, 41, 20,
            69, 106, 215, 193, 87, 226, 47, 233, 72, 68, 135, 177, 143,
        ],
    }
}
pub(super) fn input(source: &str, current: &str) -> String {
    const KEYS: &[&str] = &[
        "src/cleanup_plan/replay.rs",
        "src/cleanup_plan/replay/branch_clone/tests.rs",
        "src/cleanup_plan/replay/compact_stage/before.rs",
        "src/cleanup_plan/replay/compact_stage/lexical_before.rs",
        "src/cleanup_plan/replay/factored.rs",
        "src/cleanup_plan/replay/factored/borrowed_canonical/dictionary/keys.rs",
        "src/cleanup_plan/replay/factored/borrowed_canonical/tests.rs",
        "src/cleanup_plan/replay/factored/oracle_hook.rs",
        "src/cleanup_plan/replay/path_array.rs",
        "src/cleanup_plan/replay/path_array/before.rs",
        "src/cleanup_plan/replay/path_array/final_before.rs",
        "src/cleanup_plan/replay/path_array/sequence.rs",
        "src/cleanup_plan/replay/path_array/sequence/before.rs",
        "src/cleanup_plan/replay/path_array/sequence/tests.rs",
        "src/cleanup_plan/replay/path_array/tests.rs",
        "src/cleanup_plan/replay/sequence_prefix/before.rs",
        "src/cleanup_plan/replay/sequence_vector.rs",
        "src/cleanup_plan/replay/sequence_vector/before.rs",
        "src/cleanup_plan/replay/sequence_vector/tests.rs",
        "src/cleanup_plan/replay/skeleton_work.rs",
        "src/hir.rs",
        "src/hir/expr_nodes.rs",
        "src/hir/validation.rs",
        "src/source_verify/declaration.rs",
        "tests/hir_module_boundaries.rs",
        "tests/workspace/semantic_graph/source_inventory.rs",
    ];
    if KEYS.binary_search(&source).is_err() {
        return current.to_owned();
    }
    let metadata = super::runtime_json::main44_input();
    let acquired = current_entry(source, current);
    super::project(&metadata, source, &acquired)
}

// Only the original 26-key owner reaches this ingress. The fixed metadata
// authenticates first; every original full inverse and final hash runs after it.
fn current_entry(source: &str, supplied: &str) -> String {
    let compiled = crate::cleanup_plan::main44_current_source(source)
        .expect("original compiled Main44 source domain");
    let (archive, length, expected) = match source {
        "src/cleanup_plan/replay.rs" => ("docs/evidence/main44-current-entry-20261022/entries/807735eb8713ff6e02abd9ea6d3d4aea4fdc53f95bc6b6d1a2a1e51dba605afa.txt", 323567, "807735eb8713ff6e02abd9ea6d3d4aea4fdc53f95bc6b6d1a2a1e51dba605afa"),
        "src/cleanup_plan/replay/branch_clone/tests.rs" => ("docs/evidence/main44-current-entry-20261022/entries/46bf0451ef390225f540d031f326100c7f4288cf7e9d50e30ce7ddb68a448e0f.txt", 23912, "46bf0451ef390225f540d031f326100c7f4288cf7e9d50e30ce7ddb68a448e0f"),
        "src/cleanup_plan/replay/compact_stage/before.rs" => ("docs/evidence/main44-current-entry-20261022/entries/47fc6ffa22a2a47aab804971a0c483310cf506591c993fde481c712882d4c699.txt", 21938, "47fc6ffa22a2a47aab804971a0c483310cf506591c993fde481c712882d4c699"),
        "src/cleanup_plan/replay/compact_stage/lexical_before.rs" => ("docs/evidence/main44-current-entry-20261022/entries/2523a5f847df2d945fd324ea64908670b4c121f684f3274ca0b2de174ed7d523.txt", 960, "2523a5f847df2d945fd324ea64908670b4c121f684f3274ca0b2de174ed7d523"),
        "src/cleanup_plan/replay/factored.rs" => ("docs/evidence/main44-current-entry-20261022/entries/343eda3712d0c3b224742d72c34838a327e0c69449c8a94fd7eb81dc5e0134a6.txt", 44197, "343eda3712d0c3b224742d72c34838a327e0c69449c8a94fd7eb81dc5e0134a6"),
        "src/cleanup_plan/replay/factored/borrowed_canonical/dictionary/keys.rs" => ("docs/evidence/main44-current-entry-20261022/entries/01b3e408874930f2aad015d20cf8317fc1507183f92aae740d11985b99d223e1.txt", 8266, "01b3e408874930f2aad015d20cf8317fc1507183f92aae740d11985b99d223e1"),
        "src/cleanup_plan/replay/factored/borrowed_canonical/tests.rs" => ("docs/evidence/main44-current-entry-20261022/entries/a5f456bd1d6f7fc43c08baf8f192d4c81f6efb036b6d35d8a15f9d785eb06b46.txt", 15541, "a5f456bd1d6f7fc43c08baf8f192d4c81f6efb036b6d35d8a15f9d785eb06b46"),
        "src/cleanup_plan/replay/factored/oracle_hook.rs" => ("docs/evidence/main44-current-entry-20261022/entries/0126d25150ca4b96f11995293fc9043d23fc8fff2008485e9560abe5c85cb7ae.txt", 11716, "0126d25150ca4b96f11995293fc9043d23fc8fff2008485e9560abe5c85cb7ae"),
        "src/cleanup_plan/replay/path_array.rs" => ("docs/evidence/main44-current-entry-20261022/entries/adaca15c3ae85005ba22ad48351f41595ef30e6489d6a7e8d1b369c7e0f8bac3.txt", 7342, "adaca15c3ae85005ba22ad48351f41595ef30e6489d6a7e8d1b369c7e0f8bac3"),
        "src/cleanup_plan/replay/path_array/before.rs" => ("docs/evidence/main44-current-entry-20261022/entries/1e6f9b2f9d6252d0c24a70584d7ba67d203ff01d611ff381810c3fb8415dc7fb.txt", 5041, "1e6f9b2f9d6252d0c24a70584d7ba67d203ff01d611ff381810c3fb8415dc7fb"),
        "src/cleanup_plan/replay/path_array/final_before.rs" => ("docs/evidence/main44-current-entry-20261022/entries/e866a597b35940a2b37c8449c98ac85ef89472f48c1c21cfbd50be7be7c9709e.txt", 3894, "e866a597b35940a2b37c8449c98ac85ef89472f48c1c21cfbd50be7be7c9709e"),
        "src/cleanup_plan/replay/path_array/sequence.rs" => ("docs/evidence/main44-current-entry-20261022/entries/8f0da852b94f3e8ecd001a528131ffcab6ea2be287892dece6c6711765a30984.txt", 2991, "8f0da852b94f3e8ecd001a528131ffcab6ea2be287892dece6c6711765a30984"),
        "src/cleanup_plan/replay/path_array/sequence/before.rs" => ("docs/evidence/main44-current-entry-20261022/entries/82f9459f88d10e960903c8001c366c6b057e8818aa613c441d7f9e13e7678314.txt", 3490, "82f9459f88d10e960903c8001c366c6b057e8818aa613c441d7f9e13e7678314"),
        "src/cleanup_plan/replay/path_array/sequence/tests.rs" => ("docs/evidence/main44-current-entry-20261022/entries/53df021d2239f2754fd0c0c15fbb710d2402d8cf4c2ec4f8c193a63b5b2b5883.txt", 16543, "53df021d2239f2754fd0c0c15fbb710d2402d8cf4c2ec4f8c193a63b5b2b5883"),
        "src/cleanup_plan/replay/path_array/tests.rs" => ("docs/evidence/main44-current-entry-20261022/entries/b641a21d3e5ba8f517f79b3184f4bc3496fa32c74e3b36951877fd145b528619.txt", 14913, "b641a21d3e5ba8f517f79b3184f4bc3496fa32c74e3b36951877fd145b528619"),
        "src/cleanup_plan/replay/sequence_prefix/before.rs" => ("docs/evidence/main44-current-entry-20261022/entries/268d548459b7ef72cc260ece1570f1dffc3fbd320146f1941af103b18a87b7a8.txt", 2070, "268d548459b7ef72cc260ece1570f1dffc3fbd320146f1941af103b18a87b7a8"),
        "src/cleanup_plan/replay/sequence_vector.rs" => ("docs/evidence/main44-current-entry-20261022/entries/6129acfd3338cbd3178cf4440c1056762dbc7a666774aab7896a55e6aada7d6b.txt", 10668, "6129acfd3338cbd3178cf4440c1056762dbc7a666774aab7896a55e6aada7d6b"),
        "src/cleanup_plan/replay/sequence_vector/before.rs" => ("docs/evidence/main44-current-entry-20261022/entries/9585d39576779e49265c8a75536e8b772ba0e41749f1ed9cc61639c4890c2bd7.txt", 3019, "9585d39576779e49265c8a75536e8b772ba0e41749f1ed9cc61639c4890c2bd7"),
        "src/cleanup_plan/replay/sequence_vector/tests.rs" => ("docs/evidence/main44-current-entry-20261022/entries/9199f60a9fa34805f8de4ca6013dfa1931c668ac0fa2fdd3ff8f98a644694ffe.txt", 18626, "9199f60a9fa34805f8de4ca6013dfa1931c668ac0fa2fdd3ff8f98a644694ffe"),
        "src/cleanup_plan/replay/skeleton_work.rs" => ("docs/evidence/main44-current-entry-20261022/entries/2532173c152df743989e4afd5fc84ae4b3dd32bf45b0a72fe09039a832887c4b.txt", 47594, "2532173c152df743989e4afd5fc84ae4b3dd32bf45b0a72fe09039a832887c4b"),
        "src/hir.rs" => ("docs/evidence/main44-current-entry-20261022/entries/e3aaf1c870be500fa794ec24d393f8dc94fcd996990abf388222ce079f4e0b04.txt", 58967, "e3aaf1c870be500fa794ec24d393f8dc94fcd996990abf388222ce079f4e0b04"),
        "src/hir/expr_nodes.rs" => ("docs/evidence/main44-current-entry-20261022/entries/e2291267786bdec4371b969031209b4e77aa6876f4564048ad1c7561b004b73f.txt", 38066, "e2291267786bdec4371b969031209b4e77aa6876f4564048ad1c7561b004b73f"),
        "src/hir/validation.rs" => ("docs/evidence/main44-current-entry-20261022/entries/f89e882a3d8ed796b8b8b864247e4e7eb19631d6299cd0dd94ddcc6b5ca17eba.txt", 399117, "f89e882a3d8ed796b8b8b864247e4e7eb19631d6299cd0dd94ddcc6b5ca17eba"),
        "src/source_verify/declaration.rs" => ("docs/evidence/main44-current-entry-20261022/entries/54bca5bc3be89d2b788655f72cb8671c76b540edd99c126a0862a4f1f22a0a88.txt", 15793, "54bca5bc3be89d2b788655f72cb8671c76b540edd99c126a0862a4f1f22a0a88"),
        "tests/hir_module_boundaries.rs" => ("docs/evidence/main44-current-entry-20261022/entries/5090d08b52cb9650af5fc2c2a28739a9783c3d43c7a0f25037ce032b04f51137.txt", 98174, "5090d08b52cb9650af5fc2c2a28739a9783c3d43c7a0f25037ce032b04f51137"),
        "tests/workspace/semantic_graph/source_inventory.rs" => ("docs/evidence/main44-current-entry-20261022/entries/aa4e1844d2e867c519bd24fdb560535c9f07b7daa790e343c1b4b04e978d0989.txt", 91318, "aa4e1844d2e867c519bd24fdb560535c9f07b7daa790e343c1b4b04e978d0989"),
        _ => unreachable!("original Main44 key checked before acquisition"),
    };
    assert!(
        supplied == compiled
            || (supplied.len() == length && super::source_sha(supplied) == expected),
        "compiled current or exact sealed Main44 entry: {source}"
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let current_hash = super::source_sha(compiled);
    let physical_source = match source {
        "src/cleanup_plan/replay/sequence_vector/before.rs" => {
            "src/cleanup_plan/replay/path_array/main44/vector_before.rs"
        }
        "src/cleanup_plan/replay/sequence_vector/tests.rs" => {
            "src/cleanup_plan/replay/path_array/main44/vector_tests.rs"
        }
        _ => source,
    };
    let physical = current_file(&root.join(physical_source), compiled.len(), &current_hash)
        .expect("complete no-follow current Main44 owner");
    assert_eq!(
        physical, compiled,
        "physical/compiled Main44 owner: {source}"
    );
    current_file(&root.join(archive), length, expected).expect("complete sealed Main44 entry")
}

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
