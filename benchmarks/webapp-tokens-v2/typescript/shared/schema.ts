import { asFloat, asI64, I64_MAX, I64_MIN, isI64 } from './json.ts';

export type Row = Record<string, any>;
export type Db = Record<string, Row[]>;
export type Field = { name: string; type: 'string' | 'int' | 'float' | 'bool' | 'secret' | 'enum' | 'ref'; of: string };
type Rule = [message: string, holds: (row: any, old?: any) => boolean, fields: string[]];
type Computed = Record<string, (row: any, db: Db) => string | number | bigint | boolean>;

export const enums: Record<string, string[]> = {
  Role: ['Admin', 'Manager', 'Agent', 'Viewer'],
  Tier: ['Free', 'Pro', 'Enterprise'],
  ProjectStatus: ['Planned', 'Active', 'OnHold', 'Done'],
  Priority: ['Low', 'Medium', 'High', 'Urgent'],
  TaskStatus: ['Todo', 'Doing', 'Review', 'Done'],
  Severity: ['Minor', 'Major', 'Critical'],
  TicketState: ['Open', 'Pending', 'Resolved', 'Closed'],
  ExpenseState: ['Draft', 'Submitted', 'Approved', 'Rejected', 'Paid'],
  AssetKind: ['Laptop', 'Phone', 'Monitor', 'Other'],
  LeaveKind: ['Vacation', 'Sick', 'Other'],
  LeaveState: ['Requested', 'Approved', 'Rejected'],
  ReleaseState: ['Planned', 'Released'],
};

const primitives = ['string', 'int', 'float', 'bool', 'secret'];
const fieldsOf = (specs: string): Field[] =>
  specs.split(' ').map((spec) => {
    const [name, of] = spec.split(':');
    return { name, of, type: primitives.includes(of) ? (of as Field['type']) : of in enums ? 'enum' : 'ref' };
  });

/** Workflow transitions as 'From>To ...'; the first origin is the initial state. */
const movesOf = (text: string) => {
  const moves: Record<string, string[]> = {};
  for (const pair of text.split(' ')) {
    const [from, to] = pair.split('>');
    (moves[from] ??= []).push(to);
  }
  return moves;
};

type Def = {
  fields: string;
  rules?: Rule[];
  computed?: Computed;
  keys?: string[];
  flow?: [field: string, transitions: string];
};
const entity = ({ fields, rules = [], computed = {}, keys = [], flow }: Def) => ({
  fields: fieldsOf(fields),
  rules,
  computed,
  keys: keys.map((key) => key.split('+')),
  flow: flow && { field: flow[0], moves: movesOf(flow[1]) },
});

const bytes = (text: string) => new TextEncoder().encode(text).length;
const range = (key: string, min: number, max = Infinity, unit = '', measure = (v: any): number => v): Rule => [
  `${key} must be ${max === Infinity ? `>= ${min}` : `in ${min}..${max}`}${unit}`,
  (r) => measure(r[key]) >= min && measure(r[key]) <= max,
  [key],
];
const sized = (key: string, min: number, max: number) => range(key, min, max, ' bytes', bytes);
const hasAt: (key: string) => Rule = (key) => [`${key} must contain @`, (r) => r[key].includes('@'), [key]];
const prefix = (key: string, start: string): Rule => [`${key} must start with ${start}`, (r) => r[key].startsWith(start), [key]];
const within = (max: number): Rule[] => [
  ['end_day must be >= start_day', (r) => r.end_day >= r.start_day, ['end_day', 'start_day']],
  [`end_day - start_day must be <= ${max}`, (r) => r.end_day - r.start_day <= max, ['end_day', 'start_day']],
];

const refs = (db: Db, name: string, key: string, id: bigint) => db[name].filter((r) => r[key] === id);
const sum = (rows: Row[], key: string, integer = false): number | bigint =>
  rows.reduce((total, r) => integer ? checked((total as bigint) + r[key]) : (total as number) + r[key], integer ? 0n : 0);
const checked = (value: bigint): bigint => {
  if (value < I64_MIN || value > I64_MAX) throw new RangeError('i64 overflow');
  return value;
};
const isOpenTicket = (t: Row) => ['Open', 'Pending'].includes(t.state);
const breached = (t: Row) => isOpenTicket(t) && t.age_hours > t.sla_hours;
const expenses = (p: Row, db: Db) => (sum(refs(db, 'Expense', 'project_id', p.id), 'amount') as number);
const received = (i: Row, db: Db) => (sum(refs(db, 'Payment', 'invoice_id', i.id), 'amount') as number);

export const entities = {
  Team: entity({
    fields: 'name:string description:string',
    rules: [sized('name', 2, 60)],
    computed: { members: (t, db) => BigInt(refs(db, 'Member', 'team_id', t.id).length) },
  }),
  Member: entity({
    fields: 'team_id:Team name:string email:string role:Role active:bool password:secret',
    rules: [
      sized('name', 2, 80),
      hasAt('email'),
      ['password must have at least 8 bytes', (m, old) => (old && m.password === '') || bytes(m.password) >= 8, ['password']],
    ],
    computed: { hours: (m, db) => sum(refs(db, 'TimeEntry', 'member_id', m.id), 'hours', true) },
    keys: ['email'],
  }),
  Customer: entity({
    fields: 'company:string contact:string email:string phone:string tier:Tier seats:int',
    rules: [sized('company', 2, 120), hasAt('email'), range('seats', 1)],
    computed: {
      large: (c) => c.seats >= 100 || c.tier === 'Enterprise',
      open_tickets: (c, db) => BigInt(refs(db, 'Ticket', 'customer_id', c.id).filter(isOpenTicket).length),
      billed: (c, db) => sum(refs(db, 'Invoice', 'customer_id', c.id), 'amount'),
    },
    keys: ['email'],
  }),
  Contact: entity({
    fields: 'customer_id:Customer name:string email:string phone:string',
    rules: [sized('name', 2, 80), hasAt('email')],
  }),
  Project: entity({
    fields: 'team_id:Team customer_id:Customer name:string code:string status:ProjectStatus budget:float start_day:int due_day:int',
    rules: [
      sized('name', 2, 80),
      sized('code', 2, 12),
      range('budget', 0),
      range('start_day', 0),
      ['due_day must be >= start_day', (p) => p.due_day >= p.start_day, ['due_day', 'start_day']],
    ],
    computed: {
      duration: (p) => p.due_day - p.start_day,
      late: (p) => p.status !== 'Done' && p.due_day < 100,
      tasks: (p, db) => BigInt(refs(db, 'Task', 'project_id', p.id).length),
      open_tasks: (p, db) => BigInt(refs(db, 'Task', 'project_id', p.id).filter((t) => t.status !== 'Done').length),
      spent: (p, db) => sum(refs(db, 'Task', 'project_id', p.id), 'spent', true),
      expenses,
      over_budget: (p, db) => expenses(p, db) > p.budget,
    },
    keys: ['code'],
  }),
  Milestone: entity({
    fields: 'project_id:Project title:string due_day:int done:bool',
    rules: [sized('title', 2, 120), range('due_day', 0)],
  }),
  Sprint: entity({
    fields: 'project_id:Project name:string start_day:int end_day:int',
    rules: [sized('name', 2, 60), ...within(30)],
    computed: { length: (s) => s.end_day - s.start_day },
  }),
  Task: entity({
    fields: 'project_id:Project milestone_id:Milestone sprint_id:Sprint member_id:Member title:string details:string priority:Priority status:TaskStatus estimate:int spent:int',
    rules: [
      sized('title', 3, 120),
      range('estimate', 0, 1000),
      range('spent', 0),
      ['spent must be <= estimate * 3', (t) => t.spent <= t.estimate * 3n, ['spent', 'estimate']],
    ],
    computed: {
      remaining: (t) => (t.status === 'Done' ? 0n : t.estimate - t.spent),
      overrun: (t) => t.spent > t.estimate,
      weight: (t) => t.estimate * { Low: 1n, Medium: 2n, High: 3n, Urgent: 5n }[t.priority as string]!,
      open: (t) => t.status !== 'Done',
    },
    flow: ['status', 'Todo>Doing Doing>Todo Doing>Review Review>Doing Review>Done'],
  }),
  Comment: entity({ fields: 'task_id:Task member_id:Member body:string', rules: [sized('body', 1, 4000)] }),
  TimeEntry: entity({
    fields: 'task_id:Task member_id:Member hours:int billable:bool rate:float',
    rules: [range('hours', 1, 24), range('rate', 0)],
    computed: { amount: (e) => (e.billable ? Number(e.hours) * e.rate : 0) },
  }),
  Ticket: entity({
    fields: 'customer_id:Customer member_id:Member subject:string body:string severity:Severity state:TicketState sla_hours:int age_hours:int',
    rules: [sized('subject', 3, 160), range('sla_hours', 1), range('age_hours', 0)],
    computed: {
      breached,
      escalation: (t) => (!breached(t) ? 'ok' : t.severity === 'Critical' ? 'page' : 'watch'),
      open: isOpenTicket,
    },
    flow: ['state', 'Open>Pending Pending>Open Open>Resolved Pending>Resolved Resolved>Open Resolved>Closed'],
  }),
  TicketReply: entity({
    fields: 'ticket_id:Ticket member_id:Member body:string internal:bool',
    rules: [sized('body', 1, 4000)],
  }),
  Invoice: entity({
    fields: 'customer_id:Customer number:string amount:float paid:bool',
    rules: [prefix('number', 'INV-'), range('amount', 0)],
    computed: { received, balance: (i, db) => i.amount - received(i, db) },
    keys: ['number'],
  }),
  Payment: entity({
    fields: 'invoice_id:Invoice amount:float day:int',
    rules: [['amount must be > 0', (p) => p.amount > 0, ['amount']], range('day', 0)],
  }),
  Vendor: entity({
    fields: 'name:string email:string',
    rules: [sized('name', 2, 80), hasAt('email')],
    keys: ['email'],
  }),
  Expense: entity({
    fields: 'project_id:Project vendor_id:Vendor member_id:Member description:string amount:float day:int state:ExpenseState',
    rules: [sized('description', 3, 200), ['amount must be > 0', (e) => e.amount > 0, ['amount']], range('day', 0)],
    flow: ['state', 'Draft>Submitted Submitted>Approved Submitted>Rejected Rejected>Draft Approved>Paid'],
  }),
  Asset: entity({
    fields: 'team_id:Team name:string serial:string kind:AssetKind cost:float',
    rules: [sized('name', 2, 80), sized('serial', 4, 40), range('cost', 0)],
    keys: ['serial'],
  }),
  Leave: entity({
    fields: 'member_id:Member kind:LeaveKind state:LeaveState start_day:int end_day:int',
    rules: within(30),
    computed: { days: (l) => (l.end_day as bigint) - (l.start_day as bigint) + 1n },
    flow: ['state', 'Requested>Approved Requested>Rejected'],
  }),
  Document: entity({
    fields: 'project_id:Project title:string url:string',
    rules: [sized('title', 2, 120), prefix('url', 'https://')],
  }),
  Release: entity({
    fields: 'project_id:Project version:string day:int state:ReleaseState',
    rules: [sized('version', 1, 20), range('day', 0)],
    keys: ['project_id+version'],
    flow: ['state', 'Planned>Released'],
  }),
};
export type EntityName = keyof typeof entities;
export const names = Object.keys(entities) as EntityName[];

const valid: Record<Field['type'], (v: unknown, of: string) => boolean> = {
  string: (v) => typeof v === 'string',
  secret: (v) => typeof v === 'string',
  int: (v) => isI64(v),
  float: (v) => typeof v === 'number' && Number.isFinite(v),
  bool: (v) => typeof v === 'boolean',
  enum: (v, of) => enums[of].includes(v as string),
  ref: (v) => isI64(v) && v > 0n,
};

/** Normalize parsed numeric tokens according to the entity schema. */
export function normalizeRow(name: EntityName, input: unknown): any {
  if (typeof input !== 'object' || input === null || Array.isArray(input)) return input;
  const row = { ...input } as Row;
  for (const field of entities[name].fields) {
    const value = row[field.name];
    const decoded = field.type === 'int' || field.type === 'ref' ? asI64(value)
      : field.type === 'float' ? asFloat(value) : undefined;
    if (decoded !== undefined) row[field.name] = decoded;
  }
  if (Object.hasOwn(row, 'id')) row.id = asI64(row.id) ?? row.id;
  return row;
}

/** Decode API computed numbers with the same integer/float split as their formulas. */
export function normalizeView(name: EntityName, input: Row): Row {
  const row = normalizeRow(name, input);
  const integer: Partial<Record<EntityName, string[]>> = {
    Team: ['members'], Member: ['hours'], Customer: ['open_tickets'],
    Project: ['duration', 'tasks', 'open_tasks', 'spent'],
    Sprint: ['length'], Task: ['remaining', 'weight'], Leave: ['days'],
  };
  const float: Partial<Record<EntityName, string[]>> = {
    Customer: ['billed'], Project: ['expenses'], Invoice: ['received', 'balance'], TimeEntry: ['amount'],
  };
  for (const key of integer[name] ?? []) if (Object.hasOwn(row, key)) row[key] = asI64(row[key]) ?? row[key];
  for (const key of float[name] ?? []) if (Object.hasOwn(row, key)) row[key] = asFloat(row[key]) ?? row[key];
  return row;
}

/** Every violated rule; `old` is the stored row when updating. */
export function validate(name: EntityName, input: unknown, db: Db, old?: Row): string[] {
  if (typeof input !== 'object' || input === null || Array.isArray(input)) return ['body must be a JSON object'];
  const row = input as Row;
  const { fields, rules, keys, flow } = entities[name];
  const mistyped = fields.filter((f) => !valid[f.type](row[f.name], f.of));
  const bad = new Set(mistyped.map((field) => field.name));
  const errors = [
    ...mistyped.map((f) => `${f.name} must be of type ${f.of}`),
    ...fields
      .filter((f) => f.type === 'ref' && !bad.has(f.name) && !db[f.of].some((r) => r.id === row[f.name]))
      .map((f) => `${f.name} must reference an existing ${f.of}`),
    ...rules.filter(([, holds, dependencies]) => !dependencies.some((field) => bad.has(field)) && !holds(row, old)).map(([message]) => message),
    ...keys
      .filter((key) => !key.some((field) => bad.has(field)) && db[name].some((r) => r.id !== old?.id && key.every((k) => r[k] === row[k])))
      .map((key) => `${key.join(', ')} must be unique`),
  ];
  if (flow && !bad.has(flow.field)) {
    const [initial] = Object.keys(flow.moves);
    const [from, to] = [old ? old[flow.field] : initial, row[flow.field]];
    if (from !== to && !(old && flow.moves[from]?.includes(to)))
      errors.push(old ? `${flow.field} cannot move from ${from} to ${to}` : `${flow.field} must start as ${initial}`);
  }
  return errors;
}

/** The options a workflow field offers, else every enumeration value. */
export function choices(name: EntityName, f: Field, current?: string, me?: Row, prospective?: Row): string[] {
  const flow = entities[name].flow;
  if (flow?.field !== f.name) return enums[f.of];
  const permitted = current === undefined ? [Object.keys(flow.moves)[0]] : [current, ...(flow.moves[current] ?? [])];
  return me && prospective ? permitted.filter((next) => canWrite(me, name, { ...prospective, [f.name]: next })) : permitted;
}

export const withComputed = (name: EntityName, row: Row, db: Db): Row => ({
  ...row,
  ...Object.fromEntries(Object.entries(entities[name].computed).map(([key, compute]) => {
    try {
      const value = compute(row, db);
      return [key, typeof value === 'bigint' ? checked(value)
        : typeof value === 'number' && !Number.isFinite(value) ? { error: 'non_finite' } : value];
    } catch (error) {
      return [key, { error: error instanceof RangeError ? 'overflow' : 'computation failed' }];
    }
  })),
});

/** Stored fields: everything but the write-only password. */
export const stored = (name: EntityName) => entities[name].fields.filter((f) => f.type !== 'secret');

export const columns = (name: EntityName): Field[] => [
  { name: 'id', type: 'int', of: 'int' },
  ...stored(name),
  ...Object.keys(entities[name].computed).map((key): Field => ({ name: key, type: 'string', of: 'string' })),
];

export const labelOf = (name: EntityName, row: Row): string => {
  const f = entities[name].fields.find((f) => f.type === 'string');
  return f ? String(row[f.name]).slice(0, 40) : `#${row.id}`;
};

export const referrers = (target: EntityName) =>
  names.flatMap((name) =>
    entities[name].fields.filter((f) => f.type === 'ref' && f.of === target).map((f) => [name, f] as const),
  );

/** Case-insensitive search over string fields plus exact enumeration filters. */
export const matching = (name: EntityName, rows: Row[], search = '', filters: Record<string, string> = {}) => {
  const { fields } = entities[name];
  const needle = search.toLowerCase();
  return rows.filter(
    (r) =>
      fields.every((f) => !filters[f.name] || String(r[f.name]) === filters[f.name]) &&
      (!needle || fields.some((f) => f.type === 'string' && r[f.name].toLowerCase().includes(needle))),
  );
};

export function toCsv(name: EntityName, rows: Row[]): string {
  const cols = columns(name).map((c) => c.name);
  const cell = (v: unknown) => (/[",\r\n]/.test(String(v)) ? `"${String(v).replaceAll('"', '""')}"` : String(v));
  return [cols, ...rows.map((r) => cols.map((c) => r[c]))].map((line) => line.map(cell).join(',')).join('\r\n') + '\r\n';
}

const agentWrites = ['Task', 'Comment', 'TimeEntry', 'Ticket', 'TicketReply', 'Leave', 'Expense'];
const agentStates: Record<string, string[]> = { Expense: ['Draft', 'Submitted'], Leave: ['Requested'] };
const hidden: Record<string, string[]> = {
  Agent: ['Invoice', 'Payment'],
  Viewer: ['Invoice', 'Payment', 'Expense'],
};

/** With a row, applies row-level rules; without, only the entity-level rule. */
export const canRead = (m: Row, name: EntityName, row?: Row) =>
  !hidden[m.role]?.includes(name) && !(m.role === 'Agent' && name === 'Expense' && row && row.member_id !== m.id);

/** Every given row (stored and new) must be one the member may write. */
export const canWrite = (m: Row, name: EntityName, ...rows: Row[]) =>
  m.role === 'Admin' ||
  (m.role === 'Manager' && name !== 'Team' && name !== 'Member') ||
  (m.role === 'Agent' &&
    agentWrites.includes(name) &&
    rows.every((r) => r.member_id === m.id && (agentStates[name] ?? [r.state]).includes(r.state)));
