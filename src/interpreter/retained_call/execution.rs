//! Shared retained-call execution. The public worker and sealed synchronous
//! handoff use the same staging, call frame, failure mapping, and harvesting.
use super::*;

pub(super) struct Invocation<'a> {
    program: &'a hir::ResolvedProgram,
    prepared: &'a PreparedRetainedCall,
    pub(super) values: Vec<(hir::ValueId, Value)>,
    staging: ByteStaging,
    closure_functions: BTreeMap<hir::ExpressionId, ResolvedFunction>,
}

pub(super) fn prepare<'a>(
    program: &'a hir::ResolvedProgram,
    prepared: &'a PreparedRetainedCall,
    arguments: &[RetainedValue],
    max_steps: usize,
) -> Result<Invocation<'a>, Vec<Diagnostic>> {
    validate_step_limit(max_steps)?;
    if !index_matches_program(
        program,
        &prepared.entry_id,
        prepared.entry_index,
        &prepared.function_indices,
    ) {
        return Err(vec![guard_error(
            "retained call closure no longer matches its resolved program",
        )]);
    }
    let entry = &program.functions[prepared.entry_index];
    if RetainedSignature::of(entry) != prepared.signature {
        return Err(vec![guard_error(
            "retained call target signature no longer matches its admitted signature",
        )]);
    }
    if arguments.len() != entry.params.len() {
        return Err(vec![argument_error(format!(
            "retained call `{}` takes {} argument(s), {} were provided",
            prepared.entry_id,
            entry.params.len(),
            arguments.len()
        ))]);
    }

    // Stage every argument before the evaluator exists. A shape or capacity
    // mismatch is an argument diagnostic, never an evaluator guard.
    let mut staging = ByteStaging::default();
    let mut values = Vec::with_capacity(entry.params.len());
    for (index, (parameter, argument)) in entry.params.iter().zip(arguments).enumerate() {
        let value = stage(&program.declarations, &parameter.ty, argument, &mut staging).map_err(
            |detail| {
                vec![argument_error(format!(
                    "retained call `{}` argument {index} (`{}`): {detail}",
                    prepared.entry_id, parameter.name
                ))]
            },
        )?;
        values.push((parameter.id.clone(), value));
    }
    let closure_functions =
        super::super::closures::checked_functions(program).map_err(|error| vec![error])?;
    Ok(Invocation {
        program,
        prepared,
        values,
        staging,
        closure_functions,
    })
}

impl Invocation<'_> {
    pub(super) fn run(
        self,
        max_steps: usize,
        semantic_limit: Option<u64>,
    ) -> RetainedCallEvaluation {
        let Self {
            program,
            prepared,
            values,
            staging,
            closure_functions,
        } = self;
        let entry = &program.functions[prepared.entry_index];
        let return_type = entry.return_type.clone();
        let lookup = FunctionLookup::Prepared {
            functions: &program.functions,
            function_instances: &program.function_instances,
            indices: &prepared.function_indices,
        };
        let mut evaluator = Evaluator::new_prepared(
            lookup,
            closure_functions,
            &program.declarations,
            max_steps,
            0,
            PreparedCancellation::Never,
        );
        // Host-staged carriers occupy the same verified byte-data
        // capacity a `bytes_copy` would, so an argument can never mint
        // allocation identity or payload the program did not have.
        evaluator.next_byte_allocation = staging.allocations;
        evaluator.allocated_byte_payload = staging.payload;
        if let Some(limit) = semantic_limit {
            evaluator.semantic = super::super::semantic_work::SemanticMeter::limited(limit);
        }
        let evaluated = evaluator.call_frame(entry, values, 0);
        let mut cleanup_events = Vec::new();
        let outcome = match evaluated {
            Ok(value) => match harvest(
                &program.declarations,
                &return_type,
                value,
                &mut cleanup_events,
            ) {
                Ok(value) => RetainedCallOutcome::Returned(value),
                Err(detail) => RetainedCallOutcome::GuardError(detail),
            },
            Err(Flow::Failure(status)) => RetainedCallOutcome::LanguageFailure(status),
            Err(Flow::Exhausted) => RetainedCallOutcome::FuelExhausted,
            Err(Flow::DepthExceeded) => RetainedCallOutcome::CallDepthExceeded,
            Err(Flow::Cancelled { .. }) => RetainedCallOutcome::GuardError(
                "unexpected cancellation in retained call evaluation".to_owned(),
            ),
            Err(Flow::Utf8MaterializationLimitExceeded { .. }) => RetainedCallOutcome::GuardError(
                "unexpected UTF-8 materialization limit in retained call evaluation".to_owned(),
            ),
            Err(Flow::Residual(_)) => RetainedCallOutcome::GuardError(
                super::super::owned_try::ESCAPED_RESIDUAL_GUARD.to_owned(),
            ),
            Err(Flow::Guard(detail)) => RetainedCallOutcome::GuardError(detail.to_owned()),
        };
        RetainedCallEvaluation {
            function_id: entry.id.clone(),
            outcome,
            cleanup_events,
            steps_used: evaluator.steps,
            max_steps,
            failure: evaluator.failure_detail.take(),
            semantic_work: Some(SemanticWork {
                fuel_used: evaluator.semantic.used(),
                fuel_limit: evaluator.semantic.limit(),
                exhausted: evaluator.semantic.exhausted(),
                finalizer_events: None,
            }),
        }
    }
}
