//! Checked, ordinary-source application JSON codecs. Generated names confer no authority.

mod collection_response;
mod emit;
mod nested_request;
mod nested_response;
mod owned;
mod template;
#[cfg(test)]
mod tests;
mod utf8;
mod views;

use crate::ast::{Type, TypeDeclaration, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;
use crate::semantic_workspace::SemanticWorkspaceSource;

use super::ProjectRevision;

const MAX_SCHEMA_BYTES: usize = 65_536;
const MAX_GENERATED_BYTES: usize = 131_072;
const MAX_FIELDS: usize = 8;

/// Explicit wire policy; it is not authority to install generated HIR.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JsonCodecProfile {
    FlatScalars,
    /// One String identifier becomes a source-relative checked ASCII view.
    IdentifierViews,
    /// Two bounded arrays: String identifiers and identifier-view records.
    RequestViews,
    /// Selected streaming grammar; original source must permit stdin.read.
    StreamRequestViews,
    /// The identifier request policy, materialized into actual owning values.
    OwnedRequest,
    /// Original stdin permit plus normalization and owning request decoding.
    StreamOwnedRequest,
    /// Plain UTF-8 Strings with an explicit decoded byte bound; no identifier domain.
    Utf8OwnedRequest {
        max_string_bytes: usize,
    },
    /// Original stdin permit plus normalization and plain UTF-8 owning values.
    StreamUtf8OwnedRequest {
        max_string_bytes: usize,
    },
    /// Finite nested response: one Row vector and one flat scalar metrics record.
    CollectionResponse {
        max_string_bytes: usize,
    },
    /// Borrowed finite nested encoding with one scalar or flat-record vector.
    NestedResponse {
        max_string_bytes: usize,
        max_array_items: usize,
    },
    /// Direct finite nested decoding with one existing vector carrier.
    NestedRequest {
        max_string_bytes: usize,
        max_array_items: usize,
    },
    /// Original stdin permit and bounded normalized spelling for nested owners.
    StreamNestedRequest {
        max_string_bytes: usize,
        max_array_items: usize,
    },
}

pub(super) fn refusal(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-J180", message).with_help(
        "Select a closed JSON codec profile with explicit schema identities and no invariants; declare scan/token/digits/write dependencies; identifier views use the authenticated query dependency in scan's bundled closure. See docs/APPLICATION-JSON-CODECS-V1.md for shape, capacity and original-permit requirements",
    )]
}

/// Derive a checked canonical replacement of one existing Project source.
///
/// Wire names are field display names, ordering is declaration order, and
/// identities come from this revision's authenticated source. This is source
/// generation, not a schema-authorized HIR operation or a public nominal ABI.
/// Replacing or installing the output still uses the ordinary Project boundary.
pub fn derive_json_codec_source(
    revision: &ProjectRevision,
    source_path: &str,
    record_id: &str,
) -> Result<String, Vec<Diagnostic>> {
    derive_json_codec_source_with_profile(
        revision,
        source_path,
        record_id,
        JsonCodecProfile::FlatScalars,
    )
}

pub fn derive_json_codec_source_with_profile(
    revision: &ProjectRevision,
    source_path: &str,
    record_id: &str,
    profile: JsonCodecProfile,
) -> Result<String, Vec<Diagnostic>> {
    revision.check()?;
    let source = revision
        .sources()
        .iter()
        .find(|source| source.path() == source_path)
        .ok_or_else(|| refusal("JSON codec source must be an exact Project source path"))?;
    if !revision
        .manifest()
        .sources()
        .iter()
        .any(|path| path == source_path)
        || source.source().len() > MAX_SCHEMA_BYTES
    {
        return Err(refusal(
            "JSON codec requires an authored source of at most 65536 bytes",
        ));
    }
    let mut program = crate::parse(source.source(), source_path).map_err(|error| vec![error])?;
    let declaration = program
        .types
        .iter()
        .find(|ty| ty.stable_id == record_id)
        .ok_or_else(|| {
            refusal("JSON codec record identity was not found in the selected source")
        })?;
    let fragment = match profile {
        JsonCodecProfile::FlatScalars => {
            validate_record(declaration)?;
            emit::source(&program, declaration)
        }
        JsonCodecProfile::IdentifierViews => views::source(&program, declaration)?,
        JsonCodecProfile::RequestViews => views::request_source(&program, declaration)?,
        JsonCodecProfile::StreamRequestViews => {
            views::stream_request_source(&program, declaration)?
        }
        JsonCodecProfile::OwnedRequest => owned::source(&program, declaration, false)?,
        JsonCodecProfile::StreamOwnedRequest => owned::source(&program, declaration, true)?,
        JsonCodecProfile::Utf8OwnedRequest { max_string_bytes } => {
            utf8::source(&program, declaration, max_string_bytes)?
        }
        JsonCodecProfile::StreamUtf8OwnedRequest { max_string_bytes } => {
            utf8::stream_source(&program, declaration, max_string_bytes)?
        }
        JsonCodecProfile::CollectionResponse { max_string_bytes } => {
            collection_response::source(&program, declaration, max_string_bytes)?
        }
        JsonCodecProfile::NestedResponse {
            max_string_bytes,
            max_array_items,
        } => nested_response::derive(&program, declaration, max_string_bytes, max_array_items)?,
        JsonCodecProfile::NestedRequest {
            max_string_bytes,
            max_array_items,
        } => nested_request::derive(&program, declaration, max_string_bytes, max_array_items)?,
        JsonCodecProfile::StreamNestedRequest {
            max_string_bytes,
            max_array_items,
        } => {
            nested_request::derive_stream(&program, declaration, max_string_bytes, max_array_items)?
        }
    };
    let fragment = template::discard_bindings(&fragment, source.source(), source_path)
        .map_err(|error| vec![error])?;
    if fragment.len() > MAX_GENERATED_BYTES {
        return Err(refusal(
            "JSON codec generated source exceeds its fixed 131072-byte bound",
        ));
    }
    let mut generated = crate::parse(&fragment, source_path).map_err(|error| vec![error])?;
    // Import aliases and helper/type identities are checked normally, including
    // collisions with authored declarations. No hidden function exemption.
    for import in generated.module_uses.drain(..) {
        if !program.module_uses.iter().any(|existing| {
            existing.kind == import.kind
                && existing.persistent_id == import.persistent_id
                && existing.target_module == import.target_module
                && existing.alias == import.alias
        }) {
            program.module_uses.push(import);
        }
    }
    program.types.append(&mut generated.types);
    program.functions.append(&mut generated.functions);
    let canonical = crate::format::canonical(&program);
    if canonical.len() > MAX_SCHEMA_BYTES + MAX_GENERATED_BYTES {
        return Err(refusal(
            "JSON codec replacement exceeds its fixed canonical source bound",
        ));
    }
    let reparsed = crate::parse(&canonical, source_path).map_err(|error| vec![error])?;
    if crate::format::canonical(&reparsed) != canonical {
        return Err(refusal("JSON codec canonical source did not round trip"));
    }
    // The round trip has completed. These two complete ASTs and the fragment
    // are not inputs to Project replay; release them before its peak allocation.
    drop(reparsed);
    drop(program);
    drop(generated);
    drop(fragment);
    validate_candidate_source(revision, source_path, canonical)
}

/// Move the canonical replacement through ordinary complete Project admission
/// and return its exact retained source buffer. No generated-source authority.
fn validate_candidate_source(
    revision: &ProjectRevision,
    source_path: &str,
    canonical: String,
) -> Result<String, Vec<Diagnostic>> {
    let mut replacement = Some(canonical);
    let sources = revision
        .sources()
        .iter()
        .map(|source| SemanticWorkspaceSource {
            path: source.path().to_owned(),
            source: if source.path() == source_path {
                replacement
                    .take()
                    .expect("authenticated Project paths are unique")
            } else {
                source.source().to_owned()
            },
        })
        .collect();
    // The original profile, capabilities, exports, dependency inventory, HIR
    // validation, graph construction and cache-compatible source facts remain
    // authoritative. A refused profile is never broadened by this generator.
    let candidate = super::build::build_owned(revision.manifest(), sources)?;
    crate::hir::validate(&candidate.entry_program).map_err(|error| vec![error])?;
    crate::hir::validate(&candidate.test_program).map_err(|error| vec![error])?;
    candidate
        .sources
        .into_iter()
        .find(|source| source.path == source_path)
        .map(|source| source.source)
        .ok_or_else(|| refusal("JSON codec replacement is absent from its checked candidate"))
}

/// Replay a derivation claim against the exact authenticated revision.
/// Ordinary edited source has no derivation authority merely from helper IDs.
pub fn verify_json_codec_source(
    revision: &ProjectRevision,
    source_path: &str,
    record_id: &str,
    candidate: &str,
) -> Result<(), Vec<Diagnostic>> {
    verify_json_codec_source_with_profile(
        revision,
        source_path,
        record_id,
        candidate,
        JsonCodecProfile::FlatScalars,
    )
}

pub fn verify_json_codec_source_with_profile(
    revision: &ProjectRevision,
    source_path: &str,
    record_id: &str,
    candidate: &str,
    profile: JsonCodecProfile,
) -> Result<(), Vec<Diagnostic>> {
    if candidate.len() > MAX_SCHEMA_BYTES + MAX_GENERATED_BYTES
        || derive_json_codec_source_with_profile(revision, source_path, record_id, profile)?
            != candidate
    {
        return Err(refusal(
            "JSON codec source differs from its checked-source derivation",
        ));
    }
    Ok(())
}

fn validate_record(record: &TypeDeclaration) -> Result<(), Vec<Diagnostic>> {
    let TypeDeclarationKind::Record { fields } = &record.kind else {
        return Err(refusal("JSON codec root must be a record"));
    };
    if !record.explicit_id
        || !record.type_parameters.is_empty()
        || !record.invariants().is_empty()
        || fields.is_empty()
        || fields.len() > MAX_FIELDS
        || record.stable_id.len() > 80
        || record.name.len() > 64
        || !record
            .stable_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-:".contains(&byte))
        || !identifier(&record.name)
    {
        return Err(refusal(
            "JSON codec requires a bounded explicitly identified monomorphic record without invariants",
        ));
    }
    for field in fields {
        if !field.explicit_id
            || !identifier(&field.name)
            || field.name.len() > 64
            || !matches!(field.ty, Type::I64 | Type::U8 | Type::Usize | Type::Bool)
        {
            return Err(refusal(format!(
                "JSON codec field `{}` is outside the flat scalar v1 contract",
                field.stable_id
            )));
        }
    }
    Ok(())
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().enumerate().all(|(index, byte)| {
            byte == b'_' || byte.is_ascii_alphabetic() || index > 0 && byte.is_ascii_digit()
        })
}
