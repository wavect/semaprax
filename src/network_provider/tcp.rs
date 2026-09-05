//! Real-socket provider over `std::net::TcpStream`.
//!
//! This provider opens real TCP sockets on the host, optionally wraps either
//! side in explicit Rustls client/server policy, and applies a 30 second
//! connect, accept, read, and write timeout. Nothing in the compiler constructs
//! it implicitly: a host that wants a program to reach the network must build
//! one and pass it to `hosted_interpreter::execute_network_command` itself.

use std::collections::BTreeMap;
use std::io::{ErrorKind, Read as _, Write as _};
use std::net::{Shutdown, TcpListener, TcpStream, ToSocketAddrs as _};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::{
    HttpFailure, NetworkFailure, NetworkProvider, ProviderConnection, ProviderListener, WaitState,
};
use crate::network_io_ops;

/// Connect, read, and write timeout applied to every socket.
pub const SOCKET_TIMEOUT: Duration = Duration::from_secs(30);

/// A [`NetworkProvider`] over blocking `std::net` sockets.
///
/// **This opens real network connections.** Plain operations send cleartext;
/// TLS operations use only explicitly installed Rustls policy. There is no
/// proxying. The compiler, reference interpreter's effect-free paths, and
/// fixture-driven test seams never construct this provider implicitly.
#[derive(Debug)]
pub struct TcpNetworkProvider {
    streams: BTreeMap<u64, TransportStream>,
    listeners: BTreeMap<u64, TcpListener>,
    next_token: u64,
    next_listener_token: u64,
    tls_config: Arc<rustls::ClientConfig>,
    server_tls_config: Option<Arc<rustls::ServerConfig>>,
    #[cfg(not(any(target_arch = "wasm32", target_arch = "wasm64")))]
    https_client: Option<crate::https_client::HttpsClient>,
}

#[derive(Debug)]
enum TransportStream {
    Tcp(TcpStream),
    TlsClient(Box<rustls::StreamOwned<rustls::ClientConnection, TcpStream>>),
    TlsServer(Box<rustls::StreamOwned<rustls::ServerConnection, TcpStream>>),
}

impl TransportStream {
    fn socket(&self) -> &TcpStream {
        match self {
            Self::Tcp(stream) => stream,
            Self::TlsClient(stream) => stream.get_ref(),
            Self::TlsServer(stream) => stream.get_ref(),
        }
    }

    fn socket_mut(&mut self) -> &mut TcpStream {
        match self {
            Self::Tcp(stream) => stream,
            Self::TlsClient(stream) => stream.get_mut(),
            Self::TlsServer(stream) => stream.get_mut(),
        }
    }
}

impl std::io::Read for TransportStream {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Tcp(stream) => stream.read(buffer),
            Self::TlsClient(stream) => stream.read(buffer),
            Self::TlsServer(stream) => stream.read(buffer),
        }
    }
}

impl std::io::Write for TransportStream {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Tcp(stream) => stream.write(bytes),
            Self::TlsClient(stream) => stream.write(bytes),
            Self::TlsServer(stream) => stream.write(bytes),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::Tcp(stream) => stream.flush(),
            Self::TlsClient(stream) => stream.flush(),
            Self::TlsServer(stream) => stream.flush(),
        }
    }
}

impl Default for TcpNetworkProvider {
    fn default() -> Self {
        let roots =
            rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let tls_config = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .expect("rustls ring provider has safe protocol versions")
        .with_root_certificates(roots)
        .with_no_client_auth();
        #[cfg(not(any(target_arch = "wasm32", target_arch = "wasm64")))]
        let https_client = crate::https_client::HttpsClient::with_tls_config(
            crate::https_client::HttpsClientConfig::default(),
            tls_config.clone(),
        )
        .ok();
        Self {
            streams: BTreeMap::new(),
            listeners: BTreeMap::new(),
            next_token: 0,
            next_listener_token: 0,
            tls_config: Arc::new(tls_config),
            server_tls_config: None,
            #[cfg(not(any(target_arch = "wasm32", target_arch = "wasm64")))]
            https_client,
        }
    }
}

impl TcpNetworkProvider {
    /// Create a provider holding no connections.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a provider with an explicitly constructed Rustls client policy.
    /// This is useful for private roots and deterministic local conformance;
    /// callers remain responsible for choosing an appropriate trust store.
    pub fn with_tls_config(config: Arc<rustls::ClientConfig>) -> Self {
        let mut provider = Self::default();
        #[cfg(not(any(target_arch = "wasm32", target_arch = "wasm64")))]
        {
            provider.https_client = crate::https_client::HttpsClient::with_tls_config(
                crate::https_client::HttpsClientConfig::default(),
                config.as_ref().clone(),
            )
            .ok();
        }
        provider.tls_config = config;
        provider
    }

    /// Create a provider with explicit client and server TLS policies.
    /// Server-side TLS remains unavailable unless the host supplies the
    /// certificate chain and private key through this constructor.
    pub fn with_tls_configs(
        client: Arc<rustls::ClientConfig>,
        server: Arc<rustls::ServerConfig>,
    ) -> Self {
        let mut provider = Self::with_tls_config(client);
        provider.server_tls_config = Some(server);
        provider
    }

    fn stream_mut(
        &mut self,
        connection: ProviderConnection,
    ) -> Result<&mut TransportStream, NetworkFailure> {
        self.streams
            .get_mut(&connection.token())
            .ok_or(NetworkFailure::UnknownHandle)
    }

    fn connect_socket(host: &str, port: u16) -> Result<TcpStream, NetworkFailure> {
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
        Ok(stream)
    }

    fn insert_stream(
        &mut self,
        stream: TransportStream,
    ) -> Result<ProviderConnection, NetworkFailure> {
        let token = self.next_token;
        self.next_token = token
            .checked_add(1)
            .ok_or(NetworkFailure::CapacityExceeded)?;
        self.streams.insert(token, stream);
        Ok(ProviderConnection::new(token))
    }

    fn accept_socket(&self, listener: ProviderListener) -> Result<TcpStream, NetworkFailure> {
        let listener = self
            .listeners
            .get(&listener.token())
            .ok_or(NetworkFailure::UnknownHandle)?;
        let deadline = Instant::now() + SOCKET_TIMEOUT;
        let stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(error)
                    if error.kind() == ErrorKind::WouldBlock && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(_) => return Err(NetworkFailure::AcceptFailed),
            }
        };
        stream
            .set_nonblocking(false)
            .and_then(|()| stream.set_read_timeout(Some(SOCKET_TIMEOUT)))
            .and_then(|()| stream.set_write_timeout(Some(SOCKET_TIMEOUT)))
            .map_err(|_| NetworkFailure::AcceptFailed)?;
        Ok(stream)
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
    #[cfg(not(any(target_arch = "wasm32", target_arch = "wasm64")))]
    fn https_get(&mut self, url: &str, max: usize) -> Result<Vec<u8>, HttpFailure> {
        let client = self
            .https_client
            .as_ref()
            .ok_or(HttpFailure::TransportFailed)?;
        client.get_canonical(url, max).map_err(|error| match error {
            crate::https_client::HttpsError::InvalidConfiguration => HttpFailure::ResponseTooLarge,
            crate::https_client::HttpsError::InvalidUrl => HttpFailure::InvalidUrl,
            crate::https_client::HttpsError::InsecureScheme => HttpFailure::InsecureScheme,
            crate::https_client::HttpsError::TransportFailed => HttpFailure::TransportFailed,
            crate::https_client::HttpsError::ResponseTooLarge => HttpFailure::ResponseTooLarge,
            crate::https_client::HttpsError::UnsupportedVersion => HttpFailure::UnsupportedVersion,
        })
    }

    fn connect(&mut self, host: &str, port: u16) -> Result<ProviderConnection, NetworkFailure> {
        let stream = Self::connect_socket(host, port)?;
        self.insert_stream(TransportStream::Tcp(stream))
    }

    fn connect_tls(&mut self, host: &str, port: u16) -> Result<ProviderConnection, NetworkFailure> {
        let mut socket = Self::connect_socket(host, port)?;
        let server_name = rustls::pki_types::ServerName::try_from(host.to_owned())
            .map_err(|_| NetworkFailure::InvalidEndpoint)?;
        let mut connection = rustls::ClientConnection::new(self.tls_config.clone(), server_name)
            .map_err(|_| NetworkFailure::TlsFailed)?;
        connection
            .complete_io(&mut socket)
            .map_err(|_| NetworkFailure::TlsFailed)?;
        self.insert_stream(TransportStream::TlsClient(Box::new(
            rustls::StreamOwned::new(connection, socket),
        )))
    }

    fn listen(&mut self, host: &str, port: u16) -> Result<ProviderListener, NetworkFailure> {
        validate_endpoint(host, port)?;
        let listener = TcpListener::bind((host, port)).map_err(|_| NetworkFailure::ListenFailed)?;
        listener
            .set_nonblocking(true)
            .map_err(|_| NetworkFailure::ListenFailed)?;
        let token = self.next_listener_token;
        self.next_listener_token = token
            .checked_add(1)
            .ok_or(NetworkFailure::CapacityExceeded)?;
        self.listeners.insert(token, listener);
        Ok(ProviderListener::new(token))
    }

    fn accept(&mut self, listener: ProviderListener) -> Result<ProviderConnection, NetworkFailure> {
        let stream = self.accept_socket(listener)?;
        self.insert_stream(TransportStream::Tcp(stream))
    }

    fn accept_tls(
        &mut self,
        listener: ProviderListener,
    ) -> Result<ProviderConnection, NetworkFailure> {
        let mut socket = self.accept_socket(listener)?;
        let config = self
            .server_tls_config
            .clone()
            .ok_or(NetworkFailure::AuthorityDenied)?;
        let mut connection =
            rustls::ServerConnection::new(config).map_err(|_| NetworkFailure::TlsFailed)?;
        connection
            .complete_io(&mut socket)
            .map_err(|_| NetworkFailure::TlsFailed)?;
        self.insert_stream(TransportStream::TlsServer(Box::new(
            rustls::StreamOwned::new(connection, socket),
        )))
    }

    fn close_listener(&mut self, listener: ProviderListener) -> Result<(), NetworkFailure> {
        self.listeners
            .remove(&listener.token())
            .map(|_| ())
            .ok_or(NetworkFailure::UnknownHandle)
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
                .socket_mut()
                .set_nonblocking(true)
                .map_err(|_| NetworkFailure::TransferFailed)?;
            let peeked = stream.socket().peek(&mut probe);
            stream
                .socket_mut()
                .set_nonblocking(false)
                .map_err(|_| NetworkFailure::TransferFailed)?;
            peeked
        } else {
            stream
                .socket_mut()
                .set_read_timeout(Some(Duration::from_millis(u64::from(timeout_ms))))
                .map_err(|_| NetworkFailure::TransferFailed)?;
            let peeked = stream.socket().peek(&mut probe);
            stream
                .socket_mut()
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
        let _ = stream.socket().shutdown(Shutdown::Both);
        Ok(())
    }

    fn settle(&mut self) {
        for (_, stream) in std::mem::take(&mut self.streams) {
            let _ = stream.socket().shutdown(Shutdown::Both);
        }
        self.listeners.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode64(input: &str) -> Vec<u8> {
        fn digit(byte: u8) -> Option<u8> {
            match byte {
                b'A'..=b'Z' => Some(byte - b'A'),
                b'a'..=b'z' => Some(byte - b'a' + 26),
                b'0'..=b'9' => Some(byte - b'0' + 52),
                b'+' => Some(62),
                b'/' => Some(63),
                _ => None,
            }
        }
        let mut output = Vec::new();
        let mut bits = 0u32;
        let mut count = 0u8;
        for byte in input.bytes().filter(|byte| *byte != b'=') {
            bits = (bits << 6) | u32::from(digit(byte).unwrap());
            count += 6;
            if count >= 8 {
                count -= 8;
                output.push((bits >> count) as u8);
                bits &= (1u32 << count) - 1;
            }
        }
        output
    }

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

    #[test]
    fn real_listener_accepts_and_settles_a_loopback_connection() {
        let reservation = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = reservation.local_addr().unwrap().port();
        drop(reservation);
        let mut provider = TcpNetworkProvider::new();
        let listener = provider.listen("127.0.0.1", port).unwrap();
        let client = std::thread::spawn(move || {
            let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
            stream.write_all(b"ping").unwrap();
        });
        let connection = provider.accept(listener).unwrap();
        assert_eq!(provider.recv(connection, 4), Ok(b"ping".to_vec()));
        assert_eq!(provider.close_listener(listener), Ok(()));
        provider.settle();
        client.join().unwrap();
    }

    #[test]
    #[allow(
        dead_code,
        reason = "legacy self-signed fixture retained for rejection calibration"
    )]
    fn tls_client_authenticates_name_and_transfers_over_loopback() {
        const CERT: &str = "MIIDHzCCAgegAwIBAgIUFIcDxjb6bqmjBssaRaZHxICzAbUwDQYJKoZIhvcNAQELBQAwFDESMBAGA1UEAwwJbG9jYWxob3N0MB4XDTI2MDkwNTE0NDkwNVoXDTM2MDkwMjE0NDkwNVowFDESMBAGA1UEAwwJbG9jYWxob3N0MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAomKQfiLXU09Rj6V2AlzcOsqnmbj+bHggdPg9wgBqHnRXmzvzCz8KpWk28KpKgpJ9lWi7g5UF8MNDy/0eWUNjnJwG2T/H7cUpj7F3OHF+hoJSH/N+I13CS1hvgrrpYjDDhMcW/D+3E6P7ayQsHnY9R4Y7Y2jOrZIXLke+3MbIQ5E4ry+tSbPW+BY+zh+kBRVEGPnXXB8mTJ5GZvbW+h0WMbWRDgZfI6iSsjPASmBpj7bgh8nOiMY5ryntTPsbf9rWdCrVMKL0jOlgJGLlUgjsS+bi1LpjCJh1lbbTRTrdemLujucQSsj+CKrvsL0SWsE8cv2IXsfK10aT3FYuoh4KuwIDAQABo2kwZzAdBgNVHQ4EFgQUxrBtA/xM4knpIdRnrOKM+ziffPMwHwYDVR0jBBgwFoAUxrBtA/xM4knpIdRnrOKM+ziffPMwDwYDVR0TAQH/BAUwAwEB/zAUBgNVHREEDTALgglsb2NhbGhvc3QwDQYJKoZIhvcNAQELBQADggEBAD2B86rFdIc2rpj+xIP4/brBhosRUT7P8BjkEN0rVFBZQW8VnasartEWFN+f6Z1QgVjF6/CMINj+ereFT15aKVnG6AQD7xOaXf1ipp/8Ni9OBV5NP6iL5qyrkcHjEjeziEBgltvrK8qLDtDJweQEoD1QHQDHcdLSwt2DExR2U/ADu0FykKlJ51qpWlvL4hPzAKgHUiXhDZBxIX5kFF2L2jXRJFeY8j/SgAHPC8pdaeUorjVSxML92BPGf8BCf3ktnQSGnJX8Mvs4KdzEejJNPA/2vosddP+z1Al2nCQdCjtzsgsrku0Tg8g5CUuNy14gy85z9whd0I1/uk4VPdccZAA=";
        const KEY: &str = "MIIEvAIBADANBgkqhkiG9w0BAQEFAASCBKYwggSiAgEAAoIBAQCiYpB+ItdTT1GPpXYCXNw6yqeZuP5seCB0+D3CAGoedFebO/MLPwqlaTbwqkqCkn2VaLuDlQXww0PL/R5ZQ2OcnAbZP8ftxSmPsXc4cX6GglIf834jXcJLWG+CuuliMMOExxb8P7cTo/trJCwedj1HhjtjaM6tkhcuR77cxshDkTivL61Js9b4Fj7OH6QFFUQY+ddcHyZMnkZm9tb6HRYxtZEOBl8jqJKyM8BKYGmPtuCHyc6IxjmvKe1M+xt/2tZ0KtUwovSM6WAkYuVSCOxL5uLUumMImHWVttNFOt16Yu6O5xBKyP4Iqu+wvRJawTxy/Yhex8rXRpPcVi6iHgq7AgMBAAECggEADTb4NfZKj6e6JiEmTrWVNojyGrsd+WB37mJFTwRkSRYutZ4AqWmintNxJSS2lj8ATqhh734GfbwQ8wjQ73K3KIeKByP+9oU/sfHp7INP80z9DJyZfJyksyep63mf2d3ItjAoLrRWBxh73WbphZEZwOMA8kC/5mAn2BxT6/jr/ejc+LqvAXLgGd0JDAtCupgri7XK8lCXSwk36yMSvGdPFAR2QxpdQ4xl65KYI/uHG/rODWyayKKkDHJB3q0BGp3n8uo33R1YPHrgJIxrdlLB1FOt1a69+xeGi6aeikH9TJ0dD/FvfI+7f4J5MO/cCpPkgqv+DXuuW3lVi8ZbN9woAQKBgQDXAROPJHwJvCD5NPUL/wWFgSqOZ3kvNtl/l7YUbP4IxFzzd3QTVQsl+72Wkjhd+GaEM8TX3rSZDtjLJElfNpIhztGb6mc5oSrHab5EVY1CKxFwXSJjm9XTcfvtV74oKwfPOAGu0/8ReZqqRPdOLpXJbmmGXSynmwRF+LS0EDGJmQKBgQDBWQGibVHW3N0Rc1PeZgzjyGKtbjTdLtUEzvs65XidAu+slcX9kj9GL6cjAPC6lzjW0C4KowZEQQnKTmFcsD5XrbFISEmNpjgaSu+i5xCFb6xiJVN4S/vmFUYvW4qudwxSbLpmbJYUyYdn0TnhHI6Qp6BC1NnNIcceijo7KunzcwKBgGe/VTjVWiU4apDWRQis3nU1htuAgrGNvhYblvj0PwDsAA5brd9GQkLp3uoxVJHDs3RHpsyj4nGZAHPF5sHTC2DU88BQs87TPllLZUyEG826Cog16Mo4AE4vymkU1eV8HiCX3fgGxCYij8dp0Awh3pV8ed8kRs/5tW4uPMRGrCDBAoGATXFjMDXtU8x/V6AD9c6WVx5KOAEud3FsrVJiWoLTPsCQU2ZiOWC8q1Ym8eRMh9BOWexkpKoLtob+buPaJ5AISIIvwi4CGBR94Donpe47Ndc3CtC8kDCPIudeh1V5RMw2SUV3m9LeglD+RV0Oe9Y+XD5n+Jzc6EchRGBFVrGnp3UCgYAcWyNY04NlMX8jVkRMhXHChicIMKuUFER4VCdF48fdafra4nfsst5GHSufRyzfW74mEJTeSJFp5im5cAvbEClrsDM8scScXKbtirBmE7eQYK8pOFTvl2svJX0XsL5G5xbouw5tMvH27Nfx16PL84B4oo4r5cOCDTLW+rI7PFvPsQ=";
        const ROOT: &str = "MIIDJzCCAg+gAwIBAgIUC3kI/KYpwSCFZIOpQLwZZv3fpIUwDQYJKoZIhvcNAQELBQAwGzEZMBcGA1UEAwwQU0VNQVBSQVggVGVzdCBDQTAeFw0yNjA5MDUxNDU3MTlaFw0zNjA5MDIxNDU3MTlaMBsxGTAXBgNVBAMMEFNFTUFQUkFYIFRlc3QgQ0EwggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQCtxpzwCk3e4aRY3ozKBTi94gfLHe6yKDfDggOHGiwUGotJ9dVH8e4Hh82JamO+jH694HBmjlbGXF+BY7Gxv/Vz8Z7R9VqS1uND7J4V4pJABLL4H//k/c0WPMopTkQRmVyit34hTob14aL+hPq4DFOtH+FxXiUyPaJp6xP0UH7KTJpSBJfBlTAmJoBuMP7Ara05oozrVuLNzSDaUulGGkA5kUuv2GnPvQjTx8PG14GUfJt6okOD64JJSaoQCrraxyHIG8UmZgnHyoIq3UgFY9gj4haVW6ykKe+bkWVbwCOZcMAffzx+NKDodSahn3Qy2z0eDI0ARMtVFDE+ijtxlG/1AgMBAAGjYzBhMB0GA1UdDgQWBBT4Dg/tRse2xlFPUoKfa/7M5c40VjAfBgNVHSMEGDAWgBT4Dg/tRse2xlFPUoKfa/7M5c40VjAPBgNVHRMBAf8EBTADAQH/MA4GA1UdDwEB/wQEAwIBBjANBgkqhkiG9w0BAQsFAAOCAQEAmEWc71S2305pR9Ps29VDVdwOcVoetWsqEnCsAIHg0qfioQz3mznfxE3gOZ4gm03AOslf2sqq8ev02MnEuZWt7Y7xwstrTyo0EA4mWXzBTz0EX7Qp1PgV4MV7Lifp+Dv5ACDx75bgOziKx+u6VVvR0RoE1tUB3m3ihO7aT0HMXOBvElkuY7Ev+fR7lgSFOPGYV2IIBcfaro0dGJlixyBjP/TLGAr8S6buf0ZFCBKtMriXyfiqcQ8IPeLEOtFGxhrWKoNoRpkYwM5kut27vDkoc5UekFmU4EaGPl0cWEpoky5RMXgrA0hAzKEmgPnbIVplKwdoELQjon+MR1HA9txCeg==";
        const LEAF: &str = "MIIDSjCCAjKgAwIBAgIUK81c/KylyZTx6OJ/K9lJP7OLzBgwDQYJKoZIhvcNAQELBQAwGzEZMBcGA1UEAwwQU0VNQVBSQVggVGVzdCBDQTAeFw0yNjA5MDUxNDU3MTlaFw0zNjA5MDIxNDU3MTlaMBQxEjAQBgNVBAMMCWxvY2FsaG9zdDCCASIwDQYJKoZIhvcNAQEBBQADggEPADCCAQoCggEBAM6ibgX7OJCn5nsP0DH497ZCdsxQN23ifpv3ZWWNbKScZi4k5R0nZqJb/asrOa/vgc/An5YBYdsHV/9SqE7CVxhgCj+sYo6W2RfyDV8PF3fztxg+1Varrm0RcI4DaZN2N7fqdxZPvpIl//3n3J2G6J2d919ZPZpog0ahqlHjfvmIh1ESeS2XIu1T4dHlBvW1m3AgoFneNZDHDQs9ziuKte6KShv2I6rOzIRSC5vHM4YsDC64NANbheAV0L98rc/51A6jJxziKQtpFDhBHGvAhag3JkOUyLP7fiIPiHBI0Qxmh70EBj2EgUo5OqV1pNytbH4zBrKlyjQj+R2o8ReNpY8CAwEAAaOBjDCBiTAUBgNVHREEDTALgglsb2NhbGhvc3QwDAYDVR0TAQH/BAIwADAOBgNVHQ8BAf8EBAMCBaAwEwYDVR0lBAwwCgYIKwYBBQUHAwEwHQYDVR0OBBYEFD69svZnO8+sMQfesN19Zk40CBU8MB8GA1UdIwQYMBaAFPgOD+1Gx7bGUU9Sgp9r/szlzjRWMA0GCSqGSIb3DQEBCwUAA4IBAQAwcYsnw9zK+9lMrIN6zSxry26FFIjOP/ZRXSeloNPA2Fd2p+16b7RoHL+tcn4P4NMCKsz2Y+faX6lzSzIi0lydRsM8rH3xY4/Y8UDoLyC6zDQXpZNbEyWQALgKoZjV8l4XEbtmhLx++h2wArD/eEneBW3aCL8QzNgTU6gyobp1y6AqxQPnl+2SpBlFtpnoz0W3CCOGc0UiaobxBNTYydtY37vGQPLs32drQ2E0o9RfD+4/MTTkS380fXI4pEW4XOm/AofuMwVz1zkWXY/CzYp+1czf7/sOLDTsuwt0/QJFhK3IGSBL1wH3lU8BUHC6LMysilY3Eujo+Ya7dHAyM0lb";
        const LEAF_KEY: &str = "MIIEvAIBADANBgkqhkiG9w0BAQEFAASCBKYwggSiAgEAAoIBAQDOom4F+ziQp+Z7D9Ax+Pe2QnbMUDdt4n6b92VljWyknGYuJOUdJ2aiW/2rKzmv74HPwJ+WAWHbB1f/UqhOwlcYYAo/rGKOltkX8g1fDxd387cYPtVWq65tEXCOA2mTdje36ncWT76SJf/959ydhuidnfdfWT2aaINGoapR4375iIdREnktlyLtU+HR5Qb1tZtwIKBZ3jWQxw0LPc4rirXuikob9iOqzsyEUgubxzOGLAwuuDQDW4XgFdC/fK3P+dQOoycc4ikLaRQ4QRxrwIWoNyZDlMiz+34iD4hwSNEMZoe9BAY9hIFKOTqldaTcrWx+Mwaypco0I/kdqPEXjaWPAgMBAAECggEAS9lKyq5HOq4vB8Aru5Q4lXH7Oo89cXwA3o5m7WqG1TvFtC193oA+h919lW3F/KNNgq2hxsXWHjipYAL+3f4vSzbBvFKyUMXlhYknyFt5UWIoNOGnnOtjGQ0cRDzTbbooxL1vnkSCXxJMz+5iyH4jd+vqyFixKLMxcOVZ6Do6OyzuFK2hq1dp2R+fk0TVyQAFTtqSVC5DR/dxzX+mIkkzJWJvfsTnlBZ19j9q8ft0XnOfEpHDSfxzoOXx1SdF+CvA15kjmWVUQbHTMgcPni90NhomPgdlhqXfHx+N+ar3GJO9+GJ8QGhwPXGRGpa81lkQZMTb0Q+rsbqws3Xvl1Nz4QKBgQDtKB7jWevWtakv6k8i6HVe4iGxBwYAHUKe8IrMZt5HQ0gs4iBU6kwZtgW9c02VeHYHnSf/oEF/2OnXpxyQjiHR5LkcZ87lnuivX0bZo8Ijt1dXfczQFZA/zCfpuoTHSQKD8Mw5MbrQ1XrRZaYZMlZ6f0OBPMN8P1657nVwCg3RIQKBgQDfDXj8HqC2blafwwb2dUvKQSH7J4biz7QFl/ZTCJyEu8SSLNJRnKyrIC5mewdJFM3CT9eqIklNkrxbIqd0URy0i512cVIjQmGTtaD0c3S361N9MStlKwsrCtj7Oy4qBdlq/lG03pMubWntRdXnm6e+l+KG6fZ+h+W5y6MEXLWwrwKBgHsfISoXPQEzPqrJklwlIwonjCZD5zGX/0ZUyzpjDXMh0w66Nt7e5LNUdJZujhDTgTNiu6lSoa6mBoEXGRVTNOurOw8sNZWwckzZwgarpda1EHszrGk7SLBWZUJKuzRbCxtEoEHxN3PD4QdlJl5ea9ccywcFbNfMbnlI+183WQUBAoGAVyqBrC0f6wsFiRuC/g9qldiMOgUBXmOC22i+V0aXO/vQ3rrrWf9bLui9mUjc2P9rRVNEWXVaphkAyLCrNfZ4vEmPOHkieyr2zO1+v+japQEuuE7dwYRnseNkVhGTgdKVW42VSpRseglCCvpulDss+3uJh+WocVwUN15QD2VXj3sCgYAyP2FCNPdfg1r2LcNMn06gwnLz+NHn4HK1PNjrRTQgrKYG9xf8gvM0HgoSdR1mfDjdPqgPMdLFG23jmpOG23waokgIsBl88SGdaCVJ/+Ti4WFHhKkhRwgmNX/4se+JsD5nSGaBwkrZ6uyLs+W39hFa0MQzDdRCQjsuuRWFsn7YpA==";
        let cert = rustls::pki_types::CertificateDer::from(decode64(LEAF));
        let key = rustls::pki_types::PrivatePkcs8KeyDer::from(decode64(LEAF_KEY));
        let crypto = || Arc::new(rustls::crypto::ring::default_provider());
        let server_config = rustls::ServerConfig::builder_with_provider(crypto())
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(vec![cert.clone()], key.into())
            .unwrap();
        let mut roots = rustls::RootCertStore::empty();
        roots
            .add(rustls::pki_types::CertificateDer::from(decode64(ROOT)))
            .unwrap();
        let client_config = rustls::ClientConfig::builder_with_provider(crypto())
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(roots)
            .with_no_client_auth();
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (socket, _) = listener.accept().unwrap();
            let connection = rustls::ServerConnection::new(Arc::new(server_config)).unwrap();
            let mut stream = rustls::StreamOwned::new(connection, socket);
            let mut input = [0u8; 4];
            stream.read_exact(&mut input).unwrap();
            assert_eq!(&input, b"ping");
            stream.write_all(b"pong").unwrap();
            stream.flush().unwrap();
        });
        let mut provider = TcpNetworkProvider::with_tls_config(Arc::new(client_config.clone()));
        let connection = provider.connect_tls("localhost", port).unwrap();
        assert_eq!(provider.send(connection, b"ping"), Ok(4));
        assert_eq!(provider.recv(connection, 4), Ok(b"pong".to_vec()));
        provider.settle();
        server.join().unwrap();

        let https_listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let https_port = https_listener.local_addr().unwrap().port();
        let https_cert = rustls::pki_types::CertificateDer::from(decode64(LEAF));
        let https_key = rustls::pki_types::PrivatePkcs8KeyDer::from(decode64(LEAF_KEY));
        let https_server_config = rustls::ServerConfig::builder_with_provider(crypto())
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(vec![https_cert], https_key.into())
            .unwrap();
        let https_server = std::thread::spawn(move || {
            let (socket, _) = https_listener.accept().unwrap();
            let connection = rustls::ServerConnection::new(Arc::new(https_server_config)).unwrap();
            let mut stream = rustls::StreamOwned::new(connection, socket);
            let mut request = Vec::new();
            let mut byte = [0u8; 1];
            while !request.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            assert!(request.starts_with(b"GET / HTTP/1.1\r\n"));
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .unwrap();
            stream.flush().unwrap();
        });
        let mut https_provider = TcpNetworkProvider::with_tls_config(Arc::new(client_config));
        let response = https_provider
            .https_get(&format!("https://localhost:{https_port}/"), 1024)
            .unwrap();
        assert!(response.starts_with(b"HTTP/1.1 200 semaprax\r\n"));
        assert!(response.ends_with(b"\r\n\r\nok"));
        https_server.join().unwrap();

        let server_cert = rustls::pki_types::CertificateDer::from(decode64(LEAF));
        let server_key = rustls::pki_types::PrivatePkcs8KeyDer::from(decode64(LEAF_KEY));
        let server_config = rustls::ServerConfig::builder_with_provider(crypto())
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(vec![server_cert], server_key.into())
            .unwrap();
        let mut server_roots = rustls::RootCertStore::empty();
        server_roots
            .add(rustls::pki_types::CertificateDer::from(decode64(ROOT)))
            .unwrap();
        let server_client_config = rustls::ClientConfig::builder_with_provider(crypto())
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(server_roots)
            .with_no_client_auth();
        let reservation = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let server_port = reservation.local_addr().unwrap().port();
        drop(reservation);
        let mut server_provider = TcpNetworkProvider::with_tls_configs(
            Arc::new(server_client_config.clone()),
            Arc::new(server_config),
        );
        let server_listener = server_provider.listen("127.0.0.1", server_port).unwrap();
        let tls_client = std::thread::spawn(move || {
            let socket = TcpStream::connect(("127.0.0.1", server_port)).unwrap();
            let name = rustls::pki_types::ServerName::try_from("localhost").unwrap();
            let connection =
                rustls::ClientConnection::new(Arc::new(server_client_config), name).unwrap();
            let mut stream = rustls::StreamOwned::new(connection, socket);
            stream.write_all(b"ping").unwrap();
            stream.flush().unwrap();
            let mut output = [0u8; 4];
            stream.read_exact(&mut output).unwrap();
            assert_eq!(&output, b"pong");
        });
        let accepted = server_provider.accept_tls(server_listener).unwrap();
        let received = server_provider.recv(accepted, 4);
        assert_eq!(received, Ok(b"ping".to_vec()), "server TLS receive failed");
        assert_eq!(server_provider.send(accepted, b"pong"), Ok(4));
        server_provider.settle();
        tls_client.join().unwrap();
    }
}
