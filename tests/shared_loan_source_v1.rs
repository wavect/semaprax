use std::path::Path;

use semaprax::{parse, verify};

fn diagnostics(source: &str) -> Vec<semaprax::diagnostic::Diagnostic> {
    let program = parse(source, Path::new("shared-loan-source-v1.spx")).unwrap();
    verify::verify(&program)
}

const HEADER: &str = r#"
module test.shared_loan_source_v1;
@id("bytes.take") fn take(value: own Bytes) -> i64 { 1 }
"#;

#[test]
fn owner_can_move_after_the_last_use_of_a_same_block_view() {
    let source = format!(
        r#"{HEADER}
@id("loan.after-last-use")
fn after_last_use(input: borrow Slice<u8>) -> i64 {{
    let owned = bytes_copy(input);
    let view = bytes_as_slice(owned);
    let observed = byte_len(view);
    take(owned)
}}
@id("app.main") fn main() -> i64 {{ 0 }}
"#
    );
    assert!(diagnostics(&source).is_empty());
}

#[test]
fn a_view_used_after_the_transfer_keeps_its_exact_loan_live() {
    let source = format!(
        r#"{HEADER}
@id("loan.live-after-transfer")
fn live_after_transfer(input: borrow Slice<u8>) -> usize {{
    let owned = bytes_copy(input);
    let view = bytes_as_slice(owned);
    let moved = take(owned);
    byte_len(view)
}}
@id("app.main") fn main() -> i64 {{ 0 }}
"#
    );
    assert!(diagnostics(&source)
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-T265"));
}

#[test]
fn sibling_loans_release_independently_at_their_last_uses() {
    let accepted = format!(
        r#"{HEADER}
@id("loan.siblings-settle")
fn siblings_settle(input: borrow Slice<u8>) -> i64 {{
    let owned = bytes_copy(input);
    let left = bytes_as_slice(owned);
    let right = bytes_as_slice(owned);
    let left_len = byte_len(left);
    let right_len = byte_len(right);
    take(owned)
}}
@id("app.main") fn main() -> i64 {{ 0 }}
"#
    );
    assert!(diagnostics(&accepted).is_empty());

    let rejected = format!(
        r#"{HEADER}
@id("loan.sibling-remains")
fn sibling_remains(input: borrow Slice<u8>) -> usize {{
    let owned = bytes_copy(input);
    let left = bytes_as_slice(owned);
    let right = bytes_as_slice(owned);
    let left_len = byte_len(left);
    let moved = take(owned);
    byte_len(right)
}}
@id("app.main") fn main() -> i64 {{ 0 }}
"#
    );
    assert!(diagnostics(&rejected)
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-T265"));
}

#[test]
fn child_alias_settlement_does_not_release_a_still_used_parent() {
    let source = format!(
        r#"{HEADER}
@id("loan.parent-child")
fn parent_child(input: borrow Slice<u8>) -> usize {{
    let owned = bytes_copy(input);
    let parent = bytes_as_slice(owned);
    let child = parent;
    let child_len = byte_len(child);
    let moved = take(owned);
    byte_len(parent)
}}
@id("app.main") fn main() -> i64 {{ 0 }}
"#
    );
    assert!(diagnostics(&source)
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-T265"));
}

#[test]
fn child_alias_keeps_the_ultimate_owner_live_after_the_parents_last_use() {
    let source = format!(
        r#"{HEADER}
@id("loan.child-outlives-parent-use")
fn child_outlives_parent_use(input: borrow Slice<u8>) -> usize {{
    let owned = bytes_copy(input);
    let parent = bytes_as_slice(owned);
    let child = parent;
    let parent_len = byte_len(parent);
    let moved = take(owned);
    byte_len(child)
}}
@id("app.main") fn main() -> i64 {{ 0 }}
"#
    );
    assert!(diagnostics(&source)
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-T265"));
}

#[test]
fn structured_if_last_use_releases_before_the_following_transfer() {
    let source = format!(
        r#"{HEADER}
@id("loan.if-last-use")
fn if_last_use(input: borrow Slice<u8>, inspect: bool) -> i64 {{
    let owned = bytes_copy(input);
    let view = bytes_as_slice(owned);
    let observed = if inspect {{ byte_len(view) }} else {{ 0usize }};
    take(owned)
}}
@id("app.main") fn main() -> i64 {{ 0 }}
"#
    );
    assert!(diagnostics(&source).is_empty());
}

#[test]
fn lazy_expression_last_use_releases_before_the_following_transfer() {
    let source = format!(
        r#"{HEADER}
@id("loan.lazy-last-use")
fn lazy_last_use(input: borrow Slice<u8>, inspect: bool) -> i64 {{
    let owned = bytes_copy(input);
    let view = bytes_as_slice(owned);
    let observed = inspect && byte_len(view) > 0usize;
    take(owned)
}}
@id("app.main") fn main() -> i64 {{ 0 }}
"#
    );
    assert!(diagnostics(&source).is_empty());
}

#[test]
fn nested_match_borrow_cannot_release_the_outer_match_loan() {
    let source = r#"
module test.shared_loan_nested_match;
record Packet { payload: Bytes, }
@id("packet.take") fn take(packet: own Packet) -> i64 { 1 }
@id("packet.invalid") fn invalid(packet: own Packet) -> i64 {
    match borrow packet {
        Packet { payload } => match borrow packet {
            Packet { payload: inner } => take(packet),
        },
    }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(diagnostics(source)
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-T265"));
}

#[test]
fn projected_owned_byte_view_remains_closed_before_loan_inference() {
    let source = r#"
module test.shared_loan_projected_view;
record Packet { payload: Bytes, }
@id("packet.invalid") fn invalid(packet: own Packet) -> i64 {
    let projected = bytes_as_slice(packet.payload);
    0
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(diagnostics(source)
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-T266"));
}
