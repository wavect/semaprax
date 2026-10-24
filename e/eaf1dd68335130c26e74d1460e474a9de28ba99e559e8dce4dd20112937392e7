//! Emission context initialization; target identity is borrowed from the authenticated invocation.
use super::*;
impl<'a, O: COutput> CEmitter<'a, O> {
    // Keep independently authenticated function, invocation and ownership views explicit.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        output: &'a mut O,
        variables: HashMap<ValueId, CBinding>,
        function: &'a ResolvedFunction,
        execution: &'a FunctionExecutionId,
        emission: &'a NativeEmissionContext<'a>,
        bytes_plan: Option<&'a native_bytes::NativeBytesPlan>,
        borrowed_aggregate_bytes: HashMap<(ValueId, Vec<DeclarationId>), String>,
        track_strings: bool,
    ) -> Result<Self, Diagnostic> {
        Ok(Self {
            output,
            program: emission.program,
            resource_abi: emission.resource_abi,
            variables,
            mutable_bindings: BTreeSet::new(),
            resource_text_views: BTreeSet::new(),
            function,
            execution,
            functions: emission.functions,
            record_layouts: emission.record_layouts,
            variant_layouts: emission.variant_layouts,
            return_type: &function.return_type,
            bytes_plan: bytes_plan
                .map(|plan| native_bytes::NativeBytesView::ordinary(plan, &function.cleanup_plan)),
            cleanup_resume: None,
            iterator_true_match: None,
            active_expression: None,
            contract_labels: emission.contract_labels,
            borrowed_aggregate_bytes,
            output_profile: emission.output_profile,
            owned_str_views: emission.owned_str_views,
            semantic_metered: false,
            owned_strings: track_strings.then(owned_strings::OwnedStrings::default),
            try_target_enabled: false,
            string_owner_moves: crate::literal_format::checked_moving_string_operands(function)?,
            string_condition_reads: crate::string_ops::conditions::function_reads(function),
            next_local: 0,
            indent: 1,
        })
    }
}

impl<'a, O: COutput> CEmitter<'a, O> {
    pub(super) fn in_cleanup_resume<T>(
        &mut self,
        entry: crate::cleanup_plan::BlockId,
        emit: impl FnOnce(&mut Self) -> Result<T, Diagnostic>,
    ) -> Result<T, Diagnostic> {
        if self.function.cleanup_plan.lexical_resume(entry).is_none() {
            return Err(backend_error(
                "native cleanup resume domain is absent or ambiguous",
            ));
        }
        let previous_plan = self.bytes_plan;
        let previous_resume = self.cleanup_resume;
        self.bytes_plan = previous_plan
            .map(|plan| plan.with_resume(entry))
            .transpose()?;
        self.cleanup_resume = Some(entry);
        let result = emit(self);
        self.bytes_plan = previous_plan;
        self.cleanup_resume = previous_resume;
        result
    }
}
