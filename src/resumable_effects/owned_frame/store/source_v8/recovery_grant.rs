//! Restart-only append authority for one authenticated first-turn tail.
use super::*;

pub(crate) fn decode_recovered_authorization_state_v8(
    binding: &crate::resumable_effects::owned_frame::v2::CheckedOwnedAgentWaitBindingV8,
    state: &Value,
) -> Result<crate::interpreter::resumable::owned_frame::OwnedFrameInput, Error> {
    crate::resumable_effects::owned_frame::v2::validate_owned_wait_state_v8(binding, state)
        .map_err(|_| Error::Binding)?;
    let fields = state["fields"]
        .as_array()
        .ok_or(Error::Malformed)?
        .iter()
        .map(|field| {
            let identity = field["identity"].as_str().ok_or(Error::Malformed)?;
            let value = &field["value"];
            let value = if value.get("kind").and_then(Value::as_str) == Some("bytes") {
                super::super::super::codec::keys(value, &["kind", "hex"])?;
                crate::interpreter::resumable::owned_frame::OwnedFrameInputValue::Bytes(
                    super::super::super::codec::unhex(
                        super::super::super::codec::text(&value["hex"], 2048)?,
                        1024,
                    )?,
                )
            } else {
                crate::interpreter::resumable::owned_frame::OwnedFrameInputValue::Scalar(
                    super::super::super::codec::decode_scalar(value)?,
                )
            };
            Ok(
                crate::interpreter::resumable::owned_frame::OwnedFrameInputField {
                    identity: crate::hir::DeclarationId::new(identity),
                    value,
                },
            )
        })
        .collect::<Result<Vec<_>, Error>>()?;
    Ok(
        crate::interpreter::resumable::owned_frame::OwnedFrameInput {
            declaration: crate::hir::DeclarationId::new(
                state["declaration"].as_str().ok_or(Error::Malformed)?,
            ),
            fields,
        },
    )
}

/// One-use authority to cross the recovered read-only boundary. Its pins are
/// supplied only after the source journal validates the complete exact tail.
pub(crate) struct RecoveredAuthorizationAppendGrantV8 {
    pub(super) registration: SourceOwnedWaitStoreRegistrationV8,
    pub(super) sequence: usize,
    pub(super) acknowledged_bytes: usize,
    pub(super) authentication: String,
    pub(super) document_digest: String,
    pub(super) creator: u32,
}

pub(crate) fn recovered_authorization_append_grant_v8(
    registration: &SourceOwnedWaitStoreRegistrationV8,
    sequence: usize,
    acknowledged_bytes: usize,
    authentication: &str,
    document_digest: &str,
    protected_history_available: bool,
) -> Result<RecoveredAuthorizationAppendGrantV8, Error> {
    if !protected_history_available {
        return Err(Error::Policy);
    }
    if sequence == 0
        || acknowledged_bytes == 0
        || authentication.len() != 64
        || !authentication.bytes().all(|byte| byte.is_ascii_hexdigit())
        || !document_digest.starts_with("sha256:")
        || document_digest.len() != 71
    {
        return Err(Error::Binding);
    }
    Ok(RecoveredAuthorizationAppendGrantV8 {
        registration: registration.clone(),
        sequence,
        acknowledged_bytes,
        authentication: authentication.to_ascii_lowercase(),
        document_digest: document_digest.to_owned(),
        creator: std::process::id(),
    })
}
