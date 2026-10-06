import { randomBytes, scryptSync, timingSafeEqual } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import { createServer, type IncomingMessage, type ServerResponse } from 'node:http';
import { dirname, join } from 'node:path';
import {
  canRead, canWrite, entities, matching, names, referrers, stored, toCsv, validate, withComputed,
  type EntityName, type Row,
} from '../shared/schema.ts';

type Audit = { time: string; member_id: number; entity: EntityName; id: number; action: string; changes: Row };
type Db = { next: Record<string, number>; rows: Record<string, Row[]>; secrets: Record<number, string>; audit: Audit[] };

const file = join(process.env.DATA_DIR ?? 'data', 'teamdesk.json');
const db: Db = existsSync(file)
  ? JSON.parse(readFileSync(file, 'utf8'))
  : {
      next: Object.fromEntries(names.map((name) => [name, 1])),
      rows: Object.fromEntries(names.map((name) => [name, []])),
      secrets: {},
      audit: [],
    };
const sessions = new Map<string, number>();

const hash = (password: string, salt = randomBytes(16)) =>
  `${salt.toString('hex')}:${scryptSync(password, salt, 64).toString('hex')}`;
const verify = (password: string, secret: string) => {
  const [salt, key] = secret.split(':');
  return timingSafeEqual(scryptSync(password, Buffer.from(salt, 'hex'), 64), Buffer.from(key, 'hex'));
};
const unknown = hash(randomBytes(8).toString('hex'));

function reply(res: ServerResponse, status: number, body?: unknown, headers: Record<string, string> = {}) {
  res.writeHead(status, { 'content-type': 'application/json', ...headers });
  res.end(body === undefined ? undefined : JSON.stringify(body));
}

async function readJson(req: IncomingMessage): Promise<any> {
  let text = '';
  for await (const chunk of req) if ((text += chunk).length > 1e6) return undefined;
  try {
    return JSON.parse(text);
  } catch {
    return undefined;
  }
}

/** Stores `input` over `before` (create, update) or removes `before` (no input); records the audit entry. */
function commit(actor: number, name: EntityName, before?: Row, input?: Row): Row {
  const rows = db.rows[name];
  const after: Row | undefined = input && {
    id: before?.id ?? db.next[name]++,
    ...Object.fromEntries(stored(name).map((f) => [f.name, input[f.name]])),
  };
  const changes = Object.fromEntries(
    stored(name)
      .map((f) => [f.name, [before?.[f.name] ?? null, after?.[f.name] ?? null]] as const)
      .filter(([, [was, now]]) => was !== now),
  );
  if (before && after) rows[rows.indexOf(before)] = after;
  else if (after) rows.push(after);
  else rows.splice(rows.indexOf(before!), 1);
  const id = (after ?? before)!.id;
  if (name === 'Member') {
    if (input?.password) db.secrets[id] = hash(input.password);
    if (!after) delete db.secrets[id];
  }
  db.audit.push({ time: new Date().toISOString(), member_id: actor, entity: name, id, action: !after ? 'delete' : before ? 'update' : 'create', changes });
  mkdirSync(dirname(file), { recursive: true });
  writeFileSync(`${file}.tmp`, JSON.stringify(db));
  renameSync(`${file}.tmp`, file);
  return after ?? before!;
}

async function handle(req: IncomingMessage, res: ServerResponse) {
  const url = new URL(req.url ?? '/', 'http://localhost');
  const [, root, slug, idText, extra] = url.pathname.split('/');
  if (root !== 'api') return reply(res, 404, { error: 'not found' });

  if (slug === 'setup') {
    const needed = db.rows.Member.length === 0;
    if (req.method === 'GET') return reply(res, 200, { needed });
    if (req.method !== 'POST' || !needed) return reply(res, 403, { error: 'setup is already done' });
    const team_id = db.next.Team;
    const input = { ...(await readJson(req)), team_id, role: 'Admin', active: true };
    const errors = validate('Member', input, { ...db.rows, Team: [{ id: team_id }] });
    if (errors.length) return reply(res, 400, { errors });
    commit(0, 'Team', undefined, { name: 'Administration', description: '' });
    return reply(res, 201, commit(0, 'Member', undefined, input));
  }

  if (slug === 'session' && req.method === 'POST') {
    const { email, password } = (await readJson(req)) ?? {};
    const member = db.rows.Member.find((m) => m.email === email);
    const known = typeof password === 'string' && verify(password, db.secrets[member?.id ?? 0] ?? unknown);
    if (!member || !member.active || !known) return reply(res, 401, { error: 'invalid email or password' });
    const token = randomBytes(32).toString('base64url');
    sessions.set(token, member.id);
    return reply(res, 200, member, { 'set-cookie': `sid=${token}; HttpOnly; SameSite=Strict; Path=/` });
  }

  const token = /(?:^|; )sid=([^;]+)/.exec(req.headers.cookie ?? '')?.[1] ?? '';
  const me = db.rows.Member.find((m) => m.id === sessions.get(token) && m.active);
  if (!me) return reply(res, 401, { error: 'sign in required' });
  if (slug === 'session' && req.method === 'DELETE') {
    sessions.delete(token);
    return reply(res, 204, undefined, { 'set-cookie': 'sid=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0' });
  }
  if (slug === 'me') return reply(res, 200, me);
  if (slug === 'audit') return me.role === 'Admin' ? reply(res, 200, db.audit) : reply(res, 403, { error: 'forbidden' });

  const name = names.find((n) => n.toLowerCase() === slug);
  if (!name || (extra !== undefined && extra !== 'history')) return reply(res, 404, { error: 'not found' });
  const rows = db.rows[name];
  const id = idText === undefined ? undefined : Number(idText);
  const row = rows.find((r) => r.id === id);
  if (id !== undefined && !(row && canRead(me, name, row))) return reply(res, 404, { error: `${name} ${idText} not found` });

  if (req.method === 'GET') {
    if (row) return reply(res, 200, extra ? db.audit.filter((a) => a.entity === name && a.id === id) : withComputed(name, row, db.rows));
    const { q = '', format, ...filters } = Object.fromEntries(url.searchParams);
    const list = matching(name, rows.filter((r) => canRead(me, name, r)), q, filters).map((r) => withComputed(name, r, db.rows));
    if (format !== 'csv') return reply(res, 200, list);
    res.writeHead(200, { 'content-type': 'text/csv', 'content-disposition': `attachment; filename="${slug}.csv"` });
    return res.end(toCsv(name, list));
  }

  if (req.method === 'DELETE' && row) {
    if (!canWrite(me, name, row)) return reply(res, 403, { error: 'forbidden' });
    const blocker = referrers(name).find(([other, f]) => db.rows[other].some((r) => r[f.name] === id));
    if (blocker) return reply(res, 409, { error: `${name} ${id} is referenced by ${blocker[0]}.${blocker[1].name}` });
    commit(me.id, name, row);
    return reply(res, 204);
  }

  if ((req.method === 'POST' && id === undefined) || (req.method === 'PUT' && row)) {
    const input = await readJson(req);
    if (!canWrite(me, name, ...(row ? [row] : []), input ?? {})) return reply(res, 403, { error: 'forbidden' });
    const errors = validate(name, input, db.rows, row);
    if (errors.length) return reply(res, 400, { errors });
    return reply(res, row ? 200 : 201, withComputed(name, commit(me.id, name, row, input), db.rows));
  }

  reply(res, 405, { error: 'method not allowed' });
}

createServer((req, res) => handle(req, res).catch(() => reply(res, 500, { error: 'internal error' }))).listen(
  Number(process.env.PORT ?? 3001),
  '127.0.0.1',
);
