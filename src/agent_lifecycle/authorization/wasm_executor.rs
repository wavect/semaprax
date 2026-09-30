//! `WasmStageExecutor`: the Core Wasm leg of the sealed [`super::StageExecutor`]
//! seam (#143, #182), and an exact account of what it executes and what it
//! still refuses.
//!
//! ## What this executor reuses
//!
//! Per #143's hand-off, this executor reuses the existing Wasm build and the
//! existing Node/V8 owned-data host-call arena -- the same
//! `project::derive_public_api_descriptor` +
//! `project::prepare_owned_data_npm_build` pipeline
//! `tests/agent_runtime_v1/stage_backend_parity.rs` already drives -- and does
//! not reimplement that arena for a second engine.
//!
//! ## The gap this file used to fail closed on, and how it is now crossed
//!
//! That pipeline's own admission rule
//! (`src/project/public_api.rs::parameter_type`) accepts exactly four
//! parameter shapes: `i64`/`bool` by value, and `borrow Str`/`borrow SliceU8`.
//! It admits no record, no variant, and no owned-`Bytes` PARAMETER at all.
//! Every bound Agent stage signature (`stages.rs::bind_with_step_result`)
//! takes at least one `own`/`borrow` record parameter (`Task`/`State`/
//! `Outcome`), so no bound stage call can be handed to that arena directly.
//!
//! The way across, recorded as a design by `994c6e25` and implemented here,
//! is NOT to widen that admission rule -- widening it to force a record
//! through would be exactly the "do not bypass verification in a backend"
//! prohibition. It is to inject an ordinary, fully checked SPX *driver*
//! function whose own parameters and result are inside the existing admitted
//! vocabulary by construction, and let the record/variant value live entirely
//! inside the Wasm module, never crossing the arena boundary:
//!
//! 1. **Synthesize driver source, do not hand-build HIR.** For one stage
//!    call this renders the call's arguments as ordinary `.spx` literals
//!    (`State { objective: bytes_copy(array_as_slice(seed)), budget: 10, ..
//!    }`) inside a zero-parameter function, calls the stage by its real
//!    source name, and projects the result down to one admitted leaf --
//!    exactly the shape the hand-written `case.*` wrappers in
//!    `tests/agent_runtime_v1/stage_backend_parity.rs` already use and that
//!    all four engines already agree on. Appending source text and
//!    re-parsing is deliberately preferred over building `ast::Function` by
//!    hand: the parser, verifier and resolver then validate every literal
//!    field against the real declaration instead of this file
//!    re-implementing that checking.
//! 2. **Get a genuine plan, the only way one exists today.**
//!    `loan_plan::build_plan` and `cleanup_plan::build::build_plan` both
//!    consume an already-resolved `ResolvedFunction`, and the only thing
//!    that produces one is the whole-program pass `hir::resolve`, which
//!    needs the complete source `ast::Program`. So the driver is spliced
//!    into real `.spx` text and the whole module is re-checked and
//!    re-resolved through `crate::check` + `hir::resolve` + `hir::validate`,
//!    the same three calls `compile_agent_lifecycle` itself makes. The
//!    driver therefore has a real `CleanupPlan`/`LoanPlan`, not a
//!    hand-forged one.
//! 3. **The source text reaches this file explicitly and is target-bound.**
//!    [`WasmStageExecutor`] checks/re-resolves it before any descriptor or
//!    Node work, requires the whole resulting program and selected entry to
//!    equal the retained invocation, and derives source-revision, lifecycle
//!    and invocation identities. Those identities are the owned-data
//!    descriptor subject; a fixed synthetic subject cannot select target
//!    work. Nothing is read from the filesystem and no ambient authority is
//!    acquired.
//! 4. **The re-derived program must still be the same program.** Driver text
//!    is appended after the existing source, so every existing declaration's
//!    byte offsets, spans and `@id` identities are unchanged. That is
//!    asserted rather than assumed: the re-resolved entry function is
//!    compared for exact equality against the `ResolvedFunction` this
//!    executor was handed, and a mismatch fails closed before any artifact
//!    is built. The selected driver exports and descriptor digest are then
//!    replay-verified against that same target binding before Node runs.
//! 5. **No admission-rule change.** Each driver takes either no parameters
//!    or one `i64` by value, and returns `i64`, `bool`, `usize`, or owned
//!    `Bytes` --
//!    `project::public_api::parameter_type` and `result_type` already admit
//!    all of those unmodified. The record/variant argument and result never
//!    cross that boundary.
//!
//! ## What is still refused
//!
//! This executor's closed result vocabulary is the same one
//! `native_executor.rs` uses: a record or variant whose leaves are all
//! `Bytes`, `i64`, `bool`, or `u8`, or a record whose leaves may additionally be
//! `usize`. A bare scalar result still takes the direct path (no driver
//! needed); anything else -- a nested record leaf, a `Str`/`Float` leaf, a
//! generic instantiation, a variant `usize` leaf, or a variant case with no
//! fields -- fails closed with an `SPX-G570`
//! diagnostic naming the unsupported shape rather than guessing.
//! A source-synthesized `Bytes` argument is likewise bounded to the same
//! 65,536-byte stream limit used while decoding a projected `Bytes` result;
//! an oversized carrier refuses before it is expanded into generated source
//! or any target artifact is built.
//!
//! ## What this is not
//!
//! Running a stage body in a real Core Wasm module under a real engine is
//! not the same as running the Agent *lifecycle* on Wasm. Lifecycle stage
//! count and effect/model budgets remain in the interpreter-side driver. Its
//! existing monotonic cancellation is now rechecked at the sealed executor
//! boundary and by the registered process provider while Node is live;
//! observed cancellation kills and reaps the child. The embedding host must
//! open one absolute Node executable up front. Execution rechecks that held
//! descriptor, uses a fixed argv and empty environment, runs inside a
//! descriptor-held inventoried private workspace, and enforces a two-second
//! deadline plus the provider's bounded stdout/stderr ceilings. Aggregate
//! projection fan-out is refused before spawn when its conservative output
//! bound exceeds that ceiling. This is process admission and cleanup, not
//! Wasm instruction metering. `steps_used` is
//! reported as `0` because Wasm does not count interpreter steps, exactly as
//! `native_executor.rs` already does. The
//! Node boundary preserves only returned values and the compiler-owned
//! arithmetic/contract status table; it authenticates redundant raw and
//! normalized status fields before constructing an evaluation. It does not
//! claim contract-detail, fuel, variant-indexed-`Bytes` cleanup-event, or
//! lifecycle settlement parity. A record `Bytes` projection does report its
//! copy-out event only after the replay-verified generated facade returns an
//! actual owned `Uint8Array`: that facade returns only after its private arena
//! consumes the carrier and settles. This observes JS arena settlement, not a
//! Wasm physical free or instruction count.
//! The evidence this backend supports is local and re-runnable: its trusted
//! embedding host supplies the runtime capability, and it claims nothing
//! about hosted or browser support. The 64 KiB process-provider output ceiling
//! also means general large projected byte streams remain refused.

use std::path::Path;

use sha2::{Digest, Sha256};

use crate::agent_runtime::AgentCancellation;
use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, DeclarationId, OwnershipMode, ResolvedFieldDeclaration, ResolvedFunction, ResolvedType,
    ResolvedTypeDeclarationKind,
};
use crate::interpreter::retained_call::{
    PreparedRetainedCall, RetainedCallEvaluation, RetainedCallOutcome, RetainedField,
    RetainedRecord, RetainedValue, RetainedVariant,
};
use crate::interpreter::OwnedDataCleanupEvent;
use crate::project;

use crate::agent_lifecycle::stages::invariant;

use super::{sealed, ExecutionAuthority, StageExecutor};

#[path = "wasm_executor_outcome.rs"]
mod outcome;
#[path = "wasm_executor_process.rs"]
mod process;
#[path = "wasm_executor_result_plan.rs"]
mod result_plan;
#[path = "wasm_executor_semantic.rs"]
mod semantic;
#[path = "wasm_executor_workspace.rs"]
mod workspace;
use outcome::{decode_node_outcomes, NodeStageRun};
pub use process::WasmStageHost;
use process::{run_node_process, MAX_NODE_STDOUT_BYTES};
pub(in crate::agent_lifecycle) use result_plan::BYTE_STREAM_CAP;
use result_plan::{
    drivers_for, nominal_declaration, Driver, Leaf, Projection, ResultPlan, BYTE_HELPERS,
};
use semantic::{WasmMeter, MAX_SEMANTIC_ROW_BYTES};
use workspace::WasmStageWorkspace;

// The registered process provider admits at most 64 KiB total output. Keep a
// conservative per-projection reservation inside that hard boundary; larger
// byte-stream projections fail closed before process admission.
const MAX_NODE_OUTCOME_ROW_BYTES: usize = 4 * 1_024;
// Scalar values and the fixed compiler-owned status envelopes fit within
// this bound; only owned-byte projections require the larger row allowance.
const MAX_NODE_SCALAR_OUTCOME_ROW_BYTES: usize = 512;

/// The Core Wasm stage executor, carrying the exact module source text it is
/// allowed to re-resolve. It reads no file and opens no network; the source
/// is data the caller hands it.
pub(in crate::agent_lifecycle) struct WasmStageExecutor<'a> {
    pub(super) host: Option<&'a WasmStageHost>,
    pub(super) source: &'a str,
}

/// Exact, authority-free facts one Core Wasm dispatch binds before it may
/// derive a descriptor or start Node.  This is deliberately internal: a
/// caller chooses only the sealed Wasm selector's source text; it cannot mint
/// a subject, lifecycle identity, or invocation identity for some other
/// program.
#[derive(Debug)]
struct WasmTargetBinding<'a> {
    source: &'a str,
    source_revision: String,
    lifecycle_identity: String,
    invocation_identity: String,
    entry: DeclarationId,
}

/// The artifact-specific extension of one [`WasmTargetBinding`].  The
/// descriptor digest is retained with its exact selected export inventory so
/// Node never receives a carrier whose subject or selection was merely
/// inferred from a generated JavaScript expression.
#[derive(Clone, Debug)]
struct WasmArtifactBinding {
    source_revision: String,
    lifecycle_identity: String,
    invocation_identity: String,
    entry: DeclarationId,
    selected: Vec<String>,
    descriptor_digest: String,
}

fn target_digest(domain: &[u8], fields: impl IntoIterator<Item = String>) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    for field in fields {
        hash.update((field.len() as u64).to_le_bytes());
        hash.update(field.as_bytes());
    }
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

impl<'a> WasmTargetBinding<'a> {
    fn bind(
        source: &'a str,
        program: &hir::ResolvedProgram,
        entry: &ResolvedFunction,
        prepared: &PreparedRetainedCall,
        arguments: &[RetainedValue],
        max_steps: usize,
    ) -> Result<Self, Diagnostic> {
        // Parse/resolve/validate before any target artifact construction. The
        // whole resolved program is compared, not merely an entry name, so a
        // same-id source remint cannot substitute a different lifecycle.
        let checked = crate::check(source, Path::new("agent-lifecycle-wasm-target-binding.spx"))
            .map_err(|_| invariant("wasm_executor.binding.source_check"))?;
        let source_revision = crate::graph::revision(&checked);
        let resolved = hir::resolve(&checked)
            .map_err(|_| invariant("wasm_executor.binding.source_resolve"))?;
        hir::validate(&resolved).map_err(|_| invariant("wasm_executor.binding.source_validate"))?;
        if &resolved != program {
            return Err(invariant("wasm_executor.binding.lifecycle"));
        }
        let source_entry = resolved
            .functions
            .iter()
            .find(|function| function.id == entry.id)
            .ok_or_else(|| invariant("wasm_executor.binding.entry_absent"))?;
        if source_entry != entry || prepared.function_id() != entry.id.as_str() {
            return Err(invariant("wasm_executor.binding.entry"));
        }

        let lifecycle_identity = target_digest(
            b"semaprax.agent-lifecycle.wasm-target-lifecycle.v1\0",
            [
                source_revision.clone(),
                program.module.clone(),
                program.entrypoint.as_str().to_owned(),
            ],
        );
        let mut invocation_fields = vec![
            source_revision.clone(),
            lifecycle_identity.clone(),
            entry.id.as_str().to_owned(),
            prepared.parameter_count().to_string(),
            max_steps.to_string(),
            arguments.len().to_string(),
        ];
        invocation_fields.extend(
            arguments
                .iter()
                .map(crate::agent_lifecycle::canonical_retained_value_json),
        );
        let invocation_identity = target_digest(
            b"semaprax.agent-lifecycle.wasm-target-invocation.v1\0",
            invocation_fields,
        );
        Ok(Self {
            source,
            source_revision,
            lifecycle_identity,
            invocation_identity,
            entry: entry.id.clone(),
        })
    }

    fn bind_artifact(
        &self,
        descriptor: &project::PublicApiDescriptor,
        selected: &[String],
    ) -> Result<WasmArtifactBinding, Diagnostic> {
        if selected.is_empty()
            || selected.windows(2).any(|pair| pair[0] >= pair[1])
            || descriptor.exports().len() != selected.len()
            || descriptor
                .exports()
                .iter()
                .zip(selected)
                .any(|(export, expected)| export.stable_id().as_str() != expected)
            || descriptor.project_revision() != self.source_revision
            || descriptor.workspace_revision() != self.lifecycle_identity
            || descriptor.project_graph_digest() != self.invocation_identity
        {
            return Err(invariant("wasm_executor.binding.descriptor"));
        }
        Ok(WasmArtifactBinding {
            source_revision: self.source_revision.clone(),
            lifecycle_identity: self.lifecycle_identity.clone(),
            invocation_identity: self.invocation_identity.clone(),
            entry: self.entry.clone(),
            selected: selected.to_vec(),
            descriptor_digest: descriptor.digest(),
        })
    }

    fn subject(&self) -> project::PublicApiSubject<'_> {
        project::PublicApiSubject {
            project_schema: project::PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
            project_revision: &self.source_revision,
            workspace_revision: &self.lifecycle_identity,
            project_graph_digest: &self.invocation_identity,
        }
    }
}

impl WasmArtifactBinding {
    fn verify_descriptor(
        &self,
        descriptor: &project::PublicApiDescriptor,
    ) -> Result<(), Diagnostic> {
        if descriptor.digest() != self.descriptor_digest
            || descriptor.project_revision() != self.source_revision
            || descriptor.workspace_revision() != self.lifecycle_identity
            || descriptor.project_graph_digest() != self.invocation_identity
            || descriptor
                .exports()
                .iter()
                .map(|export| export.stable_id().as_str())
                .ne(self.selected.iter().map(String::as_str))
        {
            return Err(invariant("wasm_executor.binding.descriptor_drift"));
        }
        Ok(())
    }

    fn verify_build(
        &self,
        build: &project::ProjectNpmBuild,
        descriptor: &project::PublicApiDescriptor,
    ) -> Result<(), Diagnostic> {
        self.verify_descriptor(descriptor)?;
        build
            .verify_public_api_descriptor(descriptor)
            .map_err(|_| invariant("wasm_executor.binding.carrier"))
    }

    fn verify_invocations(&self, invocations: &[String]) -> Result<(), Diagnostic> {
        let mut ordered = invocations.to_vec();
        ordered.sort();
        if ordered != self.selected || self.entry.as_str().is_empty() {
            return Err(invariant("wasm_executor.binding.invocation"));
        }
        Ok(())
    }
}

impl sealed::Sealed for WasmStageExecutor<'_> {}

impl StageExecutor for WasmStageExecutor<'_> {
    fn execute(
        &self,
        _authority: ExecutionAuthority,
        program: &hir::ResolvedProgram,
        prepared: &PreparedRetainedCall,
        arguments: &[RetainedValue],
        max_steps: usize,
        cancellation: Option<&AgentCancellation>,
    ) -> Result<RetainedCallEvaluation, Vec<Diagnostic>> {
        run_admitted(
            self.host,
            self.source,
            program,
            prepared,
            arguments,
            max_steps,
            cancellation,
        )
        .map_err(|error| vec![error])
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_metered(
        &self,
        _authority: ExecutionAuthority,
        program: &hir::ResolvedProgram,
        prepared: &PreparedRetainedCall,
        arguments: &[RetainedValue],
        max_steps: usize,
        profile: &super::StageSemanticProfile,
        cancellation: Option<&AgentCancellation>,
    ) -> Result<RetainedCallEvaluation, Vec<Diagnostic>> {
        let meter = WasmMeter::new(profile);
        let run = || {
            let mut evaluation = run_selected(
                self.host,
                self.source,
                program,
                prepared,
                arguments,
                max_steps,
                cancellation,
                Some(&meter),
            )?;
            evaluation.semantic_work = Some(meter.take()?);
            Ok(evaluation)
        };
        run().map_err(|error| vec![error])
    }
}

fn admitted_parameter(ty: &ResolvedType, ownership: OwnershipMode) -> bool {
    matches!(
        (ty, ownership),
        (ResolvedType::I64, OwnershipMode::Value) | (ResolvedType::Bool, OwnershipMode::Value)
    )
}

fn admitted_result(ty: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::I64 | ResolvedType::Bool)
}

#[cfg(test)]
fn run(
    host: &WasmStageHost,
    source: &str,
    program: &hir::ResolvedProgram,
    prepared: &PreparedRetainedCall,
    arguments: &[RetainedValue],
    max_steps: usize,
) -> Result<RetainedCallEvaluation, Diagnostic> {
    run_admitted(
        Some(host),
        source,
        program,
        prepared,
        arguments,
        max_steps,
        None,
    )
}

fn run_admitted(
    host: Option<&WasmStageHost>,
    source: &str,
    program: &hir::ResolvedProgram,
    prepared: &PreparedRetainedCall,
    arguments: &[RetainedValue],
    max_steps: usize,
    cancellation: Option<&AgentCancellation>,
) -> Result<RetainedCallEvaluation, Diagnostic> {
    run_selected(
        host,
        source,
        program,
        prepared,
        arguments,
        max_steps,
        cancellation,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn run_selected(
    host: Option<&WasmStageHost>,
    source: &str,
    program: &hir::ResolvedProgram,
    prepared: &PreparedRetainedCall,
    arguments: &[RetainedValue],
    max_steps: usize,
    cancellation: Option<&AgentCancellation>,
    meter: Option<&WasmMeter<'_>>,
) -> Result<RetainedCallEvaluation, Diagnostic> {
    if cancellation.is_some_and(AgentCancellation::is_cancelled) {
        return Err(invariant("wasm_executor.process.cancelled"));
    }
    if !(1..=1_000_000).contains(&max_steps) {
        return Err(invariant("wasm_executor.max_steps"));
    }
    let entry = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == prepared.function_id())
        .ok_or_else(|| invariant("wasm_executor.entry.absent"))?;
    if entry.params.len() != arguments.len() || arguments.len() != prepared.parameter_count() {
        return Err(invariant("wasm_executor.argument.arity"));
    }
    let binding = WasmTargetBinding::bind(source, program, entry, prepared, arguments, max_steps)?;
    if entry
        .params
        .iter()
        .all(|parameter| admitted_parameter(&parameter.ty, parameter.ownership))
        && admitted_result(&entry.return_type)
    {
        return run_direct(
            host,
            &binding,
            program,
            entry,
            arguments,
            max_steps,
            cancellation,
            meter,
        );
    }
    run_through_injected_driver(
        host,
        &binding,
        program,
        entry,
        arguments,
        max_steps,
        cancellation,
        meter,
    )
}

// ---------------------------------------------------------------------------
// The direct path: a call the existing descriptor already admits unchanged.
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn run_direct(
    host: Option<&WasmStageHost>,
    binding: &WasmTargetBinding<'_>,
    program: &hir::ResolvedProgram,
    entry: &ResolvedFunction,
    arguments: &[RetainedValue],
    max_steps: usize,
    cancellation: Option<&AgentCancellation>,
    meter: Option<&WasmMeter<'_>>,
) -> Result<RetainedCallEvaluation, Diagnostic> {
    let mut call_args = Vec::with_capacity(arguments.len());
    for (parameter, argument) in entry.params.iter().zip(arguments) {
        let literal = match (&parameter.ty, argument) {
            // The generated bindings' own argument snapshot requires a
            // genuine JS `bigint` for an `i64` parameter, never a `Number` --
            // an un-suffixed literal like `10` is rejected with "argument 0
            // must be signed i64 bigint" before the call is even attempted.
            // Unlike the injected-driver path's SPX literal (which has no
            // negative-integer-literal syntax and must synthesize `0 - N`),
            // this is a JS expression: `-9223372036854775808n` is a plain
            // valid BigInt literal, so `i64::MIN` needs no special case here.
            (ResolvedType::I64, RetainedValue::I64(value)) => format!("{value}n"),
            (ResolvedType::Bool, RetainedValue::Bool(value)) => value.to_string(),
            _ => return Err(invariant("wasm_executor.argument.shape")),
        };
        call_args.push(literal);
    }
    // The decode below always parses plain decimal text as `i64`. A raw
    // `i64` result is a `bigint`, which THROWS if concatenated with the
    // trailing `'\n'` string (`out.map(value => value + '\n')` in
    // `drive_node`) rather than coercing like a `Number` or `boolean` would;
    // a raw `bool` result prints as `"true"`/`"false"`, which the same `i64`
    // parse cannot read either. Both are normalized to decimal text here
    // instead of guessing a decode per return type below.
    let call = match entry.return_type {
        ResolvedType::I64 => format!(
            "String(api.functions['{}']({}))",
            entry.id.as_str(),
            call_args.join(", ")
        ),
        ResolvedType::Bool => format!(
            "(api.functions['{}']({}) ? 1 : 0)",
            entry.id.as_str(),
            call_args.join(", ")
        ),
        _ => return Err(invariant("wasm_executor.decode.result_shape")),
    };
    let selected = vec![entry.id.as_str().to_owned()];
    let value = match build_and_drive(
        host.ok_or_else(|| invariant("wasm_executor.process.host_unavailable"))?,
        binding,
        program,
        &selected,
        &selected,
        &[call],
        0,
        cancellation,
        meter,
    )? {
        NodeStageRun::LanguageFailure(status) => {
            return Ok(evaluation(
                entry,
                RetainedCallOutcome::LanguageFailure(status),
                max_steps,
                Vec::new(),
            ));
        }
        NodeStageRun::FuelExhausted => {
            return Ok(evaluation(
                entry,
                RetainedCallOutcome::FuelExhausted,
                max_steps,
                Vec::new(),
            ));
        }
        NodeStageRun::CallDepthExceeded => {
            return Ok(evaluation(
                entry,
                RetainedCallOutcome::CallDepthExceeded,
                max_steps,
                Vec::new(),
            ));
        }
        NodeStageRun::Returned(mut values) => {
            let value = values
                .pop()
                .ok_or_else(|| invariant("wasm_executor.decode.arity"))?;
            value.require_projection(false)?;
            value.text
        }
    };
    let value: i64 = value
        .parse()
        .map_err(|_| invariant("wasm_executor.decode"))?;
    let outcome = match entry.return_type {
        ResolvedType::I64 => RetainedCallOutcome::Returned(RetainedValue::I64(value)),
        ResolvedType::Bool => RetainedCallOutcome::Returned(RetainedValue::Bool(value != 0)),
        _ => return Err(invariant("wasm_executor.decode.result_shape")),
    };
    Ok(evaluation(entry, outcome, max_steps, Vec::new()))
}

fn evaluation(
    entry: &ResolvedFunction,
    outcome: RetainedCallOutcome,
    max_steps: usize,
    cleanup_events: Vec<OwnedDataCleanupEvent>,
) -> RetainedCallEvaluation {
    RetainedCallEvaluation {
        function_id: entry.id.clone(),
        outcome,
        cleanup_events,
        // Core Wasm does not count interpreter steps; `native_executor.rs`
        // reports the same `0` for the same reason. Step counts are not
        // claimed comparable across engines.
        steps_used: 0,
        max_steps,
        failure: None,
        semantic_work: None,
    }
}

/// Renders one `RetainedValue` as an `.spx` expression, pushing any
/// supporting `let` bindings onto `prelude`.
///
/// The declared parameter type is checked against the runtime shape here, so
/// a malformed argument is refused before any source is synthesized rather
/// than becoming a parse error later.
fn render_value(
    program: &hir::ResolvedProgram,
    ty: &ResolvedType,
    value: &RetainedValue,
    prelude: &mut String,
    next: &mut usize,
) -> Result<String, Diagnostic> {
    match (ty, value) {
        (ResolvedType::Bool, RetainedValue::Bool(item)) => Ok(item.to_string()),
        (ResolvedType::I64, RetainedValue::I64(item)) => {
            if *item >= 0 {
                Ok(item.to_string())
            } else if *item == i64::MIN {
                // The parser folds this exact canonical spelling to one
                // signed-minimum literal. Do not route it through `0 - N`:
                // `9223372036854775808` is deliberately not a positive i64
                // literal, so that subtraction form cannot represent MIN.
                Ok("-9223372036854775808".to_owned())
            } else {
                let name = format!("spx_lit{next}");
                *next += 1;
                prelude.push_str(&format!("    let {name} = 0 - {};\n", -*item));
                Ok(name)
            }
        }
        (ResolvedType::Usize, RetainedValue::Usize(item)) => Ok(format!("{item}usize")),
        (ResolvedType::U8, RetainedValue::U8(item)) => Ok(format!("{item}u8")),
        (ResolvedType::I32, RetainedValue::I32(item)) => {
            if *item >= 0 {
                Ok(format!("{item}i32"))
            } else if *item == i32::MIN {
                // The signed minimum is one canonical literal: its positive
                // magnitude is deliberately outside the `i32` literal domain.
                Ok("-2147483648i32".to_owned())
            } else {
                let name = format!("spx_lit{next}");
                *next += 1;
                prelude.push_str(&format!("    let {name} = 0i32 - {}i32;\n", -*item));
                Ok(name)
            }
        }
        (ResolvedType::Bytes, RetainedValue::Bytes(item)) => {
            // An injected driver spells each byte in checked source. Keep
            // that expansion under the same exact payload bound as the
            // bounded result-stream reader below; otherwise one untrusted
            // carrier could turn into arbitrarily large compiler input
            // before the Node capture limits ever apply.
            if item.len() > BYTE_STREAM_CAP {
                return Err(invariant("wasm_executor.argument.bytes_budget"));
            }
            if item.is_empty() {
                // Bytes has no literal. Take the exact empty range of one
                // fixed byte and copy it: this uses the existing owned-data
                // export vocabulary (`bytes_copy`), preserving its normal
                // checked allocation and cleanup rather than adding the
                // separate bounded-buffer operation to this profile.
                let name = format!("spx_lit{next}");
                *next += 1;
                prelude.push_str(&format!("    let {name} = [0u8];\n"));
                let view = format!("spx_view{next}");
                *next += 1;
                prelude.push_str(&format!("    let {view} = array_as_slice({name});\n"));
                return Ok(format!("bytes_copy(byte_range({view}, 0usize, 0usize))"));
            }
            let name = format!("spx_lit{next}");
            *next += 1;
            let elements = item
                .iter()
                .map(|byte| format!("{byte}u8"))
                .collect::<Vec<_>>()
                .join(", ");
            prelude.push_str(&format!("    let {name} = [{elements}];\n"));
            Ok(format!("bytes_copy(array_as_slice({name}))"))
        }
        (ResolvedType::Nominal { .. }, RetainedValue::Record(record)) => {
            let declaration = nominal_declaration(program, ty)?;
            if declaration.id != record.record {
                return Err(invariant("wasm_executor.argument.shape"));
            }
            let ResolvedTypeDeclarationKind::Record { fields } = &declaration.kind else {
                return Err(invariant("wasm_executor.argument.shape"));
            };
            let rendered = render_fields(program, fields, &record.fields, prelude, next)?;
            Ok(format!("{} {{ {rendered} }}", declaration.name))
        }
        (ResolvedType::Nominal { .. }, RetainedValue::Variant(variant)) => {
            let declaration = nominal_declaration(program, ty)?;
            if declaration.id != variant.variant {
                return Err(invariant("wasm_executor.argument.shape"));
            }
            let ResolvedTypeDeclarationKind::Variant { cases } = &declaration.kind else {
                return Err(invariant("wasm_executor.argument.shape"));
            };
            let case = cases
                .iter()
                .find(|item| item.id == variant.case)
                .ok_or_else(|| invariant("wasm_executor.argument.case"))?;
            let rendered = render_fields(program, &case.fields, &variant.fields, prelude, next)?;
            Ok(format!(
                "{}::{} {{ {rendered} }}",
                declaration.name, case.name
            ))
        }
        _ => Err(invariant("wasm_executor.argument.shape")),
    }
}

/// Renders a carrier's fields in DECLARED order, regardless of the order the
/// caller supplied them in, and refuses a missing or duplicated field.
fn render_fields(
    program: &hir::ResolvedProgram,
    declared: &[ResolvedFieldDeclaration],
    supplied: &[RetainedField],
    prelude: &mut String,
    next: &mut usize,
) -> Result<String, Diagnostic> {
    if declared.len() != supplied.len() {
        return Err(invariant("wasm_executor.argument.field_count"));
    }
    let mut rendered = Vec::with_capacity(declared.len());
    for field in declared {
        let mut matching = supplied.iter().filter(|item| item.field == field.id);
        let item = matching
            .next()
            .ok_or_else(|| invariant("wasm_executor.argument.field_absent"))?;
        if matching.next().is_some() {
            return Err(invariant("wasm_executor.argument.field_duplicate"));
        }
        let expression = render_value(program, &field.ty, &item.value, prelude, next)?;
        rendered.push(format!("{}: {expression}", field.name));
    }
    Ok(rendered.join(", "))
}

#[allow(clippy::too_many_arguments)]
fn run_through_injected_driver(
    host: Option<&WasmStageHost>,
    binding: &WasmTargetBinding<'_>,
    program: &hir::ResolvedProgram,
    entry: &ResolvedFunction,
    arguments: &[RetainedValue],
    max_steps: usize,
    cancellation: Option<&AgentCancellation>,
    meter: Option<&WasmMeter<'_>>,
) -> Result<RetainedCallEvaluation, Diagnostic> {
    let plan = ResultPlan::derive(program, &entry.return_type)?;

    let mut prelude = String::new();
    let mut next = 0usize;
    let mut call_args = Vec::with_capacity(arguments.len());
    for (index, (parameter, argument)) in entry.params.iter().zip(arguments).enumerate() {
        let expression = render_value(program, &parameter.ty, argument, &mut prelude, &mut next)?;
        // Every argument is bound to a local before the call, so a borrowed
        // aggregate parameter receives a place rather than a temporary --
        // the same shape the hand-written parity wrappers use.
        prelude.push_str(&format!("    let spx_arg{index} = {expression};\n"));
        call_args.push(format!("spx_arg{index}"));
    }
    let call = format!("{}({})", entry.name, call_args.join(", "));

    let drivers = drivers_for(&plan);
    let mut injected = String::from("\n");
    if drivers
        .iter()
        .any(|driver| matches!(driver.projection, Projection::IndexedBytes | Projection::U8))
    {
        injected.push_str(BYTE_HELPERS);
    }
    for driver in &drivers {
        injected.push_str(&format!(
            "\n@id(\"{}\")\n{}\n{{\n",
            driver.id,
            driver.projection.signature(&driver.name)
        ));
        injected.push_str(&prelude);
        injected.push_str(&format!("    let spx_call = {call};\n"));
        injected.push_str(&driver.tail);
        injected.push_str("}\n");
    }

    let extended = format!("{}{}", binding.source, injected);
    let parsed = crate::check(
        &extended,
        Path::new("agent-lifecycle-wasm-stage-driver.spx"),
    )
    .map_err(|_| invariant("wasm_executor.driver.check"))?;
    let resolved = hir::resolve(&parsed).map_err(|_| invariant("wasm_executor.driver.resolve"))?;
    hir::validate(&resolved).map_err(|_| invariant("wasm_executor.driver.validate"))?;

    // The re-resolved module must still BE the module this executor was
    // handed: driver text is appended, so every earlier declaration keeps its
    // byte offsets, spans and persistent `@id`. Asserted, never assumed.
    let reresolved_entry = resolved
        .functions
        .iter()
        .find(|function| function.id == entry.id)
        .ok_or_else(|| invariant("wasm_executor.driver.entry_absent"))?;
    if reresolved_entry != entry {
        return Err(invariant("wasm_executor.driver.entry_diverged"));
    }

    let mut selected = drivers
        .iter()
        .map(|driver| driver.id.clone())
        .collect::<Vec<_>>();
    // Public descriptors require lexical export identity order: `.10` sorts
    // before `.2`. Canonicalize only that selection, leaving driver, call and
    // decoded-leaf order untouched. This never changes a cleanup plan.
    selected.sort();
    let calls = drivers
        .iter()
        .map(|driver| match driver.projection {
            Projection::I64 | Projection::Usize | Projection::U8 => {
                format!("String(api.functions['{}']())", driver.id)
            }
            Projection::Bool => format!("(api.functions['{}']() ? 'true' : 'false')", driver.id),
            Projection::OwnedBytes => format!("api.functions['{}']()", driver.id),
            // Reads the variant case's byte payload one byte at a time until
            // the module reports `-1` (past the end). The cap is a
            // fail-closed bound, not a silent truncation: exceeding it throws
            // and the whole dispatch is refused. Reassembled into a real
            // `Uint8Array`, not a hex string, so the shared `stage()`
            // observer below classifies it exactly like an `OwnedBytes`
            // driver's whole-value return -- the same `settled_owned_bytes`
            // receipt tag, because it is the same real settlement, just
            // reached through more Wasm calls.
            Projection::IndexedBytes => {
                let call = format!("api.functions['{}'](BigInt(i))", driver.id);
                // Each byte (including the sentinel) re-executes the pure
                // stage. Meter each physical call independently and require
                // identical receipts, just like the other projections.
                let call = if meter.is_some() {
                    format!("semanticProjection(() => {call})")
                } else {
                    call
                };
                format!(
                    "(() => {{ const bytes = []; for (let i = 0; ; i += 1) {{ \
                     if (i > {BYTE_STREAM_CAP}) throw new Error('indexed byte stream cap'); \
                     const byte = {call}; \
                     if (byte < 0n) break; \
                     bytes.push(Number(byte)); }} return Uint8Array.from(bytes); }})()"
                )
            }
        })
        .collect::<Vec<_>>();
    let invoked = drivers
        .iter()
        .map(|driver| driver.id.clone())
        .collect::<Vec<_>>();
    let lines = match build_and_drive(
        host.ok_or_else(|| invariant("wasm_executor.process.host_unavailable"))?,
        binding,
        &resolved,
        &selected,
        &invoked,
        &calls,
        drivers
            .iter()
            .filter(|driver| {
                matches!(
                    driver.projection,
                    Projection::OwnedBytes | Projection::IndexedBytes
                )
            })
            .count(),
        cancellation,
        meter,
    )? {
        NodeStageRun::LanguageFailure(status) => {
            return Ok(evaluation(
                entry,
                RetainedCallOutcome::LanguageFailure(status),
                max_steps,
                Vec::new(),
            ));
        }
        NodeStageRun::FuelExhausted => {
            return Ok(evaluation(
                entry,
                RetainedCallOutcome::FuelExhausted,
                max_steps,
                Vec::new(),
            ));
        }
        NodeStageRun::CallDepthExceeded => {
            return Ok(evaluation(
                entry,
                RetainedCallOutcome::CallDepthExceeded,
                max_steps,
                Vec::new(),
            ));
        }
        NodeStageRun::Returned(values) => values,
    };
    let mut leaves = Vec::with_capacity(drivers.len());
    let mut cleanup_events = Vec::new();
    for (driver, row) in drivers.iter().zip(&lines) {
        // Both projections settle a real owned Bytes copy at the Wasm
        // boundary and are strictly parsed as the same `settled_owned_bytes`
        // receipt tag: a Record field returns its whole owned value
        // directly (`OwnedBytes`); a variant case's Bytes leaf cannot leave
        // a `match own` arm as an aggregate (SPX-T216/SPX-T258), so it is
        // read back and reassembled one byte at a time (`IndexedBytes`) --
        // more Wasm calls, the same real settlement. `Record` has no case
        // selection, so every `OwnedBytes` driver's settlement below is one
        // real output leaf; a `Variant`'s settled leaves are instead counted
        // once the selected case is known, so a driver built for a case the
        // call did not take is never counted twice or spuriously.
        let expected_owned = matches!(
            driver.projection,
            Projection::OwnedBytes | Projection::IndexedBytes
        );
        row.require_projection(expected_owned)?;
        if driver.projection == Projection::OwnedBytes {
            cleanup_events.push(OwnedDataCleanupEvent::CopyOutAndSettleBytes);
        }
        let line = &row.text;
        leaves.push(match driver.projection {
            Projection::I64 => RetainedValue::I64(
                line.trim()
                    .parse()
                    .map_err(|_| invariant("wasm_executor.decode.scalar"))?,
            ),
            Projection::Bool => RetainedValue::Bool(match line.trim() {
                "false" => false,
                "true" => true,
                _ => return Err(invariant("wasm_executor.decode.bool")),
            }),
            Projection::Usize => RetainedValue::Usize(
                line.trim()
                    .parse()
                    .map_err(|_| invariant("wasm_executor.decode.usize"))?,
            ),
            Projection::U8 => RetainedValue::U8(
                line.trim()
                    .parse()
                    .map_err(|_| invariant("wasm_executor.decode.u8"))?,
            ),
            Projection::OwnedBytes | Projection::IndexedBytes => {
                RetainedValue::Bytes(decode_hex(line.trim())?)
            }
        });
    }

    let outcome = match plan {
        ResultPlan::Record { record, fields } => {
            RetainedCallOutcome::Returned(RetainedValue::Record(RetainedRecord {
                record,
                fields: fields
                    .into_iter()
                    .zip(leaves)
                    .map(|(field, value)| RetainedField {
                        field: field.field,
                        value,
                    })
                    .collect(),
            }))
        }
        ResultPlan::Variant { variant, cases, .. } => {
            let RetainedValue::I64(tag) = leaves[0] else {
                return Err(invariant("wasm_executor.decode.tag"));
            };
            let index = usize::try_from(tag).map_err(|_| invariant("wasm_executor.decode.tag"))?;
            let case = cases
                .get(index)
                .ok_or_else(|| invariant("wasm_executor.decode.tag"))?;
            // Driver 0 is the tag; the remaining drivers are laid out case by
            // case in declaration order, so the selected case's own leaves
            // start after every earlier case's.
            let start = 1 + cases
                .iter()
                .take(index)
                .map(|item| item.fields.len())
                .sum::<usize>();
            let mut fields = Vec::with_capacity(case.fields.len());
            for (position, field) in case.fields.iter().enumerate() {
                fields.push(RetainedField {
                    field: field.field.clone(),
                    value: leaves
                        .get(start + position)
                        .ok_or_else(|| invariant("wasm_executor.decode.arity"))?
                        .clone(),
                });
                // Only the selected case's own Bytes leaf was ever bound and
                // consumed by the arm that actually executed; a driver built
                // for a case the call did not take reads its designated
                // field's dummy "past the end" sentinel and never calls the
                // owned-consuming helper, so it settles nothing to report.
                if field.leaf == Leaf::Bytes {
                    cleanup_events.push(OwnedDataCleanupEvent::CopyOutAndSettleBytes);
                }
            }
            RetainedCallOutcome::Returned(RetainedValue::Variant(RetainedVariant {
                variant,
                case: case.case.clone(),
                fields,
            }))
        }
    };
    Ok(evaluation(entry, outcome, max_steps, cleanup_events))
}

fn decode_hex(hex: &str) -> Result<Vec<u8>, Diagnostic> {
    if hex.len() % 2 != 0 {
        return Err(invariant("wasm_executor.decode.bytes"));
    }
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    let mut index = 0;
    while index < hex.len() {
        bytes.push(
            u8::from_str_radix(&hex[index..index + 2], 16)
                .map_err(|_| invariant("wasm_executor.decode.bytes"))?,
        );
        index += 2;
    }
    Ok(bytes)
}

// ---------------------------------------------------------------------------
// The shared build-and-run path: one owned-data package, one Node process.
// ---------------------------------------------------------------------------

fn node_output_budget(
    calls: usize,
    owned_byte_projections: usize,
    metered: bool,
) -> Result<usize, Diagnostic> {
    if !metered {
        // Preserve the legacy pooled allowance of every unmetered route.
        return calls
            .checked_mul(MAX_NODE_OUTCOME_ROW_BYTES)
            .filter(|bytes| *bytes > 0 && *bytes <= MAX_NODE_STDOUT_BYTES)
            .ok_or_else(|| invariant("wasm_executor.process.output_budget"));
    }
    calls
        .checked_sub(owned_byte_projections)
        .and_then(|count| count.checked_mul(MAX_NODE_SCALAR_OUTCOME_ROW_BYTES))
        .and_then(|bytes| {
            owned_byte_projections
                .checked_mul(MAX_NODE_OUTCOME_ROW_BYTES)
                .and_then(|owned| bytes.checked_add(owned))
        })
        .and_then(|bytes| {
            calls
                .checked_mul(if metered { MAX_SEMANTIC_ROW_BYTES } else { 0 })
                .and_then(|semantic| bytes.checked_add(semantic))
        })
        .filter(|bytes| *bytes > 0 && *bytes <= MAX_NODE_STDOUT_BYTES)
        .ok_or_else(|| invariant("wasm_executor.process.output_budget"))
}

#[allow(clippy::too_many_arguments)]
fn build_and_drive(
    host: &WasmStageHost,
    binding: &WasmTargetBinding<'_>,
    program: &hir::ResolvedProgram,
    selected: &[String],
    invocations: &[String],
    calls: &[String],
    owned_byte_projections: usize,
    cancellation: Option<&AgentCancellation>,
    meter: Option<&WasmMeter<'_>>,
) -> Result<NodeStageRun, Diagnostic> {
    if invocations.len() != calls.len() {
        return Err(invariant("wasm_executor.binding.invocation_arity"));
    }
    let output_budget = node_output_budget(calls.len(), owned_byte_projections, meter.is_some())?;
    if cancellation.is_some_and(AgentCancellation::is_cancelled) {
        return Err(invariant("wasm_executor.process.cancelled"));
    }
    let descriptor = project::derive_public_api_descriptor(program, selected, binding.subject())
        .map_err(|_| invariant("wasm_executor.descriptor"))?;
    let artifact = binding.bind_artifact(&descriptor, selected)?;
    artifact.verify_invocations(invocations)?;
    // A metered package is built and replay-verified under one scoped
    // metering selection, so its replay re-derives the same metered module.
    let prepare = || {
        let build = project::prepare_owned_data_npm_build(
            program,
            &descriptor,
            "agent-lifecycle-wasm-stage-executor",
            "0.1.0",
            40 * 1024 * 1024,
        )
        .map_err(|_| invariant("wasm_executor.npm_build"))?;
        artifact.verify_build(&build, &descriptor)?;
        Ok(build)
    };
    let build = match meter {
        Some(meter) => crate::wasm::with_semantic_metering(meter.metering(), prepare),
        None => prepare(),
    }?;
    let envelope: serde_json::Value =
        serde_json::from_str(build.envelope()).map_err(|_| invariant("wasm_executor.envelope"))?;

    let mut workspace = WasmStageWorkspace::create()?;
    let outcome = drive_node(
        host,
        &envelope,
        calls,
        &mut workspace,
        cancellation,
        output_budget,
        meter,
    );
    let cleanup = workspace.cleanup();
    let outcome = outcome?;
    cleanup?;
    match meter {
        Some(meter) => meter.decode(&outcome, calls.len()),
        None => decode_node_outcomes(&outcome, calls.len()),
    }
}

fn drive_node(
    host: &WasmStageHost,
    envelope: &serde_json::Value,
    calls: &[String],
    workspace: &mut WasmStageWorkspace,
    cancellation: Option<&AgentCancellation>,
    output_budget: usize,
    meter: Option<&WasmMeter<'_>>,
) -> Result<String, Diagnostic> {
    for row in envelope["artifacts"]
        .as_array()
        .ok_or_else(|| invariant("wasm_executor.envelope.artifacts"))?
    {
        let hex = row["hex"]
            .as_str()
            .ok_or_else(|| invariant("wasm_executor.envelope.hex"))?;
        let path = row["path"]
            .as_str()
            .ok_or_else(|| invariant("wasm_executor.envelope.path"))?;
        workspace.write(Path::new(path), &decode_hex(hex)?)?;
    }
    let call_thunks = calls
        .iter()
        .map(|call| format!("() => {call}"))
        .collect::<Vec<_>>()
        .join(",\n");
    let observe = match meter {
        Some(meter) => meter.observe_source(&call_thunks),
        None => format!(
            r#"import fs from 'node:fs';
import instantiate from './semaprax.bindings.js';
const wasm = new Uint8Array(fs.readFileSync(new URL('./app.wasm', import.meta.url)));
const api = await instantiate(wasm);
const out = [];
const normalizeFailure = error => {{
  if(error?.semapraxSemantic !== true) throw error;
  let raw = error.status;
  if(raw === undefined) raw = error.domain === 'semaprax.arithmetic.v1' ? error.code : error.domain === 'semaprax.contract.v1' ? error.code + 8 : NaN;
  if(!Number.isInteger(raw) || raw < 1 || raw > 10) throw new Error('SEMAPRAX stage status');
  const domain = raw <= 8 ? 'semaprax.arithmetic.v1' : 'semaprax.contract.v1';
  const code = raw <= 8 ? raw : raw - 8;
  if(error.status !== undefined && error.status !== raw || error.code !== undefined && (error.code !== code || error.domain !== domain)) throw new Error('SEMAPRAX stage status mismatch');
  return {{schema:'semaprax.agent-wasm-stage-outcome.v2',kind:'language_failure',raw_status:raw,status:{{schema:'semaprax.status.v1',domain_id:domain,code,class:raw<=8?'arithmetic':'contract',retryable:false}}}};
}};
const stage = call => {{try {{
  const value = call();
  if(value instanceof Uint8Array) return {{schema:'semaprax.agent-wasm-stage-outcome.v2',kind:'settled_owned_bytes',value:Array.from(value,b=>b.toString(16).padStart(2,'0')).join(''),byte_length:value.byteLength}};
  return {{schema:'semaprax.agent-wasm-stage-outcome.v2',kind:'returned',value:String(value)}};
}} catch(error) {{return normalizeFailure(error)}}}};
const calls = [
{call_thunks}
];
for (const call of calls) {{
  const result = stage(call);
  out.push(result);
  // A checked failure settles this invocation. Do not invoke another
  // projection against that instance; the one authenticated status settles
  // the whole retained call without depending on later arena state.
  if(result.kind === 'language_failure') break;
}}
process.stdout.write(out.map(value => JSON.stringify(value) + '\n').join(''));
"#
        ),
    };
    workspace.write(Path::new("observe.mjs"), observe.as_bytes())?;
    run_node_process(host, workspace, cancellation, output_budget)
}

#[cfg(test)]
#[path = "wasm_executor_tests.rs"]
mod target_binding_tests;
