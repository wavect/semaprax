import { createRoot } from 'react-dom/client';
import { BrowserRouter, Link, Route, Routes } from 'react-router-dom';
import { names } from '../shared/schema.ts';
import { route } from './api.ts';
import { Dashboard, Detail, Entity, Form, List } from './views.tsx';

createRoot(document.getElementById('root')!).render(
  <BrowserRouter>
    <nav style={{ display: 'flex', gap: '1em', flexWrap: 'wrap' }}>
      <Link to="/">Dashboard</Link>
      {names.map((name) => (
        <Link key={name} to={route(name)}>
          {name}
        </Link>
      ))}
    </nav>
    <Routes>
      <Route path="/" element={<Dashboard />} />
      <Route path="/:entity" element={<Entity view={List} />} />
      <Route path="/:entity/new" element={<Entity view={Form} />} />
      <Route path="/:entity/:id" element={<Entity view={Detail} />} />
      <Route path="/:entity/:id/edit" element={<Entity view={Form} />} />
    </Routes>
  </BrowserRouter>,
);
