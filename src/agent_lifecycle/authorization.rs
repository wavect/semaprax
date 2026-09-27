//! The opaque, one-use authorization value, and the only place it is minted.
//!
//! [`Authorized`] has no public constructor, no `Clone`, no `Copy`, no
//! `Default`, and no `From`. Its fields are private to this module, so a
//! struct literal cannot name them from anywhere else in the crate, let alone
//! from a consumer. The only function that builds one is the private `mint`
//! below, and the only call to `mint` is inside `run_authorize_stage`.
//!
//! `run_authorize_stage` cannot be reached without an `AuthorizeStage`, which
//! only the lifecycle compiler's stage
//! binder constructs, and only after the authorize role has passed identity,
//! signature, ownership, effect and decision-shape validation. It additionally
//! requires the retained product it dispatches to name that exact validated
//! function, and it mints only when the evaluated stage returns the validated
//! grant case of the validated decision variant.
//!
//! Therefore `observe`, `reduce`, the model's proposal, and any other caller
//! have no route to an `Authorized`: a proposal is data, and a deterministic
//! stage that is not the authorize role never reaches the mint. The value is
//! consumed by move at the effect boundary, so one authorization admits at
//! most one effect.

use sha2::{Digest, Sha256};

use crate::agent_runtime::AgentCancellation;
use crate::diagnostic::Diagnostic;
use crate::hir;
use crate::interpreter::retained_call::{
    evaluate_retained_call, evaluate_retained_call_metered, PreparedRetainedCall,
    RetainedCallEvaluation, RetainedCallOutcome, RetainedValue,
};

use super::stages::AuthorizeStage;
use super::{encode_value, StageRecord};

mod native_executor;
mod semantic_work;
/// Target-neutral model/effect boundary for explicitly injected host adapters.
/// Grant construction and dispatch remain crate-owned so callers cannot mint
/// or spend authorization outside the lifecycle kernel.
pub mod target_protocol;
pub(in crate::agent_lifecycle) mod wasm_executor;

use native_executor::NativeStageExecutor;
pub(in crate::agent_lifecycle) use native_executor::NativeStageHost;
pub use semantic_work::StageSemanticProfile;
use wasm_executor::WasmStageExecutor;
pub use wasm_executor::WasmStageHost;

/// Serializes every test that drives a real subprocess through the native
/// (`native_executor`) or Wasm (`wasm_executor::process`) stage executors.
///
/// Both executors hand their compile/run subprocesses to the shared
/// process-provider boundary under a fixed, non-negotiable production
/// deadline (`native_executor.process.deadline` at 2 s,
/// `wasm_executor.process.deadline` at 2 s). That deadline is correct in
/// production, where one stage call runs alone, but a full parallel `--lib`
/// run can spawn dozens of these compile+run subprocesses at once; under
/// that contention a subprocess can legitimately need more than 2 s of
/// wall-clock scheduling even though it does negligible work once it runs,
/// and the test then reports the same diagnostic a genuine host hang would.
/// Rather than loosen the deadline, every test-only subprocess spawn takes
/// this lock first, so at most one such subprocess is ever in flight and the
/// deadline is measured against real work again, not scheduler queueing.
#[cfg(test)]
static SUBPROCESS_TEST_SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
pub(in crate::agent_lifecycle) fn subprocess_test_serial() -> std::sync::MutexGuard<'static, ()> {
    SUBPROCESS_TEST_SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

const BINDING_DOMAIN: &[u8] = b"semaprax.agent-lifecycle.authorization.v1\0";

/// One opaque, one-use authorization.
///
/// It carries the binding of the exact policy, state and proposal the
/// validated authorize stage granted against. It is not a boolean, and it is
/// not a hash a caller can hand back: the only way to obtain one is for the
/// validated authorize stage to return its grant case, and the only way to
/// spend one is to move it into the effect boundary, which consumes it.
pub struct Authorized {
    binding: String,
    budget: i64,
    seal: Vec<u8>,
}

impl Authorized {
    /// The domain-separated binding of the exact policy, state and proposal.
    #[must_use]
    pub fn binding(&self) -> &str {
        &self.binding
    }

    /// The budget the validated grant carried.
    ///
    /// Crate-internal and read-only: the durable journal records it as a fact
    /// about one intent. It is never an input to minting, and no API accepts
    /// it in place of an authorization.
    pub(in crate::agent_lifecycle) const fn granted_budget(&self) -> i64 {
        self.budget
    }

    /// Read-only access for the lifecycle kernel's final binding check before
    /// it moves this value into either the ordinary or target effect boundary.
    /// The seal is never copied into a target request or evidence document.
    pub(in crate::agent_lifecycle) fn seal(&self) -> &[u8] {
        &self.seal
    }

    /// Spends the authorization. The value is moved, so it authorizes at most
    /// one effect.
    #[must_use]
    pub fn consume(self) -> AuthorizedRequest {
        AuthorizedRequest {
            binding: self.binding,
            budget: self.budget,
            seal: self.seal,
        }
    }
}

/// The spent authorization handed to the single injected read operation.
pub struct AuthorizedRequest {
    binding: String,
    budget: i64,
    seal: Vec<u8>,
}

impl AuthorizedRequest {
    #[must_use]
    pub fn binding(&self) -> &str {
        &self.binding
    }

    /// The budget the authorize stage itself granted, read from the validated
    /// grant case rather than from the proposal.
    #[must_use]
    pub const fn budget(&self) -> i64 {
        self.budget
    }

    /// The owned seal the authorize stage constructed inside the program.
    #[must_use]
    pub fn seal(&self) -> &[u8] {
        &self.seal
    }
}

/// The closed result of running the authorizing transition.
pub(super) enum AuthorizationOutcome {
    Granted(Authorized),
    Refused(i64),
    /// The stage did not decide: a contract failure, a fuel or depth limit, or
    /// an impossible post-verify shape. No authorization exists.
    Undecided(&'static str),
}

/// The domain-separated binding of one authorization.
///
/// It is deterministic in its inputs, so the same policy, state, proposal and
/// seal replay to the same value. Reproducing the string grants nothing: no
/// API accepts one in place of an [`Authorized`].
pub(super) fn binding(
    policy_digest: &str,
    state: &RetainedValue,
    proposal_canonical: &str,
    grant_case: &hir::DeclarationId,
    seal: &[u8],
) -> String {
    let mut hash = Sha256::new();
    hash.update(BINDING_DOMAIN);
    hash.update(policy_digest.as_bytes());
    hash.update([0]);
    hash.update(encode_value(state).as_bytes());
    hash.update([0]);
    hash.update(proposal_canonical.as_bytes());
    hash.update([0]);
    hash.update(grant_case.as_str().as_bytes());
    hash.update([0]);
    hash.update(seal);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

/// The single mint site of the entire crate.
fn mint(binding: String, budget: i64, seal: Vec<u8>) -> Authorized {
    Authorized {
        binding,
        budget,
        seal,
    }
}

/// Runs the validated authorizing transition and, only on its validated grant
/// case, mints the authorization bound to that exact policy, state and
/// proposal.
pub(super) fn run_authorize_stage(
    program: &hir::ResolvedProgram,
    stage: &AuthorizeStage,
    arguments: &[RetainedValue],
    max_steps: usize,
    policy_digest: &str,
    state: &RetainedValue,
    proposal_canonical: &str,
) -> Result<(AuthorizationOutcome, StageRecord), Vec<Diagnostic>> {
    run_authorize_stage_on(
        StageBackend::Interpreter,
        program,
        stage,
        arguments,
        max_steps,
        policy_digest,
        state,
        proposal_canonical,
    )
}

/// Execute the same checked grant transition on an explicitly selected backend.
/// Decoding, binding and the single mint site are shared by every selection.
#[allow(clippy::too_many_arguments)]
pub(super) fn run_authorize_stage_on(
    backend: StageBackend<'_>,
    program: &hir::ResolvedProgram,
    stage: &AuthorizeStage,
    arguments: &[RetainedValue],
    max_steps: usize,
    policy_digest: &str,
    state: &RetainedValue,
    proposal_canonical: &str,
) -> Result<(AuthorizationOutcome, StageRecord), Vec<Diagnostic>> {
    run_authorize_stage_on_cancellable(
        backend,
        program,
        stage,
        arguments,
        max_steps,
        policy_digest,
        state,
        proposal_canonical,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_authorize_stage_on_cancellable(
    backend: StageBackend<'_>,
    program: &hir::ResolvedProgram,
    stage: &AuthorizeStage,
    arguments: &[RetainedValue],
    max_steps: usize,
    policy_digest: &str,
    state: &RetainedValue,
    proposal_canonical: &str,
    cancellation: Option<&AgentCancellation>,
) -> Result<(AuthorizationOutcome, StageRecord), Vec<Diagnostic>> {
    let prepared = stage.stage().prepared();
    if prepared.function_id() != stage.stage().function_id() {
        return Err(vec![super::stages::invariant(
            "authorize.retained_call.identity",
        )]);
    }
    let evaluation = dispatch_on_admitted(
        backend,
        program,
        prepared,
        arguments,
        max_steps,
        cancellation,
    )?;
    if evaluation.function_id.as_str() != stage.stage().function_id() {
        return Err(vec![super::stages::invariant(
            "authorize.retained_call.dispatch",
        )]);
    }
    let record = StageRecord::of(stage.stage(), &evaluation);
    let outcome = match evaluation.outcome {
        RetainedCallOutcome::Returned(RetainedValue::Variant(decision)) => {
            if decision.variant != *stage.decision_type() {
                AuthorizationOutcome::Undecided("decision_identity")
            } else if decision.case == *stage.grant_case() {
                let seal = decision
                    .fields
                    .iter()
                    .find(|field| field.field == *stage.grant_seal_field())
                    .and_then(|field| match &field.value {
                        RetainedValue::Bytes(bytes) => Some(bytes.clone()),
                        _ => None,
                    });
                let budget = decision
                    .fields
                    .iter()
                    .find(|field| field.field == *stage.grant_budget_field())
                    .and_then(|field| match field.value {
                        RetainedValue::I64(value) => Some(value),
                        _ => None,
                    });
                match (seal, budget) {
                    (Some(seal), Some(budget)) => {
                        let binding = binding(
                            policy_digest,
                            state,
                            proposal_canonical,
                            &decision.case,
                            &seal,
                        );
                        AuthorizationOutcome::Granted(mint(binding, budget, seal))
                    }
                    _ => AuthorizationOutcome::Undecided("grant_payload"),
                }
            } else if decision.case == *stage.refuse_case() {
                let code = decision
                    .fields
                    .iter()
                    .find(|field| field.field == *stage.refuse_code_field())
                    .and_then(|field| match field.value {
                        RetainedValue::I64(value) => Some(value),
                        _ => None,
                    });
                match code {
                    Some(code) => AuthorizationOutcome::Refused(code),
                    None => AuthorizationOutcome::Undecided("refusal_payload"),
                }
            } else {
                AuthorizationOutcome::Undecided("decision_case")
            }
        }
        RetainedCallOutcome::Returned(_) => AuthorizationOutcome::Undecided("decision_carrier"),
        RetainedCallOutcome::LanguageFailure(_) => AuthorizationOutcome::Undecided("contract"),
        RetainedCallOutcome::FuelExhausted => AuthorizationOutcome::Undecided("fuel"),
        RetainedCallOutcome::CallDepthExceeded => AuthorizationOutcome::Undecided("depth"),
        RetainedCallOutcome::GuardError(_) => AuthorizationOutcome::Undecided("guard"),
    };
    Ok((outcome, record))
}

// ---------------------------------------------------------------------------
// The sealed executor seam.
// ---------------------------------------------------------------------------
//
// Before this seam existed, every stage evaluation still funneled through
// the single function `evaluate_retained_call`, but through four
// independent, uncoordinated call sites: this module's own
// `run_authorize_stage` above, `rich_stage.rs`'s authorize and reduce
// dispatch (two sites), and `CompiledAgentLifecycle::evaluate` in the parent
// module file (`agent_lifecycle.rs`). A fifth backend could have been wired
// into any one of them without the others -- or a reviewer -- noticing.
//
// `StageExecutor` closes that for every site this crate's file lease can
// reach: it is the one trait a backend implements to run a bound stage's
// prepared body, it is sealed so no second implementation can appear, and
// dispatching through it requires an explicit, unforgeable
// `ExecutionAuthority` value rather than relying on being called from the
// "right" module. `run_authorize_stage` above, and every stage dispatch in
// `durable.rs`, `iterative/driver.rs`, `iterative/driver/live.rs` and
// `rich_stage.rs`, now call `dispatch` below instead of
// `evaluate_retained_call` directly.
//
// `CompiledAgentLifecycle::evaluate` in `agent_lifecycle.rs` is outside this
// module's own file and still calls `evaluate_retained_call` directly; nothing
// in this crate's `src/agent_lifecycle/**` file lease can rewrite that
// method's body, so that one remaining call site is a residual, reported gap
// rather than a closed one. See the change notes for the exact one-line edit
// that would close it.

mod sealed {
    /// Closed over this module and its descendants. `sealed` is a private
    /// module, so only `authorization.rs` itself and the submodules it
    /// declares (`native_executor`, `wasm_executor`) can name `Sealed`, and
    /// therefore only they can implement [`super::StageExecutor`] -- the
    /// standard Rust sealed-trait idiom, enforced by the compiler at the
    /// `impl` site, not by convention. Nothing outside this module tree,
    /// including every other module in this crate's own file lease, can
    /// name it.
    pub trait Sealed {}
}

/// Explicit, unforgeable authority to dispatch one stage's prepared body to
/// a [`StageExecutor`].
///
/// `ExecutionAuthority` has no public constructor, no `Clone`, no `Copy`,
/// and no `Default` -- the same no-forging shape [`Authorized`] already
/// uses in this module. A caller cannot build one from a struct literal
/// (its field is private) and cannot manufacture one from nothing; the only
/// route is [`ExecutionAuthority::grant`].
pub struct ExecutionAuthority(());

impl ExecutionAuthority {
    /// Grants execution authority for one dispatch.
    ///
    /// Restricted to this crate's agent lifecycle runtime: nothing outside
    /// this module tree -- no unrelated module, no external crate -- can
    /// call this, so nothing outside the tree can even attempt to drive a
    /// [`StageExecutor`], whether or not it could otherwise obtain one.
    pub(super) fn grant() -> Self {
        ExecutionAuthority(())
    }
}

/// The sealed executor seam for dispatching one Agent stage's prepared
/// retained-call body for execution.
///
/// Every backend capable of running a bound stage implements this trait --
/// today, exactly three: the interpreter ([`InterpreterStageExecutor`]),
/// native C11 ([`native_executor::NativeStageExecutor`], #142), and Core
/// Wasm ([`wasm_executor::WasmStageExecutor`], #143). `StageExecutor` is
/// `pub` so its contract is inspectable from outside the crate, but it
/// cannot be *implemented* from outside this module's own tree
/// (`authorization.rs` and its declared submodules): the supertrait bound
/// requires `sealed::Sealed`, and `sealed` is a private module nested here,
/// so nothing else can name it. This is checked by the compiler at the `impl`
/// site, not by convention:
///
/// ```compile_fail
/// struct RogueExecutor;
///
/// impl semaprax::agent_lifecycle::authorization::StageExecutor for RogueExecutor {
///     fn execute(
///         &self,
///         _authority: semaprax::agent_lifecycle::authorization::ExecutionAuthority,
///         _program: &semaprax::hir::ResolvedProgram,
///         _prepared: &semaprax::interpreter::retained_call::PreparedRetainedCall,
///         _arguments: &[semaprax::interpreter::retained_call::RetainedValue],
///         _max_steps: usize,
///         _cancellation: Option<&semaprax::agent_runtime::AgentCancellation>,
///     ) -> Result<
///         semaprax::interpreter::retained_call::RetainedCallEvaluation,
///         Vec<semaprax::diagnostic::Diagnostic>,
///     > {
///         unimplemented!()
///     }
///
///     fn execute_metered(
///         &self,
///         _authority: semaprax::agent_lifecycle::authorization::ExecutionAuthority,
///         _program: &semaprax::hir::ResolvedProgram,
///         _prepared: &semaprax::interpreter::retained_call::PreparedRetainedCall,
///         _arguments: &[semaprax::interpreter::retained_call::RetainedValue],
///         _max_steps: usize,
///         _profile: &semaprax::agent_lifecycle::authorization::StageSemanticProfile,
///         _cancellation: Option<&semaprax::agent_runtime::AgentCancellation>,
///     ) -> Result<
///         semaprax::interpreter::retained_call::RetainedCallEvaluation,
///         Vec<semaprax::diagnostic::Diagnostic>,
///     > {
///         unimplemented!()
///     }
/// }
/// ```
///
/// Dispatching through the one real implementation still requires a live
/// [`ExecutionAuthority`], passed by value as an ordinary parameter: the
/// authority to execute is data a caller must hold, not an ambient property
/// of which function happens to be calling.
pub trait StageExecutor: sealed::Sealed {
    /// Executes one prepared stage body.
    fn execute(
        &self,
        authority: ExecutionAuthority,
        program: &hir::ResolvedProgram,
        prepared: &PreparedRetainedCall,
        arguments: &[RetainedValue],
        max_steps: usize,
        cancellation: Option<&AgentCancellation>,
    ) -> Result<RetainedCallEvaluation, Vec<Diagnostic>>;

    /// Executes one prepared stage body under an admitted Agent Stage
    /// Semantic Work v1 profile. The backend must charge the profile's
    /// semantic points itself and report them; it may not fall back to an
    /// unmetered or interpreter execution.
    #[allow(clippy::too_many_arguments)]
    fn execute_metered(
        &self,
        authority: ExecutionAuthority,
        program: &hir::ResolvedProgram,
        prepared: &PreparedRetainedCall,
        arguments: &[RetainedValue],
        max_steps: usize,
        profile: &StageSemanticProfile,
        cancellation: Option<&AgentCancellation>,
    ) -> Result<RetainedCallEvaluation, Vec<Diagnostic>>;
}

/// The interpreter-backed stage executor.
///
/// The default, and today the only, backend any production call site in
/// this crate's file lease selects. [`NativeStageExecutor`] (#142) and
/// [`WasmStageExecutor`] (#143) are the seam's other two implementors --
/// reachable only through [`dispatch_on`], never through a second,
/// uncoordinated call into `evaluate_retained_call`.
pub(super) struct InterpreterStageExecutor;

impl sealed::Sealed for InterpreterStageExecutor {}

impl StageExecutor for InterpreterStageExecutor {
    fn execute(
        &self,
        _authority: ExecutionAuthority,
        program: &hir::ResolvedProgram,
        prepared: &PreparedRetainedCall,
        arguments: &[RetainedValue],
        max_steps: usize,
        cancellation: Option<&AgentCancellation>,
    ) -> Result<RetainedCallEvaluation, Vec<Diagnostic>> {
        if cancellation.is_some_and(AgentCancellation::is_cancelled) {
            return Err(vec![super::stages::invariant("stage_executor.cancelled")]);
        }
        evaluate_retained_call(program, prepared, arguments, max_steps)
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_metered(
        &self,
        _authority: ExecutionAuthority,
        program: &hir::ResolvedProgram,
        prepared: &PreparedRetainedCall,
        arguments: &[RetainedValue],
        max_steps: usize,
        profile: &StageSemanticProfile,
        cancellation: Option<&AgentCancellation>,
    ) -> Result<RetainedCallEvaluation, Vec<Diagnostic>> {
        if cancellation.is_some_and(AgentCancellation::is_cancelled) {
            return Err(vec![super::stages::invariant("stage_executor.cancelled")]);
        }
        evaluate_retained_call_metered(
            program,
            prepared,
            arguments,
            max_steps,
            profile.fuel_limit(),
        )
    }
}

/// Which [`StageExecutor`] one [`dispatch_on`] call selects.
///
/// This is data a caller must name explicitly -- never an ambient default
/// baked into a second function -- so every backend, including the two
/// added for #142/#143, is reachable through the exact same single call
/// point [`dispatch`] already was.
/// [`StageBackend::Native`] carries an explicitly injected held compiler
/// capability; the executor never resolves `clang` from PATH.
/// [`StageBackend::Wasm`] likewise carries an explicitly injected held Node
/// runtime capability plus the module source text its executor is allowed to
/// re-check and re-resolve when it must inject a projection driver
/// (#143/#182). That source is data the caller supplies -- the
/// lifecycle's own retained `.spx` text -- never something the executor
/// reads from the filesystem, so selecting the Wasm backend grants no
/// ambient authority the interpreter backend does not have.
#[derive(Clone, Copy)]
pub(super) enum StageBackend<'a> {
    Interpreter,
    Native {
        host: &'a NativeStageHost,
    },
    /// The same native C11 executor as [`StageBackend::Native`], compiled
    /// with an explicit `clang` optimization flag (e.g. `"-O2"`) instead of
    /// the production `-O0` default. Exists for cross-engine parity evidence
    /// (#182/#143): an optimizer is exactly where backend divergence hides,
    /// and no production call site selects this variant.
    NativeAtOptimization {
        host: &'a NativeStageHost,
        optimization: &'static str,
    },
    /// Test-only compatibility selector. Production callers must supply the
    /// explicit held runtime through [`StageBackend::WasmHeld`].
    #[cfg(test)]
    Wasm {
        source: &'a str,
    },
    WasmHeld {
        host: &'a WasmStageHost,
        source: &'a str,
    },
}

/// The single call point this module tree dispatches a bound stage's
/// prepared body through.
///
/// Every stage dispatch this crate's file lease can reach calls this (or
/// its `Interpreter`-selecting convenience wrapper [`dispatch`]) instead of
/// `evaluate_retained_call` directly, so a second, uncoordinated call site
/// into a backend cannot reappear silently within that lease: it would have
/// to show up as one more `StageExecutor` implementation, which `tests.rs`'s
/// `the_stage_executor_seam_has_exactly_one_implementation_and_one_dispatch_route`
/// pins at exactly the number this crate has actually reviewed and admitted
/// (three: interpreter, native, Wasm), rather than as one more scattered
/// call to `evaluate_retained_call` or a hand-rolled backend invocation.
pub(super) fn dispatch_on(
    backend: StageBackend<'_>,
    program: &hir::ResolvedProgram,
    prepared: &PreparedRetainedCall,
    arguments: &[RetainedValue],
    max_steps: usize,
) -> Result<RetainedCallEvaluation, Vec<Diagnostic>> {
    dispatch_on_admitted(backend, program, prepared, arguments, max_steps, None)
}

/// Dispatch after rechecking the caller's monotonic cancellation immediately
/// at the sealed executor boundary. Lifecycle callers use this route so a
/// cancellation racing their outer reservation cannot enter a compiler or
/// target process. The ordinary `dispatch_on` compatibility route remains
/// intentionally uncancellable for direct deterministic-stage tests.
pub(super) fn dispatch_on_cancellable(
    backend: StageBackend<'_>,
    program: &hir::ResolvedProgram,
    prepared: &PreparedRetainedCall,
    arguments: &[RetainedValue],
    max_steps: usize,
    cancellation: &AgentCancellation,
) -> Result<RetainedCallEvaluation, Vec<Diagnostic>> {
    dispatch_on_admitted(
        backend,
        program,
        prepared,
        arguments,
        max_steps,
        Some(cancellation),
    )
}

fn dispatch_on_admitted(
    backend: StageBackend<'_>,
    program: &hir::ResolvedProgram,
    prepared: &PreparedRetainedCall,
    arguments: &[RetainedValue],
    max_steps: usize,
    cancellation: Option<&AgentCancellation>,
) -> Result<RetainedCallEvaluation, Vec<Diagnostic>> {
    // The target executors do not implement the interpreter's instruction
    // counter, but they share its finite *admission* interval. Check it at
    // the sealed boundary, before a native compiler or Node process can be
    // admitted. Cancellation keeps its existing precedence: a cancellation
    // racing this boundary settles as cancellation rather than as a malformed
    // fuel request on every backend.
    if cancellation.is_some_and(AgentCancellation::is_cancelled) {
        return Err(vec![super::stages::invariant("stage_executor.cancelled")]);
    }
    crate::interpreter::retained_call::validate_step_limit(max_steps)?;
    let authority = ExecutionAuthority::grant();
    match backend {
        StageBackend::Interpreter => InterpreterStageExecutor.execute(
            authority,
            program,
            prepared,
            arguments,
            max_steps,
            cancellation,
        ),
        StageBackend::Native { host } => NativeStageExecutor::o0(host).execute(
            authority,
            program,
            prepared,
            arguments,
            max_steps,
            cancellation,
        ),
        StageBackend::NativeAtOptimization { host, optimization } => {
            NativeStageExecutor { host, optimization }.execute(
                authority,
                program,
                prepared,
                arguments,
                max_steps,
                cancellation,
            )
        }
        #[cfg(test)]
        StageBackend::Wasm { source } => WasmStageExecutor {
            host: super::tests::test_wasm_stage_host(),
            source,
        }
        .execute(
            authority,
            program,
            prepared,
            arguments,
            max_steps,
            cancellation,
        ),
        StageBackend::WasmHeld { host, source } => WasmStageExecutor {
            host: Some(host),
            source,
        }
        .execute(
            authority,
            program,
            prepared,
            arguments,
            max_steps,
            cancellation,
        ),
    }
}

/// Dispatch one stage under Agent Stage Semantic Work v1. Cancellation keeps
/// its precedence; the step interval and the metered profile are then admitted
/// before any compiler or Node process exists. Every selection meters the
/// same semantic points or refuses: no backend runs this request unmetered.
pub(super) fn dispatch_on_metered(
    backend: StageBackend<'_>,
    program: &hir::ResolvedProgram,
    prepared: &PreparedRetainedCall,
    arguments: &[RetainedValue],
    max_steps: usize,
    semantic_fuel_limit: u64,
    cancellation: Option<&AgentCancellation>,
) -> Result<RetainedCallEvaluation, Vec<Diagnostic>> {
    if cancellation.is_some_and(AgentCancellation::is_cancelled) {
        return Err(vec![super::stages::invariant("stage_executor.cancelled")]);
    }
    crate::interpreter::retained_call::validate_step_limit(max_steps)?;
    let profile = StageSemanticProfile::admit(program, prepared.function_id(), semantic_fuel_limit)
        .map_err(|error| vec![error])?;
    let authority = ExecutionAuthority::grant();
    let evaluation = match backend {
        StageBackend::Interpreter => InterpreterStageExecutor.execute_metered(
            authority,
            program,
            prepared,
            arguments,
            max_steps,
            &profile,
            cancellation,
        ),
        StageBackend::Native { host } => NativeStageExecutor::o0(host).execute_metered(
            authority,
            program,
            prepared,
            arguments,
            max_steps,
            &profile,
            cancellation,
        ),
        StageBackend::NativeAtOptimization { host, optimization } => {
            NativeStageExecutor { host, optimization }.execute_metered(
                authority,
                program,
                prepared,
                arguments,
                max_steps,
                &profile,
                cancellation,
            )
        }
        #[cfg(test)]
        StageBackend::Wasm { source } => WasmStageExecutor {
            host: super::tests::test_wasm_stage_host(),
            source,
        }
        .execute_metered(
            authority,
            program,
            prepared,
            arguments,
            max_steps,
            &profile,
            cancellation,
        ),
        StageBackend::WasmHeld { host, source } => WasmStageExecutor {
            host: Some(host),
            source,
        }
        .execute_metered(
            authority,
            program,
            prepared,
            arguments,
            max_steps,
            &profile,
            cancellation,
        ),
    }?;
    // Every backend must report the admitted limit it actually metered.
    match &evaluation.semantic_work {
        Some(work) if work.fuel_limit == Some(semantic_fuel_limit) => Ok(evaluation),
        _ => Err(vec![super::stages::invariant("semantic_work.unreported")]),
    }
}

/// Convenience wrapper over [`dispatch_on`] selecting [`StageBackend::Interpreter`],
/// the behavior every existing production call site in this crate's file
/// lease -- including the one remaining call site outside it,
/// `CompiledAgentLifecycle::evaluate` in `agent_lifecycle.rs` -- already
/// depends on unchanged.
pub(super) fn dispatch(
    program: &hir::ResolvedProgram,
    prepared: &PreparedRetainedCall,
    arguments: &[RetainedValue],
    max_steps: usize,
) -> Result<RetainedCallEvaluation, Vec<Diagnostic>> {
    dispatch_on(
        StageBackend::Interpreter,
        program,
        prepared,
        arguments,
        max_steps,
    )
}
