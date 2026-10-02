#!/usr/bin/env node
// Capture metadata-only token observations from two read-only calls to a real
// local serve-workspace-mcp process, then render the resulting report offline.
import { spawn, spawnSync } from 'node:child_process';
import { createInterface } from 'node:readline';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { ToolPayloadObserver, connectMcpWorkflowTransport } from '../packages/semaprax-agent-workflow/dist/index.js';

const [compilerArg, manifestArg, policyArg, outputArg] = process.argv.slice(2);
if (!compilerArg || !manifestArg || !policyArg || !outputArg) {
  throw new Error('usage: node scripts/token-report-mcp-session.mjs <semaprax> <manifest> <host-policy.json> <new-output-directory>');
}
const compiler = resolve(compilerArg), manifest = resolve(manifestArg), policy = resolve(policyArg), output = resolve(outputArg);
const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const manifestBefore = readFileSync(manifest);
const policyValue = JSON.parse(readFileSync(policy, 'utf8'));
if (policyValue.schema !== 'semaprax.workspace-host-policy.v7' || policyValue.git_commit !== null || policyValue.build_enabled !== false) {
  throw new Error('use a v7 local policy with no build or Git commit authority');
}
mkdirSync(output, { recursive: false });

const child = spawn(compiler, ['serve-workspace-mcp', manifest, policy], {
  cwd: dirname(manifest), stdio: ['pipe', 'pipe', 'pipe'], shell: false,
});
const reader = createInterface({ input: child.stdout, crlfDelay: Infinity });
const pending = [];
let stderr = '';
child.stderr.setEncoding('utf8');
child.stderr.on('data', chunk => { stderr += chunk; });
reader.on('line', line => pending.shift()?.resolve(line));
child.on('error', error => { for (const item of pending.splice(0)) item.reject(error); });

const wire = {
  sessionId: `local-token-report-${process.pid}`,
  exchange(frame) {
    if (!frame.endsWith('\n') || frame.slice(0, -1).includes('\n')) throw new Error('MCP frame must be one newline-terminated JSON line');
    const response = new Promise((resolveLine, rejectLine) => pending.push({ resolve: resolveLine, reject: rejectLine }));
    child.stdin.write(frame);
    return response;
  },
  notify(frame) {
    if (!frame.endsWith('\n') || frame.slice(0, -1).includes('\n')) throw new Error('MCP notification must be one newline-terminated JSON line');
    child.stdin.write(frame);
  },
};
const observer = new ToolPayloadObserver({ sessionId: wire.sessionId });

try {
  const transport = await connectMcpWorkflowTransport(wire, observer);
  const call = async (method, params) => {
    const frame = JSON.stringify({ jsonrpc: '2.0', id: method, method, params }) + '\n';
    const response = JSON.parse(await transport.exchange(frame));
    if (response.error) throw new Error(`${method}: ${response.error.message}`);
    return response.result;
  };
  const opened = await call('workspace/open', {});
  const imageRevision = opened.payload.image_revision;
  await call('workspace/refresh-preview', { image_revision: imageRevision });
  await observer.drain();
  const events = observer.events();
  if (events.length !== 2 || events.some(event => event.boundary !== 'mcp_content_0_text')) {
    throw new Error(`expected two observed MCP text responses; got ${events.length}`);
  }
  writeFileSync(resolve(output, 'events.jsonl'), events.map(event => JSON.stringify(event)).join('\n') + '\n', { flag: 'wx' });
  if (!manifestBefore.equals(readFileSync(manifest))) throw new Error('the local MCP journey changed the manifest');
  const reportPath = resolve(output, 'session-report.json');
  const summaryPath = resolve(output, 'session-report.txt');
  const python = process.env.PYTHON ?? 'python3';
  const report = spawnSync(python, [resolve(repoRoot, 'scripts/token_report.py'), 'session', '--events', resolve(output, 'events.jsonl'), '--output', reportPath], { cwd: repoRoot, encoding: 'utf8' });
  if (report.status !== 0) throw new Error(report.stderr || `token report exited ${report.status}`);
  const rendered = spawnSync(python, [resolve(repoRoot, 'scripts/token_report.py'), 'show', reportPath, '--format', 'text', '--output', summaryPath], { cwd: repoRoot, encoding: 'utf8' });
  if (rendered.status !== 0) throw new Error(rendered.stderr || `token report show exited ${rendered.status}`);
  process.stdout.write(readFileSync(summaryPath, 'utf8'));
} finally {
  child.stdin.end();
  const exit = await new Promise(resolveExit => child.once('exit', resolveExit));
  if (exit !== 0) throw new Error(`local MCP server exited ${exit}: ${stderr}`);
}
