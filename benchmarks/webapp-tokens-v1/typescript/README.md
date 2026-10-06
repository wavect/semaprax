# TeamDesk (TypeScript)

Vite + React 18 + react-router client, Node server using only `node:http` and
`node:fs`. Entity schema, validation and computed fields live once in
`shared/schema.ts` and drive the server, the generic list/detail/form views and
the dashboard.

## Run

    npm install
    npm run server    # API on 127.0.0.1:3001 (PORT, DATA_DIR override); needs Node >= 22.6
    npm run dev       # UI on :5173, proxies /api to :3001
    npm run build     # type-check + production client build

The server runs `.ts` directly via `node --experimental-strip-types` (no compile
step). Rows persist in `data/teamdesk.json`.

## API

`/api/<entity>` with the lowercase singular name: `team`, `member`, `customer`,
`project`, `milestone`, `task`, `ticket`, `comment`, `timeentry`, `invoice`.

- `GET /api/<entity>` list, `GET /api/<entity>/<id>` one (404)
- `POST` create (201, 400 `{errors}`), `PUT /<id>` replace (200, 400, 404)
- `DELETE /<id>` (204, 404, 409 when referenced)
