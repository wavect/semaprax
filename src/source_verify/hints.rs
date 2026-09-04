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
use super::type_table::TypeTable;
use crate::ast::{Expr, ExprKind, Function, Program, Span, Type, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;

/// A single file of a multi-file project checked on its own. Both diagnostics
/// are correct and both leave an agent editing one project module without a
/// next step, so each names the project-level command.
pub(super) const PROJECT_IMPORTS_HELP: &str = "this module imports other modules, so check it \
                                              through the project that owns it: `semaprax check \
                                              <project-dir>` or its `semaprax.toml`";
pub(super) const LIBRARY_MODULE_HELP: &str = "a module without `fn main() -> i64` is a library \
                                             module: check it through the project that owns it with \
                                             `semaprax check <project-dir>`, or add `main` to run \
                                             this file alone";

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
    if let Some(help) = variant_shorthand_help(name) {
        return diagnostic.with_help(help);
    }
    match nearest_function_name(name, functions) {
        Some(candidate) => diagnostic.with_help(format!("did you mean `{candidate}`?")),
        None => diagnostic.with_help(format!(
            "declare `{name}` in this module, or in a project import it directly after the \
             `module` line: `use function @id(\"stable.id\") from other.module as {name};`"
        )),
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

/// An owned or mismatched value reaches a user function parameter declared
/// as a borrowed view.
pub(super) fn argument_view_help(name: &str, expected: &Type, actual: &Type) -> Option<String> {
    let help = match (expected, actual) {
        (Type::Str, Type::String) => format!(
            "`{name}` takes a `str` view; borrow the owned string first: \
             `{name}(string_as_str(binding))`, binding a literal with `let` before that"
        ),
        (Type::SliceU8, Type::String | Type::Str | Type::Bytes | Type::ArrayU8(_)) => format!(
            "`{name}` takes `borrow Slice<u8>`; produce one with `str_as_bytes(view)`, \
             `array_as_slice(array)`, or `bytes_as_slice(bytes)`"
        ),
        _ => return None,
    };
    Some(help)
}

/// `Some(1)`, `None`, `Ok(x)`, or `Err(e)` used as a bare constructor or value.
pub(super) fn variant_shorthand_help(name: &str) -> Option<&'static str> {
    match name {
        "Some" | "None" => Some(
            "variant constructors spell the type and fields: `Option<i64>::Some { value: 1 }` and \
             `Option<i64>::None {}`",
        ),
        "Ok" | "Err" => Some(
            "variant constructors spell the type and fields: `Result<i64, bool>::Ok { value: 1 }` \
             and `Result<i64, bool>::Err { error: false }`",
        ),
        "null" | "nil" | "undefined" | "nullptr" => Some(
            "there is no null; an absent value is `Option<T>::None {}` and a present one \
             `Option<T>::Some { value: … }`",
        ),
        _ => None,
    }
}

/// A method call on a value whose type has no methods.
pub(super) fn method_receiver_help(receiver: &Type, method: &str) -> Option<String> {
    let (family, replacement) = match receiver {
        Type::String => (
            "strings",
            match method {
                "len" | "length" | "size" | "byte_len" => Some("string_len(s)"),
                "chars" | "char_count" | "len_chars" | "count" => Some("string_len_chars(s)"),
                "is_empty" | "empty" => Some("string_is_empty(s)"),
                "contains" | "includes" | "find" | "index_of" => Some("string_contains(s, needle)"),
                "starts_with" | "has_prefix" => Some("string_starts_with(s, prefix)"),
                "concat" | "push_str" | "append" | "add" | "join" => Some("string_concat(a, b)"),
                "as_str" | "borrow" | "view" => Some("string_as_str(binding)"),
                "as_bytes" | "bytes" | "to_bytes" => Some("str_as_bytes(string_as_str(binding))"),
                _ => None,
            },
        ),
        Type::Str => (
            "borrowed `str` views",
            match method {
                "len" | "length" | "size" | "len_bytes" => Some("str_len_bytes(s)"),
                "is_empty" | "empty" => Some("str_is_empty(s)"),
                "contains" | "includes" | "find" => Some("str_contains(s, needle)"),
                "starts_with" | "has_prefix" => Some("str_starts_with(s, prefix)"),
                "as_bytes" | "bytes" => Some("str_as_bytes(s)"),
                _ => None,
            },
        ),
        Type::SliceU8 | Type::Bytes | Type::ArrayU8(_) => (
            "byte values",
            match method {
                "len" | "length" | "size" | "count" => Some("byte_len(view)"),
                "get" | "at" | "index" | "nth" => Some("byte_get(view, index)"),
                "slice" | "range" | "sub" | "window" => Some("byte_range(view, start, end)"),
                "copy" | "clone" | "to_vec" | "to_owned" => Some("bytes_copy(view)"),
                "as_slice" | "view" | "borrow" => Some("bytes_as_slice(bytes)"),
                _ => None,
            },
        ),
        Type::Named { .. } => {
            return Some(
                "records and variants have no methods; call a function with the value as an \
                 argument, or declare a `class` when methods are needed"
                    .to_owned(),
            );
        }
        _ => return None,
    };
    Some(match replacement {
        Some(replacement) => format!(
            "{family} have no methods; write `{replacement}` with the compiler-owned function"
        ),
        None => format!("{family} have no methods; call a compiler-owned function with the value as its argument"),
    })
}

/// A method looked up on a declared type that is not a class.
pub(super) fn non_class_method_help(types: &TypeTable<'_>, name: &str) -> Option<String> {
    let declaration = types.declaration(name)?;
    let noun = match declaration.kind {
        TypeDeclarationKind::Record { .. } => "records",
        TypeDeclarationKind::Variant { .. } => "variants",
        TypeDeclarationKind::Resource { .. } => "resources",
        _ => return None,
    };
    Some(format!(
        "{noun} have no methods; call a function with the value as an argument, or declare a \
         `class` when methods are needed"
    ))
}

/// A type name from another language.
pub(super) fn unknown_type_help(name: &str) -> Option<&'static str> {
    match name {
        "String" | "str" | "Str" | "text" | "Text" => Some(
            "owned text is `string`; a borrowed view is `borrow str` in a parameter position",
        ),
        "int" | "Int" | "i8" | "i16" | "u16" | "u32" | "u64" | "i128" | "u128" | "isize"
        | "long" | "short" | "byte" | "integer" | "Integer" | "number" | "Number" => Some(
            "the integer types are `i64` (the literal default), `i32`, `u8`, and `usize`; there is no other width",
        ),
        "float" | "Float" | "double" | "Double" | "f16" | "decimal" => {
            Some("the floating-point types are `f64` and `f32`")
        }
        "boolean" | "Boolean" | "Bool" => Some("the boolean type is spelled `bool`"),
        "Vec" | "vec" | "Array" | "array" | "List" | "list" | "Slice" | "slice" => Some(
            "sequences are fixed `[u8; N]` arrays, owned `Bytes`, and borrowed `Slice<u8>` views; there is no general collection type",
        ),
        "unit" | "void" | "Unit" | "Void" | "never" => {
            Some("there is no unit type; functions return `i64` or `bool`")
        }
        "char8" | "Char" | "character" | "rune" => Some("a Unicode scalar is `char`"),
        "Option" | "Result" => None,
        _ => None,
    }
}

/// A borrowed-view operation applied to something other than a plain binding.
pub(super) fn view_place_help(operation: &str, argument: &Expr) -> String {
    let (source, binding) = match (operation, &argument.kind) {
        ("str_as_bytes", ExprKind::String(_)) | ("string_as_str", ExprKind::String(_)) => (
            "a string literal",
            "`let text = \"…\"; str_as_bytes(string_as_str(text))`",
        ),
        ("array_as_slice", ExprKind::ArrayU8(_) | ExprKind::RepeatArrayU8 { .. }) => (
            "an array literal",
            "`let bytes = [1u8, 2u8]; array_as_slice(bytes)`",
        ),
        (_, ExprKind::Call { .. } | ExprKind::MethodCall { .. }) => {
            ("a call result", "`let owner = …; <view>(owner)`")
        }
        _ => ("this expression", "`let owner = …; <view>(owner)`"),
    };
    format!(
        "`{operation}` borrows from a named `let` binding, not from {source}; bind the owner first: {binding}"
    )
}

/// Attach `help` when a hint applies.
pub(super) fn with_optional_help(diagnostic: Diagnostic, help: Option<String>) -> Diagnostic {
    match help {
        Some(help) => diagnostic.with_help(help),
        None => diagnostic,
    }
}
