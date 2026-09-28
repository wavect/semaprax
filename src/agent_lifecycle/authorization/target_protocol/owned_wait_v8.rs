//! Shared frozen grant preimage; inert digest computation grants no dispatch.
use super::*;

pub(super) fn grant_id(
    authorization_binding: &str,
    seal: &[u8],
    invocation_root: &str,
    execution_binding: Option<&str>,
    turn: u64,
    operation: &TargetOperation,
    argument_digest: &str,
) -> String {
    let mut bytes = Vec::new();
    frame(&mut bytes, authorization_binding.as_bytes());
    frame(&mut bytes, seal);
    frame(&mut bytes, invocation_root.as_bytes());
    // The legacy route intentionally has no extra frame, preserving its
    // established grant and evidence bytes. Explicit target parity binds
    // a domain-separated execution digest before an opaque grant exists.
    if let Some(binding) = execution_binding {
        frame(&mut bytes, binding.as_bytes());
    }
    frame(&mut bytes, &turn.to_be_bytes());
    operation.canonical(&mut bytes);
    frame(&mut bytes, argument_digest.as_bytes());
    digest(GRANT_DOMAIN, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owned_frame_v8_borrowed_grant_preserves_frozen_framing_known_answers() {
        let binding = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
        let invocation = "sha256:2222222222222222222222222222222222222222222222222222222222222222";
        let execution = "sha256:3333333333333333333333333333333333333333333333333333333333333333";
        let argument = "sha256:4444444444444444444444444444444444444444444444444444444444444444";
        let operation =
            TargetOperation::new("tool.op", "tool.effect", "tool.arg", "tool.result").unwrap();
        assert_eq!(
            grant_id(binding, b"\0AZ", invocation, None, 7, &operation, argument),
            "sha256:0fc13720278ac7ec72b63e07d13858c9b44dc05f17be14702de6584ee923aa63"
        );
        assert_eq!(
            grant_id(
                binding,
                b"\0AZ",
                invocation,
                Some(execution),
                7,
                &operation,
                argument
            ),
            "sha256:40474b78d5f660bf616888ca9afbf59a3995cace90176918dfb5ad63d77a4a5e"
        );
        assert_ne!(
            grant_id(
                binding,
                b"\0AZ",
                execution,
                Some(invocation),
                7,
                &operation,
                argument
            ),
            "sha256:40474b78d5f660bf616888ca9afbf59a3995cace90176918dfb5ad63d77a4a5e"
        );
    }
}
