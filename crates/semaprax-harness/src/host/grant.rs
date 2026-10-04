//! Authority grant handed from the trust store (HP-02, `profile::trust`) to the
//! adapter host (HP-03). A descriptor's `permissions` are only requests; the
//! host launches nothing without a `Grant`, and a grant is valid only for the
//! exact descriptor, adapter entry and upstream executable digests it names.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grant {
    provider_id: String,
    descriptor_digest: String,
    entry_digest: Option<String>,
    upstream_digest: Option<String>,
    permissions: GrantedPermissions,
}

/// Permission classes actually granted (each a subset of what was requested).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GrantedPermissions {
    pub read: Vec<String>,
    pub write: Vec<String>,
    pub network: Vec<String>,
    pub process: Vec<String>,
    pub secrets: Vec<String>,
}

impl Grant {
    /// Issued only by `profile::trust` after checking the machine-local trust
    /// record against current digests. Other modules must not call this.
    pub(crate) fn issue(
        provider_id: String,
        descriptor_digest: String,
        entry_digest: Option<String>,
        upstream_digest: Option<String>,
        permissions: GrantedPermissions,
    ) -> Self {
        Self {
            provider_id,
            descriptor_digest,
            entry_digest,
            upstream_digest,
            permissions,
        }
    }

    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }
    pub fn descriptor_digest(&self) -> &str {
        &self.descriptor_digest
    }
    pub fn entry_digest(&self) -> Option<&str> {
        self.entry_digest.as_deref()
    }
    pub fn upstream_digest(&self) -> Option<&str> {
        self.upstream_digest.as_deref()
    }
    pub fn permissions(&self) -> &GrantedPermissions {
        &self.permissions
    }
}
