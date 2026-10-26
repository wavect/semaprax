//! Exact malformed scalar fixtures affected by the admitted operator refusal.
//! Every other source retains the complete recursive result-parity check.
use super::*;

type ExpectedDiagnostic = (
    &'static str,
    &'static str,
    usize,
    usize,
    Option<&'static str>,
);

struct Case {
    source: &'static str,
    original: Type,
    diagnostics: &'static [ExpectedDiagnostic],
}

const CASES: &[Case] = &[
    Case {
        source: "module t; fn main(flag: bool) -> i64 { let text = \"x\"; let result = if flag { let joined = string_concat(text, \"y\"); 1 } else { 0 }; string_len(text) + result }",
        original: Type::I64,
        diagnostics: &[
            ("SPX-S109", "`result` is reserved and cannot name a local binding", 59, 65, Some("`result` names the function's return value only in `ensures`; rename a local binding to `outcome` and update its uses")),
            ("SPX-O107", "resource `text` may have been moved on another control-flow path", 144, 148, Some("owning calls consume `string`; if an application helper only reads the text, change its parameter to `borrow str` and pass `string_as_str(text)` before the first move; otherwise, provide a distinct owned string at each consuming call")),
            ("SPX-T201", "`result` is only available in postconditions", 152, 158, Some("`result` names the function's return value only in `ensures`; rename a local binding to `outcome` and update its uses")),
        ],
    },
    Case {
        source: "module t; fn main() -> i64 { -(1 + true) }",
        original: Type::I64,
        diagnostics: &[
            ("SPX-T208", "operator `+` expects i64 operands", 30, 40, None),
        ],
    },
    Case {
        source: "module t; fn main() -> bool { missing_left == missing_right }",
        original: Type::Bool,
        diagnostics: &[
            ("SPX-T202", "unknown value `missing_left` in `main`", 30, 42, None),
            ("SPX-T202", "unknown value `missing_right` in `main`", 46, 59, None),
        ],
    },
    Case {
        source: "module t; fn main(flag: bool) -> bool { flag && missing }",
        original: Type::Bool,
        diagnostics: &[
            ("SPX-T202", "unknown value `missing` in `main`", 48, 55, None),
        ],
    },
    Case {
        source: "module t; fn main(flag: bool) -> i64 { if flag { 1 } else { true } }",
        original: Type::I64,
        diagnostics: &[
            ("SPX-T211", "`if` branches return different types: i64 and bool", 39, 66, None),
        ],
    },
    Case {
        source: "module t; fn identity<T>(value: T) -> T { value } fn main() -> i64 { identity<i64>(1) + identity<bool>(true) }",
        original: Type::I64,
        diagnostics: &[
            ("SPX-T208", "operator `+` expects i64 operands", 69, 108, None),
        ],
    },
    Case {
        source: "module t; @id(\"t.shape\") variant Shape { @id(\"t.shape.dot\") Dot, @id(\"t.shape.empty\") Empty, @id(\"t.shape.square\") Square { @id(\"t.shape.square.side\") side: i64, }, } fn main(left: Shape, right: Shape) -> bool { left == right }",
        original: Type::Bool,
        diagnostics: &[
            ("SPX-T207", "equality on variant `Shape` requires a non-generic variant whose cases all carry no payload", 212, 225, Some("test the case with `match` instead, e.g. `match value { Shape::Dot {} => true, _ => false, }`")),
        ],
    },
    Case {
        source: "module t; @id(\"t.shape\") variant Shape { @id(\"t.shape.dot\") Dot, @id(\"t.shape.empty\") Empty, @id(\"t.shape.square\") Square { @id(\"t.shape.square.side\") side: i64, }, } fn main(left: Shape) -> bool { left != Shape::Dot {} }",
        original: Type::Bool,
        diagnostics: &[
            ("SPX-T207", "equality on variant `Shape` requires a non-generic variant whose cases all carry no payload", 198, 219, Some("test the case with `match` instead, e.g. `match value { Shape::Dot {} => true, _ => false, }`")),
        ],
    },
    Case {
        source: "module t; @id(\"t.status\") variant Status { @id(\"t.status.todo\") Todo, @id(\"t.status.done\") Done, } fn main(left: Status) -> bool { left == 1 }",
        original: Type::Bool,
        diagnostics: &[
            ("SPX-T207", "equality operands must have the same type", 131, 140, Some("comparing `Status` with `i64`; there are no numeric conversions, so compare two values of one type")),
        ],
    },
 ];

pub(super) fn expected(source: &str) -> Option<(Type, Vec<Diagnostic>)> {
    let case = CASES.iter().find(|case| case.source == source)?;
    let diagnostics = case
        .diagnostics
        .iter()
        .map(|&(code, message, start, end, help)| {
            let diagnostic = Diagnostic::error(
                code,
                message,
                crate::ast::Span {
                    start,
                    end,
                    line: 1,
                    column: start + 1,
                },
            )
            .at_path("iterative-verifier.spx");
            match help {
                Some(help) => diagnostic.with_help(help),
                None => diagnostic,
            }
        })
        .collect();
    Some((case.original.clone(), diagnostics))
}
