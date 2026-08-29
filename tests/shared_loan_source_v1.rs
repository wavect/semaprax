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
fn direct_owned_byte_field_view_is_admitted_and_keeps_siblings_independent() {
    let source = r#"
module test.shared_loan_projected_view;
@id("packet") record Packet {
    @id("packet.payload") payload: Bytes,
    @id("packet.sibling") sibling: Bytes,
}
@id("bytes.take") fn take(value: own Bytes) -> i64 { 1 }
@id("packet.accepted") fn accepted(packet: own Packet) -> usize {
    let projected = bytes_as_slice(packet.payload);
    let moved = take(packet.sibling);
    byte_len(projected)
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(diagnostics(source).is_empty());
}

#[test]
fn projected_view_blocks_its_field_and_parent_until_its_last_use() {
    let same_field = r#"
module test.shared_loan_projected_same_field;
@id("packet") record Packet { @id("packet.payload") payload: Bytes, }
@id("bytes.take") fn take(value: own Bytes) -> i64 { 1 }
@id("packet.invalid") fn invalid(packet: own Packet) -> usize {
    let projected = bytes_as_slice(packet.payload);
    let alias = projected;
    let range = byte_range(alias, 0usize, byte_len(alias));
    let moved = take(packet.payload);
    byte_len(range)
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(diagnostics(same_field)
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-T265"));

    let parent = r#"
module test.shared_loan_projected_parent;
@id("packet") record Packet { @id("packet.payload") payload: Bytes, }
@id("packet.take") fn take(value: own Packet) -> i64 { 1 }
@id("packet.invalid") fn invalid(packet: own Packet) -> usize {
    let projected = bytes_as_slice(packet.payload);
    let moved = take(packet);
    byte_len(projected)
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(diagnostics(parent)
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-T265"));
}

#[test]
fn projected_view_blocks_overlapping_assignment_update_and_match_own() {
    let assignment = r#"
module test.shared_loan_projected_assignment;
@id("packet") record Packet {
    @id("packet.left") left: Bytes,
    @id("packet.right") right: Bytes,
    @id("packet.marker") marker: i64,
}
@id("packet.invalid") fn invalid(input: borrow Slice<u8>) -> usize {
    let mut packet = Packet {
        left: bytes_copy(input),
        right: bytes_copy(input),
        marker: 0,
    };
    let view = bytes_as_slice(packet.left);
    packet.left = bytes_copy(input);
    byte_len(view)
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(diagnostics(assignment)
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-T265"));

    let sibling_assignment =
        assignment.replace("packet.left = bytes_copy(input);", "packet.marker = 1;");
    assert!(diagnostics(&sibling_assignment).is_empty());

    let update = r#"
module test.shared_loan_projected_update;
@id("packet") record Packet { @id("packet.left") left: Bytes, }
@id("packet.invalid") fn invalid(packet: own Packet, input: borrow Slice<u8>) -> usize {
    let view = bytes_as_slice(packet.left);
    let replacement = packet with { left: bytes_copy(input) };
    byte_len(view)
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(diagnostics(update)
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-T265"));

    let match_own = r#"
module test.shared_loan_projected_match;
@id("packet") record Packet {
    @id("packet.left") left: Bytes,
    @id("packet.marker") marker: i64,
}
@id("packet.invalid") fn invalid(packet: own Packet) -> usize {
    let view = bytes_as_slice(packet.left);
    let observed = match own packet { Packet { left: _, marker } => marker, };
    byte_len(view)
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(diagnostics(match_own)
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-T265"));
}

#[test]
fn projected_view_releases_before_a_later_parent_move() {
    let source = r#"
module test.shared_loan_projected_release;
@id("packet") record Packet { @id("packet.payload") payload: Bytes, }
@id("packet.take") fn take(value: own Packet) -> i64 { 1 }
@id("packet.accepted") fn accepted(packet: own Packet) -> i64 {
    let projected = bytes_as_slice(packet.payload);
    let observed = byte_len(projected);
    take(packet)
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(diagnostics(source).is_empty());
}

#[test]
fn projected_owned_byte_view_rejects_every_shape_outside_the_closed_profile() {
    let borrowed_root = r#"
module test.shared_loan_projected_borrowed_root;
@id("packet") record Packet { @id("packet.payload") payload: Bytes, }
@id("packet.invalid") fn invalid(packet: borrow Packet) -> usize {
    let projected = bytes_as_slice(packet.payload);
    byte_len(projected)
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(diagnostics(borrowed_root)
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-T266"));

    let temporary = r#"
module test.shared_loan_projected_temporary;
@id("packet") record Packet { @id("packet.payload") payload: Bytes, }
@id("packet.make") fn make(value: own Bytes) -> Packet { Packet { payload: value } }
@id("packet.invalid") fn invalid(value: own Bytes) -> usize {
    let projected = bytes_as_slice(make(value).payload);
    byte_len(projected)
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(diagnostics(temporary)
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-T266"));

    let deeper = r#"
module test.shared_loan_projected_deeper;
@id("inner") record Inner { @id("inner.payload") payload: Bytes, }
@id("outer") record Outer { @id("outer.inner") inner: Inner, }
@id("outer.invalid") fn invalid(outer: own Outer) -> usize {
    let projected = bytes_as_slice(outer.inner.payload);
    byte_len(projected)
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let report = diagnostics(deeper);
    assert!(report.iter().any(|diagnostic| {
        diagnostic.code == "SPX-T268" && diagnostic.message.contains("nests owned `Bytes`")
    }));
}

#[test]
fn legacy_borrowed_bytes_slice_and_string_calls_remain_admitted() {
    let source = r#"
module test.shared_loan_legacy_borrow_calls;
@id("borrow.slice") fn borrow_slice(value: borrow Slice<u8>) -> usize { byte_len(value) }
@id("borrow.string") fn borrow_string(value: borrow str) -> i64 { 1 }
@id("borrow.bytes") fn borrow_bytes(value: borrow Bytes) -> i64 { 1 }
@id("borrow.forward")
fn forward(slice: borrow Slice<u8>, text: borrow str) -> i64 {
    let length = borrow_slice(slice);
    let owned = bytes_copy(slice);
    borrow_bytes(owned) + borrow_string(text)
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(diagnostics(source).is_empty());
}
