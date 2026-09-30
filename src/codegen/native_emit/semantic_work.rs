//! Agent Stage Semantic Work v1 instrumentation for native C11.
//!
//! Only the private Agent-stage executor requests this. Every other native
//! emission leaves [`NativeEmissionContext::semantic`] unset and stays
//! byte-identical. The meter charges the same semantic points as the
//! interpreter: one unit when a metered source function frame is entered
//! (after call-depth admission, before its preconditions) and one when a
//! `while` body is entered after its condition evaluated `true`. A refused
//! charge selects a sticky adapter status and leaves through the function's
//! ordinary shared epilogue, so every live compiler-owned slot settles exactly
//! as it does on any other failure. Performed plan finalizers are recorded in
//! execution order by the cleanup-plan lowering (`native_bytes`).

use super::*;
use std::collections::BTreeMap;

/// The status domain a refused semantic charge selects. It is never one of
/// the compiler-owned arithmetic or contract domains.
pub(crate) const SEMANTIC_FUEL_STATUS_DOMAIN: &str = "semaprax.agent-stage-semantic-fuel.v1";
/// Fixed capacity of the performed-finalizer log. Overflow is sticky and the
/// executor refuses the whole observation rather than truncating it.
pub(crate) const SEMANTIC_EVENT_CAPACITY: u32 = 256;

/// The metering selected for one native stage artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativeSemanticMetering {
    /// Semantic fuel units the artifact may charge.
    pub(crate) fuel_limit: u64,
    /// Metered monomorphic source functions and their event ordinals.
    pub(crate) functions: BTreeMap<DeclarationId, u32>,
}

impl NativeSemanticMetering {
    pub(super) fn ordinal(&self, execution: &FunctionExecutionId) -> Option<u32> {
        match execution {
            FunctionExecutionId::Monomorphic(id) => self.functions.get(id).copied(),
            FunctionExecutionId::Generic(_) => None,
        }
    }
}

/// Emit the metering state and helpers after the native prelude.
pub(super) fn emit_runtime(output: &mut impl COutput, metering: &NativeSemanticMetering) {
    write!(
        output,
        "#define SPX_SEMANTIC_EVENT_CAPACITY UINT32_C({SEMANTIC_EVENT_CAPACITY})\n\
         static const uint64_t spx_semantic_fuel_limit = UINT64_C({limit});\n\
         static uint64_t spx_semantic_fuel_used = UINT64_C(0);\n\
         static bool spx_semantic_fuel_exhausted = false;\n\
         static uint32_t spx_semantic_event_count = UINT32_C(0);\n\
         static bool spx_semantic_event_overflow = false;\n\
         static uint64_t spx_semantic_events[SPX_SEMANTIC_EVENT_CAPACITY];\n\
         static __attribute__((unused)) bool spx_semantic_charge(void) {{\n\
         \x20   if (spx_semantic_fuel_exhausted || spx_semantic_fuel_used >= spx_semantic_fuel_limit) {{\n\
         \x20       spx_semantic_fuel_exhausted = true;\n\
         \x20       return false;\n\
         \x20   }}\n\
         \x20   ++spx_semantic_fuel_used;\n\
         \x20   return true;\n\
         }}\n\
         static __attribute__((unused)) void spx_semantic_cleanup_event(uint32_t function, uint32_t flag) {{\n\
         \x20   if (spx_semantic_event_count >= SPX_SEMANTIC_EVENT_CAPACITY) {{\n\
         \x20       spx_semantic_event_overflow = true;\n\
         \x20       return;\n\
         \x20   }}\n\
         \x20   spx_semantic_events[spx_semantic_event_count++] = ((uint64_t)function << 32) | (uint64_t)flag;\n\
         }}\n\
         static __attribute__((unused)) spx_status_token spx_rt_semantic_fuel_failure(\n\
         \x20   struct spx_context *spx_ctx\n\
         ) {{\n\
         \x20   spx_status_token token = SPX_STATUS_SUCCESS;\n\
         \x20   if (!spx_status_record_adapter(spx_ctx, \"{SEMANTIC_FUEL_STATUS_DOMAIN}\", UINT32_C(1), SPX_STATUS_CLASS_ADAPTER, SPX_RETRYABILITY_FALSE, &token)) {{\n\
         \x20       spx_runtime_invariant_failure(\"semantic fuel status arena exhaustion\");\n\
         \x20   }}\n\
         \x20   return token;\n\
         }}\n\n",
        limit = metering.fuel_limit,
    )
    .expect("writing to a string cannot fail");
}

impl<O: COutput> CEmitter<'_, O> {
    /// Charge one semantic unit at the current point of a metered function.
    pub(super) fn semantic_charge(&mut self) {
        if !self.semantic_metered {
            return;
        }
        self.line("if (!spx_semantic_charge()) {");
        self.indent += 1;
        self.line("spx_status = spx_rt_semantic_fuel_failure(spx_ctx);");
        self.line("goto spx_epilogue;");
        self.indent -= 1;
        self.line("}");
    }
}
