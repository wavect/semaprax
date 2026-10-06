import { Fragment, useContext, useEffect, useState, type FormEvent, type ReactElement } from 'react';
import { Link, useNavigate, useParams } from 'react-router-dom';
import {
  canRead, canWrite, choices, columns, entities, enums, labelOf, matching, names, referrers, validate,
  type EntityName, type Field, type Row,
} from '../shared/schema.ts';
import { Me, errorsOf, request, route, send, useData, type Data } from './api.ts';

type ViewProps = { name: EntityName; data: Data; reload: () => void };

const PAGE_SIZE = 25;

export function Entity({ view: View }: { view: (props: ViewProps) => ReactElement }) {
  const slug = useParams().entity;
  const me = useContext(Me);
  const name = names.find((n) => n.toLowerCase() === slug);
  const [data, reload] = useData();
  if (!name || !canRead(me, name)) return <p>Unknown entity</p>;
  if (!data) return <p>Loading...</p>;
  return <View name={name} data={data} reload={reload} />;
}

function Value({ field, value, data }: { field: Field; value: unknown; data: Data }) {
  if (field.type !== 'ref') return <>{String(value)}</>;
  const target = field.of as EntityName;
  const row = data[target].find((r) => r.id === value);
  return <Link to={`${route(target)}/${value}`}>{row ? labelOf(target, row) : `#${value}`}</Link>;
}

function DeleteButton({ name, id, onDone }: { name: EntityName; id: number; onDone: () => void }) {
  const remove = async () => {
    if (!confirm(`Delete ${name} #${id}?`)) return;
    const errors = await send('DELETE', `${route(name)}/${id}`);
    if (errors.length) alert(errors.join('\n'));
    else onDone();
  };
  return <button onClick={remove}>Delete</button>;
}

function Changes({ entries, data }: { entries: Row[]; data: Data }) {
  return (
    <ul>
      {entries.map((a, i) => (
        <li key={i}>
          {a.time} {data.Member.find((m) => m.id === a.member_id)?.name ?? `member #${a.member_id}`} {a.action} {a.entity} #{a.id}:{' '}
          {Object.entries(a.changes).map(([key, [was, now]]: any) => `${key} ${was} → ${now}`).join(', ')}
        </li>
      ))}
    </ul>
  );
}

export function Audit() {
  const [data] = useData();
  const [entries, setEntries] = useState<Row[]>();
  useEffect(() => void request('GET', '/audit').then((r) => setEntries(r.json)), []);
  if (!data || !entries) return <p>Loading...</p>;
  return (
    <>
      <h1>Audit log</h1>
      <Changes entries={[...entries].reverse()} data={data} />
    </>
  );
}

export function SignIn({ onDone }: { onDone: (me: Row) => void }) {
  const [setup, setSetup] = useState(false);
  const [values, setValues] = useState({ name: '', email: '', password: '' });
  const [errors, setErrors] = useState<string[]>([]);
  useEffect(() => void request('GET', '/setup').then((r) => setSetup(r.json.needed)), []);
  const submit = async (e: FormEvent) => {
    e.preventDefault();
    const made = setup ? errorsOf(await request('POST', '/setup', values)) : [];
    if (made.length) return setErrors(made);
    const res = await request('POST', '/session', values);
    if (res.ok) onDone(res.json);
    else setErrors(errorsOf(res));
  };
  return (
    <form onSubmit={submit}>
      <h1>{setup ? 'Create the first Admin' : 'Sign in'}</h1>
      {(setup ? (['name', 'email', 'password'] as const) : (['email', 'password'] as const)).map((key) => (
        <p key={key}>
          <label>
            {key}{' '}
            <input type={key === 'password' ? 'password' : 'text'} value={values[key]} onChange={(e) => setValues({ ...values, [key]: e.target.value })} />
          </label>
        </p>
      ))}
      {errors.length > 0 && <ul style={{ color: 'crimson' }}>{errors.map((m) => <li key={m}>{m}</li>)}</ul>}
      <button>{setup ? 'Create and sign in' : 'Sign in'}</button>
    </form>
  );
}

export function Dashboard() {
  const me = useContext(Me);
  const [data] = useData();
  if (!data) return <p>Loading...</p>;
  return (
    <>
      <h1>Dashboard</h1>
      {names.filter((name) => canRead(me, name)).map((name) => (
        <section key={name}>
          <h2>
            <Link to={route(name)}>{name}</Link>: {data[name].length}
          </h2>
          {entities[name].fields
            .filter((f) => f.type === 'enum')
            .map((f) => (
              <p key={f.name}>
                {f.name}: {enums[f.of].map((v) => `${v} ${data[name].filter((r) => r[f.name] === v).length}`).join(', ')}
              </p>
            ))}
        </section>
      ))}
    </>
  );
}

export function List({ name, data, reload }: ViewProps) {
  const me = useContext(Me);
  const [search, setSearch] = useState('');
  const [filters, setFilters] = useState<Record<string, string>>({});
  const [sort, setSort] = useState({ column: 'id', desc: false });
  const [page, setPage] = useState(0);

  const rows = matching(name, data[name], search, filters).sort(
    (a, b) => (a[sort.column] > b[sort.column] ? 1 : a[sort.column] < b[sort.column] ? -1 : 0) * (sort.desc ? -1 : 1),
  );
  const pages = Math.max(1, Math.ceil(rows.length / PAGE_SIZE));
  const current = Math.min(page, pages - 1);
  const csv = `/api${route(name)}?${new URLSearchParams({ format: 'csv', q: search, ...filters })}`;

  return (
    <>
      <h1>{name}</h1>
      <p>
        <input placeholder="Search" value={search} onChange={(e) => setSearch(e.target.value)} />{' '}
        {entities[name].fields
          .filter((f) => f.type === 'enum')
          .map((f) => (
            <select key={f.name} value={filters[f.name] ?? ''} onChange={(e) => setFilters({ ...filters, [f.name]: e.target.value })}>
              <option value="">{f.name}: all</option>
              {enums[f.of].map((v) => (
                <option key={v}>{v}</option>
              ))}
            </select>
          ))}{' '}
        {canWrite(me, name) && <Link to={`${route(name)}/new`}>New {name}</Link>} <a href={csv}>CSV</a>
      </p>
      <table>
        <thead>
          <tr>
            {columns(name).map((c) => (
              <th key={c.name} onClick={() => setSort({ column: c.name, desc: sort.column === c.name && !sort.desc })}>
                {c.name}
                {sort.column === c.name && (sort.desc ? ' ▼' : ' ▲')}
              </th>
            ))}
            <th />
          </tr>
        </thead>
        <tbody>
          {rows.slice(current * PAGE_SIZE, (current + 1) * PAGE_SIZE).map((r) => (
            <tr key={r.id}>
              {columns(name).map((c) => (
                <td key={c.name}>
                  <Value field={c} value={r[c.name]} data={data} />
                </td>
              ))}
              <td>
                <Link to={`${route(name)}/${r.id}`}>View</Link>{' '}
                {canWrite(me, name, r) && (
                  <>
                    <Link to={`${route(name)}/${r.id}/edit`}>Edit</Link> <DeleteButton name={name} id={r.id} onDone={reload} />
                  </>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <p>
        <button disabled={current === 0} onClick={() => setPage(current - 1)}>Prev</button> Page {current + 1} of {pages} ({rows.length} rows){' '}
        <button disabled={current === pages - 1} onClick={() => setPage(current + 1)}>Next</button>
      </p>
    </>
  );
}

export function Detail({ name, data }: ViewProps) {
  const me = useContext(Me);
  const id = Number(useParams().id);
  const navigate = useNavigate();
  const row = data[name].find((r) => r.id === id);
  const [history, setHistory] = useState<Row[]>([]);
  useEffect(() => void request('GET', `${route(name)}/${id}/history`).then((r) => r.ok && setHistory(r.json)), [name, id]);
  if (!row || !canRead(me, name, row)) return <p>{name} {id} not found</p>;
  return (
    <>
      <h1>{name} #{id}</h1>
      <dl>
        {columns(name).map((c) => (
          <Fragment key={c.name}>
            <dt>{c.name}</dt>
            <dd>
              <Value field={c} value={row[c.name]} data={data} />
            </dd>
          </Fragment>
        ))}
      </dl>
      {canWrite(me, name, row) && (
        <p>
          <Link to="edit">Edit</Link> <DeleteButton name={name} id={id} onDone={() => navigate(route(name))} />
        </p>
      )}
      {referrers(name).map(([other, f]) => {
        const rows = data[other].filter((r) => r[f.name] === id);
        return (
          rows.length > 0 && (
            <section key={`${other}.${f.name}`}>
              <h2>{other} ({f.name})</h2>
              <ul>
                {rows.map((r) => (
                  <li key={r.id}>
                    <Link to={`${route(other)}/${r.id}`}>{labelOf(other, r)}</Link>
                  </li>
                ))}
              </ul>
            </section>
          )
        );
      })}
      <h2>History</h2>
      <Changes entries={history} data={data} />
    </>
  );
}

function Input({ field, value, options, data, onChange }: {
  field: Field; value: any; options: string[]; data: Data; onChange: (value: unknown) => void;
}) {
  switch (field.type) {
    case 'bool':
      return <input type="checkbox" checked={value} onChange={(e) => onChange(e.target.checked)} />;
    case 'enum':
      return (
        <select value={value} onChange={(e) => onChange(e.target.value)}>
          {options.map((v) => (
            <option key={v}>{v}</option>
          ))}
        </select>
      );
    case 'ref':
      return (
        <select value={value || ''} onChange={(e) => onChange(Number(e.target.value))}>
          <option value="">-</option>
          {data[field.of as EntityName].map((r) => (
            <option key={r.id} value={r.id}>{labelOf(field.of as EntityName, r)}</option>
          ))}
        </select>
      );
    case 'string':
    case 'secret':
      return <input type={field.type === 'secret' ? 'password' : 'text'} value={value} onChange={(e) => onChange(e.target.value)} />;
    default:
      return (
        <input
          type="number"
          step={field.type === 'int' ? 1 : 'any'}
          value={Number.isNaN(value) ? '' : value}
          onChange={(e) => onChange(e.target.valueAsNumber)}
        />
      );
  }
}

const blank = (f: Field) =>
  ({ string: '', secret: '', int: 0, float: 0, bool: false, enum: enums[f.of]?.[0], ref: 0 })[f.type];

export function Form({ name, data }: ViewProps) {
  const me = useContext(Me);
  const id = useParams().id;
  const existing = id === undefined ? undefined : data[name].find((r) => r.id === Number(id));
  const { fields } = entities[name];
  const [values, setValues] = useState<Row>(() => ({
    ...Object.fromEntries(fields.map((f) => [f.name, f.name === 'member_id' ? me.id : blank(f)])),
    ...existing,
    ...(existing && { password: '' }),
  }));
  const [errors, setErrors] = useState<string[]>([]);
  const navigate = useNavigate();
  if (id !== undefined && !existing) return <p>{name} {id} not found</p>;
  if (!canWrite(me, name, ...(existing ? [existing] : []))) return <p>Forbidden</p>;

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    const found = validate(name, values, data, existing);
    const failed = found.length ? found : await send(existing ? 'PUT' : 'POST', `${route(name)}${existing ? `/${existing.id}` : ''}`, values);
    if (failed.length) setErrors(failed);
    else navigate(route(name));
  };

  return (
    <form onSubmit={submit}>
      <h1>{existing ? `Edit ${name} #${existing.id}` : `New ${name}`}</h1>
      {fields.map((f) => (
        <p key={f.name}>
          <label>
            {f.name}{' '}
            <Input
              field={f}
              value={values[f.name]}
              options={choices(name, f, existing?.[f.name])}
              data={data}
              onChange={(v) => setValues({ ...values, [f.name]: v })}
            />
          </label>
        </p>
      ))}
      {errors.length > 0 && (
        <ul style={{ color: 'crimson' }}>
          {errors.map((message) => (
            <li key={message}>{message}</li>
          ))}
        </ul>
      )}
      <button>Save</button> <Link to={route(name)}>Cancel</Link>
    </form>
  );
}
