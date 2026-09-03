use std::path::Path;

use semaprax::{format, parse};

const SOURCE: &str = r#"
module test.formatter_separators;

@id("test.empty")
fn empty(x: i64) -> i64 { match x {} }

@id("test.one")
fn one(x: i64) -> i64 { match x { _ => 1, } }

@id("test.many")
fn many(x: i64) -> i64 { match x { _ => 1, _ => 2, } }

@id("test.lets")
fn lets() -> i64 {
    let nested = { let first = 1; let second = 2; first + second };
    nested
}
"#;

const CANONICAL: &str = r#"module test.formatter_separators;

@id("test.empty")
fn empty(x: i64) -> i64
{
    match x {  }
}

@id("test.one")
fn one(x: i64) -> i64
{
    match x { _ => 1, }
}

@id("test.many")
fn many(x: i64) -> i64
{
    match x { _ => 1, _ => 2, }
}

@id("test.lets")
fn lets() -> i64
{
    let nested = { let first = 1; let second = 2; first + second };
    nested
}
"#;

#[test]
fn iterative_formatter_preserves_exact_match_and_block_separator_bytes() {
    let program = parse(SOURCE, Path::new("formatter-separators.spx")).unwrap();
    let canonical = format::canonical(&program);
    assert_eq!(canonical, CANONICAL);
    assert!(canonical.contains("match x {  }"));
    assert!(canonical.contains("match x { _ => 1, }"));
    assert!(canonical.contains("match x { _ => 1, _ => 2, }"));
    assert!(canonical.contains("{ let first = 1; let second = 2; first + second }"));

    let reparsed = parse(&canonical, Path::new("formatter-separators-canonical.spx")).unwrap();
    assert_eq!(format::canonical(&reparsed), CANONICAL);
}
