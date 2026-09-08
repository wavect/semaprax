//! Draft consuming migration adapter; registered only with the opaque seed owner.
use super::*;
use crate::agent_lifecycle::iterative::driver::{DriverFailure, IterativeDriver};
use crate::agent_runtime_v2::checkpoint::CheckpointUsage;
use crate::execution_revision::typed::migration::MigrationSeed;

pub(crate) struct SeededTypedRun {
    pub(crate) run: TypedEffectRun,
    pub(crate) usage: CheckpointUsage,
    pub(crate) iterations: usize,
    pub(crate) stages: usize,
}
pub(crate) struct SeededTypedFailure {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) usage: CheckpointUsage,
    pub(crate) terminal: Option<IterativeRun>,
    pub(crate) iterations: usize,
    pub(crate) stages: usize,
}
struct SeedDriver<'a> {
    dispatch: Dispatch<'a>,
    reserved_fuel: u64,
    max_reserved_fuel: u64,
    reservations: usize,
    completed_iterations: usize,
}
impl IterativeDriver for SeedDriver<'_> {
    fn before_stage(
        &mut self,
        _: &'static str,
        _: usize,
        max_steps: usize,
    ) -> Result<(), Vec<Diagnostic>> {
        let next = self
            .reserved_fuel
            .checked_add(max_steps as u64)
            .ok_or_else(|| error("migration.fuel.overflow"))?;
        if next > self.max_reserved_fuel {
            return Err(error("migration.fuel.exhausted"));
        }
        self.reserved_fuel = next;
        self.reservations += 1;
        Ok(())
    }
    fn read(
        &mut self,
        authorization: &AuthorizedRequest,
    ) -> Result<Option<Vec<u8>>, Vec<Diagnostic>> {
        Ok(self.dispatch.read(authorization))
    }
    fn after_transition(
        &mut self,
        _: usize,
        _: &str,
        _: &RetainedValue,
    ) -> Result<(), Vec<Diagnostic>> {
        self.completed_iterations += 1;
        Ok(())
    }
}
impl CompiledTypedEffects {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn run_from_seed(
        &self,
        task: &LifecycleTask,
        proposals: &[String],
        handler: &mut dyn TypedEffectHandler,
        stages: IterativeBudget,
        effects: EffectBudget,
        cancellation: &AgentCancellation,
        seed: &MigrationSeed,
    ) -> Result<SeededTypedRun, SeededTypedFailure> {
        let prior = seed.usage();
        let early = |field: &str| SeededTypedFailure {
            diagnostics: error(field),
            usage: prior,
            terminal: None,
            iterations: seed.prior_iterations(),
            stages: seed.prior_stages(),
        };
        let prior_calls =
            usize::try_from(prior.calls).map_err(|_| early("migration.calls.overflow"))?;
        let prior_arguments = usize::try_from(prior.argument_bytes)
            .map_err(|_| early("migration.arguments.overflow"))?;
        let prior_results =
            usize::try_from(prior.result_bytes).map_err(|_| early("migration.results.overflow"))?;
        let max_calls = effects.max_calls.min(self.limits.max_calls);
        let max_total_bytes = effects.max_total_bytes.min(self.limits.max_total_bytes);
        let max_iterations = stages.max_iterations.min(self.max_iterations);
        if prior_calls >= max_calls
            || prior_arguments
                .checked_add(prior_results)
                .is_none_or(|n| n >= max_total_bytes)
            || prior.reserved_fuel >= seed.max_reserved_fuel()
            || seed.prior_iterations() >= max_iterations
            || seed.prior_stages() >= stages.max_stages
        {
            return Err(early("migration.destination.exhausted"));
        }
        let budget = EffectBudget {
            max_calls: max_calls - prior_calls,
            max_argument_bytes: effects
                .max_argument_bytes
                .min(self.limits.max_argument_bytes),
            max_result_bytes: effects.max_result_bytes.min(self.limits.max_result_bytes),
            max_total_bytes,
        };
        let remaining = IterativeBudget {
            max_iterations: max_iterations - seed.prior_iterations(),
            max_stages: stages.max_stages - seed.prior_stages(),
            max_steps_per_stage: stages.max_steps_per_stage,
        };
        let mut driver = SeedDriver {
            dispatch: Dispatch {
                compiled: self,
                proposals,
                handler,
                budget,
                dispatched: 0,
                arguments: prior_arguments,
                results: prior_results,
                failure: None,
            },
            reserved_fuel: prior.reserved_fuel,
            max_reserved_fuel: seed.max_reserved_fuel(),
            reservations: 0,
            completed_iterations: 0,
        };
        let outcome = self.lifecycle.run_with_driver_seed(
            task,
            proposals,
            &mut driver,
            remaining,
            cancellation,
            seed,
        );
        let usage = CheckpointUsage {
            calls: prior
                .calls
                .saturating_add(driver.dispatch.dispatched as u64),
            argument_bytes: driver.dispatch.arguments as u64,
            result_bytes: driver.dispatch.results as u64,
            reserved_fuel: driver.reserved_fuel,
        };
        let iterations = seed
            .prior_iterations()
            .saturating_add(driver.completed_iterations);
        let stage_count = seed.prior_stages().saturating_add(driver.reservations);
        let lifecycle = match outcome {
            Ok(run) => run,
            Err(DriverFailure::Diagnostics(diagnostics)) => {
                return Err(SeededTypedFailure {
                    diagnostics,
                    usage,
                    terminal: None,
                    iterations,
                    stages: stage_count,
                })
            }
            Err(DriverFailure::Persistence {
                terminal,
                diagnostics,
            }) => {
                return Err(SeededTypedFailure {
                    diagnostics,
                    usage,
                    terminal: Some(terminal),
                    iterations,
                    stages: stage_count,
                })
            }
        };
        let evidence=format!("{{\"schema\":\"semaprax.agent-migrated-typed-evidence.v1\",\"seed\":{},\"registry\":{},\"lifecycle_evidence\":{},\"calls\":{},\"argument_bytes\":{},\"result_bytes\":{},\"reserved_fuel\":{},\"iterations\":{},\"stages\":{},\"failure\":{}}}\n",quote_json(seed.binding_digest()),quote_json(self.digest()),quote_json(lifecycle.evidence_digest()),usage.calls,usage.argument_bytes,usage.result_bytes,usage.reserved_fuel,iterations,stage_count,driver.dispatch.failure.map(quote_json).unwrap_or_else(||"null".into()));
        let run = TypedEffectRun {
            lifecycle,
            dispatched: usize::try_from(usage.calls)
                .map_err(|_| early("migration.calls.overflow"))?,
            argument_bytes: driver.dispatch.arguments,
            result_bytes: driver.dispatch.results,
            failure: driver.dispatch.failure,
            digest: digest(
                b"semaprax.agent-migrated-typed-evidence.v1\0",
                evidence.as_bytes(),
            ),
            evidence,
        };
        Ok(SeededTypedRun {
            run,
            usage,
            iterations,
            stages: stage_count,
        })
    }
}
