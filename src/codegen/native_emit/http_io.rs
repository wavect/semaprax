//! HTTPS Client I/O v1 for the native C11 backend.
//!
//! This profile is emitted only for an authenticated Project-v13 command. A
//! single invocation-owned libcurl easy handle retains its connection cache;
//! the generated runner grants authority, executes the command, settles the
//! handle on every path, and publishes output only afterwards.

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

const ADMITTED_PERMITS: [&str; 5] = [
    crate::command_io_ops::ARGS_READ_EFFECT,
    crate::command_io_ops::STDERR_WRITE_EFFECT,
    crate::command_io_ops::STDIN_READ_EFFECT,
    crate::host_io_ops::STDOUT_WRITE_EFFECT,
    ops::NETWORK_HTTP_EFFECT,
];

pub fn emit_c_with_https_io(program: &Program, command_id: &str) -> Result<String, Diagnostic> {
    let resolved = hir::resolve(program).map_err(first_backend_diagnostic)?;
    emit_hir_c_with_https_io(&resolved, command_id)
}

pub fn emit_hir_c_with_https_io(
    program: &ResolvedProgram,
    command_id: &str,
) -> Result<String, Diagnostic> {
    hir::validate(program)?;
    reject_native_rust_for_native(program)?;
    if program
        .permits
        .iter()
        .any(|permit| !ADMITTED_PERMITS.contains(&permit.as_str()))
        || !program
            .permits
            .iter()
            .any(|permit| permit == ops::NETWORK_HTTP_EFFECT)
    {
        return Err(backend_error(
            "HTTPS command permits must be exactly within command I/O and network.http",
        ));
    }
    let command = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == command_id)
        .ok_or_else(|| backend_error(format!("selected HTTPS command `{command_id}` is absent")))?;
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
            "selected HTTPS command must be an explicit stable-ID `fn () -> bool`",
        ));
    }
    crate::command_io_ops::validate_operation_profile(
        program,
        &command.id,
        crate::command_io_ops::CommandOperationProfile::HttpV1,
    )?;
    emit_hir_c_with_labels(
        program,
        &HashMap::new(),
        NativeOutputProfile::HttpsCommandIo,
        Some(&command.id),
    )
}

pub(super) fn emit_runtime(output: &mut impl COutput, program: &ResolvedProgram) {
    if !super::program_uses_byte_data(program) {
        native_byte_data::emit_runtime(output);
    }
    native_host_output::emit_line_command_runtime(output);
    native_command_io::emit_line_runtime(output);
    emit_constants(output);
    emit_mozilla_roots(output);
    output.push_str(HTTPS_RUNTIME_C);
}

fn emit_constants(output: &mut impl COutput) {
    writeln!(
        output,
        "#define SPX_HTTP_STATUS_DOMAIN_V1 \"{domain}\"\n\
         #define SPX_HTTP_INVALID_URL_V1 UINT32_C({invalid_url})\n\
         #define SPX_HTTP_INSECURE_SCHEME_V1 UINT32_C({insecure_scheme})\n\
         #define SPX_HTTP_TRANSPORT_FAILED_V1 UINT32_C({transport_failed})\n\
         #define SPX_HTTP_RESPONSE_TOO_LARGE_V1 UINT32_C({response_too_large})\n\
         #define SPX_HTTP_UNSUPPORTED_VERSION_V1 UINT32_C({unsupported_version})\n\
         #define SPX_HTTP_AUTHORITY_DENIED_V1 UINT32_C({authority_denied})\n\
         #define SPX_HTTP_MAX_URL_BYTES_V1 UINT64_C(2048)\n\
         #define SPX_HTTP_MAX_RESPONSE_BYTES_V1 UINT64_C({max_response})\n\
         #define SPX_HTTP_MAX_TOTAL_BYTES_V1 UINT64_C({max_total})\n\
         #define SPX_HTTP_MAX_HEADERS_V1 UINT64_C(128)\n\
         #define SPX_HTTP_TIMEOUT_MILLIS_V1 30000L\n\
         #define SPX_HTTP_MAX_REDIRECTS_V1 10L\n\
         #define SPX_HTTP_MAX_CONNECTIONS_V1 8L",
        domain = ops::HTTP_STATUS_DOMAIN,
        invalid_url = ops::HTTP_INVALID_URL,
        insecure_scheme = ops::HTTP_INSECURE_SCHEME,
        transport_failed = ops::HTTP_TRANSPORT_FAILED,
        response_too_large = ops::HTTP_RESPONSE_TOO_LARGE,
        unsupported_version = ops::HTTP_UNSUPPORTED_VERSION,
        authority_denied = ops::HTTP_AUTHORITY_DENIED,
        max_response = ops::MAX_CHUNK_BYTES,
        max_total = ops::MAX_TOTAL_BYTES,
    )
    .expect("writing native HTTPS constants cannot fail");
}

fn emit_mozilla_roots(output: &mut impl COutput) {
    output.push_str("static const char spx_https_mozilla_roots_v1[] =\n");
    for line in include_str!("../mozilla-roots.pem").lines() {
        writeln!(output, "\"{line}\\n\"").expect("writing native HTTPS root bundle cannot fail");
    }
    output.push_str(";\n");
}

const HTTPS_RUNTIME_C: &str = r#"
#include <curl/curl.h>

#if LIBCURL_VERSION_NUM < 0x075500
#error "SEMAPRAX HTTPS native output requires libcurl 7.85.0 or newer"
#endif

struct spx_http_header_v1 {
    uint32_t name_offset;
    uint32_t name_length;
    uint32_t value_offset;
    uint32_t value_length;
};

struct spx_https_state_v1 {
    bool granted;
    bool overflow;
    bool malformed_header;
    uint64_t max;
    uint64_t total_bytes;
    size_t body_length;
    size_t header_bytes_length;
    size_t header_count;
    CURL *client;
    uint8_t body[SPX_HTTP_MAX_RESPONSE_BYTES_V1];
    uint8_t header_bytes[SPX_HTTP_MAX_RESPONSE_BYTES_V1];
    uint8_t canonical[SPX_HTTP_MAX_RESPONSE_BYTES_V1];
    struct spx_http_header_v1 headers[SPX_HTTP_MAX_HEADERS_V1];
};

struct spx_https_command_state_v1 {
    struct spx_language_command_state_v1 command;
    struct spx_https_state_v1 https;
};

_Static_assert(
    offsetof(struct spx_https_command_state_v1, command) == 0,
    "language command state must be the HTTPS target-state prefix"
);

/* Production uses the compiler-owned Mozilla bundle. Focused generated-C
   tests replace only this null initializer with one explicit fixture CA path. */
static const char *spx_https_ca_info_v1 = NULL;

static __attribute__((unused)) struct spx_https_state_v1 *spx_https_state_v1(
    struct spx_context *spx_ctx
) {
    if (spx_ctx == NULL || spx_ctx->target_state == NULL) {
        spx_runtime_invariant_failure("HTTPS state is unavailable");
    }
    return &((struct spx_https_command_state_v1 *)spx_ctx->target_state)->https;
}

static __attribute__((unused)) spx_status_token spx_http_status_v1(
    struct spx_context *spx_ctx,
    uint32_t code
) {
    if (code < SPX_HTTP_INVALID_URL_V1 || code > SPX_HTTP_AUTHORITY_DENIED_V1) {
        spx_runtime_invariant_failure("HTTP status is outside the closed table");
    }
    spx_status_token token = SPX_STATUS_SUCCESS;
    if (!spx_status_record_adapter(
        spx_ctx,
        SPX_HTTP_STATUS_DOMAIN_V1,
        code,
        SPX_STATUS_CLASS_ADAPTER,
        SPX_RETRYABILITY_FALSE,
        &token
    )) {
        spx_runtime_invariant_failure("HTTP status could not be recorded");
    }
    return token;
}

static bool spx_http_size_v1(size_t size, size_t count, size_t *result) {
    if (result == NULL || (size != 0u && count > SIZE_MAX / size)) return false;
    *result = size * count;
    return true;
}

static size_t spx_http_body_v1(char *input, size_t size, size_t count, void *opaque) {
    struct spx_https_state_v1 *state = (struct spx_https_state_v1 *)opaque;
    size_t length = 0u;
    if (state == NULL || input == NULL || !spx_http_size_v1(size, count, &length)) return 0u;
    if (length > (size_t)state->max - state->body_length) {
        state->overflow = true;
        return 0u;
    }
    memcpy(state->body + state->body_length, input, length);
    state->body_length += length;
    return length;
}

static bool spx_http_token_v1(uint8_t byte) {
    if ((byte >= (uint8_t)'0' && byte <= (uint8_t)'9') ||
        (byte >= (uint8_t)'A' && byte <= (uint8_t)'Z') ||
        (byte >= (uint8_t)'a' && byte <= (uint8_t)'z')) return true;
    switch (byte) {
        case (uint8_t)'!': case (uint8_t)'#': case (uint8_t)'$':
        case (uint8_t)'%': case (uint8_t)'&': case (uint8_t)'\'':
        case (uint8_t)'*': case (uint8_t)'+': case (uint8_t)'-':
        case (uint8_t)'.': case (uint8_t)'^': case (uint8_t)'_':
        case (uint8_t)'`': case (uint8_t)'|': case (uint8_t)'~': return true;
        default: return false;
    }
}

static size_t spx_http_header_v1(char *input, size_t size, size_t count, void *opaque) {
    struct spx_https_state_v1 *state = (struct spx_https_state_v1 *)opaque;
    size_t length = 0u;
    if (state == NULL || input == NULL || !spx_http_size_v1(size, count, &length)) return 0u;
    if (length >= 5u && memcmp(input, "HTTP/", 5u) == 0) {
        state->header_count = 0u;
        state->header_bytes_length = 0u;
        state->body_length = 0u;
        return length;
    }
    size_t content = length;
    while (content != 0u && (input[content - 1u] == '\r' || input[content - 1u] == '\n')) --content;
    if (content == 0u) return length;
    size_t colon = 0u;
    while (colon < content && input[colon] != ':') ++colon;
    if (state->header_count >= (size_t)SPX_HTTP_MAX_HEADERS_V1) {
        state->overflow = true;
        return 0u;
    }
    if (colon == 0u || colon == content) {
        state->malformed_header = true;
        return 0u;
    }
    for (size_t index = 0u; index < colon; ++index) {
        if (!spx_http_token_v1((uint8_t)input[index])) {
            state->malformed_header = true;
            return 0u;
        }
    }
    size_t value_start = colon + 1u;
    while (value_start < content && (input[value_start] == ' ' || input[value_start] == '\t')) {
        ++value_start;
    }
    size_t value_end = content;
    while (value_end > value_start && (input[value_end - 1u] == ' ' || input[value_end - 1u] == '\t')) {
        --value_end;
    }
    size_t value_length = value_end - value_start;
    if (colon > UINT32_MAX || value_length > UINT32_MAX ||
        colon + value_length > (size_t)SPX_HTTP_MAX_RESPONSE_BYTES_V1 - state->header_bytes_length) {
        state->overflow = true;
        return 0u;
    }
    struct spx_http_header_v1 *header = &state->headers[state->header_count++];
    header->name_offset = (uint32_t)state->header_bytes_length;
    header->name_length = (uint32_t)colon;
    for (size_t index = 0u; index < colon; ++index) {
        uint8_t byte = (uint8_t)input[index];
        state->header_bytes[state->header_bytes_length++] =
            byte >= (uint8_t)'A' && byte <= (uint8_t)'Z' ? (uint8_t)(byte + 32u) : byte;
    }
    header->value_offset = (uint32_t)state->header_bytes_length;
    header->value_length = (uint32_t)value_length;
    memcpy(state->header_bytes + state->header_bytes_length, input + value_start, value_length);
    state->header_bytes_length += value_length;
    return length;
}

static int spx_http_compare_header_v1(
    const struct spx_https_state_v1 *state,
    const struct spx_http_header_v1 *left,
    const struct spx_http_header_v1 *right
) {
    const uint8_t *left_name = state->header_bytes + left->name_offset;
    const uint8_t *right_name = state->header_bytes + right->name_offset;
    size_t common = left->name_length < right->name_length ? left->name_length : right->name_length;
    int order = memcmp(left_name, right_name, common);
    if (order != 0) return order;
    if (left->name_length != right->name_length) return left->name_length < right->name_length ? -1 : 1;
    const uint8_t *left_value = state->header_bytes + left->value_offset;
    const uint8_t *right_value = state->header_bytes + right->value_offset;
    common = left->value_length < right->value_length ? left->value_length : right->value_length;
    order = memcmp(left_value, right_value, common);
    if (order != 0) return order;
    if (left->value_length == right->value_length) return 0;
    return left->value_length < right->value_length ? -1 : 1;
}

static void spx_http_sort_headers_v1(struct spx_https_state_v1 *state) {
    for (size_t index = 1u; index < state->header_count; ++index) {
        struct spx_http_header_v1 value = state->headers[index];
        size_t position = index;
        while (position != 0u &&
               spx_http_compare_header_v1(state, &value, &state->headers[position - 1u]) < 0) {
            state->headers[position] = state->headers[position - 1u];
            --position;
        }
        state->headers[position] = value;
    }
}

static bool spx_http_name_is_v1(
    const struct spx_https_state_v1 *state,
    const struct spx_http_header_v1 *header,
    const char *name,
    size_t length
) {
    return header->name_length == length &&
        memcmp(state->header_bytes + header->name_offset, name, length) == 0;
}

static bool spx_http_hop_by_hop_v1(
    const struct spx_https_state_v1 *state,
    const struct spx_http_header_v1 *header
) {
    return spx_http_name_is_v1(state, header, "connection", 10u) ||
        spx_http_name_is_v1(state, header, "content-length", 14u) ||
        spx_http_name_is_v1(state, header, "keep-alive", 10u) ||
        spx_http_name_is_v1(state, header, "proxy-authenticate", 18u) ||
        spx_http_name_is_v1(state, header, "proxy-authorization", 19u) ||
        spx_http_name_is_v1(state, header, "te", 2u) ||
        spx_http_name_is_v1(state, header, "trailer", 7u) ||
        spx_http_name_is_v1(state, header, "transfer-encoding", 17u) ||
        spx_http_name_is_v1(state, header, "upgrade", 7u);
}

static bool spx_http_append_v1(
    struct spx_https_state_v1 *state,
    size_t *length,
    const void *bytes,
    size_t count
) {
    if (state == NULL || length == NULL || bytes == NULL ||
        count > (size_t)state->max - *length) return false;
    memcpy(state->canonical + *length, bytes, count);
    *length += count;
    return true;
}

static bool spx_http_render_v1(
    struct spx_https_state_v1 *state,
    long status,
    const char *version,
    size_t *result_length
) {
    char prefix[96];
    int prefix_length = snprintf(
        prefix, sizeof(prefix),
        "HTTP/1.1 %ld semaprax\r\nx-semaprax-http-version: %s\r\n",
        status, version
    );
    if (prefix_length <= 0 || (size_t)prefix_length >= sizeof(prefix)) return false;
    size_t length = 0u;
    if (!spx_http_append_v1(state, &length, prefix, (size_t)prefix_length)) return false;
    spx_http_sort_headers_v1(state);
    for (size_t index = 0u; index < state->header_count; ++index) {
        const struct spx_http_header_v1 *header = &state->headers[index];
        if (spx_http_hop_by_hop_v1(state, header)) continue;
        if (!spx_http_append_v1(
                state, &length, state->header_bytes + header->name_offset, header->name_length
            ) || !spx_http_append_v1(state, &length, ": ", 2u) ||
            !spx_http_append_v1(
                state, &length, state->header_bytes + header->value_offset, header->value_length
            ) || !spx_http_append_v1(state, &length, "\r\n", 2u)) return false;
    }
    char content_length[64];
    int content_length_size = snprintf(
        content_length, sizeof(content_length), "content-length: %zu\r\n\r\n", state->body_length
    );
    if (content_length_size <= 0 || (size_t)content_length_size >= sizeof(content_length) ||
        !spx_http_append_v1(state, &length, content_length, (size_t)content_length_size) ||
        (state->body_length != 0u &&
         !spx_http_append_v1(state, &length, state->body, state->body_length))) return false;
    *result_length = length;
    return true;
}

static bool spx_http_https_scheme_v1(const uint8_t *url, uint64_t length) {
    static const uint8_t scheme[8] = {'h', 't', 't', 'p', 's', ':', '/', '/'};
    if (length < UINT64_C(8)) return false;
    for (uint64_t index = UINT64_C(0); index < UINT64_C(5); ++index) {
        uint8_t byte = url[index];
        if (byte >= (uint8_t)'A' && byte <= (uint8_t)'Z') byte = (uint8_t)(byte + 32u);
        if (byte != scheme[index]) return false;
    }
    return memcmp(url + 5u, scheme + 5u, 3u) == 0;
}

static __attribute__((unused)) spx_status_token spx_host_https_get_v1(
    struct spx_context *spx_ctx,
    spx_slice_u8_v1 url,
    uint64_t max,
    spx_bytes_v1 *result_out
) {
    if (result_out == NULL) spx_runtime_invariant_failure("https_get result slot is unavailable");
    *result_out = (spx_bytes_v1){ .ptr = NULL, .len = UINT64_C(0) };
    struct spx_https_state_v1 *state = spx_https_state_v1(spx_ctx);
    spx_slice_u8_require_valid(url);
    if (!state->granted) return spx_http_status_v1(spx_ctx, SPX_HTTP_AUTHORITY_DENIED_V1);
    if (url.len == UINT64_C(0) || url.len > SPX_HTTP_MAX_URL_BYTES_V1 ||
        !spx_command_utf8_v1(url.ptr, url.len)) {
        return spx_http_status_v1(spx_ctx, SPX_HTTP_INVALID_URL_V1);
    }
    for (uint64_t index = UINT64_C(0); index < url.len; ++index) {
        if (url.ptr[index] == UINT8_C(0)) {
            return spx_http_status_v1(spx_ctx, SPX_HTTP_INVALID_URL_V1);
        }
    }
    if (!spx_http_https_scheme_v1(url.ptr, url.len)) {
        bool has_scheme = false;
        for (uint64_t index = UINT64_C(0); index < url.len; ++index) {
            if (url.ptr[index] == (uint8_t)':') { has_scheme = true; break; }
        }
        return spx_http_status_v1(
            spx_ctx, has_scheme ? SPX_HTTP_INSECURE_SCHEME_V1 : SPX_HTTP_INVALID_URL_V1
        );
    }
    if (max == UINT64_C(0) || max > SPX_HTTP_MAX_RESPONSE_BYTES_V1) {
        return spx_http_status_v1(spx_ctx, SPX_HTTP_RESPONSE_TOO_LARGE_V1);
    }
    if (state->client == NULL) return spx_http_status_v1(spx_ctx, SPX_HTTP_TRANSPORT_FAILED_V1);

    char url_text[SPX_HTTP_MAX_URL_BYTES_V1 + 1u];
    memcpy(url_text, url.ptr, (size_t)url.len);
    url_text[url.len] = '\0';
    state->max = max;
    state->overflow = false;
    state->malformed_header = false;
    state->body_length = 0u;
    state->header_bytes_length = 0u;
    state->header_count = 0u;
    struct curl_blob roots = {
        .data = (void *)spx_https_mozilla_roots_v1,
        .len = sizeof(spx_https_mozilla_roots_v1) - 1u,
        .flags = CURL_BLOB_NOCOPY
    };
    curl_easy_reset(state->client);
    CURLcode option = CURLE_OK;
#define SPX_CURL_SET_V1(setting, value) \
    do { if (option == CURLE_OK) option = curl_easy_setopt(state->client, setting, value); } while (0)
    SPX_CURL_SET_V1(CURLOPT_URL, url_text);
    SPX_CURL_SET_V1(CURLOPT_HTTPGET, 1L);
    SPX_CURL_SET_V1(CURLOPT_PROTOCOLS_STR, "https");
    SPX_CURL_SET_V1(CURLOPT_REDIR_PROTOCOLS_STR, "https");
    SPX_CURL_SET_V1(CURLOPT_FOLLOWLOCATION, 1L);
    SPX_CURL_SET_V1(CURLOPT_MAXREDIRS, SPX_HTTP_MAX_REDIRECTS_V1);
    SPX_CURL_SET_V1(CURLOPT_MAXCONNECTS, SPX_HTTP_MAX_CONNECTIONS_V1);
    SPX_CURL_SET_V1(CURLOPT_CONNECTTIMEOUT_MS, SPX_HTTP_TIMEOUT_MILLIS_V1);
    SPX_CURL_SET_V1(CURLOPT_TIMEOUT_MS, SPX_HTTP_TIMEOUT_MILLIS_V1);
    SPX_CURL_SET_V1(CURLOPT_NOSIGNAL, 1L);
    SPX_CURL_SET_V1(CURLOPT_PROXY, "");
    SPX_CURL_SET_V1(CURLOPT_SSL_VERIFYPEER, 1L);
    SPX_CURL_SET_V1(CURLOPT_SSL_VERIFYHOST, 2L);
    SPX_CURL_SET_V1(
        CURLOPT_SSLVERSION,
        (long)(CURL_SSLVERSION_TLSv1_2 | CURL_SSLVERSION_MAX_TLSv1_3)
    );
    SPX_CURL_SET_V1(CURLOPT_HTTP_VERSION, (long)CURL_HTTP_VERSION_2TLS);
    SPX_CURL_SET_V1(CURLOPT_WRITEFUNCTION, spx_http_body_v1);
    SPX_CURL_SET_V1(CURLOPT_WRITEDATA, state);
    SPX_CURL_SET_V1(CURLOPT_HEADERFUNCTION, spx_http_header_v1);
    SPX_CURL_SET_V1(CURLOPT_HEADERDATA, state);
    if (spx_https_ca_info_v1 != NULL) {
        SPX_CURL_SET_V1(CURLOPT_CAINFO, spx_https_ca_info_v1);
    } else {
        SPX_CURL_SET_V1(CURLOPT_CAINFO_BLOB, &roots);
    }
#undef SPX_CURL_SET_V1
    if (option != CURLE_OK) return spx_http_status_v1(spx_ctx, SPX_HTTP_TRANSPORT_FAILED_V1);

    CURLcode performed = curl_easy_perform(state->client);
    if (state->overflow) return spx_http_status_v1(spx_ctx, SPX_HTTP_RESPONSE_TOO_LARGE_V1);
    if (state->malformed_header) return spx_http_status_v1(spx_ctx, SPX_HTTP_TRANSPORT_FAILED_V1);
    if (performed == CURLE_URL_MALFORMAT || performed == CURLE_UNSUPPORTED_PROTOCOL) {
        return spx_http_status_v1(spx_ctx, SPX_HTTP_INVALID_URL_V1);
    }
    if (performed != CURLE_OK) return spx_http_status_v1(spx_ctx, SPX_HTTP_TRANSPORT_FAILED_V1);

    long status = 0L;
    long http_version = CURL_HTTP_VERSION_NONE;
    if (curl_easy_getinfo(state->client, CURLINFO_RESPONSE_CODE, &status) != CURLE_OK ||
        curl_easy_getinfo(state->client, CURLINFO_HTTP_VERSION, &http_version) != CURLE_OK ||
        status < 100L || status > 999L) {
        return spx_http_status_v1(spx_ctx, SPX_HTTP_TRANSPORT_FAILED_V1);
    }
    const char *version = NULL;
    switch (http_version) {
        case CURL_HTTP_VERSION_1_0: version = "1.0"; break;
        case CURL_HTTP_VERSION_1_1: version = "1.1"; break;
        case CURL_HTTP_VERSION_2_0: version = "2"; break;
        case CURL_HTTP_VERSION_3: version = "3"; break;
        default: return spx_http_status_v1(spx_ctx, SPX_HTTP_UNSUPPORTED_VERSION_V1);
    }
    size_t result_length = 0u;
    if (!spx_http_render_v1(state, status, version, &result_length) ||
        result_length > (size_t)(SPX_HTTP_MAX_TOTAL_BYTES_V1 - state->total_bytes)) {
        return spx_http_status_v1(spx_ctx, SPX_HTTP_RESPONSE_TOO_LARGE_V1);
    }
    uint8_t *copy = (uint8_t *)malloc(result_length);
    if (copy == NULL) return spx_http_status_v1(spx_ctx, SPX_HTTP_TRANSPORT_FAILED_V1);
    memcpy(copy, state->canonical, result_length);
    state->total_bytes += result_length;
    result_out->ptr = copy;
    result_out->len = result_length;
    return SPX_STATUS_SUCCESS;
}

static void spx_https_settle_v1(struct spx_https_state_v1 *state) {
    if (state == NULL) return;
    state->granted = false;
    if (state->client != NULL) curl_easy_cleanup(state->client);
    state->client = NULL;
}
"#;

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
    if (curl_global_init(CURL_GLOBAL_DEFAULT) != CURLE_OK) return 0;
    struct spx_status_entry spx_status_entries[UINT32_C(1)];
    struct spx_https_command_state_v1 state = {{0}};
    state.command.input = input;
    state.https.client = curl_easy_init();
    state.https.granted = true;
    struct spx_context spx_ctx = {{0}};
    if (!spx_context_init(
        &spx_ctx, UINT64_C(1), spx_status_entries, UINT32_C(1), NULL, NULL, &state
    )) {{
        spx_https_settle_v1(&state.https);
        curl_global_cleanup();
        return 0;
    }}
    bool matched = false;
    spx_status_token status = {command_symbol}(&spx_ctx, &matched);
    spx_https_settle_v1(&state.https);
    curl_global_cleanup();
    if (status != SPX_STATUS_SUCCESS) {{
        const struct spx_normalized_status *failure = spx_status_resolve(&spx_ctx, status);
        (void)spx_status_resolve_detail(&spx_ctx, status);
        if (failure == NULL || failure->domain_id == NULL) {{
            memset(&state, 0, sizeof(state));
            memset(result_out, 0, sizeof(*result_out));
            return 0;
        }}
        size_t domain_size = 0u;
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
        memcpy(result_out->stdout_bytes, state.command.output.stdout_bytes,
               (size_t)state.command.output.stdout_length);
    }}
    if (state.command.output.stderr_length != UINT64_C(0)) {{
        memcpy(result_out->stderr_bytes, state.command.output.stderr_bytes,
               (size_t)state.command.output.stderr_length);
    }}
    result_out->stdout_length = state.command.output.stdout_length;
    result_out->stderr_length = state.command.output.stderr_length;
    memset(&state, 0, sizeof(state));
    return 1;
}}
"#,
        run_symbol = native_command_io::RUN_SYMBOL,
    )
    .expect("writing native HTTPS command runner cannot fail");
}

#[cfg(all(test, unix))]
mod tests {
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    use super::*;

    static SERIAL: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn source_for(url: &str) -> String {
        let source = include_str!("../../../examples/https-project/src/app.spx");
        let start = source.find("    let url = [").unwrap();
        let relative_end = source[start..].find("];\n").unwrap();
        let end = start + relative_end + 2;
        let bytes = url
            .bytes()
            .map(|byte| format!("{byte}u8"))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "{}    let url = [{bytes}];{}",
            &source[..start],
            &source[end..]
        )
        .replace(
            "https_get(array_as_slice(url), 1024usize)",
            "https_get(array_as_slice(url), 65536usize)",
        )
        .replace("== 46usize", "> 0usize")
    }

    fn c_string(path: &Path) -> String {
        path.to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
    }

    #[test]
    fn embedded_mozilla_root_bundle_is_pinned() {
        use sha2::{Digest, Sha256};

        let roots = include_bytes!("../mozilla-roots.pem");
        assert_eq!(roots.len(), 218_196);
        assert_eq!(
            roots
                .windows(b"-----BEGIN CERTIFICATE-----".len())
                .filter(|window| *window == b"-----BEGIN CERTIFICATE-----")
                .count(),
            146
        );
        assert_eq!(
            format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(roots))),
            "d839471cd89ace6cb060941d0cc880d79bded8230768d838900fcaa53f335b50"
        );
    }

    #[test]
    fn generated_c11_https_executes_verified_tls_over_loopback() {
        let fixture = Fixture(std::env::temp_dir().join(format!(
            "semaprax-native-https-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        )));
        std::fs::create_dir(&fixture.0).unwrap();
        let cert = fixture.0.join("localhost.pem");
        let key = fixture.0.join("localhost-key.pem");
        let generated = Command::new("openssl")
            .args(["req", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout"])
            .arg(&key)
            .arg("-out")
            .arg(&cert)
            .args([
                "-subj",
                "/CN=localhost",
                "-addext",
                "subjectAltName=DNS:localhost",
                "-days",
                "1",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(
            generated.success(),
            "openssl did not create the TLS fixture"
        );

        let reservation = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = reservation.local_addr().unwrap().port();
        drop(reservation);
        let mut server = Command::new("openssl")
            .args(["s_server", "-quiet", "-www", "-accept"])
            .arg(port.to_string())
            .arg("-cert")
            .arg(&cert)
            .arg("-key")
            .arg(&key)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        std::thread::sleep(Duration::from_millis(200));

        let source = source_for(&format!("https://localhost:{port}/"));
        let ast = crate::parse(&source, Path::new("native-https-loopback.spx")).unwrap();
        let emitted = emit_c_with_https_io(&ast, "https-client.fetch").unwrap();
        let configured = emitted.replacen(
            "static const char *spx_https_ca_info_v1 = NULL;",
            &format!(
                "static const char *spx_https_ca_info_v1 = \"{}\";",
                c_string(&cert)
            ),
            1,
        );
        assert_ne!(configured, emitted);
        let executable = fixture.0.join("client");
        crate::codegen::compile_native_https_command_executable(&configured, &executable).unwrap();
        let run = Command::new(&executable)
            .env("HTTPS_PROXY", "http://127.0.0.1:9")
            .output()
            .unwrap();
        let _ = server.kill();
        let _ = server.wait();
        assert!(
            run.status.success(),
            "native HTTPS command failed: {}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert!(run.stderr.is_empty());
        assert!(
            run.stdout
                .starts_with(b"HTTP/1.1 200 semaprax\r\nx-semaprax-http-version: 1.0\r\n"),
            "unexpected canonical response: {}",
            String::from_utf8_lossy(&run.stdout)
        );
        assert!(
            run.stdout.ends_with(b"</HTML>\r\n\r\n"),
            "unexpected canonical response ending: {:?}",
            &run.stdout[run.stdout.len().saturating_sub(32)..]
        );
        assert!(!run
            .stdout
            .windows(b"transfer-encoding".len())
            .any(|window| window == b"transfer-encoding"));
    }

    /// Opt-in public-PKI smoke for the compiler-owned Mozilla root bundle.
    /// Deterministic gates use the loopback certificate above instead.
    #[test]
    #[ignore = "requires public DNS and network authority"]
    fn generated_c11_https_accepts_a_public_mozilla_root() {
        let fixture = Fixture(std::env::temp_dir().join(format!(
            "semaprax-native-public-https-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        )));
        std::fs::create_dir(&fixture.0).unwrap();
        let source = source_for("https://example.com/");
        let ast = crate::parse(&source, Path::new("native-public-https.spx")).unwrap();
        let emitted = emit_c_with_https_io(&ast, "https-client.fetch").unwrap();
        let executable = fixture.0.join("client");
        crate::codegen::compile_native_https_command_executable(&emitted, &executable).unwrap();
        let run = Command::new(&executable).output().unwrap();
        assert!(
            run.status.success(),
            "native public HTTPS command failed: {}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert!(run.stderr.is_empty());
        assert!(
            run.stdout.starts_with(b"HTTP/1.1 200 semaprax\r\n"),
            "unexpected canonical response: {}",
            String::from_utf8_lossy(&run.stdout)
        );
        assert!(
            [b"1.1".as_slice(), b"2".as_slice()].iter().any(|version| {
                let marker = [
                    b"x-semaprax-http-version: ".as_slice(),
                    version,
                    b"\r\n".as_slice(),
                ]
                .concat();
                run.stdout
                    .windows(marker.len())
                    .any(|window| window == marker)
            }),
            "public endpoint did not negotiate an admitted HTTP version"
        );
    }
}
