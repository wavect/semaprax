/** JSON numbers retain their source until the schema chooses i64 or IEEE f64. */
export class JsonNumber {
  readonly source: string;
  constructor(source: string) {
    if (!/^-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?$/.test(source)) throw new SyntaxError('invalid JSON number');
    this.source = source;
  }
}
export const I64_MIN = -(1n << 63n);
export const I64_MAX = (1n << 63n) - 1n;
export const isI64 = (value: unknown): value is bigint =>
  typeof value === 'bigint' && value >= I64_MIN && value <= I64_MAX;

/** Exact mathematical integer decoding, including decimal/exponent JSON forms. */
export function asI64(value: unknown): bigint | undefined {
  if (typeof value === 'bigint') return isI64(value) ? value : undefined;
  if (typeof value === 'number') return Number.isSafeInteger(value) ? BigInt(value) : undefined;
  if (!(value instanceof JsonNumber)) return undefined;
  const parts = /^(-?)(\d+)(?:\.(\d+))?(?:[eE]([+-]?\d+))?$/.exec(value.source);
  if (!parts) return undefined;
  let digits = (parts[2] + (parts[3] ?? '')).replace(/^0+/, '');
  if (!digits) return 0n;
  const scale = BigInt(parts[4] ?? '0') - BigInt(parts[3]?.length ?? 0);
  if (scale >= 0n) {
    if (BigInt(digits.length) + scale > 19n) return undefined;
    digits += '0'.repeat(Number(scale));
  } else {
    const places = -scale;
    if (places > BigInt(digits.length)) return undefined;
    const split = digits.length - Number(places);
    if (!/^0*$/.test(digits.slice(split))) return undefined;
    digits = digits.slice(0, split);
  }
  const integer = BigInt((parts[1] || '') + digits);
  return isI64(integer) ? integer : undefined;
}

export function asFloat(value: unknown): number | undefined {
  const result = value instanceof JsonNumber ? Number(value.source) : value;
  return typeof result === 'number' && Number.isFinite(result) ? result : undefined;
}

/** Text-backed form edits keep invalid/partial text so validation can report it. */
export function integerEdit(text: string): bigint | string {
  if (!/^-?\d+$/.test(text)) return text;
  const value = BigInt(text);
  return isI64(value) ? value : text;
}
export function routeId(text: string | undefined): bigint | undefined {
  if (text === undefined || !/^[1-9]\d*$/.test(text)) return undefined;
  return asI64(new JsonNumber(text));
}

/** Works in every supported browser; no dependency on JSON reviver source support. */
export function parseJson(text: string): any {
  let position = 0;
  const fail = (): never => { throw new SyntaxError(`invalid JSON at ${position}`); };
  const space = () => { while (/[\x20\t\r\n]/.test(text[position] ?? '') && position < text.length) position++; };
  const string = (): string => {
    const start = position++;
    while (position < text.length) {
      const character = text[position++];
      if (character === '"') return JSON.parse(text.slice(start, position));
      if (character === '\\') position++;
    }
    return fail();
  };
  const value = (): any => {
    space();
    const character = text[position];
    if (character === '"') return string();
    if (character === '[') {
      position++; space();
      const items: any[] = [];
      if (text[position] === ']') { position++; return items; }
      for (;;) {
        items.push(value()); space();
        if (text[position] === ']') { position++; return items; }
        if (text[position++] !== ',') return fail();
      }
    }
    if (character === '{') {
      position++; space();
      const object: Record<string, any> = {};
      if (text[position] === '}') { position++; return object; }
      for (;;) {
        space();
        if (text[position] !== '"') return fail();
        const key = string(); space();
        if (text[position++] !== ':') return fail();
        Object.defineProperty(object, key, { value: value(), enumerable: true, configurable: true, writable: true });
        space();
        if (text[position] === '}') { position++; return object; }
        if (text[position++] !== ',') return fail();
      }
    }
    for (const [token, result] of [['true', true], ['false', false], ['null', null]] as const) {
      if (text.startsWith(token, position)) { position += token.length; return result; }
    }
    const number = /^-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?/.exec(text.slice(position));
    if (!number) return fail();
    position += number[0].length;
    return new JsonNumber(number[0]);
  };
  const result = value(); space();
  if (position !== text.length) fail();
  return result;
}

/** Numeric JSON tokens, never quoted bigint strings or a lossy Number conversion. */
export function stringifyJson(value: unknown): string {
  const active = new Set<object>();
  const encode = (item: any): string => {
    if (typeof item === 'bigint') return item.toString();
    if (item instanceof JsonNumber) return item.source;
    if (item === null || typeof item === 'boolean' || typeof item === 'string') return JSON.stringify(item);
    if (typeof item === 'number') {
      if (!Number.isFinite(item)) throw new TypeError('non-finite JSON number');
      return Object.is(item, -0) ? '-0' : JSON.stringify(item);
    }
    if (typeof item !== 'object') throw new TypeError('unsupported JSON value');
    if (active.has(item)) throw new TypeError('cyclic JSON value');
    active.add(item);
    try {
      if (Array.isArray(item)) return '[' + Array.from(item, (v) => v === undefined ? 'null' : encode(v)).join(',') + ']';
      return '{' + Object.entries(item).filter(([, v]) => v !== undefined)
        .map(([key, v]) => JSON.stringify(key) + ':' + encode(v)).join(',') + '}';
    } finally { active.delete(item); }
  };
  return encode(value);
}
