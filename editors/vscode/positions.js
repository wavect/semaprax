'use strict';
// Byte offsets to editor positions: the one mapper diagnostics, declaration
// navigation, and code lenses share. Nothing here touches VS Code, the
// filesystem or a process; callers supply the exact saved bytes the compiler
// read, so every decision below is testable with `node --test`.
//
// The compiler reports UTF-8 byte offsets (`start`/`end`) alongside a
// one-based line and a one-based Unicode-scalar column. VS Code positions are
// zero-based lines and zero-based UTF-16 code-unit characters, and a range may
// span lines. Those three unit systems agree only on ASCII, so a span is
// translated against the source rather than assumed.
const { TextDecoder } = require('node:util');

const LF = 0x0a, CR = 0x0d;
const decoder = new TextDecoder('utf-8', { fatal: true });
// Decodes a segment that starts after the line start. A U+FEFF there is an
// ordinary scalar; only `decoder` applies the line-start BOM rule.
const interior = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });
// Checkpoint stride, in bytes, for lines longer than one stride. A lookup
// decodes at most one stride plus one partial scalar from the nearest
// checkpoint at or before it, and each line is walked once, lazily, only as
// far as lookups reach, to lay its checkpoints. Retained memory is two numbers
// per stride of queried long lines (about 1/16 of their bytes); short lines
// retain nothing and decode their prefix directly, exactly as before.
const CHECKPOINT_BYTES = 256;

// The number of UTF-16 code units the byte range decodes to, or null when the
// range is not whole UTF-8 (a split code point, or a torn combining sequence's
// leading bytes). A decoded scalar above the basic plane counts as two.
function utf16Length(bytes) {
  try { return decoder.decode(bytes).length; } catch { return null; }
}

function segmentLength(bytes, atLineStart) {
  if (atLineStart) return utf16Length(bytes);
  try { return interior.decode(bytes).length; } catch { return null; }
}

// One saved source text, indexed by line so a byte offset becomes a position
// in time proportional to the line it lands on.
class SourceIndex {
  // `source` is the exact saved bytes; a string is encoded as UTF-8 first so a
  // caller that already read text does not have to.
  constructor(source) {
    this.bytes = Buffer.isBuffer(source) ? source : Buffer.from(String(source), 'utf8');
    this.starts = [0];
    for (let index = this.bytes.indexOf(LF); index >= 0; index = this.bytes.indexOf(LF, index + 1)) this.starts.push(index + 1);
    // Per-line conversion checkpoints of these exact bytes, never shared with
    // another snapshot: line -> { at: byte offsets, units: UTF-16 counts,
    // bad: end of the first segment that failed to decode, or -1 }. Every
    // checkpoint's prefix from the line start decoded as whole UTF-8, so a
    // checkpoint never stands past malformed bytes.
    this.checkpoints = new Map();
  }

  get lineCount() { return this.starts.length; }

  // Byte offset just past the last byte of the line's content: before its
  // `\n`, and before the `\r` of a `\r\n` pair, because VS Code line content
  // excludes the end-of-line sequence.
  contentEnd(line) {
    const next = line + 1 < this.starts.length ? this.starts[line + 1] - 1 : this.bytes.length;
    return next > this.starts[line] && this.bytes[next - 1] === CR ? next - 1 : next;
  }

  lineOf(offset) {
    let low = 0, high = this.starts.length - 1;
    while (low < high) {
      const middle = (low + high + 1) >> 1;
      if (this.starts[middle] <= offset) low = middle; else high = middle - 1;
    }
    return low;
  }

  // The zero-based { line, character } of one byte offset, or null when the
  // offset is not a safe non-negative integer inside the saved source, or does
  // not fall on a UTF-8 boundary. An offset inside an end-of-line sequence
  // resolves to the end of that line's content.
  position(offset) {
    if (!Number.isSafeInteger(offset) || offset < 0 || offset > this.bytes.length) return null;
    const line = this.lineOf(offset);
    const character = this.characterAt(line, Math.min(offset, this.contentEnd(line)));
    return character === null ? null : { line, character };
  }

  // UTF-16 units from the line start to byte `target` (within the line's
  // content), or null when that prefix is not whole UTF-8. Equal to decoding
  // the whole prefix: a valid prefix decomposes through every checkpoint, and
  // a prefix that crosses a failed segment cannot be valid.
  characterAt(line, target) {
    const start = this.starts[line];
    if (target - start <= CHECKPOINT_BYTES) return utf16Length(this.bytes.subarray(start, target));
    let state = this.checkpoints.get(line);
    if (!state) { state = { at: [start], units: [0], bad: -1 }; this.checkpoints.set(line, state); }
    const end = this.contentEnd(line);
    while (state.bad < 0) {
      const last = state.at[state.at.length - 1];
      if (last + CHECKPOINT_BYTES > target) break;
      // Cut before a lead byte where one is near, so a well-formed scalar is
      // never split across two segments.
      let cut = last + CHECKPOINT_BYTES;
      for (let skipped = 0; skipped < 3 && cut < end && (this.bytes[cut] & 0xc0) === 0x80; skipped++) cut++;
      const units = segmentLength(this.bytes.subarray(last, cut), last === start);
      if (units === null) { state.bad = cut; break; }
      state.at.push(cut); state.units.push(state.units[state.units.length - 1] + units);
    }
    if (state.bad >= 0 && target >= state.bad) return null;
    let low = 0, high = state.at.length - 1;
    while (low < high) {
      const middle = (low + high + 1) >> 1;
      if (state.at[middle] <= target) low = middle; else high = middle - 1;
    }
    const tail = segmentLength(this.bytes.subarray(state.at[low], target), state.at[low] === start);
    return tail === null ? null : state.units[low] + tail;
  }

  // The zero-based editor range of one byte span, or null when either endpoint
  // is unusable or the span runs backwards. An empty span is admitted and the
  // caller decides how wide to draw it.
  range(start, end) {
    const from = this.position(start);
    if (!from) return null;
    const to = this.position(end);
    if (!to) return null;
    if (to.line < from.line || (to.line === from.line && to.character < from.character)) return null;
    return { startLine: from.line, startColumn: from.character, endLine: to.line, endColumn: to.character };
  }
}

// The range a compiler location denotes. `index` is the `SourceIndex` of the
// exact saved source the compiler read, or null when it is unavailable; the
// byte span is preferred, and the fallback is the compiler's one-based line
// and column with the span's byte width, which is exact on ASCII and the best
// available guess otherwise. An empty or absent span is one character wide.
function locationRange(location, index) {
  if (!location) return { startLine: 0, startColumn: 0, endLine: 0, endColumn: 1 };
  const { line, column, start, end } = location;
  if (index && start !== null && start !== undefined && end !== null && end !== undefined) {
    const range = index.range(start, end);
    if (range) {
      if (range.startLine === range.endLine && range.startColumn === range.endColumn) return { ...range, endColumn: range.endColumn + 1 };
      return range;
    }
  }
  if (index && start !== null && start !== undefined && (end === null || end === undefined)) {
    const from = index.position(start);
    if (from) return { startLine: from.line, startColumn: from.character, endLine: from.line, endColumn: from.character + 1 };
  }
  const zeroLine = line - 1, zeroColumn = column - 1;
  const width = Number.isSafeInteger(start) && Number.isSafeInteger(end) && end > start ? end - start : 1;
  return { startLine: zeroLine, startColumn: zeroColumn, endLine: zeroLine, endColumn: zeroColumn + width };
}

SourceIndex.CHECKPOINT_BYTES = CHECKPOINT_BYTES;

module.exports = { SourceIndex, locationRange, utf16Length };
