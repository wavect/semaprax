//! An independent, versioned rich-value checkpoint codec.
//!
//! `semaprax.agent-lifecycle-typed-checkpoint.v1` wraps one canonical
//! `agent_interaction_schema` decoded value with an explicit `type_version`
//! tag. It is wholly additive: it neither reads nor changes the existing
//! flat checkpoint codec
//! (`agent_runtime_v2::checkpoint::value::{encode,decode}`, closed
//! `RetainedValue` scalars/one-level records only), which keeps every one
//! of its own known answers unchanged.
//!
//! [`encode`] never re-derives or re-validates the value's shape — it
//! trusts the already-admitted [`DecodedInteractionValue`] and only adds
//! the versioned envelope, so two calls on the same value produce
//! byte-identical output (see the golden determinism test in
//! [`super::tests`]). [`decode`] is independent replay: given the exact
//! schema the caller currently has bound (`schema`, `bound_schema_digest`),
//! it validates the envelope and the embedded revision without needing any
//! journal or prior in-memory state, then delegates the value payload
//! itself to `CompiledInteractionSchema::decode`, which performs the full
//! canonical, bounded, byte-exact admission `agent_interaction_schema`
//! already specifies.
//!
//! Rejected, each with a stable diagnostic (`SPX-Z212`), never silently
//! widened or truncated:
//! - An unrecognized `type_version` (only `1` exists today).
//! - A schema revision that disagrees with the caller's current binding
//!   (a stale schema binding: the checkpoint was written against a
//!   revision that is no longer the live one).
//! - A payload over [`MAX_CHECKPOINT_BYTES`], checked before any parsing.
//! - A malformed envelope or a value payload that does not itself decode.

use crate::agent_interaction_schema::decode::render_typed_value;
use crate::agent_interaction_schema::{CompiledInteractionSchema, DecodedInteractionValue, DOCUMENT_SCHEMA};
use crate::diagnostic::{quote_json, Diagnostic};

use super::refusal;

/// Schema identity of the versioned rich-value checkpoint envelope.
pub const CHECKPOINT_SCHEMA: &str = "semaprax.agent-lifecycle-typed-checkpoint.v1";
/// The one admitted envelope type version today.
pub const CHECKPOINT_TYPE_VERSION: u32 = 1;
/// The maximum admitted encoded checkpoint size, in bytes, checked before
/// any parsing work.
pub const MAX_CHECKPOINT_BYTES: usize = 131_072;

/// Encodes one already-admitted decoded value as a versioned checkpoint
/// document, including its terminal `\n`.
///
/// Deterministic: the same value always encodes to the same bytes, because
/// this function performs no re-derivation and reuses
/// `agent_interaction_schema`'s own canonical field rendering verbatim.
pub fn encode(value: &DecodedInteractionValue) -> Result<String, Diagnostic> {
    let rendered_value = render_typed_value(value.value());
    let body = format!(
        "{{\"schema\":{},\"type_version\":{CHECKPOINT_TYPE_VERSION},\"root_type_id\":{},\"schema_digest\":{},\"value\":{rendered_value}}}\n",
        quote_json(CHECKPOINT_SCHEMA),
        quote_json(value.root_type_id()),
        quote_json(value.schema_digest()),
    );
    if body.len() > MAX_CHECKPOINT_BYTES {
        return Err(refusal("SPX-Z212", "checkpoint.bytes"));
    }
    Ok(body)
}

/// Decodes one versioned checkpoint document against `schema`, requiring
/// its embedded schema revision to equal `bound_schema_digest` — the
/// revision the caller currently considers live.
pub fn decode(
    schema: &CompiledInteractionSchema,
    bound_schema_digest: &str,
    bytes: &[u8],
) -> Result<DecodedInteractionValue, Diagnostic> {
    if bytes.len() > MAX_CHECKPOINT_BYTES {
        return Err(refusal("SPX-Z212", "checkpoint.bytes"));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| refusal("SPX-Z212", "checkpoint.utf8"))?;

    let schema_prefix = format!("{{\"schema\":{},\"type_version\":", quote_json(CHECKPOINT_SCHEMA));
    let after_schema = text
        .strip_prefix(&schema_prefix)
        .ok_or_else(|| refusal("SPX-Z212", "checkpoint.schema"))?;

    let version_prefix = format!("{CHECKPOINT_TYPE_VERSION},\"root_type_id\":");
    let after_version = after_schema
        .strip_prefix(&version_prefix)
        .ok_or_else(|| refusal("SPX-Z212", "checkpoint.unknown_type_version"))?;

    let identity_prefix = format!(
        "{},\"schema_digest\":{},\"value\":",
        quote_json(schema.schema().root_type_id()),
        quote_json(bound_schema_digest),
    );
    let after_identity = after_version
        .strip_prefix(&identity_prefix)
        .ok_or_else(|| refusal("SPX-Z212", "checkpoint.stale_schema_binding"))?;

    let value_len = object_span(after_identity).ok_or_else(|| refusal("SPX-Z212", "checkpoint.malformed"))?;
    let (value_json, rest) = after_identity.split_at(value_len);
    if rest != "}\n" {
        return Err(refusal("SPX-Z212", "checkpoint.malformed"));
    }

    let inner_document = format!(
        "{{\"schema\":{},\"root_type_id\":{},\"schema_digest\":{},\"value\":{value_json}}}\n",
        quote_json(DOCUMENT_SCHEMA),
        quote_json(schema.schema().root_type_id()),
        quote_json(bound_schema_digest),
    );
    schema
        .decode(inner_document.as_bytes())
        .map_err(|_| refusal("SPX-Z212", "checkpoint.value"))
}

/// Returns the exact byte length of the leading well-formed JSON object or
/// array in `text` (matching brace/bracket depth, skipping quoted-string
/// content and escapes), or `None` if `text` does not begin with one or it
/// is truncated. Used only to locate the embedded value payload's span
/// without re-serializing it — the payload is forwarded to
/// `CompiledInteractionSchema::decode` byte for byte, exactly as written by
/// [`encode`], so this scanner never needs to reproduce canonical
/// formatting itself.
fn object_span(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    if !matches!(bytes.first(), Some(b'{') | Some(b'[')) {
        return None;
    }
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape = false;
    for (index, &byte) in bytes.iter().enumerate() {
        if in_string {
            if escape {
                escape = false;
            } else if byte == b'\\' {
                escape = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => depth += 1,
            b'}' | b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index + 1);
                }
            }
            _ => {}
        }
    }
    None
}
