use super::{private, Diagnostic};

/// Crate-private, read-only Runtime v1 profile facts for compatibility checks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeV1CompatibilityProfile {
    agent_id: String,
    profile_digest: String,
    max_provider_response_bytes: u64,
}

impl RuntimeV1CompatibilityProfile {
    pub(crate) fn agent_id(&self) -> &str {
        &self.agent_id
    }

    pub(crate) fn profile_digest(&self) -> &str {
        &self.profile_digest
    }

    pub(crate) fn max_provider_response_bytes(&self) -> u64 {
        self.max_provider_response_bytes
    }
}

/// Parses the frozen Runtime v1 profile and projects only adapter compatibility facts.
pub(crate) fn proposal_compatibility_profile(
    profile_source: &str,
) -> Result<RuntimeV1CompatibilityProfile, Diagnostic> {
    let profile = private::parse_profile(profile_source)?;
    Ok(RuntimeV1CompatibilityProfile {
        agent_id: profile.agent_id,
        profile_digest: profile.digest,
        max_provider_response_bytes: profile.limits.max_provider_response_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatibility_projection_retains_exact_runtime_profile_facts() {
        let profile = proposal_compatibility_profile(&super::super::tests::fixture_profile())
            .expect("fixture profile must remain admitted");
        assert_eq!(profile.agent_id(), "fixture.agent");
        assert!(profile.profile_digest().starts_with("sha256:"));
        assert_eq!(profile.max_provider_response_bytes(), 4096);
    }
}
