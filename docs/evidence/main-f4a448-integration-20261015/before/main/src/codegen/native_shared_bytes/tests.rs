use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::bridge::{bridge_c_for_tests, Outcome};
use super::closure::{
    closed_over_for_tests, select, Admission, Consumer, Support, CARRIER_SITES,
    PARTITION_STATUS_CODES,
};
use super::RUNTIME;

pub(super) fn scratch(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "shared-owner-bytes-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    root
}

pub(super) fn compile_and_run(root: &Path, source: &str, extra: &[&str]) {
    let path = root.join("probe.c");
    std::fs::write(&path, source).unwrap();
    for optimization in ["-O0", "-O2"] {
        let binary = root.join(format!(
            "probe{optimization}{}",
            std::env::consts::EXE_SUFFIX
        ));
        let compiled = Command::new(std::env::var_os("CLANG").unwrap_or_else(|| "clang".into()))
            .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
            .args(extra)
            .arg(&path)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let run = Command::new(&binary).output().unwrap();
        assert!(
            run.status.success(),
            "{optimization} status {:?}: {}",
            run.status.code(),
            String::from_utf8_lossy(&run.stderr)
        );
    }
}

#[test]
fn shared_owner_leases_hostiles_quotas_and_settlement_at_o0_o2() {
    let root = scratch("probe");
    let source = format!(
        "{}\n{}\n{}",
        include_str!("probe_prefix.c"),
        RUNTIME,
        include_str!("probe.c")
    );
    compile_and_run(&root, &source, &[]);
    std::fs::remove_dir_all(root).unwrap();
}

/// The bridge composes with the real status runtime and the frozen default
/// byte runtime: the explicit registry reference, existing status domains,
/// attach/detach settlement and no interference with `spx_bytes_v1`.
#[test]
fn bridge_maps_existing_domains_against_the_emitted_context() {
    let mut status = String::new();
    crate::codegen::native_runtime::emit_status_runtime(&mut status);
    let status = super::extend_context(status);
    let mut bytes = String::new();
    crate::codegen::native_byte_data::emit_runtime(&mut bytes);
    let selection = closed_over_for_tests([
        Consumer::Zeroed,
        Consumer::Set,
        Consumer::Drop,
        Consumer::VecWithCapacity,
        Consumer::VecDrop,
    ])
    .unwrap();
    let mut selected = String::new();
    super::emit_selected_runtime(&mut selected, &selection);
    let source = format!(
        "{status}\n#include <stdlib.h>\n\
         static __attribute__((noreturn)) void spx_runtime_invariant_failure(const char *message) {{ (void)message; exit(86); }}\n\
         {bytes}\n{selected}\n{}",
        r#"
static spx_shb_issuer issuer;
static spx_shb_registry registry;
int main(void) {
    struct spx_context context = {0};
    struct spx_status_entry entries[4];
    memset(entries, 0, sizeof(entries));
    if (!spx_context_init(&context, UINT64_C(7), entries, 4u, NULL, NULL, NULL)) return 1;
    if (context.shared_bytes != NULL) return 2;
    spx_shb_issuer_init(&issuer);
    spx_shb_attach(&context, &registry, &issuer);
    spx_shb_bytes owner;
    spx_shb_zeroed(spx_shb_held(&context), 2u, &owner);
    spx_status_token token = spx_shb_status(&context, spx_shb_set_check(spx_shb_held(&context), &owner, 2u));
    if (token == SPX_STATUS_SUCCESS || context.status_arena.length != 1u) return 3;
    if (entries[0].status.code != 1u || strcmp(entries[0].status.domain_id, "semaprax.byte-buffer.v1") != 0) return 4;
    spx_shb_vec vec;
    token = spx_shb_status(&context, spx_shb_vec_with_capacity(spx_shb_held(&context), 8193u, &vec));
    if (token == SPX_STATUS_SUCCESS || entries[1].status.code != 3u
        || strcmp(entries[1].status.domain_id, "semaprax.vec.v1") != 0) return 5;
    if (spx_shb_status(&context, SPX_SHB_OK) != SPX_STATUS_SUCCESS) return 6;
    spx_shb_drop(spx_shb_held(&context), &owner);
    spx_shb_detach(&context);
    if (context.shared_bytes != NULL || !registry.finished) return 7;
    spx_bytes_v1 legacy = spx_bytes_zeroed(1u);
    spx_bytes_drop(&legacy);
    return 0;
}
"#
    );
    let root = scratch("bridge");
    compile_and_run(&root, &source, &["-Wno-unused-function"]);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn bridge_assigns_only_existing_statuses() {
    let bridge = bridge_c_for_tests();
    for outcome in Outcome::ALL {
        let case = format!("case {}:", outcome.c_name());
        let assigned = outcome == Outcome::Ok || outcome.status().is_some();
        assert_eq!(bridge.contains(&case), assigned, "{outcome:?}");
    }
    assert_eq!(
        PARTITION_STATUS_CODES,
        Some(crate::construction_fusion::partition::operation::STATUS_CODES)
    );
    for code in crate::construction_fusion::partition::operation::STATUS_CODES {
        assert!(bridge.contains(&format!(
            "\"semaprax.byte-partition.v1\"; code = UINT32_C({code})"
        )));
    }
    assert!(bridge.contains("\"semaprax.byte-buffer.v1\"; code = UINT32_C(1)"));
    assert!(bridge.contains("\"semaprax.byte-buffer.v1\"; code = UINT32_C(2)"));
    assert!(bridge.contains("\"semaprax.vec.v1\"; code = UINT32_C(3)"));
    // The C enum order is the Rust discriminant order.
    let mut cursor = 0;
    for (index, outcome) in Outcome::ALL.into_iter().enumerate() {
        assert_eq!(outcome as u32 as usize, index);
        let at = RUNTIME[cursor..]
            .find(outcome.c_name())
            .unwrap_or_else(|| panic!("{outcome:?} missing from the runtime enum"));
        cursor += at + outcome.c_name().len();
    }
}

#[test]
fn context_extension_is_exact_and_rejects_drift() {
    let mut status = String::new();
    crate::codegen::native_runtime::emit_status_runtime(&mut status);
    let extended = super::extend_context(status.clone());
    assert!(extended.contains("    struct spx_shb_registry *shared_bytes;"));
    assert!(extended.contains("context->shared_bytes == NULL"));
    assert!(extended.contains("context->shared_bytes = NULL;"));
    let drifted = status.replace("    uint32_t call_depth;", "    uint32_t depth;");
    assert!(std::panic::catch_unwind(|| super::extend_context(drifted)).is_err());
}

#[test]
fn admission_stays_closed_even_for_a_complete_closure() {
    assert_eq!(
        Consumer::from_byte_op(crate::byte_ops::ByteOp::Partition),
        Consumer::Partition
    );
    let complete = [
        Consumer::Zeroed,
        Consumer::Partition,
        Consumer::Drop,
        Consumer::VecPop,
        Consumer::IterNext,
    ];
    assert!(closed_over_for_tests(complete).is_ok());
    let refusal = select(complete, Admission::ClosedUntilCrossBackendJoin).unwrap_err();
    assert!(format!("{refusal:?}").contains("admission stays closed"));
    let empty = select([], Admission::ClosedUntilCrossBackendJoin).unwrap_err();
    assert!(format!("{empty:?}").contains("no reachable Bytes owner"));
}

#[test]
fn unsupported_consumers_name_the_exact_blocker_before_admission() {
    for (consumer, fragment) in [
        (Consumer::PublicCopyOut, "public bridge"),
        (Consumer::ProviderDrop, "public bridge"),
        (Consumer::OwnedLeafVec, "16-byte"),
        (Consumer::SelectedFusedRow, "separate private domain"),
        (Consumer::ReservedArena, "distinct private profile"),
        (Consumer::CheckedAllocation, "own release provenance"),
        (Consumer::ImportedCall, "imported and foreign"),
        (Consumer::HostIo, "malloc-backed"),
    ] {
        let refusal = select(
            [Consumer::Zeroed, consumer],
            Admission::ClosedUntilCrossBackendJoin,
        )
        .unwrap_err();
        let text = format!("{refusal:?}");
        assert!(text.contains(&format!("{consumer:?}")), "{text}");
        assert!(text.contains(fragment), "{text}");
        assert!(!text.contains("admission stays closed"), "{text}");
    }
}

#[test]
fn every_byte_and_bytes_vec_operation_is_classified() {
    for op in crate::byte_ops::ByteOp::ALL {
        let consumer = Consumer::from_byte_op(op);
        assert!(
            matches!(consumer.support(), Support::Shared(_)),
            "{op:?} lost shared support"
        );
    }
    let mut refused = BTreeSet::new();
    for op in crate::vec_ops::ALL {
        if let Support::Refused(_) = Consumer::from_bytes_vec_op(op).support() {
            refused.insert(format!("{op:?}"));
        }
    }
    let expected: BTreeSet<String> = [
        "Sort",
        "Replace",
        "ReserveOwned",
        "SortOwned",
        "SortBy",
        "StrAt",
        "UniqueBy",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(refused, expected);
}

/// Drift gate: every production source naming the ordinary carrier or its
/// drop is classified. Tests, this module and Wasm are excluded by rule only.
#[test]
fn carrier_site_inventory_matches_the_source_tree() {
    fn walk(directory: &Path, found: &mut BTreeSet<String>) {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, found);
                continue;
            }
            let relative = path
                .strip_prefix(env!("CARGO_MANIFEST_DIR"))
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let source_like = relative.ends_with(".rs") || relative.ends_with(".c");
            let excluded = relative.ends_with("tests.rs")
                || relative.contains("/tests/")
                || relative.contains("_tests.rs")
                || relative.starts_with("src/wasm/")
                || relative.starts_with("src/codegen/native_shared_bytes");
            if !source_like || excluded {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            if text.contains("spx_bytes_v1") || text.contains("spx_bytes_drop(") {
                found.insert(relative);
            }
        }
    }
    let mut found = BTreeSet::new();
    walk(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut found,
    );
    let listed: BTreeSet<String> = CARRIER_SITES
        .iter()
        .map(|(path, _)| (*path).to_owned())
        .collect();
    assert_eq!(found, listed);
    for (path, consumers) in CARRIER_SITES {
        assert!(!consumers.is_empty(), "{path} has no classified family");
    }
}

/// Structural invariants of the selected runtime text.
#[test]
fn runtime_never_frees_interior_pointers_or_probes_headers() {
    let code: String = RUNTIME
        .lines()
        .filter(|line| !line.trim_start().starts_with("#define"))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in [
        "free(",
        "malloc(",
        "calloc(",
        "realloc(",
        "spx_bytes_drop",
        "offsetof",
        "target_state",
        ") - 1)",
        "- sizeof",
    ] {
        // Allocation hooks are uppercase macros; raw calls appear only in
        // their `#define` defaults, which are filtered above.
        assert!(!code.contains(forbidden), "{forbidden} in selected runtime");
    }
    // Payload is released only from the backing record, in one place.
    assert_eq!(code.matches("SPX_SHB_PAYLOAD_FREE(").count(), 1);
    assert!(code.contains("if (payload != NULL) SPX_SHB_PAYLOAD_FREE(payload);"));
    // Provider memory is fresh and released only by provider drop.
    assert_eq!(code.matches("SPX_SHB_PROVIDER_FREE(").count(), 1);
    // The existing limits, unchanged.
    for limit in [
        "#define SPX_SHB_EVENTS 32u",
        "#define SPX_SHB_CUMULATIVE UINT64_C(2097152)",
        "#define SPX_SHB_EXTENT UINT64_C(131072)",
        "#define SPX_SHB_EXTERNAL_ROOT UINT64_C(65536)",
        "#define SPX_SHB_VEC_CAPACITY UINT64_C(8192)",
    ] {
        assert!(RUNTIME.contains(limit), "{limit}");
    }
    assert_eq!(
        crate::byte_data_capacity::MAX_BYTES_COPY_SITES,
        32,
        "backing-event quota"
    );
    assert_eq!(
        crate::byte_data_capacity::MAX_OWNED_BYTE_PAYLOAD_BYTES,
        2_097_152
    );
    assert_eq!(crate::byte_ops::MAX_OWNED_BYTE_VALUE_BYTES, 131_072);
    assert_eq!(crate::byte_ops::MAX_EXTERNAL_ROOT_BYTES, 65_536);
    assert_eq!(crate::vec_ops::MAX_CAPACITY, 8_192);
    // Stored rows keep the existing Vec<Bytes> element budget.
    assert_eq!(crate::vec_ops::OWNED_PAYLOAD_BYTES_PER_ELEMENT, 16);
    assert!(RUNTIME.contains("sizeof(spx_shb_stored) == 16"));
}

/// The ordinary default projection is untouched by this module's presence.
#[test]
fn default_byte_runtime_still_frees_only_its_own_whole_allocations() {
    let mut emitted = String::new();
    crate::codegen::native_byte_data::emit_runtime(&mut emitted);
    assert!(!emitted.contains("spx_shb_"));
    assert!(!RUNTIME.contains("spx_bytes_v1 *ptr"));
}
