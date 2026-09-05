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

// The number of UTF-16 code units the byte range decodes to, or null when the
// range is not whole UTF-8 (a split code point, or a torn combining sequence's
// leading bytes). A decoded scalar above the basic plane counts as two.
function utf16Length(bytes) {
  try { return decoder.decode(bytes).length; } catch { return null; }
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
    const end = this.contentEnd(line);
    const character = utf16Length(this.bytes.subarray(this.starts[line], Math.min(offset, end)));
    return character === null ? null : { line, character };
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

module.exports = { SourceIndex, locationRange, utf16Length };
