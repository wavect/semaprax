import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import { createServer, type IncomingMessage, type ServerResponse } from 'node:http';
import { dirname, join } from 'node:path';
import { entities, names, referrers, validate, withComputed, type EntityName, type Row } from '../shared/schema.ts';

type Db = { next: Record<string, number>; rows: Record<string, Row[]> };

const file = join(process.env.DATA_DIR ?? 'data', 'teamdesk.json');
const db: Db = existsSync(file)
  ? JSON.parse(readFileSync(file, 'utf8'))
  : {
      next: Object.fromEntries(names.map((name) => [name, 1])),
      rows: Object.fromEntries(names.map((name) => [name, []])),
    };

function save() {
  mkdirSync(dirname(file), { recursive: true });
  writeFileSync(`${file}.tmp`, JSON.stringify(db));
  renameSync(`${file}.tmp`, file);
}

function reply(res: ServerResponse, status: number, body?: unknown) {
  res.writeHead(status, { 'content-type': 'application/json' });
  res.end(body === undefined ? undefined : JSON.stringify(body));
}

async function readJson(req: IncomingMessage): Promise<unknown> {
  let text = '';
  for await (const chunk of req) text += chunk;
  try {
    return JSON.parse(text);
  } catch {
    return undefined;
  }
}

const exists = (name: EntityName, id: number) => db.rows[name].some((row) => row.id === id);

createServer(async (req, res) => {
  const [, root, slug, idText, extra] = new URL(req.url ?? '/', 'http://localhost').pathname.split('/');
  const name = names.find((n) => n.toLowerCase() === slug);
  if (root !== 'api' || !name || extra !== undefined) return reply(res, 404, { error: 'not found' });

  const rows = db.rows[name];
  const id = idText === undefined ? undefined : Number(idText);
  const row = rows.find((r) => r.id === id);

  if (id !== undefined && !row) return reply(res, 404, { error: `${name} ${idText} not found` });
  if (req.method === 'GET') return reply(res, 200, row ? withComputed(name, row) : rows.map((r) => withComputed(name, r)));

  if (req.method === 'DELETE' && row) {
    const blocker = referrers(name).find(([other, f]) => db.rows[other].some((r) => r[f.name] === id));
    if (blocker) return reply(res, 409, { error: `${name} ${id} is referenced by ${blocker[0]}.${blocker[1].name}` });
    rows.splice(rows.indexOf(row), 1);
    save();
    return reply(res, 204);
  }

  if ((req.method === 'POST' && !row) || (req.method === 'PUT' && row)) {
    const input = await readJson(req);
    const errors = validate(name, input, exists);
    if (errors.length) return reply(res, 400, { errors });
    const fields = Object.fromEntries(entities[name].fields.map((f) => [f.name, (input as Row)[f.name]]));
    const stored = { id: row ? row.id : db.next[name]++, ...fields };
    if (row) rows[rows.indexOf(row)] = stored;
    else rows.push(stored);
    save();
    return reply(res, row ? 200 : 201, withComputed(name, stored));
  }

  reply(res, 405, { error: 'method not allowed' });
}).listen(Number(process.env.PORT ?? 3001), '127.0.0.1');
