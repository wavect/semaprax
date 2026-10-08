import { asI64, parseJson, stringifyJson } from '../shared/json.ts';
import { normalizeRow, normalizeView } from '../shared/schema.ts';
import { createContext, useCallback, useContext, useEffect, useState } from 'react';
import { canRead, names, type EntityName, type Row } from '../shared/schema.ts';

export type Data = Record<EntityName, Row[]>;

export const Me = createContext<Row>({});
export const route = (name: EntityName) => `/${name.toLowerCase()}`;

export async function request(method: string, path: string, body?: unknown) {
  const res = await fetch(`/api${path}`, {
    method,
    headers: { 'content-type': 'application/json' },
    body: body === undefined ? undefined : stringifyJson(body),
  });
  return { ok: res.ok, json: res.status === 204 ? undefined : decodeResponse(path, parseJson(await res.text())) };
}

export function decodeResponse(path: string, body: any): any {
  const audit = (entry: any) => ({ ...entry, id: asI64(entry.id), member_id: asI64(entry.member_id),
    changes: Object.fromEntries(Object.entries(entry.changes).map(([field, pair]: [string, any]) => [field,
      pair.map((value: unknown) => value === null ? null : normalizeRow(entry.entity, { [field]: value })[field])])),
  });
  if (path === '/audit' || path.split('?')[0].endsWith('/history')) return Array.isArray(body) ? body.map(audit) : body;
  const slug = path.split('/')[1]?.split('?')[0];
  const name = slug === 'me' || slug === 'session' || slug === 'setup' ? 'Member'
    : names.find((n) => n.toLowerCase() === slug);
  if (!name || !body || body.error || body.errors || typeof body !== 'object') return body;
  const row = (value: Row) => normalizeView(name, value);
  return Array.isArray(body) ? body.map(row) : row(body);
}

/** The error messages of a failed response; empty on success. */
export const errorsOf = (res: { ok: boolean; json: any }): string[] => (res.ok ? [] : (res.json.errors ?? [res.json.error]));

export const send = async (method: string, path: string, body?: unknown) => errorsOf(await request(method, path, body));

export function useData() {
  const me = useContext(Me);
  const [data, setData] = useState<Data>();
  const reload = useCallback(
    () =>
      Promise.all(names.map((name) => (canRead(me, name) ? request('GET', route(name)).then((r) => r.json) : []))).then(
        (lists) => setData(Object.fromEntries(names.map((name, i) => [name, lists[i]])) as Data),
      ),
    [me],
  );
  useEffect(() => void reload(), [reload]);
  return [data, reload] as const;
}
