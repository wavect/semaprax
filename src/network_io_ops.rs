//! Compiler-owned, capability-authenticated network I/O operations.
//!
//! Bounded Language Network I/O v1 extends the closed host-command operation
//! family with explicit, effect-gated TCP operations. Like command I/O, these
//! are not authored imports: their complete signature, authority, status
//! space, and capacity are derived from the closed operation table here and
//! every backend consumes the same facts.
//!
//! Handles are invocation-scoped integer tokens. The adapter that grants
//! network authority closes every open handle at settlement regardless of the
//! semantic outcome; a forged, stale, or closed handle fails closed with the
//! normalized `semaprax.network.v1` status domain.

use crate::ast::{Param, ParamMode, Span, Type};
use crate::hir::{OwnershipMode, ResolvedHostCommandOperation, ResolvedType};

pub(crate) const NET_CONNECT_NAME: &str = "net_connect";
pub(crate) const NET_SEND_NAME: &str = "net_send";
pub(crate) const NET_RECV_NAME: &str = "net_recv";
pub(crate) const NET_STREAM_STDOUT_NAME: &str = "net_stream_stdout";
pub(crate) const NET_WAIT_NAME: &str = "net_wait";
pub(crate) const NET_CLOSE_NAME: &str = "net_close";
pub(crate) const NET_TLS_CONNECT_NAME: &str = "net_tls_connect";
pub(crate) const NET_LISTEN_NAME: &str = "net_listen";
pub(crate) const NET_ACCEPT_NAME: &str = "net_accept";
pub(crate) const NET_CLOSE_LISTENER_NAME: &str = "net_close_listener";
pub(crate) const NET_TLS_ACCEPT_NAME: &str = "net_tls_accept";
pub(crate) const HTTPS_GET_NAME: &str = "https_get";

pub(crate) const NET_CONNECT_ID: &str = "core.host.net-connect";
pub(crate) const NET_SEND_ID: &str = "core.host.net-send";
pub(crate) const NET_RECV_ID: &str = "core.host.net-recv";
pub(crate) const NET_STREAM_STDOUT_ID: &str = "core.host.net-stream-stdout";
pub(crate) const NET_WAIT_ID: &str = "core.host.net-wait";
pub(crate) const NET_CLOSE_ID: &str = "core.host.net-close";
pub(crate) const NET_TLS_CONNECT_ID: &str = "core.host.net-tls-connect";
pub(crate) const NET_LISTEN_ID: &str = "core.host.net-listen";
pub(crate) const NET_ACCEPT_ID: &str = "core.host.net-accept";
pub(crate) const NET_CLOSE_LISTENER_ID: &str = "core.host.net-close-listener";
pub(crate) const NET_TLS_ACCEPT_ID: &str = "core.host.net-tls-accept";
pub(crate) const HTTPS_GET_ID: &str = "core.host.https-get";

pub(crate) const NETWORK_CONNECT_EFFECT: &str = "network.connect";
pub(crate) const NETWORK_READ_EFFECT: &str = "network.read";
pub(crate) const NETWORK_WRITE_EFFECT: &str = "network.write";
pub(crate) const NETWORK_TLS_EFFECT: &str = "network.tls";
pub(crate) const NETWORK_LISTEN_EFFECT: &str = "network.listen";
pub(crate) const NETWORK_ACCEPT_EFFECT: &str = "network.accept";
pub(crate) const NETWORK_HTTP_EFFECT: &str = "network.http";

/// The closed effect inventory a network-capable module may permit in
/// addition to the Language Command I/O v1 process permits.
pub(crate) const NETWORK_EFFECTS: [&str; 7] = [
    NETWORK_CONNECT_EFFECT,
    NETWORK_READ_EFFECT,
    NETWORK_WRITE_EFFECT,
    NETWORK_TLS_EFFECT,
    NETWORK_LISTEN_EFFECT,
    NETWORK_ACCEPT_EFFECT,
    NETWORK_HTTP_EFFECT,
];

pub(crate) const STATUS_DOMAIN: &str = "semaprax.network.v1";
pub(crate) const SERVICE_STATUS_DOMAIN: &str = "semaprax.network-service.v1";
pub(crate) const HTTP_STATUS_DOMAIN: &str = "semaprax.http.v1";

/// The connection attempt was refused, unreachable, or timed out.
pub(crate) const CONNECT_FAILED: u32 = 1;
/// The host bytes or port are outside the admitted shape.
pub(crate) const INVALID_ENDPOINT: u32 = 2;
/// The handle is unknown, forged, stale, or already closed.
pub(crate) const UNKNOWN_HANDLE: u32 = 3;
/// A handle, chunk, cumulative-byte, or timeout capacity was exceeded.
pub(crate) const CAPACITY_EXCEEDED: u32 = 4;
/// The peer reset the connection or a transfer failed midway.
pub(crate) const TRANSFER_FAILED: u32 = 5;
/// No network authority was granted to this invocation.
pub(crate) const AUTHORITY_DENIED: u32 = 6;
/// TLS handshake or certificate/name validation failed.
pub(crate) const TLS_FAILED: u32 = 7;
/// A listening socket could not be bound.
pub(crate) const LISTEN_FAILED: u32 = 8;
/// Accepting a peer failed.
pub(crate) const ACCEPT_FAILED: u32 = 9;

pub(crate) const HTTP_INVALID_URL: u32 = 1;
pub(crate) const HTTP_INSECURE_SCHEME: u32 = 2;
pub(crate) const HTTP_TRANSPORT_FAILED: u32 = 3;
pub(crate) const HTTP_RESPONSE_TOO_LARGE: u32 = 4;
pub(crate) const HTTP_UNSUPPORTED_VERSION: u32 = 5;
pub(crate) const HTTP_AUTHORITY_DENIED: u32 = 6;

pub(crate) const STATUS_CODES: [u32; 6] = [
    CONNECT_FAILED,
    INVALID_ENDPOINT,
    UNKNOWN_HANDLE,
    CAPACITY_EXCEEDED,
    TRANSFER_FAILED,
    AUTHORITY_DENIED,
];

pub(crate) const SERVICE_STATUS_CODES: [u32; 9] = [
    CONNECT_FAILED,
    INVALID_ENDPOINT,
    UNKNOWN_HANDLE,
    CAPACITY_EXCEEDED,
    TRANSFER_FAILED,
    AUTHORITY_DENIED,
    TLS_FAILED,
    LISTEN_FAILED,
    ACCEPT_FAILED,
];

pub(crate) const HTTP_STATUS_CODES: [u32; 6] = [1, 2, 3, 4, 5, 6];

/// Open connections per invocation.
pub(crate) const MAX_HANDLES: u64 = 8;
/// Host name bytes accepted by `net_connect` (RFC 1035 name length).
pub(crate) const MAX_HOST_BYTES: u64 = 253;
/// Largest TCP port.
pub(crate) const MAX_PORT: u64 = 65_535;
/// Bytes one `net_recv` or `net_stream_stdout` call may deliver.
pub(crate) const MAX_CHUNK_BYTES: u64 = 65_536;
/// Cumulative received-plus-sent bytes per invocation.
pub(crate) const MAX_TOTAL_BYTES: u64 = 1_048_576;
/// Longest `net_wait` timeout in milliseconds.
pub(crate) const MAX_WAIT_MILLIS: u64 = 30_000;

/// `net_wait` results.
pub(crate) const WAIT_TIMEOUT: u64 = 0;
pub(crate) const WAIT_READABLE: u64 = 1;
pub(crate) const WAIT_CLOSED: u64 = 2;

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) const OPERATIONS: [ResolvedHostCommandOperation; 12] = [
    ResolvedHostCommandOperation::NetConnect,
    ResolvedHostCommandOperation::NetSend,
    ResolvedHostCommandOperation::NetRecv,
    ResolvedHostCommandOperation::NetStreamStdout,
    ResolvedHostCommandOperation::NetWait,
    ResolvedHostCommandOperation::NetClose,
    ResolvedHostCommandOperation::NetTlsConnect,
    ResolvedHostCommandOperation::NetListen,
    ResolvedHostCommandOperation::NetAccept,
    ResolvedHostCommandOperation::NetCloseListener,
    ResolvedHostCommandOperation::NetTlsAccept,
    ResolvedHostCommandOperation::HttpsGet,
];

pub(crate) const fn is_network(op: ResolvedHostCommandOperation) -> bool {
    matches!(
        op,
        ResolvedHostCommandOperation::NetConnect
            | ResolvedHostCommandOperation::NetSend
            | ResolvedHostCommandOperation::NetRecv
            | ResolvedHostCommandOperation::NetStreamStdout
            | ResolvedHostCommandOperation::NetWait
            | ResolvedHostCommandOperation::NetClose
            | ResolvedHostCommandOperation::NetTlsConnect
            | ResolvedHostCommandOperation::NetListen
            | ResolvedHostCommandOperation::NetAccept
            | ResolvedHostCommandOperation::NetCloseListener
            | ResolvedHostCommandOperation::NetTlsAccept
            | ResolvedHostCommandOperation::HttpsGet
    )
}

pub(crate) const fn is_http(op: ResolvedHostCommandOperation) -> bool {
    matches!(op, ResolvedHostCommandOperation::HttpsGet)
}

pub(crate) const fn is_service(op: ResolvedHostCommandOperation) -> bool {
    matches!(
        op,
        ResolvedHostCommandOperation::NetTlsConnect
            | ResolvedHostCommandOperation::NetListen
            | ResolvedHostCommandOperation::NetAccept
            | ResolvedHostCommandOperation::NetCloseListener
            | ResolvedHostCommandOperation::NetTlsAccept
    )
}

pub(crate) fn by_name(name: &str) -> Option<ResolvedHostCommandOperation> {
    match name {
        NET_CONNECT_NAME => Some(ResolvedHostCommandOperation::NetConnect),
        NET_SEND_NAME => Some(ResolvedHostCommandOperation::NetSend),
        NET_RECV_NAME => Some(ResolvedHostCommandOperation::NetRecv),
        NET_STREAM_STDOUT_NAME => Some(ResolvedHostCommandOperation::NetStreamStdout),
        NET_WAIT_NAME => Some(ResolvedHostCommandOperation::NetWait),
        NET_CLOSE_NAME => Some(ResolvedHostCommandOperation::NetClose),
        NET_TLS_CONNECT_NAME => Some(ResolvedHostCommandOperation::NetTlsConnect),
        NET_LISTEN_NAME => Some(ResolvedHostCommandOperation::NetListen),
        NET_ACCEPT_NAME => Some(ResolvedHostCommandOperation::NetAccept),
        NET_CLOSE_LISTENER_NAME => Some(ResolvedHostCommandOperation::NetCloseListener),
        NET_TLS_ACCEPT_NAME => Some(ResolvedHostCommandOperation::NetTlsAccept),
        HTTPS_GET_NAME => Some(ResolvedHostCommandOperation::HttpsGet),
        _ => None,
    }
}

pub(crate) fn by_id(id: &str) -> Option<ResolvedHostCommandOperation> {
    match id {
        NET_CONNECT_ID => Some(ResolvedHostCommandOperation::NetConnect),
        NET_SEND_ID => Some(ResolvedHostCommandOperation::NetSend),
        NET_RECV_ID => Some(ResolvedHostCommandOperation::NetRecv),
        NET_STREAM_STDOUT_ID => Some(ResolvedHostCommandOperation::NetStreamStdout),
        NET_WAIT_ID => Some(ResolvedHostCommandOperation::NetWait),
        NET_CLOSE_ID => Some(ResolvedHostCommandOperation::NetClose),
        NET_TLS_CONNECT_ID => Some(ResolvedHostCommandOperation::NetTlsConnect),
        NET_LISTEN_ID => Some(ResolvedHostCommandOperation::NetListen),
        NET_ACCEPT_ID => Some(ResolvedHostCommandOperation::NetAccept),
        NET_CLOSE_LISTENER_ID => Some(ResolvedHostCommandOperation::NetCloseListener),
        NET_TLS_ACCEPT_ID => Some(ResolvedHostCommandOperation::NetTlsAccept),
        HTTPS_GET_ID => Some(ResolvedHostCommandOperation::HttpsGet),
        _ => None,
    }
}

/// Source name; panics only for a non-network operation, which callers
/// exclude through `is_network`.
pub(crate) const fn name(op: ResolvedHostCommandOperation) -> &'static str {
    match op {
        ResolvedHostCommandOperation::NetConnect => NET_CONNECT_NAME,
        ResolvedHostCommandOperation::NetSend => NET_SEND_NAME,
        ResolvedHostCommandOperation::NetRecv => NET_RECV_NAME,
        ResolvedHostCommandOperation::NetStreamStdout => NET_STREAM_STDOUT_NAME,
        ResolvedHostCommandOperation::NetWait => NET_WAIT_NAME,
        ResolvedHostCommandOperation::NetClose => NET_CLOSE_NAME,
        ResolvedHostCommandOperation::NetTlsConnect => NET_TLS_CONNECT_NAME,
        ResolvedHostCommandOperation::NetListen => NET_LISTEN_NAME,
        ResolvedHostCommandOperation::NetAccept => NET_ACCEPT_NAME,
        ResolvedHostCommandOperation::NetCloseListener => NET_CLOSE_LISTENER_NAME,
        ResolvedHostCommandOperation::NetTlsAccept => NET_TLS_ACCEPT_NAME,
        ResolvedHostCommandOperation::HttpsGet => HTTPS_GET_NAME,
        _ => unreachable!(),
    }
}

pub(crate) const fn id(op: ResolvedHostCommandOperation) -> &'static str {
    match op {
        ResolvedHostCommandOperation::NetConnect => NET_CONNECT_ID,
        ResolvedHostCommandOperation::NetSend => NET_SEND_ID,
        ResolvedHostCommandOperation::NetRecv => NET_RECV_ID,
        ResolvedHostCommandOperation::NetStreamStdout => NET_STREAM_STDOUT_ID,
        ResolvedHostCommandOperation::NetWait => NET_WAIT_ID,
        ResolvedHostCommandOperation::NetClose => NET_CLOSE_ID,
        ResolvedHostCommandOperation::NetTlsConnect => NET_TLS_CONNECT_ID,
        ResolvedHostCommandOperation::NetListen => NET_LISTEN_ID,
        ResolvedHostCommandOperation::NetAccept => NET_ACCEPT_ID,
        ResolvedHostCommandOperation::NetCloseListener => NET_CLOSE_LISTENER_ID,
        ResolvedHostCommandOperation::NetTlsAccept => NET_TLS_ACCEPT_ID,
        ResolvedHostCommandOperation::HttpsGet => HTTPS_GET_ID,
        _ => unreachable!(),
    }
}

pub(crate) const fn effect(op: ResolvedHostCommandOperation) -> &'static str {
    match op {
        ResolvedHostCommandOperation::NetConnect | ResolvedHostCommandOperation::NetClose => {
            NETWORK_CONNECT_EFFECT
        }
        ResolvedHostCommandOperation::NetTlsConnect => NETWORK_TLS_EFFECT,
        ResolvedHostCommandOperation::NetListen
        | ResolvedHostCommandOperation::NetCloseListener => NETWORK_LISTEN_EFFECT,
        ResolvedHostCommandOperation::NetAccept | ResolvedHostCommandOperation::NetTlsAccept => {
            NETWORK_ACCEPT_EFFECT
        }
        ResolvedHostCommandOperation::NetSend => NETWORK_WRITE_EFFECT,
        ResolvedHostCommandOperation::NetRecv
        | ResolvedHostCommandOperation::NetStreamStdout
        | ResolvedHostCommandOperation::NetWait => NETWORK_READ_EFFECT,
        ResolvedHostCommandOperation::HttpsGet => NETWORK_HTTP_EFFECT,
        _ => unreachable!(),
    }
}

/// `net_stream_stdout` also appends to the stdout transcript, so the caller
/// must additionally declare `process.stdout.write`.
pub(crate) const fn secondary_effect(op: ResolvedHostCommandOperation) -> Option<&'static str> {
    match op {
        ResolvedHostCommandOperation::NetStreamStdout => {
            Some(crate::command_io_ops::STDOUT_WRITE_EFFECT)
        }
        ResolvedHostCommandOperation::NetTlsAccept => Some(NETWORK_TLS_EFFECT),
        _ => None,
    }
}

pub(crate) const fn arity(op: ResolvedHostCommandOperation) -> usize {
    match op {
        ResolvedHostCommandOperation::NetClose
        | ResolvedHostCommandOperation::NetAccept
        | ResolvedHostCommandOperation::NetTlsAccept
        | ResolvedHostCommandOperation::NetCloseListener => 1,
        ResolvedHostCommandOperation::NetConnect
        | ResolvedHostCommandOperation::NetTlsConnect
        | ResolvedHostCommandOperation::NetListen
        | ResolvedHostCommandOperation::NetSend
        | ResolvedHostCommandOperation::NetRecv
        | ResolvedHostCommandOperation::NetStreamStdout
        | ResolvedHostCommandOperation::NetWait => 2,
        ResolvedHostCommandOperation::HttpsGet => 2,
        _ => unreachable!(),
    }
}

pub(crate) const fn ast_return_type(op: ResolvedHostCommandOperation) -> Type {
    match op {
        ResolvedHostCommandOperation::NetRecv | ResolvedHostCommandOperation::HttpsGet => {
            Type::Bytes
        }
        _ => Type::Usize,
    }
}

pub(crate) const fn return_type(op: ResolvedHostCommandOperation) -> ResolvedType {
    match op {
        ResolvedHostCommandOperation::NetRecv | ResolvedHostCommandOperation::HttpsGet => {
            ResolvedType::Bytes
        }
        _ => ResolvedType::Usize,
    }
}

pub(crate) const fn result_ownership(op: ResolvedHostCommandOperation) -> OwnershipMode {
    match op {
        ResolvedHostCommandOperation::NetRecv | ResolvedHostCommandOperation::HttpsGet => {
            OwnershipMode::Own
        }
        _ => OwnershipMode::Value,
    }
}

/// Whether an operation keeps a `while` body cleanup-edge-free: every
/// network operation except the owned-result `net_recv` returns a Copy
/// scalar and borrows at most one caller-owned slice.
pub(crate) const fn admitted_in_while(op: ResolvedHostCommandOperation) -> bool {
    !matches!(
        op,
        ResolvedHostCommandOperation::NetRecv | ResolvedHostCommandOperation::HttpsGet
    )
}

/// Whether a network operation is fallible. Every network operation carries
/// the closed `semaprax.network.v1` status domain.
pub(crate) const fn is_fallible(_op: ResolvedHostCommandOperation) -> bool {
    true
}

const fn param_types(
    op: ResolvedHostCommandOperation,
) -> &'static [(&'static str, ParamMode, Type)] {
    match op {
        ResolvedHostCommandOperation::NetConnect
        | ResolvedHostCommandOperation::NetTlsConnect
        | ResolvedHostCommandOperation::NetListen => &[
            ("host", ParamMode::Borrow, Type::SliceU8),
            ("port", ParamMode::Value, Type::Usize),
        ],
        ResolvedHostCommandOperation::NetSend => &[
            ("handle", ParamMode::Value, Type::Usize),
            ("value", ParamMode::Borrow, Type::SliceU8),
        ],
        ResolvedHostCommandOperation::NetRecv | ResolvedHostCommandOperation::NetStreamStdout => &[
            ("handle", ParamMode::Value, Type::Usize),
            ("max", ParamMode::Value, Type::Usize),
        ],
        ResolvedHostCommandOperation::NetWait => &[
            ("handle", ParamMode::Value, Type::Usize),
            ("timeout_ms", ParamMode::Value, Type::Usize),
        ],
        ResolvedHostCommandOperation::HttpsGet => &[
            ("url", ParamMode::Borrow, Type::SliceU8),
            ("max", ParamMode::Value, Type::Usize),
        ],
        ResolvedHostCommandOperation::NetClose
        | ResolvedHostCommandOperation::NetAccept
        | ResolvedHostCommandOperation::NetTlsAccept
        | ResolvedHostCommandOperation::NetCloseListener => {
            &[("handle", ParamMode::Value, Type::Usize)]
        }
        _ => unreachable!(),
    }
}

pub(crate) fn accepts_ast(op: ResolvedHostCommandOperation, index: usize, ty: &Type) -> bool {
    param_types(op)
        .get(index)
        .is_some_and(|(_, _, expected)| expected == ty)
}

pub(crate) fn accepts_resolved(
    op: ResolvedHostCommandOperation,
    index: usize,
    ty: &ResolvedType,
) -> bool {
    param_types(op)
        .get(index)
        .is_some_and(|(_, _, expected)| match expected {
            Type::Usize => *ty == ResolvedType::Usize,
            Type::SliceU8 => *ty == ResolvedType::SliceU8,
            _ => false,
        })
}

pub(crate) fn ast_params(op: ResolvedHostCommandOperation) -> Vec<Param> {
    param_types(op)
        .iter()
        .map(|(name, mode, ty)| Param {
            name: (*name).to_owned(),
            mode: *mode,
            ty: ty.clone(),
            span: Span::default(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_network_operation_has_a_closed_exact_table() {
        for op in OPERATIONS {
            assert!(is_network(op));
            assert_eq!(by_name(name(op)), Some(op));
            assert_eq!(by_id(id(op)), Some(op));
            assert!(
                id(op).starts_with("core.host.net-") || id(op) == HTTPS_GET_ID,
                "unexpected network operation id: {}",
                id(op)
            );
            assert!(NETWORK_EFFECTS.contains(&effect(op)));
            assert_eq!(ast_params(op).len(), arity(op));
            for (index, param) in ast_params(op).iter().enumerate() {
                assert!(accepts_ast(op, index, &param.ty));
                assert!(!accepts_ast(op, arity(op), &param.ty));
            }
            assert!(is_fallible(op));
        }
        assert_eq!(
            result_ownership(ResolvedHostCommandOperation::NetRecv),
            OwnershipMode::Own
        );
        assert_eq!(
            return_type(ResolvedHostCommandOperation::NetRecv),
            ResolvedType::Bytes
        );
        assert!(!admitted_in_while(ResolvedHostCommandOperation::NetRecv));
        assert!(admitted_in_while(
            ResolvedHostCommandOperation::NetStreamStdout
        ));
        assert_eq!(
            secondary_effect(ResolvedHostCommandOperation::NetStreamStdout),
            Some("process.stdout.write")
        );
        assert_eq!(STATUS_CODES, [1, 2, 3, 4, 5, 6]);
        assert_eq!(SERVICE_STATUS_CODES, [1, 2, 3, 4, 5, 6, 7, 8, 9]);
        assert_eq!(HTTP_STATUS_CODES, [1, 2, 3, 4, 5, 6]);
        assert_eq!(MAX_CHUNK_BYTES * 16, MAX_TOTAL_BYTES);
    }
}
