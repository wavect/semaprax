import { useCallback, useEffect, useState } from 'react';
import { names, type EntityName, type Row } from '../shared/schema.ts';

export type Data = Record<EntityName, Row[]>;

export const route = (name: EntityName) => `/${name.toLowerCase()}`;
const endpoint = (name: EntityName) => `/api${route(name)}`;

async function load(): Promise<Data> {
  const lists = await Promise.all(names.map((name) => fetch(endpoint(name)).then((res) => res.json())));
  return Object.fromEntries(names.map((name, i) => [name, lists[i]])) as Data;
}

export function useData() {
  const [data, setData] = useState<Data>();
  const reload = useCallback(() => load().then(setData), []);
  useEffect(() => void reload(), [reload]);
  return [data, reload] as const;
}

/** Resolves to the list of error messages; empty means success. */
export async function send(method: 'POST' | 'PUT' | 'DELETE', name: EntityName, id?: number, body?: Row) {
  const res = await fetch(id === undefined ? endpoint(name) : `${endpoint(name)}/${id}`, {
    method,
    headers: { 'content-type': 'application/json' },
    body: body && JSON.stringify(body),
  });
  if (res.ok) return [];
  const json = await res.json();
  return (json.errors ?? [json.error]) as string[];
}
