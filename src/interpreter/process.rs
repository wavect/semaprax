//! Explicit process-provider execution, bounded reservations, and settled publication.
use super::{Environment, Evaluator, Flow, OwnedBytesValue, Value};
use crate::conformance::{NormalizedStatus, Retryability, StatusClass};
use crate::hir::ResolvedHostCommandCall;
use crate::process_provider::{
    ProcessFailure, ProcessInvocationBudget, ProcessProvider, ProcessRequest,
};
use std::sync::Arc;

pub(super) const ADMITTED_EFFECTS: [&str; 6] = [
    "process.args.read",
    "process.environment.read",
    "process.execute",
    "process.stderr.write",
    "process.stdin.read",
    "process.stdout.write",
];
pub(super) struct ProcessState<'a> {
    provider: &'a mut dyn ProcessProvider,
    budget: ProcessInvocationBudget,
}
impl<'a> ProcessState<'a> {
    pub(super) fn new(provider: &'a mut dyn ProcessProvider) -> Self {
        Self {
            provider,
            budget: ProcessInvocationBudget::default(),
        }
    }
    pub(super) fn settle(self) -> Result<(), ProcessFailure> {
        self.provider.settle()
    }
}
pub(super) fn failure(error: ProcessFailure) -> Flow {
    Flow::Failure(
        NormalizedStatus::try_new(
            crate::process_ops::STATUS_DOMAIN,
            error.status_code(),
            StatusClass::Adapter,
            Retryability::Known(false),
        )
        .expect("closed process status"),
    )
}
fn extent(value: u64) -> Result<usize, Flow> {
    usize::try_from(value).map_err(|_| failure(ProcessFailure::CapacityExceeded))
}
impl Evaluator<'_> {
    pub(super) fn evaluate_process_operation(
        &mut self,
        call: &ResolvedHostCommandCall,
        environment: &mut Environment,
        depth: usize,
    ) -> Result<Value, Flow> {
        if call.args.len() != 8 {
            return Err(Flow::Guard("invalid process operation arity"));
        }
        let mut values = Vec::with_capacity(8);
        for argument in &call.args {
            values.push(self.evaluate(argument, environment, depth)?);
        }
        let [Value::Usize(tool), Value::BorrowedSlice(argv), Value::Usize(argv_length), Value::BorrowedSlice(stdin), Value::Usize(stdin_length), Value::Usize(timeout_ms), Value::Usize(stdout_max), Value::Usize(stderr_max)] =
            values.as_slice()
        else {
            return Err(Flow::Guard("ill-typed process arguments"));
        };
        let request = ProcessRequest::from_wire(
            *tool,
            argv.bytes(),
            extent(*argv_length)?,
            stdin.bytes(),
            extent(*stdin_length)?,
            *timeout_ms,
            extent(*stdout_max)?,
            extent(*stderr_max)?,
        )
        .map_err(failure)?;
        let state = self
            .command_input
            .as_mut()
            .and_then(|state| state.process.as_mut())
            .ok_or_else(|| failure(ProcessFailure::AuthorityDenied))?;
        state.budget.reserve(&request).map_err(failure)?;
        let next = self
            .next_byte_allocation
            .checked_add(1)
            .ok_or_else(|| failure(ProcessFailure::CapacityExceeded))?;
        let reserved = self
            .allocated_byte_payload
            .checked_add(request.reserved_output_bytes() as u64)
            .ok_or_else(|| failure(ProcessFailure::CapacityExceeded))?;
        if next > crate::byte_data_capacity::MAX_BYTES_COPY_SITES
            || reserved > crate::byte_data_capacity::MAX_OWNED_BYTE_PAYLOAD_BYTES
        {
            return Err(failure(ProcessFailure::CapacityExceeded));
        }
        let bytes = state
            .provider
            .run(&request)
            .map_err(failure)?
            .encode(&request)
            .map_err(failure)?;
        self.next_byte_allocation = next;
        self.allocated_byte_payload += bytes.len() as u64;
        Ok(Value::Bytes(OwnedBytesValue {
            allocation: next,
            bytes: Arc::from(bytes),
        }))
    }
}
