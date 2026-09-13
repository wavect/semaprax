export function validate(kind: number, version: number, payloadLen: number): number {
  if (kind !== 7) return 1;
  if (version !== 1) return 2;
  if (payloadLen < 1 || payloadLen > 64) return 3;
  return 0;
}
