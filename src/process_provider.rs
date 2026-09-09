//! Explicit, bounded process-execution provider contracts.
//!
//! Hosts inject a [`ProcessProvider`]. The interpreter admits a complete request
//! and debits its invocation budget before calling that provider. Supported Unix
//! hosts can explicitly register held tools through `registered`.

use std::collections::VecDeque;

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub mod registered;

/// Maximum number of argv entries in one request.
pub const MAX_ARGUMENTS: usize = 16;
/// Maximum combined argv-wire and stdin bytes in one request.
pub const MAX_INPUT_BYTES: usize = 65_536;
/// Maximum canonical output wire bytes, including its 32-byte header.
pub const MAX_OUTPUT_BYTES: usize = 65_536;
/// Maximum admitted process timeout in milliseconds.
pub const MAX_TIMEOUT_MS: u64 = 30_000;
/// Maximum process invocations in one language invocation.
pub const MAX_RUNS: usize = 16;
/// Maximum reserved process bytes in one language invocation.
pub const MAX_TOTAL_BYTES: usize = 1_048_576;

const OUTPUT_HEADER_BYTES: usize = 32;
const OUTPUT_STREAM_BYTES: usize = MAX_OUTPUT_BYTES - OUTPUT_HEADER_BYTES;

/// A closed status in the `semaprax.process.v1` failure domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessFailure {
    InvalidInput,
    AuthorityDenied,
    LaunchFailed,
    TimedOut,
    CapacityExceeded,
    IoFailure,
    SettlementFailed,
}

impl ProcessFailure {
    pub const DOMAIN: &'static str = "semaprax.process.v1";

    pub const fn domain(self) -> &'static str {
        Self::DOMAIN
    }

    pub const fn status_code(self) -> u32 {
        match self {
            Self::InvalidInput => 1,
            Self::AuthorityDenied => 2,
            Self::LaunchFailed => 3,
            Self::TimedOut => 4,
            Self::CapacityExceeded => 5,
            Self::IoFailure => 6,
            Self::SettlementFailed => 7,
        }
    }
}

/// An immutable, fully admitted process request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessRequest {
    tool: u64,
    arguments: Vec<Vec<u8>>,
    stdin: Vec<u8>,
    timeout_ms: u64,
    stdout_max: usize,
    stderr_max: usize,
    input_bytes: usize,
}

impl ProcessRequest {
    /// Decode the complete bounded argv wire and stdin input.
    ///
    /// `argv_length` and `stdin_length` describe admitted prefixes of supplied
    /// staging buffers. The argv parser consumes exactly its declared prefix,
    /// so bytes cannot be silently ignored within the admitted wire.
    #[allow(clippy::too_many_arguments)]
    pub fn from_wire(
        tool: u64,
        argv: &[u8],
        argv_length: usize,
        stdin: &[u8],
        stdin_length: usize,
        timeout_ms: u64,
        stdout_max: usize,
        stderr_max: usize,
    ) -> Result<Self, ProcessFailure> {
        if argv_length > argv.len()
            || stdin_length > stdin.len()
            || !(1..=MAX_TIMEOUT_MS).contains(&timeout_ms)
        {
            return Err(ProcessFailure::InvalidInput);
        }
        if argv_length
            .checked_add(stdin_length)
            .ok_or(ProcessFailure::CapacityExceeded)?
            > MAX_INPUT_BYTES
            || stdout_max > OUTPUT_STREAM_BYTES
            || stderr_max > OUTPUT_STREAM_BYTES
            || stdout_max
                .checked_add(stderr_max)
                .ok_or(ProcessFailure::CapacityExceeded)?
                > OUTPUT_STREAM_BYTES
        {
            return Err(ProcessFailure::CapacityExceeded);
        }

        let mut cursor = 0usize;
        let argv = &argv[..argv_length];
        let count = read_u32(argv, &mut cursor)? as usize;
        if count > MAX_ARGUMENTS {
            return Err(ProcessFailure::CapacityExceeded);
        }
        let mut arguments = Vec::with_capacity(count);
        for _ in 0..count {
            let length = read_u32(argv, &mut cursor)? as usize;
            let end = cursor
                .checked_add(length)
                .ok_or(ProcessFailure::InvalidInput)?;
            let bytes = argv.get(cursor..end).ok_or(ProcessFailure::InvalidInput)?;
            if bytes.contains(&0) {
                return Err(ProcessFailure::InvalidInput);
            }
            arguments.push(bytes.to_vec());
            cursor = end;
        }
        if cursor != argv_length {
            return Err(ProcessFailure::InvalidInput);
        }

        Ok(Self {
            tool,
            arguments,
            stdin: stdin[..stdin_length].to_vec(),
            timeout_ms,
            stdout_max,
            stderr_max,
            input_bytes: argv_length + stdin_length,
        })
    }

    pub const fn tool(&self) -> u64 {
        self.tool
    }
    pub fn arguments(&self) -> &[Vec<u8>] {
        &self.arguments
    }
    pub fn argument(&self, index: usize) -> Option<&[u8]> {
        self.arguments.get(index).map(Vec::as_slice)
    }
    pub fn stdin(&self) -> &[u8] {
        &self.stdin
    }
    pub const fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }
    pub const fn stdout_max(&self) -> usize {
        self.stdout_max
    }
    pub const fn stderr_max(&self) -> usize {
        self.stderr_max
    }
    pub const fn input_bytes(&self) -> usize {
        self.input_bytes
    }
    /// Bytes reserved for the output header and both requested streams.
    pub const fn reserved_output_bytes(&self) -> usize {
        OUTPUT_HEADER_BYTES + self.stdout_max + self.stderr_max
    }
}

fn read_u32(input: &[u8], cursor: &mut usize) -> Result<u32, ProcessFailure> {
    let end = cursor.checked_add(4).ok_or(ProcessFailure::InvalidInput)?;
    let bytes: [u8; 4] = input
        .get(*cursor..end)
        .ok_or(ProcessFailure::InvalidInput)?
        .try_into()
        .map_err(|_| ProcessFailure::InvalidInput)?;
    *cursor = end;
    Ok(u32::from_le_bytes(bytes))
}

/// How a completed process terminated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessTermination {
    Exited(u32),
    Signalled(u8),
}

impl ProcessTermination {
    fn packed(self) -> Result<u64, ProcessFailure> {
        match self {
            Self::Exited(code) => Ok(u64::from(code) << 2),
            Self::Signalled(0) => Err(ProcessFailure::InvalidInput),
            Self::Signalled(signal) => Ok((u64::from(signal) << 2) | 1),
        }
    }

    fn unpack(packed: u64) -> Result<Self, ProcessFailure> {
        match packed & 3 {
            0 if packed >> 2 <= u64::from(u32::MAX) => Ok(Self::Exited((packed >> 2) as u32)),
            1 => match u8::try_from(packed >> 2) {
                Ok(signal @ 1..=u8::MAX) => Ok(Self::Signalled(signal)),
                _ => Err(ProcessFailure::InvalidInput),
            },
            _ => Err(ProcessFailure::InvalidInput),
        }
    }
}

/// Bounded process output, owned by the host/provider boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessOutput {
    pub termination: ProcessTermination,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl ProcessOutput {
    /// Encode the canonical `semaprax.process.v1` output wire.
    pub fn encode(&self, request: &ProcessRequest) -> Result<Vec<u8>, ProcessFailure> {
        self.validate(request.stdout_max(), request.stderr_max())
            .map_err(|failure| match failure {
                ProcessFailure::InvalidInput => ProcessFailure::IoFailure,
                other => other,
            })?;
        let total = OUTPUT_HEADER_BYTES
            .checked_add(self.stdout.len())
            .and_then(|value| value.checked_add(self.stderr.len()))
            .ok_or(ProcessFailure::CapacityExceeded)?;
        let mut wire = Vec::with_capacity(total);
        for word in [
            1u64,
            self.termination.packed()?,
            self.stdout.len() as u64,
            self.stderr.len() as u64,
        ] {
            wire.extend_from_slice(&word.to_le_bytes());
        }
        wire.extend_from_slice(&self.stdout);
        wire.extend_from_slice(&self.stderr);
        Ok(wire)
    }

    /// Decode and authenticate a complete canonical output wire for one
    /// request's declared stream capacities.
    pub fn decode(
        wire: &[u8],
        stdout_max: usize,
        stderr_max: usize,
    ) -> Result<Self, ProcessFailure> {
        if wire.len() < OUTPUT_HEADER_BYTES {
            return Err(ProcessFailure::InvalidInput);
        }
        if wire.len() > MAX_OUTPUT_BYTES {
            return Err(ProcessFailure::CapacityExceeded);
        }
        let version = read_word(wire, 0)?;
        if version != 1 {
            return Err(ProcessFailure::InvalidInput);
        }
        let termination = ProcessTermination::unpack(read_word(wire, 8)?)?;
        let stdout_length =
            usize::try_from(read_word(wire, 16)?).map_err(|_| ProcessFailure::CapacityExceeded)?;
        let stderr_length =
            usize::try_from(read_word(wire, 24)?).map_err(|_| ProcessFailure::CapacityExceeded)?;
        let expected = OUTPUT_HEADER_BYTES
            .checked_add(stdout_length)
            .and_then(|value| value.checked_add(stderr_length))
            .ok_or(ProcessFailure::CapacityExceeded)?;
        if expected != wire.len() {
            return Err(ProcessFailure::InvalidInput);
        }
        let output = Self {
            termination,
            stdout: wire[OUTPUT_HEADER_BYTES..OUTPUT_HEADER_BYTES + stdout_length].to_vec(),
            stderr: wire[OUTPUT_HEADER_BYTES + stdout_length..].to_vec(),
        };
        output.validate(stdout_max, stderr_max)?;
        Ok(output)
    }

    pub fn validate(&self, stdout_max: usize, stderr_max: usize) -> Result<(), ProcessFailure> {
        self.validate_shape()?;
        if stdout_max > OUTPUT_STREAM_BYTES
            || stderr_max > OUTPUT_STREAM_BYTES
            || stdout_max
                .checked_add(stderr_max)
                .ok_or(ProcessFailure::CapacityExceeded)?
                > OUTPUT_STREAM_BYTES
            || self.stdout.len() > stdout_max
            || self.stderr.len() > stderr_max
            || self
                .stdout
                .len()
                .checked_add(self.stderr.len())
                .ok_or(ProcessFailure::CapacityExceeded)?
                > OUTPUT_STREAM_BYTES
        {
            return Err(ProcessFailure::CapacityExceeded);
        }
        Ok(())
    }

    fn validate_shape(&self) -> Result<(), ProcessFailure> {
        self.termination.packed()?;
        let stream_bytes = self
            .stdout
            .len()
            .checked_add(self.stderr.len())
            .ok_or(ProcessFailure::CapacityExceeded)?;
        if stream_bytes > OUTPUT_STREAM_BYTES {
            return Err(ProcessFailure::CapacityExceeded);
        }
        Ok(())
    }
}

fn read_word(input: &[u8], offset: usize) -> Result<u64, ProcessFailure> {
    let end = offset.checked_add(8).ok_or(ProcessFailure::InvalidInput)?;
    let bytes: [u8; 8] = input
        .get(offset..end)
        .ok_or(ProcessFailure::InvalidInput)?
        .try_into()
        .map_err(|_| ProcessFailure::InvalidInput)?;
    Ok(u64::from_le_bytes(bytes))
}

/// Explicit provider authority for an admitted process invocation.
pub trait ProcessProvider {
    fn run(&mut self, request: &ProcessRequest) -> Result<ProcessOutput, ProcessFailure>;
    fn settle(&mut self) -> Result<(), ProcessFailure>;
}

/// The default provider for a host that did not grant process authority.
#[derive(Default)]
pub struct DeniedProcessProvider;

impl ProcessProvider for DeniedProcessProvider {
    fn run(&mut self, _request: &ProcessRequest) -> Result<ProcessOutput, ProcessFailure> {
        Err(ProcessFailure::AuthorityDenied)
    }

    fn settle(&mut self) -> Result<(), ProcessFailure> {
        Ok(())
    }
}

/// One deterministic request/response pair for [`FixtureProcessProvider`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixtureProcessStep {
    pub request: ProcessRequest,
    pub response: Result<ProcessOutput, ProcessFailure>,
}

/// An explicit in-memory provider for deterministic tests and browser hosts.
pub struct FixtureProcessProvider {
    steps: VecDeque<FixtureProcessStep>,
    settlements: usize,
}

impl FixtureProcessProvider {
    pub fn new(steps: impl IntoIterator<Item = FixtureProcessStep>) -> Self {
        Self {
            steps: steps.into_iter().collect(),
            settlements: 0,
        }
    }

    pub fn remaining(&self) -> usize {
        self.steps.len()
    }
    pub fn settlements(&self) -> usize {
        self.settlements
    }
}

impl ProcessProvider for FixtureProcessProvider {
    fn run(&mut self, request: &ProcessRequest) -> Result<ProcessOutput, ProcessFailure> {
        let Some(step) = self.steps.pop_front() else {
            return Err(ProcessFailure::InvalidInput);
        };
        if step.request != *request {
            return Err(ProcessFailure::InvalidInput);
        }
        step.response
    }

    fn settle(&mut self) -> Result<(), ProcessFailure> {
        self.settlements = self.settlements.saturating_add(1);
        Ok(())
    }
}

/// Per-invocation reservation ledger. A failed provider call never refunds an
/// admitted reservation, so an invocation cannot turn retry failures into
/// unbounded host work.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProcessInvocationBudget {
    runs: usize,
    total_bytes: usize,
}

impl ProcessInvocationBudget {
    pub const fn new() -> Self {
        Self {
            runs: 0,
            total_bytes: 0,
        }
    }
    pub const fn runs(&self) -> usize {
        self.runs
    }
    pub const fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    pub fn reserve(&mut self, request: &ProcessRequest) -> Result<(), ProcessFailure> {
        let reservation = request
            .input_bytes()
            .checked_add(request.reserved_output_bytes())
            .ok_or(ProcessFailure::CapacityExceeded)?;
        let total = self
            .total_bytes
            .checked_add(reservation)
            .ok_or(ProcessFailure::CapacityExceeded)?;
        if self.runs >= MAX_RUNS || total > MAX_TOTAL_BYTES {
            return Err(ProcessFailure::CapacityExceeded);
        }
        self.runs += 1;
        self.total_bytes = total;
        Ok(())
    }
}

#[cfg(test)]
#[path = "process_provider/tests.rs"]
mod tests;
