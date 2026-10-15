"""One bounded source-only Claude review; no retry, tools, or child execution."""
import hashlib
import json
import os
from pathlib import Path
import selectors
import signal
import subprocess
import tempfile
import time

HERE = Path(__file__).resolve().parent
PROMPT = HERE / 'payload.txt'
ARGV = ['/Users/kevin/.local/bin/claude', '--print', '--output-format', 'json',
        '--model', 'sonnet', '--tools', '', '--no-session-persistence',
        '--safe-mode', '--restricted', '--strict-mcp-config',
        '--permission-prompts', 'none', '--prompt-suggestions', 'false',
        '--max-budget-usd', '3.5', '--system-prompt',
        'Review only the pasted source. No tools, code execution, delegation, file access, or web. '
        'Identify concrete semantic, Rust typing, accounting, or test mistakes with source line references. '
        'Distinguish blockers from uncertainties. Do not claim execution. Keep findings under 900 words.']
TIME_LIMIT = 120
OUTPUT_LIMIT = 2 * 1024 * 1024
RSS_LIMIT = 768 * 1024 * 1024
start = time.monotonic()
output = {'stdout': bytearray(), 'stderr': bytearray()}
reason = None
peak = 0
process = None
with tempfile.TemporaryDirectory(prefix='weighted-exact-capacity-review-', dir='/Volumes/Semaprax/tmp') as scratch:
    with PROMPT.open('rb') as request, selectors.DefaultSelector() as selector:
        process = subprocess.Popen(ARGV, stdin=request, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                   cwd=scratch, start_new_session=True, close_fds=True)
        try:
            for stream, name in [(process.stdout, 'stdout'), (process.stderr, 'stderr')]:
                os.set_blocking(stream.fileno(), False)
                selector.register(stream, selectors.EVENT_READ, name)
            next_sample = 0
            while selector.get_map() or process.poll() is None:
                elapsed = time.monotonic() - start
                if elapsed >= TIME_LIMIT:
                    reason = 'deadline_120_seconds'
                    break
                if elapsed >= next_sample:
                    sample = subprocess.run(['ps', '-axo', 'pgid=,rss='], capture_output=True, text=True, timeout=2)
                    rss = sum(int(row.split()[1]) * 1024 for row in sample.stdout.splitlines()
                              if len(row.split()) == 2 and int(row.split()[0]) == process.pid)
                    peak = max(peak, rss)
                    if rss > RSS_LIMIT:
                        reason = 'process_group_rss_limit'
                        break
                    next_sample = elapsed + 0.25
                for key, _ in selector.select(0.05):
                    chunk = os.read(key.fileobj.fileno(), 65536)
                    if not chunk:
                        selector.unregister(key.fileobj)
                        continue
                    room = OUTPUT_LIMIT - sum(map(len, output.values()))
                    output[key.data].extend(chunk[:room])
                    if len(chunk) > room:
                        reason = 'output_limit'
                        break
                if reason:
                    break
        finally:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            process.wait(timeout=5)
            process.stdout.close()
            process.stderr.close()
receipt = {'source_commit': '4f8f598c7935d0a5cdd4a5d5ba5b49d164ae2cfc',
           'argv': ARGV, 'dispatches': 1, 'tools_enabled': False, 'code_execution': False,
           'wall_seconds': time.monotonic() - start, 'exit_code': process.returncode,
           'peak_process_group_rss_bytes': peak, 'rss_limit_bytes': RSS_LIMIT,
           'timeout_seconds': TIME_LIMIT, 'output_limit_bytes': OUTPUT_LIMIT,
           'resource_refusal': reason, 'payload_sha256': hashlib.sha256(PROMPT.read_bytes()).hexdigest(),
           'budget_usd_max': 3.5, 'actual_billing': 'unknown'}
for name, data in output.items():
    (HERE / ('result.json' if name == 'stdout' else 'stderr.log')).write_bytes(data)
try:
    result = json.loads(output['stdout'])
    receipt.update(model_usage=result.get('modelUsage'), usage=result.get('usage'),
                   provider_list_cost_usd=result.get('total_cost_usd'), is_error=result.get('is_error'),
                   num_turns=result.get('num_turns'), subtype=result.get('subtype'))
    if isinstance(result.get('result'), str):
        (HERE / 'review.md').write_text(result['result'] + '\n')
except (ValueError, TypeError):
    receipt['result_parse'] = 'not_json'
(HERE / 'process-receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
print(json.dumps(receipt, indent=2))
