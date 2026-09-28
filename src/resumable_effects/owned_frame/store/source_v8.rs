//! Profile-specific physical registration; no codec, restoration or model authority.
use super::*;

const ID_DOMAIN: &[u8] = b"semaprax.live-invocation.source-id.v8\0";
const GENERATION_DOMAIN: &[u8] = b"semaprax.source-agent-owned-wait.generation.v1\0";
const NAME_DOMAIN: &[u8] = b"semaprax.source-agent-owned-wait.journal-name.v1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceOwnedWaitLimitsV8 {
    pub(crate) max_steps_per_stage: usize,
    pub(crate) max_total_steps: u64,
    pub(crate) max_stages: usize,
    pub(crate) max_attempts: usize,
    pub(crate) response_limit: usize,
}
impl SourceOwnedWaitLimitsV8 {
    pub(crate) fn json(&self) -> Result<Value, Error> {
        if self.max_steps_per_stage == 0
            || self.max_steps_per_stage > 1_000_000
            || self.max_total_steps < self.max_steps_per_stage as u64
            || self.max_stages == 0
            || self.max_stages > 65536
            || self.max_attempts == 0
            || self.max_attempts > 65536
            || self.response_limit == 0
            || self.response_limit > 8 * 1024 * 1024
        {
            return Err(Error::Capacity);
        }
        Ok(json!({"max_steps_per_stage":self.max_steps_per_stage,
            "max_total_steps":self.max_total_steps,"max_stages":self.max_stages,
            "max_attempts":self.max_attempts,"response_limit":self.response_limit,
            "journal_bytes":16777216,"journal_rows":65536}))
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FreshSourceOwnedWaitFactsV8 {
    pub(crate) scope: SourceCheckpointScope,
    pub(crate) execution: String,
    pub(crate) binding: String,
    pub(crate) limits: SourceOwnedWaitLimitsV8,
    pub(crate) directory_identity: (u64, u64),
}
impl FreshSourceOwnedWaitFactsV8 {
    fn validate(&self) -> Result<(), Error> {
        codec::scope(&self.scope)?;
        self.limits.json()?;
        if !codec::is_digest(self.scope.program_root())
            || !codec::is_digest(&self.execution)
            || !codec::is_digest(&self.binding)
            || self.scope.invocation_id()
                != codec::fact_digest(
                    ID_DOMAIN,
                    &json!({"execution":self.execution,"owned_wait_binding":self.binding}),
                )
        {
            return Err(Error::Binding);
        }
        Ok(())
    }
    fn name(&self) -> String {
        let hash = codec::digest(NAME_DOMAIN, self.scope.invocation_id().as_bytes());
        format!("{}.source-owned-wait.jsonl", &hash[7..])
    }
    fn scope_json(&self) -> Value {
        json!({"program_root":self.scope.program_root(),"invocation_id":self.scope.invocation_id(),"policy_epoch":self.scope.policy_epoch()})
    }
    fn generation(&self, identity: OwnedFrameStoreIdentity) -> Result<String, Error> {
        Ok(codec::fact_digest(
            GENERATION_DOMAIN,
            &json!({
            "scope":self.scope_json(),"execution":self.execution,
            "binding":self.binding,"store_identity":identity.json(),"limits":self.limits.json()?}),
        ))
    }
}
/// Explicit physical-host assertion, not a MAC or a policy-derived epoch.
pub(crate) struct ExplicitStoreRegistrationGrant {
    creator: u32,
}
impl ExplicitStoreRegistrationGrant {
    pub(crate) fn for_trusted_host(protected_history_available: bool) -> Result<Self, Error> {
        if !protected_history_available {
            return Err(Error::Policy);
        }
        Ok(Self {
            creator: std::process::id(),
        })
    }
    fn check(&self) -> Result<(), Error> {
        if self.creator != std::process::id() {
            return Err(Error::Policy);
        }
        Ok(())
    }
}
pub(crate) struct FreshSourceOwnedWaitGrantV8 {
    directory: File,
    expected: FreshSourceOwnedWaitFactsV8,
    grant: ExplicitStoreRegistrationGrant,
}
/// Complete inert registration, retained by the host outside the journal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceOwnedWaitStoreRegistrationV8 {
    expected: FreshSourceOwnedWaitFactsV8,
    identity: OwnedFrameStoreIdentity,
    generation: String,
}
impl SourceOwnedWaitStoreRegistrationV8 {
    pub(crate) fn expected_facts(&self) -> &FreshSourceOwnedWaitFactsV8 {
        &self.expected
    }
    pub(crate) fn generation(&self) -> &str {
        &self.generation
    }
    pub(crate) fn identity(&self) -> OwnedFrameStoreIdentity {
        self.identity
    }
    pub(crate) fn acknowledge_retained_by_trusted_host(
        &self,
        retained: bool,
    ) -> Result<RetainedSourceOwnedWaitRegistrationGrantV8, Error> {
        if !retained {
            return Err(Error::Policy);
        }
        Ok(RetainedSourceOwnedWaitRegistrationGrantV8 {
            registration: self.clone(),
            creator: std::process::id(),
        })
    }
}
pub(crate) struct RetainedSourceOwnedWaitRegistrationGrantV8 {
    registration: SourceOwnedWaitStoreRegistrationV8,
    creator: u32,
}
/// NonClone held lease. No conversion to a v1 lease or public raw File.
pub(crate) struct SourceOwnedWaitLeaseV8 {
    inner: RegisteredJournalLease,
    registration: SourceOwnedWaitStoreRegistrationV8,
    start_authorized: bool,
}
impl SourceOwnedWaitLeaseV8 {
    pub(crate) fn validate_registration(
        &self,
        registration: &SourceOwnedWaitStoreRegistrationV8,
    ) -> Result<(), Error> {
        self.inner.validate_process()?;
        if &self.registration != registration {
            return Err(Error::Binding);
        }
        self.validate(registration.expected_facts(), registration.generation())
    }

    pub(crate) fn validate(
        &self,
        expected: &FreshSourceOwnedWaitFactsV8,
        generation: &str,
    ) -> Result<(), Error> {
        // Process binding is checked before facts, filesystem or caller work.
        self.inner.validate_process()?;
        if &self.registration.expected != expected
            || self.registration.generation != generation
            || self.registration.identity != self.inner.identity()
        {
            return Err(Error::Binding);
        }
        self.inner
            .validate_profile(StoreProfile::SourceOwnedWaitV8)?;
        self.inner.validate_scope(&expected.scope)
    }
    pub(crate) fn authorize_fresh_start(
        &mut self,
        retained: RetainedSourceOwnedWaitRegistrationGrantV8,
    ) -> Result<(), Error> {
        self.inner.validate_process()?;
        if retained.creator != std::process::id() {
            return Err(Error::Policy);
        }
        if self.start_authorized || retained.registration != self.registration {
            return Err(Error::Binding);
        }
        self.inner
            .validate_profile(StoreProfile::SourceOwnedWaitV8)?;
        if !self.inner.read()?.is_empty() {
            return Err(Error::Binding);
        }
        self.start_authorized = true;
        Ok(())
    }
    pub(crate) fn read(&mut self) -> Result<Vec<u8>, Error> {
        self.inner
            .validate_profile(StoreProfile::SourceOwnedWaitV8)?;
        self.inner
            .validate_scope(&self.registration.expected.scope)?;
        self.inner.read()
    }
    /// Read-only admission before a physical append is attempted. This cannot
    /// authorize a fresh lease or replace its independently retained grant.
    pub(crate) fn validate_append_authorized(
        &self,
        registration: &SourceOwnedWaitStoreRegistrationV8,
    ) -> Result<(), Error> {
        self.validate_registration(registration)?;
        if !self.start_authorized {
            return Err(Error::Policy);
        }
        Ok(())
    }
    pub(crate) fn append(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.inner
            .validate_profile(StoreProfile::SourceOwnedWaitV8)?;
        self.inner
            .validate_scope(&self.registration.expected.scope)?;
        if !self.start_authorized {
            return Err(Error::Policy);
        }
        self.inner.append(bytes)
    }
}
pub(crate) fn prepare_fresh_source_owned_wait_v8(
    directory: File,
    expected: FreshSourceOwnedWaitFactsV8,
    grant: ExplicitStoreRegistrationGrant,
) -> Result<FreshSourceOwnedWaitGrantV8, Error> {
    grant.check()?;
    expected.validate()?;
    validate_directory_identity(&directory, expected.directory_identity)?;
    Ok(FreshSourceOwnedWaitGrantV8 {
        directory,
        expected,
        grant,
    })
}
pub(crate) fn fresh_source_owned_wait_v8(
    prepared: FreshSourceOwnedWaitGrantV8,
) -> Result<(SourceOwnedWaitStoreRegistrationV8, SourceOwnedWaitLeaseV8), Error> {
    prepared.grant.check()?;
    prepared.expected.validate()?;
    let mut inner = RegisteredJournalLease::fresh_profile(
        prepared.directory,
        prepared.expected.directory_identity,
        &prepared.expected.scope,
        StoreProfile::SourceOwnedWaitV8,
        prepared.expected.name(),
    )?;
    inner.validate_profile(StoreProfile::SourceOwnedWaitV8)?;
    let identity = inner.identity();
    let registration = SourceOwnedWaitStoreRegistrationV8 {
        generation: prepared.expected.generation(identity)?,
        expected: prepared.expected,
        identity,
    };
    if !inner.read()?.is_empty() {
        return Err(Error::Binding);
    }
    Ok((
        registration.clone(),
        SourceOwnedWaitLeaseV8 {
            inner,
            registration,
            start_authorized: false,
        },
    ))
}
pub(crate) fn recover_source_owned_wait_v8(
    directory: File,
    registration: &SourceOwnedWaitStoreRegistrationV8,
    expected: FreshSourceOwnedWaitFactsV8,
    grant: ExplicitStoreRegistrationGrant,
) -> Result<SourceOwnedWaitLeaseV8, Error> {
    grant.check()?;
    expected.validate()?;
    // Complete retained file pins/generation are checked before opening.
    if registration.expected != expected
        || registration.generation != expected.generation(registration.identity)?
        || (
            registration.identity.directory_device,
            registration.identity.directory_inode,
        ) != expected.directory_identity
    {
        return Err(Error::Binding);
    }
    let old_registration = OwnedFrameStoreRegistration::grant_for_trusted_host(
        registration.identity,
        &expected.scope,
        true,
    )?;
    let inner = RegisteredJournalLease::recover_profile(
        directory,
        old_registration,
        &expected.scope,
        StoreProfile::SourceOwnedWaitV8,
        expected.name(),
    )?;
    inner.validate_profile(StoreProfile::SourceOwnedWaitV8)?;
    // Recovery authorizes physical history append only. The v8 codec/fold must
    // authenticate and validate legal phases before evaluator/owner operations.
    Ok(SourceOwnedWaitLeaseV8 {
        inner,
        registration: registration.clone(),
        start_authorized: true,
    })
}
#[cfg(unix)]
fn validate_directory_identity(directory: &File, expected: (u64, u64)) -> Result<(), Error> {
    unix::validate_directory_identity(directory, expected)
}
#[cfg(not(unix))]
fn validate_directory_identity(_: &File, _: (u64, u64)) -> Result<(), Error> {
    Err(Error::UnsupportedStore)
}

#[cfg(all(test, unix))]
mod tests;

#[cfg(all(test, unix))]
impl SourceOwnedWaitLeaseV8 {
    pub(crate) fn test_fail_before_write(&mut self, number: usize) {
        self.inner
            .fail_append_stage(number, super::unix::AppendFaultStage::BeforeWrite);
    }
    pub(crate) fn test_fail_after_write(&mut self, number: usize) {
        self.inner
            .fail_append_stage(number, super::unix::AppendFaultStage::AfterWriteBeforeSync);
    }
    pub(crate) fn test_fail_before_sync(&mut self, number: usize) {
        self.inner
            .fail_append_stage(number, super::unix::AppendFaultStage::BeforeSync);
    }
    pub(crate) fn test_fail_after_sync(&mut self, number: usize) {
        self.inner
            .fail_append_stage(number, super::unix::AppendFaultStage::AfterSync);
    }
    pub(crate) fn test_mark_foreign(&mut self) {
        self.inner.mark_foreign_process_for_test();
    }
}
