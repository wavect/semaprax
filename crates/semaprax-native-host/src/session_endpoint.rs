// Included inside desktop_api so actual authority and ledger fields stay private.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "fresh acquisition awaits checked source endpoint plan"
    )
)]
mod session_endpoint {
    //! Private actual fresh local Token producer; no source endpoint activation.
    //!
    //! Existing unsafe adoption cannot produce this carrier. Endpoint attachment
    //! remains unavailable until a concrete checked endpoint/factory plan exists.

    use std::cell::RefCell;
    use std::rc::Rc;
    use std::thread::{self, ThreadId};

    use super::{LedgerState, NativeHost};
    use crate::authority::{Authority, AuthorityError, Credential};
    use crate::host_ownership::session_endpoint::{
        FreshTokenAcquisition, TOKEN_LIFECYCLE, TOKEN_RESOURCE,
    };
    use crate::host_ownership::HostIdentity;
    use crate::host_ownership::{HostBoundaryRejection, HostResourceProvenance};

    #[derive(Debug)]
    pub(crate) enum FreshTokenError {
        Draining,
        WrongShape,
        ForeignHost,
        WrongThread,
        Busy,
        Ledger(HostBoundaryRejection),
        Credential(AuthorityError),
    }

    impl std::fmt::Display for FreshTokenError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Ledger(error) => write!(f, "fresh Token registry refusal: {error:?}"),
                Self::Credential(error) => write!(f, "fresh Token credential refusal: {error:?}"),
                other => write!(f, "fresh Token admission refusal: {other:?}"),
            }
        }
    }

    /// Keeps actual registry backing first and the physical credential pin alive.
    /// This is deliberately neither Clone nor a source endpoint constructor.
    #[must_use = "retain or retire the actual registered Token owner"]
    pub(crate) struct NativeFreshTokenOwner {
        ledger: Rc<RefCell<LedgerState>>,
        acquisition: Option<FreshTokenAcquisition>,
        credential: Option<Credential>,
        thread: ThreadId,
    }

    fn acquire(
        authority: &Authority,
        ledger: Rc<RefCell<LedgerState>>,
        provenance: HostResourceProvenance,
    ) -> Result<NativeFreshTokenOwner, FreshTokenError> {
        #[cfg(test)]
        {
            acquire_impl(authority, ledger, provenance, false)
        }
        #[cfg(not(test))]
        {
            acquire_impl(authority, ledger, provenance)
        }
    }

    fn acquire_impl(
        authority: &Authority,
        ledger: Rc<RefCell<LedgerState>>,
        provenance: HostResourceProvenance,
        #[cfg(test)] fail_credential: bool,
    ) -> Result<NativeFreshTokenOwner, FreshTokenError> {
        let acquisition = ledger
            .try_borrow_mut()
            .map_err(|_| FreshTokenError::Busy)?
            .registry
            .acquire_fresh_token(provenance)
            .map_err(FreshTokenError::Ledger)?;
        // No foreign callback spans a registry borrow. Minting retains the actual
        // admitted module and authenticates these exact registered coordinates.
        #[cfg(test)]
        let minted = if fail_credential {
            Err(AuthorityError::InvalidBinding)
        } else {
            authority.mint_owner(
                TOKEN_RESOURCE.as_bytes(),
                TOKEN_LIFECYCLE.as_bytes(),
                acquisition.slot(),
                acquisition.generation(),
            )
        };
        #[cfg(not(test))]
        let minted = authority.mint_owner(
            TOKEN_RESOURCE.as_bytes(),
            TOKEN_LIFECYCLE.as_bytes(),
            acquisition.slot(),
            acquisition.generation(),
        );
        let credential = match minted {
            Ok(credential) => credential,
            Err(error) => {
                acquisition
                    .rollback(
                        &mut ledger
                            .try_borrow_mut()
                            .map_err(|_| FreshTokenError::Busy)?
                            .registry,
                    )
                    .map_err(FreshTokenError::Ledger)?;
                return Err(FreshTokenError::Credential(error));
            }
        };
        Ok(NativeFreshTokenOwner {
            ledger,
            acquisition: Some(acquisition),
            credential: Some(credential),
            thread: thread::current().id(),
        })
    }

    impl NativeFreshTokenOwner {
        fn validate(
            &self,
            authority: &Authority,
            ledger: &Rc<RefCell<LedgerState>>,
        ) -> Result<(), FreshTokenError> {
            if !Rc::ptr_eq(&self.ledger, ledger) {
                return Err(FreshTokenError::ForeignHost);
            }
            if self.thread != thread::current().id() {
                return Err(FreshTokenError::WrongThread);
            }
            let acquisition = self
                .acquisition
                .as_ref()
                .ok_or(FreshTokenError::WrongShape)?;
            acquisition
                .validate(
                    &ledger
                        .try_borrow()
                        .map_err(|_| FreshTokenError::Busy)?
                        .registry,
                )
                .map_err(FreshTokenError::Ledger)?;
            authority
                .authenticate_owner(
                    self.credential
                        .as_ref()
                        .ok_or(FreshTokenError::WrongShape)?,
                    TOKEN_RESOURCE.as_bytes(),
                    TOKEN_LIFECYCLE.as_bytes(),
                    acquisition.slot(),
                    acquisition.generation(),
                )
                .map_err(FreshTokenError::Credential)
        }

        pub(crate) fn retire(&mut self) -> Result<(), FreshTokenError> {
            if self.thread != thread::current().id() {
                return Err(FreshTokenError::WrongThread);
            }
            let acquisition = self
                .acquisition
                .as_ref()
                .ok_or(FreshTokenError::WrongShape)?;
            acquisition
                .retire(
                    &mut self
                        .ledger
                        .try_borrow_mut()
                        .map_err(|_| FreshTokenError::Busy)?
                        .registry,
                )
                .map_err(FreshTokenError::Ledger)?;
            self.acquisition = None;
            // The exact cell is gone and its generation is dead before this final
            // physical module pin is released. No imported finalizer is invoked.
            self.credential = None;
            Ok(())
        }
    }

    impl Drop for NativeFreshTokenOwner {
        fn drop(&mut self) {
            if self.acquisition.is_some() && self.retire().is_err() {
                // Foreign/busy/poisoned Drop cannot perform semantic retirement.
                // Keep unresolved actual backing and its physical code pin alive;
                // never revive or retry it from a later reconstructed certificate.
                std::mem::forget(Rc::clone(&self.ledger));
                if let Some(credential) = self.credential.take() {
                    std::mem::forget(credential);
                }
            }
        }
    }

    #[cfg(test)]
    fn acquire_with_credential_failure(
        authority: &Authority,
        ledger: Rc<RefCell<LedgerState>>,
        provenance: HostResourceProvenance,
    ) -> Result<NativeFreshTokenOwner, FreshTokenError> {
        acquire_impl(authority, ledger, provenance, true)
    }

    #[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
    mod tests {
        include!("session_endpoint/tests.rs");
    }

    impl NativeHost {
        fn fresh_token_provenance(&self) -> Result<HostResourceProvenance, FreshTokenError> {
            use crate::host_ownership::session_endpoint::{TOKEN_LIFECYCLE, TOKEN_RESOURCE};
            if self.draining {
                return Err(FreshTokenError::Draining);
            }
            let (resource, lifecycle, _) = self
                .descriptor
                .owned_parameter(0)
                .ok_or(FreshTokenError::WrongShape)?;
            if resource != TOKEN_RESOURCE || lifecycle != TOKEN_LIFECYCLE {
                return Err(FreshTokenError::WrongShape);
            }
            HostResourceProvenance::try_new(
                self.module_identity.clone(),
                self.adapter_identity.clone(),
                HostIdentity::try_new(resource.to_owned()).map_err(FreshTokenError::Ledger)?,
                HostIdentity::try_new(lifecycle.to_owned()).map_err(FreshTokenError::Ledger)?,
                self.host_thread_identity,
            )
            .map_err(FreshTokenError::Ledger)
        }

        pub(crate) fn acquire_fresh_local_token(
            &mut self,
        ) -> Result<NativeFreshTokenOwner, FreshTokenError> {
            acquire(
                &self.authority,
                Rc::clone(&self.ledger),
                self.fresh_token_provenance()?,
            )
        }

        pub(crate) fn validate_fresh_local_token(
            &self,
            owner: &NativeFreshTokenOwner,
        ) -> Result<(), FreshTokenError> {
            owner.validate(&self.authority, &self.ledger)
        }

        #[cfg(test)]
        pub(crate) fn test_fresh_credential_failure(
            &mut self,
        ) -> Result<NativeFreshTokenOwner, FreshTokenError> {
            acquire_with_credential_failure(
                &self.authority,
                Rc::clone(&self.ledger),
                self.fresh_token_provenance()?,
            )
        }
    }
}
