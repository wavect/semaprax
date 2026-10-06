# TeamDesk: the benchmark application

This is the frozen functional specification that every implementation in
`benchmarks/webapp-tokens-v1/` builds. An implementation is complete only when
it provides every numbered requirement below. Implementations may differ in
look and feel, but not in behavior.

TeamDesk is a full-stack project management and help-desk application with
ten related entities, a JSON REST API, file persistence, and a single-page
browser UI.

## Entities

Types: `string` (UTF-8 text), `int` (signed 64-bit integer), `float` (64-bit
IEEE double), `bool`, and the enumerations below. `x_id` fields reference a
row of entity `X` by its numeric id.

Enumerations:

- `Role`: Admin, Manager, Agent, Viewer
- `Tier`: Free, Pro, Enterprise
- `ProjectStatus`: Planned, Active, OnHold, Done
- `Priority`: Low, Medium, High, Urgent
- `TaskStatus`: Todo, Doing, Review, Done
- `Severity`: Minor, Major, Critical
- `TicketState`: Open, Pending, Resolved, Closed

| Entity | Fields |
| --- | --- |
| Team | name: string, description: string |
| Member | team_id, name: string, email: string, role: Role, active: bool |
| Customer | company: string, contact: string, email: string, phone: string, tier: Tier, seats: int |
| Project | team_id, name: string, code: string, status: ProjectStatus, budget: float, start_day: int, due_day: int |
| Milestone | project_id, title: string, due_day: int, done: bool |
| Task | project_id, milestone_id, member_id, title: string, details: string, priority: Priority, status: TaskStatus, estimate: int, spent: int |
| Ticket | customer_id, member_id, subject: string, body: string, severity: Severity, state: TicketState, sla_hours: int, age_hours: int |
| Comment | task_id, member_id, body: string |
| TimeEntry | task_id, member_id, hours: int, billable: bool, rate: float |
| Invoice | customer_id, number: string, amount: float, paid: bool |

Every row also has a server-assigned positive integer `id`, unique within its
entity and never reused.

## Validation rules

A create or update that violates any rule is rejected, both in the browser
before submission and by the server, and every violated rule is reported next
to the form.

1. Team: name has 2..60 bytes.
2. Member: name has 2..80 bytes; email contains `@`.
3. Customer: company has 2..120 bytes; email contains `@`; seats >= 1.
4. Project: name has 2..80 bytes; code has 2..12 bytes; budget >= 0;
   start_day >= 0; due_day >= start_day.
5. Milestone: title has 2..120 bytes; due_day >= 0.
6. Task: title has 3..120 bytes; estimate in 0..1000; spent >= 0;
   spent <= estimate * 3.
7. Ticket: subject has 3..160 bytes; sla_hours >= 1; age_hours >= 0.
8. Comment: body is not empty and has at most 4000 bytes.
9. TimeEntry: hours in 1..24; rate >= 0.
10. Invoice: number starts with `INV-`; amount >= 0.

Every reference field must name an existing row of the referenced entity.

## Business logic (computed fields)

Computed fields are derived from a row's own fields, are read-only, and are
shown in list and detail views.

1. Project `duration` = due_day - start_day.
2. Project `late` = status is not Done and due_day < 100.
3. Task `remaining` = 0 when status is Done, else estimate - spent.
4. Task `overrun` = spent > estimate.
5. Task `weight` = estimate multiplied by 1 / 2 / 3 / 5 for priority
   Low / Medium / High / Urgent.
6. Ticket `breached` = state is Open or Pending and age_hours > sla_hours.
7. Ticket `escalation` = text `"page"` when severity is Critical and the
   ticket is breached, `"watch"` when only breached, otherwise `"ok"`.
8. TimeEntry `amount` = hours * rate when billable, else 0.0.
9. Customer `large` = seats >= 100 or tier is Enterprise.
10. Invoice `status` = `"paid"` when paid, else `"due"`.

## API

For every entity, under `/api/<entity>` (lowercase plural or singular is the
implementation's choice, documented in its README):

1. `GET` list all rows with their computed fields.
2. `GET /<id>` returns one row with computed fields; 404 when absent.
3. `POST` creates a row from a JSON body; 201 with the stored row, 400 with
   the list of violated rules.
4. `PUT /<id>` replaces a row; 200, 400, or 404.
5. `DELETE /<id>` deletes; 204, 404, or 409 when any other row references it.

## Persistence

All rows survive a server restart through one JSON file in a data directory.

## Browser UI

1. A navigation menu with a dashboard and every entity.
2. A dashboard that shows the row count of every entity and, for every
   enumeration-typed field, the row count per enumeration value.
3. A list page per entity: a table of all fields and computed fields, a text
   search over its string fields, sorting by any column, a filter per
   enumeration field, and pagination at 25 rows per page.
4. A detail page per entity: all fields and computed fields, reference fields
   shown as links displaying the referenced row's first string field, and a
   list of every row of another entity that references it.
5. Create and edit forms per entity with an input suited to each type: text,
   number, checkbox, a select for enumerations, and a select for references
   that displays the referenced row's first string field.
6. Delete with a confirmation, reporting a 409 refusal to the user.

## Out of scope

Authentication, styling beyond a usable default, internationalisation, and
any third-party service.
