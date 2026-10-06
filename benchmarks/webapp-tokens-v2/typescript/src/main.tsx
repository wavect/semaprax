import { useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { BrowserRouter, Link, Route, Routes } from 'react-router-dom';
import { canRead, names, type Row } from '../shared/schema.ts';
import { Me, request, route, send } from './api.ts';
import { Audit, Dashboard, Detail, Entity, Form, List, SignIn } from './views.tsx';

function App() {
  const [me, setMe] = useState<Row | null>();
  useEffect(() => void request('GET', '/me').then((r) => setMe(r.ok ? r.json : null)), []);
  if (me === undefined) return <p>Loading...</p>;
  if (!me) return <SignIn onDone={setMe} />;
  const signOut = () => send('DELETE', '/session').then(() => setMe(null));
  return (
    <Me.Provider value={me}>
      <nav style={{ display: 'flex', gap: '1em', flexWrap: 'wrap' }}>
        <Link to="/">Dashboard</Link>
        {names.filter((name) => canRead(me, name)).map((name) => (
          <Link key={name} to={route(name)}>
            {name}
          </Link>
        ))}
        {me.role === 'Admin' && <Link to="/audit">Audit</Link>}
        <span>
          {me.name} ({me.role}) <button onClick={signOut}>Sign out</button>
        </span>
      </nav>
      <Routes>
        <Route path="/" element={<Dashboard />} />
        <Route path="/audit" element={<Audit />} />
        <Route path="/:entity" element={<Entity view={List} />} />
        <Route path="/:entity/new" element={<Entity view={Form} />} />
        <Route path="/:entity/:id" element={<Entity view={Detail} />} />
        <Route path="/:entity/:id/edit" element={<Entity view={Form} />} />
      </Routes>
    </Me.Provider>
  );
}

createRoot(document.getElementById('root')!).render(
  <BrowserRouter>
    <App />
  </BrowserRouter>,
);
