//! Deterministic replay provider for the `semaprax.network-fixture.v1`
//! document.
//!
//! ```json
//! {
//!   "schema": "semaprax.network-fixture.v1",
//!   "connections": [
//!     {
//!       "host": "example.org",
//!       "port": 80,
//!       "recv": ["<utf-8 chunk>", "..."],
//!       "expect_send": "<optional exact cumulative bytes sent before the first recv>",
//!       "ready": true
//!     }
//!   ]
//! }
//! ```
//!
//! Connections are matched in `net_connect` order: the i-th successful
//! connect binds the i-th fixture connection, and host and port must match
//! exactly or the attempt fails with `CONNECT_FAILED` without consuming the
//! fixture entry. `recv` chunks are delivered in order, each bounded by the
//! caller's `max`; a chunk larger than `max` is split and the remainder stays
//! pending. After the last chunk a read observes end of stream and a wait
//! observes `Closed`. `ready: false` makes the first wait time out once before
//! the connection reports readable.

use std::collections::VecDeque;

use serde_json::Value;

use super::{NetworkFailure, NetworkProvider, ProviderConnection, ProviderListener, WaitState};
use crate::diagnostic::Diagnostic;

/// The exact schema identity a fixture document must carry.
pub const FIXTURE_SCHEMA: &str = "semaprax.network-fixture.v1";
pub const FIXTURE_SCHEMA_V2: &str = "semaprax.network-fixture.v2";
/// Maximum canonical fixture document bytes accepted by any host lane.
pub const MAX_NETWORK_FIXTURE_BYTES: usize = 1_048_576;

const FIXTURE_DIAGNOSTIC_CODE: &str = "SPX-F110";

const CONNECTION_KEYS: [&str; 6] = ["host", "port", "recv", "expect_send", "ready", "tls"];

#[derive(Clone, Debug, Eq, PartialEq)]
struct FixtureConnection {
    host: String,
    port: u16,
    pending: VecDeque<Vec<u8>>,
    expect_send: Option<Vec<u8>>,
    ready: bool,
    sent: Vec<u8>,
    send_checked: bool,
    open: bool,
    tls: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FixtureListener {
    host: String,
    port: u16,
    accepted: VecDeque<FixtureConnection>,
    open: bool,
}

/// A replaying [`NetworkProvider`] driven by a fixture document. It performs
/// no I/O and is fully deterministic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixtureNetworkProvider {
    connections: Vec<FixtureConnection>,
    /// Index of the fixture connection the next successful connect binds.
    next_connection: usize,
    listeners: Vec<FixtureListener>,
    next_listener: usize,
}

fn fixture_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io(FIXTURE_DIAGNOSTIC_CODE, message)
}

impl FixtureNetworkProvider {
    /// Parse one `semaprax.network-fixture.v1` document. Unknown keys, a
    /// foreign schema identity, or an ill-typed field fail closed.
    pub fn from_json(document: &str) -> Result<Self, Diagnostic> {
        if document.len() > MAX_NETWORK_FIXTURE_BYTES {
            return Err(fixture_error(format!(
                "network fixture exceeds the {MAX_NETWORK_FIXTURE_BYTES}-byte limit"
            )));
        }
        let value: Value = serde_json::from_str(document)
            .map_err(|error| fixture_error(format!("network fixture is not JSON: {error}")))?;
        let root = value
            .as_object()
            .ok_or_else(|| fixture_error("network fixture must be a JSON object"))?;
        for key in root.keys() {
            if key != "schema" && key != "connections" && key != "listeners" {
                return Err(fixture_error(format!(
                    "network fixture has unknown key `{key}`"
                )));
            }
        }
        let schema = match root.get("schema").and_then(Value::as_str) {
            Some(FIXTURE_SCHEMA) => FIXTURE_SCHEMA,
            Some(FIXTURE_SCHEMA_V2) => FIXTURE_SCHEMA_V2,
            _ => {
                return Err(fixture_error(format!(
                    "network fixture must declare `schema`: \"{FIXTURE_SCHEMA}\" or \"{FIXTURE_SCHEMA_V2}\""
                )))
            }
        };
        if schema == FIXTURE_SCHEMA && root.contains_key("listeners") {
            return Err(fixture_error("network fixture v1 cannot carry listeners"));
        }
        let connections = root
            .get("connections")
            .and_then(Value::as_array)
            .ok_or_else(|| fixture_error("network fixture must carry a `connections` array"))?;
        if connections.len() > crate::network_io_ops::MAX_HANDLES as usize {
            return Err(fixture_error(format!(
                "network fixture exceeds the {}-connection limit",
                crate::network_io_ops::MAX_HANDLES
            )));
        }
        let connections = connections
            .iter()
            .enumerate()
            .map(|(index, connection)| {
                parse_connection(index, connection, schema == FIXTURE_SCHEMA_V2)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let listeners = match root.get("listeners") {
            None => Vec::new(),
            Some(Value::Array(listeners))
                if listeners.len() <= crate::network_io_ops::MAX_HANDLES as usize =>
            {
                listeners
                    .iter()
                    .enumerate()
                    .map(|(index, value)| parse_listener(index, value))
                    .collect::<Result<Vec<_>, _>>()?
            }
            Some(Value::Array(_)) => {
                return Err(fixture_error("network fixture exceeds the listener limit"))
            }
            Some(_) => {
                return Err(fixture_error(
                    "network fixture `listeners` must be an array",
                ))
            }
        };
        Ok(Self {
            connections,
            next_connection: 0,
            listeners,
            next_listener: 0,
        })
    }

    fn connection_mut(
        &mut self,
        connection: ProviderConnection,
    ) -> Result<&mut FixtureConnection, NetworkFailure> {
        usize::try_from(connection.token())
            .ok()
            .and_then(|index| self.connections.get_mut(index))
            .filter(|connection| connection.open)
            .ok_or(NetworkFailure::UnknownHandle)
    }
}

fn parse_connection(
    index: usize,
    connection: &Value,
    allow_tls: bool,
) -> Result<FixtureConnection, Diagnostic> {
    let object = connection.as_object().ok_or_else(|| {
        fixture_error(format!(
            "network fixture connection {index} must be a JSON object"
        ))
    })?;
    for key in object.keys() {
        if !CONNECTION_KEYS.contains(&key.as_str()) {
            return Err(fixture_error(format!(
                "network fixture connection {index} has unknown key `{key}`"
            )));
        }
    }
    let host = object
        .get("host")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            fixture_error(format!(
                "network fixture connection {index} must carry a string `host`"
            ))
        })?
        .to_owned();
    let port = object
        .get("port")
        .and_then(Value::as_u64)
        .and_then(|port| u16::try_from(port).ok())
        .filter(|port| *port != 0)
        .ok_or_else(|| {
            fixture_error(format!(
                "network fixture connection {index} must carry a `port` between 1 and 65535"
            ))
        })?;
    let pending = match object.get("recv") {
        None => VecDeque::new(),
        Some(Value::Array(chunks)) => chunks
            .iter()
            .map(|chunk| {
                chunk
                    .as_str()
                    .map(|chunk| chunk.as_bytes().to_vec())
                    .ok_or_else(|| {
                        fixture_error(format!(
                            "network fixture connection {index} `recv` chunks must be strings"
                        ))
                    })
            })
            .collect::<Result<VecDeque<_>, _>>()?,
        Some(_) => {
            return Err(fixture_error(format!(
                "network fixture connection {index} `recv` must be an array of strings"
            )))
        }
    };
    let expect_send = match object.get("expect_send") {
        None => None,
        Some(Value::String(expected)) => Some(expected.as_bytes().to_vec()),
        Some(_) => {
            return Err(fixture_error(format!(
                "network fixture connection {index} `expect_send` must be a string"
            )))
        }
    };
    let ready = match object.get("ready") {
        None => true,
        Some(Value::Bool(ready)) => *ready,
        Some(_) => {
            return Err(fixture_error(format!(
                "network fixture connection {index} `ready` must be a boolean"
            )))
        }
    };
    let tls = match object.get("tls") {
        None => false,
        Some(Value::Bool(value)) if allow_tls => *value,
        Some(Value::Bool(_)) => {
            return Err(fixture_error(
                "network fixture v1 cannot mark TLS connections",
            ))
        }
        Some(_) => {
            return Err(fixture_error(format!(
                "network fixture connection {index} `tls` must be a boolean"
            )))
        }
    };
    Ok(FixtureConnection {
        host,
        port,
        pending,
        expect_send,
        ready,
        sent: Vec::new(),
        send_checked: false,
        open: false,
        tls,
    })
}

fn parse_listener(index: usize, value: &Value) -> Result<FixtureListener, Diagnostic> {
    let object = value.as_object().ok_or_else(|| {
        fixture_error(format!(
            "network fixture listener {index} must be an object"
        ))
    })?;
    for key in object.keys() {
        if !["host", "port", "accept"].contains(&key.as_str()) {
            return Err(fixture_error(format!(
                "network fixture listener {index} has unknown key `{key}`"
            )));
        }
    }
    let host = object
        .get("host")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            fixture_error(format!(
                "network fixture listener {index} must carry a string `host`"
            ))
        })?
        .to_owned();
    let port = object
        .get("port")
        .and_then(Value::as_u64)
        .and_then(|port| u16::try_from(port).ok())
        .filter(|port| *port != 0)
        .ok_or_else(|| {
            fixture_error(format!(
                "network fixture listener {index} has invalid `port`"
            ))
        })?;
    let accepted = object
        .get("accept")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            fixture_error(format!(
                "network fixture listener {index} must carry an `accept` array"
            ))
        })?;
    if accepted.len() > crate::network_io_ops::MAX_HANDLES as usize {
        return Err(fixture_error(
            "network fixture listener accept queue exceeds the connection limit",
        ));
    }
    let accepted = accepted
        .iter()
        .enumerate()
        .map(|(connection_index, connection)| parse_connection(connection_index, connection, true))
        .collect::<Result<VecDeque<_>, _>>()?;
    Ok(FixtureListener {
        host,
        port,
        accepted,
        open: false,
    })
}

impl FixtureConnection {
    /// Enforce `expect_send` once, at the first receive.
    fn check_expected_send(&mut self) -> Result<(), NetworkFailure> {
        if self.send_checked {
            return Ok(());
        }
        self.send_checked = true;
        match &self.expect_send {
            Some(expected) if *expected != self.sent => Err(NetworkFailure::TransferFailed),
            _ => Ok(()),
        }
    }
}

impl NetworkProvider for FixtureNetworkProvider {
    fn connect(&mut self, host: &str, port: u16) -> Result<ProviderConnection, NetworkFailure> {
        let index = self.next_connection;
        let connection = self
            .connections
            .get_mut(index)
            .ok_or(NetworkFailure::ConnectFailed)?;
        if connection.host != host || connection.port != port {
            return Err(NetworkFailure::ConnectFailed);
        }
        if connection.tls {
            return Err(NetworkFailure::ConnectFailed);
        }
        connection.open = true;
        self.next_connection += 1;
        Ok(ProviderConnection::new(index as u64))
    }

    fn connect_tls(&mut self, host: &str, port: u16) -> Result<ProviderConnection, NetworkFailure> {
        let index = self.next_connection;
        let connection = self
            .connections
            .get_mut(index)
            .ok_or(NetworkFailure::TlsFailed)?;
        if connection.host != host || connection.port != port || !connection.tls {
            return Err(NetworkFailure::TlsFailed);
        }
        connection.open = true;
        self.next_connection += 1;
        Ok(ProviderConnection::new(index as u64))
    }

    fn listen(&mut self, host: &str, port: u16) -> Result<ProviderListener, NetworkFailure> {
        let index = self.next_listener;
        let listener = self
            .listeners
            .get_mut(index)
            .ok_or(NetworkFailure::ListenFailed)?;
        if listener.host != host || listener.port != port {
            return Err(NetworkFailure::ListenFailed);
        }
        listener.open = true;
        self.next_listener += 1;
        Ok(ProviderListener::new(index as u64))
    }

    fn accept(&mut self, listener: ProviderListener) -> Result<ProviderConnection, NetworkFailure> {
        let listener = usize::try_from(listener.token())
            .ok()
            .and_then(|index| self.listeners.get_mut(index))
            .filter(|listener| listener.open)
            .ok_or(NetworkFailure::UnknownHandle)?;
        let mut connection = listener
            .accepted
            .pop_front()
            .ok_or(NetworkFailure::AcceptFailed)?;
        connection.open = true;
        let token = self.connections.len() as u64;
        self.connections.push(connection);
        Ok(ProviderConnection::new(token))
    }

    fn close_listener(&mut self, listener: ProviderListener) -> Result<(), NetworkFailure> {
        let listener = usize::try_from(listener.token())
            .ok()
            .and_then(|index| self.listeners.get_mut(index))
            .filter(|listener| listener.open)
            .ok_or(NetworkFailure::UnknownHandle)?;
        listener.open = false;
        Ok(())
    }

    fn send(
        &mut self,
        connection: ProviderConnection,
        bytes: &[u8],
    ) -> Result<usize, NetworkFailure> {
        let connection = self.connection_mut(connection)?;
        connection.sent.extend_from_slice(bytes);
        if let Some(expected) = &connection.expect_send {
            if !connection.send_checked && !expected.starts_with(&connection.sent) {
                return Err(NetworkFailure::TransferFailed);
            }
        }
        Ok(bytes.len())
    }

    fn recv(
        &mut self,
        connection: ProviderConnection,
        max: usize,
    ) -> Result<Vec<u8>, NetworkFailure> {
        let connection = self.connection_mut(connection)?;
        connection.check_expected_send()?;
        let Some(front) = connection.pending.front_mut() else {
            return Ok(Vec::new());
        };
        if front.len() <= max {
            return Ok(connection
                .pending
                .pop_front()
                .expect("front chunk presence was checked"));
        }
        let rest = front.split_off(max);
        let delivered = std::mem::replace(front, rest);
        Ok(delivered)
    }

    fn wait(
        &mut self,
        connection: ProviderConnection,
        _timeout_ms: u32,
    ) -> Result<WaitState, NetworkFailure> {
        let connection = self.connection_mut(connection)?;
        if !connection.ready {
            connection.ready = true;
            return Ok(WaitState::Timeout);
        }
        if connection.pending.is_empty() {
            Ok(WaitState::Closed)
        } else {
            Ok(WaitState::Readable)
        }
    }

    fn close(&mut self, connection: ProviderConnection) -> Result<(), NetworkFailure> {
        self.connection_mut(connection)?.open = false;
        Ok(())
    }

    fn settle(&mut self) {
        for connection in &mut self.connections {
            connection.open = false;
        }
        for listener in &mut self.listeners {
            listener.open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOCUMENT: &str = r#"{
        "schema": "semaprax.network-fixture.v1",
        "connections": [
            {"host": "example.org", "port": 80, "recv": ["hello ", "world"], "expect_send": "GET", "ready": false},
            {"host": "example.org", "port": 81}
        ]
    }"#;

    #[test]
    fn replays_connections_in_order_with_chunk_splitting_and_eof() {
        let mut provider = FixtureNetworkProvider::from_json(DOCUMENT).unwrap();
        assert_eq!(
            provider.connect("example.org", 8080),
            Err(NetworkFailure::ConnectFailed)
        );
        let first = provider.connect("example.org", 80).unwrap();
        assert_eq!(provider.wait(first, 10), Ok(WaitState::Timeout));
        assert_eq!(provider.wait(first, 10), Ok(WaitState::Readable));
        assert_eq!(provider.send(first, b"GE"), Ok(2));
        assert_eq!(provider.send(first, b"T"), Ok(1));
        assert_eq!(provider.recv(first, 4), Ok(b"hell".to_vec()));
        assert_eq!(provider.recv(first, 64), Ok(b"o ".to_vec()));
        assert_eq!(provider.recv(first, 64), Ok(b"world".to_vec()));
        assert_eq!(provider.recv(first, 64), Ok(Vec::new()));
        assert_eq!(provider.wait(first, 10), Ok(WaitState::Closed));
        let second = provider.connect("example.org", 81).unwrap();
        assert_eq!(provider.recv(second, 64), Ok(Vec::new()));
        assert_eq!(provider.close(first), Ok(()));
        assert_eq!(provider.close(first), Err(NetworkFailure::UnknownHandle));
        assert_eq!(
            provider.connect("example.org", 81),
            Err(NetworkFailure::ConnectFailed)
        );
        provider.settle();
        assert_eq!(provider.recv(second, 1), Err(NetworkFailure::UnknownHandle));
    }

    #[test]
    fn expected_send_mismatch_is_a_transfer_failure() {
        let mut provider = FixtureNetworkProvider::from_json(DOCUMENT).unwrap();
        let first = provider.connect("example.org", 80).unwrap();
        assert_eq!(
            provider.send(first, b"POST"),
            Err(NetworkFailure::TransferFailed)
        );
        let mut provider = FixtureNetworkProvider::from_json(DOCUMENT).unwrap();
        let first = provider.connect("example.org", 80).unwrap();
        assert_eq!(provider.send(first, b"GE"), Ok(2));
        assert_eq!(provider.recv(first, 8), Err(NetworkFailure::TransferFailed));
    }

    #[test]
    fn malformed_documents_fail_closed() {
        for document in [
            "not json",
            "[]",
            r#"{"schema": "other", "connections": []}"#,
            r#"{"schema": "semaprax.network-fixture.v1"}"#,
            r#"{"schema": "semaprax.network-fixture.v1", "connections": [], "extra": 1}"#,
            r#"{"schema": "semaprax.network-fixture.v1", "connections": [{"host": "a", "port": 0}]}"#,
            r#"{"schema": "semaprax.network-fixture.v1", "connections": [{"host": "a", "port": 65536}]}"#,
            r#"{"schema": "semaprax.network-fixture.v1", "connections": [{"host": "a", "port": 1, "recv": "x"}]}"#,
            r#"{"schema": "semaprax.network-fixture.v1", "connections": [{"host": "a", "port": 1, "ready": 1}]}"#,
            r#"{"schema": "semaprax.network-fixture.v1", "connections": [{"host": "a", "port": 1, "other": 1}]}"#,
        ] {
            let error = FixtureNetworkProvider::from_json(document).unwrap_err();
            assert_eq!(error.code, FIXTURE_DIAGNOSTIC_CODE, "{document}");
        }
    }

    #[test]
    fn fixture_document_and_connection_counts_are_bounded() {
        let oversized = " ".repeat(MAX_NETWORK_FIXTURE_BYTES + 1);
        assert_eq!(
            FixtureNetworkProvider::from_json(&oversized)
                .unwrap_err()
                .code,
            FIXTURE_DIAGNOSTIC_CODE
        );
        let connection = r#"{"host":"a","port":1}"#;
        let too_many = format!(
            r#"{{"schema":"semaprax.network-fixture.v1","connections":[{}]}}"#,
            std::iter::repeat_n(connection, 9)
                .collect::<Vec<_>>()
                .join(",")
        );
        assert_eq!(
            FixtureNetworkProvider::from_json(&too_many)
                .unwrap_err()
                .code,
            FIXTURE_DIAGNOSTIC_CODE
        );
    }

    #[test]
    fn v2_replays_tls_and_listen_accept_lifecycles() {
        let document = r#"{
            "schema":"semaprax.network-fixture.v2",
            "connections":[{"host":"secure.example","port":443,"tls":true,"expect_send":"GET","recv":["ok"]}],
            "listeners":[{"host":"127.0.0.1","port":8080,"accept":[{"host":"peer","port":1,"recv":["hello"],"expect_send":"reply"}]}]
        }"#;
        let mut provider = FixtureNetworkProvider::from_json(document).unwrap();
        assert_eq!(
            provider.connect("secure.example", 443),
            Err(NetworkFailure::ConnectFailed)
        );
        let tls = provider.connect_tls("secure.example", 443).unwrap();
        assert_eq!(provider.send(tls, b"GET"), Ok(3));
        assert_eq!(provider.recv(tls, 8), Ok(b"ok".to_vec()));

        let listener = provider.listen("127.0.0.1", 8080).unwrap();
        let peer = provider.accept(listener).unwrap();
        assert_eq!(provider.recv(peer, 8), Err(NetworkFailure::TransferFailed));
        assert_eq!(provider.send(peer, b"reply"), Ok(5));
        assert_eq!(provider.recv(peer, 8), Ok(b"hello".to_vec()));
        assert_eq!(provider.close_listener(listener), Ok(()));
        assert_eq!(
            provider.accept(listener),
            Err(NetworkFailure::UnknownHandle)
        );
        provider.settle();
    }
}
