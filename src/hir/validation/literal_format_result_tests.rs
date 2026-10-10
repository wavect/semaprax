#[cfg(test)]
#[test]
fn literal_format_fresh_result_can_transfer_after_its_arguments_commit() {
    let source = r#"
module test.format_result_transfer;
@id("format.take") fn take(value: string) -> string { value }
@id("format.main") fn main() -> i64 {
    let left = "a";
    let right = "b";
    let value = take(string_format("{}{}", left, right));
    string_len(value)
}
"#;
    let parsed = crate::check(source, "format-result-transfer.spx").unwrap();
    let program = crate::hir::resolve(&parsed).unwrap();
    crate::hir::validate(&program).unwrap();
    let invalid = source.replace("string_len(value)", "string_len(left)");
    assert!(crate::check(&invalid, "format-result-reuse.spx")
        .unwrap_err()
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-O101"));
}
