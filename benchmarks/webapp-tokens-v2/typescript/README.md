# TeamDesk Enterprise (TypeScript)

Vite + React 18 + react-router client, Node server using only `node:http`,
`node:fs` and `node:crypto`. Entities, validation, keys, workflows, computed
fields, rollups, CSV and permissions live once in `shared/schema.ts` and drive
the server, the generic views and the dashboard.

## Run

    npm install
    npm run server    # API on 127.0.0.1:3001 (PORT, DATA_DIR override); needs Node >= 22.6
    npm run dev       # UI on :5173, proxies /api to :3001
    npm run build     # type-check + production client build

Data (rows, password hashes, audit log) persists in `data/teamdesk.json`.

## First run

With no members, the sign-in page asks for name, email and password and creates
the first Admin (plus a Team "Administration"): `POST /api/setup`
(`GET /api/setup` reports `{needed}`). It works only while no Member exists.

## API

- `POST /api/session {email,password}` sign in (HttpOnly SameSite=Strict cookie), `DELETE /api/session` sign out, `GET /api/me`, `GET /api/audit` (Admin)
- `/api/<entity>` with the lowercase name (`team`, `ticketreply`, `timeentry`, ...):
  `GET` list (`?q=text&<enum field>=value&format=csv`), `GET /<id>`, `GET /<id>/history`,
  `POST` (201), `PUT /<id>` (200), `DELETE /<id>` (204)
- Errors: 400 `{errors}`, 401 not signed in, 403 forbidden write, 404 absent or unreadable, 409 referenced
- A Member body carries `password` (required on create, optional on edit); it is never returned.
