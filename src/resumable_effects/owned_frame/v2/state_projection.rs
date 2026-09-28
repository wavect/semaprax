//! Borrowed structural State to frozen ordinary identity bytes. No runtime owner.
use super::{validate_owned_wait_state_v8, CheckedOwnedAgentWaitBindingV8};
use crate::diagnostic::quote_json;
use crate::resumable_effects::owned_frame::OwnedFrameError as Error;
use serde_json::Value;
pub(crate) fn owned_wait_ordinary_state_digest_v8(
    binding: &CheckedOwnedAgentWaitBindingV8,
    state: &Value,
) -> Result<String, Error> {
    let bytes = ordinary_state_bytes(binding, state)?;
    Ok(crate::live_invocation::identity::digest(
        b"semaprax.source-state.v2\0",
        bytes.as_bytes(),
    ))
}
fn ordinary_state_bytes(
    binding: &CheckedOwnedAgentWaitBindingV8,
    state: &Value,
) -> Result<String, Error> {
    validate_owned_wait_state_v8(binding, state)?;
    let mut out = format!(
        "{{\"record\":{},\"fields\":[",
        quote_json(state["declaration"].as_str().ok_or(Error::Malformed)?)
    );
    for (i, field) in state["fields"]
        .as_array()
        .ok_or(Error::Malformed)?
        .iter()
        .enumerate()
    {
        if i != 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"field\":{},\"value\":",
            quote_json(field["identity"].as_str().ok_or(Error::Malformed)?)
        ));
        let value = &field["value"];
        match value["kind"].as_str().or_else(|| value["tag"].as_str()) {
            Some("bytes") => out.push_str(&format!(
                "{{\"bytes\":{}}}",
                quote_json(value["hex"].as_str().ok_or(Error::Malformed)?)
            )),
            Some("i64") => out.push_str(&quote_json(
                &value["value"].as_i64().ok_or(Error::Malformed)?.to_string(),
            )),
            _ => return Err(Error::Binding),
        }
        out.push('}');
    }
    out.push_str("]}");
    Ok(out)
}
#[cfg(test)]
mod tests;
