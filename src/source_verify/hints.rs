//! Fix hints for the type-level habits a newcomer or coding agent brings from
//! other languages: a misspelled or foreign function name, a generic call or
//! generic variant without explicit type arguments, an unsuffixed integer
//! literal against a narrower operand, and an owned `string` handed to a byte
//! or host operation.
//!
//! Every helper here decorates a diagnostic the verifier already emits. Codes,
//! messages, and spans are unchanged; only `help` is added, and the iterative
//! verifier and the test-only recursive oracle call the same helpers so their
//! diagnostics stay byte-identical.

use std::collections::HashMap;

use super::diagnostics::error;
use crate::ast::{Expr, ExprKind, Function, Program, Span, Type};
use crate::diagnostic::Diagnostic;

/// Foreign output routines and the one admitted way to write bytes.
const PRINT_FAMILY: [&str; 8] = [
    "print",
    "println",
    "printf",
    "puts",
    "echo",
    "console_log",
    "log",
    "write",
];
const PRINT_HELP: &str = "there is no print routine; write bytes with `stdout_write(str_as_bytes(view))` \
                          under `permit { process.stdout.write }` and `uses { process.stdout.write }`, \
                          where `view` is `string_as_str(binding)` or a `borrow str` parameter";

/// `unknown function` with the nearest declared or compiler-owned name when one
/// is unambiguous and close, or the output hint for print-family names.
pub(super) fn unknown_function(
    program: &Program,
    name: &str,
    functions: &HashMap<&str, &Function>,
    span: Span,
) -> Diagnostic {
    let diagnostic = error(
        program,
        "SPX-T203",
        format!("unknown function `{name}`"),
        span,
    );
    if PRINT_FAMILY.contains(&name) {
        return diagnostic.with_help(PRINT_HELP);
    }
    match nearest_function_name(name, functions) {
        Some(candidate) => diagnostic.with_help(format!("did you mean `{candidate}`?")),
        None => diagnostic,
    }
}

fn nearest_function_name(name: &str, functions: &HashMap<&str, &Function>) -> Option<String> {
    let threshold = 1 + name.len() / 5;
    let mut candidates = functions
        .keys()
        .map(|key| (*key).to_owned())
        .collect::<Vec<_>>();
    candidates.extend(
        crate::string_ops::StringOp::ALL
            .iter()
            .map(|op| op.name().to_owned()),
    );
    candidates.extend(
        crate::byte_ops::ByteOp::ALL
            .iter()
            .map(|op| op.name().to_owned()),
    );
    candidates.extend(
        [
            crate::str_ops::LEN_BYTES_NAME,
            crate::str_ops::IS_EMPTY_NAME,
            crate::str_ops::STARTS_WITH_NAME,
            crate::str_ops::CONTAINS_NAME,
            crate::host_io_ops::STDOUT_WRITE_NAME,
            crate::command_io_ops::ARGS_LEN_NAME,
            crate::command_io_ops::ARG_UTF8_NAME,
            crate::command_io_ops::STDIN_READ_NAME,
            crate::command_io_ops::STDERR_WRITE_NAME,
        ]
        .iter()
        .map(|candidate| (*candidate).to_owned()),
    );
    candidates.sort();
    candidates.dedup();
    let mut nearest = None;
    let mut nearest_distance = usize::MAX;
    let mut ambiguous = false;
    for candidate in candidates {
        if candidate.len() > 64 || candidate == name {
            continue;
        }
        let distance = edit_distance(name.as_bytes(), candidate.as_bytes());
        if distance < nearest_distance {
            nearest = Some(candidate);
            nearest_distance = distance;
            ambiguous = false;
        } else if distance == nearest_distance {
            ambiguous = true;
        }
    }
    (nearest_distance <= threshold && !ambiguous)
        .then_some(nearest)
        .flatten()
}

/// Levenshtein distance over bytes, bounded to 64-byte operands by the caller.
fn edit_distance(left: &[u8], right: &[u8]) -> usize {
    let mut previous = [0usize; 65];
    let mut current = [0usize; 65];
    for (index, slot) in previous.iter_mut().take(right.len() + 1).enumerate() {
        *slot = index;
    }
    for (left_index, left_byte) in left.iter().enumerate() {
        current[0] = left_index + 1;
        for (right_index, right_byte) in right.iter().enumerate() {
            current[right_index + 1] = (previous[right_index + 1] + 1)
                .min(current[right_index] + 1)
                .min(previous[right_index] + usize::from(left_byte != right_byte));
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right.len()]
}

/// A generic function called without its explicit type arguments.
pub(super) fn generic_call_help(name: &str) -> String {
    format!(
        "call it as `{name}<i64>(…)`: generic calls spell a direct `i64` or `bool` type argument"
    )
}

/// A generic type named without its type arguments, in a signature or a
/// constructor.
pub(super) fn type_arguments_help(name: &str, expected: usize) -> String {
    match (name, expected) {
        ("Option", 1) => "spell the type argument at every use: `Option<i64>` in a signature and \
                          `Option<i64>::Some { value: … }` when constructing; patterns stay \
                          `Option::Some { value: v }`"
            .to_owned(),
        ("Result", 2) => "spell both type arguments at every use: `Result<i64, bool>` in a signature \
                          and `Result<i64, bool>::Ok { value: … }` when constructing; patterns stay \
                          `Result::Ok { value: v }`"
            .to_owned(),
        _ => format!("spell the type arguments at every use, including constructors: `{name}<…>`"),
    }
}

/// An unsuffixed integer literal meets a narrower integer operand.
pub(super) fn literal_suffix_help(expected: &Type, left: &Expr, right: &Expr) -> Option<String> {
    let suffix = match expected {
        Type::I32 => "i32",
        Type::U8 => "u8",
        Type::Usize => "usize",
        _ => return None,
    };
    let literal = [left, right]
        .into_iter()
        .find_map(|operand| match operand.kind {
            ExprKind::Int(value) => Some(value),
            _ => None,
        })?;
    Some(format!(
        "integer literals are `i64` unless suffixed; write `{literal}{suffix}` to match the `{expected}` operand"
    ))
}

/// An owned or mismatched value reaches a byte or host operation that takes a
/// borrowed view.
pub(super) fn view_argument_help(operation: &str, actual: &Type) -> Option<String> {
    let help = match (operation, actual) {
        ("str_as_bytes", Type::String) => {
            "`str_as_bytes` takes a `str` view; borrow the owned string first: \
             `str_as_bytes(string_as_str(binding))`"
        }
        ("string_as_str", Type::Str) => {
            "`string_as_str` takes an owned `string` binding; this value is already a `str` view"
        }
        ("stdout_write" | "stderr_write", Type::String | Type::Str) => {
            "output takes `borrow Slice<u8>`; write `stdout_write(str_as_bytes(view))` where `view` \
             is `string_as_str(binding)` or a `borrow str` parameter"
        }
        (_, Type::String | Type::Str | Type::Bytes | Type::ArrayU8(_)) => {
            "byte operations take `borrow Slice<u8>`; produce one with `str_as_bytes(view)`, \
             `array_as_slice(array)`, or `bytes_as_slice(bytes)`"
        }
        _ => return None,
    };
    Some(help.to_owned())
}

/// Attach `help` when a hint applies.
pub(super) fn with_optional_help(diagnostic: Diagnostic, help: Option<String>) -> Diagnostic {
    match help {
        Some(help) => diagnostic.with_help(help),
        None => diagnostic,
    }
}
