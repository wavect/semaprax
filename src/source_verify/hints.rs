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
use std::sync::OnceLock;

use super::diagnostics::error;
use super::type_table::TypeTable;
use crate::ast::{
    Expr, ExprKind, Function, Program, Span, Type, TypeDeclarationKind, VariantCaseDeclaration,
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
    provider: String,
    id: String,
    signature: String,
}

/// Foreign spellings of an operation the language provides under another name.
fn foreign_function_help(name: &str) -> Option<&'static str> {
    match name {
        "i64_from_u8" => Some(
            "`u8` conversion is `std.bytes.byte_to_i64`, not a compiler builtin; inspect it with \
             `semaprax help library std.bytes.byte_to_i64` and import it in a `useful-data.v1` \
             project, or declare a byte conversion helper in a single file",
        ),
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
    if let Some(help) = variant_shorthand_help(name).or_else(|| foreign_function_help(name)) {
        return diagnostic.with_help(help);
    }
    // An exact standard-library name outranks a near local spelling: `min`
    // is `std.core`'s, not a typo of `main`.
    if let Some(function) = standard_library_function(name) {
        let StandardFunction {
            dependency,
            provider,
            id,
            signature,
        } = function;
        return diagnostic.with_help(format!(
            "`{signature}` is in `{provider}`: add `[dependencies] {dependency} = \"^0.1.0\"` to \
             `semaprax.toml` and import it directly after the `module` line: `use function \
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
             `let view = string_as_str(text); let bytes = str_as_bytes(view); {name}(bytes)`"
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

/// A borrowed-view operation applied to something other than a plain binding.
pub(super) fn view_place_help(operation: &str, argument: &Expr) -> String {
    let (source, binding) = match (operation, &argument.kind) {
        ("str_as_bytes", ExprKind::String(_)) | ("string_as_str", ExprKind::String(_)) => (
            "a string literal",
            "`let text = \"…\"; let view = string_as_str(text); str_as_bytes(view)`",
        ),
        ("array_as_slice", ExprKind::ArrayU8(_) | ExprKind::RepeatArrayU8 { .. }) => (
            "an array literal",
            "`let bytes = [1u8, 2u8]; array_as_slice(bytes)`",
        ),
        ("str_as_bytes", ExprKind::Call { name, .. }) if name == "string_as_str" => (
            "a nested view",
            "`let view = string_as_str(text); str_as_bytes(view)`",
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
    "join strings with `string_concat(a, b)`, which consumes both; strings compare with `==` and `!=`";
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
    program: &Program,
    function: &Function,
    effect: &str,
) -> Diagnostic {
    error(
        program,
        "SPX-E101",
        format!(
            "function `{}` uses `{effect}` but module `{}` does not permit it",
            function.name, program.module
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
    program: &Program,
    callee: &str,
    effect: &str,
    caller: &Function,
    span: Span,
) -> Diagnostic {
    let uses = format!("add `uses {{ {effect} }}` on its own line between the signature and `{{`");
    let help = if program.permits.iter().any(|permit| permit == effect) {
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

/// A moved resource read again. Strings and byte buffers have no implicit
/// copy, so the generic "borrow it" advice needs the concrete route.
pub(super) fn moved_resource_help(ty: &Type) -> &'static str {
    match ty {
        Type::String => {
            "`string_concat` and `own` parameters consume a `string`; pass `string_as_str(name)` \
             to a `borrow str` parameter instead, or build a second string before the first use"
        }
        Type::Bytes => {
            "`own` parameters consume `Bytes`; pass `bytes_as_slice(name)` to a `borrow \
             Slice<u8>` parameter instead, or copy first with `bytes_copy(bytes_as_slice(name))`"
        }
        _ => "borrow the resource if the callee does not need ownership",
    }
}
