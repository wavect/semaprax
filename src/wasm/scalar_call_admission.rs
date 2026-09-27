//! Core Wasm call-depth admission for the legacy scalar-core emitter (issue
//! #293 P2-2).
//!
//! `aggregate::call_admission` (P2-1) gave every function the aggregate
//! builder emits (`emit_byte_exports_profile`,
//! `emit_profile_with_scalar_exports`) the same call-depth ceiling the
//! interpreter (`crate::interpreter::MAX_CALL_DEPTH`, 256) and the native
//! C11 backend (`SPX_MAX_CALL_DEPTH`) already enforce. It left one family
//! unreached: `emit_resolved_module_internal`'s final branch -- the
//! separate legacy scalar-core emitter reached by a plain scalar, or
//! owned-Bytes/owned-String, program with no authored record/class/variant,
//! no concrete generic variant, no byte-array data, no Vec, and no Box (no
//! aggregate lowering applies). That emitter's own `emit_expr` never called
//! `call_admission`, so a plain recursive function compiled through
//! `wasm::emit_module` had no depth ceiling on Core Wasm at all, diverging
//! from every other backend's checked `CallDepthExceeded` outcome into an
//! uncontrolled host-engine stack trap instead.
//!
//! This module gives that emitter's shared function-body loop the identical
//! admission, expressed the same way `aggregate::call_admission` does: one
//! always-on mutable global per module build, incremented at every
//! function's entry (before its preconditions -- this backend has no
//! separate semantic charge to precede) and decremented once on that
//! function's one normal-return path.
//!
//! The two families differ in how a refused frame is *reported*, because
//! they differ in how every other runtime failure already is. The aggregate
//! builder converges every recoverable failure -- refused call depth
//! included -- on one shared exit that sets a status local and returns
//! normally, so its decrement runs unconditionally, refused or not (see
//! `aggregate::call_admission`'s module documentation). The legacy
//! scalar-core emitter has no such convergence: every runtime failure it
//! already reports -- a contract violation, a checked-arithmetic overflow --
//! reports by calling a host import that is contractually obligated to
//! throw, with a Wasm `unreachable` immediately after as the fail-closed
//! backstop for a hostile host that returns instead. A genuine
//! `unreachable` trap unwinds the *entire* call activation back to the host
//! boundary; it never resumes into this function's own decrement, nor any
//! caller's, so a refused (or otherwise trapped) frame's increment is never
//! paired with a decrement.
//!
//! Reporting reuses this backend's existing `spx_contract_fail` host import
//! (see `emit_contract_guard`) rather than inventing a second channel: it
//! passes the identical wire value `aggregate::call_admission::CALL_DEPTH_STATUS`
//! (18), which `wasm/browser_runtime.js`'s `spx_contract_fail` normalizes to
//! the stable `("semaprax.runtime.v1", 1)` status -- the same domain and
//! code the native C11 backend's `spx_rt_call_depth_failure` reports (see
//! `codegen::native_scalar_runtime::CALL_DEPTH_STATUS_DOMAIN`), so a caller
//! observing the normalized status cannot tell which backend produced it.
//!
//! **A trap does not discard the module instance.** The live-frame counter
//! is one instance's persistent state, and the production host glue
//! (`wasm/browser_runtime.js`'s `invoke`) catches exactly this class of
//! failure and keeps calling the same instance afterward -- that is the
//! entire point of normalizing it to a typed status instead of leaking an
//! uncaught exception. So an uncompensated increment left behind by one
//! trapped call would poison every later call on that instance: a refused
//! deep recursion would leave even a shallow, otherwise-successful later
//! call refused too. [`emit_reset`] closes that gap: every genuine external
//! entry -- the only point a host can call into this module afresh, never
//! an internal call within it -- resets the counter to zero as the first
//! thing its own bytecode does, so no earlier call's leftover increments
//! survive into the next one. Internal calls between program functions
//! never reset it; only [`emit_admission`]'s increment and [`emit_decrement`]
//! ever touch it there. None of this emitter's host imports call back into
//! any of its exports (each is a plain `(i64/i32...) -> i64/i32` numeric
//! function per the ABI in `emit_resolved_module_internal`; the module
//! grants them no table or function-reference capability to do so), so a
//! reset at every export's own entry is exhaustive: nothing else can be a
//! genuine external entry.

use super::aggregate::call_admission::{CALL_DEPTH_STATUS, MAX_CALL_DEPTH};
use super::{write_i32, write_u32, ByteOutput, I32};

/// Import index of `spx_contract_fail`: always the seventh import declared
/// (`spx_add, spx_sub, spx_mul, spx_div, spx_rem, spx_neg,
/// spx_contract_fail`), before any optional owned-resource or string
/// imports, so this index is stable regardless of which of those a given
/// module admits.
const CONTRACT_FAIL_IMPORT: u32 = 6;

/// Number of globals this module appends: the one private live-frame
/// counter.
pub(super) const GLOBAL_COUNT: u32 = 1;

/// Append the always-on live-frame counter: mutable i32, initialized zero.
pub(super) fn append_global(globals: &mut impl ByteOutput) {
    globals.extend_bytes(&[I32, 0x01, 0x41, 0x00, 0x0b]);
}

/// Refuse admission at the identical call-depth ceiling
/// `aggregate::call_admission`, the interpreter and the native C11 backend
/// already enforce, before this function's own preconditions -- the first
/// bytecode this backend's function bodies execute. A no-op call site never
/// exists: every executable function in this emitter's shared loop calls
/// this exactly once, unconditionally.
pub(super) fn emit_admission(body: &mut impl ByteOutput, depth_global: u32) {
    // ++depth
    body.push(0x23); // global.get
    write_u32(body, depth_global);
    body.push(0x41); // i32.const 1
    body.push(0x01);
    body.push(0x6a); // i32.add
    body.push(0x24); // global.set
    write_u32(body, depth_global);
    // if depth > MAX_CALL_DEPTH { spx_contract_fail(CALL_DEPTH_STATUS); unreachable }
    body.push(0x23); // global.get
    write_u32(body, depth_global);
    body.push(0x41); // i32.const MAX_CALL_DEPTH
    write_i32(body, MAX_CALL_DEPTH as i32);
    body.push(0x4b); // i32.gt_u
    body.extend_bytes(&[0x04, 0x40]); // if (void)
    body.push(0x41); // i32.const CALL_DEPTH_STATUS
    write_i32(body, CALL_DEPTH_STATUS);
    body.push(0x10); // call
    write_u32(body, CONTRACT_FAIL_IMPORT);
    body.push(0x00); // unreachable: fail-closed fallback only, see module docs
    body.push(0x0b); // end if
}

/// Reset the live-frame counter to zero. Emitted once, as the first thing
/// its own bytecode does (after its own locals declaration, which every
/// Wasm function body must still lead with), at every genuine external
/// entry this emitter produces -- see the module documentation for why that
/// reset is exhaustive and why it must never run for an internal call
/// between program functions.
pub(super) fn emit_reset(body: &mut impl ByteOutput, depth_global: u32) {
    body.push(0x41); // i32.const 0
    body.push(0x00);
    body.push(0x24); // global.set
    write_u32(body, depth_global);
}

/// The bare legacy web target's synthesized `main` entry wrapper: reset the
/// live-frame counter, then call straight into `main`'s own compiled body
/// (which may itself be recursive, hence why this reset lives here and not
/// inside that body -- see the module documentation). No locals: the
/// call's i64 result is left on the stack for this function's own implicit
/// return.
pub(super) fn emit_main_entry_body(body: &mut impl ByteOutput, depth_global: u32, target: u32) {
    write_u32(body, 0);
    emit_reset(body, depth_global);
    body.push(0x10); // call
    write_u32(body, target);
    body.push(0x0b);
}

/// Decrement the live-frame counter on this function's one normal-return
/// path. Never reached on a refused frame: see the module documentation for
/// why no compensating decrement is needed there.
pub(super) fn emit_decrement(body: &mut impl ByteOutput, depth_global: u32) {
    body.push(0x23); // global.get
    write_u32(body, depth_global);
    body.push(0x41); // i32.const 1
    body.push(0x01);
    body.push(0x6b); // i32.sub
    body.push(0x24); // global.set
    write_u32(body, depth_global);
}
