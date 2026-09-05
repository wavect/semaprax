//! Host-injected transport for Bounded Language Network I/O v1.
//!
//! The compiler grants no ambient network authority. A hosted invocation
//! that reaches a network operation does so only because the host handed the
//! interpreter a [`NetworkProvider`]; the reference `run` and `interpret`
//! paths hold none and refuse every network operation before it executes.
//!
//! The split of responsibilities is fixed:
//!
//! - The evaluator owns the program-visible handle table (dense `1..=8` per
//!   invocation, never reused), the argument capacities, and the cumulative
//!   byte budget. It validates the endpoint shape before a provider sees it.
//! - The provider owns transport only: it opens, transfers on, waits on, and
//!   closes provider-side connections. It never sees a program handle.
//!
//! Three providers ship with the compiler. [`FixtureNetworkProvider`] replays
//! a deterministic `semaprax.network-fixture.v1` document and is the only
//! provider tests and browsers need. [`DeniedNetworkProvider`] refuses every
//! operation with `AUTHORITY_DENIED`. [`TcpNetworkProvider`] opens real
//! `std::net` sockets and is constructed only by an explicit host caller.

pub mod deadline;
mod fixture;
pub mod resolver;
mod tcp;

pub use deadline::{
    DeadlinePolicy, MonotonicClock, ScriptedClock, SystemClock, DEFAULT_OPERATION_DEADLINE,
    MAX_OPERATION_DEADLINE,
};
pub use fixture::{FixtureNetworkProvider, FIXTURE_SCHEMA_V3, MAX_NETWORK_FIXTURE_BYTES};
pub use resolver::{NameResolver, ResolveFailure, ScriptedResolver, SystemResolver};
pub use tcp::TcpNetworkProvider;

use crate::network_io_ops;

/// One normalized failure from a complete HTTPS request. This is kept
/// separate from stream failures because URL validation and HTTP protocol
/// negotiation happen above a single transport connection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpFailure {
    InvalidUrl,
    InsecureScheme,
    TransportFailed,
    ResponseTooLarge,
    UnsupportedVersion,
    AuthorityDenied,
}

impl HttpFailure {
    /// The `semaprax.http.v1` status code this failure normalizes to.
    pub const fn status_code(self) -> u32 {
        match self {
            Self::InvalidUrl => network_io_ops::HTTP_INVALID_URL,
            Self::InsecureScheme => network_io_ops::HTTP_INSECURE_SCHEME,
            Self::TransportFailed => network_io_ops::HTTP_TRANSPORT_FAILED,
            Self::ResponseTooLarge => network_io_ops::HTTP_RESPONSE_TOO_LARGE,
            Self::UnsupportedVersion => network_io_ops::HTTP_UNSUPPORTED_VERSION,
            Self::AuthorityDenied => network_io_ops::HTTP_AUTHORITY_DENIED,
        }
    }
}

/// One normalized network failure. Every variant maps onto exactly one code
/// of the closed `semaprax.network.v1` status domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkFailure {
    /// The connection attempt was refused, unreachable, or timed out (`1`).
    ConnectFailed,
    /// The host bytes or port are outside the admitted shape (`2`).
    InvalidEndpoint,
    /// The connection is unknown, stale, or already closed (`3`).
    UnknownHandle,
    /// A handle, chunk, cumulative-byte, or timeout capacity was exceeded
    /// (`4`).
    CapacityExceeded,
    /// The peer reset the connection or a transfer failed midway (`5`).
    TransferFailed,
    /// No network authority was granted to this invocation (`6`).
    AuthorityDenied,
    /// TLS negotiation or certificate/name validation failed (`7`).
    TlsFailed,
    /// Binding a listening socket failed (`8`).
    ListenFailed,
    /// Accepting a connection failed (`9`).
    AcceptFailed,
}

impl NetworkFailure {
    /// The `semaprax.network.v1` status code this failure normalizes to.
    pub const fn status_code(self) -> u32 {
        match self {
            Self::ConnectFailed => network_io_ops::CONNECT_FAILED,
            Self::InvalidEndpoint => network_io_ops::INVALID_ENDPOINT,
            Self::UnknownHandle => network_io_ops::UNKNOWN_HANDLE,
            Self::CapacityExceeded => network_io_ops::CAPACITY_EXCEEDED,
            Self::TransferFailed => network_io_ops::TRANSFER_FAILED,
            Self::AuthorityDenied => network_io_ops::AUTHORITY_DENIED,
            Self::TlsFailed => network_io_ops::TLS_FAILED,
            Self::ListenFailed => network_io_ops::LISTEN_FAILED,
            Self::AcceptFailed => network_io_ops::ACCEPT_FAILED,
        }
    }
}

/// The readiness a `net_wait` call observed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WaitState {
    /// The timeout elapsed with nothing to read (`0`).
    Timeout,
    /// At least one byte is readable without blocking (`1`).
    Readable,
    /// The peer closed the stream; the next read observes end of stream
    /// (`2`).
    Closed,
}

impl WaitState {
    /// The `net_wait` result value the program observes.
    pub const fn code(self) -> u64 {
        match self {
            Self::Timeout => network_io_ops::WAIT_TIMEOUT,
            Self::Readable => network_io_ops::WAIT_READABLE,
            Self::Closed => network_io_ops::WAIT_CLOSED,
        }
    }
}

/// A provider-side connection token. It is distinct from the program-visible
/// handle: the evaluator maps each dense handle onto the token the provider
/// returned from [`NetworkProvider::connect`], so a forged program handle can
/// never name a provider connection.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProviderConnection(u64);

impl ProviderConnection {
    /// Wrap a provider-chosen token.
    pub const fn new(token: u64) -> Self {
        Self(token)
    }

    /// The provider-chosen token.
    pub const fn token(self) -> u64 {
        self.0
    }
}

/// Provider-side listening-socket token, never exposed directly to a program.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProviderListener(u64);

impl ProviderListener {
    pub const fn new(token: u64) -> Self {
        Self(token)
    }
    pub const fn token(self) -> u64 {
        self.0
    }
}

/// Transport behind the six network operations.
///
/// Implementations receive endpoints the evaluator already validated (non-empty
/// UTF-8 host of at most 253 bytes without NUL, port `1..=65535`) and byte
/// counts already within the chunk and cumulative capacities. They may still
/// reject anything they cannot serve; every failure is normalized through
/// [`NetworkFailure`].
///
/// A provider is invocation-scoped. The evaluator calls [`settle`] exactly once
/// when the invocation settles, on every outcome, and the provider must then
/// release every connection it still holds regardless of whether the program
/// closed it.
///
/// [`settle`]: NetworkProvider::settle
pub trait NetworkProvider {
    /// Fetch one HTTPS URL and return a bounded, canonical HTTP/1.1-shaped
    /// response that can be consumed by `std.http`.
    fn https_get(&mut self, _url: &str, _max: usize) -> Result<Vec<u8>, HttpFailure> {
        Err(HttpFailure::AuthorityDenied)
    }

    /// Open a connection to `host:port`.
    fn connect(&mut self, host: &str, port: u16) -> Result<ProviderConnection, NetworkFailure>;

    /// Open a TLS 1.2/1.3 client connection with authenticated server name.
    fn connect_tls(
        &mut self,
        _host: &str,
        _port: u16,
    ) -> Result<ProviderConnection, NetworkFailure> {
        Err(NetworkFailure::AuthorityDenied)
    }

    /// Bind a TCP listening socket to an explicitly admitted local address.
    fn listen(&mut self, _host: &str, _port: u16) -> Result<ProviderListener, NetworkFailure> {
        Err(NetworkFailure::AuthorityDenied)
    }

    /// Accept one connection from a listening socket.
    fn accept(
        &mut self,
        _listener: ProviderListener,
    ) -> Result<ProviderConnection, NetworkFailure> {
        Err(NetworkFailure::AuthorityDenied)
    }

    /// Accept one connection and authenticate it as a TLS server.
    fn accept_tls(
        &mut self,
        _listener: ProviderListener,
    ) -> Result<ProviderConnection, NetworkFailure> {
        Err(NetworkFailure::AuthorityDenied)
    }

    /// Close one listening socket.
    fn close_listener(&mut self, _listener: ProviderListener) -> Result<(), NetworkFailure> {
        Err(NetworkFailure::AuthorityDenied)
    }

    /// Write every byte of `bytes` and return the count written. A provider
    /// that cannot write the whole slice fails with
    /// [`NetworkFailure::TransferFailed`] rather than returning a short count.
    fn send(
        &mut self,
        connection: ProviderConnection,
        bytes: &[u8],
    ) -> Result<usize, NetworkFailure>;

    /// Perform one blocking read of at most `max` bytes. An empty result means
    /// the peer closed the stream.
    fn recv(
        &mut self,
        connection: ProviderConnection,
        max: usize,
    ) -> Result<Vec<u8>, NetworkFailure>;

    /// Wait up to `timeout_ms` milliseconds for the connection to become
    /// readable or closed.
    fn wait(
        &mut self,
        connection: ProviderConnection,
        timeout_ms: u32,
    ) -> Result<WaitState, NetworkFailure>;

    /// Close one connection. The token is unknown afterwards.
    fn close(&mut self, connection: ProviderConnection) -> Result<(), NetworkFailure>;

    /// Release every connection the provider still holds. Called once at
    /// invocation settlement on every outcome.
    fn settle(&mut self);
}

/// A provider that grants nothing: every operation fails with
/// [`NetworkFailure::AuthorityDenied`]. Hosts use it to run a network-profile
/// command whose network authority they refuse at runtime, so the program
/// observes the normalized `AUTHORITY_DENIED` status instead of an admission
/// error.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DeniedNetworkProvider;

impl NetworkProvider for DeniedNetworkProvider {
    fn connect(&mut self, _host: &str, _port: u16) -> Result<ProviderConnection, NetworkFailure> {
        Err(NetworkFailure::AuthorityDenied)
    }

    fn send(&mut self, _: ProviderConnection, _: &[u8]) -> Result<usize, NetworkFailure> {
        Err(NetworkFailure::AuthorityDenied)
    }

    fn recv(&mut self, _: ProviderConnection, _: usize) -> Result<Vec<u8>, NetworkFailure> {
        Err(NetworkFailure::AuthorityDenied)
    }

    fn wait(&mut self, _: ProviderConnection, _: u32) -> Result<WaitState, NetworkFailure> {
        Err(NetworkFailure::AuthorityDenied)
    }

    fn close(&mut self, _: ProviderConnection) -> Result<(), NetworkFailure> {
        Err(NetworkFailure::AuthorityDenied)
    }

    fn settle(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failures_and_wait_states_map_onto_the_closed_tables() {
        let failures = [
            NetworkFailure::ConnectFailed,
            NetworkFailure::InvalidEndpoint,
            NetworkFailure::UnknownHandle,
            NetworkFailure::CapacityExceeded,
            NetworkFailure::TransferFailed,
            NetworkFailure::AuthorityDenied,
            NetworkFailure::TlsFailed,
            NetworkFailure::ListenFailed,
            NetworkFailure::AcceptFailed,
        ];
        let codes: Vec<u32> = failures.iter().map(|f| f.status_code()).collect();
        assert_eq!(codes, network_io_ops::SERVICE_STATUS_CODES);
        assert_eq!(WaitState::Timeout.code(), 0);
        assert_eq!(WaitState::Readable.code(), 1);
        assert_eq!(WaitState::Closed.code(), 2);
        assert_eq!(ProviderConnection::new(7).token(), 7);
        assert_eq!(ProviderListener::new(3).token(), 3);
    }
}
