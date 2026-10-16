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

use crate::ast::program_source::ProgramSource;
use std::collections::HashMap;
use std::sync::OnceLock;

use super::diagnostics::error;
use super::type_table::TypeTable;
use crate::ast::{
    Expr, ExprKind, Function, Span, Type, TypeDeclarationKind, VariantCaseDeclaration,
};
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
const PRINT_HELP: &str =
    "there is no print routine; write bytes with `let view = string_as_str(text); \
                          let written = stdout_write(str_as_bytes(view));` under `permit { \
                          process.stdout.write }` and `uses { process.stdout.write }`";

/// Compiler-bundled standard-library functions are discoverable without a
/// checkout through `semaprax help library <name>`. Keep this lookup bound to the
/// same generated catalog rather than duplicating its growing function list.
/// An unambiguous name yields its package, stable identity, and signature line.
fn standard_library_function(name: &str) -> Option<&'static StandardFunction> {
    static FUNCTIONS: OnceLock<HashMap<String, Option<StandardFunction>>> = OnceLock::new();
    let functions = FUNCTIONS.get_or_init(|| {
        let catalog: serde_json::Value =
            serde_json::from_str(include_str!("../../std/catalog.json"))
                .expect("the checked-in standard-library catalog is valid JSON");
        let mut functions = HashMap::new();
        for module in catalog["modules"]
            .as_array()
            .expect("the standard-library catalog has modules")
        {
            let provider = module["module"]
                .as_str()
                .expect("a standard-library module has an identity");
            let dependency = module["dependency"]
                .as_str()
                .and_then(|value| value.split_once(" = "))
                .map(|(name, _)| name)
                .expect("a standard-library module has a dependency route");
            for declaration in module["declarations"]
                .as_array()
                .expect("a standard-library module has declarations")
            {
                if declaration["kind"].as_str() != Some("function") {
                    continue;
                }
                let function = declaration["name"]
                    .as_str()
                    .expect("a standard-library function has a name")
                    .to_owned();
                let entry = StandardFunction {
                    dependency: dependency.to_owned(),
                    profile: module["required_profile"]
                        .as_str()
                        .expect("a standard-library module has a profile")
                        .to_owned(),
                    provider: provider.to_owned(),
                    id: declaration["id"].as_str().unwrap_or_default().to_owned(),
                    signature: declaration["head"][0]
                        .as_str()
                        .unwrap_or_default()
                        .to_owned(),
                };
                functions
                    .entry(function)
                    .and_modify(|selected| *selected = None)
                    .or_insert_with(|| Some(entry));
            }
        }
        functions
    });
    functions.get(name).and_then(Option::as_ref)
}

struct StandardFunction {
    dependency: String,
    profile: String,
    provider: String,
    id: String,
    signature: String,
}

/// Foreign spellings of an operation the language provides under another name.
fn foreign_function_help(name: &str) -> Option<&'static str> {
    match name {
        "to_string" | "toString" | "str" | "itoa" | "string" | "String" => Some(
            "render an integer with the compiler-owned `string_from_i64(value)` or \
             `string_from_usize(value)`",
        ),
        "assert" | "assert_eq" | "assertEqual" | "expect" | "panic" => Some(
            "there is no assert or panic; a test returns `0` on success, as in \
             `if <condition> { 0 } else { 1 }`, and a checked condition is a `requires` or \
             `ensures` line",
        ),
        _ => None,
    }
}

/// `unknown function` with the nearest declared or compiler-owned name when one
/// is unambiguous and close, or the output hint for print-family names.
pub(super) fn unknown_function(
    program: &dyn ProgramSource,
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
    if let Some(help) = variant_shorthand_help(name).or_else(|| foreign_function_help(name)) {
        return diagnostic.with_help(help);
    }
    // An exact standard-library name outranks a near local spelling: `min`
    // is `std.core`'s, not a typo of `main`.
    if let Some(function) = standard_library_function(name) {
        let StandardFunction {
            dependency,
            profile,
            provider,
            id,
            signature,
        } = function;
        return diagnostic.with_help(format!(
            "`{signature}` is in `{provider}`: add `[dependencies] {dependency} = \"^0.1.0\"` to \
             `semaprax.toml`, select the required `[package] profile = \"{profile}\"`, and import it directly after the `module` line: `use function \
             @id(\"{id}\") from {provider} as {name};`"
        ));
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
            crate::string_ops::I64_FROM_U8_NAME,
            crate::string_ops::U8_FROM_I64_NAME,
            crate::string_ops::CHAR_FROM_U8_NAME,
            crate::string_ops::CHAR_FROM_I64_NAME,
            crate::str_ops::LEN_BYTES_NAME,
            crate::str_ops::IS_EMPTY_NAME,
            crate::str_ops::STARTS_WITH_NAME,
            crate::str_ops::CONTAINS_NAME,
            crate::str_ops::BYTE_AT_NAME,
            crate::host_io_ops::STDOUT_WRITE_NAME,
            crate::command_io_ops::ARGS_LEN_NAME,
            crate::command_io_ops::ARG_UTF8_NAME,
            crate::command_io_ops::STDIN_READ_NAME,
            crate::command_io_ops::STDERR_WRITE_NAME,
        ]
        .iter()
        .map(|candidate| (*candidate).to_owned()),
    );
    // Enumerate the same compiler-owned operation identities used for admission.
    candidates.extend(crate::vec_ops::ALL.iter().map(|op| op.name().to_owned()));
    candidates.extend(crate::list_ops::ALL.iter().map(|op| op.name().to_owned()));
    candidates.extend(
        crate::map_ops::MapOp::ALL
            .iter()
            .map(|op| op.name().to_owned()),
    );
    candidates.extend(crate::box_ops::ALL.iter().map(|op| op.name().to_owned()));
    candidates.extend(
        crate::iterator_ops::ALL
            .iter()
            .map(|op| op.name().to_owned()),
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

pub(super) fn nearest_variant_case_name(
    name: &str,
    cases: &[VariantCaseDeclaration],
) -> Option<String> {
    let threshold = 1 + name.len() / 5;
    let mut nearest = None;
    let mut nearest_distance = usize::MAX;
    let mut ambiguous = false;
    for case in cases {
        if case.name.len() > 64 || case.name == name {
            continue;
        }
        let distance = edit_distance(name.as_bytes(), case.name.as_bytes());
        if distance < nearest_distance {
            nearest = Some(case.name.clone());
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
            "`str_as_bytes` takes a `str` view; bind one first: \
             `let view = string_as_str(binding); str_as_bytes(view)`"
        }
        ("string_as_str", Type::Str) => {
            "`string_as_str` takes an owned `string` binding; this value is already a `str` view"
        }
        ("stdout_write" | "stderr_write", Type::String | Type::Str) => {
            "output takes `borrow Slice<u8>`; write `let view = string_as_str(text); \
             stdout_write(str_as_bytes(view))`, or pass a `borrow str` parameter as `view`"
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
        (Type::SliceU8, Type::String) => format!(
            "`{name}` takes `borrow Slice<u8>`; bind a literal first, then both views: \
             `let bytes = str_as_bytes(string_as_str(text)); {name}(bytes)`"
        ),
        (Type::SliceU8, Type::Str | Type::Bytes | Type::ArrayU8(_)) => format!(
            "`{name}` takes `borrow Slice<u8>`; produce one with `str_as_bytes(view)`, \
             `array_as_slice(array)`, or `bytes_as_slice(bytes)`"
        ),
        (Type::String, Type::I64) => format!(
            "`{name}` takes a `string`; render the integer first with `string_from_i64(value)`"
        ),
        (Type::String, Type::Usize) => format!(
            "`{name}` takes a `string`; render the integer first with `string_from_usize(value)`"
        ),
        (Type::I32 | Type::U8 | Type::Usize, Type::I64) => format!(
            "integer literals are `i64` unless suffixed; pass a `{expected}` value such as \
             `1{expected}`, since there are no numeric conversions"
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
    if matches!(
        receiver,
        Type::I64
            | Type::I32
            | Type::U8
            | Type::Usize
            | Type::F64
            | Type::F32
            | Type::Bool
            | Type::Char
    ) {
        let call = match method {
            "to_string" | "toString" if *receiver == Type::Usize => "string_from_usize(x)",
            "to_string" | "toString" => "string_from_i64(x)",
            _ => return Some(format!(
                "scalars have no methods; call a function with the value as its argument: `{method}(x)`, \
                 importing it if it comes from the standard library"
            )),
        };
        return Some(format!("scalars have no methods; write `{call}`"));
    }
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
                "as_bytes" | "bytes" | "to_bytes" => {
                    Some("let view = string_as_str(s); str_as_bytes(view)")
                }
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
            "a list is `Vec<T>` with an explicit Copy scalar `T`: `vec_with_capacity<i64>(4usize)`, `vec_push<i64>(v, x)`, `vec_get<i64>(v, i)`, `vec_len<i64>(v)`; bytes are `[u8; N]`, `Bytes`, and `Slice<u8>`",
        ),
        "unit" | "void" | "Unit" | "Void" | "never" => {
            Some("there is no unit type; functions return `i64` or `bool`")
        }
        "char8" | "Char" | "character" | "rune" => Some("a Unicode scalar is `char`"),
        "Option" | "Result" => None,
        _ => None,
    }
}

/// `Option<usize>`: a generic Copy type instantiated with an integer that only
/// `i64` can carry. Names the two conversions that round-trip it.
pub(super) fn generic_scalar_argument_help(name: &str, arguments: &[Type]) -> Option<String> {
    let (scalar, widen, narrow) = arguments.iter().find_map(|argument| match argument {
        Type::Usize => Some(("usize", "i64_from_usize", "usize_from_i64")),
        Type::U8 => Some(("u8", "i64_from_u8", "u8_from_i64")),
        _ => None,
    })?;
    let range = if scalar == "usize" {
        "; this round trip requires a value at most 9223372036854775807 (i64::MAX), otherwise the conversion fails"
    } else {
        ""
    };
    Some(format!(
        "carry the `{scalar}` as `i64`: instantiate `{name}` with `i64`, store `{widen}(value)`, and read it back with `{narrow}(value)`{range}"
    ))
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
        ("str_as_bytes", ExprKind::Call { name, .. }) if name == "string_as_str" => (
            "a nested view",
            "`let text = …; str_as_bytes(string_as_str(text))`",
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

/// `"a" + "b"`: string concatenation spelled as an operator.
pub(super) const STRING_OPERATOR_HELP: &str =
    "join strings with `string_concat(a, b)`, which consumes both; strings compare with `==`, `!=`, `<`, `<=`, `>`, and `>=` (UTF-8 byte order)";
/// `result` is the postcondition value, not an admitted local name.
pub(super) const RESULT_NAME_HELP: &str = "`result` names the function's return value only in \
    `ensures`; rename a local binding to `outcome` and update its uses";

pub(super) fn reserved_local_binding(
    program: &dyn ProgramSource,
    name: &str,
    span: Span,
) -> Diagnostic {
    let diagnostic = error(
        program,
        "SPX-S109",
        format!("`{name}` is reserved and cannot name a local binding"),
        span,
    );
    if name == "result" {
        diagnostic.with_help(RESULT_NAME_HELP)
    } else {
        diagnostic
    }
}

/// `let x = …; let x = …;`: shadowing from Rust.
pub(super) const SHADOW_HELP: &str =
    "there is no shadowing; pick a new name, or declare the first \
                                      binding with `let mut` and assign to it";
/// `f()?` in a function that does not itself return `Result`.
pub(super) const TRY_RESULT_RETURN_HELP: &str = "`?` returns the error from the enclosing function, so \
                                                 that function must return `Result<…>`; otherwise match \
                                                 it: `match f() { Result::Ok { value: v } => v, \
                                                 Result::Err { error: e } => 1, }`";

/// The entry point has one admitted signature; name the part that differs.
pub(super) fn entry_signature_help(main: &Function) -> &'static str {
    if !main.params.is_empty() {
        "`main` takes no parameters; read command-line arguments with `args_len()` and `arg_utf8(index)`"
    } else if !main.type_parameters.is_empty() {
        "`main` cannot be generic; move the generic code into a helper and call it with explicit type arguments"
    } else {
        "`main` returns `i64`, the process exit status: `0` conventionally means success"
    }
}

/// `let a: i32 = 5;`: an unsuffixed literal bound to a narrower declared type,
/// or a declared type spelled the way another language spells it.
pub(super) fn declared_binding_help(declared: &Type, value: &Expr) -> Option<String> {
    let suffix = match declared {
        Type::I32 => Some("i32"),
        Type::U8 => Some("u8"),
        Type::Usize => Some("usize"),
        _ => None,
    };
    if let (Some(suffix), ExprKind::Int(literal)) = (suffix, &value.kind) {
        return Some(format!(
            "integer literals are `i64` unless suffixed; write `{literal}{suffix}`"
        ));
    }
    unknown_type_help(&declared.to_string()).map(str::to_owned)
}

/// `for item in values` over the `let mut` binding that built the vector.
pub(super) const IMMUTABLE_TRAVERSAL_HELP: &str = "move the finished vector into an immutable binding \
                                                   first: `let values = building;`, then `for item in \
                                                   values { … }`";

/// A function declares an effect its module does not permit.
pub(super) fn unpermitted_effect(
    program: &dyn ProgramSource,
    function: &Function,
    effect: &str,
) -> Diagnostic {
    error(
        program,
        "SPX-E101",
        format!(
            "function `{}` uses `{effect}` but module `{}` does not permit it",
            function.name,
            program.module()
        ),
        function.span,
    )
    .with_help(format!(
        "add `permit {{ {effect} }}` at module level, below the `module` line"
    ))
}

/// A call needs an effect its caller does not declare. Name both edits, so
/// fixing `uses` does not surface the module `permit` as a second round trip.
pub(super) fn missing_effect(
    program: &dyn ProgramSource,
    callee: &str,
    effect: &str,
    caller: &Function,
    span: Span,
) -> Diagnostic {
    let uses = format!("add `uses {{ {effect} }}` on its own line between the signature and `{{`");
    let help = if program.permits().iter().any(|permit| permit == effect) {
        uses
    } else {
        format!("{uses}, and `permit {{ {effect} }}` at module level, below the `module` line")
    };
    error(
        program,
        "SPX-E102",
        format!(
            "call to `{callee}` requires effect `{effect}`; add it to `{}`",
            caller.name
        ),
        span,
    )
    .with_help(help)
}

/// `a == b` across two types: name both, and the literal suffix when one side
/// is an unsuffixed literal.
pub(super) fn equality_types_help(
    left_type: Option<&Type>,
    right_type: Option<&Type>,
    left: &Expr,
    right: &Expr,
) -> Option<String> {
    let (left_type, right_type) = (left_type?, right_type?);
    let typed_side = if matches!(left.kind, ExprKind::Int(_)) {
        right_type
    } else {
        left_type
    };
    Some(match literal_suffix_help(typed_side, left, right) {
        Some(suffix) => format!("comparing `{left_type}` with `{right_type}`: {suffix}"),
        None => format!(
            "comparing `{left_type}` with `{right_type}`; there are no numeric conversions, so \
             compare two values of one type"
        ),
    })
}

/// A moved or conditionally moved resource read again. Strings and byte
/// buffers have no implicit copy, so the generic advice needs a concrete route.
pub(super) fn moved_resource_help(ty: &Type, name: &str) -> String {
    match ty {
        Type::String => format!(
            "owning calls consume `string`; if an application helper only reads the text, change its parameter to `borrow str` and pass `string_as_str({name})` before the first move; otherwise, provide a distinct owned string at each consuming call"
        ),
        Type::Bytes => {
            "`own` parameters consume `Bytes`; pass `bytes_as_slice(name)` to a `borrow \
             Slice<u8>` parameter instead, or copy first with `bytes_copy(bytes_as_slice(name))"
                .to_owned()
        }
        _ => "borrow the resource if the callee does not need ownership".to_owned(),
    }
}

/// Presentation-only guidance for exact compiler-authored refusal families.
/// These strings explain admitted syntax; they do not fabricate a fresh name,
/// a flow witness, source-edit authority, or a larger execution profile.
fn authoring_repair_help(code: &str, message: &str) -> Option<&'static str> {
    match code {
        "SPX-M104" if message.starts_with("invalid or duplicate pattern binding `") => Some(
            "use a valid alias unique in this scope; `result` is reserved. For a payload, keep the field name and write `field: fresh_alias`, then use that alias in the arm; `fresh_alias` is an example, so check it is unused",
        ),
        "SPX-T269" if message.starts_with("stdout_write path reaches ")
            || message.starts_with("stderr_write path reaches ") => Some(
            "construct bounded output first, then publish once per stream on each path with stdout_write or stderr_write; keep the selected profile's byte limits",
        ),
        "SPX-T269" if message == "transcript output is reachable from a while condition" => Some(
            "keep transcript publication out of the while condition; compute the condition without output and publish bounded output once after the loop",
        ),
        "SPX-T269" if message == "a while body may contain only direct runtime-bounded append output" => Some(
            "build output within the selected profile's bounds and publish once after the loop; direct runtime-bounded append is a separate admitted shape, and an indirect writer is not that shape",
        ),
        "SPX-T269" if message == "transcript-write executable closure is cyclic" => Some(
            "keep transcript writers out of cyclic call closures; construct output within the selected profile's bounds and publish once from an acyclic caller",
        ),
        _ => None,
    }
}

pub(super) fn with_authoring_repair_help(mut diagnostic: Diagnostic) -> Diagnostic {
    if diagnostic.help.is_some() {
        return diagnostic;
    }
    let Some(text) = authoring_repair_help(diagnostic.code, &diagnostic.message) else {
        return diagnostic;
    };
    // Optional presentation must never select a new refusal or consume the
    // authoritative diagnostic floor. Sticky refusal rejects even fitting help.
    // Admitted help consumes real bytes; later required-work boundaries can move.
    if !crate::bounded_output::reserve_active_preserving(text.len(), 0) {
        return diagnostic;
    }
    let mut help = String::new();
    if help.try_reserve_exact(text.len()).is_err() || help.capacity() != text.len() {
        return diagnostic;
    }
    help.push_str(text);
    diagnostic.help = Some(help);
    diagnostic
}

fn capacity_witness_receiving_bytes() -> usize {
    use std::mem::size_of;
    // Full actual prospective controls, admitted before their first formation.
    // The existing Diagnostic/witness arguments replace the old with_help
    // receiver/value; no extra Diagnostic owner or failure Result is introduced.
    2 * size_of::<String>() // String::new initializer and named composed owner
        + 19 * size_of::<usize>() // initializer/binding/argument site map in the source packet
        + 2 * size_of::<Option<usize>>() // two checked-add results, no and_then
        + size_of::<Option<&str>>() // exact classifier return; no filter transport
        + size_of::<&Option<String>>() // match the already-owned help field
        + 6 * size_of::<&String>() // help binding/as_str, message, witness len/push, capacity
        + 10 * size_of::<&str>() // classifier/selected/comparison/push/len receiving roles
        + 4 * size_of::<&&str>() // code and exact repair-text equality arguments
        + 4 * size_of::<&mut String>() // try_reserve_exact and three push_str calls
        + size_of::<Result<(), std::collections::TryReserveError>>()
        + size_of::<&Result<(), std::collections::TryReserveError>>() // is_err
        + 9 * size_of::<bool>() // two admissions/negations, two guards, error/capacity/OR
}

/// Optional composition has two real frontiers: control acquisition, then
/// exact String backing. A later refusal keeps the already-paid control spend;
/// neither frontier selects a failure or makes the optional String mandatory.
pub(super) fn with_capacity_witness(mut diagnostic: Diagnostic, witness: String) -> Diagnostic {
    if !crate::bounded_output::reserve_active_preserving(capacity_witness_receiving_bytes(), 0) {
        // Initial unreceived admission publishes the already-owned old fallback.
        return diagnostic.with_help(witness);
    }
    let repair = match &diagnostic.help {
        Some(help) if diagnostic.code == "SPX-T269" => {
            match authoring_repair_help(diagnostic.code, &diagnostic.message) {
                Some(expected) if expected == help.as_str() => expected,
                _ => return diagnostic.with_help(witness),
            }
        }
        _ => return diagnostic.with_help(witness),
    };
    let witness_bytes = witness.len();
    let repair_bytes = repair.len();
    let with_separator = match witness_bytes.checked_add(2) {
        Some(length) => length,
        None => return diagnostic.with_help(witness),
    };
    let length = match with_separator.checked_add(repair_bytes) {
        Some(length) => length,
        None => return diagnostic.with_help(witness),
    };
    if !crate::bounded_output::reserve_active_preserving(length, 0) {
        return diagnostic.with_help(witness);
    }
    let mut composed = String::new();
    if composed.try_reserve_exact(length).is_err() || composed.capacity() != length {
        return diagnostic.with_help(witness);
    }
    composed.push_str(&witness);
    composed.push_str("\n\n");
    composed.push_str(repair);
    diagnostic.help = Some(composed);
    diagnostic
}

#[cfg(test)]
mod before_capacity_witness;

#[cfg(test)]
mod authoring_repair_tests {
    use super::*;
    fn primary() -> Diagnostic {
        Diagnostic::io_static("SPX-M104", "invalid or duplicate pattern binding `value`")
    }
    #[test]
    fn optional_help_pays_exact_backing_and_preserves_primary() {
        let original = primary();
        let length = authoring_repair_help(original.code, &original.message)
            .unwrap()
            .len();
        let (found, overflow, used) = crate::bounded_output::with_limit_usage(length, || {
            with_authoring_repair_help(original.clone())
        });
        assert!(!overflow);
        assert_eq!(used, length);
        assert_eq!(found.help.as_ref().unwrap().capacity(), length);
        let mut stripped = found;
        stripped.help = None;
        assert_eq!(stripped, original);
    }
    #[test]
    fn short_zero_and_reserved_floor_keep_the_primary_without_refusal() {
        let length = authoring_repair_help(primary().code, &primary().message)
            .unwrap()
            .len();
        for limit in [0, length - 1, length] {
            let (found, overflow, used, refusal) =
                crate::bounded_output::with_limit_usage_refusal(limit, || {
                    if limit == length {
                        assert!(crate::bounded_output::set_active_floor(1));
                    }
                    with_authoring_repair_help(primary())
                });
            assert_eq!(found, primary());
            assert!(!overflow);
            assert_eq!(used, 0);
            assert_eq!(refusal, None);
        }
    }
    #[test]
    fn sticky_refusal_and_existing_help_remain_authoritative() {
        let (found, overflow, used, refusal) =
            crate::bounded_output::with_limit_usage_refusal(4096, || {
                assert!(!crate::bounded_output::reserve_active_required(4097));
                with_authoring_repair_help(primary())
            });
        assert_eq!(found, primary());
        assert!(overflow);
        assert_eq!(used, 0);
        assert_eq!(refusal.unwrap().requested, 4097);
        let original = primary().with_help("existing authoritative help");
        assert_eq!(with_authoring_repair_help(original.clone()), original);
    }
    #[test]
    fn reasons_do_not_decorate_unrelated_fields_or_transcript_errors() {
        assert!(
            authoring_repair_help("SPX-M104", "unknown or duplicate pattern field `x`").is_none()
        );
        assert!(authoring_repair_help("SPX-T269", "stdout_write expects Slice<u8>").is_none());
        for message in [
            "stdout_write path reaches 2 sites; limit is 1",
            "stderr_write path reaches 2 sites; limit is 1",
            "transcript output is reachable from a while condition",
            "a while body may contain only direct runtime-bounded append output",
            "transcript-write executable closure is cyclic",
        ] {
            assert!(authoring_repair_help("SPX-T269", message).is_some());
        }
    }

    #[test]
    fn publication_witness_keeps_exact_repair_and_primary_at_optional_limits() {
        use crate::bounded_output::{set_active_floor, with_limit_usage_refusal};
        let mut original = with_authoring_repair_help(Diagnostic::io_static(
            "SPX-T269",
            "stdout_write path reaches 2 sites; limit is 1",
        ));
        original.path = Some("publication.spx".to_owned());
        original.span = Some(crate::ast::Span {
            start: 7,
            end: 13,
            line: 2,
            column: 3,
        });
        let witness = "call path: app.main -> app.write; first and second stdout sites";
        let expected = format!("{witness}\n\n{}", original.help.as_deref().unwrap());
        let controls = capacity_witness_receiving_bytes();
        let exact = controls + expected.len();
        assert!(controls > 0 && expected.len() > 1);
        for (limit, floor) in [
            (0, 0),
            (controls - 1, 0),
            (controls, 0),
            (exact - 1, 0),
            (exact, 0),
            (controls, 1),
            (exact, 1),
        ] {
            let input = original.clone();
            let proof = witness.to_owned();
            let (actual, refused, used, first) = with_limit_usage_refusal(limit, || {
                assert!(set_active_floor(floor));
                with_capacity_witness(input, proof)
            });
            assert!(!refused && first.is_none());
            eprintln!("capacity_witness controls={controls} backing={} limit={limit} floor={floor} used={used}", expected.len());
            if limit == exact && floor == 0 {
                assert_eq!(used, exact);
                assert_eq!(actual, original.clone().with_help(&expected));
                assert_eq!(actual.help.as_ref().unwrap().capacity(), expected.len());
                assert!(actual.help.as_deref().unwrap().starts_with(witness));
            } else {
                // Paid control work is not refunded when optional backing is short.
                assert_eq!(
                    used,
                    if limit - floor >= controls {
                        controls
                    } else {
                        0
                    }
                );
                assert_eq!(actual, original.clone().with_help(witness));
            }
        }
    }

    #[test]
    fn publication_witness_never_replaces_a_sticky_refusal_or_invents_a_repair() {
        use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal};
        let original = with_authoring_repair_help(Diagnostic::io_static(
            "SPX-T269",
            "stderr_write path reaches 2 sites; limit is 1",
        ));
        let expected = original.clone().with_help("original witness");
        let witness = "original witness".to_owned();
        let (actual, refused, used, first) = with_limit_usage_refusal(0, || {
            assert!(!reserve_active_required(1));
            with_capacity_witness(original, witness)
        });
        assert!(refused);
        assert_eq!(used, 0);
        assert_eq!(first.unwrap().requested, 1);
        assert_eq!(actual, expected);
        let unrelated = Diagnostic::io_static("SPX-T269", "stdout_write expects Slice<u8>")
            .with_help("existing unrelated help");
        assert_eq!(
            with_capacity_witness(unrelated.clone(), "witness".to_owned()),
            unrelated.with_help("witness")
        );
        // An absent repair and an explicit empty repair remain distinct inputs;
        // both preserve the old complete Some("") witness fallback.
        for help in [None, Some(String::new())] {
            let mut original =
                Diagnostic::io_static("SPX-T269", "stdout_write path reaches 2 sites; limit is 1");
            original.help = help;
            let expected = original.clone().with_help(String::new());
            let (actual, refused, used, first) =
                with_limit_usage_refusal(capacity_witness_receiving_bytes(), || {
                    with_capacity_witness(original, String::new())
                });
            assert_eq!(actual, expected);
            assert_eq!(actual.help.as_deref(), Some(""));
            assert_eq!(used, capacity_witness_receiving_bytes());
            assert!(!refused && first.is_none());
        }
    }

    #[test]
    fn publication_composition_pays_all_backing_and_complete_drop() {
        use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal};
        use crate::test_allocation_observer::measure;
        let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let diagnostic = with_authoring_repair_help(Diagnostic::io_static(
                    "SPX-T269",
                    "stdout_write path reaches 2 sites; limit is 1",
                ));
                let witness = "original causal witness";
                assert!(reserve_active_required(witness.len()));
                let diagnostic = with_capacity_witness(diagnostic, witness.to_owned());
                assert!(diagnostic.help.as_deref().unwrap().starts_with(witness));
                assert!(diagnostic
                    .help
                    .as_deref()
                    .unwrap()
                    .contains("publish once per stream"));
                drop(diagnostic);
            })
        });
        assert!(!refused && first.is_none());
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        assert!(allocation.allocated <= paid);
        // Input ownership and every partial Drop are inside this second probe.
        let controls = capacity_witness_receiving_bytes();
        let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let diagnostic = with_authoring_repair_help(Diagnostic::io_static(
                    "SPX-T269",
                    "stdout_write path reaches 2 sites; limit is 1",
                ));
                let witness = "original causal witness";
                assert!(reserve_active_required(witness.len()));
                let witness = witness.to_owned();
                // A floor leaves room only for the real control bank. No String
                // backing is acquired, and the complete old witness is retained.
                let remaining = crate::bounded_output::active_remaining().unwrap();
                assert!(crate::bounded_output::set_active_floor(
                    remaining - controls
                ));
                let diagnostic = with_capacity_witness(diagnostic, witness);
                assert_eq!(diagnostic.code, "SPX-T269");
                assert_eq!(diagnostic.help.as_deref(), Some("original causal witness"));
                drop(diagnostic);
            })
        });
        assert!(!refused && first.is_none());
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        assert!(allocation.allocated <= paid);
        complete_capacity_witness_source_custody();
    }

    fn complete_capacity_witness_source_custody() {
        let helpers = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/capacity-witness-controls-851-next-20261016/helpers.before.rs.txt"
        ));
        let tests = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/capacity-witness-controls-851-next-20261016/three-tests.before.rs.txt"
        ));
        let expected = format!("//! Complete Main8fb capacity-witness receiver and all three original controls.\nuse super::*;\n\n{helpers}\n#[cfg(test)]\nmod tests {{\n    use super::*;\n\n{tests}\n}}\n");
        assert_eq!(include_str!("hints/before_capacity_witness.rs"), expected);
        let receiving = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/capacity-witness-controls-851-next-20261016/helpers.after.rs.txt"
        ));
        assert_eq!(include_str!("hints.rs").matches(receiving).count(), 1);
    }
}
