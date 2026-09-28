//! Checked feedback-driven source loop; shares the parent driver hooks.
use super::super::source_live;
use super::*;

mod kernel;
mod retained;

impl CompiledIterativeLifecycle {
    /// The live route. Shares initialize/observe/authorize/effect/reduce with
    /// [`Self::run_with_driver_initial`] exactly; the only difference is where
    /// each turn's proposal text comes from: a call to `source.propose`,
    /// carrying the real previous effect result, instead of indexing a
    /// predeclared slice by turn.
    ///
    /// A decode failure does not end the turn immediately: the source gets a
    /// bounded number of attempts (see [`MAX_PROPOSAL_ATTEMPTS`]), each one
    /// counted and producing no effect call, before the run ends as
    /// `ModelFailed`. Every other terminal condition — refusal, budget
    /// exhaustion, cancellation, and the reducer's own Complete/Suspend/Fail
    /// selection — is exactly the frozen route's, because this reuses the
    /// same authorize/effect/reduce calls on the same driver.
    pub(crate) fn run_with_driver_live(
        &self,
        task: &LifecycleTask,
        source: &mut dyn ProposalSource,
        driver: &mut dyn IterativeDriver,
        budget: IterativeBudget,
        cancellation: &AgentCancellation,
    ) -> Result<IterativeRun, DriverFailure> {
        self.run_with_driver_live_session(task, source, driver, budget, cancellation, None)
    }

    pub(crate) fn run_with_driver_live_session(
        &self,
        task: &LifecycleTask,
        source: &mut dyn ProposalSource,
        driver: &mut dyn IterativeDriver,
        budget: IterativeBudget,
        cancellation: &AgentCancellation,
        session: Option<&mut source_live::SourceExecutionSession<'_>>,
    ) -> Result<IterativeRun, DriverFailure> {
        self.run_with_driver_live_seed(
            task,
            source,
            driver,
            budget,
            cancellation,
            session,
            None,
            false,
        )
    }

    pub(crate) fn run_with_target_driver_live(
        &self,
        task: &LifecycleTask,
        source: &mut dyn ProposalSource,
        driver: &mut dyn IterativeDriver,
        budget: IterativeBudget,
        cancellation: &AgentCancellation,
    ) -> Result<IterativeRun, DriverFailure> {
        self.run_with_driver_live_seed_on(
            task,
            source,
            driver,
            budget,
            cancellation,
            None,
            None,
            true,
            authorization::StageBackend::Interpreter,
        )
    }

    /// Local parity-only target route. Production entry points retain the
    /// interpreter selection above; this accepts only the sealed executor
    /// selector and still uses the exact same live authorization/effect loop.
    pub(in crate::agent_lifecycle) fn run_with_target_driver_live_on(
        &self,
        task: &LifecycleTask,
        source: &mut dyn ProposalSource,
        driver: &mut dyn IterativeDriver,
        budget: IterativeBudget,
        cancellation: &AgentCancellation,
        backend: authorization::StageBackend<'_>,
    ) -> Result<IterativeRun, DriverFailure> {
        self.run_with_driver_live_seed_on(
            task,
            source,
            driver,
            budget,
            cancellation,
            None,
            None,
            true,
            backend,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn run_with_driver_live_seed(
        &self,
        task: &LifecycleTask,
        source: &mut dyn ProposalSource,
        driver: &mut dyn IterativeDriver,
        budget: IterativeBudget,
        cancellation: &AgentCancellation,
        mut session: Option<&mut source_live::SourceExecutionSession<'_>>,
        migrated: Option<&source_live::SourceMigrationSeed>,
        allow_target_effect: bool,
    ) -> Result<IterativeRun, DriverFailure> {
        self.run_with_driver_live_seed_on(
            task,
            source,
            driver,
            budget,
            cancellation,
            session,
            migrated,
            allow_target_effect,
            authorization::StageBackend::Interpreter,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn run_with_driver_live_seed_on(
        &self,
        task: &LifecycleTask,
        source: &mut dyn ProposalSource,
        driver: &mut dyn IterativeDriver,
        budget: IterativeBudget,
        cancellation: &AgentCancellation,
        session: Option<&mut source_live::SourceExecutionSession<'_>>,
        migrated: Option<&source_live::SourceMigrationSeed>,
        allow_target_effect: bool,
        backend: authorization::StageBackend<'_>,
    ) -> Result<IterativeRun, DriverFailure> {
        retained::run(
            self,
            task,
            source,
            driver,
            budget,
            cancellation,
            session,
            migrated,
            allow_target_effect,
            backend,
        )
    }
}
