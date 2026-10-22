//! Test-only current-byte custody before the unchanged empty-ensures proof.
use sha2::{Digest, Sha256};
fn hash(source: &str) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes())))
}
fn restore(current: &str, receiver: &str) -> String {
    let binding: serde_json::Value = serde_json::from_str(include_str!("source_inverse.json")).unwrap();
    assert_eq!(hash(current), binding["root_current_sha256"].as_str().unwrap());
    assert_eq!(hash(receiver), binding["receiver_current_sha256"].as_str().unwrap());
    let mut body = receiver.as_bytes().to_vec();
    for edit in binding["edits"].as_array().unwrap().iter().rev() {
        let start = usize::try_from(edit["start"].as_u64().unwrap()).unwrap();
        let end = usize::try_from(edit["end"].as_u64().unwrap()).unwrap();
        assert_eq!(body.get(start..end).unwrap(), edit["current"].as_str().unwrap().as_bytes());
        body.splice(start..end, edit["before"].as_str().unwrap().as_bytes().iter().copied());
    }
    let body = String::from_utf8(body).unwrap();
    assert_eq!(hash(&body), binding["receiver_before_sha256"].as_str().unwrap());
    let wrapper = binding["wrapper_current"].as_str().unwrap();
    assert_eq!(current.matches(wrapper).count(), 1);
    let mut restored = current.replacen(wrapper, &body, 1);
    for (current, previous) in [
        ("    #[cfg(test)]\n    fn validate_expr_persistent(", "    fn validate_expr_persistent("),
        ("pub(super) mod lifecycle;", "mod lifecycle;"),
    ] {
        assert_eq!(restored.matches(current).count(), 1);
        restored = restored.replacen(current, previous, 1);
    }
    assert_eq!(hash(&restored), binding["root_before_sha256"].as_str().unwrap());
    restored
}
pub(in crate::hir::validation) fn function_scope(current: &str) -> String {
    restore(current, include_str!("function.rs"))
}
#[test]
fn relocated_function_scope_requires_complete_root_and_receiver_before_unchanged_proof() {
    let current = include_str!("../function_scope.rs");
    let receiver = include_str!("function.rs");
    assert_eq!(hash(&restore(current, receiver)),
        "2cb5bf4ab361f172241d1c148788a5db09fdb264471818091ea1d83a1be41426");
    for (root, body) in [(current.to_owned()+" ", receiver.to_owned()),
        (current.to_owned(), receiver.to_owned()+" ")] {
        assert!(std::panic::catch_unwind(|| restore(&root, &body)).is_err());
    }
}
