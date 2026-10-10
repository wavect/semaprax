//! Checked Unicode scalars share the integer conversion status and cleanup path.
use super::{checked_program, parity_cases, parity_cases_with_internal_strings};
use semaprax::{graph, hir};

const SOURCE: &str = r#"module test.unicode_scalar;
@id("scalar.convert") fn convert(value: i64) -> char { char_from_i64(value) }
@id("scalar.valid") fn valid() -> i64 {
    let valid = convert(0) == '\0' && convert(127) == '\u{7f}' && convert(128) == '\u{80}' && convert(255) == '\u{ff}' && convert(256) == '\u{100}' && convert(2047) == '\u{7ff}' && convert(2048) == '\u{800}' && convert(55295) == '\u{d7ff}' && convert(57344) == '\u{e000}' && convert(65535) == '\u{ffff}' && convert(65536) == '\u{10000}' && convert(1114111) == '\u{10ffff}';
    let mut value = 0;
    let mut same = true;
    while value <= 255 {
        same = same && char_from_i64(value) == char_from_u8(u8_from_i64(value));
        value = value + 1;
        0
    }
    if valid && same { 0 } else { 1 }
}
@id("scalar.negative") fn negative() -> i64 { let invalid = convert(-1); 1 }
@id("scalar.surrogate_start") fn surrogate_start() -> i64 { let invalid = convert(55296); 1 }
@id("scalar.surrogate_end") fn surrogate_end() -> i64 { let invalid = convert(57343); 1 }
@id("scalar.too_large") fn too_large() -> i64 { let invalid = convert(1114112); 1 }
@id("scalar.wide") fn wide() -> i64 { let invalid = convert(4294967361); 1 }
@id("scalar.maximum") fn maximum() -> i64 { let invalid = convert(9223372036854775807); 1 }
@id("scalar.minimum") fn minimum() -> i64 { let invalid = convert(-9223372036854775808); 1 }
@id("scalar.lazy") fn lazy() -> i64 {
    let left = false && convert(55296) == 'a';
    let right = true || convert(1114112) == 'a';
    if left { 1 } else { if right { if true { 0 } else { let invalid = convert(-1); 1 } } else { 1 } }
}
@id("scalar.pair") fn pair(first: char, second: i64) -> i64 { second }
@id("scalar.reverse") fn reverse(first: i64, second: char) -> i64 { first }
@id("scalar.convert_first") fn convert_first() -> i64 { pair(convert(55296), 1 / 0) }
@id("scalar.operand_first") fn operand_first() -> i64 { let invalid = convert(1 / 0); 1 }
@id("scalar.arithmetic_first") fn arithmetic_first() -> i64 { reverse(1 / 0, convert(55296)) }
@id("app.main") fn main() -> i64 { valid() }
"#;

const CASES: &[(&str, &str)] = &[
    ("scalar.valid", "ok|0"),
    ("scalar.negative", "semaprax.convert.v1|1"),
    ("scalar.surrogate_start", "semaprax.convert.v1|1"),
    ("scalar.surrogate_end", "semaprax.convert.v1|1"),
    ("scalar.too_large", "semaprax.convert.v1|1"),
    ("scalar.wide", "semaprax.convert.v1|1"),
    ("scalar.maximum", "semaprax.convert.v1|1"),
    ("scalar.minimum", "semaprax.convert.v1|1"),
    ("scalar.lazy", "ok|0"),
    ("scalar.convert_first", "semaprax.convert.v1|1"),
    ("scalar.operand_first", "semaprax.arithmetic.v1|4"),
    ("scalar.arithmetic_first", "semaprax.arithmetic.v1|4"),
];

#[test]
fn unicode_scalar_boundaries_and_order_agree_on_all_backends() {
    let ast = checked_program(SOURCE);
    assert!(graph::to_json(&ast)
        .unwrap()
        .contains("\"callee\":\"core.num.char_from_i64\""));
    parity_cases(SOURCE, CASES, true);
    let aggregate = SOURCE.replace(
        "module test.unicode_scalar;",
        "module test.unicode_scalar;\n@id(\"force.record\") record Force { @id(\"force.field\") value: i64, }",
    );
    parity_cases(&aggregate, CASES, false);
}

const OWNED: &str = r#"module test.unicode_owned;
@id("scalar.width") fn width(value: i64) -> i64 {
    let text = string_from_char(char_from_i64(value));
    string_len(text)
}
@id("scalar.utf8") fn utf8() -> i64 {
    if width(0) == 1 && width(127) == 1 && width(128) == 2 && width(255) == 2 && width(256) == 2 && width(2047) == 2 && width(2048) == 3 && width(55295) == 3 && width(57344) == 3 && width(65535) == 3 && width(65536) == 4 && width(1114111) == 4 { 0 } else { 1 }
}
@id("scalar.live") fn live() -> i64 {
    let live = bytes_zeroed(4usize);
    let invalid = char_from_i64(55296);
    if invalid == 'a' { 1 } else { i64_from_usize(byte_len(bytes_as_slice(live))) }
}
@id("scalar.staged") fn staged(text: string, value: char) -> i64 { string_len(text) }
@id("scalar.staged_failure") fn staged_failure() -> i64 {
    let text = "staged";
    staged(text, char_from_i64(1114112))
}
@id("app.main") fn main() -> i64 { utf8() }
"#;

#[test]
fn unicode_scalar_failure_settles_live_and_staged_owners() {
    let program = hir::resolve(&checked_program(OWNED)).unwrap();
    for id in ["scalar.live", "scalar.staged_failure"] {
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == id)
            .unwrap();
        let hir::ResolvedExprKind::Block { statements, tail } = &function.body.kind else {
            panic!("owning conversion witness remains a block")
        };
        let conversion = if id == "scalar.live" {
            let hir::ResolvedStatement::Let { value, .. } = &statements[1] else {
                panic!("conversion result binding")
            };
            value
        } else {
            let hir::ResolvedExprKind::Call { args, .. } = &tail.kind else {
                panic!("owning sibling call")
            };
            &args[1]
        };
        assert!(
            matches!(&conversion.kind, hir::ResolvedExprKind::Call { callee, .. }
            if callee.as_str() == "core.num.char_from_i64")
        );
        assert!(
            function.cleanup_plan.exits.iter().any(|exit| {
                matches!(
                    &exit.continuation,
                    semaprax::cleanup_plan::ExitContinuation::ReturnFailure { source }
                        if source.expression == conversion.id
                            && source.lane == semaprax::cleanup_plan::StatusLane::OperationFailure
                ) && exit.finalize_in_order.iter().any(|action| {
                    if id == "scalar.live" {
                        matches!(
                            action.source.storage,
                            semaprax::cleanup_plan::StorageId::Value(_)
                        )
                    } else {
                        matches!(
                            action.source.storage,
                            semaprax::cleanup_plan::StorageId::CallArgument { .. }
                        )
                    }
                })
            }),
            "{id} must retain canonical owner cleanup on conversion failure"
        );
    }
    // Direct String parameters require the explicit InternalStrings interpreter
    // profile, matching the other owning String signature witnesses.
    parity_cases_with_internal_strings(
        OWNED,
        &[
            ("scalar.utf8", "ok|0"),
            ("scalar.live", "semaprax.convert.v1|1"),
            ("scalar.staged_failure", "semaprax.convert.v1|1"),
        ],
        false,
        true,
    );
}
