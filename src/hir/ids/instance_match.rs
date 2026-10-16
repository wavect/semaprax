//! Exact comparison without an encoded-argument or final identity buffer.
//!
//! Argument identity keys remain owned and are always produced left to right.
//! A mismatch never interrupts formatting or a later key producer. A physical
//! refusal rejects publication even if a truncated key could match retained text.
use std::fmt;
use std::mem::size_of;

pub(in crate::hir) struct Compare<'a> {
    pub(in crate::hir) remaining: &'a [u8],
    pub(in crate::hir) failed: bool,
    pub(in crate::hir) complete: bool,
}

// Aggregate initializer and named receiving writer; retained-ID method receiver;
// caller/constructor/as_bytes string references; header write result/expect receiver;
// header mutable receiver;
// admission bool, three final bool operands and their two conjunction results;
// final empty-slice method receiver. No Result<bool> or forwarding call is added.
pub(in crate::hir) const CONTROLS: usize = 2 * size_of::<Compare<'static>>()
    + size_of::<&super::FunctionInstanceId>()
    + 3 * size_of::<&str>()
    + 2 * size_of::<fmt::Result>()
    + size_of::<&mut Compare<'static>>()
    + 6 * size_of::<bool>()
    + size_of::<&[u8]>()
    + crate::bounded_output::CheckedCappedString::FIRST_FAILURE_CONTROLS;

// Each actual formatter write pays its comparison before forming it: prefix
// predicate; starts_with receiver and argument; indexed-slice receiver; byte
// length, range and additional as_bytes string receiver. The old write_str receiver/value/result and reserve predicate
// are corresponding surviving CappedString write boundaries, not new credit.
const WRITE_CONTROLS: usize = size_of::<bool>()
    + size_of::<&str>()
    + 3 * size_of::<&[u8]>()
    + size_of::<usize>()
    + size_of::<std::ops::RangeFrom<usize>>();

// The original key method receiver and String return are corresponding old
// roles. Only the new completion caller/parameter references and payment bool
// are added, before the completion argument is evaluated.
pub(in crate::hir) const KEY_CONTROLS: usize = 2 * size_of::<&mut bool>() + size_of::<bool>();

impl<'a> Compare<'a> {
    pub(in crate::hir) fn new(actual: &'a str) -> Self {
        Self {
            remaining: actual.as_bytes(),
            failed: false,
            complete: true,
        }
    }
}

impl fmt::Write for Compare<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if crate::bounded_output::reserve_active_required(WRITE_CONTROLS) {
            if self.remaining.starts_with(value.as_bytes()) {
                self.remaining = &self.remaining[value.len()..];
            } else {
                self.failed = true;
            }
        } else {
            self.failed = true;
        }
        // fmt::Error would skip later formatting and therefore key producers.
        Ok(())
    }
}

macro_rules! streamed_instance_matches {
    ($actual:expr, $template:expr, $arguments:expr) => {{
        if crate::bounded_output::reserve_active_required(crate::hir::ids::instance_match::CONTROLS)
        {
            let mut output = crate::hir::ids::instance_match::Compare::new(($actual).as_str());
            std::fmt::Write::write_fmt(
                &mut output,
                format_args!(
                    "semaprax.function-instance.v1:{}:{}:{}:",
                    ($template).as_str().len(),
                    $template,
                    ($arguments).len(),
                ),
            )
            .expect("comparison formatting cannot fail");
            for argument in $arguments {
                #[cfg(test)]
                crate::hir::ids::instance_match::tests::record_argument();
                let key;
                if crate::bounded_output::reserve_active_required(
                    crate::hir::ids::instance_match::KEY_CONTROLS,
                ) {
                    key = argument.identity_key_checked(&mut output.complete);
                } else {
                    output.complete = false;
                    key = argument.identity_key();
                }
                std::fmt::Write::write_fmt(&mut output, format_args!("{}:{key}", key.len()))
                    .expect("comparison formatting cannot fail");
            }
            output.remaining.is_empty() & !output.failed & output.complete
        } else {
            // Retain every old key producer even after the new admission fails.
            // No comparison carrier is constructed on this cold failure path.
            for argument in $arguments {
                #[cfg(test)]
                crate::hir::ids::instance_match::tests::record_argument();
                let _ = argument.identity_key();
            }
            false
        }
    }};
}
pub(in crate::hir) use streamed_instance_matches;

macro_rules! instance_matches {
    ($actual:expr, $template:expr, $arguments:expr) => {{
        #[cfg(test)]
        {
            if crate::hir::ids::instance_match::tests::legacy_selected() {
                crate::hir::FunctionInstanceId::derive($template, $arguments) == *($actual)
            } else {
                crate::hir::ids::instance_match::streamed_instance_matches!(
                    $actual, $template, $arguments
                )
            }
        }
        #[cfg(not(test))]
        {
            crate::hir::ids::instance_match::streamed_instance_matches!(
                $actual, $template, $arguments
            )
        }
    }};
}
pub(in crate::hir) use instance_matches;

#[cfg(test)]
#[path = "instance_match/tests.rs"]
pub(in crate::hir) mod tests;
