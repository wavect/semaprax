#[path = "historical_inputs.rs"]
mod historical_inputs;
use super::*;
use std::io::{Cursor, Error, ErrorKind};
type ReceiverTextEdits<'a> = (usize, [u8; 32], usize, [u8; 32], &'a [(&'a str, &'a str)]);
type ReceiverSpanEdits<'a> = (
    usize,
    [u8; 32],
    usize,
    [u8; 32],
    &'a [(usize, usize, &'a str, &'a str)],
);
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Drop for Directory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn authenticate_keys(fixture: Fixture, rows: &serde_json::Value) {
    let mut actual = rows
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|row| row["source"].as_str())
        .collect::<Vec<_>>();
    actual.sort();
    // Keep duplicates: presence selects the unchanged projector, which still
    // refuses more than one matching row. Never silently deduplicate evidence.
    assert_eq!(actual.as_slice(), membership::keys(fixture));
    for key in &actual {
        assert!(membership::keys(fixture).binary_search(key).is_ok());
    }
    assert!(membership::keys(fixture).binary_search(&"\0").is_err());
    assert_eq!(project(fixture, "\0", "unchanged"), "unchanged");
}
#[test]
fn fixed_projection_json_transport_refuses_hostile_io_before_parsing() {
    let shared = include_str!("sealed_io.rs");
    assert_eq!(
        super::super::main_e177_final_inputs::value_direct(
            "src/source_reflows_merge/runtime_json/sealed_io.rs",
            "bef85d126a5ce9c72f6f74835d9966d60d7b9bff6622302d4df864adf1701fd0",
            shared,
        ),
        shared,
        "complete shared sealed reader before all original I/O hostiles",
    );
    // Both exact selector arms normalize to the identical common source before
    // any historical source inverse. No arbitrary edited source is accepted.
    let authenticated = super::super::main_e177_final_inputs::value_direct(
        "src/source_reflows_merge/runtime_json.rs",
        "5a95fa1f07fac99c62f6572817df247695a4d2e274d5accc5ce231d7dc150bc4",
        include_str!("../runtime_json.rs"),
    );
    let authenticated = complete_preformat_transport(&authenticated);
    let current = super::super::release_current_input(
        "src/source_reflows_merge/runtime_json.rs",
        &authenticated,
    );
    let release = super::super::release_custody::input(
        super::super::release_custody::Fixture::Previous2c,
        "src/source_reflows_merge/runtime_json.rs",
        &current,
    );
    let joined =
        super::super::joined_custody::input("src/source_reflows_merge/runtime_json.rs", &release);
    let prefix_view = super::super::prefix_custody::input(
        super::super::prefix_custody::Fixture::SealedB5,
        "src/source_reflows_merge/runtime_json.rs",
        &joined,
    );
    let successor = super::super::successor_custody::input(
        super::super::successor_custody::Fixture::Sealed724,
        "src/source_reflows_merge/runtime_json.rs",
        &prefix_view,
    );
    let final_view = super::super::final_custody::input(
        super::super::final_custody::Fixture::Common,
        "src/source_reflows_merge/runtime_json.rs",
        &successor,
    );
    let arm_view = super::super::metadata_json::previous(
        "src/source_reflows_merge/runtime_json.rs",
        &final_view,
    );
    let arm = arm_view.as_str();
    let common = pair_custody::normalize("src/source_reflows_merge/runtime_json.rs", arm);
    let embedded = common.replace(
        "runtime_json/transport_runtime.rs",
        "runtime_json/transport_embedded.rs",
    );
    assert_eq!(
        pair_custody::normalize("src/source_reflows_merge/runtime_json.rs", &embedded),
        common
    );
    assert!(std::panic::catch_unwind(|| {
        pair_custody::normalize(
            "src/source_reflows_merge/runtime_json.rs",
            &(common.clone() + " "),
        )
    })
    .is_err());
    let path = std::env::temp_dir().join(format!(
        "semaprax-projection-json-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    // Acquire cleanup ownership only after successful creation. Never remove a
    // pre-existing directory when creation itself fails.
    std::fs::create_dir(&path).unwrap();
    let directory = Directory(path);
    let path = directory.0.join("fixture.json");
    let bytes = b"[{\"source\":\"exact\"}]";
    let sha = digest(bytes);
    std::fs::write(&path, bytes).unwrap();
    assert_eq!(
        read_fixture(&path, bytes.len(), sha).unwrap().as_bytes(),
        bytes
    );
    assert_eq!(
        read_body(&mut Cursor::new(bytes), bytes.len(), sha)
            .unwrap()
            .as_bytes(),
        bytes
    );
    // Check every original allowlisted repository fixture without retaining a
    // cache or multiple payloads. A missing/drifted fixture must fail this owner.
    let custody = load(Fixture::Custody);
    assert_eq!(selected_transport::load(Fixture::Custody), custody);
    let custody: serde_json::Value = serde_json::from_str(&custody).unwrap();
    assert_eq!(custody.as_array().unwrap().len(), 6);
    authenticate_keys(Fixture::Custody, &custody);
    for fixture in inventory::ORIGINAL_FIXTURES {
        let payload = load(fixture);
        assert_eq!(selected_transport::load(fixture), payload);
        let rows: serde_json::Value = serde_json::from_str(&payload).unwrap();
        authenticate_keys(fixture, &rows);
    }
    // New bindings traverse the unchanged reader before their owning JSON/source
    // assertions. Every original fixture and all hostile-I/O controls stay above
    // and below this loop unchanged.
    for (fixture, rows) in [
        (AdditionalFixture::HistoricalCompact7, 7),
        (AdditionalFixture::HistoricalReceivers12, 12),
        (AdditionalFixture::OrdinaryPlanHistory, 6),
        (AdditionalFixture::OrdinaryPlanIntegrated, 6),
    ] {
        let spec = fixture.spec();
        let payload = additional(fixture);
        assert_eq!(payload.len(), spec.length);
        assert_eq!(digest(payload.as_bytes()), spec.sha256);
        let parsed: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(parsed.as_array().unwrap().len(), rows);
    }
    // Compiled presence must reach the original duplicate-row refusal, never
    // choose one row or turn a duplicate into an unchanged-input success.
    let duplicate = r#"[{"source":"same"},{"source":"same"}]"#;
    assert!(
        std::panic::catch_unwind(|| { super::super::project(duplicate, "same", "unchanged") })
            .is_err()
    );
    // Ten distinct hostile boundaries. None hands unauthenticated data to serde.
    assert_eq!(
        read_fixture(&directory.0.join("missing"), bytes.len(), sha),
        Err(Failure::Input)
    );
    assert!(matches!(
        read_fixture(&directory.0, bytes.len(), sha),
        Err(Failure::Input | Failure::Bound)
    ));
    assert_eq!(
        read_fixture(&path, OUTPUT_CAP + 1, sha),
        Err(Failure::Bound)
    );
    assert_eq!(
        read_fixture(&path, bytes.len() + 1, sha),
        Err(Failure::Bound)
    );
    assert_eq!(
        read_fixture(&path, bytes.len() - 1, sha),
        Err(Failure::Bound)
    );
    let mut altered = bytes.to_vec();
    altered[0] ^= 1;
    std::fs::write(&path, &altered).unwrap();
    assert_eq!(read_fixture(&path, bytes.len(), sha), Err(Failure::Digest));
    let invalid = [0xff_u8];
    std::fs::write(&path, invalid).unwrap();
    assert_eq!(
        read_fixture(&path, invalid.len(), digest(&invalid)),
        Err(Failure::Utf8)
    );
    assert_eq!(
        read_body(
            &mut Cursor::new(&bytes[..bytes.len() - 1]),
            bytes.len(),
            sha
        ),
        Err(Failure::Read)
    );
    let mut trailing = bytes.to_vec();
    trailing.push(0);
    assert_eq!(
        read_body(&mut Cursor::new(trailing), bytes.len(), sha),
        Err(Failure::Trailing)
    );
    struct RefuseEof(Cursor<&'static [u8]>);
    impl Read for RefuseEof {
        #[allow(
            clippy::io_other_error,
            reason = "The complete saved control retains its original expressions for independent parity checks."
        )]
        fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
            if self.0.position() as usize == self.0.get_ref().len() {
                Err(Error::new(ErrorKind::Other, "refused EOF check"))
            } else {
                self.0.read(output)
            }
        }
    }
    assert_eq!(
        read_body(&mut RefuseEof(Cursor::new(bytes)), bytes.len(), sha),
        Err(Failure::Read)
    );
}

#[test]
fn maina3_parts_cover_every_fixed_key_once_under_the_original_reader_cap() {
    use super::super::maina3_custody::Input;
    for fixture in [
        Input::Previous16c,
        Input::Maina3,
        Input::Alias1ae,
        Input::MainE47,
    ] {
        let mut opened = std::collections::BTreeSet::new();
        let mut complete = Vec::new();
        for source in fixture.keys() {
            let spec = fixture.spec(source);
            assert!(spec.length <= OUTPUT_CAP);
            if !opened.insert(spec.path) {
                continue;
            }
            let text = maina3_input(fixture, source);
            assert_eq!(text.len(), spec.length);
            let rows: serde_json::Value = serde_json::from_str(&text).unwrap();
            for row in rows.as_array().unwrap() {
                let key = row["source"].as_str().unwrap();
                assert!(fixture.keys().binary_search(&key).is_ok());
                assert_eq!(fixture.spec(key).path, spec.path);
                complete.push(key.to_owned());
            }
        }
        complete.sort();
        assert_eq!(complete, fixture.keys());
    }
}

#[test]
fn maina3_known_part_source_refuses_gaps_duplicates_and_foreign_rows() {
    for payload in [
        "[]",
        r#"[{"source":"foreign"}]"#,
        r#"[{"source":"known"},{"source":"known"}]"#,
        "{}",
        "not json",
    ] {
        assert!(std::panic::catch_unwind(|| maina3_part_source(payload, "known")).is_err());
    }
    maina3_part_source(r#"[{"source":"known"}]"#, "known");
}

// RUNTIME848_RECEIVER_CONTROLS_BEGIN
#[test]
fn fixed_receiver_json_fixtures_keep_complete_inputs_inverses_and_hostile_io() {
    let directory = std::env::temp_dir().join(format!(
        "semaprax-receiver-json-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&directory).unwrap();
    let directory = Directory(directory);
    let path = directory.0.join("fixture.json");
    for (fixture, expected_rows) in [
        (ReceiverFixture::SelectedCalls, 18),
        (ReceiverFixture::Main67107, 12),
    ] {
        let spec = fixture.spec();
        assert!(spec.length <= OUTPUT_CAP);
        let text = receiver_fixture(fixture);
        assert_eq!(text.len(), spec.length);
        assert_eq!(digest(text.as_bytes()), spec.sha256);
        let rows: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(rows.as_array().unwrap().len(), expected_rows);
        std::fs::write(&path, text.as_bytes()).unwrap();
        assert_eq!(read_fixture(&path, spec.length, spec.sha256).unwrap(), text);
        assert_eq!(
            read_fixture(&directory.0.join("missing"), spec.length, spec.sha256),
            Err(Failure::Input)
        );
        std::fs::write(&path, &text.as_bytes()[..spec.length - 1]).unwrap();
        assert_eq!(
            read_fixture(&path, spec.length, spec.sha256),
            Err(Failure::Bound)
        );
        let mut extra = text.as_bytes().to_vec();
        extra.push(0);
        std::fs::write(&path, &extra).unwrap();
        assert_eq!(
            read_fixture(&path, spec.length, spec.sha256),
            Err(Failure::Bound)
        );
        assert_eq!(
            read_body(&mut Cursor::new(&extra), spec.length, spec.sha256),
            Err(Failure::Trailing)
        );
        extra.truncate(spec.length);
        extra[0] ^= 1;
        std::fs::write(&path, &extra).unwrap();
        assert_eq!(
            read_fixture(&path, spec.length, spec.sha256),
            Err(Failure::Digest)
        );
    }
    let name = "src/cleanup_plan/replay/structural_control/calls/custody_tests/source.rs";
    let current =
        include_str!("../../cleanup_plan/replay/structural_control/calls/custody_tests/source.rs");
    let original = receiver_fixture_source_before(name, current);
    assert!(std::panic::catch_unwind(|| receiver_fixture_source_before(name, &original)).is_err());
    assert!(std::panic::catch_unwind(|| receiver_fixture_source_before(
        name,
        &(current.to_owned() + " ")
    ))
    .is_err());
    let runtime = include_str!("../runtime_json.rs");
    let runtime = super::super::main_e177_final_inputs::value_direct(
        "src/source_reflows_merge/runtime_json.rs",
        "d53fe461e231d8bfa061cf8b1fe31e709b32c2fe0fa2ea4a79c194bf08625e43",
        runtime,
    );
    assert_eq!(runtime.len(), 9945);
    let (runtime_before, _) = runtime
        .split_once("\n// RUNTIME848_FIXED_RECEIVERS_BEGIN\n")
        .unwrap();
    assert_eq!(
        digest(runtime_before.as_bytes()),
        *b"\x94\x70\xc1\x19\xd0\x32\x31\x1e\x9b\x96\x67\xc6\x42\xd8\x47\x52\x81\x14\xb4\xa1\xad\x24\x69\xed\xe1\x7d\x31\x13\x4d\x95\x9b\xec"
    );
    let mut root_before = complete_receiver_entry_glue(
        "src/source_reflows_merge.rs",
        include_str!("../../source_reflows_merge.rs"),
    );
    assert_eq!(&root_before[685..867], ";\n// Closed authenticated repository DATA only; no live-source or compiler authority.\npub(crate) use runtime_json::{receiver_fixture, receiver_fixture_source_before, ReceiverFixture}");
    root_before.replace_range(685..867, "");
    assert_eq!(
        digest(root_before.as_bytes()),
        *b"\x2f\xc7\x51\x6b\xf1\x08\xa3\x53\x87\xc3\xa4\xe3\x2c\x7e\x15\xd2\x66\x5a\x15\x3c\x9a\xed\x9a\xea\x67\xcb\x72\xae\xda\x84\x70\xe1"
    );
    let mut receiver_before = complete_receiver_entry_glue(
        "src/source_reflows/incoming67107_semantic_receivers.rs",
        include_str!("../../source_reflows/incoming67107_semantic_receivers.rs"),
    );
    assert_eq!(receiver_before.matches("fn proofs() -> &'static [serde_json::Value] {\n    // Preserve the old parsed metadata cache, but authenticate the runtime\n    // dependency on every reached lookup, including an already warm cache.\n    let payload = crate::source_reflows_merge::receiver_fixture(\n        crate::source_reflows_merge::ReceiverFixture::Main67107,\n    );\n    PROOF_RECORDS.get_or_init(|| {\n        let mut original: Vec<serde_json::Value> = serde_json::from_str(&payload).unwrap();\n        let additional: Vec<serde_json::Value> = serde_json::from_str(LIVE_AUDITS).unwrap();\n        assert_eq!(original.len(), 12);\n        assert_eq!(additional.len(), 2);\n        original.extend(additional);\n        original\n    })\n}\n").count(), 1);
    receiver_before = receiver_before.replacen("fn proofs() -> &'static [serde_json::Value] {\n    // Preserve the old parsed metadata cache, but authenticate the runtime\n    // dependency on every reached lookup, including an already warm cache.\n    let payload = crate::source_reflows_merge::receiver_fixture(\n        crate::source_reflows_merge::ReceiverFixture::Main67107,\n    );\n    PROOF_RECORDS.get_or_init(|| {\n        let mut original: Vec<serde_json::Value> = serde_json::from_str(&payload).unwrap();\n        let additional: Vec<serde_json::Value> = serde_json::from_str(LIVE_AUDITS).unwrap();\n        assert_eq!(original.len(), 12);\n        assert_eq!(additional.len(), 2);\n        original.extend(additional);\n        original\n    })\n}\n", "fn proofs() -> &'static [serde_json::Value] {\n    PROOF_RECORDS.get_or_init(|| {\n        let mut original: Vec<serde_json::Value> = serde_json::from_str(PROOFS).unwrap();\n        let additional: Vec<serde_json::Value> = serde_json::from_str(LIVE_AUDITS).unwrap();\n        assert_eq!(original.len(), 12);\n        assert_eq!(additional.len(), 2);\n        original.extend(additional);\n        original\n    })\n}\n", 1);
    assert_eq!(receiver_before.matches("const LIVE_AUDITS:").count(), 1);
    receiver_before = receiver_before.replacen("const LIVE_AUDITS:", "const PROOFS: &str = include_str!(\n    \"../../docs/evidence/main67107-production-composition-20261017/receiver-main12.json\"\n);\nconst LIVE_AUDITS:", 1);
    assert_eq!(
        digest(receiver_before.as_bytes()),
        *b"\x27\x41\xae\x80\x1d\xcf\x4a\x4d\x2e\x41\x68\x12\x57\x6b\xcb\xf6\x9f\x66\xe9\x4a\xd6\x97\x46\xea\x81\x98\x9a\x55\x4f\x1c\xfd\x3f"
    );
}

// RUNTIME848_SIX_PACKET_CONTROLS_BEGIN
#[test]
fn six_large_historical_packets_keep_full_inputs_whole_consumers_and_fresh_hostile_reads() {
    let directory = std::env::temp_dir().join(format!(
        "semaprax-six-history-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&directory).unwrap();
    let directory = Directory(directory);
    let path = directory.0.join("fixture.json");
    let mut total = 0;
    for (fixture, expected_rows, source, current, before, after, whole_before) in [
        (ReceiverFixture::PaidLiveReuse, 3, "src/loan_plan/paid_live/reuse_tests.rs",
            include_str!("../../loan_plan/paid_live/reuse_tests.rs"),
            "    let rows: serde_json::Value = serde_json::from_str(include_str!(\n        \"../../../docs/evidence/paid-live-pending-reuse-3c351/whole-source-inverses.json\"\n    ))\n    .unwrap();", "    let rows: serde_json::Value =\n        serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(\n            crate::source_reflows_merge::ReceiverFixture::PaidLiveReuse,\n        ))\n        .unwrap();", *b"\xd8\x6d\xf5\x6a\x91\x64\x3b\x4f\xd0\xbd\xf3\xb2\xba\xfa\xbc\x74\x72\xc1\xfd\xcb\xec\x6f\xba\x36\x8c\xd4\xc0\x51\x6f\x2b\x4b\x4f"),
        (ReceiverFixture::LexicalOperands, 14, "src/cleanup_plan/replay/lexical_replay/operand_tests/source.rs",
            include_str!("../../cleanup_plan/replay/lexical_replay/operand_tests/source.rs"),
            "    let rows: serde_json::Value = serde_json::from_str(include_str!(\n        \"../../../../../docs/evidence/ordinary-lexical-return-operands-832/exact-source-inverses.json\"\n    )).unwrap();", "    let rows: serde_json::Value =\n        serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(\n            crate::source_reflows_merge::ReceiverFixture::LexicalOperands,\n        ))\n        .unwrap();", *b"\x57\x5f\x74\x61\xfb\x66\x24\xf8\xdf\xdf\x1e\xd5\x11\xcb\xad\xe1\x22\x7a\x47\x35\x8a\xbd\x08\x58\x75\x53\x96\xd9\x49\xe2\x93\x1b"),
        (ReceiverFixture::PrivateLayoutCapacity, 9, "src/workspace_graph/expected_projection/private_layout/capacity_tests/source.rs",
            include_str!("../../workspace_graph/expected_projection/private_layout/capacity_tests/source.rs"),
            "    let rows: serde_json::Value = serde_json::from_str(include_str!(\n        \"../../../../../docs/evidence/private-layout-entry-capacity-834/exact-source-inverses.json\"\n    ))\n    .unwrap();", "    let rows: serde_json::Value =\n        serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(\n            crate::source_reflows_merge::ReceiverFixture::PrivateLayoutCapacity,\n        ))\n        .unwrap();", *b"\xcb\x4c\x70\x9e\xcd\x0d\x89\x66\xec\x03\xc9\xcc\x9d\x66\x64\x68\x18\x28\x61\xde\xd8\xcb\x0c\x86\x0e\xd2\x0c\x35\x30\x00\x64\xe2"),
        (ReceiverFixture::RegistryScratch, 5, "src/workspace_graph/expected_projection/private_layout/registry/scratch_tests.rs",
            include_str!("../../workspace_graph/expected_projection/private_layout/registry/scratch_tests.rs"),
            "    let packet: serde_json::Value = serde_json::from_str(include_str!(\n        \"../../../../../docs/evidence/registry-census-scratch-833/source-inverses.json\"\n    ))\n    .unwrap();", "    let packet: serde_json::Value =\n        serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(\n            crate::source_reflows_merge::ReceiverFixture::RegistryScratch,\n        ))\n        .unwrap();", *b"\x0b\xf3\xa8\x68\x12\x5c\x5d\x38\x1e\xad\x3b\x67\x12\x7f\x62\x20\x85\xd8\xfb\xa0\x9b\x23\x86\x2d\xb6\x8b\xda\xd1\x3c\x27\x1e\xc6"),
        (ReceiverFixture::ColdCanonical, 3, "src/workspace_graph/cold_canonical/tests.rs",
            include_str!("../../workspace_graph/cold_canonical/tests.rs"),
            "    let packet: serde_json::Value = serde_json::from_str(include_str!(\n        \"../../../docs/evidence/canonical-comment-comparison-835/source-inverses.json\"\n    ))\n    .unwrap();", "    let packet: serde_json::Value =\n        serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(\n            crate::source_reflows_merge::ReceiverFixture::ColdCanonical,\n        ))\n        .unwrap();", *b"\xec\xc2\xa0\x7a\xc3\x66\xad\xea\xdd\xc9\x86\xc5\xbf\x1c\x67\x60\x00\xe9\x5e\xc9\x8f\x5d\xd5\x26\xc3\x7c\x78\xf6\x54\x06\xd6\x2c"),
        (ReceiverFixture::BorrowedGeneric, 3, "src/workspace_graph/generic_type_import/borrowed_tests.rs",
            include_str!("../../workspace_graph/generic_type_import/borrowed_tests.rs"),
            "    let rows: serde_json::Value = serde_json::from_str(include_str!(\n        \"../../../docs/evidence/borrowed-generic-type-rewrite-141387/whole-source-inverses.json\"\n    ))\n    .unwrap();", "    let rows: serde_json::Value =\n        serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(\n            crate::source_reflows_merge::ReceiverFixture::BorrowedGeneric,\n        ))\n        .unwrap();", *b"\xd3\x31\xc8\x55\xc4\x1b\x28\x62\x85\x84\xbd\x25\x76\xc9\x3c\x0f\x72\x22\x6e\xd0\x7b\xeb\x1c\xfe\x25\x27\x2e\x17\x8a\x0a\x9b\x31"),
    ] {
        let spec = fixture.spec();
        assert!(spec.length <= OUTPUT_CAP);
        let payload = receiver_fixture(fixture);
        assert_eq!((payload.len(), digest(payload.as_bytes())), (spec.length, spec.sha256));
        total += payload.len();
        let packet: serde_json::Value = serde_json::from_str(&payload).unwrap();
        let rows = packet.as_array().unwrap_or_else(|| packet["owners"].as_array().unwrap());
        assert_eq!(rows.len(), expected_rows);
        let names: std::collections::BTreeSet<_> = rows.iter().map(|row| row["source"].as_str().unwrap()).collect();
        assert_eq!(names.len(), expected_rows, "complete unique source inventory");
        let entry = complete_receiver_entry_glue(source, current);
        let current = entry.as_str();
        assert_eq!(current.matches(after).count(), 1, "one transport change: {source}");
        let original = current.replacen(after, before, 1);
        assert_eq!(digest(original.as_bytes()), whole_before, "entire original consumer: {source}");
        assert_eq!(original.matches(before).count(), 1);
        assert_eq!(original.replacen(before, after, 1), current, "whole forward transport: {source}");
        assert_ne!(digest((original + " ").as_bytes()), whole_before);

        // Warm a valid read, then mutate the SAME path. Every read authenticates
        // a newly opened regular handle; no parsed or successful-verdict cache.
        std::fs::write(&path, payload.as_bytes()).unwrap();
        assert_eq!(read_fixture(&path, spec.length, spec.sha256).unwrap(), payload);
        assert_eq!(read_fixture(&directory.0.join("missing"), spec.length, spec.sha256), Err(Failure::Input));
        std::fs::write(&path, &payload.as_bytes()[..spec.length-1]).unwrap();
        assert_eq!(read_fixture(&path, spec.length, spec.sha256), Err(Failure::Bound));
        let mut changed = payload.as_bytes().to_vec(); changed.push(0);
        std::fs::write(&path, &changed).unwrap();
        assert_eq!(read_fixture(&path, spec.length, spec.sha256), Err(Failure::Bound));
        assert_eq!(read_body(&mut Cursor::new(&changed), spec.length, spec.sha256), Err(Failure::Trailing));
        changed.truncate(spec.length); changed[0] ^= 1;
        std::fs::write(&path, &changed).unwrap();
        assert_eq!(read_fixture(&path, spec.length, spec.sha256), Err(Failure::Digest));
        std::fs::write(&path, payload.as_bytes()).unwrap();
        assert_eq!(read_fixture(&path, spec.length, spec.sha256).unwrap(), payload);
    }
    assert_eq!(
        total, 13_704_202,
        "six complete physical inputs, not measured compiler memory"
    );
    let runtime = include_str!("../runtime_json.rs");
    let mut original =
        complete_receiver_entry_glue("src/source_reflows_merge/runtime_json.rs", runtime);
    for (start, end) in [
        (
            "    // RUNTIME848_SIX_VARIANTS_BEGIN\n",
            "    // RUNTIME848_SIX_VARIANTS_END\n",
        ),
        (
            "            // RUNTIME848_SIX_SPECS_BEGIN\n",
            "            // RUNTIME848_SIX_SPECS_END\n",
        ),
    ] {
        assert_eq!(original.matches(start).count(), 1);
        assert_eq!(original.matches(end).count(), 1);
        let at = original.find(start).unwrap();
        let stop = original.find(end).unwrap() + end.len();
        original.replace_range(at..stop, "");
    }
    assert_eq!(
        digest(original.as_bytes()),
        *b"\xd5\x3f\xe4\x61\xe2\x31\xd8\xbf\xa0\x61\xcf\x8b\x1f\xe3\x1e\x70\x9b\x32\xc2\xfe\x0f\xa2\xea\x4a\x79\xc1\x94\xbf\x08\x62\x5e\x43",
        "whole pre-six runtime reader facade"
    );
    let prior_tests = historical_inputs::before(include_str!("tests.rs"));
    let mut original_tests = prior_tests
        .split_once("\n// RUNTIME848_SIX_PACKET_CONTROLS_BEGIN\n")
        .unwrap()
        .0
        .to_owned();
    // The two finite ENTRY acquisitions below preserve the complete old test
    // body after their exact source-only glue is removed, before the unchanged
    // formatter/transport inverse. The helper itself is outside this prefix.
    for (after, before) in [
        ("    let mut root_before = complete_receiver_entry_glue(\n        \"src/source_reflows_merge.rs\",\n        include_str!(\"../../source_reflows_merge.rs\"),\n    );", "    let mut root_before = include_str!(\"../../source_reflows_merge.rs\").to_owned();"),
        ("    let mut receiver_before = complete_receiver_entry_glue(\n        \"src/source_reflows/incoming67107_semantic_receivers.rs\",\n        include_str!(\"../../source_reflows/incoming67107_semantic_receivers.rs\"),\n    );", "    let mut receiver_before =\n        include_str!(\"../../source_reflows/incoming67107_semantic_receivers.rs\").to_owned();"),
    ] {
        assert_eq!(original_tests.matches(after).count(), 1);
        original_tests = original_tests.replacen(after, before, 1);
    }
    let bridge = "    let authenticated = complete_preformat_transport(&authenticated);\n";
    assert_eq!(original_tests.matches(bridge).count(), 1);
    original_tests = original_tests.replacen(bridge, "", 1);
    // Exact formatter-only inverse; preserve every complete preexisting assertion.
    let formatted = "    assert!(std::panic::catch_unwind(|| {\n        pair_custody::normalize(\n            \"src/source_reflows_merge/runtime_json.rs\",\n            &(common.clone() + \" \"),\n        )\n    })\n    .is_err());\n";
    let prior = "    assert!(\n        std::panic::catch_unwind(|| {\n            pair_custody::normalize(\n                \"src/source_reflows_merge/runtime_json.rs\",\n                &(common.clone() + \" \"),\n            )\n        })\n        .is_err()\n    );\n";
    assert_eq!(original_tests.matches(formatted).count(), 1);
    original_tests = original_tests.replacen(formatted, prior, 1);
    assert_eq!(
        digest(original_tests.as_bytes()),
        *b"\x33\xe3\x78\x8d\x8e\xb3\xa9\x8e\xf7\x5a\x79\x40\x70\xa2\x7c\x4b\x1c\x16\x1d\x5c\x4c\x24\x2c\x1b\x45\xd6\xf3\x32\xe1\x6f\x61\xf3",
        "all prior runtime fixture controls retained"
    );
}

// The current-first reader above acquires the complete formatted transport.
// Restore only its exact compatibility annotation and four formatter hunks
// before replaying all unchanged independent historical transport controls.
fn complete_preformat_transport(current: &str) -> String {
    assert_eq!(current.len(), 8140);
    assert_eq!(
        digest(current.as_bytes()),
        *b"\x5a\x95\xfa\x1f\x07\xfa\xc9\x9c\x62\xf6\x57\x28\x17\xdf\x24\x76\x95\xa4\xd2\xe2\x74\xd5\xac\xcc\x5c\xe2\x31\xd7\xdc\x15\x0b\xc4"
    );
    let mut original = current.to_owned();
    for (after, before) in [
        ("#[allow(\n    dead_code,\n    reason = \"Retained compatibility entry for independently compiled historical and semantic controls.\"\n)]\n", ""),
        ("    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)\n        .expect(\"complete authenticated Main44 metadata\")\n", "    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256).expect(\"complete authenticated Main44 metadata\")\n"),
        ("    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)\n        .expect(\"complete authenticated Main44 source domain\")\n", "    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256).expect(\"complete authenticated Main44 source domain\")\n"),
        ("    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)\n        .expect(\"complete authenticated Main043 metadata\")\n", "    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256).expect(\"complete authenticated Main043 metadata\")\n"),
        ("    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)\n        .expect(\"complete authenticated Main043 helper input\")\n", "    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256).expect(\"complete authenticated Main043 helper input\")\n"),

    ] {
        assert_eq!(original.matches(after).count(), 1);
        original = original.replacen(after, before, 1);
    }
    assert_eq!(original.len(), 7972);
    assert_eq!(
        digest(original.as_bytes()),
        *b"\x86\x43\x01\xd0\xb8\x7f\x8d\xe9\x5c\x6b\x8d\x1f\x2f\xd3\x06\xc3\xbb\x96\xea\xc9\x2f\x56\x69\x98\x63\x08\x06\x5f\x2a\xbd\xae\xef"
    );
    original
}

// Exact approved ENTRY-glue inverse, separate from every original runtime-data
// transport inverse. Full compiled bytes are bound before any literal edit;
// no archived source is substituted and no old assertion/hash is retargeted.
#[rustfmt::skip]
fn complete_receiver_entry_glue(source: &str, current: &str) -> String {
    let current = complete_receiver_followup_glue(source, current);
    let current = current.as_str();
    let (length, current_sha, old_length, old_sha, edits): ReceiverTextEdits<'_> = match source {
        "src/source_reflows_merge.rs" => (
            54109, *b"\xdc\xf4\xe0\xdf\xee\xaf\x24\x8d\x4f\xbe\x92\x5b\x8d\xa3\x1b\x8e\x81\x96\x2d\x87\xfc\x62\xf1\x52\x33\x6e\x89\x3e\x7c\x72\x27\xd5",
            47396, *b"\x67\xc9\x9a\x6f\xb7\x90\xd9\xf2\x66\xf1\x2d\x09\x86\x05\xb1\x52\x36\xea\x8e\xa0\x7d\xc8\x94\xe6\x8f\x3d\x15\x25\x95\xfb\xef\xdf",
            &[
                ("}\n\n// These three formatter inverses bind the ENTIRE actual compiled child body,\n// not merely an archived before image. No token or string-content normalization.\npub(crate) fn receiver_child_entry(source: &str, current: &str) -> String {\n    let (expected, length, compiled_sha, after, before) = match source {\n        \"src/workspace_graph/expected_projection/private_layout/before_registry.rs\" => (\n            \"0d6e2e30c78d040eafcd7c11e66a4696d17e949aa09eac7d79d6d1b6634721cd\", 8142,\n            \"986c2f1eb41715186ee5a1856e58abbcde03d53e86e53b1e903e1b0845c002b8\",\n            \"}\\n\\n#[cfg(test)]\\nthread_local! {static DERIVATIONS:\",\n            \"}\\n\\n\\n#[cfg(test)]\\nthread_local! {static DERIVATIONS:\",\n        ),\n        \"src/workspace_graph/cold_canonical/before.rs\" => (\n            \"78fbcd95f4430f0750b4ae1e8b2cfdc5277c3bf9ef02fe4776cea96bc5a494e2\", 19289,\n            \"cd153b0db9d19b6b8983fc66f3970494d1247c2180b595d839db96eeaf4f8487\",\n            \"                    staged!(\\n                        CorePrebound,\\n                        constructor_failure::attempt::reserve(resolve_builder_bytes)\\n                    )?;\",\n            \"                    staged!(CorePrebound, constructor_failure::attempt::reserve(resolve_builder_bytes))?;\",\n        ),\n        \"src/format/comments/compare/tests.rs\" => (\n            \"7adedbc8fc515199d21e71a93fbe7a1624c5500e46278f4ddf834050ef0a7b38\", 3421,\n            \"4b422dd4781d2123eddd2eabb1459c5e3bedb222fc6957729d4c444e3ca2cf2c\",\n            \"    assert_eq!(\\n        refusal.message,\\n        \\\"a module must declare at least one function\\\"\\n    );\",\n            \"    assert_eq!(refusal.message, \\\"a module must declare at least one function\\\");\",\n        ),\n        _ => return current.to_owned(),\n    };\n    // Fresh physical/compiled/archive authentication precedes historical lookup.\n    let entry = main_e177_final_inputs::value_direct(source, expected, current);\n    assert_eq!(\n        current.len(),\n        length,\n        \"complete compiled child extent: {source}\"\n    );\n    assert_eq!(\n        format!(\n            \"{:x}\",\n            crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))\n        ),\n        compiled_sha,\n        \"complete compiled child bytes: {source}\"\n    );\n    assert_eq!(\n        current.matches(after).count(),\n        1,\n        \"one literal child formatter inverse: {source}\"\n    );\n    let inverse = current.replacen(after, before, 1);\n    assert_eq!(\n        format!(\n            \"{:x}\",\n            crate::digest_hex::LowerHex(Sha256::digest(inverse.as_bytes()))\n        ),\n        expected,\n        \"complete original compiled child: {source}\"\n    );\n    assert_eq!(\n        inverse, entry,\n        \"whole compiled child equals independently acquired history: {source}\"\n    );\n    inverse\n}\n\npub(crate) fn consumer_child(domain: &str, source: &str, current: &str) -> String {\n    let entry = receiver_child_entry(source, current);\n    let current = entry.as_str();\n    let branch = final_fix_input(source, current);\n    let current = branch.as_str();\n", "}\n\npub(crate) fn consumer_child(domain: &str, source: &str, current: &str) -> String {\n    let branch = final_fix_input(source, current);\n    let current = branch.as_str();\n"),
                ("    runtime_json::project(runtime_json::Fixture::F16, source, current)\n}\n// Closed packet/source pairs only. This acquires the original consumer INPUT,\n// never its final inverse output; all caller hooks and full-byte checks remain.\npub(crate) fn receiver_fixture_entry(\n    fixture: ReceiverFixture,\n    source: &str,\n    current: &str,\n) -> String {\n    let expected = match (fixture, source) {\n        (ReceiverFixture::PaidLiveReuse, \"src/loan_plan/paid_live.rs\") => \"bcb520a64f14a76f2bac1e6053ce40f424d15e11e8b1e70f8c67d4406c450374\",\n        (ReceiverFixture::PaidLiveReuse, \"src/loan_plan/paid_plan.rs\") => \"0eb3df7863a61410f5a4547831aa399ffd03d460dad86f1f40caf626de0f2be7\",\n        (ReceiverFixture::PaidLiveReuse, \"crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs\") => \"d764e8d6fbd373a2dbb40953baabf688fb4459562760fd5e1545cb9b73b9e6b0\",\n        (ReceiverFixture::BorrowedGeneric, \"src/workspace_graph/generic_type_import.rs\") => \"e4aaa275d0b6d79539a30b496844388ba9fc3b4b793499b907b803f759ee8dc9\",\n        (ReceiverFixture::BorrowedGeneric, \"src/workspace_graph/expected_projection/cost.rs\") => \"ddf8dc2e74e4c0cc193e4e592671ffcfd9118b8bfe91cef2d0cf19eca3a635ce\",\n        (ReceiverFixture::BorrowedGeneric, \"tests/workspace/semantic_graph/source_inventory.rs\") => \"4a1920f6abdff90036fe9afabe687c091b08a38d4f26a28e2e7c7feb678cbd14\",\n        (ReceiverFixture::LexicalOperands, \"src/cleanup_plan/replay/expression_facts/receiving.rs\") => \"db822f51812769eadf1570bfee4b315ceadd31b689fc6efa643e5da03d7a9bb7\",\n        (ReceiverFixture::LexicalOperands, \"src/cleanup_plan/replay/expression_facts/receiving/directory.rs\") => \"d161622003e500b3b9197c0f463b88eee388d9447c5a87ba75f464d3aa2a59cc\",\n        _ => return current.to_owned(),\n    };\n    let entry = main_e177_final_inputs::value_direct(source, expected, current);\n    assert_eq!(\n        format!(\n            \"{:x}\",\n            crate::digest_hex::LowerHex(Sha256::digest(entry.as_bytes()))\n        ),\n        expected,\n        \"complete fixed receiver input: {source}\"\n    );\n    entry\n}\n\npub(crate) fn consumer_input(domain: &str, source: &str, current: &str) -> String {\n    let entry = if domain == \"lexical832\" {\n        receiver_fixture_entry(ReceiverFixture::LexicalOperands, source, current)\n    } else {\n        current.to_owned()\n    };\n    let current = entry.as_str();\n    let predecessor = corrected_consumers_previous(source, current);\n    let current = predecessor.as_str();\n", "    runtime_json::project(runtime_json::Fixture::F16, source, current)\n}\npub(crate) fn consumer_input(domain: &str, source: &str, current: &str) -> String {\n    let predecessor = corrected_consumers_previous(source, current);\n    let current = predecessor.as_str();\n"),
                ("        return current.to_owned();\n    };\n    let entry = historical_project_entry(source, row[\"current_sha256\"].as_str().unwrap(), current);\n    let acquired = main67107_inputs::compile35f_expected_input(\n        source,\n        row[\"current_sha256\"].as_str().unwrap(),\n        &entry,\n    );\n    let current = acquired.as_str();\n", "        return current.to_owned();\n    };\n    let acquired = main67107_inputs::compile35f_expected_input(\n        source,\n        row[\"current_sha256\"].as_str().unwrap(),\n        current,\n    );\n    let current = acquired.as_str();\n"),
                ("    metadata_json::project(metadata_json::Fixture::M24, source, current)\n}\n// Only literal historical ENTRY stages are acquired here. The complete existing\n// compile35f front and every intermediate inverse below still execute in order.\nfn historical_project_entry(source: &str, expected: &str, current: &str) -> String {\n    let admitted = matches!(\n        (source, expected),\n        (\n            \"src/cleanup_plan/replay/live_oracle/source_custody/b.rs\",\n            \"ca0e42fbd3f14f29ff59b7dc4c6bff26a364a1e305967bcf548970a53774a9ca\"\n        ) | (\n            \"src/workspace_graph.rs\",\n            \"b8d71709bbe9961b338552bba4f6098e59c8d78c5da7ca5f8ff4157b36ea3f31\"\n        ) | (\n            \"tests/workspace/semantic_graph/source_inventory.rs\",\n            \"aa4e1844d2e867c519bd24fdb560535c9f07b7daa790e343c1b4b04e978d0989\"\n        ) | (\n            \"src/workspace_graph/expected_projection/private_layout/registry/scratch_tests.rs\",\n            \"0bf3a868125c5d381ead3b67127f622085d8fba09b23862db68bdad13c271ec6\"\n        ) | (\n            \"src/workspace_graph/cold_canonical/tests.rs\",\n            \"ecc2a07ac366adeaddc986c5bf1c676000e95ec98f5dd526c37c78f65406d62c\"\n        )\n    );\n    if !admitted {\n        return current.to_owned();\n    }\n    let entry = main_e177_final_inputs::value_direct(source, expected, current);\n    assert_eq!(\n        format!(\n            \"{:x}\",\n            crate::digest_hex::LowerHex(Sha256::digest(entry.as_bytes()))\n        ),\n        expected,\n        \"complete fixed historical projector entry: {source}\"\n    );\n    entry\n}\nfn project(rows: &str, source: &str, current: &str) -> String {\n    let rows: serde_json::Value = serde_json::from_str(rows).expect(\"fixed merge projections\");\n", "    metadata_json::project(metadata_json::Fixture::M24, source, current)\n}\nfn project(rows: &str, source: &str, current: &str) -> String {\n    let rows: serde_json::Value = serde_json::from_str(rows).expect(\"fixed merge projections\");\n"),
            ],
        ),
        "src/source_reflows/incoming67107_semantic_receivers.rs" => (
            36321, *b"\x8a\xc5\xb0\xbc\x82\xb7\x48\x96\xed\xe9\x91\x86\x39\xb9\x06\x93\x0d\x34\x84\xf4\xbb\xb3\x91\xe8\x1c\x74\xe4\xab\x51\xc4\x91\x2a",
            34859, *b"\x7a\x14\xee\x94\x6d\x44\xe4\x24\x48\xc7\xb0\x69\xee\x0d\xaa\x1a\x35\xde\xda\xe5\x2e\x8c\x7a\x99\x8c\xba\xd8\xb4\x3b\x37\x95\xe8",
            &[
                ("\npub(super) fn invert_ordinary(proof: &serde_json::Value, current: &str) -> String {\n    // Only these six exact Main67107 entry tuples changed after this packet.\n    // The full existing MAIN21 acquisition and every inverse below still run.\n    let path = proof[\"path\"].as_str().unwrap();\n    let expected = proof[\"after_sha256\"].as_str().unwrap();\n    let entry = match (path, expected) {\n        (\n            \"src/cleanup_plan/replay.rs\",\n            \"30616d71694d393a730c0c8e49a25bbad7539b07ff96ed901b9d07be29765b9c\",\n        )\n        | (\n            \"tests/hir_module_boundaries.rs\",\n            \"f25b784eba1a2ea26a07b260b2ddff67b6dd90c0a6402fd143dc42ca68847d4e\",\n        )\n        | (\n            \"tests/workspace/semantic_graph/source_inventory.rs\",\n            \"59e5b98593c2c2d443d1479e7c89564551d515ca921f7abab2cc8f5d579c8c21\",\n        )\n        | (\n            \"src/source_verify/declaration.rs\",\n            \"c93ac4c883fe6b10c831a2603c994ea03880986fd0b7a394bfa41ec8a06d88cb\",\n        )\n        | (\n            \"src/cleanup_plan/replay/branch_clone/tests.rs\",\n            \"46bf0451ef390225f540d031f326100c7f4288cf7e9d50e30ce7ddb68a448e0f\",\n        )\n        | (\n            \"src/cleanup_plan/replay/factored.rs\",\n            \"343eda3712d0c3b224742d72c34838a327e0c69449c8a94fd7eb81dc5e0134a6\",\n        ) => Some(\n            crate::source_reflows_merge::main_e177_final_inputs::value_direct(\n                path, expected, current,\n            ),\n        ),\n        _ => None,\n    };\n    let acquired = super::semantic_receivers::incoming_main21::ours_expected_input(\n        proof,\n        entry.as_deref().unwrap_or(current),\n    );\n    let current = acquired.as_str();\n    let path = proof[\"path\"].as_str().unwrap();\n", "\npub(super) fn invert_ordinary(proof: &serde_json::Value, current: &str) -> String {\n    let acquired = super::semantic_receivers::incoming_main21::ours_expected_input(proof, current);\n    let current = acquired.as_str();\n    let path = proof[\"path\"].as_str().unwrap();\n"),
            ],
        ),
        "src/source_reflows_merge/runtime_json.rs" => (
            14714, *b"\xa7\xd3\x8b\x1f\x8b\xd2\x93\xdd\x84\x3a\x0d\xae\x9f\x6b\x27\x6f\x37\x4e\xad\x45\x97\x82\x99\x8f\xb4\x01\x35\x43\x11\x02\x6a\x63",
            12620, *b"\x2a\x0b\xb2\xa4\xaa\x8d\xd3\x4a\x67\x56\x5c\x23\xf5\x47\x3c\x44\x97\xb0\x63\x25\xe6\x70\x1c\x3f\x87\x8d\xc3\xdf\x18\xf1\x67\x43",
            &[
                ("// exactly the two transport edits before applying its unchanged original inverse.\npub(crate) fn receiver_fixture_source_before(source: &str, current: &str) -> String {\n    // These ten exact entry images belong to the unchanged SelectedCalls\n    // inverse packet. Acquire them only after current physical/compiled/archive\n    // authentication; the original self-transport inverse below stays intact.\n    let expected = match source {\n        \"crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs\" => Some(\"d764e8d6fbd373a2dbb40953baabf688fb4459562760fd5e1545cb9b73b9e6b0\"),\n        \"src/cleanup_plan/replay/lexical_replay.rs\" => Some(\"d0f2b7fcfc3815f0c1207c064d8bcb14d4323dff614d953988b962138ff975d0\"),\n        \"src/cleanup_plan/replay/live_oracle/source_custody/b.rs\" => Some(\"4d534b5433078d8d65ec9f09c7f763614a177cd743aa02465de64c023e509173\"),\n        \"src/cleanup_plan/replay/scalar_replay.rs\" => Some(\"6cbb996ba104a91029e7fb323ad2801c483d6f71b4b19ef10fd097a16f0c764a\"),\n        \"src/cleanup_plan/replay/selected_entry/tests/session_errors.rs\" => Some(\"2b27003287b4e325ed126b4e26f9f933fc039e42f66739242b63759d3d713b05\"),\n        \"src/cleanup_plan/replay/semantic_refusal.rs\" => Some(\"fc1a6f41a225e0936ede78ff9923eb0eea1e93ce5c8e37fbde9709e6ee78c823\"),\n        \"src/cleanup_plan/replay/semantic_refusal/tests.rs\" => Some(\"aca69e92356ddfb4d7bbb774ca647a981cf86d402d9b0dff71108adef777b1ad\"),\n        \"src/cleanup_plan/replay/structural_control/calls.rs\" => Some(\"17f757fd22a55b7aaaca952c0ab84b7c8d45bf9285fa15a0a4032ecfbdaa5000\"),\n        \"tests/hir_module_boundaries.rs\" => Some(\"055a8f7ddd8b9f5b2d12122154de053a0a6823b75c298580cd4770628c4e8851\"),\n        \"tests/workspace/semantic_graph/source_inventory.rs\" => Some(\"4a1920f6abdff90036fe9afabe687c091b08a38d4f26a28e2e7c7feb678cbd14\"),\n        _ => None,\n    };\n    if let Some(expected) = expected {\n        let entry = super::main_e177_final_inputs::value_direct(source, expected, current);\n        assert_eq!(\n            format!(\n                \"{:x}\",\n                crate::digest_hex::LowerHex(digest(entry.as_bytes()))\n            ),\n            expected,\n            \"complete SelectedCalls entry: {source}\"\n        );\n        return entry;\n    }\n    if source != \"src/cleanup_plan/replay/structural_control/calls/custody_tests/source.rs\" {\n        return current.to_owned();\n", "// exactly the two transport edits before applying its unchanged original inverse.\npub(crate) fn receiver_fixture_source_before(source: &str, current: &str) -> String {\n    if source != \"src/cleanup_plan/replay/structural_control/calls/custody_tests/source.rs\" {\n        return current.to_owned();\n"),
            ],
        ),
        "src/loan_plan/paid_live/reuse_tests.rs" => (
            23070, *b"\x0b\x52\x79\x17\x76\x39\x85\x04\x1a\x50\x0b\xd9\xd7\x07\x2d\x23\x8d\x21\x03\xd7\x90\xa1\x53\x81\xa0\x2c\xd5\xca\x05\xef\x78\x90",
            22879, *b"\x1d\x1f\x1c\x68\xaf\xc5\xb1\xfd\x41\x79\x7e\x01\x8f\xb4\x6c\xce\xdc\x2a\x1c\xec\x86\xe3\x47\xdc\x5c\x4d\x7e\xcb\x29\xf5\xe4\xd7",
            &[
                ("    ];\n    for (index, (input, original)) in inputs.into_iter().enumerate() {\n        let mut restored = crate::source_reflows_merge::receiver_fixture_entry(\n            crate::source_reflows_merge::ReceiverFixture::PaidLiveReuse,\n            rows[index][\"source\"].as_str().unwrap(),\n            input,\n        );\n        for hook in rows[index][\"hooks\"].as_array().unwrap() {\n            let after = hook[\"after\"].as_str().unwrap();\n", "    ];\n    for (index, (input, original)) in inputs.into_iter().enumerate() {\n        let mut restored = input.to_owned();\n        for hook in rows[index][\"hooks\"].as_array().unwrap() {\n            let after = hook[\"after\"].as_str().unwrap();\n"),
            ],
        ),
        "src/workspace_graph/expected_projection/private_layout/registry/scratch_tests.rs" => (
            17919, *b"\xa7\xe1\x46\xf8\x9c\x90\xb1\xca\xcb\x07\xa3\x99\x71\xe7\xd5\x1f\xe1\xeb\x32\xb8\x75\x33\x05\xd0\x92\xb6\xae\xe7\x16\x27\xc8\x90",
            17756, *b"\x4d\x5e\xa7\x83\xb1\x85\x5f\xcb\x0a\xd7\x9b\xbc\x0a\xa0\x13\x43\x0d\x44\x06\xeb\x8b\xb4\xbf\x6d\xe1\x16\x69\xfb\x08\x8a\x5d\x50",
            &[
                ("    );\n    assert_eq!(\n        crate::source_reflows_merge::receiver_child_entry(\n            \"src/workspace_graph/expected_projection/private_layout/before_registry.rs\",\n            include_str!(\"../before_registry.rs\"),\n        ),\n        old_registry\n            .replace(\n", "    );\n    assert_eq!(\n        include_str!(\"../before_registry.rs\"),\n        old_registry\n            .replace(\n"),
            ],
        ),
        "src/workspace_graph/cold_canonical/tests.rs" => (
            16089, *b"\x9f\x86\xfb\xb6\xc4\x0d\x52\x85\xd1\x3a\xd2\xb9\x0e\xb0\x98\x60\xec\xbb\x82\x92\xb9\x4a\xa3\x68\xa8\x90\xb7\xc3\xe9\xd7\x1c\xc6",
            15976, *b"\xfd\xfb\x6b\xcb\xca\x86\x66\x8e\xfc\xd8\x5d\xc0\x54\xc2\x89\x86\xae\x2d\x62\xb9\xee\xfa\x9b\xa1\x85\xb9\xe0\x91\x15\x2d\x1f\xca",
            &[
                ("        1,\n    );\n    assert_eq!(crate::source_reflows_merge::receiver_child_entry(\n        \"src/workspace_graph/cold_canonical/before.rs\", include_str!(\"before.rs\")\n    ),format!(\"//! Complete immutable940 cold preflight; no comparison-sink delegation.\\nuse super::super::*;\\n{body}\\n{wrapper}\"));\n    assert_eq!(\n        include_str!(\"../../format.rs\"),\n", "        1,\n    );\n    assert_eq!(include_str!(\"before.rs\"),format!(\"//! Complete immutable940 cold preflight; no comparison-sink delegation.\\nuse super::super::*;\\n{body}\\n{wrapper}\"));\n    assert_eq!(\n        include_str!(\"../../format.rs\"),\n"),
            ],
        ),
        "src/workspace_graph/generic_type_import/borrowed_tests.rs" => (
            22118, *b"\xca\xbb\x93\x5f\x95\x35\x96\x05\x70\x0f\x59\x09\xfd\x27\xf9\xe6\xdb\x72\xbe\x4f\xc3\x7b\x0c\xa3\x06\x0a\x59\x33\x26\x47\xf7\x2d",
            21925, *b"\x7a\x16\x3c\xa8\x27\x30\xbc\xba\xc6\xe3\xc5\x73\xb9\x06\x46\x3d\xe5\xb2\x22\x28\xcd\x2e\x34\xc3\x77\xb0\xbd\x81\xa8\x91\x86\xf4",
            &[
                ("    ];\n    for (index, (input, before)) in inputs.into_iter().enumerate() {\n        let mut restored = crate::source_reflows_merge::receiver_fixture_entry(\n            crate::source_reflows_merge::ReceiverFixture::BorrowedGeneric,\n            rows[index][\"source\"].as_str().unwrap(),\n            input,\n        );\n        for hook in rows[index][\"hooks\"].as_array().unwrap() {\n            let after = hook[\"after\"].as_str().unwrap();\n", "    ];\n    for (index, (input, before)) in inputs.into_iter().enumerate() {\n        let mut restored = input.to_owned();\n        for hook in rows[index][\"hooks\"].as_array().unwrap() {\n            let after = hook[\"after\"].as_str().unwrap();\n"),
            ],
        ),
        // The other two six-packet consumers are byteexact already; their
        // unchanged whole original transport checks below remain mandatory.
        _ => return current.to_owned(),
    };
    assert_eq!(current.len(), length, "complete compiled ENTRY-glue extent: {source}");
    assert_eq!(digest(current.as_bytes()), current_sha, "complete compiled ENTRY-glue input: {source}");
    let mut original = current.to_owned();
    for &(after, before) in edits {
        assert_eq!(original.matches(after).count(), 1, "one fixed ENTRY-glue hunk: {source}");
        original = original.replacen(after, before, 1);
    }
    assert_eq!(original.len(), old_length, "whole original ENTRY extent: {source}");
    assert_eq!(digest(original.as_bytes()), old_sha, "whole original ENTRY bytes: {source}");
    original
}

// Later ENTRY and compiled-receiver repairs bind each whole current input before
// restoring its complete predecessor. Every original transport proof still runs.
#[rustfmt::skip]
fn complete_receiver_followup_glue(source: &str, current: &str) -> String {
    let three_before = complete_three_fixture_runtime_glue(source, current);
    let current = three_before.as_str();
    let receiver;
    let current = if source == "src/source_reflows/incoming67107_semantic_receivers.rs" {
        receiver = super::super::main_e177_final_inputs::value_direct(
            source,
            "de536dea276a4e1801a8df06f7c45cbed916b1b861e7c7859a2f040ee37759c8",
            current,
        );
        receiver.as_str()
    } else {
        current
    };
    let (length, current_sha, old_length, old_sha, edits): ReceiverTextEdits<'_> = match source {
        "src/source_reflows_merge/runtime_json.rs" => (
            16413, *b"\x73\x1f\x28\x59\x97\x57\xa0\x90\xce\x12\xc1\x1d\xa1\x08\xcc\x0a\xea\x5b\x3e\x3e\x18\x0a\xc0\xbb\x5a\xd5\x6d\x53\x16\x5f\xfd\xec",
            14714, *b"\xa7\xd3\x8b\x1f\x8b\xd2\x93\xdd\x84\x3a\x0d\xae\x9f\x6b\x27\x6f\x37\x4e\xad\x45\x97\x82\x99\x8f\xb4\x01\x35\x43\x11\x02\x6a\x63",
            &[
                ("    // These fourteen exact entry images belong to the unchanged SelectedCalls\n", "    // These ten exact entry images belong to the unchanged SelectedCalls\n"),
                ("        \"src/cleanup_plan/replay/structural_control.rs\" => Some(\"84c9d537b31a2f24e0f4c977ea0341bf4a4a6a738c299f5e7e35976a4946a037\"),\n        \"src/cleanup_plan/replay/structural_control/blocks.rs\" => Some(\"be348bfc7e713d79dccb11d6022a7371a3419390703aac7e667ae613f591b725\"),\n        \"src/cleanup_plan/replay/structural_control/blocks_before_receipt.rs\" => Some(\"93a5af9fc855967b0fd48573c0dd7ff8075ef63cda3ab8e9a28787d6be47ea04\"),\n        \"src/cleanup_plan/replay/structural_control/calls/custody_tests.rs\" => Some(\"55245329e3a803f31cbdf5488c4068934f1b74ac03cfca84bbf5eb6d93f53dc7\"),\n", ""),
                ("    assert_eq!(current.len(), 7873);\n    assert_eq!(digest(current.as_bytes()), [22, 25, 246, 163, 6, 93, 58, 87, 222, 52, 59, 43, 203, 215, 114, 78, 99, 244, 200, 168, 70, 222, 252, 141, 0, 68, 117, 28, 248, 37, 90, 5]);\n    assert_eq!(current.matches(\"        crate::source_reflows_merge::receiver_fixture_source_before(\\n            \\\"src/cleanup_plan/replay/structural_control/blocks_before_receipt.rs\\\",\\n            include_str!(\\\"../../blocks_before_receipt.rs\\\"),\\n        ).replacen(\").count(), 1);\n    let prior = current.replacen(\"        crate::source_reflows_merge::receiver_fixture_source_before(\\n            \\\"src/cleanup_plan/replay/structural_control/blocks_before_receipt.rs\\\",\\n            include_str!(\\\"../../blocks_before_receipt.rs\\\"),\\n        ).replacen(\", \"        include_str!(\\\"../../blocks_before_receipt.rs\\\").replacen(\", 1);\n    assert_eq!(prior.len(), 7706);\n    assert_eq!(digest(prior.as_bytes()), [131, 224, 221, 224, 214, 14, 199, 238, 82, 83, 218, 38, 95, 37, 112, 115, 28, 115, 130, 137, 146, 83, 41, 253, 48, 76, 196, 9, 53, 64, 39, 52]);\n    let current = prior.as_str();\n", ""),
            ],
        ),
        "src/source_reflows/incoming67107_semantic_receivers.rs" => (
            36765, *b"\xde\x53\x6d\xea\x27\x6a\x4e\x18\x01\xa8\xdf\x06\xf7\xc4\x5c\xbe\xd9\x16\xb1\xb8\x61\xe7\xc7\x85\x9a\x2f\x04\x0e\xe3\x77\x59\xc8",
            36321, *b"\x8a\xc5\xb0\xbc\x82\xb7\x48\x96\xed\xe9\x91\x86\x39\xb9\x06\x93\x0d\x34\x84\xf4\xbb\xb3\x91\xe8\x1c\x74\xe4\xab\x51\xc4\x91\x2a",
            &[
                ("    // Only these nine exact Main67107 entry tuples changed after this packet.\n", "    // Only these six exact Main67107 entry tuples changed after this packet.\n"),
                ("        )\n        | (\n            \"src/cleanup_plan/replay/factored/oracle_hook.rs\",\n            \"0126d25150ca4b96f11995293fc9043d23fc8fff2008485e9560abe5c85cb7ae\",\n        )\n        | (\n            \"src/hir/validation.rs\",\n            \"f89e882a3d8ed796b8b8b864247e4e7eb19631d6299cd0dd94ddcc6b5ca17eba\",\n        )\n        | (\n            \"src/hir/expr_nodes.rs\",\n            \"e2291267786bdec4371b969031209b4e77aa6876f4564048ad1c7561b004b73f\",\n", ""),
            ],
        ),
        "src/loan_plan/paid_live/reuse_tests.rs" => (
            25265, *b"\x67\x22\xb6\xfe\x25\xb8\x41\x00\x67\x19\xf4\x7a\xfe\xb6\x61\x0d\x18\x80\xfc\x5e\xdd\x6f\x7a\x72\x37\xdf\xda\x7a\x3f\x16\xab\xb5",
            23070, *b"\x0b\x52\x79\x17\x76\x39\x85\x04\x1a\x50\x0b\xd9\xd7\x07\x2d\x23\x8d\x21\x03\xd7\x90\xa1\x53\x81\xa0\x2c\xd5\xca\x05\xef\x78\x90",
            &[
                ("    let old_live = original_test_receiver(\n        \"src/loan_plan/paid_live/original.rs\",\n        include_str!(\"original.rs\"),\n    );\n", "    let old_live = include_str!(\"original.rs\");\n"),
                ("    let mut before_facade = original_test_receiver(\n        \"src/loan_plan/paid_live/funded_before.rs\",\n        include_str!(\"funded_before.rs\"),\n    );\n", "    let mut before_facade = include_str!(\"funded_before.rs\").to_owned();\n"),
                ("\n// Only test-module lint/namespace glue separates these compiled receivers from\n// their complete original bodies. Authenticate current bytes before inversion.\nfn original_test_receiver(source: &str, compiled: &str) -> String {\n    use sha2::{Digest, Sha256};\n    let (length, current_hash, count, schema_import, old_length, old_hash) = match source {\n        \"src/loan_plan/paid_live/original.rs\" => (\n            2893,\n            \"53f1bf0c833c8bf88d391938c958e05b4210a78f7a26fd787cd7e06e0f27c210\",\n            1,\n            false,\n            2786,\n            \"9315163a396ddc3f3905dc8b56dd220ccbb6d852dd270d18f59a171308af4c26\",\n        ),\n        \"src/loan_plan/paid_live/funded_before.rs\" => (\n            3628,\n            \"9934c14303d48cf9f221065b9525628edc69919a65e8002e85444d03d7eee918\",\n            3,\n            true,\n            3264,\n            \"8acbf4f2aef5cb26524261c8ac27a9d0b6adce6334d43e11eb972fb88ef61401\",\n        ),\n        _ => panic!(\"unknown complete receiver source\"),\n    };\n    let mut original = crate::source_reflows_merge::main_e177_final_inputs::value_direct(\n        source,\n        current_hash,\n        compiled,\n    );\n    assert_eq!(original.len(), length);\n    assert_eq!(\n        format!(\n            \"{:x}\",\n            crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))\n        ),\n        current_hash\n    );\n    let lint = \"#[allow(\\n    clippy::duplicate_mod,\\n    reason = \\\"Run the same controls against independent receivers.\\\"\\n)]\\n\";\n    assert_eq!(original.matches(lint).count(), count);\n    original = original.replace(lint, \"\");\n    if schema_import {\n        let import = \"use crate::loan_plan::LOAN_PLAN_SCHEMA_V3;\\n\";\n        assert_eq!(original.matches(import).count(), 1);\n        original = original.replacen(import, \"\", 1);\n    }\n    assert_eq!(original.len(), old_length);\n    assert_eq!(\n        format!(\n            \"{:x}\",\n            crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))\n        ),\n        old_hash\n    );\n    original\n}\n", ""),
            ],
        ),
        "src/workspace_graph/expected_projection/private_layout/capacity_tests/source.rs" => (
            6885, *b"\x4c\x66\x45\x03\xb8\xc2\x94\x0f\x9c\x35\x19\x52\x4d\x80\xc9\xa5\x8f\x23\x4f\x8d\xbd\xb9\x73\x84\x38\xdd\xa5\xef\xf6\xfd\x1e\xdd",
            4326, *b"\x59\x2f\xf9\x9b\x19\x90\x00\xa5\x29\xe6\x8f\x39\xe9\x1e\x98\x6f\xc1\x33\x87\x17\x18\x96\x58\x89\x23\x1f\x01\x39\xff\x6a\x57\x5e",
            &[
                ("        let entry = capacity_receiver_entry(path, source);\n        let projected = crate::source_reflows_merge::consumer_input(\"capacity834\", path, &entry);\n", "        let projected = crate::source_reflows_merge::consumer_input(\"capacity834\", path, source);\n"),
                ("    let before = capacity_receiver_entry(\n        \"src/workspace_graph/expected_projection/private_layout/capacity_before.rs\",\n        include_str!(\"../capacity_before.rs\"),\n    );\n", "    let before = include_str!(\"../capacity_before.rs\");\n"),
                ("\n// Closed whole-input acquisition precedes every unchanged capacity inverse.\nfn capacity_receiver_entry(source: &str, compiled: &str) -> String {\n    use sha2::{Digest, Sha256};\n    let digest = |s: &str| {\n        format!(\n            \"{:x}\",\n            crate::digest_hex::LowerHex(Sha256::digest(s.as_bytes()))\n        )\n    };\n    match source {\n        \"src/workspace_graph/expected_projection/private_layout/capacity_tests/source.rs\" => {\n            let prior = crate::source_reflows_merge::main_e177_final_inputs::value_direct(\n                source,\n                \"592ff99b199000a529e68f39e91e986fc133871718965889231f0139ff6a575e\",\n                compiled,\n            );\n            assert_eq!(prior.len(), 4326);\n            assert_eq!(\n                digest(&prior),\n                \"592ff99b199000a529e68f39e91e986fc133871718965889231f0139ff6a575e\"\n            );\n            prior\n        }\n        \"src/workspace_graph/expected_projection/private_layout/capacity_before.rs\" => {\n            let current = crate::source_reflows_merge::main_e177_final_inputs::value_direct(\n                source,\n                \"e0dfdd0ec25d52c0855f8c53027e1c4b23fbe8f7d24b045d5f0d34f1ee91ccff\",\n                compiled,\n            );\n            assert_eq!(current.len(), 3983);\n            assert_eq!(\n                digest(&current),\n                \"e0dfdd0ec25d52c0855f8c53027e1c4b23fbe8f7d24b045d5f0d34f1ee91ccff\"\n            );\n            let formatted = \"use super::{\\n    debit, explicit_only, ledger, names, owner_index, private_name, refusal,\\n    reserve_builder_structure, sorting, visit, Absence, AuthoredDeclaration, BTreeMap, Diagnostic,\\n    Entry, Frame, LayoutClosure, Program, DEPTH,\\n};\\n\";\n            let original = \"use super::{Absence, AuthoredDeclaration, BTreeMap, DEPTH, Diagnostic, Entry, Frame, LayoutClosure, Program, debit, explicit_only, ledger, names, owner_index, private_name, refusal, reserve_builder_structure, sorting, visit};\\n\";\n            assert_eq!(current.matches(formatted).count(), 1);\n            let prior = current.replacen(formatted, original, 1);\n            assert_eq!(prior.len(), 3968);\n            assert_eq!(\n                digest(&prior),\n                \"8f3117e619ea89862c658ef2104c50022c05a0876298737f8446e0168e32e75d\"\n            );\n            prior\n        }\n        _ => compiled.to_owned(),\n    }\n}\n", ""),
            ],
        ),
        "src/source_reflows_merge.rs" => (
            54430, *b"\x00\xb5\x91\x34\xc1\x63\x0e\xef\x13\xc1\xe1\xdf\xc8\x33\x7f\x9a\x68\x49\x6e\xf4\xa6\x1a\x0e\xa4\x32\x62\xf5\xfa\xd2\xd1\xbe\x1d",
            54109, *b"\xdc\xf4\xe0\xdf\xee\xaf\x24\x8d\x4f\xbe\x92\x5b\x8d\xa3\x1b\x8e\x81\x96\x2d\x87\xfc\x62\xf1\x52\x33\x6e\x89\x3e\x7c\x72\x27\xd5",
            &[
                ("        ) | (\n            \"src/cleanup_plan/replay/lexical_replay.rs\",\n            \"52a1b94530881bd2f59d873a71663fcd1656a9a2401440bc4f8edb69839fbd7f\"\n        ) | (\n            \"src/cleanup_plan/replay/lexical_replay/operand_tests/source.rs\",\n            \"575f7461fb6624f8dfdf1ed511cbade1227a47358abd0858755396d949e2931b\"\n", ""),
            ],
        ),
        _ => return current.to_owned(),
    };
    assert_eq!(current.len(), length, "complete later ENTRY helper extent: {source}");
    assert_eq!(digest(current.as_bytes()), current_sha, "complete later ENTRY helper source: {source}");
    let mut before = current.to_owned();
    for (after, prior) in edits {
        assert_eq!(before.matches(after).count(), 1, "exact later ENTRY helper edit: {source}");
        before = before.replacen(after, prior, 1);
    }
    assert_eq!(before.len(), old_length, "complete prior ENTRY helper extent: {source}");
    assert_eq!(digest(before.as_bytes()), old_sha, "complete prior ENTRY helper body: {source}");
    before
}

// RUNTIME848_THREE_CONTROLS_BEGIN
// Only these fixed DATA acquisitions change. Every historical source assertion
// below receives its complete original input; nothing is read under a compiler
// allocation or budget probe and no parsed/verdict cache is introduced.
fn complete_three_fixture_runtime_glue(source: &str, current: &str) -> String {
    let transport;
    let current = if source == "src/source_reflows_merge/runtime_json.rs" {
        transport = super::super::main_e177_final_inputs::value_direct(
            source,
            "56ec9fc1ede65a5610ac46f2d82d4a26ac83a8e4ece2b4226bbc206e111b926e",
            current,
        );
        assert_eq!(transport.len(), 26084);
        transport.as_str()
    } else {
        current
    };
    let restored = complete_current_writer_signature_entry_before(source, current);
    let current = restored.as_str();
    let restored = complete_embedded_fixture_parent_before(source, current);
    let current = restored.as_str();
    let restored = complete_projection_entry_followup_glue(source, current);
    let current = restored.as_str();
    if source != "src/source_reflows_merge/runtime_json.rs" {
        return current.to_owned();
    }
    assert_eq!(current.len(), 20520);
    assert_eq!(
        digest(current.as_bytes()),
        *b"\x80\x87\x4f\xca\x48\x53\xbc\x67\xef\xe4\x40\x62\x74\x3d\x65\x62\xaa\xf0\xf8\x2f\x6f\x8f\x11\xc2\x89\xde\x27\x83\xc4\x7e\x08\x55"
    );
    let mut before = current.to_owned();
    for (start, end) in [
        (
            "    // RUNTIME848_THREE_VARIANTS_BEGIN\n",
            "    // RUNTIME848_THREE_VARIANTS_END\n",
        ),
        (
            "            // RUNTIME848_THREE_SPECS_BEGIN\n",
            "            // RUNTIME848_THREE_SPECS_END\n",
        ),
        (
            "    // RUNTIME848_THREE_SOURCE_BEGIN\n",
            "    // RUNTIME848_THREE_SOURCE_END\n",
        ),
    ] {
        assert_eq!(before.matches(start).count(), 1);
        assert_eq!(before.matches(end).count(), 1);
        let first = before.find(start).unwrap();
        let last = before.find(end).unwrap() + end.len();
        assert!(first < last);
        before.replace_range(first..last, "");
    }
    assert_eq!(before.len(), 16413);
    assert_eq!(
        digest(before.as_bytes()),
        *b"\x73\x1f\x28\x59\x97\x57\xa0\x90\xce\x12\xc1\x1d\xa1\x08\xcc\x0a\xea\x5b\x3e\x3e\x18\x0a\xc0\xbb\x5a\xd5\x6d\x53\x16\x5f\xfd\xec"
    );
    before
}

fn three_fixture_test_source_before(current: &str) -> String {
    let marker = "\n// RUNTIME848_THREE_CONTROLS_BEGIN\n";
    assert_eq!(current.matches(marker).count(), 1);
    assert!(current.ends_with("// RUNTIME848_THREE_CONTROLS_END\n"));
    let (original, _) = current.split_once(marker).unwrap();
    let mut before = original.to_owned();
    let hook = "    let three_before = complete_three_fixture_runtime_glue(source, current);\n    let current = three_before.as_str();\n";
    assert_eq!(before.matches(hook).count(), 1);
    before = before.replacen(hook, "", 1);
    assert_eq!(before.len(), 62885);
    assert_eq!(
        digest(before.as_bytes()),
        *b"\xb5\x6d\x7b\xac\x24\x99\xc4\x23\x57\x12\x4c\x54\x5e\xfa\x6b\xb8\x53\x92\x2c\xd5\x1c\xab\x4d\x36\x28\xeb\xed\x44\xb3\xba\x78\x59"
    );
    before
}

#[test]
fn three_runtime_packets_keep_whole_consumers_and_refuse_each_fresh_hostile_read() {
    let directory = std::env::temp_dir().join(format!(
        "semaprax-three-history-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::create_dir(&directory).unwrap();
    let directory = Directory(directory);
    let path = directory.0.join("fixture.json");
    for (fixture, row_count, current, length, sha, old_length, old_sha, edits) in [
        (
            ReceiverFixture::RegionEntry,
            6,
            include_str!(
                "../../cleanup_plan/replay/expression_facts/receiving/regions_entry/tests.rs"
            ),
            22278,
            *b"\x61\x69\x59\x18\x3d\x4e\x46\x3e\xf2\x57\xe3\xdf\x5a\xfd\x21\x17\x0e\x2b\xae\x72\xb2\xc8\x90\x51\x3f\xa7\x51\x69\xff\xad\x76\x2d",
            22284,
            *b"\xe7\xfd\xd3\xd4\x11\xb2\x60\xb0\x67\xe4\x6a\x4a\x61\xb4\x6b\x24\x46\xa0\xd3\x07\x0c\x64\x71\x75\x3b\x57\xc8\x79\x3d\xfb\x62\xa8",
            &[(
                "    let rows: serde_json::Value =\n        serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(\n            crate::source_reflows_merge::ReceiverFixture::RegionEntry,\n        ))\n        .unwrap();",
                "    let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(\n        env!(\"CARGO_MANIFEST_DIR\"),\n        \"/docs/evidence/region-status-import-view-d4-20261015/runtime-views.json\"\n    )))\n    .unwrap();",
            )][..],
        ),
        (
            ReceiverFixture::DiagnosticWriter,
            10,
            include_str!("../../diagnostic/writer/tests.rs"),
            12196,
            *b"\xbe\xc4\x6f\x81\x95\x0a\xf2\x57\x62\x52\xc6\x62\x87\x46\x02\x6c\xc6\x35\xc4\xc8\x18\x57\x15\x64\x3e\xb0\xca\x4d\x15\x7b\x92\xeb",
            12183,
            *b"\x9b\xb4\xd3\x7e\xf0\x18\x99\x23\x38\x45\x9d\x90\x5c\x75\x58\x1f\x7e\x42\x14\x87\x50\x88\x0d\xfc\x8b\xf6\x86\xf3\xfb\x67\x64\xf0",
            &[(
                "    let rows: serde_json::Value =\n        serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(\n            crate::source_reflows_merge::ReceiverFixture::DiagnosticWriter,\n        ))\n        .unwrap();",
                "    let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(\n        env!(\"CARGO_MANIFEST_DIR\"),\n        \"/docs/evidence/diagnostic-writer-846/source-inverses.json\"\n    )))\n    .unwrap();",
            )][..],
        ),
        (
            ReceiverFixture::SignatureRewrite,
            7,
            include_str!("../../workspace_graph/expected_projection/type_rewrite/receiving.rs"),
            28277,
            *b"\xc6\x11\x7e\x98\x19\xb7\xfe\xab\x1a\x53\x08\xdb\x09\x47\x03\x6b\xb0\xc2\x7d\x2d\x8b\x62\xfb\xb8\x32\x2f\xc5\xdd\xee\xc7\x7f\x66",
            28245,
            *b"\xeb\xf3\xd8\x2b\xcd\x0c\x46\xc3\xc7\xb8\x6c\x2b\x37\xc2\x4d\x1e\x0f\x94\x2e\xe3\xfe\xdd\xae\x85\xd6\x3a\xed\x23\x78\x6d\xc7\x18",
            &[
                (
                    "        let rows: serde_json::Value =\n            serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(\n                crate::source_reflows_merge::ReceiverFixture::SignatureRewrite,\n            ))\n            .unwrap();",
                    "        let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(\n            env!(\"CARGO_MANIFEST_DIR\"),\n            \"/docs/evidence/constructor-signature-rewrite-825/source-inverses.json\"\n        )))\n        .unwrap();",
                ),
                (
                    "        let current_receiver =\n            crate::source_reflows_merge::ReceiverFixture::signature_rewrite_source_before(\n                include_str!(\"receiving.rs\"),\n            );\n",
                    "",
                ),
                (
                    "current_receiver.as_str()",
                    "include_str!(concat!(\n                    env!(\"CARGO_MANIFEST_DIR\"),\n                    \"/src/workspace_graph/expected_projection/type_rewrite/receiving.rs\"\n                ))",
                ),
            ][..],
        ),
    ] {
        let source = match fixture {
            ReceiverFixture::RegionEntry => "src/cleanup_plan/replay/expression_facts/receiving/regions_entry/tests.rs",
            ReceiverFixture::DiagnosticWriter => "src/diagnostic/writer/tests.rs",
            ReceiverFixture::SignatureRewrite => "src/workspace_graph/expected_projection/type_rewrite/receiving.rs",
            _ => unreachable!(),
        };
        let prior = embedded_acquisition_source_before(source, current)
            .expect("each whole compiled consumer has one closed restored acquisition");
        assert!(std::panic::catch_unwind(|| embedded_acquisition_source_before(source, &prior)).is_err(),
            "already-runtime is not the new embedded current source");
        assert!(embedded_acquisition_source_before("foreign/source.rs", current).is_none());
        for foreign in [format!("{current} "), current[..current.len() - 1].to_owned(),
            current.replacen("use ", "use  ", 1)] {
            assert_ne!(foreign, current);
            assert!(std::panic::catch_unwind(|| embedded_acquisition_source_before(source, &foreign)).is_err());
        }
        let restored = complete_projection_entry_followup_glue(source, current);
        let current = restored.as_str();
        let spec = fixture.spec();
        assert!(spec.length <= OUTPUT_CAP);
        let payload = receiver_fixture(fixture);
        assert_eq!(payload.len(), spec.length);
        assert_eq!(digest(payload.as_bytes()), spec.sha256);
        let rows: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(rows.as_array().unwrap().len(), row_count);
        assert_eq!(current.len(), length);
        assert_eq!(digest(current.as_bytes()), sha);
        let restore = |subject: &str| {
            assert_eq!(subject.len(), length);
            assert_eq!(digest(subject.as_bytes()), sha);
            let mut before = subject.to_owned();
            for &(after, prior) in edits {
                assert_eq!(before.matches(after).count(), 1);
                before = before.replacen(after, prior, 1);
            }
            assert_eq!(before.len(), old_length);
            assert_eq!(digest(before.as_bytes()), old_sha);
            before
        };
        drop(restore(current));
        for foreign in [
            format!("{current} "),
            current[..current.len() - 1].to_owned(),
            current.replacen("use ", "use  ", 1),
        ] {
            assert_ne!(foreign, current);
            assert!(std::panic::catch_unwind(|| restore(&foreign)).is_err());
        }
        assert_eq!(
            read_fixture(&directory.0.join("missing"), spec.length, spec.sha256),
            Err(Failure::Input)
        );
        std::fs::write(&path, &payload).unwrap();
        assert_eq!(
            read_fixture(&path, spec.length, spec.sha256).unwrap(),
            payload
        );
        std::fs::write(&path, &payload.as_bytes()[..spec.length - 1]).unwrap();
        assert_eq!(
            read_fixture(&path, spec.length, spec.sha256),
            Err(Failure::Bound)
        );
        let mut altered = payload.as_bytes().to_vec();
        altered.push(b' ');
        std::fs::write(&path, &altered).unwrap();
        assert_eq!(
            read_fixture(&path, spec.length, spec.sha256),
            Err(Failure::Bound)
        );
        altered.pop();
        altered[0] ^= 1;
        std::fs::write(&path, &altered).unwrap();
        assert_eq!(
            read_fixture(&path, spec.length, spec.sha256),
            Err(Failure::Digest)
        );
        std::fs::write(&path, &payload).unwrap();
        assert_eq!(
            read_fixture(&path, spec.length, spec.sha256).unwrap(),
            payload
        );
    }
    let signature =
        include_str!("../../workspace_graph/expected_projection/type_rewrite/receiving.rs");
    drop(ReceiverFixture::signature_rewrite_source_before(signature));
    for foreign in [
        format!("{signature} "),
        signature.replacen("use ", "uSe ", 1),
    ] {
        assert_ne!(foreign, signature);
        assert!(
            std::panic::catch_unwind(|| ReceiverFixture::signature_rewrite_source_before(&foreign))
                .is_err()
        );
    }
    let runtime = include_str!("../runtime_json.rs");
    drop(complete_three_fixture_runtime_glue(
        "src/source_reflows_merge/runtime_json.rs",
        runtime,
    ));
    assert!(
        std::panic::catch_unwind(|| complete_three_fixture_runtime_glue(
            "src/source_reflows_merge/runtime_json.rs",
            &format!("{runtime} "),
        ))
        .is_err()
    );
    let root = include_str!("../../source_reflows_merge.rs");
    let prior = complete_current_writer_signature_entry_before("src/source_reflows_merge.rs", root);
    assert_ne!(root, prior);
    for foreign in [
        format!("{root} "),
        root[..root.len() - 1].to_owned(),
        root.replacen(
            "fn historical_project_entry",
            "fn historical_project_Entry",
            1,
        ),
        prior,
    ] {
        assert!(
            std::panic::catch_unwind(|| complete_current_writer_signature_entry_before(
                "src/source_reflows_merge.rs",
                &foreign
            ))
            .is_err()
        );
    }
    let diagnostic = include_str!("../../diagnostic.rs");
    let prior = super::super::historical_project_entry(
        "src/diagnostic.rs",
        "922f3150549194a6016b86d86fe29dc56a9fb6a23384032b1afc144b248b782e",
        diagnostic,
    );
    assert_eq!(prior.len(), 11582);
    assert_eq!(
        super::super::diagnostic846_input("src/diagnostic.rs", diagnostic),
        prior,
        "the actual writer endpoint must enter the complete unchanged old pipeline",
    );
    for foreign in [format!("{diagnostic} "), prior] {
        assert!(
            std::panic::catch_unwind(|| super::super::diagnostic846_input(
                "src/diagnostic.rs",
                &foreign,
            ))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(|| super::super::historical_project_entry(
                "src/diagnostic.rs",
                "922f3150549194a6016b86d86fe29dc56a9fb6a23384032b1afc144b248b782e",
                &foreign,
            ))
            .is_err()
        );
    }
    for (source, expected, compiled) in [
        (
            "src/workspace_graph/diagnostics.rs",
            "1c1552d56400ec781eb96ff6fd825bbbeec854f5042fab960e4d3d2941e60127",
            include_str!("../../workspace_graph/diagnostics.rs"),
        ),
        (
            "src/workspace_graph/constructor_failure/typed.rs",
            "1482ad177d1d605a183cccc3ef185aaed36e7214b1bf4db8065a466303fc99f6",
            include_str!("../../workspace_graph/constructor_failure/typed.rs"),
        ),
        (
            "src/workspace_graph/expected_projection/defaults/typed.rs",
            "9761dcd2ec5fd2351807f7526479680a56c51211eb584385ca85dbc893f38fb7",
            include_str!("../../workspace_graph/expected_projection/defaults/typed.rs"),
        ),
        (
            "src/workspace_graph/expected_projection/import_stub/typed.rs",
            "c220a90bfa131f6b75131e461ead26aea1dcee6c3ad09c42e5f481f5c12c5b11",
            include_str!("../../workspace_graph/expected_projection/import_stub/typed.rs"),
        ),
        (
            "src/workspace_graph/constructor_failure/report.rs",
            "fc2eb1ecc98cc13b6df906a1e2f59f09d198cf958a4c7ff3472e0aae1a866a84",
            include_str!("../../workspace_graph/constructor_failure/report.rs"),
        ),
        (
            "src/workspace_graph/constructor_failure/receiving/typed/tests.rs",
            "2cd6c2ce3b35abb30253549dc931c6c025653419d2bd16b9fb6c35224a34d6e4",
            include_str!("../../workspace_graph/constructor_failure/receiving/typed/tests.rs"),
        ),
        (
            "src/workspace_graph/expected_projection/defaults/inline_tests.rs",
            "1fa04bba6279f410439f819afeb9cbe0b9e33c43d6af56550e1cd39ade0e7ead",
            include_str!("../../workspace_graph/expected_projection/defaults/inline_tests.rs"),
        ),
        (
            "src/workspace_graph/constructor_failure/receiving/before.rs",
            "f07589cda30b524062f7ad4dcb87cfc1244b2746729b2b1e5cd0e84320786aed",
            include_str!("../../workspace_graph/constructor_failure/receiving/before.rs"),
        ),
        (
            "src/workspace_graph/constructor_failure/receiving/before.rs",
            "6135d75cf3f57029b9dd754030aba155a75ca902772aa036345481c1d3abc8b1",
            include_str!("../../workspace_graph/constructor_failure/receiving/before.rs"),
        ),
    ] {
        let prior = super::super::historical_project_entry(source, expected, compiled);
        assert_ne!(prior, compiled);
        for foreign in [
            format!("{compiled} "),
            compiled[..compiled.len() - 1].to_owned(),
            prior,
        ] {
            if source == "src/workspace_graph/constructor_failure/receiving/before.rs"
                && expected == "6135d75cf3f57029b9dd754030aba155a75ca902772aa036345481c1d3abc8b1"
                && foreign.len() == 14489
                && format!(
                    "{:x}",
                    crate::digest_hex::LowerHex(digest(foreign.as_bytes()))
                ) == expected
            {
                assert_eq!(
                    super::super::historical_project_entry(source, expected, &foreign),
                    foreign,
                    "exact authenticated intermediate remains a valid projector input"
                );
                assert!(
                    std::panic::catch_unwind(|| super::super::constructor825_compiled_entry(
                        source, expected, &foreign
                    ))
                    .is_err()
                );
            } else {
                assert!(
                    std::panic::catch_unwind(|| super::super::historical_project_entry(
                        source, expected, &foreign
                    ))
                    .is_err()
                );
            }
        }
    }
    let attempt = include_str!("../../workspace_graph/constructor_failure/attempt.rs");
    let old_attempt = super::super::constructor825_attempt_before(attempt);
    let prior = super::super::setup_typed_input(
        "src/workspace_graph/constructor_failure/attempt.rs",
        attempt,
    );
    assert_eq!(
        digest(prior.as_bytes()),
        *b"\xd7\x07\x00\xd1\x0c\xea\xb7\xa6\x74\x0e\x2a\x90\xd4\x6c\xf4\xd1\x08\x88\xb9\x8b\x06\xf5\x91\xa8\x0c\xda\x96\x0f\x73\xa2\x6d\x70"
    );
    for foreign in [format!("{attempt} "), old_attempt] {
        assert!(std::panic::catch_unwind(|| super::super::setup_typed_input(
            "src/workspace_graph/constructor_failure/attempt.rs",
            &foreign
        ))
        .is_err());
    }
    for (source, compiled) in [
        (
            "src/bounded_output.rs",
            include_str!("../../bounded_output.rs"),
        ),
        (
            "src/workspace_graph/initial_report/build.rs",
            include_str!("../../workspace_graph/initial_report/build.rs"),
        ),
        (
            "src/workspace_graph/initial_report/tests.rs",
            include_str!("../../workspace_graph/initial_report/tests.rs"),
        ),
    ] {
        let prior = super::super::initial845_current_owner_before(source, compiled).unwrap();
        assert_ne!(prior, compiled);
        for foreign in [
            format!("{compiled} "),
            compiled[..compiled.len() - 1].to_owned(),
            prior,
        ] {
            assert!(
                std::panic::catch_unwind(|| super::super::initial845_input(source, &foreign))
                    .is_err()
            );
        }
    }
    let ledger = include_str!("../../bounded_output.rs");
    let old_ledger =
        super::super::initial845_current_owner_before("src/bounded_output.rs", ledger).unwrap();
    let entry =
        super::super::initial845_ledger_entry("src/bounded_output.rs", &old_ledger).unwrap();
    assert_eq!(entry.len(), 50190);
    assert_eq!(
        digest(entry.as_bytes()),
        *b"\xd0\x1a\x82\xd6\x02\xc2\x86\x04\x4c\xc1\xd5\xdf\x8d\x05\xd0\x2d\xfe\x51\x68\xd1\x13\x9a\x82\x38\x0a\xa0\x1b\x06\xe4\x7c\x7f\xd5"
    );
    assert!(super::super::initial845_ledger_entry("foreign/source.rs", &old_ledger).is_none());
    for foreign in [
        format!("{old_ledger} "),
        old_ledger[..old_ledger.len() - 1].to_owned(),
        entry,
    ] {
        assert!(
            std::panic::catch_unwind(|| super::super::initial845_ledger_entry(
                "src/bounded_output.rs",
                &foreign
            ))
            .is_err()
        );
    }
    {
        let source = "src/workspace_graph/initial_report/before.rs";
        let actual = include_str!("../../workspace_graph/initial_report/before.rs");
        let acquired = super::super::initial845_current_owner_before(source, actual);
        let current = acquired.as_deref().unwrap_or(actual);
        let prior = super::super::initial845_compiled_owner_entry(source, current).unwrap();
        assert_eq!(prior.len(), 17445);
        assert_eq!(
            digest(prior.as_bytes()),
            *b"\x5c\x7f\x71\x67\x0f\xb7\xbf\xb0\xff\xd7\x70\x71\xfc\xfa\x60\xc2\x42\x30\x69\x2e\x31\x06\x44\xce\x47\x26\x3a\x59\xd4\x3a\xb8\x7f"
        );
        for foreign in [
            format!("{current} "),
            current[..current.len() - 1].to_owned(),
            prior,
        ] {
            assert!(
                std::panic::catch_unwind(|| super::super::initial845_compiled_owner_entry(
                    source, &foreign
                ))
                .is_err()
            );
        }
    }
    {
        let source = "src/workspace_graph/initial_report/before_mappers.rs";
        let actual = include_str!("../../workspace_graph/initial_report/before_mappers.rs");
        let acquired = super::super::initial845_current_owner_before(source, actual);
        let current = acquired.as_deref().unwrap_or(actual);
        let prior = super::super::initial845_compiled_owner_entry(source, current).unwrap();
        assert_eq!(prior.len(), 4109);
        assert_eq!(
            digest(prior.as_bytes()),
            *b"\xd7\x36\xb5\xa9\x2c\xaa\x2a\x35\x5e\x5f\xa1\xa2\x11\x24\x69\x29\x72\x72\xaf\xd6\x73\xb7\x04\xc4\x19\x52\x24\xe7\x82\xeb\x3e\x1d"
        );
        for foreign in [
            format!("{current} "),
            current[..current.len() - 1].to_owned(),
            prior,
        ] {
            assert!(
                std::panic::catch_unwind(|| super::super::initial845_compiled_owner_entry(
                    source, &foreign
                ))
                .is_err()
            );
        }
    }
    {
        let source = "src/workspace_graph/initial_report/build.rs";
        let actual = include_str!("../../workspace_graph/initial_report/build.rs");
        let acquired = super::super::initial845_current_owner_before(source, actual);
        let current = acquired.as_deref().unwrap_or(actual);
        let prior = super::super::initial845_compiled_owner_entry(source, current).unwrap();
        assert_eq!(prior.len(), 16803);
        assert_eq!(
            digest(prior.as_bytes()),
            *b"\x60\x7a\xf6\xca\xb2\x43\xe1\x97\x0a\xed\x32\x6c\x8d\xc7\xce\x7c\x21\x1f\xfe\x90\x2f\x4e\x42\x54\x67\x59\x95\x38\xae\xf0\x13\xd1"
        );
        for foreign in [
            format!("{current} "),
            current[..current.len() - 1].to_owned(),
            prior,
        ] {
            assert!(
                std::panic::catch_unwind(|| super::super::initial845_compiled_owner_entry(
                    source, &foreign
                ))
                .is_err()
            );
        }
    }
    let live_controls = include_str!("tests.rs");
    let controls = historical_inputs::before(live_controls);
    drop(three_fixture_test_source_before(&controls));
    for foreign in [
        format!("{live_controls} "),
        controls.clone(),
        live_controls[..live_controls.len() - 1].to_owned(),
    ] {
        assert!(std::panic::catch_unwind(|| historical_inputs::before(&foreign)).is_err());
    }
    assert!(
        std::panic::catch_unwind(|| three_fixture_test_source_before(&format!("{controls} ")))
            .is_err()
    );
}

// Each fixed complete compiled source is restored before its unchanged transport inverse.
fn complete_projection_entry_followup_glue(source: &str, current: &str) -> String {
    let restored =
        embedded_acquisition_source_before(source, current).unwrap_or_else(|| current.to_owned());
    let current = restored.as_str();
    let (length, sha, old_length, old_sha, edits): ReceiverSpanEdits<'_> = match source {
        "src/cleanup_plan/replay/expression_facts/receiving/regions_entry/tests.rs" => (30430, *b"\x06\x76\x9d\x45\x55\xe1\x8d\x5b\xaa\xfb\xad\x36\x23\xd7\x29\x00\x4c\x0a\xf6\x11\x71\x34\x28\xdd\x29\x2f\x70\x17\xce\xff\x35\xf4", 22278, *b"\x61\x69\x59\x18\x3d\x4e\x46\x3e\xf2\x57\xe3\xdf\x5a\xfd\x21\x17\x0e\x2b\xae\x72\xb2\xc8\x90\x51\x3f\xa7\x51\x69\xff\xad\x76\x2d", &[
            (21158, 22438, "expected_entry = match source {\n        \"src/cleanup_plan/replay.rs\" => {\n            Some(\"cce98dd37682ccb93081bae2eeb53ea103562370461844c12d16a59cb732d31b\")\n        }\n        \"src/cleanup_plan/replay/lexical_replay.rs\" => {\n            Some(\"d0f2b7fcfc3815f0c1207c064d8bcb14d4323dff614d953988b962138ff975d0\")\n        }\n        \"src/cleanup_plan/replay/expression_facts/receiving.rs\" => {\n            Some(\"db822f51812769eadf1570bfee4b315ceadd31b689fc6efa643e5da03d7a9bb7\")\n        }\n        \"src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs\" => {\n            Some(\"112b1a3f5e47c5d0c99c476c2374e7eb9dbfdc04d9220a94f6bf73b613ee294e\")\n        }\n        _ => None,\n    };\n    let entry = match expected_entry {\n        Some(expected) => {\n            let entry = crate::source_reflows_merge::main_e177_final_inputs::value_direct(\n                source, expected, current,\n            );\n            assert_eq!(\n                format!(\n                    \"{:x}\",\n                    crate::digest_hex::LowerHex(Sha256::digest(entry.as_bytes()))\n                ),\n                expected,\n                \"complete region-entry historical input: {source}\"\n            );\n            entry\n        }\n        None => current.to_owned(),\n    };\n    let ", ""),
            (22569, 22570, "&", "curr"),
            (22573, 22575, "ry", ""),
            (22596, 22598, "di", "cur"),
            (22600, 22601, "c", "n"),
            (22602, 22619, "ory_before_lookup", ""),
            (22622, 22668, "region_entry_directory_before_lookup(source, &", ""),
            (22678, 22722, ");\n    let current = directory_before_lookup", ""),
            (23663, 30430, "\n// The existing OPT885 inverse runs first. This exact later lookup-stage inverse\n// then supplies its returned bytes to every unchanged original d4 row/hook.\n#[rustfmt::skip]\nfn region_entry_directory_before_lookup(source: &str, current: &str) -> String {\n    use sha2::{Digest, Sha256};\n    if source != \"src/cleanup_plan/replay/expression_facts/receiving/directory.rs\" {\n        return current.to_owned();\n    }\n    assert_eq!(current.len(), 20833);\n    assert_eq!(format!(\"{:x}\", crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))),\n        \"71bddf2e91cd7b6768523035ce49c111e41686dc5104adbcdb8dd7165675e464\");\n    let mut restored = current.to_owned();\n    for added in [\n        \"    fn status_key(&self) -> Option<&StatusSourceId> {\\n        None\\n    }\\n\",\n        \"    fn status_key(&self) -> Option<&StatusSourceId> {\\n        Some(self)\\n    }\\n\",\n        \"    fn status_key(&self) -> Option<&StatusSourceId> {\\n        T::status_key(self)\\n    }\\n\",\n        \"    lookup: Option<Box<status_prefix::FrozenStatusIndex>>,\\n\",\n        \"\\nstruct LookupCursor {\\n    at: usize,\\n    byte: usize,\\n    mask: u16,\\n    symbol: u16,\\n    direction: bool,\\n    units: usize,\\n}\\n// Complete initialized cursor plus receiving state, status-key Option and\\n// borrowed receivers, final candidate/ordering and comparison temporaries.\\nconst LOOKUP_BYTES: usize = 2 * std::mem::size_of::<LookupCursor>()\\n    + 2 * std::mem::size_of::<Option<&StatusSourceId>>()\\n    + 4 * std::mem::size_of::<&StatusSourceId>()\\n    + std::mem::size_of::<&status_prefix::FrozenStatusIndex>()\\n    + 2 * std::mem::size_of::<Option<usize>>()\\n    + std::mem::size_of::<usize>()\\n    + std::mem::size_of::<Ordering>();\\n\",\n        \"    #[cfg(test)]\\n    pub(super) fn has_frozen_lookup_for_test(&self) -> bool {\\n        self.lookup.is_some()\\n    }\\n\\n\",\n        \"        // The new optional field has two initialized receiving slots. Existing\\n        // directory fields and reservation requests receive no credit.\\n        budget.reserve_bytes(\\n            function,\\n            2 * std::mem::size_of::<Option<Box<status_prefix::FrozenStatusIndex>>>(),\\n            phase,\\n        )?;\\n        budget.charge(function, 1, phase)?;\\n\",\n        \"            lookup: None,\\n\",\n        \"        if let Some(index) = &self.lookup {\\n            // Binding checks read no foreign payer or cached key. A rejected\\n            // receiver remains rejected even after its original owner returns.\\n            if index.failed.get() {\\n                return Err(Diagnostic::io(\\\"SPX-H006\\\", String::new()));\\n            }\\n            if !std::ptr::eq(index.function, function)\\n                || !std::ptr::eq(index.budget, budget)\\n                || !index.ledger.current()\\n                || index.keys != self.nodes.as_ptr() as usize\\n                || index.len != self.nodes.len()\\n            {\\n                index.failed.set(true);\\n                return Err(Diagnostic::io(\\\"SPX-H006\\\", String::new()));\\n            }\\n            self.authenticate(function, budget)?;\\n            // All traversal state is one fixed receiving frame. Loop steps\\n            // update its paid fields rather than constructing a new path.\\n            budget.reserve_bytes(function, LOOKUP_BYTES, self.phase)?;\\n            budget.charge(function, 24, self.phase)?;\\n            let Some(query) = key.status_key() else {\\n                index.failed.set(true);\\n                return Err(Diagnostic::io(\\\"SPX-H006\\\", String::new()));\\n            };\\n            let mut cursor = LookupCursor {\\n                at: index.root,\\n                byte: 0,\\n                mask: 0,\\n                symbol: 0,\\n                direction: false,\\n                units: 0,\\n            };\\n            cursor.byte = index.nodes[cursor.at].byte;\\n            while cursor.byte != NIL {\\n                budget.charge(function, VISIT_UNITS + 24, self.phase)?;\\n                cursor.mask = index.nodes[cursor.at].mask;\\n                cursor.symbol = if cursor.byte < query.expression.as_str().len() {\\n                    256 | query.expression.as_str().as_bytes()[cursor.byte] as u16\\n                } else if cursor.byte == query.expression.as_str().len() {\\n                    match query.lane {\\n                        StatusLane::OwnerAdmission => 0,\\n                        StatusLane::OperationFailure => 1,\\n                        StatusLane::AllocationFailure => 2,\\n                        StatusLane::ContractFalse => 3,\\n                    }\\n                } else {\\n                    0\\n                };\\n                cursor.direction = cursor.symbol & cursor.mask != 0;\\n                cursor.at = if cursor.direction {\\n                    index.nodes[cursor.at].one\\n                } else {\\n                    index.nodes[cursor.at].zero\\n                };\\n                cursor.byte = index.nodes[cursor.at].byte;\\n            }\\n            budget.charge(function, VISIT_UNITS + 2, self.phase)?;\\n            cursor.at = index.nodes[cursor.at].zero;\\n            let candidate = &self.nodes[cursor.at];\\n            // The index selects only a candidate. Preserve the complete old\\n            // key-comparison request and actual Ord equality, without a prefix\\n            // discount, identifier parsing, hashes or numeric-ID authority.\\n            cursor.units = match key\\n                .comparison_units(&candidate.key)\\n                .and_then(|units| units.checked_add(VISIT_UNITS))\\n            {\\n                Some(units) => units,\\n                None => {\\n                    return Err(overflow(\\n                        function,\\n                        budget,\\n                        \\\"directory key comparison work overflow\\\",\\n                    ))\\n                }\\n            };\\n            budget.charge(function, cursor.units, self.phase)?;\\n            return Ok(Ord::cmp(key, &candidate.key) == Ordering::Equal);\\n        }\\n\",\n        \"        if let Some(index) = &self.lookup {\\n            // Publication seals this privately owned directory. No mutation or\\n            // fallback can revive a rejected read-only index.\\n            index.failed.set(true);\\n            return Err(Diagnostic::io(\\\"SPX-H006\\\", String::new()));\\n        }\\n\",\n        \"\\n#[cfg(test)]\\nmod before_cursor;\\n\",\n\n    ] {\n        assert_eq!(restored.matches(added).count(), 1, \"complete unique directory lookup-stage inverse\");\n        restored = restored.replacen(added, \"\", 1);\n    }\n    assert_eq!(restored.len(), 15428);\n    assert_eq!(format!(\"{:x}\", crate::digest_hex::LowerHex(Sha256::digest(restored.as_bytes()))),\n        \"d161622003e500b3b9197c0f463b88eee388d9447c5a87ba75f464d3aa2a59cc\");\n    restored\n}\n", ""),
        ]),
        "src/source_reflows_merge/runtime_json.rs" => (21463, *b"\x24\x83\xea\xfd\xc7\xd6\x4e\xff\xed\x01\xa3\x7b\xf8\x93\xb2\x77\xf6\xc5\x7c\xc3\xed\xcc\x35\x3e\xe7\x4e\xee\xe5\x7c\x83\x2a\x18", 20520, *b"\x80\x87\x4f\xca\x48\x53\xbc\x67\xef\xe4\x40\x62\x74\x3d\x65\x62\xaa\xf0\xf8\x2f\x6f\x8f\x11\xc2\x89\xde\x27\x83\xc4\x7e\x08\x55", &[
            (9360, 10303, "\n        );\n        // The whole transport predecessor above is unchanged. Re-enter its\n        // exact pre-formatter consumer stage before every original projector.\n        let formatted = \"            let projected = crate::source_reflows_merge::consumer_input(\\n                \\\"signature825\\\",\\n                row[\\\"source\\\"].as_str().unwrap(),\\n                current,\\n            );\";\n        let original = \"            let projected = crate::source_reflows_merge::consumer_input(\\\"signature825\\\", row[\\\"source\\\"].as_str().unwrap(), current);\";\n        assert_eq!(before.matches(formatted).count(), 1);\n        before = before.replacen(formatted, original, 1);\n        assert_eq!(before.len(), 28182);\n        assert_eq!(\n            format!(\n                \"{:x}\",\n                crate::digest_hex::LowerHex(digest(before.as_bytes()))\n            ),\n            \"b5eb131534d7bbd10d95aa2e8eeeafd3efa7d55fde34b5ab8fa0a24fbf396062\"", ""),
        ]),
        _ => return current.to_owned(),
    };
    assert_eq!(current.len(), length);
    assert_eq!(digest(current.as_bytes()), sha);
    let mut before = current.to_owned();
    for &(start, end, after, prior) in edits.iter().rev() {
        assert!(start <= end && end <= before.len());
        assert!(before.is_char_boundary(start) && before.is_char_boundary(end));
        assert_eq!(&before[start..end], after);
        before.replace_range(start..end, prior);
    }
    assert_eq!(before.len(), old_length);
    assert_eq!(digest(before.as_bytes()), old_sha);
    before
}

// The facade itself keeps every original receiver. This full-current front
// removes only the new closed consumer front before all unchanged predecessors.
fn complete_embedded_fixture_parent_before(source: &str, current: &str) -> String {
    if source != "src/source_reflows_merge/runtime_json.rs" {
        return current.to_owned();
    }
    assert_eq!(current.len(), 26084);
    assert_eq!(
        digest(current.as_bytes()),
        *b"\x56\xec\x9f\xc1\xed\xe6\x5a\x56\x10\xac\x46\xf2\xd8\x2d\x4a\x26\xac\x83\xa8\xe4\xec\xe2\xb4\x22\x6b\xbc\x20\x6e\x11\x1b\x92\x6e"
    );
    let mut before = current.to_owned();
    let added = "\n// EMBEDDED848_THREE_CURRENT_BEGIN\n// Only the three evaluated acquisitions were restored. Authenticate each whole\n// compiled consumer before reconstructing its complete runtime-source stage;\n// every original transport inverse and historical assertion still follows.\nfn embedded_acquisition_source_before(source: &str, current: &str) -> Option<String> {\n    let (length, sha, old_length, old_sha, embedded, runtime): (\n        usize,\n        [u8; 32],\n        usize,\n        [u8; 32],\n        &str,\n        &str,\n    ) = match source {\n        \"src/cleanup_plan/replay/expression_facts/receiving/regions_entry/tests.rs\" => (\n            30436,\n            [\n                237, 115, 207, 242, 72, 24, 228, 75, 198, 221, 208, 179, 60, 28, 61, 102, 143, 8,\n                11, 1, 245, 253, 222, 0, 14, 31, 63, 135, 113, 217, 167, 142,\n            ],\n            30430,\n            [\n                6, 118, 157, 69, 85, 225, 141, 91, 170, 251, 173, 54, 35, 215, 41, 0, 76, 10, 246,\n                17, 113, 52, 40, 221, 41, 47, 112, 23, 206, 255, 53, 244,\n            ],\n            \"    let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(\\n        env!(\\\"CARGO_MANIFEST_DIR\\\"),\\n        \\\"/docs/evidence/region-status-import-view-d4-20261015/runtime-views.json\\\"\\n    )))\\n    .unwrap();\",\n            \"    let rows: serde_json::Value =\\n        serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(\\n            crate::source_reflows_merge::ReceiverFixture::RegionEntry,\\n        ))\\n        .unwrap();\",\n        ),\n        \"src/diagnostic/writer/tests.rs\" => (\n            12183,\n            [\n                155, 180, 211, 126, 240, 24, 153, 35, 56, 69, 157, 144, 92, 117, 88, 31, 126, 66,\n                20, 135, 80, 136, 13, 252, 139, 246, 134, 243, 251, 103, 100, 240,\n            ],\n            12196,\n            [\n                190, 196, 111, 129, 149, 10, 242, 87, 98, 82, 198, 98, 135, 70, 2, 108, 198, 53,\n                196, 200, 24, 87, 21, 100, 62, 176, 202, 77, 21, 123, 146, 235,\n            ],\n            \"    let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(\\n        env!(\\\"CARGO_MANIFEST_DIR\\\"),\\n        \\\"/docs/evidence/diagnostic-writer-846/source-inverses.json\\\"\\n    )))\\n    .unwrap();\",\n            \"    let rows: serde_json::Value =\\n        serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(\\n            crate::source_reflows_merge::ReceiverFixture::DiagnosticWriter,\\n        ))\\n        .unwrap();\",\n        ),\n        \"src/workspace_graph/expected_projection/type_rewrite/receiving.rs\" => (\n            28276,\n            [\n                22, 221, 107, 83, 203, 128, 52, 44, 220, 161, 220, 11, 112, 61, 14, 175, 0, 10,\n                191, 237, 97, 47, 8, 152, 187, 200, 254, 55, 207, 140, 29, 71,\n            ],\n            28277,\n            [\n                198, 17, 126, 152, 25, 183, 254, 171, 26, 83, 8, 219, 9, 71, 3, 107, 176, 194, 125,\n                45, 139, 98, 251, 184, 50, 47, 197, 221, 238, 199, 127, 102,\n            ],\n            \"        let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(\\n            env!(\\\"CARGO_MANIFEST_DIR\\\"),\\n            \\\"/docs/evidence/constructor-signature-rewrite-825/source-inverses.json\\\"\\n        )))\\n        .unwrap();\",\n            \"        let rows: serde_json::Value =\\n            serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(\\n                crate::source_reflows_merge::ReceiverFixture::SignatureRewrite,\\n            ))\\n            .unwrap();\",\n        ),\n        _ => return None,\n    };\n    assert_eq!(\n        current.len(),\n        length,\n        \"complete embedded consumer extent: {source}\"\n    );\n    assert_eq!(\n        digest(current.as_bytes()),\n        sha,\n        \"complete embedded consumer body: {source}\"\n    );\n    assert_eq!(\n        current.matches(embedded).count(),\n        1,\n        \"unique acquisition restoration: {source}\"\n    );\n    let before = current.replacen(embedded, runtime, 1);\n    assert_eq!(\n        before.len(),\n        old_length,\n        \"complete runtime consumer extent: {source}\"\n    );\n    assert_eq!(\n        digest(before.as_bytes()),\n        old_sha,\n        \"complete runtime consumer body: {source}\"\n    );\n    Some(before)\n}\n// EMBEDDED848_THREE_CURRENT_END\n";
    assert_eq!(before.matches(added).count(), 1);
    before = before.replacen(added, "", 1);
    let added = "        let prior = embedded_acquisition_source_before(\n            \"src/workspace_graph/expected_projection/type_rewrite/receiving.rs\",\n            current,\n        )\n        .expect(\"the signature self-source is a closed embedded consumer\");\n        let current = prior.as_str();\n";
    assert_eq!(before.matches(added).count(), 1);
    before = before.replacen(added, "", 1);
    assert_eq!(before.len(), 21463);
    assert_eq!(
        digest(before.as_bytes()),
        *b"\x24\x83\xea\xfd\xc7\xd6\x4e\xff\xed\x01\xa3\x7b\xf8\x93\xb2\x77\xf6\xc5\x7c\xc3\xed\xcc\x35\x3e\xe7\x4e\xee\xe5\x7c\x83\x2a\x18"
    );
    before
}
// This complete compiled root is authenticated before removing only the closed
// ENTRY fronts. Every original Native/receiver inverse and old assertion follows.
fn complete_current_writer_signature_entry_before(source: &str, current: &str) -> String {
    let restored = current64_complete_root_before(source, current);
    let current = restored.as_str();
    let restored = initial845_current_root_before(source, current);
    let restored = constructor825_attempt_root_before(source, &restored);
    let restored = constructor825_root_before(source, &restored);
    let restored = complete_diagnostic846_endpoint_before(source, &restored);
    let current = restored.as_str();
    if source != "src/source_reflows_merge.rs" {
        return current.to_owned();
    }
    assert_eq!(current.len(), 55950);
    assert_eq!(
        digest(current.as_bytes()),
        *b"\xde\x5a\xee\x83\x34\xae\xe4\xc6\x13\xf9\x95\x27\x3b\x4e\xbb\x86\xf1\x42\x58\x5f\xd1\x6f\x61\x79\x12\xca\x1b\xd4\x4c\x67\xf1\xc6"
    );
    let mut before = current.to_owned();
    let added = "            \"CHANGELOG.md\",\n            \"dabe64d8fc192968a47f494ebf1ace0de89092c213f615e077d7fe4bc90f2148\"\n        ) | (\n            \"docs/ARCHITECTURE.md\",\n            \"ebc8d9becb91cb72845e18825d3f49bec992e15c63459b6368305a532cb235c4\"\n        ) | (\n            \"src/workspace_graph/expected_projection/borrowed_imports/typed.rs\",\n            \"2532586630034d6c7f2953a1f487c34c25953a5b13dc86a2170bb7561a8fac36\"\n        ) | (\n";
    assert_eq!(before.matches(added).count(), 1);
    before = before.replacen(added, "", 1);
    let added = "    // This source is compiled DATA, not a canonical-row admission. Restore\n    // its exact formatter-only predecessor before the unchanged old projector.\n    if source == \"src/diagnostic.rs\"\n        && expected == \"922f3150549194a6016b86d86fe29dc56a9fb6a23384032b1afc144b248b782e\"\n    {\n        assert_eq!(current, include_str!(\"diagnostic.rs\"));\n        assert_eq!(current.len(), 11599);\n        assert_eq!(\n            format!(\n                \"{:x}\",\n                crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))\n            ),\n            \"028ee6f9753c7c70c88cf146f35f7b7c61c692d6b4035d3abc03260456eefaa1\"\n        );\n        let formatted = \"#[rustfmt::skip]\\nmod before_writer;\";\n        assert_eq!(current.matches(formatted).count(), 1);\n        let entry = current.replacen(formatted, \"mod before_writer;\", 1);\n        assert_eq!(entry.len(), 11582);\n        assert_eq!(\n            format!(\n                \"{:x}\",\n                crate::digest_hex::LowerHex(Sha256::digest(entry.as_bytes()))\n            ),\n            expected\n        );\n        return entry;\n    }\n";
    assert_eq!(before.matches(added).count(), 1);
    before = before.replacen(added, "", 1);
    assert_eq!(before.len(), 54430);
    assert_eq!(
        digest(before.as_bytes()),
        *b"\x00\xb5\x91\x34\xc1\x63\x0e\xef\x13\xc1\xe1\xdf\xc8\x33\x7f\x9a\x68\x49\x6e\xf4\xa6\x1a\x0e\xa4\x32\x62\xf5\xfa\xd2\xd1\xbe\x1d"
    );
    before
}

// Normalize only the complete immediate endpoint glue before the unchanged
// whole-root ENTRY, Native transport, and original source assertions.
fn complete_diagnostic846_endpoint_before(source: &str, current: &str) -> String {
    if source != "src/source_reflows_merge.rs" {
        return current.to_owned();
    }
    assert_eq!(current.len(), 56251);
    assert_eq!(
        digest(current.as_bytes()),
        *b"\x66\xdf\x4c\x15\x56\x90\x77\x9d\x82\xc6\x37\x99\x18\x52\xad\xe4\xcf\x7b\xe2\x8e\x0a\xf3\x82\x5d\x66\x38\xe2\x4f\xd6\x66\xce\x54"
    );
    let added = "    let entry;\n    let current = if source == \"src/diagnostic.rs\" {\n        entry = historical_project_entry(\n            source,\n            \"922f3150549194a6016b86d86fe29dc56a9fb6a23384032b1afc144b248b782e\",\n            current,\n        );\n        entry.as_str()\n    } else {\n        current\n    };\n";
    assert_eq!(current.matches(added).count(), 1);
    let before = current.replacen(added, "", 1);
    assert_eq!(before.len(), 55950);
    assert_eq!(
        digest(before.as_bytes()),
        *b"\xde\x5a\xee\x83\x34\xae\xe4\xc6\x13\xf9\x95\x27\x3b\x4e\xbb\x86\xf1\x42\x58\x5f\xd1\x6f\x61\x79\x12\xca\x1b\xd4\x4c\x67\xf1\xc6"
    );
    before
}

fn constructor825_root_before(source: &str, current: &str) -> String {
    if source != "src/source_reflows_merge.rs" {
        return current.to_owned();
    }
    assert_eq!(current.len(), 59943);
    assert_eq!(
        digest(current.as_bytes()),
        *b"\x66\x5f\xbf\x3c\x93\xa7\x7e\x7e\x4f\x91\x54\xe2\x65\xc1\xb1\xb1\x82\x98\xbf\x55\x1c\xdc\x44\x24\x1a\xd4\x94\xa3\x71\x4d\x18\xdf"
    );
    let body = "\n// Closed complete compiled ENTRY acquisition precedes every original #825 inverse.\nfn constructor825_compiled_entry(source: &str, expected: &str, current: &str) -> Option<String> {\n    let (compiled, length, current_sha, edits): (&str, usize, &str, &[(&str, &str)]) = match (source, expected) {\n        (\"src/workspace_graph/constructor_failure/report.rs\", \"fc2eb1ecc98cc13b6df906a1e2f59f09d198cf958a4c7ff3472e0aae1a866a84\") => (include_str!(\"workspace_graph/constructor_failure/report.rs\"), 10609, \"a389503a2f7e6b241bbbaf906f71e980e564fa689d2389543d581dbc12fe5662\", &[\n            (\"        // Reserve the full width of the captured usize maximum. Otherwise the\\n        // same successful construction acquires different report backing at\\n        // each decimal limit boundary, so its exact minimum changes its bill.\\n        // Rendering below still uses the actual maximum and original help.\\n        let message_len = counted(|out| message(out, usize::MAX))?;\\n        let base_len = counted(|out| write!(out, \\\"{}\\\", BorrowedBuilderHelp(usize::MAX)))?;\\n\", \"        let message_len = counted(|out| message(out, maximum))?;\\n        let base_len = counted(|out| write!(out, \\\"{}\\\", BorrowedBuilderHelp(maximum)))?;\\n\"),\n        ]),\n        (\"src/workspace_graph/constructor_failure/receiving/before.rs\", \"6135d75cf3f57029b9dd754030aba155a75ca902772aa036345481c1d3abc8b1\") => (include_str!(\"workspace_graph/constructor_failure/receiving/before.rs\"), 14616, \"a43797702bc674b8764a7b5533059e49c0eb1272006d38aaf22538bf8334254b\", &[\n            (\"    charge_builder_prebound, checked_retention, collect_expected_edges, constructor_failure,\\n    expected_projection, graph_error, identity_resolution, loan_retention, owned_generics,\\n\", \"    AuthoredDeclaration, ResolvedCore, ResolvedModuleInput, WorkspaceSource,\\n    WorkspaceValidationIndex, charge_builder_prebound, checked_retention, collect_expected_edges,\\n    constructor_failure, expected_projection, graph_error, identity_resolution, loan_retention,\\n    owned_generics, reconstruct_workspace_declaration_facts_with_authority,\\n\"),\n            (\"    validate_retained_declaration_shapes, verify_resolved_call_edges, AuthoredDeclaration,\\n    ResolvedCore, ResolvedModuleInput, WorkspaceSource,\\n\", \"    validate_retained_declaration_shapes, validate_stub_signatures_authored,\\n    verify_resolved_call_edges,\\n\"),\n            (\"                    &mut core_proofs,\\n                    frontend.as_deref(),\\n\", \"                    &mut core_proofs,frontend.as_deref(),\\n\"),\n            (\"\\nuse crate::workspace_graph::before_private_roots::reconstruct_workspace_declaration_facts_with_authority;\\nuse crate::workspace_graph::validation::before_roots::{\\n    validate_stub_signatures_authored, WorkspaceValidationIndex,\\n};\\n\", \"\"),\n        ]),\n        _ => return None,\n    };\n    assert_eq!(current, compiled, \"actual complete compiled constructor ENTRY\");\n    assert_eq!(current.len(), length);\n    assert_eq!(format!(\"{:x}\", crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))), current_sha);\n    let mut entry = current.to_owned();\n    for &(new, old) in edits {\n        assert_eq!(entry.matches(new).count(), 1, \"unique complete constructor ENTRY hunk\");\n        entry = entry.replacen(new, old, 1);\n    }\n    assert_eq!(format!(\"{:x}\", crate::digest_hex::LowerHex(Sha256::digest(entry.as_bytes()))), expected);\n    Some(entry)\n}\n";
    assert_eq!(current.matches(body).count(), 1);
    let before = current.replacen(body, "", 1);
    let added = "    if let Some(entry) = constructor825_compiled_entry(source, expected, current) {\n        return entry;\n    }\n";
    assert_eq!(before.matches(added).count(), 1);
    let before = before.replacen(added, "", 1);
    let added = "            \"src/workspace_graph/expected_projection/borrowed_imports.rs\",\n            \"326d155ffc918d43dd00ccbb2f1aa562e45fbfd9d0fe5b4809ac4401408c7d1c\"\n        ) | (\n";
    assert_eq!(before.matches(added).count(), 1);
    let before = before.replacen(added, "", 1);
    assert_eq!(before.len(), 56251);
    assert_eq!(
        digest(before.as_bytes()),
        *b"\x66\xdf\x4c\x15\x56\x90\x77\x9d\x82\xc6\x37\x99\x18\x52\xad\xe4\xcf\x7b\xe2\x8e\x0a\xf3\x82\x5d\x66\x38\xe2\x4f\xd6\x66\xce\x54"
    );
    before
}

fn constructor825_attempt_root_before(source: &str, current: &str) -> String {
    if source != "src/source_reflows_merge.rs" {
        return current.to_owned();
    }
    assert_eq!(current.len(), 61106);
    assert_eq!(
        digest(current.as_bytes()),
        *b"\x29\x0d\x9d\x36\xa2\x01\xf2\x36\x46\xda\x63\x5c\x19\x93\x71\xff\x9f\x07\x03\xd5\xb0\x69\x7b\xa7\x29\xf0\x77\x42\x8a\x1f\xfd\x64"
    );
    let body = "\n// Authenticate the complete current owner before its unchanged historical chain.\nfn constructor825_attempt_before(current: &str) -> String {\n    assert_eq!(current, include_str!(\"workspace_graph/constructor_failure/attempt.rs\"));\n    assert_eq!(current.len(), 4136);\n    assert_eq!(format!(\"{:x}\", crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))), \"a6ee9ade44a91d067b516603e52453d965b0e4bd8060add721ebe057f5fcb082\");\n    let added = \"    #[cfg(feature = \\\"unstable-workflow-profiling\\\")]\\n    pub(in crate::workspace_graph) fn observation_succeeded(&self) -> bool {\\n        self.result.is_ok()\\n    }\\n\";\n    assert_eq!(current.matches(added).count(), 1);\n    let before = current.replacen(added, \"\", 1);\n    assert_eq!(before.len(), 3973);\n    assert_eq!(format!(\"{:x}\", crate::digest_hex::LowerHex(Sha256::digest(before.as_bytes()))), \"6a4ab15e6a334f636565dc83a21411c4ba40c6de8e61445c015309c94bc892f3\");\n    before\n}\n";
    assert_eq!(current.matches(body).count(), 1);
    let before = current.replacen(body, "", 1);
    let call = "    let acquired;\n    let current = if source == \"src/workspace_graph/constructor_failure/attempt.rs\" {\n        acquired = constructor825_attempt_before(current);\n        acquired.as_str()\n    } else {\n        current\n    };\n";
    assert_eq!(before.matches(call).count(), 1);
    let before = before.replacen(call, "", 1);
    assert_eq!(before.len(), 59943);
    assert_eq!(
        digest(before.as_bytes()),
        *b"\x66\x5f\xbf\x3c\x93\xa7\x7e\x7e\x4f\x91\x54\xe2\x65\xc1\xb1\xb1\x82\x98\xbf\x55\x1c\xdc\x44\x24\x1a\xd4\x94\xa3\x71\x4d\x18\xdf"
    );
    before
}

fn initial845_current_root_before(source: &str, current: &str) -> String {
    if source != "src/source_reflows_merge.rs" {
        return current.to_owned();
    }
    assert_eq!(current.len(), 66330);
    assert_eq!(
        digest(current.as_bytes()),
        *b"\x38\x73\x9f\xa9\x5e\x1c\x18\xff\x09\x39\xe6\x9b\x5b\x66\x87\x64\xa8\x8e\x82\xce\xc0\xec\xc5\x0b\xf7\xdd\x58\x66\x24\x63\x5f\x0e"
    );
    let body = "\n// Exact current owners enter the original #845 source stages before any edits.\nfn initial845_current_owner_before(source: &str, current: &str) -> Option<String> {\n    let (compiled, length, current_sha, old_length, old_sha, edits): (&str, usize, &str, usize, &str, &[(&str, &str)]) = match source {\n        \"src/bounded_output.rs\" => (include_str!(\"bounded_output.rs\"), 52412, \"fa91c8eca4825931211d066d6ebde8c0ff157de1ec78b5950abe867c295e481f\", 51963, \"f9379a489970b6cb369265386226ecaa82e2d657355d50eebedb9e33c8b6589a\", &[\n            (\"#[cfg(feature = \\\"unstable-workflow-profiling\\\")]\\npub(crate) mod exclusive_debits;\\n\", \"\"),\n            (\"                    #[cfg(feature = \\\"unstable-workflow-profiling\\\")]\\n                    exclusive_debits::exceptional_settlement(&parent, remaining.min(consumed));\\n\", \"\"),\n            (\"        #[cfg(feature = \\\"unstable-workflow-profiling\\\")]\\n        exclusive_debits::refusal(budget, length);\\n\", \"\"),\n            (\"    #[cfg(feature = \\\"unstable-workflow-profiling\\\")]\\n    exclusive_debits::debit(budget, length);\\n\", \"\"),\n        ]),\n        \"src/workspace_graph/initial_report/build.rs\" => (include_str!(\"workspace_graph/initial_report/build.rs\"), 17509, \"48b56f916c25d00004bf6d8bc13e56a0972d07f531fceda2c7c94b57d2af20e1\", 17054, \"0fd9df9abe4ceadd45105394a4efcda2edc3a3ab780d0326d924288a60ea5bda\", &[\n            (\"                #[cfg(feature = \\\"unstable-workflow-profiling\\\")]\\n                let mut exclusive = crate::bounded_output::exclusive_debits::attempt(\\n                    resolve_builder_bytes,\\n                    layouts.receipt(),\\n                    fallback_mode,\\n                );\\n                let result = constructor_failure::attempt::run(active_builder_limit(), || {\\n\", \"                constructor_failure::attempt::run(active_builder_limit(), || {\\n\"),\n            (\"                });\\n                #[cfg(feature = \\\"unstable-workflow-profiling\\\")]\\n                exclusive.complete(result.observation_succeeded());\\n                result\\n\", \"                })\\n\"),\n        ]),\n        \"src/workspace_graph/initial_report/tests.rs\" => (include_str!(\"workspace_graph/initial_report/tests.rs\"), 15103, \"d4b4261a0de2fe323d9d4a52422c99e7a1d5e8ca0df594ed5364f2776bef2527\", 13095, \"52afb2ca386cf27e4bd8bf2e70c46565539427a51b11a1cdded57b3762570971\", &[\n            (\"\\n#[cfg(feature = \\\"unstable-workflow-profiling\\\")]\\n#[test]\\nfn exclusive_core_capture_keeps_complete_real_builder_outputs_and_every_original_limit() {\\n    use crate::workflow_profile::plan_comparison::{Binding, Capture, CoreDebitAlgorithm};\\n    for limit in [0, 1, 127, 1023, 8191, MAX_BUILDER_BYTES] {\\n        let run = || {\\n            build_owned_retaining_sources_with_frontend_limit::<Vec<Diagnostic>>(\\n                sources(),\\n                limit,\\n                None,\\n                false,\\n                None,\\n            )\\n            .map(|(build, _)| build.source_graph_schemas().unwrap())\\n        };\\n        let plain = run();\\n        let capture = Capture::begin(Binding::caller_labelled(\\n            \\\"real initial builder parity\\\",\\n            [0; 32],\\n        ))\\n        .unwrap();\\n        let watched = run();\\n        let report = capture.finish().unwrap();\\n        assert_eq!(\\n            plain, watched,\\n            \\\"actual full compiler product at unchanged limit {limit}\\\"\\n        );\\n        assert_eq!(\\n            report.resolved_core.algorithm,\\n            CoreDebitAlgorithm::InstrumentedLibraryTest\\n        );\\n        assert!(\\n            report.resolved_core.complete\\n                && !report.resolved_core.invalid\\n                && !report.resolved_core.lost\\n                && !report.resolved_core.overflow,\\n            \\\"{report:?}\\\"\\n        );\\n        for attempt in &report.resolved_core.attempts[..report.resolved_core.stored_attempts] {\\n            assert!(attempt.complete && !attempt.invalid);\\n            assert_eq!(attempt.stage_sum(), Some(attempt.consumed));\\n            assert_eq!(attempt.operation_sum(), Some(attempt.consumed));\\n        }\\n        if limit == MAX_BUILDER_BYTES {\\n            assert!(watched.is_ok());\\n            assert!(report.resolved_core.entered_attempts > 0);\\n            assert_eq!(\\n                report.resolved_core.final_attempt_ordinal,\\n                Some(report.resolved_core.entered_attempts - 1)\\n            );\\n        }\\n    }\\n}\\n\", \"\"),\n        ]),\n        _ => return None,\n    };\n    assert_eq!(current, compiled, \"complete actual current #845 source owner\");\n    assert_eq!(current.len(), length);\n    assert_eq!(format!(\"{:x}\", crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))), current_sha);\n    let mut before = current.to_owned();\n    for &(new, old) in edits {\n        assert_eq!(before.matches(new).count(), 1, \"unique entire #845 owner inverse\");\n        before = before.replacen(new, old, 1);\n    }\n    assert_eq!(before.len(), old_length);\n    assert_eq!(format!(\"{:x}\", crate::digest_hex::LowerHex(Sha256::digest(before.as_bytes()))), old_sha);\n    Some(before)\n}\n";
    assert_eq!(current.matches(body).count(), 1);
    let before = current.replacen(body, "", 1);
    let call = "    let acquired = initial845_current_owner_before(source, current);\n    let current = acquired.as_deref().unwrap_or(current);\n";
    assert_eq!(before.matches(call).count(), 1);
    let before = before.replacen(call, "", 1);
    assert_eq!(before.len(), 61106);
    assert_eq!(
        digest(before.as_bytes()),
        *b"\x29\x0d\x9d\x36\xa2\x01\xf2\x36\x46\xda\x63\x5c\x19\x93\x71\xff\x9f\x07\x03\xd5\xb0\x69\x7b\xa7\x29\xf0\x77\x42\x8a\x1f\xfd\x64"
    );
    before
}

// Authenticate this complete source batch before every unchanged older root inverse.
fn current13_typed_root_before(source: &str, current: &str) -> String {
    if source != "src/source_reflows_merge.rs" {
        return current.to_owned();
    }
    assert_eq!(current, include_str!("../../source_reflows_merge.rs"));
    let restored = super::super::main_e177_final_inputs::value_direct(
        source,
        "59adcd7b9f642d026dec8c12658db880a275b9dcdb1e28d83dd9248427506764",
        current,
    );
    let current = restored.as_str();
    assert_eq!(current.len(), 100875);
    assert_eq!(
        digest(current.as_bytes()),
        *b"\x59\xad\xcd\x7b\x9f\x64\x2d\x02\x6d\xec\x8c\x12\x65\x8d\xb8\x80\xa2\x75\xb9\xdc\xdb\x1e\x28\xd8\x3d\xd9\x24\x84\x27\x50\x67\x64"
    );
    let mut before = current.to_owned();
    for &(after, prior) in &[
        ("    let _complete_original845_before =\n        runtime_json::project(runtime_json::Fixture::F87, source, &checkpoint);\n    if source == \"src/workspace_graph/diagnostics.rs\" {\n        return constructor825_compiled_entry(source, \"1c1552d56400ec781eb96ff6fd825bbbeec854f5042fab960e4d3d2941e60127\", &checkpoint).expect(\"complete diagnostics845 entry\");\n    }\n    checkpoint\n", "    let _complete_original845_before =\n        runtime_json::project(runtime_json::Fixture::F87, source, &checkpoint);\n    checkpoint\n"),
        ("(\"src/workspace_graph/constructor_failure/receiving/typed/tests.rs\", \"2cd6c2ce3b35abb30253549dc931c6c025653419d2bd16b9fb6c35224a34d6e4\") => (typed_before.as_deref().expect(\"authenticated complete prior typed owner\"), 25856", "(\"src/workspace_graph/constructor_failure/receiving/typed/tests.rs\", \"2cd6c2ce3b35abb30253549dc931c6c025653419d2bd16b9fb6c35224a34d6e4\") => (include_str!(\"workspace_graph/constructor_failure/receiving/typed/tests.rs\"), 25856"),
        ("fn constructor825_compiled_entry(source: &str, expected: &str, current: &str) -> Option<String> {\n    let typed_before = if source == \"src/workspace_graph/constructor_failure/receiving/typed/tests.rs\" && expected == \"2cd6c2ce3b35abb30253549dc931c6c025653419d2bd16b9fb6c35224a34d6e4\" {\n        assert_eq!(current, include_str!(\"workspace_graph/constructor_failure/receiving/typed/tests.rs\"));\n        assert_eq!(current.len(), 27662);\n        assert_eq!(format!(\"{:x}\", crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))), \"ddaa2f3c3d54bab731acb976de3a9482d71fa65a4d409a83c2036ed8f9ad1251\");\n        let mut before = current.to_owned();\n        for &(after, prior) in &[\n            (\"    // Retain the exact complete typed825 caller already authenticated above.\\n    let caller = historical_caller.expect(\\\"complete historical workspace caller\\\");\\n    assert_eq!(caller.len(), 231867);\\n    assert_eq!(format!(\\\"{:x}\\\", crate::digest_hex::LowerHex(Sha256::digest(caller.as_bytes()))), \\\"f517b9c93b4de7db618db0abc0139d4d88121f298ae9f3b16246dc20e833c0a9\\\");\\n    let live_caller = include_str!(concat!(env!(\\\"CARGO_MANIFEST_DIR\\\"), \\\"/src/workspace_graph.rs\\\"));\\n    let validate_live = |actual: &str| {\\n        assert_eq!(actual, live_caller, \\\"complete current workspace caller\\\");\\n        assert_eq!(actual.len(), 217358);\\n        assert_eq!(format!(\\\"{:x}\\\", crate::digest_hex::LowerHex(Sha256::digest(actual.as_bytes()))), \\\"763fd745d6b8afb41a7738ad14b8c0a0cffc782c70f698f4a272d398f1651699\\\");\\n        assert!(actual.contains(\\\"constructor_failure::typed::construct_selected_core(\\\"));\\n        assert!(actual.contains(\\\".receive_typed(failure)\\\"));\\n    };\\n    validate_live(live_caller);\\n    let dispatcher = include_str!(concat!(env!(\\\"CARGO_MANIFEST_DIR\\\"), \\\"/src/workspace_graph/constructor_failure/typed.rs\\\"));\\n    assert_eq!(dispatcher.len(), 11670);\\n    assert_eq!(format!(\\\"{:x}\\\", crate::digest_hex::LowerHex(Sha256::digest(dispatcher.as_bytes()))), \\\"b07ecf1da889cf25e1b9296bf320070d49c58af316efe6036dce534fccd73fb7\\\");\\n    assert!(dispatcher.contains(\\\"    if allow_transient && possible {\\\\n        construct_transient_alias_core(program, authored, programs, registry, core, owner)\\\\n    } else {\\\\n        construct_transient_core(program, authored, programs, registry, core, owner)\\\\n    }\\\"));\\n    for hostile in [format!(\\\"{live_caller}\\\\n\\\"), live_caller[..live_caller.len()-1].to_owned(), caller.clone()] {\\n        assert!(std::panic::catch_unwind(|| validate_live(&hostile)).is_err());\\n    }\\n\", \"    let caller = include_str!(concat!(\\n        env!(\\\"CARGO_MANIFEST_DIR\\\"),\\n        \\\"/src/workspace_graph.rs\\\"\\n    ));\\n\"),\n            (\"        assert_eq!(restored, original, \\\"{path}\\\");\\n        if path == \\\"src/workspace_graph.rs\\\" { historical_caller = Some(current.to_owned()); }\\n\", \"        assert_eq!(restored, original, \\\"{path}\\\");\\n\"),\n            (\"    let mut historical_caller = None;\\n    let records = packet.as_array().unwrap();\\n\", \"    let records = packet.as_array().unwrap();\\n\"),\n        ] {\n            assert_eq!(before.matches(after).count(), 1, \"unique current typed owner inverse\");\n            before = before.replacen(after, prior, 1);\n        }\n        assert_eq!(before.len(), 25856);\n        assert_eq!(format!(\"{:x}\", crate::digest_hex::LowerHex(Sha256::digest(before.as_bytes()))), \"6199f2912e6064de90f1ecccfcbc516ba0e275faabbaac96126abb470d3755b7\");\n        Some(before)\n    } else { None };\n    let current = typed_before.as_deref().unwrap_or(current);\n", "fn constructor825_compiled_entry(source: &str, expected: &str, current: &str) -> Option<String> {\n"),
    ] {
        assert_eq!(before.matches(after).count(), 1, "unique current13 typed root inverse");
        before = before.replacen(after, prior, 1);
    }
    assert_eq!(before.len(), 97193);
    assert_eq!(
        digest(before.as_bytes()),
        *b"\x2f\xf9\x43\x54\xd2\x3b\x7d\xeb\xb0\xdd\x89\x5a\xf5\x8d\xfb\xdc\x14\x70\x8e\x34\x6c\x9c\x02\x28\x4c\x53\x09\x2a\xb2\xfd\x52\xd3"
    );
    before
}

fn current64_complete_root_before(source: &str, current: &str) -> String {
    if source != "src/source_reflows_merge.rs" {
        return current.to_owned();
    }
    let acquired = current13_typed_root_before(source, current);
    let compiled_before =
        current13_typed_root_before(source, include_str!("../../source_reflows_merge.rs"));
    let current = acquired.as_str();
    assert_eq!(current, compiled_before.as_str());
    assert_eq!(current.len(), 97193);
    assert_eq!(
        digest(current.as_bytes()),
        *b"\x2f\xf9\x43\x54\xd2\x3b\x7d\xeb\xb0\xdd\x89\x5a\xf5\x8d\xfb\xdc\x14\x70\x8e\x34\x6c\x9c\x02\x28\x4c\x53\x09\x2a\xb2\xfd\x52\xd3"
    );
    let mut before = current.to_owned();
    for &(after, prior) in &[
        ("// Only literal historical ENTRY stages are acquired here. The complete existing\n// compile35f front and every intermediate inverse below still execute in order.\nfn historical_project_entry(source: &str, expected: &str, current: &str) -> String {\n    // The preceding authentic a3 projector already restored this complete stage.\n    let restored_receiving_stage = source == \"src/workspace_graph/constructor_failure/receiving/before.rs\"\n        && expected == \"6135d75cf3f57029b9dd754030aba155a75ca902772aa036345481c1d3abc8b1\"\n        && current.len() == 14489\n        && format!(\"{:x}\", crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))) == expected;\n    if !restored_receiving_stage {\n        if let Some(entry) = constructor825_compiled_entry(source, expected, current) {\n            return entry;\n        }\n    }\n    // This source is compiled DATA, not a canonical-row admission. Restore\n    // its exact formatter-only predecessor before the unchanged old projector.\n", "// Only literal historical ENTRY stages are acquired here. The complete existing\n// compile35f front and every intermediate inverse below still execute in order.\nfn historical_project_entry(source: &str, expected: &str, current: &str) -> String {\n    if let Some(entry) = constructor825_compiled_entry(source, expected, current) {\n        return entry;\n    }\n    // This source is compiled DATA, not a canonical-row admission. Restore\n    // its exact formatter-only predecessor before the unchanged old projector.\n"),
        ("}\n// Preserve the complete immutable typed825 runtime manifest's own exact domain.\npub(crate) fn setup_typed_input(source: &str, current: &str) -> String {\n    let typed_entry = match source {\n        \"src/workspace_graph/expected_projection/defaults/typed.rs\" => constructor825_compiled_entry(source, \"9761dcd2ec5fd2351807f7526479680a56c51211eb584385ca85dbc893f38fb7\", current),\n        \"src/workspace_graph/expected_projection/import_stub/typed.rs\" => constructor825_compiled_entry(source, \"c220a90bfa131f6b75131e461ead26aea1dcee6c3ad09c42e5f481f5c12c5b11\", current),\n        _ => None,\n    };\n    let current = typed_entry.as_deref().unwrap_or(current);\n    let acquired;\n    let current = if source == \"src/workspace_graph/constructor_failure/attempt.rs\" {\n        acquired = constructor825_attempt_before(current);\n", "}\n// Preserve the complete immutable typed825 runtime manifest's own exact domain.\npub(crate) fn setup_typed_input(source: &str, current: &str) -> String {\n    let acquired;\n    let current = if source == \"src/workspace_graph/constructor_failure/attempt.rs\" {\n        acquired = constructor825_attempt_before(current);\n"),
        ("pub(crate) fn initial845_input(source: &str, current: &str) -> String {\n    let acquired = initial845_current_owner_before(source, current);\n    let current = acquired.as_deref().unwrap_or(current);\n    let ledger_entry = initial845_ledger_entry(source, current);\n    let current = ledger_entry.as_deref().unwrap_or(current);\n    let owner_entry = initial845_compiled_owner_entry(source, current);\n    let current = owner_entry.as_deref().unwrap_or(current);\n    let checkpoint = final_batch_input(\"845\", source, current);\n    let _complete_original845_before =\n        runtime_json::project(runtime_json::Fixture::F87, source, &checkpoint);\n", "pub(crate) fn initial845_input(source: &str, current: &str) -> String {\n    let acquired = initial845_current_owner_before(source, current);\n    let current = acquired.as_deref().unwrap_or(current);\n    let checkpoint = final_batch_input(\"845\", source, current);\n    let _complete_original845_before =\n        runtime_json::project(runtime_json::Fixture::F87, source, &checkpoint);\n"),
        ("            (\"                    &mut core_proofs,\\n                    frontend.as_deref(),\\n\", \"                    &mut core_proofs,frontend.as_deref(),\\n\"),\n            (\"\\nuse crate::workspace_graph::before_private_roots::reconstruct_workspace_declaration_facts_with_authority;\\nuse crate::workspace_graph::validation::before_roots::{\\n    validate_stub_signatures_authored, WorkspaceValidationIndex,\\n};\\n\", \"\"),\n        ]),\n        (\"src/workspace_graph/constructor_failure/receiving/typed/tests.rs\", \"2cd6c2ce3b35abb30253549dc931c6c025653419d2bd16b9fb6c35224a34d6e4\") => (include_str!(\"workspace_graph/constructor_failure/receiving/typed/tests.rs\"), 25856, \"6199f2912e6064de90f1ecccfcbc516ba0e275faabbaac96126abb470d3755b7\", &[\n            (\"        let old =\\n            constructor_test_inline_defaults::default_expr(&ty, &declarations, false).unwrap();\\n\", \"        let old = constructor_test_inline_defaults::default_expr(&ty, &declarations, false).unwrap();\\n\"),\n            (\"                let actual = constructor_test_typed_defaults::default_expr(\\n                    &ty,\\n                    &declarations,\\n                    false,\\n                    &mut owner,\\n                )\\n                .unwrap();\\n\", \"                let actual =\\n                    constructor_test_typed_defaults::default_expr(&ty, &declarations, false, &mut owner).unwrap();\\n\"),\n            (\"                        let actual = constructor_test_typed_defaults::default_expr(\\n                            &ty,\\n                            &declarations,\\n                            false,\\n                            &mut owner,\\n                        );\\n\", \"                        let actual =\\n                            constructor_test_typed_defaults::default_expr(&ty, &declarations, false, &mut owner);\\n\"),\n            (\"                let actual = constructor_test_typed_import_stub::default_expr(\\n                    function,\\n                    &[],\\n                    false,\\n                    false,\\n                    &mut owner,\\n                );\\n\", \"                let actual =\\n                    constructor_test_typed_import_stub::default_expr(function, &[], false, false, &mut owner);\\n\"),\n            (\"    use crate::workspace_graph::expected_projection::{private_layout, CoreProviderProofs};\\n\", \"    use crate::workspace_graph::expected_projection::{CoreProviderProofs, private_layout};\\n\"),\n            (\"                        assert!(actual\\n                            .source()\\n                            .types()\\n                            .iter()\\n                            .eq(old.source().types().iter()));\\n                        assert!(actual\\n                            .source()\\n                            .functions()\\n                            .iter()\\n                            .eq(old.source().functions().iter()));\\n\", \"                        assert!(actual.source().types().iter().eq(old.source().types().iter()));\\n                        assert!(actual.source().functions().iter().eq(old.source().functions().iter()));\\n\"),\n            (\"            match constructor_test_typed_defaults::default_expr(&Type::I64, &[], false, &mut owner)\\n            {\\n\", \"            match constructor_test_typed_defaults::default_expr(&Type::I64, &[], false, &mut owner) {\\n\"),\n        ]),\n        (\"src/workspace_graph/expected_projection/defaults/inline_tests.rs\", \"1fa04bba6279f410439f819afeb9cbe0b9e33c43d6af56550e1cd39ade0e7ead\") => (include_str!(\"workspace_graph/expected_projection/defaults/inline_tests.rs\"), 15754, \"21751179a0051ebb89691962eb1cd4f3161a96cdc5ad5158c60faceb8ec5df4c\", &[\n            (\"            record[\\\"source\\\"].as_str().unwrap(),\\n            current,\\n\", \"            record[\\\"source\\\"].as_str().unwrap(), current,\\n\"),\n            (\"            record[\\\"source\\\"].as_str().unwrap(),\\n            source,\\n\", \"            record[\\\"source\\\"].as_str().unwrap(), source,\\n\"),\n        ]),\n        (\"src/workspace_graph/constructor_failure/receiving/before.rs\", \"f07589cda30b524062f7ad4dcb87cfc1244b2746729b2b1e5cd0e84320786aed\") => (include_str!(\"workspace_graph/constructor_failure/receiving/before.rs\"), 14616, \"a43797702bc674b8764a7b5533059e49c0eb1272006d38aaf22538bf8334254b\", &[\n            (\"//! Complete original core constructor, independently compiled; body unchanged.\\nuse crate::bounded_output::staged;\\nuse crate::workspace_graph::{\\n    charge_builder_prebound, checked_retention, collect_expected_edges, constructor_failure,\\n    expected_projection, graph_error, identity_resolution, loan_retention, owned_generics,\\n    reserve_builder_structure, reserve_workspace_module_carrier, resolved_loan_bytes,\\n    retain_workspace_module, retained_validation, static_text,\\n    validate_retained_declaration_shapes, verify_resolved_call_edges, AuthoredDeclaration,\\n    ResolvedCore, ResolvedModuleInput, WorkspaceSource,\\n};\\nuse crate::{ast::Program, diagnostic::Diagnostic, hir};\\nuse std::cell::Cell;\\n\", \"//! Complete original core constructor, independently compiled; body unchanged.\\nuse crate::bounded_output::staged;\\nuse crate::workspace_graph::{\\n    AuthoredDeclaration, ResolvedCore, ResolvedModuleInput, WorkspaceSource,\\n    charge_builder_prebound, checked_retention, collect_expected_edges,\\n    constructor_failure, expected_projection, graph_error, identity_resolution, loan_retention,\\n    owned_generics,\\n    reserve_builder_structure, reserve_workspace_module_carrier, resolved_loan_bytes,\\n    retain_workspace_module, retained_validation, static_text,\\n    validate_retained_declaration_shapes,\\n    verify_resolved_call_edges,\\n};\\nuse crate::{ast::Program, diagnostic::Diagnostic, hir};\\nuse std::cell::Cell;\\n\"),\n            (\"            // until this legacy outer adapter.\\n            let (resolved, function_costs, reused_functions, static_text) =\\n                identity_resolution::resolve_module_with_core(\\n                    &mut core_proofs,\\n                    frontend.as_deref(),\\n                    &program.path,\\n                    &synthetic,\\n                    private,\\n\", \"            // until this legacy outer adapter.\\n            let (resolved, function_costs, reused_functions, static_text) =\\n                identity_resolution::resolve_module_with_core(\\n                    &mut core_proofs,frontend.as_deref(),\\n                    &program.path,\\n                    &synthetic,\\n                    private,\\n\"),\n            (\"    ))\\n}\\n\\nuse crate::workspace_graph::before_private_roots::reconstruct_workspace_declaration_facts_with_authority;\\nuse crate::workspace_graph::validation::before_roots::{\\n    validate_stub_signatures_authored, WorkspaceValidationIndex,\\n};\\n\", \"    ))\\n}\\n\\nuse crate::workspace_graph::validation::before_roots::{WorkspaceValidationIndex, validate_stub_signatures_authored};\\nuse crate::workspace_graph::before_private_roots::reconstruct_workspace_declaration_facts_with_authority;\\n\"),\n        ]),\n        (\"src/workspace_graph/diagnostics.rs\", \"1c1552d56400ec781eb96ff6fd825bbbeec854f5042fab960e4d3d2941e60127\") => (include_str!(\"workspace_graph/diagnostics.rs\"), 15942, \"b0785724bdbde5253b6a1aee53779d668f4fdbd13fe079badb586082b4045612\", &[\n            (\"use crate::diagnostic::Diagnostic;\\n\\nuse super::{\\n    WorkspaceResolvedModule, MAX_BUILDER_BYTES, MAX_CALLABLES, MAX_CALLS, MAX_CROSS_FILE_EDGES,\\n    MAX_DECLARATIONS, MAX_DEPENDENCY_DEPTH, MAX_ENTRY_MODULE_BYTES, MAX_FILES, MAX_OUTPUT_BYTES,\\n    MAX_TOTAL_SOURCE_BYTES, MAX_USES,\\n};\\n\\npub(super) const PROVIDER_MAIN_HELP: &str = \\\"`entry` in semaprax.toml must name the module that declares `main`; every other listed source is a provider module and declares no `main`\\\";\\n\", \"use crate::diagnostic::Diagnostic;\\n\\nuse super::{\\n    MAX_BUILDER_BYTES, MAX_CALLABLES, MAX_CALLS, MAX_CROSS_FILE_EDGES, MAX_DECLARATIONS,\\n    MAX_DEPENDENCY_DEPTH, MAX_ENTRY_MODULE_BYTES, MAX_FILES, MAX_OUTPUT_BYTES,\\n    MAX_TOTAL_SOURCE_BYTES, MAX_USES, WorkspaceResolvedModule,\\n};\\n\\npub(super) const PROVIDER_MAIN_HELP: &str = \\\"`entry` in semaprax.toml must name the module that declares `main`; every other listed source is a provider module and declares no `main`\\\";\\n\"),\n        ]),\n        (\"src/workspace_graph/constructor_failure/typed.rs\", \"1482ad177d1d605a183cccc3ef185aaed36e7214b1bf4db8065a466303fc99f6\") => (include_str!(\"workspace_graph/constructor_failure/typed.rs\"), 11670, \"b07ecf1da889cf25e1b9296bf320070d49c58af316efe6036dce534fccd73fb7\", &[\n            (\"/// Reserved wrappers and ordinary function imports can trigger a full probe;\\n\", \"/// Only these reserved identity families can reach FunctionEntry::Borrowed;\\n\"),\n            (\"    if SCALAR_SIGNATURE_BEFORE.with(std::cell::Cell::get) {\\n        return before_scalar_dispatch::construct_selected_core(\\n            program,\\n            authored,\\n            programs,\\n            registry,\\n            core,\\n            allow_transient,\\n            owner,\\n        );\\n    }\\n    #[cfg(test)]\\n\", \"\"),\n            (\"            before_scalar_dispatch::construct_transient_alias_core(\\n                program, authored, programs, registry, core, owner,\\n            )\\n\", \"            construct_transient_alias_core(program, authored, programs, registry, core, owner)\\n\"),\n            (\"    owner.reserve(program.module_uses.len().saturating_mul(\\n        PER_IMPORT_BYTES + 2 * size_of::<&crate::ast::ModuleUseKind>() + 2 * size_of::<bool>(),\\n    ))?;\\n    let possible = program.module_uses.iter().any(|import| {\\n        may_borrow_wrapper(import.persistent_id.as_str())\\n            || import.kind == crate::ast::ModuleUseKind::Function\\n    });\\n\", \"    owner.reserve(program.module_uses.len().saturating_mul(PER_IMPORT_BYTES))?;\\n    let possible = program\\n        .module_uses\\n        .iter()\\n        .any(|import| may_borrow_wrapper(import.persistent_id.as_str()));\\n\"),\n            (\"\\n#[cfg(test)]\\n#[path = \\\"before_scalar_dispatch.rs\\\"]\\npub(in crate::workspace_graph) mod before_scalar_dispatch;\\n\\n#[cfg(test)]\\nthread_local! {\\n    static SCALAR_SIGNATURE_BEFORE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };\\n}\\n/// The entire immediate-old actual selector and its original producer choices.\\n#[cfg(test)]\\npub(in crate::workspace_graph) fn with_original_scalar_dispatch<T>(run: impl FnOnce() -> T) -> T {\\n    struct Restore(bool);\\n    impl Drop for Restore {\\n        fn drop(&mut self) {\\n            SCALAR_SIGNATURE_BEFORE.with(|value| value.set(self.0));\\n        }\\n    }\\n    let restore = Restore(SCALAR_SIGNATURE_BEFORE.with(|value| value.replace(true)));\\n    let result = run();\\n    drop(restore);\\n    result\\n}\\n\", \"\"),\n        ]),\n        (\"src/workspace_graph/expected_projection/defaults/typed.rs\", \"9761dcd2ec5fd2351807f7526479680a56c51211eb584385ca85dbc893f38fb7\") => (include_str!(\"workspace_graph/expected_projection/defaults/typed.rs\"), 6321, \"3316c665e3b4cc8cdcf5c8a71c90ba721f2e67df36c9ed743c58e6008ca55233\", &[\n            (\"    checked_builder_sum, Span, StructuralCost, Type, TypeDeclaration, TypeDeclarationKind,\\n\", \"    Span, StructuralCost, Type, TypeDeclaration, TypeDeclarationKind, checked_builder_sum,\\n\"),\n        ]),\n        (\"src/workspace_graph/expected_projection/import_stub/typed.rs\", \"c220a90bfa131f6b75131e461ead26aea1dcee6c3ad09c42e5f481f5c12c5b11\") => (include_str!(\"workspace_graph/expected_projection/import_stub/typed.rs\"), 1466, \"cbb50f93fbfdb170e83b2955461b50a018f7541179d79ab613ef502cef8b8d31\", &[\n            (\"use super::{forward_parameter, record_shape, Expr, ExprKind, Function, ParamMode, Type};\\n\", \"use super::{Expr, ExprKind, Function, ParamMode, Type, forward_parameter, record_shape};\\n\"),\n        ]),\n        _ => return None,\n    };\n    assert_eq!(current, compiled, \"actual complete compiled constructor ENTRY\");\n", "            (\"                    &mut core_proofs,\\n                    frontend.as_deref(),\\n\", \"                    &mut core_proofs,frontend.as_deref(),\\n\"),\n            (\"\\nuse crate::workspace_graph::before_private_roots::reconstruct_workspace_declaration_facts_with_authority;\\nuse crate::workspace_graph::validation::before_roots::{\\n    validate_stub_signatures_authored, WorkspaceValidationIndex,\\n};\\n\", \"\"),\n        ]),\n        _ => return None,\n    };\n    assert_eq!(current, compiled, \"actual complete compiled constructor ENTRY\");\n"),
        ("            (\"                #[cfg(feature = \\\"unstable-workflow-profiling\\\")]\\n                let mut exclusive = crate::bounded_output::exclusive_debits::attempt(\\n                    resolve_builder_bytes,\\n                    layouts.receipt(),\\n                    fallback_mode,\\n                );\\n                let result = constructor_failure::attempt::run(active_builder_limit(), || {\\n\", \"                constructor_failure::attempt::run(active_builder_limit(), || {\\n\"),\n            (\"                });\\n                #[cfg(feature = \\\"unstable-workflow-profiling\\\")]\\n                exclusive.complete(result.observation_succeeded());\\n                result\\n\", \"                })\\n\"),\n        ]),\n        \"src/workspace_graph/initial_report/tests.rs\" => (include_str!(\"workspace_graph/initial_report/tests.rs\"), 15256, \"0f530933b3964478e93a13bd7065cda6a97ab121b7c77508db45e72aff401ad7\", 13095, \"52afb2ca386cf27e4bd8bf2e70c46565539427a51b11a1cdded57b3762570971\", &[\n            (\"    let complete_before = crate::source_reflows_merge::initial845_input(\\n        \\\"src/workspace_graph/initial_report/before.rs\\\", include_str!(\\\"before.rs\\\"),\\n    );\\n    assert!(complete_before.contains(&body));\\n\", \"    assert!(include_str!(\\\"before.rs\\\").contains(&body));\\n\"),\n            (\"\\n#[cfg(feature = \\\"unstable-workflow-profiling\\\")]\\n#[test]\\nfn exclusive_core_capture_keeps_complete_real_builder_outputs_and_every_original_limit() {\\n    use crate::workflow_profile::plan_comparison::{Binding, Capture, CoreDebitAlgorithm};\\n    for limit in [0, 1, 127, 1023, 8191, MAX_BUILDER_BYTES] {\\n        let run = || {\\n            build_owned_retaining_sources_with_frontend_limit::<Vec<Diagnostic>>(\\n                sources(),\\n                limit,\\n                None,\\n                false,\\n                None,\\n            )\\n            .map(|(build, _)| build.source_graph_schemas().unwrap())\\n        };\\n        let plain = run();\\n        let capture = Capture::begin(Binding::caller_labelled(\\n            \\\"real initial builder parity\\\",\\n            [0; 32],\\n        ))\\n        .unwrap();\\n        let watched = run();\\n        let report = capture.finish().unwrap();\\n        assert_eq!(\\n            plain, watched,\\n            \\\"actual full compiler product at unchanged limit {limit}\\\"\\n        );\\n        assert_eq!(\\n            report.resolved_core.algorithm,\\n            CoreDebitAlgorithm::InstrumentedLibraryTest\\n        );\\n        assert!(\\n            report.resolved_core.complete\\n                && !report.resolved_core.invalid\\n                && !report.resolved_core.lost\\n                && !report.resolved_core.overflow,\\n            \\\"{report:?}\\\"\\n        );\\n        for attempt in &report.resolved_core.attempts[..report.resolved_core.stored_attempts] {\\n            assert!(attempt.complete && !attempt.invalid);\\n            assert_eq!(attempt.stage_sum(), Some(attempt.consumed));\\n            assert_eq!(attempt.operation_sum(), Some(attempt.consumed));\\n        }\\n        if limit == MAX_BUILDER_BYTES {\\n            assert!(watched.is_ok());\\n            assert!(report.resolved_core.entered_attempts > 0);\\n            assert_eq!(\\n                report.resolved_core.final_attempt_ordinal,\\n                Some(report.resolved_core.entered_attempts - 1)\\n            );\\n        }\\n    }\\n}\\n\", \"\"),\n        ]),\n        _ => return None,\n", "            (\"                #[cfg(feature = \\\"unstable-workflow-profiling\\\")]\\n                let mut exclusive = crate::bounded_output::exclusive_debits::attempt(\\n                    resolve_builder_bytes,\\n                    layouts.receipt(),\\n                    fallback_mode,\\n                );\\n                let result = constructor_failure::attempt::run(active_builder_limit(), || {\\n\", \"                constructor_failure::attempt::run(active_builder_limit(), || {\\n\"),\n            (\"                });\\n                #[cfg(feature = \\\"unstable-workflow-profiling\\\")]\\n                exclusive.complete(result.observation_succeeded());\\n                result\\n\", \"                })\\n\"),\n        ]),\n        \"src/workspace_graph/initial_report/tests.rs\" => (include_str!(\"workspace_graph/initial_report/tests.rs\"), 15103, \"d4b4261a0de2fe323d9d4a52422c99e7a1d5e8ca0df594ed5364f2776bef2527\", 13095, \"52afb2ca386cf27e4bd8bf2e70c46565539427a51b11a1cdded57b3762570971\", &[\n            (\"\\n#[cfg(feature = \\\"unstable-workflow-profiling\\\")]\\n#[test]\\nfn exclusive_core_capture_keeps_complete_real_builder_outputs_and_every_original_limit() {\\n    use crate::workflow_profile::plan_comparison::{Binding, Capture, CoreDebitAlgorithm};\\n    for limit in [0, 1, 127, 1023, 8191, MAX_BUILDER_BYTES] {\\n        let run = || {\\n            build_owned_retaining_sources_with_frontend_limit::<Vec<Diagnostic>>(\\n                sources(),\\n                limit,\\n                None,\\n                false,\\n                None,\\n            )\\n            .map(|(build, _)| build.source_graph_schemas().unwrap())\\n        };\\n        let plain = run();\\n        let capture = Capture::begin(Binding::caller_labelled(\\n            \\\"real initial builder parity\\\",\\n            [0; 32],\\n        ))\\n        .unwrap();\\n        let watched = run();\\n        let report = capture.finish().unwrap();\\n        assert_eq!(\\n            plain, watched,\\n            \\\"actual full compiler product at unchanged limit {limit}\\\"\\n        );\\n        assert_eq!(\\n            report.resolved_core.algorithm,\\n            CoreDebitAlgorithm::InstrumentedLibraryTest\\n        );\\n        assert!(\\n            report.resolved_core.complete\\n                && !report.resolved_core.invalid\\n                && !report.resolved_core.lost\\n                && !report.resolved_core.overflow,\\n            \\\"{report:?}\\\"\\n        );\\n        for attempt in &report.resolved_core.attempts[..report.resolved_core.stored_attempts] {\\n            assert!(attempt.complete && !attempt.invalid);\\n            assert_eq!(attempt.stage_sum(), Some(attempt.consumed));\\n            assert_eq!(attempt.operation_sum(), Some(attempt.consumed));\\n        }\\n        if limit == MAX_BUILDER_BYTES {\\n            assert!(watched.is_ok());\\n            assert!(report.resolved_core.entered_attempts > 0);\\n            assert_eq!(\\n                report.resolved_core.final_attempt_ordinal,\\n                Some(report.resolved_core.entered_attempts - 1)\\n            );\\n        }\\n    }\\n}\\n\", \"\"),\n        ]),\n        _ => return None,\n"),
        ("    assert_eq!(format!(\"{:x}\", crate::digest_hex::LowerHex(Sha256::digest(before.as_bytes()))), old_sha);\n    Some(before)\n}\n\n// Closed complete legacy ledger ENTRY after the authentic current #908 inverse.\nfn initial845_ledger_entry(source: &str, current: &str) -> Option<String> {\n    if source != \"src/bounded_output.rs\" { return None; }\n    assert_eq!(current.len(), 51963);\n    assert_eq!(format!(\"{:x}\", crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))), \"f9379a489970b6cb369265386226ecaa82e2d657355d50eebedb9e33c8b6589a\");\n    let mut entry = current.to_owned();\n    for &(new, old) in &[\n        (\"pub(crate) mod cfg_reservation;\\npub(crate) mod format_acceptance;\\npub(crate) mod identity_reservation;\\npub(crate) mod key_directory;\\npub(crate) mod prepaid;\\n\", \"pub(crate) mod format_acceptance;\\npub(crate) mod cfg_reservation;\\npub(crate) mod identity_reservation;\\npub(crate) mod key_directory;\\npub(crate) mod prepaid;\\n\"),\n        (\"            ACTIVE.with(|active| {\\n                active.replace(previous.clone());\\n            });\\n            #[cfg(feature = \\\"unstable-workflow-profiling\\\")]\\n            crate::workflow_profile::plan_comparison::observe_ledger_transition();\\n            if let Some(parent) = previous {\\n                if !reserve(Some(&parent), consumed) {\\n                    record_refusal(&parent, consumed);\\n\", \"            ACTIVE.with(|active| {\\n                active.replace(previous.clone());\\n            });\\n            if let Some(parent) = previous {\\n                if !reserve(Some(&parent), consumed) {\\n                    record_refusal(&parent, consumed);\\n\"),\n        (\"        stage: Cell::new(None),\\n    });\\n    let previous = ACTIVE.with(|active| active.replace(Some(Rc::clone(&budget))));\\n    #[cfg(feature = \\\"unstable-workflow-profiling\\\")]\\n    crate::workflow_profile::plan_comparison::observe_ledger_transition();\\n    let restore = Restore {\\n        previous,\\n        current: Rc::clone(&budget),\\n\", \"        stage: Cell::new(None),\\n    });\\n    let previous = ACTIVE.with(|active| active.replace(Some(Rc::clone(&budget))));\\n    let restore = Restore {\\n        previous,\\n        current: Rc::clone(&budget),\\n\"),\n        (\"    stage: ReservationStage,\\n    operation: impl FnOnce() -> T,\\n) -> T {\\n    #[cfg(test)]\\n    let _allocation_phase = crate::test_allocation_observer::phase_guard(stage);\\n    let restore = reservation_stage_guard(stage);\\n    let value = operation();\\n    drop(restore);\\n\", \"    stage: ReservationStage,\\n    operation: impl FnOnce() -> T,\\n) -> T {\\n    let restore = reservation_stage_guard(stage);\\n    let value = operation();\\n    drop(restore);\\n\"),\n        (\"    if let Some(budget) = active() {\\n        budget.floor.set(0);\\n    }\\n}\\n\\n// Read-only feature evidence; no ledger reference or reservation authority escapes.\\n#[cfg(feature = \\\"unstable-workflow-profiling\\\")]\\npub(crate) fn active_ledger_snapshot()\\n-> Option<crate::workflow_profile::plan_comparison::LedgerSnapshot> {\\n    ACTIVE.with(|active| {\\n        let active = active.try_borrow().ok()?;\\n        let budget = active.as_ref()?;\\n        crate::workflow_profile::plan_comparison::LedgerSnapshot::observe(\\n            Rc::as_ptr(budget) as usize,\\n            budget.remaining.get(),\\n        )\\n    })\\n}\\n\\npub(crate) fn active_remaining() -> Option<usize> {\\n\", \"    if let Some(budget) = active() {\\n        budget.floor.set(0);\\n    }\\n}\\n\\npub(crate) fn active_remaining() -> Option<usize> {\\n\"),\n        (\"        );\\n    }\\n}\\n\\n/// Keep the original clone's text debit and refusal/floor selection while\\n/// borrowing its immutable input instead of allocating a temporary String.\\n/// The caller must hold that input through the returned view. This grants no\\n/// heap backing or authority and retains the original clone tariff unchanged.\\npub(crate) fn borrowed_clone_view(value: &str) -> &str {\\n    let budget = active();\\n    if !reserve(budget.as_deref(), value.len()) {\\n        return \\\"\\\";\\n    }\\n    value\\n}\\n\\n// Same input, budget owner, reserve receiver and bool as budgeted_clone. Its\\n// old String result slot covers this replacement borrowed-text result; no\\n// unused bytes are credited and no heap-carrier allowance is borrowed.\\nconst _: () = {\\n    assert!(std::mem::size_of::<&str>() <= std::mem::size_of::<String>());\\n    assert!(std::mem::align_of::<&str>() <= std::mem::align_of::<String>());\\n};\\n\", \"        );\\n    }\\n}\\n\"),\n] {\n        assert_eq!(entry.matches(new).count(), 1, \"unique complete legacy ledger ENTRY hunk\");\n        entry = entry.replacen(new, old, 1);\n    }\n    assert_eq!(entry.len(), 50190);\n    assert_eq!(format!(\"{:x}\", crate::digest_hex::LowerHex(Sha256::digest(entry.as_bytes()))), \"d01a82d602c286044cc1d5df8d05d02dfe5168d1139a82380aa01b06e47c7fd5\");\n    Some(entry)\n}\n\n// Fixed complete original #845 owner inputs; all historical pipelines remain intact.\nfn initial845_compiled_owner_entry(source: &str, current: &str) -> Option<String> {\n    let (length, sha, prior_length, prior_sha, edits): (usize, &str, usize, &str, &[(&str, &str)]) = match source {\n        \"src/workspace_graph/initial_report/before.rs\" => (17548, \"ae04e8fc229cd104dc096c44f968119aa76395676ca90b821aec2f4e14131f27\", 17445, \"5c7f71670fb7bfb0ffd77071fcfa60c24230692e310644ce47263a59d43ab87f\", &[\n            (\"            if let Some(frontend) = frontend.as_deref_mut() {\\n                frontend.canonicalized();\\n            }\\n            canonical\\n                .bytes\\n                .expect(\\\"canonical byte count checked before source equality\\\")\\n        };\\n        canonical_bytes = checked_usage(\\n            canonical_bytes,\\n\", \"            if let Some(frontend) = frontend.as_deref_mut() {\\n                frontend.canonicalized();\\n            }\\n            canonical.bytes.expect(\\\"canonical byte count checked before source equality\\\")\\n        };\\n        canonical_bytes = checked_usage(\\n            canonical_bytes,\\n\"),\n            (\"                );\\n                constructor_failure::attempt::run(active_builder_limit(), || {\\n                    constructor_failure::attempt::reserve(layouts.receipt())?;\\n                    staged!(\\n                        CorePrebound,\\n                        constructor_failure::attempt::reserve(resolve_builder_bytes)\\n                    )?;\\n                    let custody = custody::SourceCustody::plan(\\n                        &sources,\\n                        &programs,\\n\", \"                );\\n                constructor_failure::attempt::run(active_builder_limit(), || {\\n                    constructor_failure::attempt::reserve(layouts.receipt())?;\\n                    staged!(CorePrebound, constructor_failure::attempt::reserve(resolve_builder_bytes))?;\\n                    let custody = custody::SourceCustody::plan(\\n                        &sources,\\n                        &programs,\\n\"),\n            (\"    Ok((build, sources))\\n}\\n\\n// The exact original terminal Vec producer, independently callable after a\\n// real failed attempt. The whole original Err site remains in the body above.\\npub(super) fn terminal(\\n\", \"    Ok((build, sources))\\n}\\n\\n\\n// The exact original terminal Vec producer, independently callable after a\\n// real failed attempt. The whole original Err site remains in the body above.\\npub(super) fn terminal(\\n\"),\n        ]),\n        \"src/workspace_graph/initial_report/before_mappers.rs\" => (4561, \"69b2e94928db4f1f68c5172258be1c1332c802581d84dba238b8455664bcceea\", 4109, \"d736b5a92caa2a355e5fa1a2112469297272afd673b704c4195224e782eb3e1d\", &[\n            (\"//! Complete original owning mapper bodies; only lexical visibility adapts.\\n\\npub(super) mod change {\\n    use crate::diagnostic::Diagnostic;\\n\\n    use crate::semantic_workspace;\\n\\n    pub(in crate::workspace_graph::initial_report) fn map_change_builder_limit(\\n        diagnostics: Vec<Diagnostic>,\\n        internal_maximum: usize,\\n    ) -> Vec<Diagnostic> {\\n        let expected = crate::bounded_output::budgeted_format(format_args!(\\n            \\\"Workspace Semantic Graph `change_builder_bytes` exceeds {internal_maximum}\\\"\\n        ));\\n        if diagnostics.len() == 1\\n            && diagnostics[0].code == \\\"SPX-G171\\\"\\n            && diagnostics[0].message == expected\\n        {\\n            limit(\\n                \\\"total_analysis_builder_bytes\\\",\\n                semantic_workspace::MAX_CHANGE_BUILDER_BYTES,\\n            )\\n        } else {\\n            diagnostics\\n        }\\n    }\\n\\n    fn limit(field: &'static str, maximum: usize) -> Vec<Diagnostic> {\\n        vec![Diagnostic::io(\\n            \\\"SPX-G183\\\",\\n            crate::bounded_output::budgeted_format(format_args!(\\n                \\\"Semantic Workspace Change `{field}` exceeds {maximum}\\\"\\n            )),\\n        )]\\n    }\\n}\\n\\npub(super) mod structural {\\n    use crate::diagnostic::Diagnostic;\\n\\n    pub(in crate::workspace_graph::initial_report) const MAX_CANDIDATE_GRAPH_BUILDER_BYTES: usize =\\n        16 * 1024 * 1024;\\n\\n    pub(in crate::workspace_graph::initial_report) const MAX_ANALYSIS_BUILDER_BYTES: usize =\\n        32 * 1024 * 1024;\\n\\n    pub(in crate::workspace_graph::initial_report) fn map_candidate_builder_limit(\\n        diagnostics: Vec<Diagnostic>,\\n        internal_maximum: usize,\\n    ) -> Vec<Diagnostic> {\\n        let expected = crate::bounded_output::budgeted_format(format_args!(\\n            \\\"Workspace Semantic Graph `change_builder_bytes` exceeds {internal_maximum}\\\"\\n        ));\\n        if diagnostics.len() == 1\\n            && diagnostics[0].code == \\\"SPX-G171\\\"\\n            && diagnostics[0].message == expected\\n        {\\n            limit(\\n                if internal_maximum == MAX_CANDIDATE_GRAPH_BUILDER_BYTES {\\n                    \\\"candidate_graph_builder_bytes\\\"\\n                } else {\\n                    \\\"analysis_builder_bytes\\\"\\n                },\\n                if internal_maximum == MAX_CANDIDATE_GRAPH_BUILDER_BYTES {\\n                    MAX_CANDIDATE_GRAPH_BUILDER_BYTES\\n                } else {\\n                    MAX_ANALYSIS_BUILDER_BYTES\\n                },\\n            )\\n        } else {\\n            diagnostics\\n        }\\n    }\\n\\n    fn limit(field: &'static str, maximum: usize) -> Vec<Diagnostic> {\\n        vec![Diagnostic::io(\\n            \\\"SPX-G191\\\",\\n            crate::bounded_output::budgeted_format(format_args!(\\n                \\\"Semantic Workspace Structural Change limit exceeded: {field} maximum {maximum}\\\"\\n            )),\\n        )]\\n    }\\n}\\n\\npub(super) mod operations {\\n    use crate::diagnostic::Diagnostic;\\n\\n    pub(in crate::workspace_graph::initial_report) const MAX_CANDIDATE_GRAPH_BUILDER_BYTES: usize =\\n        16_777_216;\\n\\n    pub(in crate::workspace_graph::initial_report) const MAX_OPERATIONS_BUILDER_BYTES: usize =\\n        67_108_864;\\n\\n    pub(in crate::workspace_graph::initial_report) fn map_candidate_diagnostics(\\n        diagnostics: Vec<Diagnostic>,\\n        candidate_graph_limit: usize,\\n        operations_builder_limit: usize,\\n    ) -> Vec<Diagnostic> {\\n        if diagnostics.len() != 1 || diagnostics[0].code != \\\"SPX-G171\\\" {\\n            return diagnostics;\\n        }\\n        let builder_message =\\n            format!(\\\"Workspace Semantic Graph `builder_bytes` exceeds {candidate_graph_limit}\\\");\\n        if diagnostics[0].message == builder_message {\\n            return if candidate_graph_limit == MAX_CANDIDATE_GRAPH_BUILDER_BYTES {\\n                limit(\\n                    \\\"candidate_graph_builder_bytes\\\",\\n                    MAX_CANDIDATE_GRAPH_BUILDER_BYTES,\\n                )\\n            } else {\\n                limit(\\\"operations_builder_bytes\\\", MAX_OPERATIONS_BUILDER_BYTES)\\n            };\\n        }\\n        let change_message = format!(\\n            \\\"Workspace Semantic Graph `change_builder_bytes` exceeds {}\\\",\\n            operations_builder_limit\\n        );\\n        if diagnostics[0].message == change_message {\\n            return limit(\\\"operations_builder_bytes\\\", MAX_OPERATIONS_BUILDER_BYTES);\\n        }\\n        diagnostics\\n    }\\n\\n    fn limit(f: &str, m: usize) -> Vec<Diagnostic> {\\n        vec![Diagnostic::io(\\n            \\\"SPX-G199\\\",\\n            format!(\\\"Semantic Workspace Operations exceeds {f} maximum {m}\\\"),\\n        )]\\n    }\\n}\\n\", \"//! Complete original owning mapper bodies; only lexical visibility adapts.\\n\\npub(super) mod change {\\nuse crate::diagnostic::Diagnostic;\\n\\nuse crate::semantic_workspace;\\n\\npub(in crate::workspace_graph::initial_report) fn map_change_builder_limit(\\n    diagnostics: Vec<Diagnostic>,\\n    internal_maximum: usize,\\n) -> Vec<Diagnostic> {\\n    let expected = crate::bounded_output::budgeted_format(format_args!(\\n        \\\"Workspace Semantic Graph `change_builder_bytes` exceeds {internal_maximum}\\\"\\n    ));\\n    if diagnostics.len() == 1\\n        && diagnostics[0].code == \\\"SPX-G171\\\"\\n        && diagnostics[0].message == expected\\n    {\\n        limit(\\n            \\\"total_analysis_builder_bytes\\\",\\n            semantic_workspace::MAX_CHANGE_BUILDER_BYTES,\\n        )\\n    } else {\\n        diagnostics\\n    }\\n}\\n\\nfn limit(field: &'static str, maximum: usize) -> Vec<Diagnostic> {\\n    vec![Diagnostic::io(\\n        \\\"SPX-G183\\\",\\n        crate::bounded_output::budgeted_format(format_args!(\\n            \\\"Semantic Workspace Change `{field}` exceeds {maximum}\\\"\\n        )),\\n    )]\\n}\\n}\\n\\npub(super) mod structural {\\nuse crate::diagnostic::Diagnostic;\\n\\npub(in crate::workspace_graph::initial_report) const MAX_CANDIDATE_GRAPH_BUILDER_BYTES: usize = 16 * 1024 * 1024;\\n\\npub(in crate::workspace_graph::initial_report) const MAX_ANALYSIS_BUILDER_BYTES: usize = 32 * 1024 * 1024;\\n\\npub(in crate::workspace_graph::initial_report) fn map_candidate_builder_limit(\\n    diagnostics: Vec<Diagnostic>,\\n    internal_maximum: usize,\\n) -> Vec<Diagnostic> {\\n    let expected = crate::bounded_output::budgeted_format(format_args!(\\n        \\\"Workspace Semantic Graph `change_builder_bytes` exceeds {internal_maximum}\\\"\\n    ));\\n    if diagnostics.len() == 1\\n        && diagnostics[0].code == \\\"SPX-G171\\\"\\n        && diagnostics[0].message == expected\\n    {\\n        limit(\\n            if internal_maximum == MAX_CANDIDATE_GRAPH_BUILDER_BYTES {\\n                \\\"candidate_graph_builder_bytes\\\"\\n            } else {\\n                \\\"analysis_builder_bytes\\\"\\n            },\\n            if internal_maximum == MAX_CANDIDATE_GRAPH_BUILDER_BYTES {\\n                MAX_CANDIDATE_GRAPH_BUILDER_BYTES\\n            } else {\\n                MAX_ANALYSIS_BUILDER_BYTES\\n            },\\n        )\\n    } else {\\n        diagnostics\\n    }\\n}\\n\\nfn limit(field: &'static str, maximum: usize) -> Vec<Diagnostic> {\\n    vec![Diagnostic::io(\\n        \\\"SPX-G191\\\",\\n        crate::bounded_output::budgeted_format(format_args!(\\n            \\\"Semantic Workspace Structural Change limit exceeded: {field} maximum {maximum}\\\"\\n        )),\\n    )]\\n}\\n}\\n\\npub(super) mod operations {\\nuse crate::diagnostic::Diagnostic;\\n\\npub(in crate::workspace_graph::initial_report) const MAX_CANDIDATE_GRAPH_BUILDER_BYTES: usize = 16_777_216;\\n\\npub(in crate::workspace_graph::initial_report) const MAX_OPERATIONS_BUILDER_BYTES: usize = 67_108_864;\\n\\npub(in crate::workspace_graph::initial_report) fn map_candidate_diagnostics(\\n    diagnostics: Vec<Diagnostic>,\\n    candidate_graph_limit: usize,\\n    operations_builder_limit: usize,\\n) -> Vec<Diagnostic> {\\n    if diagnostics.len() != 1 || diagnostics[0].code != \\\"SPX-G171\\\" {\\n        return diagnostics;\\n    }\\n    let builder_message =\\n        format!(\\\"Workspace Semantic Graph `builder_bytes` exceeds {candidate_graph_limit}\\\");\\n    if diagnostics[0].message == builder_message {\\n        return if candidate_graph_limit == MAX_CANDIDATE_GRAPH_BUILDER_BYTES {\\n            limit(\\n                \\\"candidate_graph_builder_bytes\\\",\\n                MAX_CANDIDATE_GRAPH_BUILDER_BYTES,\\n            )\\n        } else {\\n            limit(\\\"operations_builder_bytes\\\", MAX_OPERATIONS_BUILDER_BYTES)\\n        };\\n    }\\n    let change_message = format!(\\n        \\\"Workspace Semantic Graph `change_builder_bytes` exceeds {}\\\",\\n        operations_builder_limit\\n    );\\n    if diagnostics[0].message == change_message {\\n        return limit(\\\"operations_builder_bytes\\\", MAX_OPERATIONS_BUILDER_BYTES);\\n    }\\n    diagnostics\\n}\\n\\nfn limit(f: &str, m: usize) -> Vec<Diagnostic> {\\n    vec![Diagnostic::io(\\n        \\\"SPX-G199\\\",\\n        format!(\\\"Semantic Workspace Operations exceeds {f} maximum {m}\\\"),\\n    )]\\n}\\n}\\n\"),\n        ]),\n        \"src/workspace_graph/initial_report/build.rs\" => (17054, \"0fd9df9abe4ceadd45105394a4efcda2edc3a3ab780d0326d924288a60ea5bda\", 16803, \"607af6cab243e1970aed326c8dc7ce7c211ffe902f4e425467599538aef013d1\", &[\n            (\"                })\\n            });\\n        let (core, paid_refusal) = core.into_parts();\\n        #[cfg(test)]\\n        tests::record_core_attempt(\\n            fallback_mode,\\n            resolve_builder_bytes,\\n            consumed,\\n            overflowed,\\n            &refusal,\\n            &site,\\n            &core.as_ref().err(),\\n        );\\n        core_builder_bytes = core_builder_bytes.max(consumed);\\n        if !overflowed {\\n            break core;\\n\", \"                })\\n            });\\n        let (core, paid_refusal) = core.into_parts();\\n        core_builder_bytes = core_builder_bytes.max(consumed);\\n        if !overflowed {\\n            break core;\\n\"),\n        ]),\n        _ => return None,\n    };\n    assert_eq!(current.len(), length);\n    assert_eq!(format!(\"{:x}\", crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))), sha);\n    let mut prior = current.to_owned();\n    for &(new, old) in edits {\n        assert_eq!(prior.matches(new).count(), 1, \"unique complete original845 owner hunk\");\n        prior = prior.replacen(new, old, 1);\n    }\n    assert_eq!(prior.len(), prior_length);\n    assert_eq!(format!(\"{:x}\", crate::digest_hex::LowerHex(Sha256::digest(prior.as_bytes()))), prior_sha);\n    Some(prior)\n}\n", "    assert_eq!(format!(\"{:x}\", crate::digest_hex::LowerHex(Sha256::digest(before.as_bytes()))), old_sha);\n    Some(before)\n}\n"),
] {
        assert_eq!(before.matches(after).count(), 1, "unique complete current64 root inverse");
        before = before.replacen(after, prior, 1);
    }
    assert_eq!(before.len(), 66330);
    assert_eq!(
        digest(before.as_bytes()),
        *b"\x38\x73\x9f\xa9\x5e\x1c\x18\xff\x09\x39\xe6\x9b\x5b\x66\x87\x64\xa8\x8e\x82\xce\xc0\xec\xc5\x0b\xf7\xdd\x58\x66\x24\x63\x5f\x0e"
    );
    before
}
// RUNTIME848_THREE_CONTROLS_END
