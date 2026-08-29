//! Unpublished Native Rust Interoperability v1 A+B implementation.

#![forbid(unsafe_code)]
#![allow(clippy::result_large_err)]
#![allow(
    clippy::write_with_newline,
    reason = "generated source writers bind their terminal newline in the frozen literal"
)]

#[cfg(test)]
pub(crate) use semaprax::format;
#[cfg(test)]
pub(crate) use semaprax::parse;
pub(crate) use semaprax::{ast, cleanup, cleanup_plan, diagnostic, hir};
#[path = "../../../src/private_capacity_contract.rs"]
pub(crate) mod private_capacity_contract;
#[allow(dead_code, clippy::all)]
#[path = "../../../src/format.rs"]
pub(crate) mod private_format;
use semaprax_native_rust_interop_platform as platform;
use std::path::Path;

mod public_sdk;

pub use public_sdk::{
    build_native_rust_owned_data_sdk, build_native_rust_sdk, build_project_native_rust_sdk,
    NativeRustOwnedDataSdkBundle, NativeRustSdkBundle, NativeRustSdkOptions,
    ProjectNativeRustSdkBundle, NATIVE_RUST_OWNED_DATA_SDK_SCHEMA, PROJECT_NATIVE_RUST_SDK_SCHEMA,
    PROJECT_NATIVE_RUST_SUBJECT_SCHEMA,
};

pub(crate) mod workspace {
    use super::*;

    pub(crate) struct AuthenticatedDirectory(platform::HeldDirectory);

    pub(crate) enum CreatedDirectoryAuthenticationError {
        Disagreement(platform::HeldDirectory),
    }

    impl AuthenticatedDirectory {
        pub(crate) fn recheck(&self) -> Result<(), platform::Error> {
            platform::recheck_directory(&self.0)
        }

        pub(crate) fn same_directory_path(&self, path: &Path) -> bool {
            platform::same_directory_path(&self.0, path).unwrap_or(false)
        }

        pub(crate) fn held(&self) -> &platform::HeldDirectory {
            &self.0
        }
    }

    pub(crate) fn authenticate_directory_held(
        path: &Path,
    ) -> Result<AuthenticatedDirectory, platform::Error> {
        platform::hold_directory(path).map(AuthenticatedDirectory)
    }

    pub(crate) fn authenticate_created_directory(
        path: &Path,
        held: platform::HeldDirectory,
    ) -> Result<AuthenticatedDirectory, CreatedDirectoryAuthenticationError> {
        match platform::same_directory_path(&held, path) {
            Ok(true) => Ok(AuthenticatedDirectory(held)),
            Ok(false) | Err(_) => Err(CreatedDirectoryAuthenticationError::Disagreement(held)),
        }
    }
}

pub(crate) mod bounded_output {
    use std::cell::{Cell, RefCell};
    use std::fmt;
    use std::rc::Rc;

    struct Budget {
        initial: usize,
        remaining: Cell<usize>,
        overflowed: Cell<bool>,
    }

    thread_local! {
        static ACTIVE: RefCell<Option<Rc<Budget>>> = const { RefCell::new(None) };
    }

    pub(crate) fn with_limit<T>(limit: usize, operation: impl FnOnce() -> T) -> (T, bool) {
        let (value, overflowed, _) = with_limit_usage(limit, operation);
        (value, overflowed)
    }

    pub(crate) fn with_limit_usage<T>(
        limit: usize,
        operation: impl FnOnce() -> T,
    ) -> (T, bool, usize) {
        struct Restore {
            previous: Option<Rc<Budget>>,
            current: Rc<Budget>,
        }
        impl Drop for Restore {
            fn drop(&mut self) {
                let consumed = self
                    .current
                    .initial
                    .saturating_sub(self.current.remaining.get());
                let previous = self.previous.take();
                ACTIVE.with(|active| active.replace(previous.clone()));
                if let Some(parent) = previous {
                    let remaining = parent.remaining.get();
                    if consumed > remaining {
                        parent.overflowed.set(true);
                    } else {
                        parent.remaining.set(remaining - consumed);
                    }
                }
            }
        }
        let parent = ACTIVE.with(|active| active.borrow().clone());
        let effective_limit = parent
            .as_ref()
            .map_or(limit, |budget| limit.min(budget.remaining.get()));
        let budget = Rc::new(Budget {
            initial: effective_limit,
            remaining: Cell::new(effective_limit),
            overflowed: Cell::new(false),
        });
        let previous = ACTIVE.with(|active| active.replace(Some(Rc::clone(&budget))));
        let restore = Restore {
            previous,
            current: Rc::clone(&budget),
        };
        let value = operation();
        let overflowed = budget.overflowed.get();
        let consumed = effective_limit.saturating_sub(budget.remaining.get());
        drop(restore);
        (value, overflowed, consumed)
    }

    pub(crate) fn reserve_active(length: usize) -> bool {
        ACTIVE.with(|active| {
            let active = active.borrow();
            let Some(budget) = active.as_ref() else {
                return true;
            };
            let remaining = budget.remaining.get();
            if length > remaining {
                budget.overflowed.set(true);
                return false;
            }
            budget.remaining.set(remaining - length);
            true
        })
    }

    pub(crate) fn remaining_active() -> Option<usize> {
        ACTIVE.with(|active| {
            active
                .borrow()
                .as_ref()
                .map(|budget| budget.remaining.get())
        })
    }

    pub(crate) fn release_active(length: usize) {
        ACTIVE.with(|active| {
            if let Some(budget) = active.borrow().as_ref() {
                budget
                    .remaining
                    .set(budget.remaining.get().saturating_add(length));
            }
        });
    }

    #[allow(dead_code)]
    pub(crate) struct CappedString(String);

    #[allow(dead_code)]
    impl CappedString {
        pub(crate) fn new() -> Self {
            Self(String::new())
        }

        pub(crate) fn into_string(self) -> String {
            self.0
        }
    }

    #[allow(dead_code)]
    impl fmt::Write for CappedString {
        fn write_str(&mut self, value: &str) -> fmt::Result {
            if reserve_active(value.len()) {
                self.0.push_str(value);
            }
            Ok(())
        }
    }
}

#[allow(
    dead_code,
    reason = "private A+B has no externally callable surface before evidence-gated public phase C"
)]
mod implementation;
