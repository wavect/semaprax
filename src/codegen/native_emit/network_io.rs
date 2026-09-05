//! Bounded Language Network I/O v1 for the native C11 backend.
//!
//! The profile is Language Command I/O v1 (argument, stdin, and two-channel
//! append staging) plus the closed TCP operation family from
//! `network_io_ops`. Semantic functions still see only the injected
//! `spx_context`; the generated runner is the sole owner of every socket, and
//! it settles (closes) all of them on every path before the process adapter
//! publishes a transcript. A translation unit emitted for any other profile
//! contains none of this text, so a program without network permits carries
//! no socket code or includes.

use std::collections::HashMap;

use crate::ast::Program;
use crate::diagnostic::Diagnostic;
use crate::hir::{self, ResolvedProgram};
use crate::network_io_ops as ops;

use super::super::{
    backend_error, first_backend_diagnostic, native_byte_data, native_command_io,
    native_host_output, reject_native_rust_for_native, COutput,
};
use super::{emit_hir_c_with_labels, NativeOutputProfile};

/// Every permit a network-capable command module may declare.
const ADMITTED_PERMITS: [&str; 7] = [
    crate::command_io_ops::ARGS_READ_EFFECT,
    crate::command_io_ops::STDERR_WRITE_EFFECT,
    crate::command_io_ops::STDIN_READ_EFFECT,
    crate::host_io_ops::STDOUT_WRITE_EFFECT,
    ops::NETWORK_CONNECT_EFFECT,
    ops::NETWORK_READ_EFFECT,
    ops::NETWORK_WRITE_EFFECT,
];

/// Resolve source and emit Bounded Language Network I/O v1 for one selected
/// zero-argument boolean command.
pub fn emit_c_with_network_io(program: &Program, command_id: &str) -> Result<String, Diagnostic> {
    let resolved = hir::resolve(program).map_err(first_backend_diagnostic)?;
    emit_hir_c_with_network_io(&resolved, command_id)
}

/// Emit Bounded Language Network I/O v1 from validated HIR.
///
/// The module may permit only the Language Command I/O v1 process effects and
/// the three network effects, and must permit at least one network effect.
/// The selected command is an explicit stable-ID `fn () -> bool` whose
/// reachable operations satisfy the target-neutral `NetworkV1` profile.
pub fn emit_hir_c_with_network_io(
    program: &ResolvedProgram,
    command_id: &str,
) -> Result<String, Diagnostic> {
    hir::validate(program)?;
    reject_native_rust_for_native(program)?;
    if program
        .permits
        .iter()
        .any(|permit| !ADMITTED_PERMITS.contains(&permit.as_str()))
    {
        return Err(backend_error(
            "network command permits must stay within the command-I/O and network inventory",
        ));
    }
    if !program
        .permits
        .iter()
        .any(|permit| ops::NETWORK_EFFECTS.contains(&permit.as_str()))
    {
        return Err(backend_error(
            "network command requires at least one network permit",
        ));
    }
    let command = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == command_id)
        .ok_or_else(|| {
            backend_error(format!("selected network command `{command_id}` is absent"))
        })?;
    if program
        .declarations
        .declaration(&command.id)
        .is_none_or(|declaration| {
            declaration.identity_origin != crate::hir::IdentityOrigin::Explicit
        })
        || !command.params.is_empty()
        || command.return_type != crate::hir::ResolvedType::Bool
    {
        return Err(backend_error(
            "selected network command must be an explicit stable-ID `fn () -> bool`",
        ));
    }
    crate::command_io_ops::validate_operation_profile(
        program,
        &command.id,
        crate::command_io_ops::CommandOperationProfile::NetworkV1,
    )?;
    emit_hir_c_with_labels(
        program,
        &HashMap::new(),
        NativeOutputProfile::NetworkCommandIo,
        Some(&command.id),
    )
}

/// Feature-test macros the socket headers need on glibc-style hosts when the
/// translation unit is compiled as strict C11. They must precede the first
/// system include, so the profile emits them ahead of the shared prelude.
pub(super) fn emit_feature_macros(output: &mut impl COutput) {
    output.push_str(
        "#if defined(__linux__) && !defined(_DEFAULT_SOURCE)\n\
         #define _DEFAULT_SOURCE 1\n\
         #endif\n",
    );
}

/// Emit the complete network profile runtime: the line-command output and
/// input runtimes, then the network table and its six host helpers.
///
/// The command input carriers are byte slices, so a program that reaches only
/// handle-shaped operations (`net_wait`, `net_close`) still needs the byte
/// data runtime the prelude would otherwise omit as unreachable.
pub(super) fn emit_runtime(output: &mut impl COutput, program: &ResolvedProgram) {
    if !super::program_uses_byte_data(program) {
        native_byte_data::emit_runtime(output);
    }
    native_host_output::emit_line_command_runtime(output);
    native_command_io::emit_line_runtime(output);
    emit_constants(output);
    output.push_str(NETWORK_RUNTIME_C);
}

/// The closed constants, taken from the shared operation table so every
/// backend agrees on the same bounds and status codes.
fn emit_constants(output: &mut impl COutput) {
    writeln!(
        output,
        "#define SPX_NETWORK_STATUS_DOMAIN_V1 \"{domain}\"\n\
         #define SPX_NETWORK_CONNECT_FAILED_V1 UINT32_C({connect_failed})\n\
         #define SPX_NETWORK_INVALID_ENDPOINT_V1 UINT32_C({invalid_endpoint})\n\
         #define SPX_NETWORK_UNKNOWN_HANDLE_V1 UINT32_C({unknown_handle})\n\
         #define SPX_NETWORK_CAPACITY_EXCEEDED_V1 UINT32_C({capacity_exceeded})\n\
         #define SPX_NETWORK_TRANSFER_FAILED_V1 UINT32_C({transfer_failed})\n\
         #define SPX_NETWORK_AUTHORITY_DENIED_V1 UINT32_C({authority_denied})\n\
         #define SPX_NETWORK_MAX_HANDLES_V1 UINT64_C({max_handles})\n\
         #define SPX_NETWORK_MAX_HOST_BYTES_V1 UINT64_C({max_host_bytes})\n\
         #define SPX_NETWORK_MAX_PORT_V1 UINT64_C({max_port})\n\
         #define SPX_NETWORK_MAX_CHUNK_BYTES_V1 UINT64_C({max_chunk_bytes})\n\
         #define SPX_NETWORK_MAX_TOTAL_BYTES_V1 UINT64_C({max_total_bytes})\n\
         #define SPX_NETWORK_MAX_WAIT_MILLIS_V1 UINT64_C({max_wait_millis})\n\
         #define SPX_NETWORK_WAIT_TIMEOUT_V1 UINT64_C({wait_timeout})\n\
         #define SPX_NETWORK_WAIT_READABLE_V1 UINT64_C({wait_readable})\n\
         #define SPX_NETWORK_WAIT_CLOSED_V1 UINT64_C({wait_closed})\n\
         #define SPX_NETWORK_IO_TIMEOUT_MILLIS_V1 {io_timeout_millis}\n\
         #define SPX_NETWORK_OPERATION_DEADLINE_MILLIS_V1 UINT64_C({operation_deadline_millis})",
        domain = ops::STATUS_DOMAIN,
        connect_failed = ops::CONNECT_FAILED,
        invalid_endpoint = ops::INVALID_ENDPOINT,
        unknown_handle = ops::UNKNOWN_HANDLE,
        capacity_exceeded = ops::CAPACITY_EXCEEDED,
        transfer_failed = ops::TRANSFER_FAILED,
        authority_denied = ops::AUTHORITY_DENIED,
        max_handles = ops::MAX_HANDLES,
        max_host_bytes = ops::MAX_HOST_BYTES,
        max_port = ops::MAX_PORT,
        max_chunk_bytes = ops::MAX_CHUNK_BYTES,
        max_total_bytes = ops::MAX_TOTAL_BYTES,
        max_wait_millis = ops::MAX_WAIT_MILLIS,
        wait_timeout = ops::WAIT_TIMEOUT,
        wait_readable = ops::WAIT_READABLE,
        wait_closed = ops::WAIT_CLOSED,
        io_timeout_millis = ops::MAX_WAIT_MILLIS,
        operation_deadline_millis = ops::MAX_WAIT_MILLIS,
    )
    .expect("writing native network constants cannot fail");
}

/// The socket table, its status normalization, and the six host helpers.
///
/// POSIX sockets are the primary target; the `_WIN32` branches map the same
/// shape onto Winsock2. The runner calls `spx_network_settle_v1` on every
/// path, so no socket outlives the invocation regardless of program outcome.
const NETWORK_RUNTIME_C: &str = r#"
#include <limits.h>
#include <time.h>
#if defined(_WIN32)
#include <winsock2.h>
#include <ws2tcpip.h>
#include <sysinfoapi.h>
typedef SOCKET spx_net_socket_v1;
typedef int spx_net_ssize_v1;
typedef int spx_net_len_v1;
#define SPX_NET_INVALID_SOCKET_V1 INVALID_SOCKET
#define SPX_NET_SEND_FLAGS_V1 0
#else
#include <errno.h>
#include <fcntl.h>
#include <netdb.h>
#include <netinet/in.h>
#include <poll.h>
#include <sys/socket.h>
#include <sys/time.h>
#include <sys/types.h>
#include <unistd.h>
typedef int spx_net_socket_v1;
typedef ssize_t spx_net_ssize_v1;
typedef size_t spx_net_len_v1;
#define SPX_NET_INVALID_SOCKET_V1 (-1)
#if defined(MSG_NOSIGNAL)
#define SPX_NET_SEND_FLAGS_V1 MSG_NOSIGNAL
#else
#define SPX_NET_SEND_FLAGS_V1 0
#endif
#endif

struct spx_network_state_v1 {
    /* False until the runner grants network authority for this invocation. */
    bool granted;
    /* Handles issued so far; handles are dense 1..=opened and never reused. */
    uint64_t opened;
    /* Cumulative sent plus received payload bytes. */
    uint64_t total_bytes;
    bool open[SPX_NETWORK_MAX_HANDLES_V1];
    spx_net_socket_v1 sockets[SPX_NETWORK_MAX_HANDLES_V1];
    uint8_t scratch[SPX_NETWORK_MAX_CHUNK_BYTES_V1];
};

struct spx_network_command_state_v1 {
    /* First-member layout is intentional: the command input and two-channel
       output helpers authenticate target_state through this exact prefix. */
    struct spx_language_command_state_v1 command;
    struct spx_network_state_v1 network;
};

_Static_assert(
    offsetof(struct spx_network_command_state_v1, command) == 0,
    "language command state must be the network target-state prefix"
);

static __attribute__((unused)) struct spx_network_command_state_v1 *spx_network_command_state_v1(
    struct spx_context *spx_ctx
) {
    if (spx_ctx == NULL || spx_ctx->target_state == NULL) {
        spx_runtime_invariant_failure("network state is unavailable");
    }
    return (struct spx_network_command_state_v1 *)spx_ctx->target_state;
}

static __attribute__((unused)) spx_status_token spx_network_status_v1(
    struct spx_context *spx_ctx,
    uint32_t code
) {
    if (code < SPX_NETWORK_CONNECT_FAILED_V1 || code > SPX_NETWORK_AUTHORITY_DENIED_V1) {
        spx_runtime_invariant_failure("network status is outside the closed table");
    }
    spx_status_token token = SPX_STATUS_SUCCESS;
    if (!spx_status_record_adapter(
        spx_ctx,
        SPX_NETWORK_STATUS_DOMAIN_V1,
        code,
        SPX_STATUS_CLASS_ADAPTER,
        SPX_RETRYABILITY_FALSE,
        &token
    )) {
        spx_runtime_invariant_failure("network status could not be recorded");
    }
    return token;
}

static __attribute__((unused)) void spx_network_close_socket_v1(spx_net_socket_v1 socket_handle) {
#if defined(_WIN32)
    (void)closesocket(socket_handle);
#else
    (void)close(socket_handle);
#endif
}

static __attribute__((unused)) bool spx_network_set_blocking_v1(spx_net_socket_v1 socket_handle, bool blocking) {
#if defined(_WIN32)
    u_long mode = blocking ? 0u : 1u;
    return ioctlsocket(socket_handle, FIONBIO, &mode) == 0;
#else
    int flags = fcntl(socket_handle, F_GETFL, 0);
    if (flags < 0) return false;
    flags = blocking ? (flags & ~O_NONBLOCK) : (flags | O_NONBLOCK);
    return fcntl(socket_handle, F_SETFL, flags) == 0;
#endif
}

static __attribute__((unused)) int spx_network_poll_v1(spx_net_socket_v1 socket_handle, short events, int timeout_ms) {
#if defined(_WIN32)
    WSAPOLLFD descriptor;
    memset(&descriptor, 0, sizeof(descriptor));
    descriptor.fd = socket_handle;
    descriptor.events = events;
    return WSAPoll(&descriptor, 1u, timeout_ms);
#else
    struct pollfd descriptor;
    memset(&descriptor, 0, sizeof(descriptor));
    descriptor.fd = socket_handle;
    descriptor.events = events;
    int ready;
    do {
        ready = poll(&descriptor, 1u, timeout_ms);
    } while (ready < 0 && errno == EINTR);
    return ready;
#endif
}

/* One aggregate operation deadline, on a monotonic clock. Name resolution,
   every candidate address, every partial write, every retried read, and every
   readiness wait draw down this single budget; nothing restarts it. This is
   distinct from the per-syscall timeout the socket options below enforce and
   from the evaluator's byte/handle invocation budget. */
struct spx_network_deadline_v1 {
    uint64_t expires_at_millis;
};

static __attribute__((unused)) uint64_t spx_network_monotonic_millis_v1(void) {
#if defined(_WIN32)
    return (uint64_t)GetTickCount64();
#else
    struct timespec now;
#if defined(CLOCK_MONOTONIC)
    if (clock_gettime(CLOCK_MONOTONIC, &now) != 0) return UINT64_C(0);
#else
    if (clock_gettime(CLOCK_REALTIME, &now) != 0) return UINT64_C(0);
#endif
    return (uint64_t)now.tv_sec * UINT64_C(1000) + (uint64_t)(now.tv_nsec / 1000000L);
#endif
}

static __attribute__((unused)) struct spx_network_deadline_v1 spx_network_deadline_start_v1(void) {
    struct spx_network_deadline_v1 deadline;
    deadline.expires_at_millis =
        spx_network_monotonic_millis_v1() + SPX_NETWORK_OPERATION_DEADLINE_MILLIS_V1;
    return deadline;
}

/* Remaining budget, never negative. Zero means the aggregate deadline is spent
   and the caller must fail rather than start another blocking call. */
static __attribute__((unused)) uint64_t spx_network_deadline_remaining_v1(
    struct spx_network_deadline_v1 deadline
) {
    uint64_t now = spx_network_monotonic_millis_v1();
    if (now >= deadline.expires_at_millis) return UINT64_C(0);
    return deadline.expires_at_millis - now;
}

/* The remaining budget as a per-syscall timeout, never zero so a socket option
   is never read as "block forever". Returns false once the budget is spent. */
static __attribute__((unused)) bool spx_network_deadline_slice_v1(
    struct spx_network_deadline_v1 deadline,
    uint64_t *slice_out
) {
    uint64_t remaining = spx_network_deadline_remaining_v1(deadline);
    if (remaining == UINT64_C(0)) return false;
    *slice_out = remaining;
    return true;
}

static __attribute__((unused)) bool spx_network_set_timeouts_v1(
    spx_net_socket_v1 socket_handle,
    uint64_t timeout_millis
) {
    if (timeout_millis == UINT64_C(0)) return false;
#if defined(_WIN32)
    DWORD timeout = (DWORD)timeout_millis;
    return setsockopt(
            socket_handle, SOL_SOCKET, SO_RCVTIMEO,
            (const char *)&timeout, (int)sizeof(timeout)
        ) == 0 &&
        setsockopt(
            socket_handle, SOL_SOCKET, SO_SNDTIMEO,
            (const char *)&timeout, (int)sizeof(timeout)
        ) == 0;
#else
    struct timeval timeout;
    memset(&timeout, 0, sizeof(timeout));
    timeout.tv_sec = (time_t)(timeout_millis / 1000);
    timeout.tv_usec = (suseconds_t)((timeout_millis % 1000) * 1000);
#if defined(SO_NOSIGPIPE)
    int no_sigpipe = 1;
    if (setsockopt(
        socket_handle, SOL_SOCKET, SO_NOSIGPIPE,
        &no_sigpipe, (socklen_t)sizeof(no_sigpipe)
    ) != 0) return false;
#endif
    return setsockopt(
            socket_handle, SOL_SOCKET, SO_RCVTIMEO,
            &timeout, (socklen_t)sizeof(timeout)
        ) == 0 &&
        setsockopt(
            socket_handle, SOL_SOCKET, SO_SNDTIMEO,
            &timeout, (socklen_t)sizeof(timeout)
        ) == 0;
#endif
}

static __attribute__((unused)) bool spx_network_connect_with_timeout_v1(
    spx_net_socket_v1 socket_handle,
    const struct sockaddr *address,
    socklen_t address_length,
    struct spx_network_deadline_v1 deadline
) {
    uint64_t remaining = UINT64_C(0);
    if (!spx_network_deadline_slice_v1(deadline, &remaining)) return false;
    if (remaining > (uint64_t)INT_MAX) remaining = (uint64_t)INT_MAX;
    if (!spx_network_set_blocking_v1(socket_handle, false)) return false;
    if (connect(socket_handle, address, address_length) != 0) {
#if defined(_WIN32)
        if (WSAGetLastError() != WSAEWOULDBLOCK) return false;
#else
        if (errno != EINPROGRESS) return false;
#endif
        /* Each candidate address gets only what is left of the aggregate
           budget, so several failing addresses cannot multiply it. */
        if (spx_network_poll_v1(socket_handle, POLLOUT, (int)remaining) <= 0) {
            return false;
        }
        int error = 0;
        socklen_t error_length = (socklen_t)sizeof(error);
        if (getsockopt(
                socket_handle, SOL_SOCKET, SO_ERROR,
                (char *)&error, &error_length
            ) != 0 || error != 0) {
            return false;
        }
    }
    return spx_network_set_blocking_v1(socket_handle, true);
}

static __attribute__((unused)) bool spx_network_lookup_v1(
    const struct spx_network_state_v1 *state,
    uint64_t handle,
    spx_net_socket_v1 *socket_out
) {
    if (handle == UINT64_C(0) || handle > state->opened ||
        handle > SPX_NETWORK_MAX_HANDLES_V1 || !state->open[handle - UINT64_C(1)]) {
        return false;
    }
    *socket_out = state->sockets[handle - UINT64_C(1)];
    return true;
}

/* One read, bounded by what is left of the aggregate deadline. An interrupted
   call resumes against the same deadline instead of paying a fresh timeout. */
static __attribute__((unused)) spx_net_ssize_v1 spx_network_recv_once_v1(
    spx_net_socket_v1 socket_handle,
    uint8_t *buffer,
    uint64_t max,
    int flags,
    struct spx_network_deadline_v1 deadline
) {
    spx_net_ssize_v1 received;
    uint64_t remaining = UINT64_C(0);
    if (!spx_network_deadline_slice_v1(deadline, &remaining) ||
        !spx_network_set_timeouts_v1(socket_handle, remaining)) {
        return (spx_net_ssize_v1)-1;
    }
#if defined(_WIN32)
    received = recv(socket_handle, (char *)buffer, (int)max, flags);
#else
    for (;;) {
        received = recv(socket_handle, buffer, (spx_net_len_v1)max, flags);
        if (received >= 0 || errno != EINTR) break;
        if (!spx_network_deadline_slice_v1(deadline, &remaining) ||
            !spx_network_set_timeouts_v1(socket_handle, remaining)) {
            return (spx_net_ssize_v1)-1;
        }
    }
#endif
    return received;
}

static __attribute__((unused)) spx_status_token spx_host_net_connect_v1(
    struct spx_context *spx_ctx,
    spx_slice_u8_v1 host,
    uint64_t port,
    uint64_t *result_out
) {
    if (result_out == NULL) {
        spx_runtime_invariant_failure("net_connect result slot is unavailable");
    }
    *result_out = UINT64_C(0);
    struct spx_network_state_v1 *state = &spx_network_command_state_v1(spx_ctx)->network;
    spx_slice_u8_require_valid(host);
    if (!state->granted) {
        return spx_network_status_v1(spx_ctx, SPX_NETWORK_AUTHORITY_DENIED_V1);
    }
    if (host.len == UINT64_C(0) || host.len > SPX_NETWORK_MAX_HOST_BYTES_V1 ||
        port == UINT64_C(0) || port > SPX_NETWORK_MAX_PORT_V1 ||
        !spx_command_utf8_v1(host.ptr, host.len)) {
        return spx_network_status_v1(spx_ctx, SPX_NETWORK_INVALID_ENDPOINT_V1);
    }
    for (uint64_t index = UINT64_C(0); index < host.len; ++index) {
        if (host.ptr[index] == UINT8_C(0)) {
            return spx_network_status_v1(spx_ctx, SPX_NETWORK_INVALID_ENDPOINT_V1);
        }
    }
    if (state->opened >= SPX_NETWORK_MAX_HANDLES_V1) {
        return spx_network_status_v1(spx_ctx, SPX_NETWORK_CAPACITY_EXCEEDED_V1);
    }
    char host_name[SPX_NETWORK_MAX_HOST_BYTES_V1 + 1u];
    memcpy(host_name, host.ptr, (size_t)host.len);
    host_name[host.len] = '\0';
    char port_text[6];
    if (snprintf(port_text, sizeof(port_text), "%u", (unsigned)port) <= 0) {
        return spx_network_status_v1(spx_ctx, SPX_NETWORK_INVALID_ENDPOINT_V1);
    }
    struct addrinfo hints;
    memset(&hints, 0, sizeof(hints));
    hints.ai_family = AF_UNSPEC;
    hints.ai_socktype = SOCK_STREAM;
    hints.ai_protocol = IPPROTO_TCP;
    /* The aggregate deadline starts before resolution, so a slow name service
       consumes the same budget the connection attempts draw from. The C11
       adapter cannot interrupt getaddrinfo itself: this bounds what the
       adapter does with the answer, and the specification states that
       non-claim rather than promising cancellation the host cannot enforce. */
    struct spx_network_deadline_v1 deadline = spx_network_deadline_start_v1();
    struct addrinfo *candidates = NULL;
    if (getaddrinfo(host_name, port_text, &hints, &candidates) != 0 || candidates == NULL) {
        return spx_network_status_v1(spx_ctx, SPX_NETWORK_CONNECT_FAILED_V1);
    }
    spx_net_socket_v1 connected = SPX_NET_INVALID_SOCKET_V1;
    for (const struct addrinfo *candidate = candidates;
         candidate != NULL && connected == SPX_NET_INVALID_SOCKET_V1;
         candidate = candidate->ai_next) {
        uint64_t remaining = UINT64_C(0);
        if (!spx_network_deadline_slice_v1(deadline, &remaining)) break;
        spx_net_socket_v1 attempt = socket(
            candidate->ai_family, candidate->ai_socktype, candidate->ai_protocol
        );
        if (attempt == SPX_NET_INVALID_SOCKET_V1) continue;
        if (spx_network_connect_with_timeout_v1(
                attempt, candidate->ai_addr, (socklen_t)candidate->ai_addrlen, deadline
            ) && spx_network_deadline_slice_v1(deadline, &remaining)
            && spx_network_set_timeouts_v1(attempt, remaining)) {
            connected = attempt;
        } else {
            spx_network_close_socket_v1(attempt);
        }
    }
    freeaddrinfo(candidates);
    if (connected == SPX_NET_INVALID_SOCKET_V1) {
        return spx_network_status_v1(spx_ctx, SPX_NETWORK_CONNECT_FAILED_V1);
    }
    state->sockets[state->opened] = connected;
    state->open[state->opened] = true;
    state->opened += UINT64_C(1);
    *result_out = state->opened;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_host_net_send_v1(
    struct spx_context *spx_ctx,
    uint64_t handle,
    spx_slice_u8_v1 value,
    uint64_t *result_out
) {
    if (result_out == NULL) {
        spx_runtime_invariant_failure("net_send result slot is unavailable");
    }
    *result_out = UINT64_C(0);
    struct spx_network_state_v1 *state = &spx_network_command_state_v1(spx_ctx)->network;
    spx_slice_u8_require_valid(value);
    if (!state->granted) {
        return spx_network_status_v1(spx_ctx, SPX_NETWORK_AUTHORITY_DENIED_V1);
    }
    spx_net_socket_v1 socket_handle = SPX_NET_INVALID_SOCKET_V1;
    if (!spx_network_lookup_v1(state, handle, &socket_handle)) {
        return spx_network_status_v1(spx_ctx, SPX_NETWORK_UNKNOWN_HANDLE_V1);
    }
    if (value.len > SPX_NETWORK_MAX_TOTAL_BYTES_V1 - state->total_bytes) {
        return spx_network_status_v1(spx_ctx, SPX_NETWORK_CAPACITY_EXCEEDED_V1);
    }
    /* One deadline covers every partial write, so a peer that accepts one byte
       at a time cannot extend the operation past its budget. */
    struct spx_network_deadline_v1 deadline = spx_network_deadline_start_v1();
    uint64_t sent = UINT64_C(0);
    while (sent < value.len) {
        uint64_t remaining = UINT64_C(0);
        if (!spx_network_deadline_slice_v1(deadline, &remaining) ||
            !spx_network_set_timeouts_v1(socket_handle, remaining)) {
            return spx_network_status_v1(spx_ctx, SPX_NETWORK_TRANSFER_FAILED_V1);
        }
        spx_net_ssize_v1 written = send(
            socket_handle,
            (const char *)value.ptr + (size_t)sent,
            (spx_net_len_v1)(value.len - sent),
            SPX_NET_SEND_FLAGS_V1
        );
#if !defined(_WIN32)
        /* An interrupted call resumes against the same deadline. */
        if (written < 0 && errno == EINTR) continue;
#endif
        if (written <= 0) {
            return spx_network_status_v1(spx_ctx, SPX_NETWORK_TRANSFER_FAILED_V1);
        }
        sent += (uint64_t)written;
    }
    state->total_bytes += value.len;
    *result_out = value.len;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_host_net_recv_v1(
    struct spx_context *spx_ctx,
    uint64_t handle,
    uint64_t max,
    spx_bytes_v1 *result_out
) {
    if (result_out == NULL) {
        spx_runtime_invariant_failure("net_recv result slot is unavailable");
    }
    *result_out = (spx_bytes_v1){ .ptr = NULL, .len = UINT64_C(0) };
    struct spx_network_state_v1 *state = &spx_network_command_state_v1(spx_ctx)->network;
    if (!state->granted) {
        return spx_network_status_v1(spx_ctx, SPX_NETWORK_AUTHORITY_DENIED_V1);
    }
    if (max > SPX_NETWORK_MAX_CHUNK_BYTES_V1) {
        return spx_network_status_v1(spx_ctx, SPX_NETWORK_CAPACITY_EXCEEDED_V1);
    }
    spx_net_socket_v1 socket_handle = SPX_NET_INVALID_SOCKET_V1;
    if (!spx_network_lookup_v1(state, handle, &socket_handle)) {
        return spx_network_status_v1(spx_ctx, SPX_NETWORK_UNKNOWN_HANDLE_V1);
    }
    if (max == UINT64_C(0)) return SPX_STATUS_SUCCESS;
    struct spx_network_deadline_v1 deadline = spx_network_deadline_start_v1();
    spx_net_ssize_v1 received =
        spx_network_recv_once_v1(socket_handle, state->scratch, max, 0, deadline);
    if (received < 0) {
        return spx_network_status_v1(spx_ctx, SPX_NETWORK_TRANSFER_FAILED_V1);
    }
    if (received == 0) return SPX_STATUS_SUCCESS;
    if ((uint64_t)received > SPX_NETWORK_MAX_TOTAL_BYTES_V1 - state->total_bytes) {
        return spx_network_status_v1(spx_ctx, SPX_NETWORK_CAPACITY_EXCEEDED_V1);
    }
    uint8_t *copy = (uint8_t *)malloc((size_t)received);
    if (copy == NULL) {
        return spx_network_status_v1(spx_ctx, SPX_NETWORK_TRANSFER_FAILED_V1);
    }
    memcpy(copy, state->scratch, (size_t)received);
    state->total_bytes += (uint64_t)received;
    result_out->ptr = copy;
    result_out->len = (uint64_t)received;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_host_net_stream_stdout_v1(
    struct spx_context *spx_ctx,
    uint64_t handle,
    uint64_t max,
    uint64_t *result_out
) {
    if (result_out == NULL) {
        spx_runtime_invariant_failure("net_stream_stdout result slot is unavailable");
    }
    *result_out = UINT64_C(0);
    struct spx_network_command_state_v1 *command_state = spx_network_command_state_v1(spx_ctx);
    struct spx_network_state_v1 *state = &command_state->network;
    if (!state->granted) {
        return spx_network_status_v1(spx_ctx, SPX_NETWORK_AUTHORITY_DENIED_V1);
    }
    if (max > SPX_NETWORK_MAX_CHUNK_BYTES_V1) {
        return spx_network_status_v1(spx_ctx, SPX_NETWORK_CAPACITY_EXCEEDED_V1);
    }
    spx_net_socket_v1 socket_handle = SPX_NET_INVALID_SOCKET_V1;
    if (!spx_network_lookup_v1(state, handle, &socket_handle)) {
        return spx_network_status_v1(spx_ctx, SPX_NETWORK_UNKNOWN_HANDLE_V1);
    }
    if (max == UINT64_C(0)) return SPX_STATUS_SUCCESS;
    struct spx_network_deadline_v1 deadline = spx_network_deadline_start_v1();
    spx_net_ssize_v1 received =
        spx_network_recv_once_v1(socket_handle, state->scratch, max, 0, deadline);
    if (received < 0) {
        return spx_network_status_v1(spx_ctx, SPX_NETWORK_TRANSFER_FAILED_V1);
    }
    if (received == 0) return SPX_STATUS_SUCCESS;
    if ((uint64_t)received > SPX_NETWORK_MAX_TOTAL_BYTES_V1 - state->total_bytes) {
        return spx_network_status_v1(spx_ctx, SPX_NETWORK_CAPACITY_EXCEEDED_V1);
    }
    const struct spx_command_output_staging_v1 *staging = &command_state->command.output;
    if (staging->stdout_length > SPX_COMMAND_OUTPUT_CAPACITY_V1 ||
        staging->stderr_length > SPX_COMMAND_OUTPUT_CAPACITY_V1 - staging->stdout_length ||
        (uint64_t)received > SPX_COMMAND_OUTPUT_CAPACITY_V1 -
            staging->stdout_length - staging->stderr_length) {
        return spx_network_status_v1(spx_ctx, SPX_NETWORK_CAPACITY_EXCEEDED_V1);
    }
    spx_slice_u8_v1 chunk = { .ptr = state->scratch, .len = (uint64_t)received };
    uint64_t appended = UINT64_C(0);
    spx_status_token token = spx_host_command_stdout_append_v1(spx_ctx, chunk, &appended);
    if (token != SPX_STATUS_SUCCESS) return token;
    if (appended != (uint64_t)received) {
        spx_runtime_invariant_failure("streamed stdout append disagrees with its chunk");
    }
    state->total_bytes += (uint64_t)received;
    *result_out = (uint64_t)received;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_host_net_wait_v1(
    struct spx_context *spx_ctx,
    uint64_t handle,
    uint64_t timeout_ms,
    uint64_t *result_out
) {
    if (result_out == NULL) {
        spx_runtime_invariant_failure("net_wait result slot is unavailable");
    }
    *result_out = SPX_NETWORK_WAIT_TIMEOUT_V1;
    struct spx_network_state_v1 *state = &spx_network_command_state_v1(spx_ctx)->network;
    if (!state->granted) {
        return spx_network_status_v1(spx_ctx, SPX_NETWORK_AUTHORITY_DENIED_V1);
    }
    if (timeout_ms > SPX_NETWORK_MAX_WAIT_MILLIS_V1) {
        return spx_network_status_v1(spx_ctx, SPX_NETWORK_CAPACITY_EXCEEDED_V1);
    }
    spx_net_socket_v1 socket_handle = SPX_NET_INVALID_SOCKET_V1;
    if (!spx_network_lookup_v1(state, handle, &socket_handle)) {
        return spx_network_status_v1(spx_ctx, SPX_NETWORK_UNKNOWN_HANDLE_V1);
    }
    /* The program's own timeout caps the wait; the operation deadline caps it
       again, so a readiness wait can never outlast the aggregate budget. */
    struct spx_network_deadline_v1 deadline = spx_network_deadline_start_v1();
    uint64_t remaining = UINT64_C(0);
    if (!spx_network_deadline_slice_v1(deadline, &remaining)) {
        *result_out = SPX_NETWORK_WAIT_TIMEOUT_V1;
        return SPX_STATUS_SUCCESS;
    }
    if (timeout_ms < remaining) remaining = timeout_ms;
    if (remaining > (uint64_t)INT_MAX) remaining = (uint64_t)INT_MAX;
    int ready = spx_network_poll_v1(socket_handle, POLLIN, (int)remaining);
    if (ready < 0) {
        return spx_network_status_v1(spx_ctx, SPX_NETWORK_TRANSFER_FAILED_V1);
    }
    if (ready == 0) {
        *result_out = SPX_NETWORK_WAIT_TIMEOUT_V1;
        return SPX_STATUS_SUCCESS;
    }
    uint8_t probe = UINT8_C(0);
    spx_net_ssize_v1 peeked =
        spx_network_recv_once_v1(socket_handle, &probe, UINT64_C(1), MSG_PEEK, deadline);
    if (peeked < 0) {
        return spx_network_status_v1(spx_ctx, SPX_NETWORK_TRANSFER_FAILED_V1);
    }
    *result_out = peeked == 0 ? SPX_NETWORK_WAIT_CLOSED_V1 : SPX_NETWORK_WAIT_READABLE_V1;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_host_net_close_v1(
    struct spx_context *spx_ctx,
    uint64_t handle,
    uint64_t *result_out
) {
    if (result_out == NULL) {
        spx_runtime_invariant_failure("net_close result slot is unavailable");
    }
    *result_out = UINT64_C(0);
    struct spx_network_state_v1 *state = &spx_network_command_state_v1(spx_ctx)->network;
    if (!state->granted) {
        return spx_network_status_v1(spx_ctx, SPX_NETWORK_AUTHORITY_DENIED_V1);
    }
    spx_net_socket_v1 socket_handle = SPX_NET_INVALID_SOCKET_V1;
    if (!spx_network_lookup_v1(state, handle, &socket_handle)) {
        return spx_network_status_v1(spx_ctx, SPX_NETWORK_UNKNOWN_HANDLE_V1);
    }
    spx_network_close_socket_v1(socket_handle);
    state->open[handle - UINT64_C(1)] = false;
    state->sockets[handle - UINT64_C(1)] = SPX_NET_INVALID_SOCKET_V1;
    return SPX_STATUS_SUCCESS;
}

/* Settlement: every still-open socket closes regardless of the semantic
   outcome. The runner calls this on every path before publishing anything. */
static void spx_network_settle_v1(struct spx_network_state_v1 *state) {
    if (state == NULL) return;
    for (uint64_t index = UINT64_C(0); index < SPX_NETWORK_MAX_HANDLES_V1; ++index) {
        if (state->open[index]) {
            spx_network_close_socket_v1(state->sockets[index]);
        }
        state->open[index] = false;
        state->sockets[index] = SPX_NET_INVALID_SOCKET_V1;
    }
    state->granted = false;
}

"#;

/// Emit the runner under the Language Command I/O v1 symbol so the shared
/// process adapter drives it unchanged. Network authority is granted to the
/// state here and withdrawn by settlement on every path.
pub(super) fn emit_runner(output: &mut impl COutput, command_symbol: &str) {
    writeln!(
        output,
        r#"int {run_symbol}(
    const struct spx_language_command_input_v1 *input,
    struct spx_language_command_result_v1 *result_out
) {{
    if (result_out == NULL) return 0;
    memset(result_out, 0, sizeof(*result_out));
    if (!spx_language_command_input_is_valid_v1(input)) return 0;
#if defined(_WIN32)
    WSADATA winsock_data;
    if (WSAStartup(MAKEWORD(2, 2), &winsock_data) != 0) return 0;
#endif

    struct spx_status_entry spx_status_entries[UINT32_C(1)];
    struct spx_network_command_state_v1 state = {{0}};
    state.command.input = input;
    state.network.granted = true;
    struct spx_context spx_ctx = {{0}};
    if (!spx_context_init(
        &spx_ctx,
        UINT64_C(1),
        spx_status_entries,
        UINT32_C(1),
        NULL,
        NULL,
        &state
    )) {{
        spx_network_settle_v1(&state.network);
#if defined(_WIN32)
        (void)WSACleanup();
#endif
        return 0;
    }}

    bool matched = false;
    spx_status_token status = {command_symbol}(&spx_ctx, &matched);
    spx_network_settle_v1(&state.network);
#if defined(_WIN32)
    (void)WSACleanup();
#endif
    if (status != SPX_STATUS_SUCCESS) {{
        const struct spx_normalized_status *failure =
            spx_status_resolve(&spx_ctx, status);
        (void)spx_status_resolve_detail(&spx_ctx, status);
        if (failure == NULL || failure->domain_id == NULL) {{
            memset(&state, 0, sizeof(state));
            memset(result_out, 0, sizeof(*result_out));
            return 0;
        }}
        size_t domain_size = 0;
        if (!spx_status_domain_size(failure->domain_id, &domain_size) ||
            domain_size > sizeof(result_out->status_domain)) {{
            memset(&state, 0, sizeof(state));
            memset(result_out, 0, sizeof(*result_out));
            return 0;
        }}
        memcpy(result_out->status_domain, failure->domain_id, domain_size);
        result_out->status_code = failure->code;
        result_out->status_class = failure->status_class;
        result_out->status_retryability = failure->retryability;
        memset(&state, 0, sizeof(state));
        return 1;
    }}

    if (state.command.output.stdout_length > SPX_COMMAND_OUTPUT_CAPACITY_V1 ||
        state.command.output.stderr_length >
            SPX_COMMAND_OUTPUT_CAPACITY_V1 - state.command.output.stdout_length) {{
        memset(&state, 0, sizeof(state));
        memset(result_out, 0, sizeof(*result_out));
        return 0;
    }}
    result_out->semantic_success = true;
    result_out->matched = matched;
    if (state.command.output.stdout_length != UINT64_C(0)) {{
        memcpy(
            result_out->stdout_bytes,
            state.command.output.stdout_bytes,
            (size_t)state.command.output.stdout_length
        );
    }}
    if (state.command.output.stderr_length != UINT64_C(0)) {{
        memcpy(
            result_out->stderr_bytes,
            state.command.output.stderr_bytes,
            (size_t)state.command.output.stderr_length
        );
    }}
    result_out->stdout_length = state.command.output.stdout_length;
    result_out->stderr_length = state.command.output.stderr_length;
    memset(&state, 0, sizeof(state));
    return 1;
}}
"#,
        run_symbol = native_command_io::RUN_SYMBOL,
    )
    .expect("writing native network command runner cannot fail");
}
