//! Core Wasm call-depth admission (issue #293 P2-1).
//!
//! Every backend that claims equivalent checked behavior must refuse the
//! same unbounded recursion the same way. The interpreter enforces
//! `MAX_CALL_DEPTH` (`crate::interpreter::MAX_CALL_DEPTH`, 256) in
//! `call_frame`, before its own semantic charge; the native C11 backend
//! enforces the identical `SPX_MAX_CALL_DEPTH` in every emitted function's
//! prologue, before `semantic_charge`, and decrements a per-context counter
//! in its shared `spx_epilogue`. Core Wasm previously had neither, so a
//! recursive metered function diverged into fuel exhaustion or an
//! uncontrolled host-engine stack trap instead of a reported
//! `CallDepthExceeded` outcome.
//!
//! This module gives `emit_function_profile` (`super::emit_function_profile`)
//! the same admission, expressed with one always-on mutable global per
//! module build: a live-frame counter, incremented at every function's
//! entry (before its own `semantic_charge`, before its preconditions) and
//! decremented once at the shared exit every recoverable status already
//! converges on. Unlike native's check-then-increment plus an
//! entry-guard flag, this increments unconditionally and only then compares
//! against the ceiling; because the shared exit decrements unconditionally
//! too, a refused frame's increment and its own decrement cancel exactly,
//! so no extra per-function local is needed to guard against underflow --
//! the two orderings refuse the identical prior depth
//! (`prior_depth >= MAX_CALL_DEPTH`) and leave the counter at the identical
//! value afterward.
//!
//! Admission is activated per top-level module build (`activate`), not
//! globally: a build that never calls `activate` gets no depth global and no
//! check at all, unchanged from before this module existed. Both module
//! families the Agent Stage executor and the ordinary compiled-Wasm target
//! reach (`emit_byte_exports_profile` and `emit_profile_with_scalar_exports`)
//! activate it; the narrower WIT-component test harnesses under
//! `src/wasm/*_component_v*.rs` (reached only through
//! `lower_selected_functions`/`lower_selected_function_instances`) do not,
//! and so keep their pre-existing behavior.

use std::cell::Cell;

use super::*;

/// Fixed call-depth ceiling; exceeding it is a compiler-capacity outcome,
/// never a language status. Matches `interpreter::MAX_CALL_DEPTH` and native
/// C11's `SPX_MAX_CALL_DEPTH`.
pub(crate) const MAX_CALL_DEPTH: i64 = 256;

/// The private status a refused call-depth admission propagates through the
/// ordinary status lane. It lies outside the public `1..=10` arithmetic and
/// contract range, and outside every other private status this backend
/// already reserves (through `STATUS_BOX_ALLOCATION_FAILURE = 17`).
pub(crate) const CALL_DEPTH_STATUS: i32 = 18;

/// Exported sticky marker a depth-refused build sets. The status value alone
/// is not decodable by the generated JS observers -- their raw-status
/// convention only recognizes `1..=10` -- so the executor observes this flag
/// directly instead, exactly as it already observes Agent Stage Semantic
/// Work v1's `spx_semantic_fuel_exhausted` sticky global.
pub(crate) const CALL_DEPTH_EXCEEDED_EXPORT: &str = "spx_call_depth_exceeded";

/// Number of globals `append_globals` appends: the live-frame counter, then
/// the sticky exceeded flag.
pub(super) const GLOBAL_COUNT: u32 = 2;

thread_local! {
    static GLOBAL_INDEX: Cell<Option<u32>> = const { Cell::new(None) };
}

/// Deactivates call-depth admission for the current thread when dropped,
/// including on unwind. Held for the lifetime of one top-level module build.
pub(super) struct Scope;

impl Drop for Scope {
    fn drop(&mut self) {
        GLOBAL_INDEX.with(|cell| cell.set(None));
    }
}

/// Activate call-depth admission for one top-level module build. `index` is
/// the module's next free global index once every other global -- including
/// `append_globals`' own two -- has been counted; callers append those two
/// globals themselves via [`append_globals`] before activating. Returns a
/// guard that deactivates admission when the build function returns.
pub(super) fn activate(index: u32) -> Result<Scope, Diagnostic> {
    GLOBAL_INDEX.with(|cell| {
        if cell.get().is_some() {
            return Err(error(
                "call-depth admission is already active on this thread",
            ));
        }
        cell.set(Some(index));
        Ok(())
    })?;
    Ok(Scope)
}

fn global_index() -> Option<u32> {
    GLOBAL_INDEX.with(Cell::get)
}

/// Append the always-on live-frame counter and sticky exceeded-flag globals,
/// both mutable `i32`, both initialized zero.
pub(super) fn append_globals(globals: &mut Vec<u8>) {
    globals.extend([I32, 0x01, 0x41, 0x00, 0x0b]);
    globals.extend([I32, 0x01, 0x41, 0x00, 0x0b]);
}

/// Export the sticky exceeded-flag global (the frame counter itself is a
/// private implementation detail, never observed). No-op unless admission is
/// active for this build.
pub(super) fn append_exports(exports: &mut Vec<u8>) {
    let Some(base) = global_index() else {
        return;
    };
    write_name(exports, CALL_DEPTH_EXCEEDED_EXPORT);
    exports.push(0x03);
    write_u32(exports, base + 1);
}

impl Emitter<'_> {
    /// Refuse admission at the same call-depth ceiling the interpreter and
    /// native C11 backend enforce, before this function's own semantic
    /// charge and before its preconditions. A no-op when admission is not
    /// active for this build.
    pub(super) fn call_depth_admission(&mut self) -> Result<(), Diagnostic> {
        let Some(depth_global) = global_index() else {
            return Ok(());
        };
        let exceeded_global = depth_global + 1;
        // ++depth
        self.output.push(0x23);
        write_u32(self.output, depth_global);
        self.output.extend([0x41, 0x01, 0x6a, 0x24]);
        write_u32(self.output, depth_global);
        // if depth > MAX_CALL_DEPTH
        self.output.push(0x23);
        write_u32(self.output, depth_global);
        self.output.push(0x41);
        write_i64(self.output, MAX_CALL_DEPTH);
        self.output.push(0x4b); // i32.gt_u
        self.output.extend([0x04, 0x40]); // if (void)
        self.output.extend([0x41, 0x01, 0x24]);
        write_u32(self.output, exceeded_global);
        self.output.push(0x41);
        write_i64(self.output, i64::from(CALL_DEPTH_STATUS));
        self.output.push(0x21);
        write_u32(self.output, self.plan.status);
        self.output.push(0x0c);
        write_u32(
            self.output,
            self.control_depth + self.status_exit_extra_depth,
        );
        self.output.push(0x0b); // end if
        Ok(())
    }
}

/// Decrement the live-frame counter at the function's shared exit. Every
/// entered frame -- refused or not -- incremented it exactly once in
/// [`Emitter::call_depth_admission`], so this runs unconditionally too,
/// with no entry-guard flag needed (see the module documentation). A no-op
/// unless admission is active for this build.
pub(super) fn emit_decrement(body: &mut Vec<u8>) {
    let Some(depth_global) = global_index() else {
        return;
    };
    body.push(0x23);
    write_u32(body, depth_global);
    body.extend([0x41, 0x01, 0x6b, 0x24]);
    write_u32(body, depth_global);
}
