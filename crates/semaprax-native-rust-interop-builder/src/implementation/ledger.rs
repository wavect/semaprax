//! The builder byte ledger: bounded diagnostics, debits, the temporary
//! reservation guard, and the test target binding.

use super::*;

pub(super) fn b106() -> Diagnostic {
    Diagnostic::io(
        "SPX-B106",
        "Native Rust Interop specification is not canonical semaprax.native-rust-interop-spec.v1 JSON",
    )
}

pub(super) fn b107(reason: &'static str) -> Diagnostic {
    Diagnostic::io(
        "SPX-B107",
        format!("Native Rust Interop declaration set is unsupported: {reason}"),
    )
}

pub(super) fn b108() -> Diagnostic {
    Diagnostic::io(
        "SPX-B108",
        "Native Rust Interop descriptor disagrees with validated source and HIR",
    )
}

pub(super) fn b109(field: &'static str, maximum: usize) -> Diagnostic {
    Diagnostic::io(
        "SPX-B109",
        format!("Native Rust Interop {field} exceeds {maximum}"),
    )
}

pub(super) fn b110() -> Diagnostic {
    Diagnostic::io(
        "SPX-B110",
        "Native Rust Interop target or toolchain is unsupported",
    )
}

pub(super) fn b111() -> Diagnostic {
    Diagnostic::io(
        "SPX-B111",
        "Native Rust Interop generated artifact replay failed",
    )
}

pub(super) fn debit(bytes: usize) -> Result<(), Diagnostic> {
    if crate::bounded_output::reserve_active(bytes) {
        Ok(())
    } else {
        Err(b109("max_builder_bytes", MAX_BUILDER_BYTES))
    }
}

pub(super) fn reserve_temporary_exact(maximum: usize) -> Result<TemporaryBudget, Diagnostic> {
    let remaining = crate::bounded_output::remaining_active().unwrap_or(MAX_BUILDER_BYTES);
    if maximum > remaining {
        return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
    }
    debit(maximum)?;
    Ok(TemporaryBudget { reserved: maximum })
}

pub(super) struct TemporaryBudget {
    pub(super) reserved: usize,
}

impl TemporaryBudget {
    pub(super) fn maximum(&self) -> usize {
        self.reserved
    }

    pub(super) fn retain(mut self, actual: usize) -> Result<(), Diagnostic> {
        if actual > self.reserved {
            return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
        }
        crate::bounded_output::release_active(self.reserved - actual);
        self.reserved = 0;
        Ok(())
    }

    pub(super) fn shrink_held(&mut self, actual: usize) -> Result<(), Diagnostic> {
        if actual > self.reserved {
            return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
        }
        crate::bounded_output::release_active(self.reserved - actual);
        self.reserved = actual;
        Ok(())
    }

    pub(super) fn check(&self, actual: usize) -> Result<(), Diagnostic> {
        if actual > self.reserved {
            Err(b109("max_builder_bytes", MAX_BUILDER_BYTES))
        } else {
            Ok(())
        }
    }
}

impl Drop for TemporaryBudget {
    fn drop(&mut self) {
        crate::bounded_output::release_active(self.reserved);
    }
}

fn debit_source(source: &str) -> Result<(), Diagnostic> {
    debit(source.len())
}

#[cfg(test)]
thread_local! {
    pub(super) static TEST_TARGET_OVERRIDE: std::cell::RefCell<Option<Target>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(super) fn with_test_target<T>(target: Target, run: impl FnOnce() -> T) -> T {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            TEST_TARGET_OVERRIDE.with(|slot| *slot.borrow_mut() = None);
        }
    }
    TEST_TARGET_OVERRIDE.with(|slot| {
        assert!(slot.borrow().is_none(), "test target override nested");
        *slot.borrow_mut() = Some(target);
    });
    let reset = Reset;
    let result = run();
    drop(reset);
    result
}

pub(super) fn current_target() -> Option<Target> {
    #[cfg(test)]
    if let Some(target) = TEST_TARGET_OVERRIDE.with(|slot| slot.borrow().clone()) {
        return Some(target);
    }
    target_from_native_host(crate::platform::current_native_host_target())
}

pub(super) fn target_from_native_host(
    host: Option<crate::platform::NativeHostTarget>,
) -> Option<Target> {
    let triple = host?.triple();
    Some(Target {
        triple: triple.to_owned(),
        pointer_width: 64,
        endian: "little".to_owned(),
        panic_strategy: "unwind".to_owned(),
        thread_policy: "same_thread".to_owned(),
    })
}
