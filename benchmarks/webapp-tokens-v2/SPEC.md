# TeamDesk Enterprise: the large benchmark application

The frozen functional specification every implementation in
`benchmarks/webapp-tokens-v2/` builds. An implementation is complete only
when it provides every requirement below. Look and feel may differ; behavior
may not.

TeamDesk Enterprise is a full-stack project, help-desk, finance, and HR
application with 20 related entities, sign-in with role-based and row-level
permissions, workflows, unique keys, rollups, an audit history, CSV export, a
JSON REST API, file persistence, and a single-page browser UI.

## Entities

Types: `string` (UTF-8), `int` (signed 64-bit), `float` (64-bit IEEE),
`bool`, and the enumerations below. An `x_id` field references a row of
entity `X` by numeric id; `member_id` and the account always mean Member.

Enumerations:

- Role: Admin, Manager, Agent, Viewer
- Tier: Free, Pro, Enterprise
- ProjectStatus: Planned, Active, OnHold, Done
- Priority: Low, Medium, High, Urgent
- TaskStatus: Todo, Doing, Review, Done
- Severity: Minor, Major, Critical
- TicketState: Open, Pending, Resolved, Closed
- ExpenseState: Draft, Submitted, Approved, Rejected, Paid
- AssetKind: Laptop, Phone, Monitor, Other
- LeaveKind: Vacation, Sick, Other
- LeaveState: Requested, Approved, Rejected
- ReleaseState: Planned, Released

| Entity | Fields |
| --- | --- |
| Team | name: string, description: string |
| Member | team_id, name: string, email: string, role: Role, active: bool |
| Customer | company: string, contact: string, email: string, phone: string, tier: Tier, seats: int |
| Contact | customer_id, name: string, email: string, phone: string |
| Project | team_id, customer_id, name: string, code: string, status: ProjectStatus, budget: float, start_day: int, due_day: int |
| Milestone | project_id, title: string, due_day: int, done: bool |
| Sprint | project_id, name: string, start_day: int, end_day: int |
| Task | project_id, milestone_id, sprint_id, member_id, title: string, details: string, priority: Priority, status: TaskStatus, estimate: int, spent: int |
| Comment | task_id, member_id, body: string |
| TimeEntry | task_id, member_id, hours: int, billable: bool, rate: float |
| Ticket | customer_id, member_id, subject: string, body: string, severity: Severity, state: TicketState, sla_hours: int, age_hours: int |
| TicketReply | ticket_id, member_id, body: string, internal: bool |
| Invoice | customer_id, number: string, amount: float, paid: bool |
| Payment | invoice_id, amount: float, day: int |
| Vendor | name: string, email: string |
| Expense | project_id, vendor_id, member_id, description: string, amount: float, day: int, state: ExpenseState |
| Asset | team_id, name: string, serial: string, kind: AssetKind, cost: float |
| Leave | member_id, kind: LeaveKind, state: LeaveState, start_day: int, end_day: int |
| Document | project_id, title: string, url: string |
| Release | project_id, version: string, day: int, state: ReleaseState |

Every row also has a server-assigned positive integer `id`, unique within its
entity and never reused.

## Validation rules

A create or update that violates a rule is rejected in the browser before
submission and by the server; every violated rule is reported.

1. Team: name 2..60 bytes.
2. Member: name 2..80 bytes; email contains `@`.
3. Customer: company 2..120 bytes; email contains `@`; seats >= 1.
4. Contact: name 2..80 bytes; email contains `@`.
5. Project: name 2..80 bytes; code 2..12 bytes; budget >= 0; start_day >= 0;
   due_day >= start_day.
6. Milestone: title 2..120 bytes; due_day >= 0.
7. Sprint: name 2..60 bytes; end_day >= start_day; end_day - start_day <= 30.
8. Task: title 3..120 bytes; estimate 0..1000; spent >= 0;
   spent <= estimate * 3.
9. Comment: body not empty, at most 4000 bytes.
10. TimeEntry: hours 1..24; rate >= 0.
11. Ticket: subject 3..160 bytes; sla_hours >= 1; age_hours >= 0.
12. TicketReply: body not empty, at most 4000 bytes.
13. Invoice: number starts with `INV-`; amount >= 0.
14. Payment: amount > 0; day >= 0.
15. Vendor: name 2..80 bytes; email contains `@`.
16. Expense: description 3..200 bytes; amount > 0; day >= 0.
17. Asset: name 2..80 bytes; serial 4..40 bytes; cost >= 0.
18. Leave: end_day >= start_day; end_day - start_day <= 30.
19. Document: title 2..120 bytes; url starts with `https://`.
20. Release: version 1..20 bytes; day >= 0.

Every reference field must name an existing row.

## Unique keys

Two rows of one entity may not share a key; a violation is rejected like a
rule. Keys: Member.email, Customer.email, Project.code, Invoice.number,
Asset.serial, Vendor.email, and Release (project_id, version) together.

## Workflows

A row is created in the first state of its workflow field. An update may
change that field only along an allowed transition.

- Task.status: Todo→Doing, Doing→Todo, Doing→Review, Review→Doing,
  Review→Done.
- Ticket.state: Open→Pending, Pending→Open, Open→Resolved, Pending→Resolved,
  Resolved→Open, Resolved→Closed.
- Expense.state: Draft→Submitted, Submitted→Approved, Submitted→Rejected,
  Rejected→Draft, Approved→Paid.
- Leave.state: Requested→Approved, Requested→Rejected.
- Release.state: Planned→Released.

## Computed fields and rollups

Read-only, shown in list and detail views and returned by the API.

1. Project `duration` = due_day - start_day.
2. Project `late` = status is not Done and due_day < 100.
3. Task `remaining` = 0 when status is Done, else estimate - spent.
4. Task `overrun` = spent > estimate.
5. Task `weight` = estimate × 1/2/3/5 for Low/Medium/High/Urgent.
6. Task `open` = status is not Done.
7. Ticket `breached` = state is Open or Pending and age_hours > sla_hours.
8. Ticket `escalation` = `"page"` when severity is Critical and breached,
   `"watch"` when only breached, else `"ok"`.
9. Ticket `open` = state is Open or Pending.
10. TimeEntry `amount` = hours × rate when billable, else 0.0.
11. Customer `large` = seats >= 100 or tier is Enterprise.
12. Sprint `length` = end_day - start_day.
13. Leave `days` = end_day - start_day + 1.

Rollups aggregate the rows that reference a row:

14. Team `members` = number of its Members.
15. Project `tasks` = number of its Tasks; `open_tasks` = number of its open
    Tasks; `spent` = sum of its Tasks' spent; `expenses` = sum of its
    Expenses' amount; `over_budget` = expenses > budget.
16. Customer `open_tickets` = number of its open Tickets; `billed` = sum of
    its Invoices' amount.
17. Invoice `received` = sum of its Payments' amount; `balance` = amount -
    received.
18. Member `hours` = sum of their TimeEntries' hours.

## Accounts and permissions

1. Members are the accounts. A member signs in with email and password and
   signs out. An inactive member cannot sign in. Passwords are stored only
   as a salted slow hash and are never returned by the API.
2. A member's password is set when the member is created or edited, by
   someone allowed to write that Member row.
3. Every API call and page requires a signed-in member, except sign-in.
   First-run setup: the implementation documents how the first Admin is
   created.
4. Permissions, enforced by the server and reflected in the UI (hidden
   actions):
   - Admin reads and writes everything.
   - Manager reads everything and writes everything except Team and Member.
   - Agent reads everything except Invoice and Payment, and reads only
     their own Expense rows; writes Task, Comment, TimeEntry, Ticket,
     TicketReply, Leave, and Expense rows only where member_id is
     themselves; and may only have an Expense in Draft or Submitted and a
     Leave in Requested (so approval is for Managers and Admins).
   - Viewer reads everything except Invoice, Payment, and Expense, and
     writes nothing.
5. A row the member may not read is absent from lists and is 404 when
   requested directly; a forbidden write is 403.

## Audit history

Every create, update, and delete is recorded with time, member, entity, id,
and the changed fields with old and new values. A row's detail page shows its
history. Admins can list the whole audit log.

## CSV export

Every list page can download the rows the member may read, after the current
search and filters, as CSV with a header row.

## API

For every entity, under `/api/<entity>`: list, get one, create (201), replace
(200), delete (204); 400 with every violated rule, type, reference, key, or
workflow error; 401 when not signed in; 403 for a forbidden write; 404; 409
when deleting a referenced row. Plus sign-in, sign-out, current member, and
the audit log.

## Persistence

All rows, accounts, and the audit log survive a server restart through files
in one data directory.

## Browser UI

As in v1 (navigation, dashboard with counts per entity and per enumeration
value, list pages with search, sort, filters per enumeration field and
25-row pagination, detail pages with reference links and back-references,
forms with typed inputs and reference selects, delete with confirmation and
409 reporting), plus: a sign-in page, the signed-in member and sign-out in
the header, only permitted actions shown, workflow fields offering only
allowed next states, a row's audit history on its detail page, and CSV
export on list pages.

## Out of scope

Password reset, email, multi-factor sign-in, styling beyond a usable default,
internationalisation, and third-party services.
