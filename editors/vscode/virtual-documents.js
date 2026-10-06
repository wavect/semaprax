'use strict';
// Content stores for the read-only `semaprax-review` and
// `semaprax-token-report` views. Nothing here touches VS Code; extension.js
// reports the lifecycle events, so every decision below is testable with
// `node --test`.
//
// A view's content is admitted under per-item, aggregate and count limits and
// pinned while the view is being opened: `setTextDocumentLanguage` emits a
// close and an open for the same URI during creation, and that close must not
// retire it. After the open settles, a close retires the content only when no
// open document still shows the URI; retiring releases exactly its bytes and
// count once. A failed open rolls its insertion back. Released URIs resolve to
// nothing, so reopening an expired view fails explicitly instead of binding to
// current content.
const REVIEW_LIMITS = Object.freeze({
  maxItemBytes: 16 * 1024 * 1024, maxTotalBytes: 32 * 1024 * 1024, maxCount: 64,
  exhausted: 'Virtual document budget reached; restart session'
});
// Token reports are bounded like review views; one rendered report is at most
// the report contract's own 16 MiB input bound.
const TOKEN_REPORT_LIMITS = Object.freeze({
  maxItemBytes: 16 * 1024 * 1024, maxTotalBytes: 32 * 1024 * 1024, maxCount: 64,
  exhausted: 'Token report view budget reached; close some token report views first'
});

class ContentStore {
  constructor({ maxItemBytes, maxTotalBytes, maxCount, exhausted, onRelease = () => {} }) {
    this.limits = { maxItemBytes, maxTotalBytes, maxCount };
    this.exhausted = exhausted;
    this.onRelease = onRelease;
    this.entries = new Map(); // uri -> { text, bytes, pins }
    this.total = 0;
  }
  get size() { return this.entries.size; }
  get bytes() { return this.total; }
  has(uri) { return this.entries.has(uri); }
  get(uri) { return this.entries.get(uri)?.text; }
  keys() { return [...this.entries.keys()]; }
  list() { return [...this.entries].map(([uri, entry]) => [uri, entry.text]); }
  // Admit one new view's content, pinned until `unpin` or `rollback`.
  admit(uri, text) {
    const bytes = Buffer.byteLength(text);
    if (this.entries.has(uri) || bytes > this.limits.maxItemBytes || this.total + bytes > this.limits.maxTotalBytes || this.entries.size >= this.limits.maxCount) throw new Error(this.exhausted);
    this.entries.set(uri, { text, bytes, pins: 1 });
    this.total += bytes;
    return uri;
  }
  // Replace a live view's content (invalidation). An expired view stays gone.
  set(uri, text) {
    const entry = this.entries.get(uri);
    if (!entry) return false;
    const bytes = Buffer.byteLength(text);
    this.total += bytes - entry.bytes;
    entry.text = text; entry.bytes = bytes;
    return true;
  }
  unpin(uri) { const entry = this.entries.get(uri); if (entry && entry.pins > 0) entry.pins--; }
  rollback(uri) { this.release(uri); }
  // A document with this URI was closed. `stillOpen` is whether any open
  // document still shows it.
  closed(uri, stillOpen) {
    const entry = this.entries.get(uri);
    if (entry && entry.pins === 0 && !stillOpen) this.release(uri);
  }
  release(uri) {
    const entry = this.entries.get(uri);
    if (!entry) return;
    this.entries.delete(uri);
    this.total -= entry.bytes;
    this.onRelease(uri);
  }
  clear() { for (const uri of this.keys()) this.release(uri); }
}

module.exports = { ContentStore, REVIEW_LIMITS, TOKEN_REPORT_LIMITS };
