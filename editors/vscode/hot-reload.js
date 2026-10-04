'use strict';
const { EventEmitter } = require('node:events');
const path = require('node:path');
const MAX_FRAME = 8192, MAX_HISTORY = 32;
class HotReload extends EventEmitter {
  constructor(spawn, compiler, manifest) { super(); if (!path.isAbsolute(compiler) || !path.isAbsolute(manifest)) throw new Error('Hot reload requires absolute machine paths'); this.spawn = spawn; this.compiler = compiler; this.manifest = manifest; this.child = null; this.id = 0; this.epoch = 0; this.buffer = ''; this.closed = true; this.terminal = false; this.pending = new Map(); this.state = { lane:'interpreter', active:null, pending:null, dirty:false, event:'stopped', detail:'Not started', history:[] }; }
  start() {
    if (!this.closed) throw new Error('Hot reload is already started');
    const child = this.spawn(this.compiler, ['dev', this.manifest, '--jsonl'], { shell: false, windowsHide: true, cwd: require('node:path').dirname(this.manifest), stdio: ['pipe','pipe','pipe'] });
    this.child = child; this.closed = false; this.terminal = false; const epoch = ++this.epoch;
    child.stdout.on('data', chunk => this.#data(epoch, chunk));
    child.once('exit', () => this.#terminal(epoch, 'process exited without a clean stop'));
    child.once('error', () => this.#terminal(epoch, 'process failed'));
    return this.request('start');
  }
  request(op) {
    if (this.closed || this.terminal || !this.child) throw new Error('Hot reload is not started');
    const id = ++this.id; const frame = JSON.stringify({ schema: 'semaprax.hot-reload-control.v1', id, op }) + '\n';
    if (Buffer.byteLength(frame) > MAX_FRAME) throw new Error('Hot reload request exceeds its bound');
    this.pending.set(id, { op, epoch: this.epoch }); this.child.stdin.write(frame); return id;
  }
  markDirty() { if (!this.closed) { this.state.dirty = true; this.state.detail = 'Saved source changed; active code may be older than the editor'; this.emit('status', this.detail()); } }
  detail() { return { ...this.state, history: [...this.state.history] }; }
  stop() { const child=this.child; if (!this.closed && child) { try { this.request('stop'); } catch {} } this.closed=true; this.pending.clear(); this.child=null; child?.kill(); this.state={...this.state,event:'stopped',pending:null,detail:'Stopped'}; this.emit('status',this.detail()); }
  #data(epoch, chunk) { if (this.closed || epoch !== this.epoch) return; this.buffer += chunk.toString('utf8'); if (Buffer.byteLength(this.buffer) > MAX_FRAME) return this.#terminal(epoch, 'response exceeds its bound'); let i; while ((i = this.buffer.indexOf('\n')) >= 0) { const line = this.buffer.slice(0,i); this.buffer = this.buffer.slice(i+1); try { const value = JSON.parse(line), request=this.pending.get(value?.id); if (!value || value.schema !== 'semaprax.hot-reload-control.v1' || !Number.isSafeInteger(value.id) || typeof value.event !== 'string' || !request || request.epoch !== epoch) throw Error(); this.pending.delete(value.id); this.state.active=typeof value.active_project_revision==='string'?value.active_project_revision:this.state.active; this.state.pending=value.plan?.candidate_project_revision||null; this.state.dirty=false; this.state.event=value.event; this.state.detail=value.event==='candidate_rejected'?'Candidate rejected; previous active code remains available':value.event==='candidate_admitted'?'Checked candidate pending explicit activation':value.event==='waiting_safe_point'?'Waiting for a safe activation boundary':value.event==='activated'?'Active new checked code':value.event==='terminal_uncertainty'?'Activation state is unknown':value.event; this.state.history.push({event:value.event,active:this.state.active}); if(this.state.history.length>MAX_HISTORY)this.state.history.shift(); if(value.event==='terminal_uncertainty')this.terminal=true; this.emit('status',this.detail()); } catch { return this.#terminal(epoch,'malformed, stale, or unsolicited hot reload response'); } } }
  #terminal(epoch,message) { if(this.closed||epoch!==this.epoch)return; this.terminal=true; this.closed=true; this.pending.clear(); this.child?.kill(); this.state={...this.state,event:'unknown',pending:null,detail:message}; this.emit('status',this.detail()); this.emit('terminal',message); }
}
module.exports = { HotReload, MAX_HISTORY };
