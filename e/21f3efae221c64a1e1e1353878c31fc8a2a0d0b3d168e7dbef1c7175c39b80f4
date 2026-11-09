//! Exact new-source acquisition before every unchanged historical raw guard.
use sha2::{Digest, Sha256};
fn digest(text: &str) -> String {
    format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
    )
}
const PACKET_LENGTH: usize = 784669; // sealed by the source binding recipe
const PACKET_SHA256: &str = "a5d995cad4d5ecff6ea81eb310580a9a2caaf63150718d4992726cb99b877278";
fn records() -> serde_json::Value {
    let packet = super::runtime_json::additional(
        super::runtime_json::AdditionalFixture::OrdinaryPlanHistory,
    );
    assert!(
        packet.len() <= 2 * 1024 * 1024,
        "fixed custody packet exceeds cap"
    );
    assert_eq!(packet.len(), PACKET_LENGTH, "whole custody packet length");
    assert_eq!(
        digest(&packet),
        PACKET_SHA256,
        "whole custody packet digest"
    );
    serde_json::from_str(&packet).unwrap()
}
const CURRENT: [(&str, &str); 6] = [
    (
        "src/cleanup_plan/replay.rs",
        include_str!("../cleanup_plan/replay.rs"),
    ),
    (
        "src/source_reflows_merge.rs",
        include_str!("../source_reflows_merge.rs"),
    ),
    (
        "src/source_reflows_merge/main67107_inputs.rs",
        include_str!("main67107_inputs.rs"),
    ),
    (
        "tests/hir_module_boundaries.rs",
        include_str!("../../tests/hir_module_boundaries.rs"),
    ),
    (
        "tests/workspace/semantic_graph/source_inventory.rs",
        include_str!("../../tests/workspace/semantic_graph/source_inventory.rs"),
    ),
    (
        "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs",
        include_str!(
            "../../crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs"
        ),
    ),
];
fn restore(row: &serde_json::Value, current: &str) -> String {
    assert_eq!(
        digest(current),
        row["after_sha256"],
        "whole ordinary plan source input"
    );
    let mut result = current.to_owned();
    for edit in row["inverse"].as_array().unwrap() {
        let new = edit["after"].as_str().unwrap();
        assert_eq!(
            result.matches(new).count(),
            1,
            "unique complete source inverse"
        );
        result = result.replacen(new, edit["before"].as_str().unwrap(), 1);
    }
    assert_eq!(digest(&result), row["before_sha256"]);
    assert_eq!(result, row["before"].as_str().unwrap());
    result
}
pub(super) fn acquire(source: &str, current: &str) -> String {
    if !matches!(
        source,
        "src/cleanup_plan/replay.rs"
            | "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs"
            | "src/source_reflows_merge.rs"
            | "src/source_reflows_merge/main67107_inputs.rs"
            | "tests/hir_module_boundaries.rs"
            | "tests/workspace/semantic_graph/source_inventory.rs"
    ) {
        return current.to_owned();
    }
    let rows = records();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 6);
    let matching: Vec<_> = rows.iter().filter(|row| row["path"] == source).collect();
    assert!(matching.len() <= 1);
    let Some(row) = matching.first() else {
        return current.to_owned();
    };
    // Only this exact successor is acquired here. Other historical inputs keep
    // their unchanged expected-input owner and strict downstream byte guard.
    if digest(current) != row["after_sha256"] {
        return current.to_owned();
    }
    restore(row, current)
}
#[test]
fn complete_originals_inverse_and_foreign_inputs_remain_independent() {
    let rows = records();
    for row in rows.as_array().unwrap() {
        let path = row["path"].as_str().unwrap();
        let current = CURRENT.iter().find(|(name, _)| *name == path).unwrap().1;
        assert!(current.len() <= 2 * 1024 * 1024);
        // Acquire the complete integrated physical owner before exercising the
        // unchanged complete legacy assertions below on its exact old stage.
        let current = legacy_checkpoint(path, row["before_sha256"].as_str().unwrap(), current);
        let before = restore(row, &current);
        assert_eq!(acquire(path, &current), before);
        assert_eq!(acquire(path, &before), before);
        let bad = format!("{current} ");
        assert!(std::panic::catch_unwind(|| restore(row, &bad)).is_err());
        let mut forged = row.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| restore(&forged, &current)).is_err());
        assert_eq!(acquire("foreign", &current), current);
    }
}

// Distinct expected namespace for the finite integrated six-source successor.
// The original packet, restore and acquire above remain independent controls.
const INTEGRATED_LENGTH: usize = 1539271;
const INTEGRATED_SHA256: &str = "da99e3b9599f64e879ba43646aef4b69f927f566b25007c96cd6d0c932a815bd";
fn integrated_records() -> serde_json::Value {
    let integrated = super::runtime_json::additional(
        super::runtime_json::AdditionalFixture::OrdinaryPlanIntegrated,
    );
    assert!(integrated.len() <= 2 * 1024 * 1024);
    assert_eq!(integrated.len(), INTEGRATED_LENGTH);
    assert_eq!(digest(&integrated), INTEGRATED_SHA256);
    let rows: serde_json::Value = serde_json::from_str(&integrated).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 6);
    rows
}
fn edge(row: &serde_json::Value, text: &str, name: &str) -> String {
    let edge = &row[name];
    assert_eq!(
        digest(text),
        edge["after_sha256"],
        "whole integrated source edge"
    );
    let mut restored = text.to_owned();
    let mut last = restored.len();
    for edit in edge["inverse"].as_array().unwrap() {
        let start = edit["start_byte"].as_u64().unwrap() as usize;
        let after = edit["after"].as_str().unwrap();
        let end = start
            .checked_add(after.len())
            .expect("bounded inverse endpoint");
        assert!(end <= last);
        assert_eq!(restored.get(start..end), Some(after));
        restored.replace_range(start..end, edit["before"].as_str().unwrap());
        last = start;
    }
    assert_eq!(digest(&restored), edge["before_sha256"]);
    restored
}
fn legacy_checkpoint(source: &str, expected: &str, current: &str) -> String {
    let rows = integrated_records();
    let matching = rows
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["path"] == source)
        .collect::<Vec<_>>();
    assert_eq!(matching.len(), 1, "finite integrated source owner");
    let row = matching[0];
    assert_eq!(
        row["expected_sha256"], expected,
        "explicit original expected namespace"
    );
    let (acquired_physical, acquired_current) = final_ingress_checkpoint(source, expected, current);
    let current = acquired_current.as_str();
    let physical = acquired_physical.as_str();
    assert!(physical.len() <= 2 * 1024 * 1024);
    assert_eq!(
        physical.len() as u64,
        row["current_length"].as_u64().unwrap()
    );
    assert_eq!(
        digest(physical),
        row["current_sha256"],
        "whole actual integrated source"
    );
    let checkpoint = edge(row, physical, "current_to_e24");
    assert_eq!(checkpoint, row["e24"].as_str().unwrap());
    let legacy = edge(row, &checkpoint, "e24_to_legacy");
    assert_eq!(legacy, row["legacy"].as_str().unwrap());
    let old = records();
    let old = old
        .as_array()
        .unwrap()
        .iter()
        .find(|old| old["path"] == source)
        .unwrap();
    assert_eq!(digest(&legacy), old["after_sha256"]);
    assert_eq!(row["expected_sha256"], old["before_sha256"]);
    assert!(
        current == physical
            || current == checkpoint
            || current == legacy
            || current == old["before"].as_str().unwrap(),
        "foreign integrated source stage"
    );
    legacy
}
pub(super) fn acquire_expected(source: &str, expected: &str, current: &str) -> String {
    if !matches!(
        source,
        "src/cleanup_plan/replay.rs"
            | "src/source_reflows_merge.rs"
            | "src/source_reflows_merge/main67107_inputs.rs"
            | "tests/hir_module_boundaries.rs"
            | "tests/workspace/semantic_graph/source_inventory.rs"
            | "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs"
    ) {
        return current.to_owned();
    }
    let old = records();
    let matching = old
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["path"] == source)
        .collect::<Vec<_>>();
    assert!(matching.len() <= 1);
    let Some(row) = matching.first() else {
        return current.to_owned();
    };
    // A current/raw or older expected namespace is never normalized here.
    if row["before_sha256"] != expected {
        return current.to_owned();
    }
    let legacy = legacy_checkpoint(source, expected, current);
    restore(row, &legacy)
}
#[test]
fn finite_integrated_expected_namespace_authenticates_every_complete_stage() {
    let rows = integrated_records();
    let old = records();
    for row in rows.as_array().unwrap() {
        let source = row["path"].as_str().unwrap();
        let expected = row["expected_sha256"].as_str().unwrap();
        let physical = CURRENT.iter().find(|(path, _)| *path == source).unwrap().1;
        let original = old
            .as_array()
            .unwrap()
            .iter()
            .find(|old| old["path"] == source)
            .unwrap()["before"]
            .as_str()
            .unwrap();
        for stage in [
            physical,
            row["e24"].as_str().unwrap(),
            row["legacy"].as_str().unwrap(),
            original,
        ] {
            assert_eq!(acquire_expected(source, expected, stage), original);
        }
        assert_eq!(
            acquire_expected(source, &digest(physical), physical),
            physical
        );
        assert_eq!(
            acquire_expected(source, "older selected namespace", "older source stage"),
            "older source stage"
        );
        for bad in [
            String::new(),
            format!("{physical} "),
            format!("{physical}{physical}"),
            "foreign".to_owned(),
        ] {
            assert!(std::panic::catch_unwind(|| acquire_expected(source, expected, &bad)).is_err());
        }
    }
    assert_eq!(
        acquire_expected("foreign", "foreign", "unchanged"),
        "unchanged"
    );
}

// Separate FINAL -> sealed787 edge. Existing integrated/legacy packets are
// immutable; only an exact original expected namespace reaches this reader.
const FINAL_INGRESS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/evidence/ordinary-plan-final-ingress-20261017/current-inputs.json"
));
const FINAL_INGRESS_LENGTH: usize = 786289;
const FINAL_INGRESS_SHA256: &str =
    "31afd14c8fa4c8792aeaec9e58fea785d9414b151cc88812a4b936e1dd3502e2";
const FINAL_INGRESS_CAP: usize = 2 * 1024 * 1024;
// Both complete historical FINAL packets remain independent of the canonical
// producer's next generation. Only the literal current six-source view below
// can enter either historical replay.
const OURS_FINAL_INGRESS: &str =
    include_str!("../../e/2fcdc7c3798717cea1df0172e442cc79368f525d687a0fdd87cac92a30f648d6");
const MAIN_FINAL_INGRESS: &str =
    include_str!("../../e/3d61f09b3235194aba756af2ab9ec48ef7d24d77a5dcdba3190d104a306477fe");
const PRIOR_LIVE_INGRESS: [(&str, usize, &str); 6] = [
    ("src/cleanup_plan/replay.rs", 326143, "392da88cb3c30b1adcff8d845c8af6a797586dfcaab9af2f0c898243f7c8350a"),
    ("src/source_reflows_merge.rs", 102740, "426ddd09476688811850a2a583df6821fd8ee8ad9b469300ae824dcb4958fe3a"),
    ("src/source_reflows_merge/main67107_inputs.rs", 49667, "555d676283e593101b9fc21b8e26f71b1b31b9567c52d3ca402077c4f8bf7fab"),
    ("tests/hir_module_boundaries.rs", 164775, "d38a89d107d7b8c666be6346a7a6a28ad167c740b5ba9c7f97156cc300805169"),
    ("tests/workspace/semantic_graph/source_inventory.rs", 259361, "5a3db1c2f1beae041e3df91b474afe27a67a9e3b1b2de9668a5f549d8a30b47e"),
    ("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", 244150, "d49f3e5888a6368cd2a0271319415fdacd18e8fc9252246bfe34d2bec14d4ae2"),
];
const LIVE_INGRESS: [(&str, usize, &str); 6] = [
    ("src/cleanup_plan/replay.rs", 326143, "392da88cb3c30b1adcff8d845c8af6a797586dfcaab9af2f0c898243f7c8350a"),
    ("src/source_reflows_merge.rs", 102740, "426ddd09476688811850a2a583df6821fd8ee8ad9b469300ae824dcb4958fe3a"),
    ("src/source_reflows_merge/main67107_inputs.rs", 49667, "555d676283e593101b9fc21b8e26f71b1b31b9567c52d3ca402077c4f8bf7fab"),
    ("tests/hir_module_boundaries.rs", 165307, "48cab54b215cb17575b26218b69b0a2dc900111f33f9bfe8eb8b9036457864d7"),
    ("tests/workspace/semantic_graph/source_inventory.rs", 259881, "ec37e8b4352ead6f44068a9f91e67a1279f914a3ff74acf88467abad5f4ffa3e"),
    ("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", 244715, "e06a8003cf54de257aff28af1a78b7be85541b2c520c5506ba5510d25431ec39"),
];
const PRIOR_LIVE_INGRESS_IMAGES: [(&str, &str); 6] = [
    ("src/cleanup_plan/replay.rs", include_str!("../../e/392da88cb3c30b1adcff8d845c8af6a797586dfcaab9af2f0c898243f7c8350a")),
    ("src/source_reflows_merge.rs", include_str!("../../e/426ddd09476688811850a2a583df6821fd8ee8ad9b469300ae824dcb4958fe3a")),
    ("src/source_reflows_merge/main67107_inputs.rs", include_str!("../../e/555d676283e593101b9fc21b8e26f71b1b31b9567c52d3ca402077c4f8bf7fab")),
    ("tests/hir_module_boundaries.rs", include_str!("../../e/d38a89d107d7b8c666be6346a7a6a28ad167c740b5ba9c7f97156cc300805169")),
    ("tests/workspace/semantic_graph/source_inventory.rs", include_str!("../../e/5a3db1c2f1beae041e3df91b474afe27a67a9e3b1b2de9668a5f549d8a30b47e")),
    ("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", include_str!("../../e/d49f3e5888a6368cd2a0271319415fdacd18e8fc9252246bfe34d2bec14d4ae2")),
];

fn final_ingress_same_file(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
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
fn final_ingress_file(path: &std::path::Path, length: usize, expected: &str) -> String {
    use std::io::Read;
    assert!(length <= FINAL_INGRESS_CAP);
    let first = std::fs::symlink_metadata(path).unwrap();
    assert!(first.is_file() && !first.file_type().is_symlink());
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
        panic!("unsupported bounded physical source reader");
    }
    let mut file = options.open(path).unwrap();
    let opened = file.metadata().unwrap();
    assert!(final_ingress_same_file(&first, &opened));
    assert_eq!(opened.len(), u64::try_from(length).unwrap());
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).unwrap();
    bytes.resize(length, 0);
    file.read_exact(&mut bytes).unwrap();
    assert_eq!(file.read(&mut [0u8; 1]).unwrap(), 0);
    let after = file.metadata().unwrap();
    let last = std::fs::symlink_metadata(path).unwrap();
    assert!(
        !last.file_type().is_symlink()
            && final_ingress_same_file(&opened, &after)
            && final_ingress_same_file(&after, &last)
    );
    let text = String::from_utf8(bytes).unwrap();
    assert_eq!(digest(&text), expected);
    text
}

// Actual current sources are acquired before the complete retained LIVE stage.
// This explicit successor changes no old row, inverse, cap, or stage assertion.
const CURRENT_INGRESS_SUCCESSOR: &str = r####"[{"source":"src/cleanup_plan/replay.rs","current":{"length":326576,"sha256":"2ae4da23cf6002ee01f95842c072dc8f80533814090607e442d2da05a5324a16"},"previous":{"length":326143,"sha256":"392da88cb3c30b1adcff8d845c8af6a797586dfcaab9af2f0c898243f7c8350a"},"inverse":[{"start":1455,"end":1489,"current":"#[cfg(test)]\nmod trailing_active;\n","before":""},{"start":121014,"end":121171,"current":"    #[cfg(test)]\n    if trailing_active::before_selected() {\n        return trailing_active::before_sequence_skeleton_paths(prefixes, suffixes, work);\n    }\n","before":""},{"start":122004,"end":122172,"current":"    }\n    #[cfg(test)]\n    if !single && first {\n        trailing_active::reuse!(prefixes, suffixes, work, share_prefix, minimum, precharged, first, single, combined);\n","before":""},{"start":124572,"end":124741,"current":"if !precharged { final_output::record_final_frontier(0, work); }\n        if precharged { precharged = false; }\n        else { work.charge(1, \"skeleton prefix clone\")?; }","before":"final_output::record_final_frontier(0, work);\n        work.charge(1, \"skeleton prefix clone\")?;"}]},{"source":"src/source_reflows_merge.rs","current":{"length":102740,"sha256":"426ddd09476688811850a2a583df6821fd8ee8ad9b469300ae824dcb4958fe3a"},"previous":{"length":102740,"sha256":"426ddd09476688811850a2a583df6821fd8ee8ad9b469300ae824dcb4958fe3a"},"inverse":[]},{"source":"src/source_reflows_merge/main67107_inputs.rs","current":{"length":55585,"sha256":"840ae60fb8cd641c8242e0290df82c7708fa26ff448087f4906faa1367faa038"},"previous":{"length":49667,"sha256":"555d676283e593101b9fc21b8e26f71b1b31b9567c52d3ca402077c4f8bf7fab"},"inverse":[{"start":714,"end":814,"current":"#[allow(dead_code, reason = \"Complete standalone source-custody entry retains its own invocation\")]\n","before":""},{"start":892,"end":1189,"current":"    let mut invocation = super::main_e177_final_inputs::ProjectionInvocation::new();\n    restore_with(&mut invocation, rows, domain, current)\n}\nfn restore_with(invocation: &mut super::main_e177_final_inputs::ProjectionInvocation, rows: &serde_json::Value, domain: &str, current: &str) -> String {\n","before":""},{"start":2318,"end":2336,"current":"_with(invocation, ","before":"("},{"start":2486,"end":2503,"current":"_with(invocation,","before":"("},{"start":2683,"end":2700,"current":"_with(invocation,","before":"("},{"start":2842,"end":2860,"current":"_with(invocation, ","before":"("},{"start":2979,"end":2990,"current":"invocation.","before":"super::main_e177_final_inputs::"},{"start":4211,"end":4512,"current":"let mut invocation = super::main_e177_final_inputs::ProjectionInvocation::new();\n    project_with(&mut invocation, domain, current)\n}\npub(crate) fn project_with(invocation: &mut super::main_e177_final_inputs::ProjectionInvocation, domain: &str, current: &str) -> String {\n    restore_with(invocation, ","before":"restore("},{"start":15861,"end":15961,"current":"#[allow(dead_code, reason = \"Complete standalone source-custody entry retains its own invocation\")]\n","before":""},{"start":16035,"end":16330,"current":"    let mut invocation = super::main_e177_final_inputs::ProjectionInvocation::new();\n    compile35f_restore_with(&mut invocation, row, current)\n}\nfn compile35f_restore_with(invocation: &mut super::main_e177_final_inputs::ProjectionInvocation, row: &serde_json::Value, current: &str) -> String {\n","before":""},{"start":16460,"end":16471,"current":"invocation.","before":"super::main_e177_final_inputs::"},{"start":16653,"end":16671,"current":"_with(invocation, ","before":"("},{"start":16764,"end":16782,"current":"_with(invocation, ","before":"("},{"start":17153,"end":17171,"current":"_with(invocation, ","before":"("},{"start":17269,"end":17287,"current":"_with(invocation, ","before":"("},{"start":17594,"end":17611,"current":"_with(invocation,","before":"("},{"start":17756,"end":17774,"current":"_with(invocation, ","before":"("},{"start":17803,"end":17814,"current":"invocation.","before":"super::main_e177_final_inputs::"},{"start":19227,"end":19521,"current":"    let mut invocation = super::main_e177_final_inputs::ProjectionInvocation::new();\n    compile35f_input_with(&mut invocation, source, current)\n}\npub(crate) fn compile35f_input_with(invocation: &mut super::main_e177_final_inputs::ProjectionInvocation, source: &str, current: &str) -> String {\n","before":""},{"start":19811,"end":19829,"current":"_with(invocation, ","before":"("},{"start":19950,"end":20350,"current":"mut invocation = super::main_e177_final_inputs::ProjectionInvocation::new();\n    compile35f_expected_input_with(&mut invocation, source, expected, current)\n}\npub(super) fn compile35f_expected_input_with(invocation: &mut super::main_e177_final_inputs::ProjectionInvocation, source: &str, expected: &str, current: &str) -> String {\n    let selected = selected_successor_expected_input_with(invocation, ","before":"selected = selected_successor_expected_input("},{"start":20469,"end":20487,"current":"_with(invocation, ","before":"("},{"start":20725,"end":20743,"current":"_with(invocation, ","before":"("},{"start":21262,"end":21280,"current":"_with(invocation, ","before":"("},{"start":26646,"end":26746,"current":"#[allow(dead_code, reason = \"Complete standalone source-custody entry retains its own invocation\")]\n","before":""},{"start":26878,"end":27297,"current":"let mut invocation = super::main_e177_final_inputs::ProjectionInvocation::new();\n    factored_core_file_with(&mut invocation, path, length, expected)\n}\nfn factored_core_file_with(invocation: &mut super::main_e177_final_inputs::ProjectionInvocation,\n    path: &::std::path::Path,\n    length: usize,\n    expected: &str,\n) -> Result<String, &'static str> {\n    if let Some(value) = accepted_expected_file_with(invocation, ","before":"if let Some(value) = accepted_expected_file("},{"start":27388,"end":27399,"current":"invocation.","before":"crate::source_reflows_merge::main_e177_final_inputs::"},{"start":27520,"end":27620,"current":"#[allow(dead_code, reason = \"Complete standalone source-custody entry retains its own invocation\")]\n","before":""},{"start":27672,"end":27943,"current":"    let mut invocation = super::main_e177_final_inputs::ProjectionInvocation::new();\n    factored_core_physical_with(&mut invocation, source)\n}\nfn factored_core_physical_with(invocation: &mut super::main_e177_final_inputs::ProjectionInvocation, source: &str) -> String {\n","before":""},{"start":28086,"end":28104,"current":"_with(invocation, ","before":"("},{"start":28294,"end":28312,"current":"_with(invocation, ","before":"("},{"start":28368,"end":28385,"current":"_with(invocation,","before":"("},{"start":28738,"end":28755,"current":"_with(invocation,","before":"("},{"start":28993,"end":29093,"current":"#[allow(dead_code, reason = \"Complete standalone source-custody entry retains its own invocation\")]\n","before":""},{"start":29157,"end":29446,"current":"    let mut invocation = super::main_e177_final_inputs::ProjectionInvocation::new();\n    factored_core_input_with(&mut invocation, source, current)\n}\nfn factored_core_input_with(invocation: &mut super::main_e177_final_inputs::ProjectionInvocation, source: &str, current: &str) -> String {\n","before":""},{"start":29561,"end":29579,"current":"_with(invocation, ","before":"("},{"start":31263,"end":31363,"current":"#[allow(dead_code, reason = \"Complete standalone source-custody entry retains its own invocation\")]\n","before":""},{"start":31458,"end":31803,"current":"    let mut invocation = super::main_e177_final_inputs::ProjectionInvocation::new();\n    singleton_successor_expected_input_with(&mut invocation, source, expected, current)\n}\nfn singleton_successor_expected_input_with(invocation: &mut super::main_e177_final_inputs::ProjectionInvocation, source: &str, expected: &str, current: &str) -> String {\n","before":""},{"start":32653,"end":32671,"current":"_with(invocation, ","before":"("},{"start":32885,"end":32902,"current":"_with(invocation,","before":"("},{"start":33197,"end":33214,"current":"_with(invocation,","before":"("},{"start":35136,"end":35236,"current":"#[allow(dead_code, reason = \"Complete standalone source-custody entry retains its own invocation\")]\n","before":""},{"start":35310,"end":35619,"current":"    let mut invocation = super::main_e177_final_inputs::ProjectionInvocation::new();\n    corrected_alias_fixture_input_with(&mut invocation, source, current)\n}\nfn corrected_alias_fixture_input_with(invocation: &mut super::main_e177_final_inputs::ProjectionInvocation, source: &str, current: &str) -> String {\n","before":""},{"start":36691,"end":36702,"current":"invocation.","before":"super::main_e177_final_inputs::"},{"start":36880,"end":36891,"current":"invocation.","before":"super::main_e177_final_inputs::"},{"start":37196,"end":37214,"current":"_with(invocation, ","before":"("},{"start":37468,"end":37485,"current":"_with(invocation,","before":"("},{"start":39855,"end":39955,"current":"#[allow(dead_code, reason = \"Complete standalone source-custody entry retains its own invocation\")]\n","before":""},{"start":40052,"end":40401,"current":"    let mut invocation = super::main_e177_final_inputs::ProjectionInvocation::new();\n    supplemental_receiver_expected_input_with(&mut invocation, source, expected, current)\n}\nfn supplemental_receiver_expected_input_with(invocation: &mut super::main_e177_final_inputs::ProjectionInvocation, source: &str, expected: &str, current: &str) -> String {\n","before":""},{"start":40696,"end":40713,"current":"_with(invocation,","before":"("},{"start":41265,"end":41376,"current":"_with(invocation, &root.join(source), length, sha).unwrap();\n    let held = factored_core_file_with(invocation,","before":"(&root.join(source), length, sha).unwrap();\n    let held = factored_core_file("},{"start":41643,"end":41654,"current":"invocation.","before":"crate::source_reflows_merge::main_e177_final_inputs::"},{"start":41849,"end":41860,"current":"invocation.","before":"crate::source_reflows_merge::main_e177_final_inputs::"},{"start":42079,"end":42096,"current":"_with(invocation,","before":"("},{"start":44281,"end":44381,"current":"#[allow(dead_code, reason = \"Complete standalone source-custody entry retains its own invocation\")]\n","before":""},{"start":44475,"end":44818,"current":"    let mut invocation = super::main_e177_final_inputs::ProjectionInvocation::new();\n    selected_successor_expected_input_with(&mut invocation, source, expected, current)\n}\nfn selected_successor_expected_input_with(invocation: &mut super::main_e177_final_inputs::ProjectionInvocation, source: &str, expected: &str, current: &str) -> String {\n","before":""},{"start":46797,"end":46815,"current":"_with(invocation, ","before":"("},{"start":47196,"end":47213,"current":"_with(invocation,","before":"("},{"start":48437,"end":48455,"current":"_with(invocation, ","before":"("},{"start":51226,"end":51326,"current":"#[allow(dead_code, reason = \"Complete standalone source-custody entry retains its own invocation\")]\n","before":""},{"start":51393,"end":51688,"current":"    let mut invocation = super::main_e177_final_inputs::ProjectionInvocation::new();\n    owned_compile35f_input_with(&mut invocation, source, current)\n}\nfn owned_compile35f_input_with(invocation: &mut super::main_e177_final_inputs::ProjectionInvocation, source: &str, current: &str) -> String {\n","before":""},{"start":52007,"end":52018,"current":"invocation.","before":"crate::source_reflows_merge::main_e177_final_inputs::"},{"start":52176,"end":52194,"current":"_with(invocation, ","before":"("},{"start":52212,"end":52312,"current":"#[allow(dead_code, reason = \"Complete standalone source-custody entry retains its own invocation\")]\n","before":""},{"start":52388,"end":52701,"current":"    let mut invocation = super::main_e177_final_inputs::ProjectionInvocation::new();\n    owned_compile35f_input_previous_with(&mut invocation, source, current)\n}\nfn owned_compile35f_input_previous_with(invocation: &mut super::main_e177_final_inputs::ProjectionInvocation, source: &str, current: &str) -> String {\n","before":""},{"start":53756,"end":53826,"current":"invocation: &mut super::main_e177_final_inputs::ProjectionInvocation, ","before":""},{"start":54014,"end":54031,"current":"_with(invocation,","before":"("},{"start":54188,"end":54200,"current":"invocation, ","before":""},{"start":54336,"end":54347,"current":"invocation,","before":""},{"start":54503,"end":54514,"current":"invocation,","before":""},{"start":54652,"end":54663,"current":"invocation,","before":""}]},{"source":"tests/hir_module_boundaries.rs","current":{"length":165766,"sha256":"bcd8f3ae73aa3eb9874f26f032c333958069f77c9628316fcc092a3a91536502"},"previous":{"length":165307,"sha256":"48cab54b215cb17575b26218b69b0a2dc900111f33f9bfe8eb8b9036457864d7"},"inverse":[{"start":145661,"end":146120,"current":"trailing_active.rs\"),include_str!(\"../src/cleanup_plan/replay/trailing_active/tests.rs\"),include_str!(\"../src/hir/validation/scope_checkpoint/merge/partial_retain.rs\"),include_str!(\"../src/hir/validation/scope_checkpoint/merge/partial_retain/before.rs\"),include_str!(\"../src/hir/validation/scope_checkpoint/merge/partial_retain/tests.rs\"),include_str!(\"../src/cleanup_plan/replay/factored/paired/visited_transfer.rs\"),include_str!(\"../src/cleanup_plan/replay/","before":""}]},{"source":"tests/workspace/semantic_graph/source_inventory.rs","current":{"length":260358,"sha256":"ae1578dc985752abc2a7dece0d3d9741afc7c5d70aa6c1d79e7211ac230bf897"},"previous":{"length":259881,"sha256":"ec37e8b4352ead6f44068a9f91e67a1279f914a3ff74acf88467abad5f4ffa3e"},"inverse":[{"start":26913,"end":27390,"current":"trailing_active.rs\"),include_str!(\"../../src/cleanup_plan/replay/trailing_active/tests.rs\"),include_str!(\"../../src/hir/validation/scope_checkpoint/merge/partial_retain.rs\"),include_str!(\"../../src/hir/validation/scope_checkpoint/merge/partial_retain/before.rs\"),include_str!(\"../../src/hir/validation/scope_checkpoint/merge/partial_retain/tests.rs\"),include_str!(\"../../src/cleanup_plan/replay/factored/paired/visited_transfer.rs\"),include_str!(\"../../src/cleanup_plan/replay/","before":""}]},{"source":"crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs","current":{"length":245246,"sha256":"72fd391d36446797db40734c64def6012ecd4471a901502ccd33dcbe4b8f1aa8"},"previous":{"length":244715,"sha256":"e06a8003cf54de257aff28af1a78b7be85541b2c520c5506ba5510d25431ec39"},"inverse":[{"start":50291,"end":50822,"current":"trailing_active.rs\"),include_str!(\"../../../../../src/cleanup_plan/replay/trailing_active/tests.rs\"),include_str!(\"../../../../../src/hir/validation/scope_checkpoint/merge/partial_retain.rs\"),include_str!(\"../../../../../src/hir/validation/scope_checkpoint/merge/partial_retain/before.rs\"),include_str!(\"../../../../../src/hir/validation/scope_checkpoint/merge/partial_retain/tests.rs\"),include_str!(\"../../../../../src/cleanup_plan/replay/factored/paired/visited_transfer.rs\"),include_str!(\"../../../../../src/cleanup_plan/replay/","before":""}]}]"####;
fn current_ingress_view(root: &std::path::Path, source: &str, supplied: &str) -> String {
    let (position, (_, compiled)) = CURRENT.iter().enumerate().find(|(_, r)| r.0 == source).expect("six compiled ingress owners only");
    assert_eq!(supplied, *compiled, "actual compiled source before historical normalization");
    assert!(CURRENT_INGRESS_SUCCESSOR.len() <= FINAL_INGRESS_CAP);
    let packet: serde_json::Value = serde_json::from_str(CURRENT_INGRESS_SUCCESSOR).unwrap();
    let rows = packet.as_array().unwrap();
    assert_eq!(rows.len(), CURRENT.len());
    for (row, owner) in rows.iter().zip(CURRENT) { assert_eq!(row["source"], owner.0); }
    let row = &rows[position];
    let length = usize::try_from(row["current"]["length"].as_u64().unwrap()).unwrap();
    let sha = row["current"]["sha256"].as_str().unwrap();
    assert_eq!((compiled.len(), digest(compiled)), (length, sha.to_owned()));
    let actual = final_ingress_file(&root.join(source), length, sha);
    assert_eq!(actual, *compiled);
    let (_, old_length, old_sha) = LIVE_INGRESS[position];
    assert_eq!(row["previous"], serde_json::json!({"length":old_length,"sha256":old_sha}));
    let prior = final_ingress_file(&root.join(format!("e/{old_sha}")), old_length, old_sha);
    let edits = row["inverse"].as_array().unwrap(); assert!(edits.len() <= 512);
    let mut restored = actual.clone(); let mut next = actual.len();
    for edit in edits.iter().rev() {
        let lo = usize::try_from(edit["start"].as_u64().unwrap()).unwrap();
        let hi = usize::try_from(edit["end"].as_u64().unwrap()).unwrap();
        assert!(lo <= hi && hi <= next);
        assert_eq!(restored.get(lo..hi), Some(edit["current"].as_str().unwrap()));
        restored.replace_range(lo..hi, edit["before"].as_str().unwrap());
        assert!(restored.len() <= FINAL_INGRESS_CAP); next = lo;
    }
    assert_eq!(restored, prior, "ENTIRE fixed LIVE source before unchanged earlier stages");
    assert_eq!(retained_current_ingress_view(root, source, &restored), restored);
    assert_eq!(final_ingress_file(&root.join(format!("e/{old_sha}")), old_length, old_sha), prior);
    assert_eq!(final_ingress_file(&root.join(source), length, sha), actual);
    actual
}
fn retained_current_ingress_view(root: &std::path::Path, source: &str, compiled: &str) -> String {
    let (_, length, expected) = LIVE_INGRESS
        .iter()
        .find(|(path, _, _)| *path == source)
        .expect("six literal current ingress sources only");
    let actual = final_ingress_file(&root.join(format!("e/{expected}")), *length, expected);
    assert_eq!(actual, compiled, "complete current compiled ingress source");
    let packet = latest_ingress_records();
    let epoch = &packet["current_source_epoch"];
    assert_eq!(epoch["schema"], "semaprax.ordinary-final-ingress-current-source-epoch.v1");
    let rows = epoch["rows"].as_array().unwrap();
    assert_eq!(rows.len(), LIVE_INGRESS.len());
    assert_eq!(rows.iter().map(|r| r["source"].as_str().unwrap()).collect::<Vec<_>>(), LIVE_INGRESS.iter().map(|r| r.0).collect::<Vec<_>>());
    let position = LIVE_INGRESS.iter().position(|r| r.0 == source).unwrap();
    let row = &rows[position];
    assert_eq!(row["current"], serde_json::json!({"length": length, "sha256": expected}));
    let (prior_source, prior_length, prior_sha) = PRIOR_LIVE_INGRESS[position];
    assert_eq!(prior_source, source);
    assert_eq!(row["previous"], serde_json::json!({"length": prior_length, "sha256": prior_sha}));
    assert_eq!(row["previous_archive"], format!("e/{prior_sha}"));
    let prior_compiled = PRIOR_LIVE_INGRESS_IMAGES[position];
    assert_eq!(prior_compiled.0, source);
    assert_eq!((prior_compiled.1.len(), digest(prior_compiled.1)), (prior_length, prior_sha.to_owned()));
    let prior = final_ingress_file(&root.join(format!("e/{prior_sha}")), prior_length, prior_sha);
    assert_eq!(prior, prior_compiled.1);
    let mut restored = actual.clone();
    let edits = row["inverse"].as_array().unwrap();
    assert!(edits.len() <= 512);
    let mut next = restored.len();
    for edit in edits.iter().rev() {
        let start = usize::try_from(edit["start"].as_u64().unwrap()).unwrap();
        let end = usize::try_from(edit["end"].as_u64().unwrap()).unwrap();
        assert!(start <= end && end <= next);
        assert!(restored.is_char_boundary(start) && restored.is_char_boundary(end));
        assert_eq!(&restored[start..end], edit["after"].as_str().unwrap());
        restored.replace_range(start..end, edit["before"].as_str().unwrap());
        assert!(restored.len() <= FINAL_INGRESS_CAP);
        next = start;
    }
    assert_eq!(restored, prior, "entire actual source to independently retained predecessor");
    assert_eq!(final_ingress_file(&root.join(format!("e/{prior_sha}")), prior_length, prior_sha), prior_compiled.1);
    assert_eq!(final_ingress_file(&root.join(format!("e/{expected}")), *length, expected), actual);
    actual
}
fn historical_final_records(text: &str, length: usize, expected: &str) -> serde_json::Value {
    assert!(text.len() <= FINAL_INGRESS_CAP);
    assert_eq!(text.len(), length);
    assert_eq!(digest(text), expected);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        final_ingress_file(&root.join(format!("e/{expected}")), length, expected),
        text
    );
    let rows: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), CURRENT.len());
    rows
}
// Each packet independently reconstructs the entire fixed checkpoint. A live
// source is authenticated separately; archived bytes never stand in for it.
fn replay_final_history(
    row: &serde_json::Value,
    frozen: &serde_json::Value,
    old: &serde_json::Value,
    path: &str,
    root: &std::path::Path,
) -> (String, String) {
    assert_eq!(row["path"], path);
    assert_eq!(row["expected_sha256"], old["before_sha256"]);
    assert_eq!(row["checkpoint_sha256"], frozen["current_sha256"]);
    assert_eq!(row["checkpoint_length"], frozen["current_length"]);
    let physical = final_ingress_file(
        &root.join(row["current_archive"].as_str().unwrap()),
        usize::try_from(row["current_length"].as_u64().unwrap()).unwrap(),
        row["current_sha256"].as_str().unwrap(),
    );
    let checkpoint = final_ingress_file(
        &root.join(row["checkpoint_archive"].as_str().unwrap()),
        usize::try_from(row["checkpoint_length"].as_u64().unwrap()).unwrap(),
        row["checkpoint_sha256"].as_str().unwrap(),
    );
    let mut restored = physical.clone();
    let mut last = restored.len();
    let inverse = row["inverse"].as_array().unwrap();
    assert!(inverse.len() <= 512);
    for edit in inverse {
        let start = usize::try_from(edit["start_byte"].as_u64().unwrap()).unwrap();
        let after = edit["after"].as_str().unwrap();
        let end = start
            .checked_add(after.len())
            .expect("bounded historical final inverse endpoint");
        assert!(end <= last);
        assert_eq!(restored.get(start..end), Some(after));
        restored.replace_range(start..end, edit["before"].as_str().unwrap());
        last = start;
    }
    assert_eq!(
        restored, checkpoint,
        "complete historical final to frozen787 inverse"
    );
    (physical, checkpoint)
}

// A separate newest->FINAL phase. The sealed FINAL, integrated and legacy
// records below remain unchanged and replay their original inverses.
fn latest_ingress_records() -> serde_json::Value {
    let data = include_str!(
        "../../docs/evidence/source-coverage-254-20261024/latest-ordinary-inputs.json"
    );
    assert!(data.len() <= FINAL_INGRESS_CAP);
    assert_eq!(data.len(), 498173);
    assert_eq!(
        digest(data),
        "f713e382b4909d838ac484af8fb83674cd7a9d3adb5af3622c537c6aa03a0ceb"
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        final_ingress_file(
            &root.join("docs/evidence/source-coverage-254-20261024/latest-ordinary-inputs.json"),
            data.len(),
            "f713e382b4909d838ac484af8fb83674cd7a9d3adb5af3622c537c6aa03a0ceb"
        ),
        data
    );
    let result: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(
        result["schema"],
        "semaprax.ordinary-final-ingress-latest.v1"
    );
    assert_eq!(result["rows"].as_array().unwrap().len(), CURRENT.len());
    result
}

// Entire incoming f65 successor source views are compiled historical archives.
// They never replace the separately authenticated current physical owners.
const INCOMING_SUCCESSOR_VIEWS: [(&str, &str); 6] = [
    (
        "src/cleanup_plan/replay.rs",
        include_str!("../../e/bdce82425869a91e406c78175753f19ff0db46a468c4bba33f8bef793b4fe234"),
    ),
    (
        "src/source_reflows_merge.rs",
        include_str!("../../e/5e5db20c191765da259132014f7945f5b651208102778a2381821f31d0805229"),
    ),
    (
        "src/source_reflows_merge/main67107_inputs.rs",
        include_str!("../../e/555d676283e593101b9fc21b8e26f71b1b31b9567c52d3ca402077c4f8bf7fab"),
    ),
    (
        "tests/hir_module_boundaries.rs",
        include_str!("../../e/246d91d06278340a16c3ef24d6660a292f68f74ff4533e87ba349c062956f4f4"),
    ),
    (
        "tests/workspace/semantic_graph/source_inventory.rs",
        include_str!("../../e/0144e54abac27fcfc032f9bc873fa616539da1a2b20de17495b84f68428104e7"),
    ),
    (
        "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs",
        include_str!("../../e/3a43d350fbe971a7c3bb46000b559fb3c1bc5995e196953382068149ee82c96f"),
    ),
];
fn historical_successor_ingress_checkpoint(
    path: &str,
    compiled: &str,
    previous: &serde_json::Value,
) -> (String, String) {
    const PATH: &str = "docs/evidence/ordinary-ingress-successor-20261026/current-inputs.json";
    const DATA: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/ordinary-ingress-successor-20261026/current-inputs.json"
    ));
    const LENGTH: usize = 14971;
    const SHA: &str = "aef6ee23a1724873cd15d3590f0086fc8a304ce3db94ca4525989994e8360046";
    const { assert!(LENGTH <= FINAL_INGRESS_CAP) };
    assert_eq!(DATA.len(), LENGTH);
    assert_eq!(digest(DATA), SHA);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(final_ingress_file(&root.join(PATH), LENGTH, SHA), DATA);
    let packet: serde_json::Value = serde_json::from_str(DATA).unwrap();
    assert_eq!(
        packet["schema"],
        "semaprax.ordinary-plan-source-successor.v1"
    );
    let rows = packet["rows"].as_array().unwrap();
    assert_eq!(rows.len(), CURRENT.len());
    for (row, (source, _)) in rows.iter().zip(CURRENT) {
        assert_eq!(row["path"], source, "closed ordered physical source owners");
    }
    let row = rows.iter().find(|row| row["path"] == path).unwrap();
    assert_eq!(row["previous_length"], previous["current_length"]);
    assert_eq!(row["previous_sha256"], previous["current_sha256"]);
    assert_eq!(row["previous_archive"], previous["current_archive"]);
    let length = usize::try_from(row["current_length"].as_u64().unwrap()).unwrap();
    let sha = row["current_sha256"].as_str().unwrap();
    let historical_archive = row["current_archive"].as_str().unwrap();
    assert_eq!(historical_archive, format!("e/{sha}"));
    let live = final_ingress_file(&root.join(historical_archive), length, sha);
    assert_eq!(
        live, compiled,
        "complete incoming compiled and physical historical archive"
    );
    let archive = row["current_archive"].as_str().unwrap();
    assert_eq!(archive, format!("e/{sha}"));
    assert_eq!(final_ingress_file(&root.join(archive), length, sha), live);
    let mut restored = live.clone();
    let edits = row["inverse"].as_array().unwrap();
    assert!(edits.len() <= 512);
    let mut last = restored.len();
    for edit in edits {
        let start = usize::try_from(edit["start_byte"].as_u64().unwrap()).unwrap();
        let after = edit["after"].as_str().unwrap();
        let end = start
            .checked_add(after.len())
            .expect("bounded successor inverse");
        assert!(end <= last);
        assert_eq!(restored.get(start..end), Some(after));
        restored.replace_range(start..end, edit["before"].as_str().unwrap());
        last = start;
    }
    assert_eq!(
        restored.len(),
        usize::try_from(previous["current_length"].as_u64().unwrap()).unwrap()
    );
    assert_eq!(digest(&restored), previous["current_sha256"]);
    assert_eq!(
        restored,
        final_ingress_file(
            &root.join(previous["current_archive"].as_str().unwrap()),
            restored.len(),
            previous["current_sha256"].as_str().unwrap(),
        )
    );
    (live, restored)
}

fn final_ingress_checkpoint(source: &str, expected: &str, current: &str) -> (String, String) {
    assert!(FINAL_INGRESS.len() <= FINAL_INGRESS_CAP);
    assert_eq!(FINAL_INGRESS.len(), FINAL_INGRESS_LENGTH);
    assert_eq!(digest(FINAL_INGRESS), FINAL_INGRESS_SHA256);
    let canonical: serde_json::Value = serde_json::from_str(FINAL_INGRESS).unwrap();
    let canonical = canonical.as_array().unwrap();
    assert_eq!(canonical.len(), CURRENT.len());
    let ours_packet = historical_final_records(
        OURS_FINAL_INGRESS,
        706318,
        "2fcdc7c3798717cea1df0172e442cc79368f525d687a0fdd87cac92a30f648d6",
    );
    let ours = ours_packet.as_array().unwrap();
    let main_packet = historical_final_records(
        MAIN_FINAL_INGRESS,
        455009,
        "3d61f09b3235194aba756af2ab9ec48ef7d24d77a5dcdba3190d104a306477fe",
    );
    let rows = main_packet.as_array().unwrap();
    assert_eq!(rows.len(), CURRENT.len());
    let frozen = integrated_records();
    let frozen = frozen.as_array().unwrap();
    let old = records();
    let old = old.as_array().unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut selected = None;
    let latest_packet = latest_ingress_records();
    let latest_rows = latest_packet["rows"].as_array().unwrap();
    for (position, ((((row, frozen), old), (path, compiled)), newest)) in rows
        .iter()
        .zip(frozen)
        .zip(old)
        .zip(CURRENT)
        .zip(latest_rows)
        .enumerate()
    {
        assert_eq!(row["path"], path);
        assert_eq!(frozen["path"], path);
        assert_eq!(old["path"], path);
        assert_eq!(row["expected_sha256"], old["before_sha256"]);
        assert_eq!(row["checkpoint_sha256"], frozen["current_sha256"]);
        assert_eq!(row["checkpoint_length"], frozen["current_length"]);
        let length = usize::try_from(row["current_length"].as_u64().unwrap()).unwrap();
        assert_eq!(newest["path"], path);
        assert_eq!(newest["previous_length"], row["current_length"]);
        assert_eq!(newest["previous_sha256"], row["current_sha256"]);
        let latest_length = usize::try_from(newest["current_length"].as_u64().unwrap()).unwrap();
        let latest_sha = newest["current_sha256"].as_str().unwrap();
        let actual = current_ingress_view(root, path, compiled);
        let latest = final_ingress_file(
            &root.join(newest["current_archive"].as_str().unwrap()),
            latest_length,
            latest_sha,
        );
        assert_eq!(
            actual, compiled,
            "actual complete compiled and physical final source"
        );
        let historical_compiled = INCOMING_SUCCESSOR_VIEWS
            .iter()
            .find(|(name, _)| *name == path)
            .expect("six literal incoming historical owners only")
            .1;
        let (incoming_successor, incoming_latest) =
            historical_successor_ingress_checkpoint(path, historical_compiled, newest);
        assert_eq!(incoming_latest, latest, "both entire common latest stages agree");
        let latest_archive = newest["current_archive"].as_str().unwrap();
        assert_eq!(latest_archive, format!("e/{latest_sha}"));
        assert_eq!(
            latest,
            final_ingress_file(&root.join(latest_archive), latest_length, latest_sha)
        );
        let mut physical = latest.clone();
        let mut last_latest = physical.len();
        let latest_inverse = newest["inverse"].as_array().unwrap();
        assert!(latest_inverse.len() <= 512);
        for edit in latest_inverse {
            let start = usize::try_from(edit["start_byte"].as_u64().unwrap()).unwrap();
            let after = edit["after"].as_str().unwrap();
            let end = start
                .checked_add(after.len())
                .expect("bounded newest inverse endpoint");
            assert!(end <= last_latest);
            assert_eq!(physical.get(start..end), Some(after));
            physical.replace_range(start..end, edit["before"].as_str().unwrap());
            last_latest = start;
        }
        assert_eq!(physical.len(), length);
        assert_eq!(digest(&physical), row["current_sha256"]);
        let retained = final_ingress_file(
            &root.join(row["current_archive"].as_str().unwrap()),
            length,
            row["current_sha256"].as_str().unwrap(),
        );
        assert_eq!(physical, retained);
        let checkpoint = final_ingress_file(
            &root.join(row["checkpoint_archive"].as_str().unwrap()),
            usize::try_from(row["checkpoint_length"].as_u64().unwrap()).unwrap(),
            row["checkpoint_sha256"].as_str().unwrap(),
        );
        let mut restored = physical.clone();
        let mut last = restored.len();
        let inverse = row["inverse"].as_array().unwrap();
        assert!(inverse.len() <= 512);
        for edit in inverse {
            let start = usize::try_from(edit["start_byte"].as_u64().unwrap()).unwrap();
            let after = edit["after"].as_str().unwrap();
            let end = start
                .checked_add(after.len())
                .expect("bounded final inverse endpoint");
            assert!(end <= last);
            assert_eq!(restored.get(start..end), Some(after));
            restored.replace_range(start..end, edit["before"].as_str().unwrap());
            last = start;
        }
        assert_eq!(restored, checkpoint, "complete final to frozen787 inverse");
        let (canonical_physical, canonical_checkpoint) =
            replay_final_history(&canonical[position], frozen, old, path, root);
        let (ours_physical, ours_checkpoint) =
            replay_final_history(&ours[position], frozen, old, path, root);
        assert_eq!(
            canonical_checkpoint, checkpoint,
            "canonical and incoming complete checkpoints agree"
        );
        assert_eq!(
            ours_checkpoint, checkpoint,
            "retained ours and incoming complete checkpoints agree"
        );
        assert_eq!(current_ingress_view(root, path, compiled), actual, "late whole source after all original ingress routes");
        if path == source {
            assert_eq!(
                row["expected_sha256"], expected,
                "exact original c874 expected namespace"
            );
            assert!(
                current == actual
                    || current == incoming_successor
                    || current == canonical_physical
                    || current == ours_physical
                    || current == latest
                    || current == physical
                    || current == checkpoint
                    || current == frozen["e24"].as_str().unwrap()
                    || current == frozen["legacy"].as_str().unwrap()
                    || current == old["before"].as_str().unwrap(),
                "foreign final ingress stage"
            );
            let supplied = if current == actual
                || current == incoming_successor
                || current == canonical_physical
                || current == ours_physical
                || current == latest
                || current == physical
            {
                checkpoint.clone()
            } else {
                current.to_owned()
            };
            selected = Some((checkpoint, supplied));
        }
    }
    selected.expect("six literal final source owners only")
}
#[test]
fn final_ingress_six_physical_owners_preserve_frozen_stages_and_expected_dispatch() {
    let frozen = integrated_records();
    let old = records();
    for ((row, old), (source, physical)) in frozen
        .as_array()
        .unwrap()
        .iter()
        .zip(old.as_array().unwrap())
        .zip(CURRENT)
    {
        let expected = old["before_sha256"].as_str().unwrap();
        let (checkpoint, supplied) = final_ingress_checkpoint(source, expected, physical);
        assert_eq!(checkpoint, supplied);
        assert_eq!(digest(&checkpoint), row["current_sha256"]);
        for staged in [
            checkpoint.as_str(),
            row["e24"].as_str().unwrap(),
            row["legacy"].as_str().unwrap(),
            old["before"].as_str().unwrap(),
        ] {
            let (again, supplied) = final_ingress_checkpoint(source, expected, staged);
            assert_eq!(again, checkpoint);
            assert_eq!(supplied, staged);
            assert_eq!(
                acquire_expected(source, expected, staged),
                old["before"].as_str().unwrap()
            );
        }
        assert_eq!(
            acquire_expected(source, &digest(physical), physical),
            physical
        );
        assert_eq!(
            acquire_expected(source, "older expected namespace", "older stage"),
            "older stage"
        );
        for bad in [String::new(), format!("{physical} "), "foreign".to_owned()] {
            assert!(
                std::panic::catch_unwind(|| final_ingress_checkpoint(source, expected, &bad))
                    .is_err()
            );
        }
    }
}

// Exact root-only custody entry. It shares the complete six-owner FINAL
// authenticator; the caller supplies actual compiled root and flat old owner.
pub(super) fn original_root_checkpoint(current: &str, compiled_before: &str) -> String {
    assert_eq!(CURRENT[0].0, "src/cleanup_plan/replay.rs");
    assert_eq!(
        current, CURRENT[0].1,
        "custody entry requires actual compiled root"
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let actual_before = final_ingress_file(
        &root.join("src/cleanup_plan/replay/ordinary_plan_history/before.rs"),
        27915,
        "9e07f08634f9a79e9d38955da089790acf951521362e46ba23b073e67e9ba3a5",
    );
    assert_eq!(
        actual_before, compiled_before,
        "complete current compiled flat predecessor"
    );
    let legacy = records();
    assert_eq!(legacy[0]["path"], "src/cleanup_plan/replay.rs");
    let (checkpoint, _) = final_ingress_checkpoint(
        "src/cleanup_plan/replay.rs",
        legacy[0]["before_sha256"].as_str().unwrap(),
        current,
    );
    checkpoint
}
