#!/usr/bin/env node
// The existing MCP adapter and token observer also carry selected-law v7
// results. This is a scripted codec test; the Workspace selector owns the
// separate real installed-Z3 physical gate.
import assert from 'node:assert/strict';
import {
  MCP_PROTOCOL_VERSION, ToolPayloadObserver, connectMcpWorkflowTransport,
} from '../packages/semaprax-agent-workflow/dist/index.js';

const revision = `sha256:${'a'.repeat(64)}`;
const calls = [];
const notices = [];
const result = (id, value) => `${JSON.stringify({ jsonrpc: '2.0', id, result: value })}\n`;
const wire = {
  sessionId: 'law12-selected-mcp',
  exchange(frame) {
    const request = JSON.parse(frame);
    if (request.method === 'initialize') {
      assert.equal(request.params.protocolVersion, MCP_PROTOCOL_VERSION);
      return result(request.id, {
        protocolVersion: MCP_PROTOCOL_VERSION,
        capabilities: { tools: { listChanged: false } },
        serverInfo: { name: 'semaprax-selected-law', version: '0.7.0' },
      });
    }
    assert.equal(request.method, 'tools/call');
    calls.push(request.params);
    const inner = request.params.name === 'law__status'
      ? { schema: 'semaprax.selected-law-agent-status.v1', candidate_revision: revision }
      : {
          schema: 'semaprax.project-law-workflow-cli.v2',
          candidate_revision: revision, selected_law_id: 'app.law',
          proof_attempt: { outcome: 'disproved_concrete' },
          view: { accepted: false, counts: { required: 1, satisfied: 0 } },
          validity: { schema: 'semaprax.selected-law-validity.v1', accepted: false,
            proof_attempt: 'disproved_concrete', counts: { required: 1, satisfied: 0 },
            delivery_independent: true },
          work: { schema: 'semaprax.installed-law-work.v1',
            reserved_process_invocations: 4, reserved_solver_queries: 2,
            reserved_io_bytes: 1024, model_tokens: null,
            provider_cost_micros: null, cost_status: 'unavailable' },
          source_authority: false, publication_authority: false,
        };
    return result(request.id, {
      content: [{ type: 'text', text: JSON.stringify({ jsonrpc: '2.0', id: 0, result: inner }) }],
      isError: false,
    });
  },
  notify(frame) { notices.push(JSON.parse(frame)); },
};
const observer = new ToolPayloadObserver({ sessionId: wire.sessionId });
const transport = await connectMcpWorkflowTransport(wire, observer);
const call = async (id, method, params) => {
  const response = JSON.parse(await transport.exchange(`${JSON.stringify({
    jsonrpc: '2.0', id, method, params,
  })}\n`));
  assert.equal(response.id, id);
  return response.result;
};
assert.deepEqual(notices, [{ jsonrpc: '2.0', method: 'notifications/initialized' }]);
const status = await call('status', 'law/status', {});
assert.equal(status.candidate_revision, revision);
const checked = await call('check', 'law/check', {
  candidate_revision: revision, law_id: 'app.law', view: 'summary', limit: 1,
});
assert.equal(checked.proof_attempt.outcome, 'disproved_concrete');
assert.equal(checked.view.accepted, false);
assert.equal(checked.view.counts.required, 1);
assert.equal(checked.validity.accepted, false);
assert.equal(checked.validity.proof_attempt, 'disproved_concrete');
assert.equal(checked.work.cost_status, 'unavailable');
assert.deepEqual(calls.map(call => call.name), ['law__status', 'law__check']);
assert.deepEqual(calls[1].arguments, {
  candidate_revision: revision, law_id: 'app.law', view: 'summary', limit: 1,
});
await observer.drain();
const events = observer.events();
assert.equal(events.length, 2);
assert(events.every(event => event.schema === 'semaprax.token-observation.v1'
  && event.boundary === 'mcp_content_0_text' && event.bytes > 0));
assert.equal(events[1].subjectRevision, revision);
// Success here means the exact tool payload was delivered, not that the law
// passed. The independent strict view above preserves the failed validity.
assert.equal(events[1].outcome, 'success');
assert.equal(observer.summary().partial, false);
process.stdout.write('LAW-12 existing MCP observer bridge: 1 passed\n');
