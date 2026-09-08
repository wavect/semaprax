//! Invocation-scoped filesystem authority and checked owned-byte publication.
pub(crate) mod command;
use super::{Environment, Evaluator, Flow, OwnedBytesValue, Value};
use crate::conformance::{NormalizedStatus, Retryability, StatusClass};
use crate::filesystem_ops as ops;
use crate::filesystem_provider::{FileFailure, FileProvider};
use crate::hir::{ResolvedHostCommandCall, ResolvedHostCommandOperation as Op};
use std::sync::Arc;

pub(super) struct FileState<'a> {
    provider: &'a mut dyn FileProvider,
    operations: u64,
    reserved: u64,
}
impl<'a> FileState<'a> {
    pub(super) fn new(provider: &'a mut dyn FileProvider) -> Self {
        Self {
            provider,
            operations: 0,
            reserved: 0,
        }
    }
    pub(super) fn settle(self) {
        self.provider.settle();
    }
    fn reserve(&mut self, count: u64) -> Result<(), Flow> {
        self.operations = self
            .operations
            .checked_add(1)
            .ok_or_else(|| failure(FileFailure::CapacityExceeded))?;
        self.reserved = self
            .reserved
            .checked_add(count)
            .ok_or_else(|| failure(FileFailure::CapacityExceeded))?;
        if self.operations > ops::MAX_OPERATIONS || self.reserved > ops::MAX_TOTAL_BYTES {
            return Err(failure(FileFailure::CapacityExceeded));
        }
        Ok(())
    }
}
fn failure(error: FileFailure) -> Flow {
    Flow::Failure(
        NormalizedStatus::try_new(
            ops::STATUS_DOMAIN,
            error.status_code(),
            StatusClass::Adapter,
            Retryability::Known(false),
        )
        .expect("closed filesystem status"),
    )
}
fn prefix(bytes: &[u8], length: u64) -> Result<&[u8], Flow> {
    let count = usize::try_from(length).map_err(|_| failure(FileFailure::CapacityExceeded))?;
    bytes
        .get(..count)
        .ok_or_else(|| failure(FileFailure::CapacityExceeded))
}
impl Evaluator<'_> {
    pub(super) fn evaluate_filesystem_operation(
        &mut self,
        call: &ResolvedHostCommandCall,
        environment: &mut Environment,
        depth: usize,
    ) -> Result<Value, Flow> {
        if call.args.len() != ops::arity(call.operation) {
            return Err(Flow::Guard("invalid filesystem arity"));
        }
        let mut values = Vec::with_capacity(call.args.len());
        for argument in &call.args {
            values.push(self.evaluate(argument, environment, depth)?);
        }
        let state = self
            .command_input
            .as_mut()
            .and_then(|input| input.filesystem.as_mut())
            .ok_or_else(|| failure(FileFailure::AuthorityDenied))?;
        match (call.operation, values.as_slice()) {
            (
                Op::FileRead,
                [Value::BorrowedSlice(path), Value::Usize(length), Value::Usize(max)],
            ) => {
                state.reserve(*max)?;
                let path =
                    prefix(path.bytes(), *length).map_err(|_| failure(FileFailure::InvalidPath))?;
                crate::filesystem_provider::validate_path(path).map_err(failure)?;
                if *max > ops::MAX_FILE_BYTES {
                    return Err(failure(FileFailure::CapacityExceeded));
                }
                let next = self
                    .next_byte_allocation
                    .checked_add(1)
                    .ok_or_else(|| failure(FileFailure::CapacityExceeded))?;
                if next > crate::byte_data_capacity::MAX_BYTES_COPY_SITES {
                    return Err(failure(FileFailure::CapacityExceeded));
                }
                let reserved_payload = self
                    .allocated_byte_payload
                    .checked_add(*max)
                    .ok_or_else(|| failure(FileFailure::CapacityExceeded))?;
                if reserved_payload > crate::byte_data_capacity::MAX_OWNED_BYTE_PAYLOAD_BYTES {
                    return Err(failure(FileFailure::CapacityExceeded));
                }
                let bytes = state.provider.read(path, *max as usize).map_err(failure)?;
                if bytes.len() as u64 > *max {
                    return Err(failure(FileFailure::CapacityExceeded));
                }
                self.next_byte_allocation = next;
                self.allocated_byte_payload += bytes.len() as u64;
                Ok(Value::Bytes(OwnedBytesValue {
                    allocation: next,
                    bytes: Arc::from(bytes),
                }))
            }
            (
                Op::FileWriteNew,
                [Value::BorrowedSlice(path), Value::Usize(length), Value::BorrowedSlice(data), Value::Usize(data_length)],
            ) => {
                state.reserve(*data_length)?;
                let path =
                    prefix(path.bytes(), *length).map_err(|_| failure(FileFailure::InvalidPath))?;
                crate::filesystem_provider::validate_path(path).map_err(failure)?;
                if *data_length > ops::MAX_FILE_BYTES {
                    return Err(failure(FileFailure::CapacityExceeded));
                }
                let data = prefix(data.bytes(), *data_length)?;
                let written = state.provider.write_new(path, data).map_err(failure)?;
                if written != data.len() {
                    return Err(failure(FileFailure::IoFailure));
                }
                Ok(Value::Usize(written as u64))
            }
            _ => Err(Flow::Guard("ill-typed filesystem operation")),
        }
    }
}
