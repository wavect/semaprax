// Independent, reviewed facts from frozen SPEC.md; never read a candidate schema.
export const SPEC_SHA256 = "7658414ed2bbb53477a93e4a00f36269954c5bc45f3148fe72ddd02f203a50dc";
export const ENTITIES = {
  "Team": {
    "name": "string",
    "description": "string"
  },
  "Member": {
    "team_id": "ref:Team",
    "name": "string",
    "email": "string",
    "role": "Role",
    "active": "bool"
  },
  "Customer": {
    "company": "string",
    "contact": "string",
    "email": "string",
    "phone": "string",
    "tier": "Tier",
    "seats": "int"
  },
  "Contact": {
    "customer_id": "ref:Customer",
    "name": "string",
    "email": "string",
    "phone": "string"
  },
  "Project": {
    "team_id": "ref:Team",
    "customer_id": "ref:Customer",
    "name": "string",
    "code": "string",
    "status": "ProjectStatus",
    "budget": "float",
    "start_day": "int",
    "due_day": "int"
  },
  "Milestone": {
    "project_id": "ref:Project",
    "title": "string",
    "due_day": "int",
    "done": "bool"
  },
  "Sprint": {
    "project_id": "ref:Project",
    "name": "string",
    "start_day": "int",
    "end_day": "int"
  },
  "Task": {
    "project_id": "ref:Project",
    "milestone_id": "ref:Milestone",
    "sprint_id": "ref:Sprint",
    "member_id": "ref:Member",
    "title": "string",
    "details": "string",
    "priority": "Priority",
    "status": "TaskStatus",
    "estimate": "int",
    "spent": "int"
  },
  "Comment": {
    "task_id": "ref:Task",
    "member_id": "ref:Member",
    "body": "string"
  },
  "TimeEntry": {
    "task_id": "ref:Task",
    "member_id": "ref:Member",
    "hours": "int",
    "billable": "bool",
    "rate": "float"
  },
  "Ticket": {
    "customer_id": "ref:Customer",
    "member_id": "ref:Member",
    "subject": "string",
    "body": "string",
    "severity": "Severity",
    "state": "TicketState",
    "sla_hours": "int",
    "age_hours": "int"
  },
  "TicketReply": {
    "ticket_id": "ref:Ticket",
    "member_id": "ref:Member",
    "body": "string",
    "internal": "bool"
  },
  "Invoice": {
    "customer_id": "ref:Customer",
    "number": "string",
    "amount": "float",
    "paid": "bool"
  },
  "Payment": {
    "invoice_id": "ref:Invoice",
    "amount": "float",
    "day": "int"
  },
  "Vendor": {
    "name": "string",
    "email": "string"
  },
  "Expense": {
    "project_id": "ref:Project",
    "vendor_id": "ref:Vendor",
    "member_id": "ref:Member",
    "description": "string",
    "amount": "float",
    "day": "int",
    "state": "ExpenseState"
  },
  "Asset": {
    "team_id": "ref:Team",
    "name": "string",
    "serial": "string",
    "kind": "AssetKind",
    "cost": "float"
  },
  "Leave": {
    "member_id": "ref:Member",
    "kind": "LeaveKind",
    "state": "LeaveState",
    "start_day": "int",
    "end_day": "int"
  },
  "Document": {
    "project_id": "ref:Project",
    "title": "string",
    "url": "string"
  },
  "Release": {
    "project_id": "ref:Project",
    "version": "string",
    "day": "int",
    "state": "ReleaseState"
  }
};
export const ENUMS = {
  "Role": [
    "Admin",
    "Manager",
    "Agent",
    "Viewer"
  ],
  "Tier": [
    "Free",
    "Pro",
    "Enterprise"
  ],
  "ProjectStatus": [
    "Planned",
    "Active",
    "OnHold",
    "Done"
  ],
  "Priority": [
    "Low",
    "Medium",
    "High",
    "Urgent"
  ],
  "TaskStatus": [
    "Todo",
    "Doing",
    "Review",
    "Done"
  ],
  "Severity": [
    "Minor",
    "Major",
    "Critical"
  ],
  "TicketState": [
    "Open",
    "Pending",
    "Resolved",
    "Closed"
  ],
  "ExpenseState": [
    "Draft",
    "Submitted",
    "Approved",
    "Rejected",
    "Paid"
  ],
  "AssetKind": [
    "Laptop",
    "Phone",
    "Monitor",
    "Other"
  ],
  "LeaveKind": [
    "Vacation",
    "Sick",
    "Other"
  ],
  "LeaveState": [
    "Requested",
    "Approved",
    "Rejected"
  ],
  "ReleaseState": [
    "Planned",
    "Released"
  ]
};
export const KEYS = {
  "Member": [
    "email"
  ],
  "Customer": [
    "email"
  ],
  "Project": [
    "code"
  ],
  "Invoice": [
    "number"
  ],
  "Asset": [
    "serial"
  ],
  "Vendor": [
    "email"
  ],
  "Release": [
    "project_id",
    "version"
  ]
};
export const WORKFLOWS = {
  "Task": [
    "status",
    [
      [
        "Todo",
        "Doing"
      ],
      [
        "Doing",
        "Todo"
      ],
      [
        "Doing",
        "Review"
      ],
      [
        "Review",
        "Doing"
      ],
      [
        "Review",
        "Done"
      ]
    ]
  ],
  "Ticket": [
    "state",
    [
      [
        "Open",
        "Pending"
      ],
      [
        "Pending",
        "Open"
      ],
      [
        "Open",
        "Resolved"
      ],
      [
        "Pending",
        "Resolved"
      ],
      [
        "Resolved",
        "Open"
      ],
      [
        "Resolved",
        "Closed"
      ]
    ]
  ],
  "Expense": [
    "state",
    [
      [
        "Draft",
        "Submitted"
      ],
      [
        "Submitted",
        "Approved"
      ],
      [
        "Submitted",
        "Rejected"
      ],
      [
        "Rejected",
        "Draft"
      ],
      [
        "Approved",
        "Paid"
      ]
    ]
  ],
  "Leave": [
    "state",
    [
      [
        "Requested",
        "Approved"
      ],
      [
        "Requested",
        "Rejected"
      ]
    ]
  ],
  "Release": [
    "state",
    [
      [
        "Planned",
        "Released"
      ]
    ]
  ]
};

export const PASSWORD = 'benchmark-passphrase-197';
export const AGENT_WRITES = ['Task', 'Comment', 'TimeEntry', 'Ticket', 'TicketReply', 'Leave', 'Expense'];
export const COMPUTED = {
  Project: ['duration', 'late', 'tasks', 'open_tasks', 'spent', 'expenses', 'over_budget'],
  Task: ['remaining', 'overrun', 'weight', 'open'],
  Ticket: ['breached', 'escalation', 'open'], TimeEntry: ['amount'], Customer: ['large', 'open_tickets', 'billed'],
  Sprint: ['length'], Leave: ['days'], Team: ['members'], Invoice: ['received', 'balance'], Member: ['hours'],
};
export const integer = value => {
  const text = typeof value === 'number' && Number.isSafeInteger(value) ? String(value) : value;
  if (typeof text !== 'string' || !/^-?\d+$/.test(text)) throw new Error(`not an exact integer: ${value}`);
  const number = BigInt(text);
  if (number < -(1n << 63n) || number >= (1n << 63n)) throw new Error(`i64 out of range: ${text}`);
  return number;
};
export const equalId = (a, b) => integer(a) === integer(b);
export const canRead = (role, entity, row, me) => role === 'Admin' || role === 'Manager' ||
  (!['Invoice', 'Payment'].includes(entity) && (entity !== 'Expense' || role === 'Agent' && equalId(row.member_id, me)));
export const canWrite = (role, entity, row, me) => role === 'Admin' || role === 'Manager' && !['Team', 'Member'].includes(entity) ||
  role === 'Agent' && AGENT_WRITES.includes(entity) && equalId(row.member_id, me) &&
  (entity !== 'Expense' || ['Draft', 'Submitted'].includes(row.state)) && (entity !== 'Leave' || row.state === 'Requested');
export function seed(entity, refs, ordinal) {
  const row = {};
  for (const [field, type] of Object.entries(ENTITIES[entity])) {
    row[field] = type.startsWith('ref:') ? refs[type.slice(4)] : ENUMS[type]?.[0] ??
      ({ string: `Text ${ordinal} 😀`, int: 1, float: 2.5, bool: true })[type];
  }
  Object.assign(row, {
    ...({Team:{name:`Team ${ordinal}`}, Member:{name:`Member ${ordinal}`,email:`member-${ordinal}@example.test`,password:PASSWORD},
    Customer:{company:`Customer ${ordinal}`,email:`customer-${ordinal}@example.test`,seats:120,tier:'Enterprise'},
    Contact:{name:`Contact ${ordinal}`,email:`contact-${ordinal}@example.test`},
    Project:{name:`Project ${ordinal}`,code:`P${ordinal}`,budget:10,start_day:50,due_day:75},
    Milestone:{title:`Milestone ${ordinal}`,due_day:70}, Sprint:{name:`Sprint ${ordinal}`,start_day:50,end_day:60},
    Task:{title:`Task ${ordinal}`,priority:'High',estimate:10,spent:2}, Comment:{body:'Comment, "quoted"\n😀'},
    TimeEntry:{hours:3,billable:true,rate:7.5}, Ticket:{subject:`Ticket ${ordinal}`,severity:'Critical',sla_hours:10,age_hours:20},
    TicketReply:{body:'Reply text'}, Invoice:{number:`INV-${ordinal}`,amount:100,paid:false}, Payment:{amount:30,day:51},
    Vendor:{name:`Vendor ${ordinal}`,email:`vendor-${ordinal}@example.test`}, Expense:{description:`Expense ${ordinal}`,amount:15,day:52},
    Asset:{name:`Asset ${ordinal}`,serial:`SERIAL-${ordinal}`,cost:42.5}, Leave:{start_day:50,end_day:54},
    Document:{title:`Document ${ordinal}`,url:`https://example.test/${ordinal}`}, Release:{version:`v${ordinal}`,day:60}})[entity],
  });
  return row;
}
export function computed(entity, row, all) {
  const children = (name, field) => all[name].filter(child => equalId(child[field], row.id));
  const sum = (rows, field) => rows.reduce((value, child) => value + Number(child[field]), 0);
  switch (entity) {
    case 'Project': {const tasks=children('Task','project_id'),expenses=sum(children('Expense','project_id'),'amount');return {duration:String(integer(row.due_day)-integer(row.start_day)),late:row.status!=='Done'&&integer(row.due_day)<100n,tasks:String(tasks.length),open_tasks:String(tasks.filter(t=>t.status!=='Done').length),spent:String(sum(tasks,'spent')),expenses,over_budget:expenses>Number(row.budget)};}
    case 'Task': return {remaining:String(row.status==='Done'?0n:integer(row.estimate)-integer(row.spent)),overrun:integer(row.spent)>integer(row.estimate),weight:String(integer(row.estimate)*BigInt({Low:1,Medium:2,High:3,Urgent:5}[row.priority])),open:row.status!=='Done'};
    case 'Ticket': {const open=['Open','Pending'].includes(row.state),breached=open&&integer(row.age_hours)>integer(row.sla_hours);return {open,breached,escalation:breached?(row.severity==='Critical'?'page':'watch'):'ok'};}
    case 'TimeEntry': return {amount:row.billable?Number(row.hours)*Number(row.rate):0};
    case 'Customer': return {large:integer(row.seats)>=100n||row.tier==='Enterprise',open_tickets:String(children('Ticket','customer_id').filter(t=>['Open','Pending'].includes(t.state)).length),billed:sum(children('Invoice','customer_id'),'amount')};
    case 'Sprint': return {length:String(integer(row.end_day)-integer(row.start_day))};
    case 'Leave': return {days:String(integer(row.end_day)-integer(row.start_day)+1n)};
    case 'Team': return {members:String(children('Member','team_id').length)};
    case 'Invoice': {const received=sum(children('Payment','invoice_id'),'amount');return {received,balance:Number(row.amount)-received};}
    case 'Member': return {hours:String(sum(children('TimeEntry','member_id'),'hours'))};
    default: return {};
  }
}
// Every individual rule has an independent invalid witness; paired endpoints
// prevent a predicate with only one side of a range from qualifying.
const textRange = (entity, field, min, max) => [
  [entity,`${field}.short`,{[field]:'x'.repeat(min-1)},1],
  [entity,`${field}.long`,{[field]:'x'.repeat(max+1)},1],
  [entity,`${field}.utf8`,{[field]:'😀'.repeat(Math.floor(max/4)+1)},1],
];
export const INVALID = [
  ...textRange('Team','name',2,60),...textRange('Member','name',2,80),['Member','email',{email:'invalid'},1],
  ...textRange('Customer','company',2,120),['Customer','email',{email:'invalid'},1],['Customer','seats',{seats:0},1],
  ...textRange('Contact','name',2,80),['Contact','email',{email:'invalid'},1],
  ...textRange('Project','name',2,80),...textRange('Project','code',2,12),['Project','budget',{budget:-1},1],['Project','start',{start_day:-1},1],['Project','due',{due_day:49},1],
  ...textRange('Milestone','title',2,120),['Milestone','due',{due_day:-1},1],
  ...textRange('Sprint','name',2,60),['Sprint','end-before',{end_day:49},1],['Sprint','length',{end_day:81},1],
  ...textRange('Task','title',3,120),['Task','estimate-low',{estimate:-1,spent:0},1],['Task','estimate-high',{estimate:1001},1],['Task','spent-low',{spent:-1},1],['Task','spent-over',{spent:31},1],
  ...textRange('Comment','body',1,4000),['TimeEntry','hours-low',{hours:0},1],['TimeEntry','hours-high',{hours:25},1],['TimeEntry','rate',{rate:-1},1],
  ...textRange('Ticket','subject',3,160),['Ticket','sla',{sla_hours:0},1],['Ticket','age',{age_hours:-1},1],
  ...textRange('TicketReply','body',1,4000),['Invoice','number',{number:'wrong'},1],['Invoice','amount',{amount:-1},1],['Payment','amount',{amount:0},1],['Payment','day',{day:-1},1],
  ...textRange('Vendor','name',2,80),['Vendor','email',{email:'invalid'},1],...textRange('Expense','description',3,200),['Expense','amount',{amount:0},1],['Expense','day',{day:-1},1],
  ...textRange('Asset','name',2,80),...textRange('Asset','serial',4,40),['Asset','cost',{cost:-1},1],['Leave','end-before',{end_day:49},1],['Leave','length',{end_day:81},1],
  ...textRange('Document','title',2,120),['Document','url',{url:'http://example.test'},1],...textRange('Release','version',1,20),['Release','day',{day:-1},1],
  ['Member','all-errors',{name:'x',email:'invalid'},2],['Project','all-errors',{name:'x',code:'x',budget:-1,start_day:-1,due_day:-2},5],
  ['Customer','all-errors',{company:'x',email:'invalid',seats:0},3],['Task','all-errors',{title:'x',estimate:-1,spent:-1},4],
];
export const COVERAGE = ['entities.fields.types', 'validation.every-rule', 'validation.utf8', 'validation.all-errors', 'references', 'keys', 'workflows', 'computed', 'rollups', 'roles', 'own-other-rows', 'auth', 'password.slow-salted-hash', 'audit', 'csv', 'durability', 'nonreused-id', 'i64.exact', 'browser.navigation', 'browser.dashboard', 'browser.list', 'browser.forms', 'browser.references', 'browser.workflow', 'browser.permission', 'browser.audit', 'browser.csv', 'browser.delete'];
