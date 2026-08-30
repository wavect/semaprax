import {instantiate} from './generated/semaprax.js';

declare const bytes: Uint8Array;
const runtime = await instantiate(bytes);
const scalar = runtime.call('web.divide', 2n);
if (scalar.kind === 'success') {
  const value: bigint = scalar.value;
  void value;
} else if (scalar.kind === 'failure') {
  const domain: 'semaprax.arithmetic.v1' | 'semaprax.contract.v1' = scalar.domain;
  const code: number = scalar.code;
  void domain; void code;
} else {
  const cause: 'owners' | 'value_bytes' | 'live_bytes' | 'cumulative_bytes' | 'tokens' = scalar.cause;
  void cause;
}
const boolean = runtime.call('web.bool', true);
if (boolean.kind === 'success') {
  const value: boolean = boolean.value;
  void value;
}
runtime.call('__proto__');
runtime.call('');
runtime.call('web."</script>λ');
runtime.call('web.capacity', 1n);
