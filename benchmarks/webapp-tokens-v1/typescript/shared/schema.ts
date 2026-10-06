export type Row = Record<string, any>;
export type Field = { name: string; type: 'string' | 'int' | 'float' | 'bool' | 'enum' | 'ref'; of: string };
type Rule = [message: string, holds: (row: any) => boolean];
type Computed = Record<string, (row: any) => string | number | boolean>;

export const enums: Record<string, string[]> = {
  Role: ['Admin', 'Manager', 'Agent', 'Viewer'],
  Tier: ['Free', 'Pro', 'Enterprise'],
  ProjectStatus: ['Planned', 'Active', 'OnHold', 'Done'],
  Priority: ['Low', 'Medium', 'High', 'Urgent'],
  TaskStatus: ['Todo', 'Doing', 'Review', 'Done'],
  Severity: ['Minor', 'Major', 'Critical'],
  TicketState: ['Open', 'Pending', 'Resolved', 'Closed'],
};

const primitives = ['string', 'int', 'float', 'bool'];
const field = (spec: string): Field => {
  const [name, of] = spec.split(':');
  const type = primitives.includes(of) ? (of as Field['type']) : of in enums ? 'enum' : 'ref';
  return { name, type, of };
};
const entity = (specs: string[], rules: Rule[], computed: Computed = {}) => ({
  fields: specs.map(field),
  rules,
  computed,
});

const bytes = (text: string) => new TextEncoder().encode(text).length;
const between = (n: number, min: number, max = Infinity) => n >= min && n <= max;
const sized = (key: string, min: number, max: number): Rule => [
  `${key} must have ${min}..${max} bytes`,
  (r) => between(bytes(r[key]), min, max),
];
const atLeast = (key: string, min: number): Rule => [`${key} must be >= ${min}`, (r) => r[key] >= min];
const hasAt = (key: string): Rule => [`${key} must contain @`, (r) => r[key].includes('@')];
const breached = (t: Row) => ['Open', 'Pending'].includes(t.state) && t.age_hours > t.sla_hours;

export const entities = {
  Team: entity(['name:string', 'description:string'], [sized('name', 2, 60)]),
  Member: entity(
    ['team_id:Team', 'name:string', 'email:string', 'role:Role', 'active:bool'],
    [sized('name', 2, 80), hasAt('email')],
  ),
  Customer: entity(
    ['company:string', 'contact:string', 'email:string', 'phone:string', 'tier:Tier', 'seats:int'],
    [sized('company', 2, 120), hasAt('email'), atLeast('seats', 1)],
    { large: (c) => c.seats >= 100 || c.tier === 'Enterprise' },
  ),
  Project: entity(
    ['team_id:Team', 'name:string', 'code:string', 'status:ProjectStatus', 'budget:float', 'start_day:int', 'due_day:int'],
    [
      sized('name', 2, 80),
      sized('code', 2, 12),
      atLeast('budget', 0),
      atLeast('start_day', 0),
      ['due_day must be >= start_day', (p) => p.due_day >= p.start_day],
    ],
    {
      duration: (p) => p.due_day - p.start_day,
      late: (p) => p.status !== 'Done' && p.due_day < 100,
    },
  ),
  Milestone: entity(
    ['project_id:Project', 'title:string', 'due_day:int', 'done:bool'],
    [sized('title', 2, 120), atLeast('due_day', 0)],
  ),
  Task: entity(
    [
      'project_id:Project', 'milestone_id:Milestone', 'member_id:Member', 'title:string', 'details:string',
      'priority:Priority', 'status:TaskStatus', 'estimate:int', 'spent:int',
    ],
    [
      sized('title', 3, 120),
      ['estimate must be in 0..1000', (t) => between(t.estimate, 0, 1000)],
      atLeast('spent', 0),
      ['spent must be <= estimate * 3', (t) => t.spent <= t.estimate * 3],
    ],
    {
      remaining: (t) => (t.status === 'Done' ? 0 : t.estimate - t.spent),
      overrun: (t) => t.spent > t.estimate,
      weight: (t) => t.estimate * { Low: 1, Medium: 2, High: 3, Urgent: 5 }[t.priority as string]!,
    },
  ),
  Ticket: entity(
    ['customer_id:Customer', 'member_id:Member', 'subject:string', 'body:string', 'severity:Severity', 'state:TicketState', 'sla_hours:int', 'age_hours:int'],
    [sized('subject', 3, 160), atLeast('sla_hours', 1), atLeast('age_hours', 0)],
    {
      breached,
      escalation: (t) => (!breached(t) ? 'ok' : t.severity === 'Critical' ? 'page' : 'watch'),
    },
  ),
  Comment: entity(
    ['task_id:Task', 'member_id:Member', 'body:string'],
    [['body must have 1..4000 bytes', (c) => between(bytes(c.body), 1, 4000)]],
  ),
  TimeEntry: entity(
    ['task_id:Task', 'member_id:Member', 'hours:int', 'billable:bool', 'rate:float'],
    [['hours must be in 1..24', (e) => between(e.hours, 1, 24)], atLeast('rate', 0)],
    { amount: (e) => (e.billable ? e.hours * e.rate : 0) },
  ),
  Invoice: entity(
    ['customer_id:Customer', 'number:string', 'amount:float', 'paid:bool'],
    [['number must start with INV-', (i) => i.number.startsWith('INV-')], atLeast('amount', 0)],
    { status: (i) => (i.paid ? 'paid' : 'due') },
  ),
};
export type EntityName = keyof typeof entities;
export const names = Object.keys(entities) as EntityName[];

const valid: Record<Field['type'], (v: unknown, of: string) => boolean> = {
  string: (v) => typeof v === 'string',
  int: (v) => Number.isSafeInteger(v),
  float: (v) => typeof v === 'number' && Number.isFinite(v),
  bool: (v) => typeof v === 'boolean',
  enum: (v, of) => enums[of].includes(v as string),
  ref: (v) => Number.isSafeInteger(v) && (v as number) > 0,
};

export function validate(
  name: EntityName,
  input: unknown,
  exists: (entity: EntityName, id: number) => boolean = () => true,
): string[] {
  if (typeof input !== 'object' || input === null || Array.isArray(input)) return ['body must be a JSON object'];
  const row = input as Row;
  const { fields, rules } = entities[name];
  const mistyped = fields.filter((f) => !valid[f.type](row[f.name], f.of));
  if (mistyped.length) return mistyped.map((f) => `${f.name} must be of type ${f.of}`);
  return [
    ...fields
      .filter((f) => f.type === 'ref' && !exists(f.of as EntityName, row[f.name]))
      .map((f) => `${f.name} must reference an existing ${f.of}`),
    ...rules.filter(([, holds]) => !holds(row)).map(([message]) => message),
  ];
}

export const withComputed = (name: EntityName, row: Row): Row => ({
  ...row,
  ...Object.fromEntries(Object.entries(entities[name].computed).map(([key, compute]) => [key, compute(row)])),
});

export const columns = (name: EntityName): Field[] => [
  { name: 'id', type: 'int', of: 'int' },
  ...entities[name].fields,
  ...Object.keys(entities[name].computed).map((key): Field => ({ name: key, type: 'string', of: 'string' })),
];

export const labelOf = (name: EntityName, row: Row): string =>
  String(row[entities[name].fields.find((f) => f.type === 'string')!.name]);

export const referrers = (target: EntityName) =>
  names.flatMap((name) =>
    entities[name].fields.filter((f) => f.type === 'ref' && f.of === target).map((f) => [name, f] as const),
  );
