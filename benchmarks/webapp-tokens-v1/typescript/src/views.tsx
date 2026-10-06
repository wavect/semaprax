import { Fragment, useState, type FormEvent, type ReactElement } from 'react';
import { Link, useNavigate, useParams } from 'react-router-dom';
import {
  columns, entities, enums, labelOf, names, referrers, validate,
  type EntityName, type Field, type Row,
} from '../shared/schema.ts';
import { route, send, useData, type Data } from './api.ts';

type ViewProps = { name: EntityName; data: Data; reload: () => void };

const PAGE_SIZE = 25;

export function Entity({ view: View }: { view: (props: ViewProps) => ReactElement }) {
  const slug = useParams().entity;
  const name = names.find((n) => n.toLowerCase() === slug);
  const [data, reload] = useData();
  if (!name) return <p>Unknown entity</p>;
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
    const errors = await send('DELETE', name, id);
    if (errors.length) alert(errors.join('\n'));
    else onDone();
  };
  return <button onClick={remove}>Delete</button>;
}

export function Dashboard() {
  const [data] = useData();
  if (!data) return <p>Loading...</p>;
  return (
    <>
      <h1>Dashboard</h1>
      {names.map((name) => (
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
  const { fields } = entities[name];
  const [search, setSearch] = useState('');
  const [filters, setFilters] = useState<Record<string, string>>({});
  const [sort, setSort] = useState({ column: 'id', desc: false });
  const [page, setPage] = useState(0);

  const needle = search.toLowerCase();
  const rows = data[name]
    .filter((r) => fields.every((f) => !filters[f.name] || r[f.name] === filters[f.name]))
    .filter((r) => fields.some((f) => f.type === 'string' && r[f.name].toLowerCase().includes(needle)))
    .sort((a, b) => (a[sort.column] > b[sort.column] ? 1 : a[sort.column] < b[sort.column] ? -1 : 0) * (sort.desc ? -1 : 1));
  const pages = Math.max(1, Math.ceil(rows.length / PAGE_SIZE));
  const current = Math.min(page, pages - 1);

  return (
    <>
      <h1>{name}</h1>
      <p>
        <input placeholder="Search" value={search} onChange={(e) => setSearch(e.target.value)} />{' '}
        {fields
          .filter((f) => f.type === 'enum')
          .map((f) => (
            <select key={f.name} value={filters[f.name] ?? ''} onChange={(e) => setFilters({ ...filters, [f.name]: e.target.value })}>
              <option value="">{f.name}: all</option>
              {enums[f.of].map((v) => (
                <option key={v}>{v}</option>
              ))}
            </select>
          ))}{' '}
        <Link to={`${route(name)}/new`}>New {name}</Link>
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
                <Link to={`${route(name)}/${r.id}`}>View</Link> <Link to={`${route(name)}/${r.id}/edit`}>Edit</Link>{' '}
                <DeleteButton name={name} id={r.id} onDone={reload} />
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
  const id = Number(useParams().id);
  const navigate = useNavigate();
  const row = data[name].find((r) => r.id === id);
  if (!row) return <p>{name} {id} not found</p>;
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
      <p>
        <Link to="edit">Edit</Link> <DeleteButton name={name} id={id} onDone={() => navigate(route(name))} />
      </p>
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
    </>
  );
}

function Input({ field, value, data, onChange }: { field: Field; value: any; data: Data; onChange: (value: unknown) => void }) {
  switch (field.type) {
    case 'bool':
      return <input type="checkbox" checked={value} onChange={(e) => onChange(e.target.checked)} />;
    case 'enum':
      return (
        <select value={value} onChange={(e) => onChange(e.target.value)}>
          {enums[field.of].map((v) => (
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
      return <input value={value} onChange={(e) => onChange(e.target.value)} />;
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
  ({ string: '', int: 0, float: 0, bool: false, enum: enums[f.of]?.[0], ref: 0 })[f.type];

export function Form({ name, data }: ViewProps) {
  const id = useParams().id;
  const existing = id === undefined ? undefined : data[name].find((r) => r.id === Number(id));
  const { fields } = entities[name];
  const [values, setValues] = useState<Row>(() => existing ?? Object.fromEntries(fields.map((f) => [f.name, blank(f)])));
  const [errors, setErrors] = useState<string[]>([]);
  const navigate = useNavigate();
  if (id !== undefined && !existing) return <p>{name} {id} not found</p>;

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    const found = validate(name, values, (target, rowId) => data[target].some((r) => r.id === rowId));
    const failed = found.length ? found : await send(existing ? 'PUT' : 'POST', name, existing?.id, values);
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
            <Input field={f} value={values[f.name]} data={data} onChange={(v) => setValues({ ...values, [f.name]: v })} />
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
