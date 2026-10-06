import { createContext, useCallback, useContext, useEffect, useState } from 'react';
import { canRead, names, type EntityName, type Row } from '../shared/schema.ts';

export type Data = Record<EntityName, Row[]>;

export const Me = createContext<Row>({});
export const route = (name: EntityName) => `/${name.toLowerCase()}`;

export async function request(method: string, path: string, body?: unknown) {
  const res = await fetch(`/api${path}`, {
    method,
    headers: { 'content-type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  return { ok: res.ok, json: res.status === 204 ? undefined : await res.json() };
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
