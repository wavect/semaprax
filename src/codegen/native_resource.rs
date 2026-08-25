//! Deterministic C11 resource wrapper and finalizer ABI descriptors.
//!
//! This module is deliberately only a representation scaffold. It does not
//! lower cleanup plans, call finalizers, or make resource-bearing programs
//! executable. `codegen.rs` keeps the `SPX-B104` gate until cleanup and trace
//! conformance are implemented.

// The ABI is intentionally staged behind SPX-B104. Its descriptors become
// production-reachable with cleanup lowering; tests exercise them meanwhile.
#![cfg_attr(not(test), allow(dead_code))]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, DeclarationId, OwnershipMode, ResolvedImport, ResolvedImportFailure,
    ResolvedImportResultKind, ResolvedProgram, ResolvedResourceDropKind, ResolvedType,
    ResolvedTypeDeclarationKind,
};

const RESOURCE_SYMBOL_DOMAIN: &[u8] = b"semaprax.native-resource-type.v1\0";
const FINALIZER_SYMBOL_DOMAIN: &[u8] = b"semaprax.native-finalizer-import.v1\0";
const SYMBOL_DIGEST_BYTES: usize = 24;

/// Complete, deterministic C declaration scaffold for one resolved program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NativeResourceAbi {
    pub(super) declarations: String,
    pub(super) resources: Vec<NativeResourceDescriptor>,
    pub(super) lifecycles: Vec<NativeLifecycleDescriptor>,
}

impl NativeResourceAbi {
    /// Select the exact C representation for a resolved scalar or direct
    /// opaque-resource type. Aggregate and generic representations remain
    /// gated until their target layouts are validated.
    pub(super) fn c_type<'a>(
        &'a self,
        program: &ResolvedProgram,
        ty: &ResolvedType,
    ) -> Result<&'a str, Diagnostic> {
        match ty {
            ResolvedType::Unit => Err(resource_error(
                "unit has no ordinary native value representation",
            )),
            ResolvedType::I64 => Ok("int64_t"),
            ResolvedType::I32 => Ok("int32_t"),
            ResolvedType::Char => Ok("uint32_t"),
            ResolvedType::U8 => Ok("uint8_t"),
            ResolvedType::Usize => Ok("uint64_t"),
            ResolvedType::F32 => Ok("float"),
            ResolvedType::F64 => Ok("double"),
            ResolvedType::Bool => Ok("bool"),
            // Owned strings lower to a C heap pointer; the backend owns the
            // allocation and frees it exactly once per value.
            ResolvedType::String => Ok("char *"),
            // Borrowed UTF-8 text is a non-owning pointer/length view. The
            // definition is emitted only for programs reaching Str ops.
            ResolvedType::Str => Ok("spx_str_v1"),
            ResolvedType::SliceU8 => Ok("spx_slice_u8_v1"),
            ResolvedType::TypeParameter { .. } => Err(resource_error(format!(
                "native representation is unavailable for generic type `{}`",
                ty.identity_key()
            ))),
            ResolvedType::Nominal {
                declaration,
                arguments,
            } => {
                if !arguments.is_empty() {
                    return Err(resource_error(format!(
                        "native representation is unavailable for generic nominal type `{}`",
                        ty.identity_key()
                    )));
                }
                let resolved = program
                    .types
                    .iter()
                    .find(|candidate| candidate.id == *declaration)
                    .ok_or_else(|| {
                        resource_error(format!(
                            "native representation references unknown type `{declaration}`"
                        ))
                    })?;
                match &resolved.kind {
                    ResolvedTypeDeclarationKind::Record { .. }
                    | ResolvedTypeDeclarationKind::Class { .. } => Err(resource_error(format!(
                        "native aggregate representation is unavailable for record `{declaration}`"
                    ))),
                    ResolvedTypeDeclarationKind::Variant { .. } => Err(resource_error(format!(
                        "native variant representation is unavailable for variant `{declaration}`"
                    ))),
                    ResolvedTypeDeclarationKind::Resource { .. } => self
                        .resources
                        .binary_search_by(|resource| resource.resource_id.cmp(declaration))
                        .ok()
                        .map(|index| self.resources[index].c_type.as_str())
                        .ok_or_else(|| {
                            resource_error(format!(
                                "native resource ABI has no wrapper for `{declaration}`"
                            ))
                        }),
                }
            }
        }
    }
}

/// Stable-ID-derived physical type for one opaque resource declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NativeResourceDescriptor {
    pub(super) resource_id: DeclarationId,
    pub(super) c_type: String,
}

/// One resolved lifecycle and its target binding, if it has one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NativeLifecycleDescriptor {
    pub(super) lifecycle_id: DeclarationId,
    pub(super) resource_id: DeclarationId,
    pub(super) resource_c_type: String,
    pub(super) kind: NativeFinalizerKind,
}

/// Trivial lifecycles have no host slot. Imported lifecycles have a strongly
/// typed callback descriptor but are not callable until cleanup lowering uses
/// them after binding preflight.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum NativeFinalizerKind {
    Trivial,
    Imported(NativeImportedFinalizer),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NativeImportedFinalizer {
    pub(super) import_id: DeclarationId,
    pub(super) import_key: String,
    pub(super) callback_type: String,
    pub(super) binding_field: String,
}

/// Validate the resolved contracts and build the resource-side C ABI scaffold.
///
/// Hook contract for `codegen.rs`:
///
/// ```text
/// let resource_abi = native_resource::build_resource_abi(program)?;
/// output.push_str(&resource_abi.declarations);
/// ```
///
/// The caller must still reject resource execution until it consumes the
/// descriptors to lower the validated cleanup plan and exact trace protocol.
pub(super) fn build_resource_abi(
    program: &ResolvedProgram,
) -> Result<NativeResourceAbi, Diagnostic> {
    hir::validate(program)?;

    if (!program.function_templates.is_empty() || !program.function_instances.is_empty())
        && program.types.iter().any(|declaration| {
            matches!(
                declaration.kind,
                ResolvedTypeDeclarationKind::Resource { .. }
            )
        })
    {
        return Err(resource_error(
            "native resource ABI does not admit mixed generic functions and resources",
        ));
    }

    let imports = import_index(program)?;
    let mut identifiers = BTreeMap::<String, String>::new();
    let mut lifecycle_ids = BTreeSet::new();
    let mut imported_lifecycles = BTreeSet::new();
    let mut resources = Vec::new();
    let mut lifecycles = Vec::new();

    for declaration in &program.types {
        let ResolvedTypeDeclarationKind::Resource { drop } = &declaration.kind else {
            continue;
        };
        require_identity("resource", &declaration.id)?;
        require_identity("resource lifecycle", &drop.id)?;
        if !lifecycle_ids.insert(drop.id.clone()) {
            return Err(resource_error(format!(
                "duplicate native lifecycle identity `{}`",
                drop.id
            )));
        }

        let c_type = stable_identifier("spx_r_", RESOURCE_SYMBOL_DOMAIN, &declaration.id);
        register_identifier(
            &mut identifiers,
            &c_type,
            format!("resource `{}`", declaration.id),
        )?;
        resources.push(NativeResourceDescriptor {
            resource_id: declaration.id.clone(),
            c_type: c_type.clone(),
        });

        let kind = match &drop.kind {
            ResolvedResourceDropKind::Trivial => NativeFinalizerKind::Trivial,
            ResolvedResourceDropKind::Imported { import, import_key } => {
                require_identity("finalizer import", import)?;
                if !imported_lifecycles.insert(import.clone()) {
                    return Err(resource_error(format!(
                        "finalizer import `{import}` is bound by more than one lifecycle"
                    )));
                }
                let resolved = imports.get(import).ok_or_else(|| {
                    resource_error(format!(
                        "lifecycle `{}` references unknown finalizer import `{import}`",
                        drop.id
                    ))
                })?;
                validate_finalizer_import(program, declaration, resolved, import_key)?;

                let callback_type = stable_identifier("spx_f_", FINALIZER_SYMBOL_DOMAIN, import);
                let binding_field = stable_identifier("spx_i_", FINALIZER_SYMBOL_DOMAIN, import);
                register_identifier(
                    &mut identifiers,
                    &callback_type,
                    format!("finalizer callback type `{import}`"),
                )?;
                // Members have their own namespace in C, but collision checking
                // here keeps a future flat generated binding API fail-closed.
                register_identifier(
                    &mut identifiers,
                    &binding_field,
                    format!("finalizer binding field `{import}`"),
                )?;
                NativeFinalizerKind::Imported(NativeImportedFinalizer {
                    import_id: import.clone(),
                    import_key: import_key.clone(),
                    callback_type,
                    binding_field,
                })
            }
        };
        lifecycles.push(NativeLifecycleDescriptor {
            lifecycle_id: drop.id.clone(),
            resource_id: declaration.id.clone(),
            resource_c_type: c_type,
            kind,
        });
    }

    resources.sort_by(|left, right| left.resource_id.cmp(&right.resource_id));
    lifecycles.sort_by(|left, right| left.lifecycle_id.cmp(&right.lifecycle_id));
    let declarations = emit_declarations(&resources, &lifecycles);
    Ok(NativeResourceAbi {
        declarations,
        resources,
        lifecycles,
    })
}

fn import_index(
    program: &ResolvedProgram,
) -> Result<BTreeMap<DeclarationId, &ResolvedImport>, Diagnostic> {
    let mut imports = BTreeMap::new();
    for interface in &program.interfaces {
        require_identity("interface", &interface.id)?;
        for import in &interface.imports {
            require_identity("import", &import.id)?;
            if import.interface != interface.id {
                return Err(resource_error(format!(
                    "finalizer import `{}` has inconsistent interface identity",
                    import.id
                )));
            }
            if imports.insert(import.id.clone(), import).is_some() {
                return Err(resource_error(format!(
                    "duplicate native import identity `{}`",
                    import.id
                )));
            }
        }
    }
    Ok(imports)
}

fn validate_finalizer_import(
    program: &ResolvedProgram,
    resource: &crate::hir::ResolvedTypeDeclaration,
    import: &ResolvedImport,
    lifecycle_import_key: &str,
) -> Result<(), Diagnostic> {
    if import.import_key.is_empty()
        || import.import_key != lifecycle_import_key
        || import.parameters.len() != 1
        || import.parameters[0].ownership != OwnershipMode::Own
        || !import.parameters[0].consumes_on_failure
        || import.result.kind != ResolvedImportResultKind::Unit
        || import.result.ownership != OwnershipMode::Value
        || import.result.producer != "callee"
        || import.result.out_slot_initialization != "success_only"
        || import.result.ownership_transfer != "final_zero_status_commit"
        || !matches!(import.failure, ResolvedImportFailure::Infallible)
        || import.required_authority != import.effects
    {
        return Err(resource_error(format!(
            "finalizer import `{}` is incompatible with lifecycle resource `{}`",
            import.id, resource.id
        )));
    }
    let expected_type = ResolvedType::Nominal {
        declaration: resource.id.clone(),
        arguments: Vec::new(),
    };
    if import.parameters[0].ty != expected_type {
        return Err(resource_error(format!(
            "finalizer import `{}` does not consume resource `{}`",
            import.id, resource.id
        )));
    }
    let Some(interface) = program
        .interfaces
        .iter()
        .find(|interface| interface.id == import.interface)
    else {
        return Err(resource_error(format!(
            "finalizer import `{}` has no resolved interface",
            import.id
        )));
    };
    let permits = interface.permits.iter().collect::<BTreeSet<_>>();
    let effects = import.effects.iter().collect::<BTreeSet<_>>();
    if permits.len() != interface.permits.len()
        || effects.len() != import.effects.len()
        || effects.iter().any(|effect| !permits.contains(effect))
    {
        return Err(resource_error(format!(
            "finalizer import `{}` has a noncanonical authority contract",
            import.id
        )));
    }
    Ok(())
}

fn emit_declarations(
    resources: &[NativeResourceDescriptor],
    lifecycles: &[NativeLifecycleDescriptor],
) -> String {
    if resources.is_empty() {
        return String::new();
    }
    let mut output = String::from(
        "/* semaprax.native-resource-abi.v1 */\n\
         #include <limits.h>\n\
         #include <stdint.h>\n\
         #if !defined(UINTPTR_MAX)\n\
         #error \"SEMAPRAX native resource ABI requires uintptr_t\"\n\
         #endif\n\
         _Static_assert(CHAR_BIT == 8, \"SEMAPRAX native resource ABI requires 8-bit bytes\");\n\
         struct spx_context;\n\n",
    );
    for resource in resources {
        writeln!(
            output,
            "/* Payload zero is valid; cleanup liveness is stored separately. */\n\
             typedef struct {} {{\n\
                 uintptr_t payload;\n\
             }} {};\n\
             _Static_assert(sizeof({}) == sizeof(uintptr_t), \"SEMAPRAX resource wrapper size\");\n\
             _Static_assert(_Alignof({}) == _Alignof(uintptr_t), \"SEMAPRAX resource wrapper alignment\");\n",
            resource.c_type, resource.c_type, resource.c_type, resource.c_type
        )
        .expect("writing to a string cannot fail");
    }
    let mut imported = lifecycles
        .iter()
        .filter_map(|lifecycle| match &lifecycle.kind {
            NativeFinalizerKind::Trivial => None,
            NativeFinalizerKind::Imported(finalizer) => Some((lifecycle, finalizer)),
        })
        .collect::<Vec<_>>();
    imported.sort_by(|(_, left), (_, right)| left.import_id.cmp(&right.import_id));
    for (lifecycle, finalizer) in imported {
        writeln!(
            output,
            "typedef void (*{})\n\
             (struct spx_context *, {});\n\
             /* binding field: {} */\n",
            finalizer.callback_type, lifecycle.resource_c_type, finalizer.binding_field
        )
        .expect("writing to a string cannot fail");
    }
    output
}

fn stable_identifier(prefix: &str, domain: &[u8], identity: &DeclarationId) -> String {
    let bytes = identity.as_str().as_bytes();
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut identifier = String::with_capacity(prefix.len() + SYMBOL_DIGEST_BYTES * 2);
    identifier.push_str(prefix);
    for byte in &digest[..SYMBOL_DIGEST_BYTES] {
        write!(identifier, "{byte:02x}").expect("writing to a string cannot fail");
    }
    identifier
}

fn register_identifier(
    identifiers: &mut BTreeMap<String, String>,
    identifier: &str,
    semantic_owner: String,
) -> Result<(), Diagnostic> {
    if let Some(previous) = identifiers.insert(identifier.to_owned(), semantic_owner.clone()) {
        return Err(resource_error(format!(
            "native resource identifier collision `{identifier}` between {previous} and {semantic_owner}"
        )));
    }
    Ok(())
}

fn require_identity(kind: &str, identity: &DeclarationId) -> Result<(), Diagnostic> {
    if identity.as_str().is_empty() {
        Err(resource_error(format!(
            "native {kind} has an empty stable identity"
        )))
    } else {
        Ok(())
    }
}

fn resource_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-B104", message)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::{hir, parse};

    use super::*;

    fn resolve(source: &str) -> ResolvedProgram {
        let parsed = parse(source, Path::new("native-resource-test.spx")).unwrap();
        hir::resolve(&parsed).unwrap()
    }

    fn source(resource_name: &str, interface_name: &str, import_name: &str) -> String {
        format!(
            r#"module test.native_resource;
permit {{ io.release }}

@id("token.type")
resource {resource_name} {{
    @id("token.drop")
    drop trivial;
}}

@id("file.type")
resource File {{
    @id("file.drop")
    drop import "file.finalize";
}}

@id("file.host")
interface {interface_name} permits {{ io.release }} {{
    @id("file.finalize")
    import fn {import_name}(file: own File) -> unit
        effects {{ io.release }}
        failure infallible
        consumes file always;
}}

@id("app.main")
fn main() -> i64 {{ 0 }}
"#
        )
    }

    #[test]
    fn display_renames_do_not_change_the_resource_abi() {
        let first = build_resource_abi(&resolve(&source("Token", "FileHost", "finalize"))).unwrap();
        let second = build_resource_abi(&resolve(&source(
            "RenamedToken",
            "RenamedFileHost",
            "renamed_finalize",
        )))
        .unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn distinct_resource_ids_produce_distinct_wrapper_types() {
        let abi = build_resource_abi(&resolve(&source("Token", "FileHost", "finalize"))).unwrap();
        assert_eq!(abi.resources.len(), 2);
        assert_ne!(abi.resources[0].c_type, abi.resources[1].c_type);
    }

    #[test]
    fn generated_identifiers_fit_the_portable_internal_identifier_budget() {
        let abi = build_resource_abi(&resolve(&source("Token", "FileHost", "finalize"))).unwrap();
        let identifiers = abi
            .resources
            .iter()
            .map(|resource| resource.c_type.as_str())
            .chain(abi.lifecycles.iter().filter_map(|lifecycle| {
                let NativeFinalizerKind::Imported(finalizer) = &lifecycle.kind else {
                    return None;
                };
                Some(finalizer.callback_type.as_str())
            }));
        for identifier in identifiers {
            assert!(
                identifier.len() <= 63,
                "identifier `{identifier}` is too long"
            );
            assert!(identifier
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'));
        }
    }

    #[test]
    fn zero_payload_is_never_emitted_as_a_liveness_test() {
        let abi = build_resource_abi(&resolve(&source("Token", "FileHost", "finalize"))).unwrap();
        assert!(abi.declarations.contains("Payload zero is valid"));
        assert!(!abi.declarations.contains("payload =="));
        assert!(!abi.declarations.contains("payload !="));
        assert!(!abi.declarations.contains("NULL"));
    }

    #[test]
    fn emission_is_deterministic() {
        let program = resolve(&source("Token", "FileHost", "finalize"));
        let first = build_resource_abi(&program).unwrap();
        let second = build_resource_abi(&program).unwrap();
        assert_eq!(first.declarations, second.declarations);
        assert_eq!(first.resources, second.resources);
        assert_eq!(first.lifecycles, second.lifecycles);

        let file = first
            .resources
            .iter()
            .find(|resource| resource.resource_id.as_str() == "file.type")
            .unwrap();
        let token = first
            .resources
            .iter()
            .find(|resource| resource.resource_id.as_str() == "token.type")
            .unwrap();
        let file_position = first.declarations.find(&file.c_type).unwrap();
        let token_position = first.declarations.find(&token.c_type).unwrap();
        let callback_position = first
            .lifecycles
            .iter()
            .find_map(|lifecycle| match &lifecycle.kind {
                NativeFinalizerKind::Imported(finalizer) => {
                    first.declarations.find(&finalizer.callback_type)
                }
                NativeFinalizerKind::Trivial => None,
            })
            .unwrap();
        assert!(file_position < token_position);
        assert!(token_position < callback_position);
    }

    #[test]
    fn type_selection_rejects_unknown_record_and_generic_shapes() {
        let program = resolve(&format!(
            "{}\n@id(\"record.type\")\nrecord Record {{\n    @id(\"record.value\")\n    value: i64,\n}}\n",
            source("Token", "FileHost", "finalize")
        ));
        let abi = build_resource_abi(&program).unwrap();

        let unknown = ResolvedType::Nominal {
            declaration: DeclarationId::new("unknown.type"),
            arguments: Vec::new(),
        };
        let record = ResolvedType::Nominal {
            declaration: DeclarationId::new("record.type"),
            arguments: Vec::new(),
        };
        let generic_nominal = ResolvedType::Nominal {
            declaration: DeclarationId::new("token.type"),
            arguments: vec![ResolvedType::I64],
        };
        let type_parameter = ResolvedType::TypeParameter {
            owner: DeclarationId::new("generic.owner"),
            index: 0,
        };

        for (ty, expected) in [
            (unknown, "unknown type"),
            (record, "record"),
            (generic_nominal, "generic nominal"),
            (type_parameter, "generic type"),
        ] {
            let diagnostic = abi.c_type(&program, &ty).unwrap_err();
            assert_eq!(diagnostic.code, "SPX-B104");
            assert!(
                diagnostic.message.contains(expected),
                "unexpected diagnostic: {}",
                diagnostic.message
            );
        }
    }

    #[test]
    fn imported_and_trivial_lifecycles_have_distinct_descriptors() {
        let abi = build_resource_abi(&resolve(&source("Token", "FileHost", "finalize"))).unwrap();
        let trivial = abi
            .lifecycles
            .iter()
            .find(|lifecycle| lifecycle.lifecycle_id.as_str() == "token.drop")
            .unwrap();
        assert_eq!(trivial.resource_id.as_str(), "token.type");
        assert!(matches!(trivial.kind, NativeFinalizerKind::Trivial));

        let imported = abi
            .lifecycles
            .iter()
            .find(|lifecycle| lifecycle.lifecycle_id.as_str() == "file.drop")
            .unwrap();
        let NativeFinalizerKind::Imported(finalizer) = &imported.kind else {
            panic!("file lifecycle must have an imported finalizer descriptor");
        };
        assert_eq!(finalizer.import_id.as_str(), "file.finalize");
        assert_eq!(finalizer.import_key, "file.finalize");
        assert!(abi.declarations.contains(&finalizer.callback_type));
        assert!(abi.declarations.contains(&finalizer.binding_field));
    }

    #[test]
    fn identifier_registration_rejects_collisions() {
        let mut identifiers = BTreeMap::new();
        register_identifier(
            &mut identifiers,
            "spx_r_collision",
            "resource `a`".to_owned(),
        )
        .unwrap();
        let diagnostic = register_identifier(
            &mut identifiers,
            "spx_r_collision",
            "resource `b`".to_owned(),
        )
        .unwrap_err();
        assert_eq!(diagnostic.code, "SPX-B104");
        assert!(diagnostic.message.contains("identifier collision"));
    }
}
