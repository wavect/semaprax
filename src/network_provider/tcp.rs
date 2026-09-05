//! Real-socket provider over `std::net::TcpStream`.
//!
//! This provider opens real TCP sockets on the host. It performs no TLS, uses
//! no name resolution beyond the platform resolver, and applies a 30 second
//! connect, read, and write timeout. Nothing in the compiler constructs it
//! implicitly: a host that wants a program to reach the network must build one
//! and pass it to `hosted_interpreter::execute_network_command` itself.

use std::collections::BTreeMap;
use std::io::{ErrorKind, Read as _, Write as _};
use std::net::{Shutdown, TcpStream, ToSocketAddrs as _};
use std::time::Duration;

use super::{NetworkFailure, NetworkProvider, ProviderConnection, WaitState};
use crate::network_io_ops;

/// Connect, read, and write timeout applied to every socket.
pub const SOCKET_TIMEOUT: Duration = Duration::from_secs(30);

/// A [`NetworkProvider`] over blocking `std::net` sockets.
///
/// **This opens real network connections.** There is no TLS and no proxying;
/// bytes go to the named peer in the clear. Only an explicit host caller
/// constructs it; the compiler, the reference interpreter's effect-free
/// paths, and the fixture-driven test seams never do.
#[derive(Debug, Default)]
pub struct TcpNetworkProvider {
    streams: BTreeMap<u64, TcpStream>,
    next_token: u64,
}

impl TcpNetworkProvider {
    /// Create a provider holding no connections.
    pub fn new() -> Self {
        Self::default()
    }

    fn stream_mut(
        &mut self,
        connection: ProviderConnection,
    ) -> Result<&mut TcpStream, NetworkFailure> {
        self.streams
            .get_mut(&connection.token())
            .ok_or(NetworkFailure::UnknownHandle)
    }
}

/// Reject endpoints the evaluator already excludes, so a direct library caller
/// gets the same closed shape.
fn validate_endpoint(host: &str, port: u16) -> Result<(), NetworkFailure> {
    let host_bytes = host.len() as u64;
    if host.is_empty()
        || host_bytes > network_io_ops::MAX_HOST_BYTES
        || host.as_bytes().contains(&0)
        || port == 0
    {
        return Err(NetworkFailure::InvalidEndpoint);
    }
    Ok(())
}

fn classify_wait_error(error: &std::io::Error) -> Result<WaitState, NetworkFailure> {
    match error.kind() {
        ErrorKind::WouldBlock | ErrorKind::TimedOut => Ok(WaitState::Timeout),
        _ => Err(NetworkFailure::TransferFailed),
    }
}

impl NetworkProvider for TcpNetworkProvider {
    fn connect(&mut self, host: &str, port: u16) -> Result<ProviderConnection, NetworkFailure> {
        validate_endpoint(host, port)?;
        let addresses = (host, port)
            .to_socket_addrs()
            .map_err(|_| NetworkFailure::ConnectFailed)?;
        let mut stream = None;
        for address in addresses {
            if let Ok(connected) = TcpStream::connect_timeout(&address, SOCKET_TIMEOUT) {
                stream = Some(connected);
                break;
            }
        }
        let stream = stream.ok_or(NetworkFailure::ConnectFailed)?;
        stream
            .set_read_timeout(Some(SOCKET_TIMEOUT))
            .and_then(|()| stream.set_write_timeout(Some(SOCKET_TIMEOUT)))
            .map_err(|_| NetworkFailure::ConnectFailed)?;
        let token = self.next_token;
        self.next_token = token
            .checked_add(1)
            .ok_or(NetworkFailure::CapacityExceeded)?;
        self.streams.insert(token, stream);
        Ok(ProviderConnection::new(token))
    }

    fn send(
        &mut self,
        connection: ProviderConnection,
        bytes: &[u8],
    ) -> Result<usize, NetworkFailure> {
        let stream = self.stream_mut(connection)?;
        stream
            .write_all(bytes)
            .and_then(|()| stream.flush())
            .map_err(|_| NetworkFailure::TransferFailed)?;
        Ok(bytes.len())
    }

    fn recv(
        &mut self,
        connection: ProviderConnection,
        max: usize,
    ) -> Result<Vec<u8>, NetworkFailure> {
        let stream = self.stream_mut(connection)?;
        let mut buffer = vec![0u8; max];
        loop {
            match stream.read(&mut buffer) {
                Ok(count) => {
                    buffer.truncate(count);
                    return Ok(buffer);
                }
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(_) => return Err(NetworkFailure::TransferFailed),
            }
        }
    }

    fn wait(
        &mut self,
        connection: ProviderConnection,
        timeout_ms: u32,
    ) -> Result<WaitState, NetworkFailure> {
        let stream = self.stream_mut(connection)?;
        let mut probe = [0u8; 1];
        let observed = if timeout_ms == 0 {
            stream
                .set_nonblocking(true)
                .map_err(|_| NetworkFailure::TransferFailed)?;
            let peeked = stream.peek(&mut probe);
            stream
                .set_nonblocking(false)
                .map_err(|_| NetworkFailure::TransferFailed)?;
            peeked
        } else {
            stream
                .set_read_timeout(Some(Duration::from_millis(u64::from(timeout_ms))))
                .map_err(|_| NetworkFailure::TransferFailed)?;
            let peeked = stream.peek(&mut probe);
            stream
                .set_read_timeout(Some(SOCKET_TIMEOUT))
                .map_err(|_| NetworkFailure::TransferFailed)?;
            peeked
        };
        match observed {
            Ok(0) => Ok(WaitState::Closed),
            Ok(_) => Ok(WaitState::Readable),
            Err(error) => classify_wait_error(&error),
        }
    }

    fn close(&mut self, connection: ProviderConnection) -> Result<(), NetworkFailure> {
        let stream = self
            .streams
            .remove(&connection.token())
            .ok_or(NetworkFailure::UnknownHandle)?;
        // A peer that already reset the connection makes shutdown fail; the
        // program observes success because the handle is released either way.
        let _ = stream.shutdown(Shutdown::Both);
        Ok(())
    }

    fn settle(&mut self) {
        for (_, stream) in std::mem::take(&mut self.streams) {
            let _ = stream.shutdown(Shutdown::Both);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_endpoints_are_rejected_before_resolution() {
        let mut provider = TcpNetworkProvider::new();
        assert_eq!(
            provider.connect("", 80),
            Err(NetworkFailure::InvalidEndpoint)
        );
        assert_eq!(
            provider.connect("127.0.0.1", 0),
            Err(NetworkFailure::InvalidEndpoint)
        );
        assert_eq!(
            provider.connect("a\0b", 80),
            Err(NetworkFailure::InvalidEndpoint)
        );
        let long = "a".repeat(254);
        assert_eq!(
            provider.connect(&long, 80),
            Err(NetworkFailure::InvalidEndpoint)
        );
        assert_eq!(
            provider.recv(ProviderConnection::new(9), 1),
            Err(NetworkFailure::UnknownHandle)
        );
    }
}
