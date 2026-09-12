//! A second, structurally different [`ModelHandler`] fixture — the proof
//! that `model.invoke`'s provider independence (issue #177) is a property of
//! the trait boundary and not an accident of there being exactly one
//! implementation.
//!
//! [`super::fixture::FixtureModelHandler`] scripts one whole
//! [`ModelInvocationOutcome`] per call and hands it back verbatim: nothing
//! sits between "what the test wrote" and "what the trait returns". If
//! `ModelHandler` had quietly been shaped around that one implementation's
//! internals — say, around always holding a complete pre-encoded response —
//! a second handler with a genuinely different internal representation would
//! not fit through it without changing the trait.
//!
//! [`ChunkedModelHandler`] is that second handler. Its script does not hold
//! finished responses; it holds a sequence of small tokenised text
//! fragments — the way a real streaming transport accumulates a response
//! body incrementally, frame by frame, rather than receiving it as one
//! buffer. `invoke` performs a real assembly step (join on an internal
//! 0x1F unit-separator wire form, then re-encode into the same boundary
//! shape [`super::fixture::FixtureProposalDecoder`] accepts) to produce the
//! exact [`ModelInvocationOutcome`] a whole-response handler would have
//! returned for an equivalent script. The internal representation differs;
//! the boundary contract does not — see `neutrality_tests` for the
//! cross-transport equivalence proof this exists to support.
//!
//! This handler also gives [`super::model_invoke::ModelFailure::MalformedResponse`]
//! a *handler-reported* path (an assembly step whose fragments cannot be
//! joined into a well-formed response), distinct from the only other
//! `MalformedResponse` coverage in this crate
//! (`an_oversized_response_is_treated_as_malformed_before_decode` in
//! `tests.rs`), which is entirely kernel-derived and never calls a handler
//! for that case at all.

use std::collections::VecDeque;

use super::fixture::fixture_response;
use super::model_invoke::{
    ModelFailure, ModelHandler, ModelInvocationOutcome, ModelInvocationRequest,
    ModelInvokeCapability,
};

/// ASCII unit separator: the byte [`ChunkedModelHandler`] joins its internal
/// tokenised fragments on before re-splitting and reassembling them at the
/// boundary. Never appears in a fragment's own text in this module's
/// fixtures, so join/split round-trips exactly.
const UNIT_SEPARATOR: u8 = 0x1f;

/// One scripted turn for [`ChunkedModelHandler`]. Unlike
/// [`ModelInvocationOutcome`] (the boundary type), this is the handler's own
/// internal shape: a turn is either a list of fragments still awaiting
/// assembly, a script entry that deliberately cannot be assembled, or a
/// failure passed straight through.
pub enum ChunkedTurn {
    /// Tokenised fragments of one turn's answer text, in order. `invoke`
    /// joins them on [`UNIT_SEPARATOR`] into an internal wire buffer, then
    /// splits and re-concatenates that buffer back into plain text —
    /// genuine assembly work a whole-response handler never performs.
    Chunks(Vec<&'static str>),
    /// Fragments exist but are declared unassemblable: the handler itself
    /// reports [`ModelFailure::MalformedResponse`], never the kernel's
    /// separate oversize-response path.
    Unassemblable,
    /// A scripted non-`Settled` outcome, passed straight through.
    Failed(ModelFailure, usize),
}

/// A `model.invoke` fixture whose internal representation is a queue of
/// tokenised fragment lists rather than a queue of finished outcomes. See
/// the module documentation for why that difference is the point.
pub struct ChunkedModelHandler {
    script: VecDeque<ChunkedTurn>,
    pub calls: usize,
}

impl ChunkedModelHandler {
    #[must_use]
    pub fn scripted(script: Vec<ChunkedTurn>) -> Self {
        Self {
            script: script.into(),
            calls: 0,
        }
    }
}

impl ModelHandler for ChunkedModelHandler {
    fn invoke(
        &mut self,
        _capability: &ModelInvokeCapability,
        request: &ModelInvocationRequest,
    ) -> ModelInvocationOutcome {
        self.calls += 1;
        let turn_script = self
            .script
            .pop_front()
            .unwrap_or_else(|| panic!("chunked model handler called past its scripted script"));
        match turn_script {
            ChunkedTurn::Chunks(chunks) => {
                // Internal wire form: fragments joined by a separator byte,
                // as an incremental transport would accumulate frames before
                // any boundary-facing decode ever sees them.
                let mut wire = Vec::new();
                for (index, chunk) in chunks.iter().enumerate() {
                    if index > 0 {
                        wire.push(UNIT_SEPARATOR);
                    }
                    wire.extend_from_slice(chunk.as_bytes());
                }
                // The assembly/decode step at the boundary: split the wire
                // form back apart and concatenate the pieces into the plain
                // answer text the shared boundary format expects. A
                // whole-response handler has no equivalent step at all.
                let assembled: String = wire
                    .split(|byte| *byte == UNIT_SEPARATOR)
                    .map(|part| std::str::from_utf8(part).unwrap_or_default())
                    .collect::<Vec<_>>()
                    .concat();
                ModelInvocationOutcome::Settled(fixture_response(request.turn, &assembled))
            }
            ChunkedTurn::Unassemblable => ModelInvocationOutcome::Failed {
                failure: ModelFailure::MalformedResponse,
                attempted_bytes: 0,
            },
            ChunkedTurn::Failed(failure, attempted_bytes) => ModelInvocationOutcome::Failed {
                failure,
                attempted_bytes,
            },
        }
    }
}
