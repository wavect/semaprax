//! Test-only current-byte custody before the unchanged empty-ensures proof.
use sha2::{Digest, Sha256};
fn hash(source: &str) -> String {
    format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))
    )
}
fn restore(current: &str, receiver: &str) -> String {
    let binding: serde_json::Value =
        serde_json::from_str(include_str!("source_inverse.json")).unwrap();
    assert_eq!(
        hash(current),
        binding["root_current_sha256"].as_str().unwrap()
    );
    // Enter the unchanged receiver epoch through its exact visibility inverse.
    let ingress = &binding["receiver_visibility"];
    assert_eq!(receiver.len() as u64, ingress["current_length"].as_u64().unwrap());
    assert_eq!(hash(receiver), ingress["current_sha256"].as_str().unwrap());
    let after = ingress["after"].as_str().unwrap();
    assert_eq!(receiver.matches(after).count(), 1);
    let receiver_before = receiver.replacen(after, ingress["before"].as_str().unwrap(), 1);
    assert_eq!(receiver_before.len() as u64, ingress["before_length"].as_u64().unwrap());
    assert_eq!(hash(&receiver_before), ingress["before_sha256"].as_str().unwrap());
    let receiver = receiver_before.as_str();
    assert_eq!(
        hash(receiver),
        binding["receiver_current_sha256"].as_str().unwrap()
    );
    let mut body = receiver.as_bytes().to_vec();
    for edit in binding["edits"].as_array().unwrap().iter().rev() {
        let start = usize::try_from(edit["start"].as_u64().unwrap()).unwrap();
        let end = usize::try_from(edit["end"].as_u64().unwrap()).unwrap();
        assert_eq!(
            body.get(start..end).unwrap(),
            edit["current"].as_str().unwrap().as_bytes()
        );
        body.splice(
            start..end,
            edit["before"].as_str().unwrap().as_bytes().iter().copied(),
        );
    }
    let body = String::from_utf8(body).unwrap();
    assert_eq!(
        hash(&body),
        binding["receiver_before_sha256"].as_str().unwrap()
    );
    let wrapper = binding["wrapper_current"].as_str().unwrap();
    assert_eq!(current.matches(wrapper).count(), 1);
    let mut restored = current.replacen(wrapper, &body, 1);
    for (current, previous) in [
        (
            "    #[cfg(test)]\n    fn validate_expr_persistent(",
            "    fn validate_expr_persistent(",
        ),
        ("pub(super) mod lifecycle;", "mod lifecycle;"),
    ] {
        assert_eq!(restored.matches(current).count(), 1);
        restored = restored.replacen(current, previous, 1);
    }
    assert_eq!(
        hash(&restored),
        binding["root_before_sha256"].as_str().unwrap()
    );
    restored
}
pub(in crate::hir::validation) fn function_scope(current: &str) -> String {
    restore(current, include_str!("function.rs"))
}
// Authenticate every relocated child before the fixed whole-source ingress.
pub(in crate::hir::validation) fn whole_root(source: &str, current: &str) -> String {
    let root_before;
    let current = if source == "src/hir/validation.rs" {
        root_before = super::super::lexical_exit::lexical_join::before_source(source, current)
            .expect("exact physical current validation root");
        root_before.as_str()
    } else { current };
    let lexical_join_before = super::super::lexical_exit::lexical_join::before_source(source, current);
    let current = lexical_join_before.as_deref().unwrap_or(current);
    let observed;
    let current = if source == "src/hir/validation/lexical_exit.rs" {
        observed = crate::source_reflows::batch_913_entry(source, current)
            .expect("complete diagnostic observer ingress");
        observed.as_str()
    } else {
        current
    };
    let binding: serde_json::Value = serde_json::from_str(include_str!("source_inverse.json")).unwrap();
    for (path, compiled) in [
        ("src/hir/validation/continuation_frame.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/continuation_frame.rs"))),
        ("src/hir/validation/frame_reuse.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/frame_reuse.rs"))),
        ("src/hir/validation/frame_reuse/before.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/frame_reuse/before.rs"))),
        ("src/hir/validation/frame_reuse/before_0.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/frame_reuse/before_0.rs"))),
        ("src/hir/validation/frame_reuse/before_1.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/frame_reuse/before_1.rs"))),
        ("src/hir/validation/frame_reuse/before_10.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/frame_reuse/before_10.rs"))),
        ("src/hir/validation/frame_reuse/before_11.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/frame_reuse/before_11.rs"))),
        ("src/hir/validation/frame_reuse/before_12.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/frame_reuse/before_12.rs"))),
        ("src/hir/validation/frame_reuse/before_13.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/frame_reuse/before_13.rs"))),
        ("src/hir/validation/frame_reuse/before_14.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/frame_reuse/before_14.rs"))),
        ("src/hir/validation/frame_reuse/before_15.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/frame_reuse/before_15.rs"))),
        ("src/hir/validation/frame_reuse/before_16.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/frame_reuse/before_16.rs"))),
        ("src/hir/validation/frame_reuse/before_17.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/frame_reuse/before_17.rs"))),
        ("src/hir/validation/frame_reuse/before_2.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/frame_reuse/before_2.rs"))),
        ("src/hir/validation/frame_reuse/before_3.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/frame_reuse/before_3.rs"))),
        ("src/hir/validation/frame_reuse/before_4.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/frame_reuse/before_4.rs"))),
        ("src/hir/validation/frame_reuse/before_5.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/frame_reuse/before_5.rs"))),
        ("src/hir/validation/frame_reuse/before_6.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/frame_reuse/before_6.rs"))),
        ("src/hir/validation/frame_reuse/before_7.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/frame_reuse/before_7.rs"))),
        ("src/hir/validation/frame_reuse/before_8.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/frame_reuse/before_8.rs"))),
        ("src/hir/validation/frame_reuse/before_9.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/frame_reuse/before_9.rs"))),
        ("src/hir/validation/frame_reuse/contracts.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/frame_reuse/contracts.rs"))),
        ("src/hir/validation/frame_reuse/contracts_before.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/frame_reuse/contracts_before.rs"))),
        ("src/hir/validation/frame_reuse/custody.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/frame_reuse/custody.rs"))),
        ("src/hir/validation/frame_reuse/function.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/frame_reuse/function.rs"))),
        ("src/hir/validation/frame_reuse/function_before.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/frame_reuse/function_before.rs"))),
        ("src/hir/validation/frame_reuse/source_inverse.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/frame_reuse/source_inverse.rs"))),
        ("src/hir/validation/frame_reuse/tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/frame_reuse/tests.rs"))),
    ] {
        let physical_compiled = compiled;
        let lexical_join_before = super::super::lexical_exit::lexical_join::before_source(path, compiled);
        let compiled = lexical_join_before.as_deref().unwrap_or(compiled);
        let predecessor = if matches!(path, "src/hir/validation/frame_reuse/tests.rs" | "src/hir/validation/frame_reuse/source_inverse.rs") {
            crate::source_reflows::batch_913_entry(path, physical_compiled)
        } else {
            None
        };
        let authenticated = predecessor.as_deref().unwrap_or(compiled);
        assert_eq!(hash(authenticated), binding["whole_children"][path].as_str().unwrap());
    }
    let row = &binding["whole_roots"][source];
    assert_eq!(current.len() as u64, row["current_length"].as_u64().unwrap());
    assert_eq!(hash(current), row["current_sha256"].as_str().unwrap());
    let mut bytes = current.as_bytes().to_vec();
    for edit in row["edits"].as_array().unwrap().iter().rev() {
        let start = usize::try_from(edit["start"].as_u64().unwrap()).unwrap();
        let end = usize::try_from(edit["end"].as_u64().unwrap()).unwrap();
        assert_eq!(bytes.get(start..end).unwrap(), edit["current"].as_str().unwrap().as_bytes());
        bytes.splice(start..end, edit["before"].as_str().unwrap().as_bytes().iter().copied());
    }
    let restored = String::from_utf8(bytes).unwrap();
    assert_eq!(restored.len() as u64, row["before_length"].as_u64().unwrap());
    assert_eq!(hash(&restored), row["before_sha256"].as_str().unwrap());
    restored
}
#[test]
fn relocated_function_scope_requires_complete_root_and_receiver_before_unchanged_proof() {
    let current = include_str!("../function_scope.rs");
    let receiver = include_str!("function.rs");
    for (path, current) in [
        ("src/hir/validation.rs", include_str!("../../validation.rs")),
        ("src/hir/validation/lexical_exit.rs", include_str!("../lexical_exit.rs")),
    ] {
        assert!(std::panic::catch_unwind(|| whole_root(path, &(current.to_owned() + " "))).is_err());
        let restored = whole_root(path, current);
        assert!(std::panic::catch_unwind(|| whole_root(path, &restored)).is_err());
    }
    assert_eq!(
        hash(&restore(current, receiver)),
        "2cb5bf4ab361f172241d1c148788a5db09fdb264471818091ea1d83a1be41426"
    );
    for (root, body) in [
        (current.to_owned() + " ", receiver.to_owned()),
        (current.to_owned(), receiver.to_owned() + " "),
    ] {
        assert!(std::panic::catch_unwind(|| restore(&root, &body)).is_err());
    }
}
