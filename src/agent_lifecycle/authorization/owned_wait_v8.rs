//! Pure frozen authorization commitment over borrowed canonical State/seal.
//! Computing this digest mints no Authorized, grant, ACK or dispatch authority.
use super::*;

pub(in crate::agent_lifecycle) fn binding_from_canonical_state(
    policy_digest: &str,
    state_canonical: &str,
    proposal_canonical: &str,
    grant_case: &hir::DeclarationId,
    seal: &[u8],
) -> String {
    let mut hash = Sha256::new();
    hash.update(BINDING_DOMAIN);
    hash.update(policy_digest.as_bytes());
    hash.update([0]);
    hash.update(state_canonical.as_bytes());
    hash.update([0]);
    hash.update(proposal_canonical.as_bytes());
    hash.update([0]);
    hash.update(grant_case.as_str().as_bytes());
    hash.update([0]);
    hash.update(seal);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owned_frame_v8_borrowed_authorization_preserves_frozen_known_answer() {
        let canonical = r#"{"bytes":"0041"}"#;
        let case = hir::DeclarationId::new("case");
        let expected = "sha256:1062a72c95bc86a799ecbaa4437fa4aaa747dcd987d3dd105f1b40a76c48b8c4";
        assert_eq!(
            binding_from_canonical_state("policy", canonical, "proposal\n", &case, b"\0AZ"),
            expected
        );
        assert_eq!(
            super::super::binding(
                "policy",
                &RetainedValue::Bytes(vec![0, 65]),
                "proposal\n",
                &case,
                b"\0AZ"
            ),
            expected
        );
        assert_ne!(
            binding_from_canonical_state("policy", canonical, "proposal", &case, b"\0AZ"),
            expected
        );
    }
}
