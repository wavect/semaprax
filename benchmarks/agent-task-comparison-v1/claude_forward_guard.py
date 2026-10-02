"""Private, fixed-destination request guard; no credential values enter receipts.

This module proves byte/count/output-request bounds, not tokenizer or invoice
bounds. It is not enabled by the live pilot until those admission rules freeze.
"""
from __future__ import annotations

import hashlib
import http.client
import http.server
import json
import os
import re
import signal
import subprocess
from pathlib import Path
import secrets
import ssl
import threading
import time

HOST = 'api.anthropic.com'
CA_PATH = '/private/etc/ssl/cert.pem'
CA_SHA256 = '9dae8d76e55cb08991f2b672d58999ea15560d910759c16b544f843bdffbb994'
MODELS = frozenset(('claude-haiku-4-5-20251001', 'claude-sonnet-5-5'))
LIMITS = {'request_bytes': 32768, 'total_request_bytes': 65536,
          'forwards': 9, 'max_tokens': 512, 'response_bytes': 1048576}
ROOT_KEYS = frozenset(('model', 'messages', 'system', 'tools', 'tool_choice',
                       'max_tokens', 'stream', 'thinking', 'output_config',
                       'metadata', 'stop_sequences', 'temperature', 'top_p', 'top_k', 'context_management'))


class McpProcess:
    """Launch outside the CLI's seatbelt, under the original MCP seatbelt.

    The CLI gets only FIFO endpoints. macOS forbids applying a second seatbelt
    inside an already sandboxed process; this preserves the two distinct grants.
    """
    def __init__(self, state, command, cwd):
        self.state, self.command, self.cwd = state, command, cwd
        self.fds = []
        self.process = None

    def __enter__(self):
        try:
            for name in ('mcp-to-server', 'mcp-from-server'):
                path = self.state / name
                os.mkfifo(path, 0o600)
                self.fds.append(os.open(path, os.O_RDWR | os.O_NOFOLLOW))
            self.process = subprocess.Popen(self.command, cwd=self.cwd,
                env={'PATH': '/usr/bin:/bin', 'HOME': str(self.state), 'TMPDIR': str(self.state)},
                stdin=self.fds[0], stdout=self.fds[1], stderr=subprocess.DEVNULL,
                start_new_session=True, close_fds=True)
            return self
        except BaseException:
            self.__exit__()
            raise

    def __exit__(self, *_):
        if self.process is not None:
            # Reap an already-exited sandbox launcher before signaling its
            # group; Darwin can report EPERM for its unreaped zombie group.
            self.process.poll()
            try:
                os.killpg(self.process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            except PermissionError:
                if self.process.returncode is None:
                    raise
            self.process.wait(timeout=5)
        for fd in self.fds:
            os.close(fd)
        self.fds = []


def network_profile(port):
    if type(port) is not int or not 1 <= port <= 65535:
        raise ValueError('loopback_port_required')
    return ('(version 1)\n(allow default)\n(deny network*)\n'
            f'(allow network-outbound (remote ip "localhost:{port}"))\n')


def _pairs(items):
    result = {}
    for key, value in items:
        if key in result:
            raise ValueError('duplicate_json_key')
        result[key] = value
    return result


def _constant(_):
    raise ValueError('nonfinite_json')


def _walk(value):
    if isinstance(value, dict):
        if 'cache_control' in value:
            raise ValueError('cache_write_refused')
        for child in value.values():
            _walk(child)
    elif isinstance(value, list):
        for child in value:
            _walk(child)


def _content(value, types):
    if isinstance(value, str):
        return
    if not isinstance(value, list):
        raise ValueError('content_shape')
    for block in value:
        if not isinstance(block, dict) or block.get('type') not in types:
            raise ValueError('content_modality_refused')
        if block['type'] == 'tool_result':
            _content(block.get('content', ''), {'text'})


def admit(raw, model):
    if model not in MODELS or len(raw) > LIMITS['request_bytes']:
        raise ValueError('request_bound')
    try:
        value = json.loads(raw, object_pairs_hook=_pairs, parse_constant=_constant)
        if not isinstance(value, dict) or set(value) - ROOT_KEYS:
            raise ValueError('request_fields_refused')
        if value.get('model') != model:
            raise ValueError('model_mismatch')
        maximum = value.get('max_tokens')
        if type(maximum) is not int or not 1 <= maximum <= LIMITS['max_tokens']:
            raise ValueError('output_bound')
        if value.get('context_management', {'edits': []}) not in (
                {'edits': []}, {'edits': [{'type': 'clear_thinking_20251015', 'keep': 'all'}]}):
            raise ValueError('context_management_refused')
        messages = value.get('messages')
        if not isinstance(messages, list) or not messages:
            raise ValueError('messages_required')
        for message in messages:
            if (not isinstance(message, dict) or set(message) - {'role', 'content', 'output_config'}
                    or not {'role', 'content'} <= set(message) or message['role'] not in ('user', 'assistant', 'system')):
                raise ValueError('message_shape')
            if message.get('output_config', {'effort': 'medium'}) != {'effort': 'medium'}:
                raise ValueError('message_shape')
            _content(message['content'], {'text', 'tool_use', 'tool_result', 'thinking', 'redacted_thinking'})
        _content(value.get('system', ''), {'text'})
        tools = value.get('tools', [])
        if not isinstance(tools, list) or len(tools) != 1:
            raise ValueError('tools_refused')
        for tool in tools:
            if (not isinstance(tool, dict) or set(tool) - {'name', 'description', 'input_schema'}
                    or tool.get('name') != 'mcp__semaprax__command'):
                raise ValueError('server_or_other_tool_refused')
        _walk(value)
        # This also bounds multibyte text and JSON escapes after reserialization.
        forwarded = json.dumps(value, ensure_ascii=True, sort_keys=True, separators=(',', ':'), allow_nan=False).encode('ascii')
        if len(forwarded) > LIMITS['request_bytes']:
            raise ValueError('canonical_request_bound')
        return forwarded, maximum
    except (UnicodeError, RecursionError, OverflowError) as error:
        raise ValueError('invalid_json_encoding_or_depth') from error


def _tls_forward(body, headers, timeout):
    """No caller-controlled destination, path, TLS context, proxy or redirects."""
    roots = Path(CA_PATH).read_bytes()
    if hashlib.sha256(roots).hexdigest() != CA_SHA256:
        raise ValueError('tls_trust_drift')
    connection = http.client.HTTPSConnection(HOST, 443, timeout=timeout,
                        context=ssl.create_default_context(cadata=roots.decode('ascii')))
    deadline = time.monotonic() + timeout
    try:
        connection.request('POST', '/v1/messages?beta=true', body=body, headers=headers)
        if connection.sock is not None:
            connection.sock.settimeout(max(.01, deadline - time.monotonic()))
        response = connection.getresponse()
        chunks = []; total = 0
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise ValueError('guard_closed_or_deadline')
            if connection.sock is not None:
                connection.sock.settimeout(remaining)
            chunk = response.read1(min(65536, LIMITS['response_bytes'] + 1 - total))
            if not chunk:
                break
            chunks.append(chunk); total += len(chunk)
            if total > LIMITS['response_bytes']:
                raise ValueError('response_bound')
        data = b''.join(chunks)
        if 300 <= response.status < 400:
            raise ValueError('upstream_redirect_refused')
        return response.status, response.getheader('Content-Type', 'application/json'), data
    finally:
        connection.close()


class Guard:
    def __init__(self, model, seconds=120):
        if model not in MODELS or not 0 < seconds <= 120:
            raise ValueError('guard_configuration')
        self.model = model
        self.deadline = time.monotonic() + seconds
        self.lock = threading.Lock()
        self.events = []
        self.bytes = 0
        self.forwards = 0
        self.closed = False
        self.nonce = secrets.token_hex(24)
        owner = self

        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self, *_):
                pass

            def do_POST(self):
                self.connection.settimeout(min(5, max(.01, owner.deadline - time.monotonic())))
                try:
                    if self.path not in ('/' + owner.nonce + '/v1/messages', '/' + owner.nonce + '/v1/messages?beta=true'):
                        raise ValueError('path_refused')
                    lengths = self.headers.get_all('Content-Length', [])
                    if len(lengths) != 1 or not lengths[0].isascii() or not lengths[0].isdigit() or self.headers.get('Transfer-Encoding'):
                        raise ValueError('request_framing')
                    length = int(lengths[0])
                    if not 0 < length <= LIMITS['request_bytes']:
                        raise ValueError('request_bound')
                    chunks = []; total = 0
                    request_deadline = min(owner.deadline, time.monotonic() + 5)
                    while total < length:
                        remaining = request_deadline - time.monotonic()
                        if remaining <= 0:
                            raise ValueError('guard_closed_or_deadline')
                        self.connection.settimeout(remaining)
                        chunk = self.rfile.read1(length - total)
                        if not chunk:
                            raise ValueError('truncated_request')
                        chunks.append(chunk); total += len(chunk)
                    raw = b''.join(chunks)
                    body, output = admit(raw, owner.model)
                    # Values remain in memory only. No x-api-key, cookie, proxy,
                    # caller Host, compression, routing, or arbitrary headers.
                    headers = {'Content-Type': 'application/json', 'Accept-Encoding': 'identity'}
                    for name in ('Authorization', 'Anthropic-Version', 'Anthropic-Beta', 'User-Agent',
                                 'Anthropic-Dangerous-Direct-Browser-Access', 'X-App', 'X-Claude-Code-Session-Id',
                                 'X-Stainless-Arch', 'X-Stainless-Lang', 'X-Stainless-Os',
                                 'X-Stainless-Package-Version', 'X-Stainless-Retry-Count',
                                 'X-Stainless-Runtime', 'X-Stainless-Runtime-Version', 'X-Stainless-Timeout'):
                        values = self.headers.get_all(name, [])
                        if len(values) > 1:
                            raise ValueError('duplicate_forward_header')
                        if values:
                            headers[name] = values[0]
                    if not headers.get('Authorization', '').startswith('Bearer ') or self.headers.get('X-Api-Key'):
                        raise ValueError('subscription_auth_required')
                    with owner.lock:
                        if owner.closed or time.monotonic() >= owner.deadline:
                            raise ValueError('guard_closed_or_deadline')
                        if owner.forwards >= LIMITS['forwards'] or owner.bytes + len(body) > LIMITS['total_request_bytes']:
                            raise ValueError('aggregate_request_bound')
                        # Charge before forwarding, including ambiguous failures.
                        owner.forwards += 1
                        owner.bytes += len(body)
                        event = {'sequence': owner.forwards, 'request_sha256': hashlib.sha256(body).hexdigest(),
                                 'request_bytes': len(body), 'max_tokens': output, 'status': 'reserved'}
                        owner.events.append(event)
                    status, content_type, response = _tls_forward(body, headers, max(.01, owner.deadline - time.monotonic()))
                    event.update(status='returned', http_status=status, response_bytes=len(response),
                                 response_sha256=hashlib.sha256(response).hexdigest())
                except Exception as error:
                    # Never interpolate upstream/HTTP exceptions: they can embed
                    # request data or authentication values.
                    reason = str(error) if type(error) is ValueError and str(error) in SAFE_REASONS else 'guard_transport_failure'
                    with owner.lock:
                        owner.events.append({'status': 'refused', 'reason': reason})
                    status, content_type, response = 400, 'application/json', b'{"type":"error","error":{"type":"invalid_request_error","message":"pilot request guard refused"}}'
                try:
                    self.send_response(status)
                    self.send_header('Content-Type', content_type)
                    self.send_header('Content-Length', str(len(response)))
                    self.end_headers()
                    self.wfile.write(response)
                except OSError:
                    pass

        self.server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        self.server.daemon_threads = False
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)

    def __enter__(self):
        self.thread.start()
        return self

    @property
    def base_url(self):
        return f'http://127.0.0.1:{self.server.server_port}/{self.nonce}'

    def __exit__(self, *_):
        with self.lock:
            self.closed = True
        self.server.shutdown()
        self.server.server_close()
        self.thread.join()

    def receipt(self):
        with self.lock:
            return {'model': self.model, 'upstream': 'https://' + HOST + ':443/v1/messages?beta=true',
                    'tls_ca_sha256': CA_SHA256,
                    'limits': dict(LIMITS), 'forward_count': self.forwards,
                    'forwarded_request_bytes': self.bytes, 'events': [dict(x) for x in self.events],
                    'credential_values_retained': False, 'tokenizer_or_invoice_bound_proven': False}


SAFE_REASONS = frozenset(('path_refused', 'request_framing', 'request_bound', 'truncated_request',
    'duplicate_forward_header', 'subscription_auth_required', 'guard_closed_or_deadline',
    'aggregate_request_bound', 'duplicate_json_key', 'nonfinite_json', 'request_fields_refused',
    'model_mismatch', 'output_bound', 'messages_required', 'message_shape', 'content_shape',
    'content_modality_refused', 'tools_refused', 'server_or_other_tool_refused', 'cache_write_refused',
    'canonical_request_bound', 'invalid_json_encoding_or_depth', 'response_bound', 'upstream_redirect_refused',
    'context_management_refused', 'tls_trust_drift', 'guard_transport_failure'))


def validate_receipt(value, model):
    if (value.get('model') != model or value.get('limits') != LIMITS
            or value.get('tls_ca_sha256') != CA_SHA256
            or value.get('upstream') != 'https://' + HOST + ':443/v1/messages?beta=true'
            or value.get('credential_values_retained') is not False
            or value.get('tokenizer_or_invoice_bound_proven') is not False):
        raise ValueError('guard_receipt_subject')
    total = 0; count = 0
    for event in value['events']:
        if event.get('status') == 'refused':
            if set(event) != {'status', 'reason'} or event['reason'] not in SAFE_REASONS:
                raise ValueError('guard_receipt_refusal')
            continue
        count += 1
        if (event.get('status') not in ('reserved', 'returned') or event.get('sequence') != count
                or type(event.get('request_bytes')) is not int or not 0 < event['request_bytes'] <= LIMITS['request_bytes']
                or type(event.get('max_tokens')) is not int or not 0 < event['max_tokens'] <= LIMITS['max_tokens']
                or not re.fullmatch('[0-9a-f]{64}', event.get('request_sha256', ''))):
            raise ValueError('guard_receipt_forward')
        total += event['request_bytes']
        if event['status'] == 'returned' and (type(event.get('response_bytes')) is not int
                or not 0 <= event['response_bytes'] <= LIMITS['response_bytes']
                or not re.fullmatch('[0-9a-f]{64}', event.get('response_sha256', ''))):
            raise ValueError('guard_receipt_response')
    if (count != value.get('forward_count') or total != value.get('forwarded_request_bytes')
            or count > LIMITS['forwards'] or total > LIMITS['total_request_bytes']):
        raise ValueError('guard_receipt_aggregate')
