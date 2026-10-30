//! Import-inventory admission for legacy scalar Web packages.
//!
//! The generated browser loader links a fixed host object: without owned
//! exports it instantiates with `{ env: { ...imports.env, ...byteRuntime.imports } }`,
//! and with owned exports it also spreads `runtime.linkImports.env`
//! (`browser_runtime.js`, `instantiateBytes`). A legacy package whose module
//! imports anything outside the inventory of the selected branch cannot be
//! instantiated by its own loader, so `build_web` refuses it with SPX-W131
//! before it creates the output directory.
//!
//! The check reads import headers only; it is not a full module validation.
//! Signature compatibility is not checked because the loader supplies plain
//! JavaScript functions.

use wasmparser::{Parser, Payload, TypeRef};

use crate::diagnostic::Diagnostic;

/// Keys of `imports.env` followed by the keys of `byteImports`, in the order
/// the generated browser runtime declares them.
const LOADER_BASE_IMPORTS: [&str; 35] = [
    // imports.env
    "spx_add",
    "spx_sub",
    "spx_mul",
    "spx_div",
    "spx_rem",
    "spx_neg",
    "spx_contract_fail",
    // byteImports
    "spx_format_step_v1",
    "spx_collection_checked_v2",
    "spx_collection_drop_v2",
    "spx_bytes_copy",
    "spx_bytes_zeroed",
    "spx_bytes_set",
    "spx_bytes_set5",
    "spx_bytes_set1_or5",
    "spx_bytes_set1_or6_or48",
    "spx_bytes_get",
    "spx_bytes_drop",
    "spx_bytes_as_slice",
    "spx_string_slice_v2",
    "spx_string_find_v2",
    "spx_string_to_i64_v2",
    "spx_string_trim_v2",
    "spx_string_byte_at_v2",
    "spx_string_from_utf8_v1",
    "spx_string_from_str_v2",
    "spx_file_read_text_v2",
    "spx_string_concat_v1",
    "spx_string_from_char_v1",
    "spx_string_len_chars_v1",
    "spx_string_from_i64_v1",
    "spx_string_from_usize_v1",
    "spx_string_starts_with_v1",
    "spx_string_contains_v1",
    "spx_string_compare_v2",
];

const LOADER_MODULE: &str = "env";

/// Admit `bytes` for the legacy Web package only when every import is an
/// `env` function the generated loader supplies on the branch it will take.
///
/// `owned_branch` is true exactly when the package carries owned exports,
/// which is when the loader also links the owned-ABI imports.
pub(super) fn admit(bytes: &[u8], owned_branch: bool) -> Result<(), Diagnostic> {
    for payload in Parser::new(0).parse_all(bytes) {
        let payload = payload.map_err(malformed)?;
        let Payload::ImportSection(section) = payload else {
            continue;
        };
        for import in section.into_imports() {
            let import = import.map_err(malformed)?;
            let supplied = import.module == LOADER_MODULE
                && matches!(import.ty, TypeRef::Func(_))
                && (LOADER_BASE_IMPORTS.contains(&import.name)
                    || (owned_branch && super::owned::IMPORT_NAMES.contains(&import.name)));
            if !supplied {
                return Err(unsupported(import.module, import.name, import.ty));
            }
        }
    }
    Ok(())
}

fn kind(ty: TypeRef) -> &'static str {
    match ty {
        TypeRef::Func(_) => "function",
        TypeRef::FuncExact(_) => "exact function",
        TypeRef::Table(_) => "table",
        TypeRef::Memory(_) => "memory",
        TypeRef::Global(_) => "global",
        TypeRef::Tag(_) => "tag",
    }
}

fn unsupported(module: &str, name: &str, ty: TypeRef) -> Diagnostic {
    let kind = kind(ty);
    Diagnostic::io(
        "SPX-W131",
        format!(
            "legacy scalar Web packages do not supply the required runtime import {module}.{name} ({kind})"
        ),
    )
    .with_help(
        "the generated browser loader supplies only a fixed inventory of checked-arithmetic, byte and String-toolkit, collection-check and owned-ABI imports; imports outside it, such as the Vec imports env.spx_vec_*, are refused on this route; run the program with `semaprax run` or build a native target instead",
    )
}

fn malformed(error: wasmparser::BinaryReaderError) -> Diagnostic {
    Diagnostic::io(
        "SPX-W100",
        format!("internal error: emitted Wasm import section is unreadable: {error}"),
    )
}

#[cfg(test)]
mod tests {
    use super::{admit, LOADER_BASE_IMPORTS};
    use std::collections::BTreeSet;
    use wasm_encoder::{EntityType, ImportSection, MemoryType, Module, TypeSection, ValType};

    /// Keys of the JavaScript object literal that starts at the unique
    /// `marker` line, read at the object's own four-space key indentation.
    fn object_keys(marker: &str) -> BTreeSet<String> {
        let runtime = super::super::browser_runtime();
        assert_eq!(runtime.matches(marker).count(), 1, "{marker}");
        let body = &runtime[runtime.find(marker).unwrap() + marker.len()..];
        let mut keys = BTreeSet::new();
        for line in body.lines() {
            if line.starts_with("  }") {
                break;
            }
            let Some(rest) = line.strip_prefix("    spx_") else {
                continue;
            };
            let end = rest
                .find(|character: char| {
                    !(character.is_ascii_lowercase()
                        || character.is_ascii_digit()
                        || character == '_')
                })
                .unwrap();
            if matches!(rest.as_bytes()[end], b':' | b'(') {
                assert!(keys.insert(format!("spx_{}", &rest[..end])), "{line}");
            }
        }
        keys
    }

    fn set(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn loader_inventory_matches_browser_runtime_object_keys() {
        let env = object_keys("export const imports = {");
        let bytes = object_keys("const byteImports = Object.freeze({");
        let owned = object_keys("const ownedImports = {");
        assert_eq!(env.len(), 7);
        assert_eq!(bytes.len(), 28);
        let mut base = env.clone();
        base.extend(bytes);
        assert_eq!(base, set(&LOADER_BASE_IMPORTS));
        assert_eq!(base.len(), LOADER_BASE_IMPORTS.len());
        assert_eq!(owned, set(&super::super::owned::IMPORT_NAMES));
    }

    fn module_with(imports: &[(&str, &str, bool)]) -> Vec<u8> {
        let mut types = TypeSection::new();
        types.ty().function([ValType::I64], [ValType::I64]);
        let mut section = ImportSection::new();
        for (module, name, function) in imports {
            let ty = if *function {
                EntityType::Function(0)
            } else {
                EntityType::Memory(MemoryType {
                    minimum: 1,
                    maximum: None,
                    memory64: false,
                    shared: false,
                    page_size_log2: None,
                })
            };
            section.import(module, name, ty);
        }
        let mut module = Module::new();
        module.section(&types);
        if !imports.is_empty() {
            module.section(&section);
        }
        module.finish()
    }

    fn refusal(bytes: &[u8], owned: bool) -> crate::diagnostic::Diagnostic {
        admit(bytes, owned).unwrap_err()
    }

    #[test]
    fn base_env_function_imports_are_admitted_on_both_branches() {
        let bytes = module_with(&[
            ("env", "spx_add", true),
            ("env", "spx_contract_fail", true),
            ("env", "spx_string_compare_v2", true),
        ]);
        admit(&bytes, false).unwrap();
        admit(&bytes, true).unwrap();
        admit(&module_with(&[]), false).unwrap();
    }

    #[test]
    fn vec_import_is_refused_with_its_qualified_name() {
        let bytes = module_with(&[
            ("env", "spx_add", true),
            ("env", "spx_vec_with_capacity", true),
        ]);
        for owned in [false, true] {
            let error = refusal(&bytes, owned);
            assert_eq!(error.code, "SPX-W131");
            assert!(
                error
                    .message
                    .contains("env.spx_vec_with_capacity (function)"),
                "{}",
                error.message
            );
        }
    }

    #[test]
    fn owned_abi_imports_are_admitted_only_on_the_owned_branch() {
        let bytes = module_with(&[("env", "spx_owned_begin", true)]);
        let error = refusal(&bytes, false);
        assert_eq!(error.code, "SPX-W131");
        assert!(error.message.contains("env.spx_owned_begin"));
        admit(&bytes, true).unwrap();
    }

    #[test]
    fn foreign_module_and_non_function_imports_are_refused() {
        let foreign = refusal(&module_with(&[("host", "spx_add", true)]), true);
        assert_eq!(foreign.code, "SPX-W131");
        assert!(foreign.message.contains("host.spx_add (function)"));
        let memory = refusal(&module_with(&[("env", "spx_add", false)]), true);
        assert_eq!(memory.code, "SPX-W131");
        assert!(memory.message.contains("env.spx_add (memory)"));
    }

    #[test]
    fn first_unsupported_import_in_section_order_is_reported() {
        let bytes = module_with(&[
            ("env", "spx_vec_len", true),
            ("env", "spx_vec_with_capacity", true),
        ]);
        let error = refusal(&bytes, false);
        assert!(
            error.message.contains("env.spx_vec_len"),
            "{}",
            error.message
        );
        assert!(!error.message.contains("spx_vec_with_capacity"));
    }

    #[test]
    fn unreadable_import_sections_are_internal_errors_not_admissions() {
        let bytes = module_with(&[("env", "spx_vec_with_capacity", true)]);
        // A section whose declared size runs past the end of the input.
        let truncated = &bytes[..bytes.len() - 3];
        assert_eq!(refusal(truncated, false).code, "SPX-W100");
        // A complete section whose import count promises more entries than
        // its bytes hold: the per-entry reader fails, not the payload.
        let valid = module_with(&[("env", "spx_add", true)]);
        let mut lying = valid.clone();
        let count = lying.len() - (1 + "env".len() + 1 + "spx_add".len() + 2) - 1;
        assert_eq!(lying[count], 1);
        lying[count] = 2;
        assert_eq!(refusal(&lying, false).code, "SPX-W100");
    }
}
