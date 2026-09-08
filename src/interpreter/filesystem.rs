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
                Op::FileRead | Op::FileList,
                [Value::BorrowedSlice(path), Value::Usize(length), Value::Usize(max)],
            ) => {
                state.reserve(*max)?;
                let path =
                    prefix(path.bytes(), *length).map_err(|_| failure(FileFailure::InvalidPath))?;
                if !(path.is_empty() && ops::permits_root(call.operation)) {
                    crate::filesystem_provider::validate_path(path).map_err(failure)?;
                }
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
                let bytes = if call.operation == Op::FileList {
                    state.provider.list(path, *max as usize)
                } else {
                    state.provider.read(path, *max as usize)
                }
                .map_err(failure)?;
                if call.operation == Op::FileList {
                    ops::validate_listing(&bytes).map_err(failure)?;
                }
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
                Op::FileWriteNew | Op::FileWriteAtomic,
                [Value::BorrowedSlice(path), Value::Usize(length), Value::BorrowedSlice(data), Value::Usize(data_length)],
            ) => {
                state.reserve(*data_length)?;
                let path =
                    prefix(path.bytes(), *length).map_err(|_| failure(FileFailure::InvalidPath))?;
                if !(path.is_empty() && ops::permits_root(call.operation)) {
                    crate::filesystem_provider::validate_path(path).map_err(failure)?;
                }
                if *data_length > ops::MAX_FILE_BYTES {
                    return Err(failure(FileFailure::CapacityExceeded));
                }
                let data = prefix(data.bytes(), *data_length)?;
                let written = if call.operation == Op::FileWriteAtomic {
                    state.provider.write_atomic(path, data)
                } else {
                    state.provider.write_new(path, data)
                }
                .map_err(failure)?;
                if written != data.len() {
                    return Err(failure(FileFailure::IoFailure));
                }
                Ok(Value::Usize(written as u64))
            }
            (
                Op::FileStat | Op::FileCreateDir | Op::FileRemove,
                [Value::BorrowedSlice(path), Value::Usize(length)],
            ) => {
                state.reserve(0)?;
                let path =
                    prefix(path.bytes(), *length).map_err(|_| failure(FileFailure::InvalidPath))?;
                if !(path.is_empty() && ops::permits_root(call.operation)) {
                    crate::filesystem_provider::validate_path(path).map_err(failure)?;
                }
                let result = match call.operation {
                    Op::FileStat => {
                        ops::encode_metadata(state.provider.stat(path).map_err(failure)?)
                            .map_err(failure)?
                    }
                    Op::FileCreateDir => {
                        state.provider.create_dir(path).map_err(failure)?;
                        0
                    }
                    Op::FileRemove => {
                        state.provider.remove(path).map_err(failure)?;
                        0
                    }
                    _ => unreachable!(),
                };
                Ok(Value::Usize(result))
            }
            _ => Err(Flow::Guard("ill-typed filesystem operation")),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::filesystem_provider::{FileFailure, FileKind, FileMetadata, FileProvider};
    struct Provider {
        calls: usize,
        settlements: usize,
        malformed: bool,
    }
    impl FileProvider for Provider {
        fn read(&mut self, _: &[u8], _: usize) -> Result<Vec<u8>, FileFailure> {
            Err(FileFailure::AuthorityDenied)
        }
        fn write_new(&mut self, _: &[u8], _: &[u8]) -> Result<usize, FileFailure> {
            Err(FileFailure::AuthorityDenied)
        }
        fn stat(&mut self, path: &[u8]) -> Result<FileMetadata, FileFailure> {
            assert!(path.is_empty());
            self.calls += 1;
            Ok(FileMetadata {
                kind: FileKind::Directory,
                size: 0,
            })
        }
        fn list(&mut self, path: &[u8], _: usize) -> Result<Vec<u8>, FileFailure> {
            assert!(path.is_empty());
            self.calls += 1;
            Ok(if self.malformed {
                b"b\0a\0".to_vec()
            } else {
                b"a\0b\0".to_vec()
            })
        }
        fn settle(&mut self) {
            self.settlements += 1;
        }
    }
    #[test]
    fn filesystem_v2_root_metadata_and_invalid_listing_settle() {
        let text = r#"
module fs.v2;
permit { fs.read }
@id("fs.main") fn main()->i64 { 0 }
@id("fs.run") fn run()->bool uses { fs.read } {
    let path=[0u8];
    let metadata=file_stat(array_as_slice(path),0usize);
    let names=file_list(array_as_slice(path),0usize,8usize);
    metadata==2usize && byte_len(bytes_as_slice(names))==4usize
}
"#;
        let source = crate::check(text, "fs-v2.spx").unwrap();
        let program = crate::hir::resolve(&source).unwrap();
        let mut provider = Provider {
            calls: 0,
            settlements: 0,
            malformed: false,
        };
        assert!(crate::hosted_interpreter::execute_filesystem_command(
            &program,
            "fs.run",
            &mut provider,
            1000
        )
        .is_err());
        assert_eq!(provider.calls, 0);
        let run = crate::hosted_interpreter::execute_filesystem_command_v2(
            &program,
            "fs.run",
            &mut provider,
            1000,
        )
        .unwrap();
        assert!(matches!(
            run.outcome,
            crate::interpreter::CommandEvaluationOutcome::ReturnedBool(true)
        ));
        provider.malformed = true;
        let run = crate::hosted_interpreter::execute_filesystem_command_v2(
            &program,
            "fs.run",
            &mut provider,
            1000,
        )
        .unwrap();
        match run.outcome {
            crate::interpreter::CommandEvaluationOutcome::LanguageFailure(status) => {
                assert_eq!(status.code(), 5)
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(provider.calls, 4);
        assert_eq!(provider.settlements, 2);
    }
}
