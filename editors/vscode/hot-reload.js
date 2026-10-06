'use strict';
const { EventEmitter } = require('node:events');
const path = require('node:path');
const MAX_FRAME = 8192, MAX_HISTORY = 32, MAX_PENDING = 64, STOP_GRACE_MS = 1000;
class HotReload extends EventEmitter {
  constructor(spawn, compiler, manifest) { super(); if (!path.isAbsolute(compiler) || !path.isAbsolute(manifest)) throw new Error('Hot reload requires absolute machine paths'); this.spawn = spawn; this.compiler = compiler; this.manifest = manifest; this.child = null; this.id = 0; this.epoch = 0; this.chunks = []; this.frameBytes = 0; this.closed = true; this.terminal = false; this.stopping = false; this.pending = new Map(); this.stopTimer = null; this.state = { lane:'interpreter', active:null, pending:null, dirty:false, sourceChanged:false, event:'stopped', detail:'Not started', history:[] }; }
  start() {
    if (!this.closed) throw new Error('Hot reload is already started');
    const child = this.spawn(this.compiler, ['dev', this.manifest, '--jsonl'], { shell: false, windowsHide: true, cwd: require('node:path').dirname(this.manifest), stdio: ['pipe','pipe','pipe'] });
    this.child = child; this.closed = false; this.terminal = false; this.stopping = false; const epoch = ++this.epoch;
    this.chunks = []; this.frameBytes = 0;
    // The error consumer is never removed: a late EPIPE after Stop or from a superseded epoch is consumed here.
    child.stdin.on('error', () => this.#writeFailed(epoch));
    child.stdout.on('data', chunk => this.#data(epoch, chunk));
    child.stdout.on('end', () => { if (this.frameBytes > 0) this.#terminal(epoch, 'response stream ended inside an unterminated frame'); });
    child.once('exit', () => { if (this.closed) this.#clearStopTimer(); else this.#terminal(epoch, 'process exited without a clean stop'); });
    child.once('error', () => this.#terminal(epoch, 'process failed'));
    return this.request('start');
  }
  request(op) {
    if (this.closed || this.terminal || !this.child) throw new Error('Hot reload is not started');
    const id = ++this.id; const frame = JSON.stringify({ schema: 'semaprax.hot-reload-control.v1', id, op }) + '\n';
    if (Buffer.byteLength(frame) > MAX_FRAME) throw new Error('Hot reload request exceeds its bound');
    if (this.pending.size >= MAX_PENDING) throw new Error('Hot reload request queue is full'); this.pending.set(id, { op, epoch: this.epoch });
    const epoch = this.epoch;
    try { this.child.stdin.write(frame, error => { if (error) this.#writeFailed(epoch); }); } catch { this.#writeFailed(epoch); }
    return id;
  }
  markDirty() { if (!this.closed) { this.state.dirty = true; this.state.detail = 'Editor has unsaved source; active code remains the last checked revision'; this.emit('status', this.detail()); } }
  markSaved() { if (!this.closed) { this.state.dirty = false; this.state.sourceChanged = true; this.state.detail = 'Saved source changed; active code may be older than the saved file'; this.emit('status', this.detail()); } }
  detail() { return { ...this.state, history: [...this.state.history] }; }
  stop() { if (this.closed) return; const child=this.child, uncertain=this.state.event==='terminal_uncertainty' || this.state.event==='unknown' || [...this.pending.values()].some(row=>row.op==='activate'); this.stopping = true; if (!this.closed && child) { try { this.request('stop'); } catch {} } this.closed=true; this.pending.clear(); this.child=null; this.state={...this.state,event:uncertain?'unknown':'stopped',pending:null,detail:uncertain?'Activation acknowledgement was interrupted; active state is unknown':'Stopped'}; this.emit('status',this.detail()); if(uncertain) { this.emit('terminal',this.state.detail); this.stopTimer=setTimeout(()=>child?.kill(),STOP_GRACE_MS); this.stopTimer.unref?.(); } else child?.kill(); }
  // Frames are LF-delimited wire bytes. The bound applies to each frame (LF included), never to a read chunk, and
  // decoding is strict UTF-8 on the complete frame so a code point split across reads survives intact.
  #data(epoch, chunk) {
    let offset = 0;
    while (offset < chunk.length) {
      if (this.closed || epoch !== this.epoch) return;
      const lf = chunk.indexOf(0x0a, offset), end = lf < 0 ? chunk.length : lf;
      if (this.frameBytes + (end - offset) + 1 > MAX_FRAME) return this.#terminal(epoch, 'response exceeds its bound');
      if (end > offset) { this.chunks.push(chunk.subarray(offset, end)); this.frameBytes += end - offset; }
      if (lf < 0) return;
      const line = Buffer.concat(this.chunks, this.frameBytes); this.chunks = []; this.frameBytes = 0; offset = lf + 1;
      if (!this.#frame(epoch, line)) return;
    }
  }
  #frame(epoch, line) {
    try { const value = JSON.parse(new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(line)), request=this.pending.get(value?.id), rejected=value?.event==='rejected', terminalStop=value?.event==='stopped' && request?.op==='stop', status=!rejected && !terminalStop, candidate=value?.plan?.candidate_project_revision, migration=rejected && /source-Agent.*migration/i.test(String(value?.message || '')); if (!value || value.schema !== 'semaprax.hot-reload-control.v1' || !Number.isSafeInteger(value.id) || typeof value.event !== 'string' || !request || request.epoch !== epoch || (status && (!Number.isSafeInteger(value.generation) || value.generation < 0 || !this.#revision(value.active_project_revision) || (value.terminal_uncertainty !== undefined && typeof value.terminal_uncertainty !== 'boolean'))) || (candidate !== undefined && !this.#revision(candidate))) throw Error(); this.pending.delete(value.id); if(status) this.state.active=value.active_project_revision; this.state.pending=candidate||null; this.state.event=migration?'migration_required':value.event; this.state.detail=value.event==='candidate_rejected'?'Candidate rejected; previous active code remains available':value.event==='candidate_admitted'?'Checked candidate pending explicit activation':value.event==='waiting_safe_point'?'Waiting for a safe activation boundary':value.event==='activated'?'Active new checked code':value.event==='terminal_uncertainty'?'Activation state is unknown':migration?'Source-Agent migration required; editor supports interpreter sessions only':value.event==='rejected'?String(value.message || 'Request rejected'):value.event; this.state.history.push({event:this.state.event,active:this.state.active}); if(this.state.history.length>MAX_HISTORY)this.state.history.shift(); if(value.event==='terminal_uncertainty')this.terminal=true; this.emit('status',this.detail()); } catch { this.#terminal(epoch,'malformed, stale, or unsolicited hot reload response'); return false; } return true; }
  #writeFailed(epoch) {
    if (this.closed || this.stopping || epoch !== this.epoch) return;
    const dispatched = [...this.pending.values()].some(row => row.op === 'activate');
    this.#terminal(epoch, dispatched ? 'Activation acknowledgement was interrupted; active state is unknown' : 'Hot reload control channel failed');
  }
  #revision(value) { return typeof value === 'string' && /^sha256:[0-9a-f]{64}$/.test(value); }
  #clearStopTimer() { if(this.stopTimer) { clearTimeout(this.stopTimer); this.stopTimer=null; } }
  #terminal(epoch,message) { if(this.closed||epoch!==this.epoch)return; this.#clearStopTimer(); this.terminal=true; this.closed=true; this.pending.clear(); this.chunks=[]; this.frameBytes=0; this.child?.kill(); this.state={...this.state,event:'unknown',pending:null,detail:message}; this.emit('status',this.detail()); this.emit('terminal',message); }
}
module.exports = { HotReload, MAX_HISTORY };
