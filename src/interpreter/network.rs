//! Bounded Language Network I/O v1 evaluation inside the reference
//! interpreter.
//!
//! Network operations reach the evaluator only through an explicitly injected
//! [`NetworkProvider`]; the effect-free `run`/`interpret` paths hold none and
//! fail closed with a guard before any transport runs. The evaluator owns the
//! program-visible handle table, the argument capacities, and the cumulative
//! byte budget; the provider owns transport.

pub(crate) mod command;

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::conformance::{NormalizedStatus, Retryability, StatusClass};
use crate::hir::{ResolvedHostCommandCall, ResolvedHostCommandOperation as Operation};
use crate::network_io_ops;
use crate::network_provider::{
    NetworkFailure, NetworkProvider, ProviderConnection, ProviderListener,
};

use super::{Environment, Evaluator, Flow, OwnedBytesValue, Value};

/// Invocation-scoped network authority: the injected provider plus the dense
/// handle table and byte budget the evaluator enforces in front of it.
pub(super) struct NetworkState<'a> {
    provider: &'a mut dyn NetworkProvider,
    /// Program handle to provider connection. Handles are dense from 1 and
    /// never reused within an invocation, so a closed handle stays unknown.
    handles: BTreeMap<u64, ProviderConnection>,
    listeners: BTreeMap<u64, ProviderListener>,
    /// The next handle to hand out; `MAX_HANDLES + 1` once the table is spent.
    next_handle: u64,
    /// Cumulative bytes sent plus received.
    transferred: u64,
}

impl<'a> NetworkState<'a> {
    pub(super) fn new(provider: &'a mut dyn NetworkProvider) -> Self {
        Self {
            provider,
            handles: BTreeMap::new(),
            listeners: BTreeMap::new(),
            next_handle: 1,
            transferred: 0,
        }
    }

    /// Release every connection the provider still holds. Runs once at
    /// invocation settlement on every outcome.
    pub(super) fn settle(self) {
        self.provider.settle();
    }

    fn connection(&self, handle: u64) -> Result<ProviderConnection, Flow> {
        self.handles
            .get(&handle)
            .copied()
            .ok_or_else(|| failure(network_io_ops::UNKNOWN_HANDLE))
    }

    /// Charge `count` transferred bytes against the cumulative budget.
    fn charge(&mut self, count: usize) -> Result<(), Flow> {
        let next = u64::try_from(count)
            .ok()
            .and_then(|count| self.transferred.checked_add(count))
            .ok_or_else(|| failure(network_io_ops::CAPACITY_EXCEEDED))?;
        if next > network_io_ops::MAX_TOTAL_BYTES {
            return Err(failure(network_io_ops::CAPACITY_EXCEEDED));
        }
        self.transferred = next;
        Ok(())
    }

    fn connect(&mut self, host: &[u8], port: u64) -> Result<u64, Flow> {
        self.connect_with(host, port, false)
    }

    fn connect_with(&mut self, host: &[u8], port: u64, tls: bool) -> Result<u64, Flow> {
        let fail = if tls { service_failure } else { failure };
        let host_len = u64::try_from(host.len()).unwrap_or(u64::MAX);
        if host.is_empty() || host_len > network_io_ops::MAX_HOST_BYTES || host.contains(&0) {
            return Err(fail(network_io_ops::INVALID_ENDPOINT));
        }
        let host = std::str::from_utf8(host).map_err(|_| fail(network_io_ops::INVALID_ENDPOINT))?;
        let port = u16::try_from(port)
            .ok()
            .filter(|port| *port != 0 && u64::from(*port) <= network_io_ops::MAX_PORT)
            .ok_or_else(|| fail(network_io_ops::INVALID_ENDPOINT))?;
        let handle = self.next_handle;
        if handle > network_io_ops::MAX_HANDLES {
            return Err(fail(network_io_ops::CAPACITY_EXCEEDED));
        }
        let connection = if tls {
            self.provider.connect_tls(host, port)
        } else {
            self.provider.connect(host, port)
        }
        .map_err(|error| {
            if tls {
                provider_service_failure(error)
            } else {
                provider_failure(error)
            }
        })?;
        self.next_handle = handle + 1;
        self.handles.insert(handle, connection);
        Ok(handle)
    }

    fn listen(&mut self, host: &[u8], port: u64) -> Result<u64, Flow> {
        let host = std::str::from_utf8(host)
            .map_err(|_| service_failure(network_io_ops::INVALID_ENDPOINT))?;
        if host.is_empty()
            || host.len() as u64 > network_io_ops::MAX_HOST_BYTES
            || host.as_bytes().contains(&0)
        {
            return Err(service_failure(network_io_ops::INVALID_ENDPOINT));
        }
        let port = u16::try_from(port)
            .ok()
            .filter(|port| *port != 0)
            .ok_or_else(|| service_failure(network_io_ops::INVALID_ENDPOINT))?;
        let handle = self.next_handle;
        if handle > network_io_ops::MAX_HANDLES {
            return Err(service_failure(network_io_ops::CAPACITY_EXCEEDED));
        }
        let listener = self
            .provider
            .listen(host, port)
            .map_err(provider_service_failure)?;
        self.next_handle += 1;
        self.listeners.insert(handle, listener);
        Ok(handle)
    }

    fn accept(&mut self, handle: u64) -> Result<u64, Flow> {
        let listener = self
            .listeners
            .get(&handle)
            .copied()
            .ok_or_else(|| service_failure(network_io_ops::UNKNOWN_HANDLE))?;
        let accepted = self
            .provider
            .accept(listener)
            .map_err(provider_service_failure)?;
        let connection_handle = self.next_handle;
        if connection_handle > network_io_ops::MAX_HANDLES {
            return Err(service_failure(network_io_ops::CAPACITY_EXCEEDED));
        }
        self.next_handle += 1;
        self.handles.insert(connection_handle, accepted);
        Ok(connection_handle)
    }

    fn close_listener(&mut self, handle: u64) -> Result<u64, Flow> {
        let listener = self
            .listeners
            .remove(&handle)
            .ok_or_else(|| service_failure(network_io_ops::UNKNOWN_HANDLE))?;
        self.provider
            .close_listener(listener)
            .map_err(provider_service_failure)?;
        Ok(0)
    }

    fn send(&mut self, handle: u64, bytes: &[u8]) -> Result<u64, Flow> {
        let connection = self.connection(handle)?;
        let pending = u64::try_from(bytes.len())
            .ok()
            .and_then(|count| self.transferred.checked_add(count))
            .ok_or_else(|| failure(network_io_ops::CAPACITY_EXCEEDED))?;
        if pending > network_io_ops::MAX_TOTAL_BYTES {
            return Err(failure(network_io_ops::CAPACITY_EXCEEDED));
        }
        let written = self
            .provider
            .send(connection, bytes)
            .map_err(provider_failure)?;
        if written != bytes.len() {
            return Err(failure(network_io_ops::TRANSFER_FAILED));
        }
        self.charge(written)?;
        Ok(written as u64)
    }

    /// One bounded read; the result is already charged against the budget.
    fn recv(&mut self, handle: u64, max: u64) -> Result<Vec<u8>, Flow> {
        let connection = self.connection(handle)?;
        if max > network_io_ops::MAX_CHUNK_BYTES {
            return Err(failure(network_io_ops::CAPACITY_EXCEEDED));
        }
        let max = usize::try_from(max).map_err(|_| failure(network_io_ops::CAPACITY_EXCEEDED))?;
        let mut received = self
            .provider
            .recv(connection, max)
            .map_err(provider_failure)?;
        if received.len() > max {
            // A provider that over-delivers breaks the chunk contract; keep
            // the program's view within `max` and fail the transfer.
            received.truncate(max);
            return Err(failure(network_io_ops::TRANSFER_FAILED));
        }
        self.charge(received.len())?;
        Ok(received)
    }

    fn wait(&mut self, handle: u64, timeout_ms: u64) -> Result<u64, Flow> {
        let connection = self.connection(handle)?;
        if timeout_ms > network_io_ops::MAX_WAIT_MILLIS {
            return Err(failure(network_io_ops::CAPACITY_EXCEEDED));
        }
        let timeout_ms =
            u32::try_from(timeout_ms).map_err(|_| failure(network_io_ops::CAPACITY_EXCEEDED))?;
        let state = self
            .provider
            .wait(connection, timeout_ms)
            .map_err(provider_failure)?;
        Ok(state.code())
    }

    fn close(&mut self, handle: u64) -> Result<u64, Flow> {
        let connection = self.connection(handle)?;
        self.handles.remove(&handle);
        self.provider.close(connection).map_err(provider_failure)?;
        Ok(0)
    }
}

fn failure(code: u32) -> Flow {
    failure_in(network_io_ops::STATUS_DOMAIN, code)
}

fn service_failure(code: u32) -> Flow {
    failure_in(network_io_ops::SERVICE_STATUS_DOMAIN, code)
}

fn failure_in(domain: &'static str, code: u32) -> Flow {
    Flow::Failure(
        NormalizedStatus::try_new(
            domain,
            code,
            StatusClass::Adapter,
            Retryability::Known(false),
        )
        .expect("compiler-owned network status table is valid"),
    )
}

fn provider_failure(error: NetworkFailure) -> Flow {
    failure(error.status_code())
}

fn provider_service_failure(error: NetworkFailure) -> Flow {
    service_failure(error.status_code())
}

impl Evaluator<'_> {
    pub(super) fn evaluate_network_operation(
        &mut self,
        call: &ResolvedHostCommandCall,
        environment: &mut Environment,
        depth: usize,
    ) -> Result<Value, Flow> {
        if call.args.len() != network_io_ops::arity(call.operation) {
            return Err(Flow::Guard("invalid network operation arity"));
        }
        // Arguments evaluate left to right before any host call.
        let mut values = Vec::with_capacity(call.args.len());
        for argument in &call.args {
            values.push(self.evaluate(argument, environment, depth)?);
        }
        let Some(input) = self.command_input.as_mut() else {
            return Err(Flow::Guard(
                "network operation reached an evaluator without a network provider",
            ));
        };
        let Some(network) = input.network.as_mut() else {
            return Err(failure(network_io_ops::AUTHORITY_DENIED));
        };
        match (call.operation, values.as_slice()) {
            (Operation::NetConnect, [Value::BorrowedSlice(host), Value::Usize(port)]) => {
                network.connect(host.bytes(), *port).map(Value::Usize)
            }
            (Operation::NetTlsConnect, [Value::BorrowedSlice(host), Value::Usize(port)]) => network
                .connect_with(host.bytes(), *port, true)
                .map(Value::Usize),
            (Operation::NetListen, [Value::BorrowedSlice(host), Value::Usize(port)]) => {
                network.listen(host.bytes(), *port).map(Value::Usize)
            }
            (Operation::NetAccept, [Value::Usize(listener)]) => {
                network.accept(*listener).map(Value::Usize)
            }
            (Operation::NetCloseListener, [Value::Usize(listener)]) => {
                network.close_listener(*listener).map(Value::Usize)
            }
            (Operation::NetSend, [Value::Usize(handle), Value::BorrowedSlice(bytes)]) => {
                network.send(*handle, bytes.bytes()).map(Value::Usize)
            }
            (Operation::NetRecv, [Value::Usize(handle), Value::Usize(max)]) => {
                // One owned-byte allocation site, exactly like `stdin_read`.
                let next_count = self
                    .next_byte_allocation
                    .checked_add(1)
                    .ok_or_else(|| failure(network_io_ops::CAPACITY_EXCEEDED))?;
                if next_count > crate::byte_data_capacity::MAX_BYTES_COPY_SITES {
                    return Err(failure(network_io_ops::CAPACITY_EXCEEDED));
                }
                let received = network.recv(*handle, *max)?;
                let length = u64::try_from(received.len())
                    .map_err(|_| failure(network_io_ops::CAPACITY_EXCEEDED))?;
                let next_payload = self
                    .allocated_byte_payload
                    .checked_add(length)
                    .ok_or_else(|| failure(network_io_ops::CAPACITY_EXCEEDED))?;
                if next_payload > crate::byte_data_capacity::MAX_OWNED_BYTE_PAYLOAD_BYTES {
                    return Err(failure(network_io_ops::CAPACITY_EXCEEDED));
                }
                self.next_byte_allocation = next_count;
                self.allocated_byte_payload = next_payload;
                Ok(Value::Bytes(OwnedBytesValue {
                    allocation: next_count,
                    bytes: Arc::from(received),
                }))
            }
            (Operation::NetStreamStdout, [Value::Usize(handle), Value::Usize(max)]) => {
                let received = network.recv(*handle, *max)?;
                let stdout_length = self.stdout_transcript.as_ref().map_or(0, Vec::len);
                let stderr_length = self.stderr_transcript.as_ref().map_or(0, Vec::len);
                let combined = stdout_length
                    .checked_add(stderr_length)
                    .and_then(|length| length.checked_add(received.len()))
                    .ok_or(Flow::Guard("command transcript length overflowed"))?;
                if combined > crate::command_io_ops::MAX_OUTPUT_BYTES as usize {
                    return Err(failure(network_io_ops::CAPACITY_EXCEEDED));
                }
                self.stdout_transcript
                    .as_mut()
                    .ok_or(Flow::Guard(
                        "net_stream_stdout reached an evaluator without command output",
                    ))?
                    .extend_from_slice(&received);
                Ok(Value::Usize(received.len() as u64))
            }
            (Operation::NetWait, [Value::Usize(handle), Value::Usize(timeout_ms)]) => {
                network.wait(*handle, *timeout_ms).map(Value::Usize)
            }
            (Operation::NetClose, [Value::Usize(handle)]) => {
                network.close(*handle).map(Value::Usize)
            }
            _ => Err(Flow::Guard("ill-typed network operation operand")),
        }
    }
}
