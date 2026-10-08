import * as S from "./schema.js";
import * as rt from "./runtime.js";

const { app, enums, entities } = S, ACC = S.account ?? null;
const ents = Object.fromEntries(entities.map((e) => [e.path, e]));
const PAGE = 25;
const $main = document.getElementById("main");
const $nav = document.getElementById("nav");
const $who = document.getElementById("who");
document.title = app.title;

// ---- DOM helper: text is always set as text nodes, never as HTML ----
function h(tag, attrs, ...kids) {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs || {})) {
    if (k.startsWith("on")) e.addEventListener(k.slice(2), v);
    else if (k === "class") e.className = v;
    else if (k === "value" || k === "checked" || k === "selected" || k === "disabled") e[k] = v;
    else if (v !== false && v != null) e.setAttribute(k, v === true ? "" : v);
  }
  for (const c of kids.flat(9)) if (c != null && c !== false) e.append(c.nodeType ? c : String(c));
  return e;
}
const link = (href, text, cls) => h("a", { href, class: cls }, text);

// ---- data ----
async function api(method, url, body) {
  const headers = body ? { "content-type": "application/json" } : {};
  if (!["GET", "HEAD"].includes(method)) {
    const csrf = await fetch("/api/session/csrf");
    headers["x-csrf-token"] = (await csrf.json()).token;
  }
  const r = await fetch(url, { method, headers, body });
  const text = await r.text();
  let data = null;
  if (text) try { data = rt.parseJSON(text); } catch { /* non-JSON body */ }
  return { status: r.status, data };
}
let D = Object.create(null); // path -> readable rows (with .$c computed values as served)
let I = Object.create(null); // path -> Map(id -> row)
let ME = null, SETUP = false; // signed-in account row; setup mode
const decode = (e, o) => { const { row } = rt.decodeRow(e, enums, o, true); row.$c = rt.decodeComputed(e, enums, o); return row; };
async function loadAll() {
  const res = await Promise.all(entities.map((e) => api("GET", "/api/" + e.path)));
  if (res.some((r) => r.status === 401)) return false;
  D = Object.create(null); I = Object.create(null);
  entities.forEach((e, i) => {
    D[e.path] = (Array.isArray(res[i].data) ? res[i].data : []).map((o) => decode(e, o));
    I[e.path] = new Map(D[e.path].map((r) => [r.id, r]));
  });
  return true;
}
// ---- permissions (the server stays authoritative) ----
const free = () => !ACC || SETUP;
const pass = (p, r, u) => { try { return p.test(r, u) === true; } catch { return false; } };
const canW = (e, r) => free() || !e.canWrite || pass(e.canWrite, r, ME);
const canNew = (e) => {
  if (free() || !e.canWrite) return true;
  if (!e.canWrite.row) return pass(e.canWrite, {}, ME);
  // Only the compiler's partial predicate can prove that no prospective row
  // is writable. Unknown row fields retain access to the validated form.
  if (typeof e.canWrite.create !== "function") return true;
  try { return e.canWrite.create(ME) !== false; } catch { return true; }
};
const canAudit = () => free() || !ents[ACC.entity].canWrite || pass(ents[ACC.entity].canWrite, ME, ME);

const colsOf = (e) => [{ name: "id", type: "int", id: true }, ...e.fields, ...(e.computed || []).map((c) => ({ ...c, computed: true }))];
const valOf = (row, c) => (c.computed ? row.$c[c.name] : row[c.name]);
const isErr = (v) => v !== null && typeof v === "object";
function labelOf(path, id) {
  const e = ents[path], r = I[path] && I[path].get(id);
  if (!r) return `#${id}` + (ACC ? "" : " (missing)");
  return e.label === "id" ? `#${id}` : String(r[e.label]);
}
function cell(c, v) {
  if (isErr(v)) return h("span", { class: "warn" }, "⚠ " + v.error);
  if (c.type === "ref") return I[c.ref] && I[c.ref].has(v) ? link(`#/${c.ref}/${v}`, labelOf(c.ref, v)) : h("span", { class: ACC ? "mut" : "warn" }, labelOf(c.ref, v));
  if (c.id) return String(v);
  if (c.type === "bool") return v ? "yes" : "no";
  return String(v);
}
const numeric = (c) => c.type === "int" || c.type === "float";
const cmp = (a, b) => (isErr(a) || isErr(b) ? isErr(a) - isErr(b) : a < b ? -1 : a > b ? 1 : 0);
const flash = (msg) => { const old = document.getElementById("flash"); if (old) old.remove(); if (msg) $main.prepend(h("div", { class: "flash", id: "flash", role: "alert" }, msg)); };

// ---- views ----
function dashboard() {
  return h("div", {}, h("h1", {}, app.title),
    h("div", { class: "grid" }, entities.map((e) => h("div", {},
      h("h3", {}, link("#/" + e.path, e.name)),
      h("p", { class: "big" }, D[e.path].length),
      e.fields.filter((f) => f.type === "enum").map((f) => h("div", {},
        h("p", {}, h("strong", {}, f.name)),
        enums[f.enum].map((c) => h("p", {}, `${c}: ${D[e.path].filter((r) => r[f.name] === c).length}`))))))));
}

const LS = Object.create(null); // per-entity list state
function listView(e) {
  const S = (LS[e.path] ||= { q: "", sort: null, dir: 1, filters: Object.create(null), page: 0 });
  const cols = colsOf(e), strs = e.fields.filter((f) => f.type === "string");
  const box = h("div", { class: "wrap" }), csv = h("a", { class: "btn", download: e.path + ".csv" }, "Export CSV");
  const draw = () => {
    const q = S.q.toLowerCase();
    let rows = D[e.path].filter((r) => (!q || strs.some((f) => r[f.name].toLowerCase().includes(q))) &&
      Object.entries(S.filters).every(([k, v]) => !v || r[k] === v));
    if (S.sort) { const c = cols.find((x) => x.name === S.sort); rows = rows.map((r, i) => [r, i]).sort((a, b) => S.dir * cmp(valOf(a[0], c), valOf(b[0], c)) || a[1] - b[1]).map((x) => x[0]); }
    const pages = Math.max(1, Math.ceil(rows.length / PAGE));
    S.page = Math.min(S.page, pages - 1);
    const go = (d) => () => { S.page += d; draw(); };
    const qs = new URLSearchParams({ format: "csv" });
    if (S.q) qs.set("q", S.q);
    for (const [k, v] of Object.entries(S.filters)) if (v) qs.set(k, v);
    csv.href = `/api/${e.path}?${qs}`;
    box.replaceChildren(
      h("table", {},
        h("thead", {}, h("tr", {}, cols.map((c) => h("th", { onclick: () => { S.dir = S.sort === c.name ? -S.dir : 1; S.sort = c.name; draw(); }, "aria-sort": S.sort === c.name ? (S.dir > 0 ? "ascending" : "descending") : null },
          c.name + (S.sort === c.name ? (S.dir > 0 ? " ▲" : " ▼") : "")), h("th", {}))),
        h("tbody", {}, rows.slice(S.page * PAGE, (S.page + 1) * PAGE).map((r) => h("tr", {},
          cols.map((c) => h("td", { class: numeric(c) ? "num" : "" }, cell(c, valOf(r, c)))),
          h("td", {}, link(`#/${e.path}/${r.id}`, "view"), canW(e, r) && [" ", link(`#/${e.path}/${r.id}/edit`, "edit"), " ",
            h("button", { class: "del", onclick: () => del(e, r) }, "delete")]))))),
      rows.length ? null : h("p", { class: "mut" }, "No rows."),
      h("div", { class: "bar" }, h("button", { disabled: S.page === 0, onclick: go(-1) }, "‹ Prev"),
        h("span", {}, `Page ${S.page + 1} of ${pages} · ${rows.length} rows`),
        h("button", { disabled: S.page >= pages - 1, onclick: go(1) }, "Next ›"))));
  };
  const reset = () => { S.page = 0; draw(); };
  const bar = h("div", { class: "bar" },
    h("input", { type: "search", placeholder: "Search " + strs.map((f) => f.name).join(", "), value: S.q, "aria-label": "search", oninput: (ev) => { S.q = ev.target.value; reset(); } }),
    e.fields.filter((f) => f.type === "enum").map((f) => h("select", { "aria-label": "filter " + f.name, onchange: (ev) => { S.filters[f.name] = ev.target.value; reset(); } },
      h("option", { value: "" }, `${f.name}: all`), enums[f.enum].map((c) => h("option", { value: c, selected: S.filters[f.name] === c }, c)))),
    h("span", { class: "grow" }), csv, canNew(e) && link(`#/${e.path}/new`, "+ New " + e.name, "btn pri"));
  draw();
  return h("div", {}, h("h1", {}, e.name + " list"), bar, box);
}

// One audit entry as a list item: time, actor, action, then field: old -> new.
function entryView(en, withRow) {
  const e = ents[en.entity], id = rt.decodeValue({ type: "int" }, enums, en.id)[0];
  const byId = en.by === null ? null : rt.decodeValue({ type: "int" }, enums, en.by)[0];
  const by = byId === null ? (ACC ? "setup" : "anonymous") : ACC ? labelOf(ACC.entity, byId) : `#${byId}`;
  const disp = (f, v) => { if (v === null) return "—"; if (!f) return String(v); const [d] = rt.decodeValue(f, enums, v); return d === undefined ? "?" : cell(f, d); };
  return h("li", {}, h("span", { class: "mut" }, `${en.at} · ${by} · `), en.action,
    withRow && [" ", e ? link(`#/${e.path}/${id}`, `${e.name} ${id}`) : `${en.entity} ${id}`],
    h("ul", {}, Object.entries(en.changes || {}).map(([k, [a, b]]) => {
      const f = e && e.fields.find((x) => x.name === k);
      return h("li", {}, k + ": ", disp(f, a), " → ", disp(f, b));
    })));
}
function historyBox(url, withRow) {
  const box = h("div", {}, h("p", { class: "mut" }, "Loading…"));
  api("GET", url).then((res) => box.replaceChildren(Array.isArray(res.data) && res.data.length
    ? h("ul", { class: "hist" }, [...res.data].reverse().map((en) => entryView(en, withRow)))
    : h("p", { class: "mut" }, Array.isArray(res.data) ? "No history." : (res.data && res.data.error) || "history unavailable")));
  return box;
}

function detailView(e, row) {
  const back = [];
  for (const o of entities) for (const f of o.fields) if (f.type === "ref" && f.ref === e.path) {
    const rows = D[o.path].filter((r) => r[f.name] === row.id);
    back.push(h("div", {}, h("h2", {}, `${o.name} (via ${f.name}) · ${rows.length}`),
      rows.length ? h("ul", {}, rows.map((r) => h("li", {}, link(`#/${o.path}/${r.id}`, labelOf(o.path, r.id))))) : h("p", { class: "mut" }, "None.")));
  }
  return h("div", {}, h("h1", {}, `${e.name} ${labelOf(e.path, row.id)}`),
    h("div", { class: "bar" }, canW(e, row) && [link(`#/${e.path}/${row.id}/edit`, "Edit", "btn"), h("button", { class: "del", onclick: () => del(e, row) }, "Delete")], link("#/" + e.path, "Back to list")),
    h("dl", {}, colsOf(e).map((c) => [h("dt", {}, c.name), h("dd", {}, cell(c, valOf(row, c)))])), back,
    h("h2", {}, "History"), historyBox(`/api/${e.path}/${row.id}/history`, false));
}

function formView(e, row) {
  const ctl = Object.create(null), errBox = Object.create(null), isAcc = ACC && ACC.entity === e.path;
  const inputFor = (f) => {
    const v = row ? row[f.name] : undefined, st = (e.steps || []).find((s) => s.field === f.name);
    if (f.type === "bool") return h("input", { type: "checkbox", checked: !!v });
    if (f.type === "enum") {
      const cases = !st ? enums[f.enum] : row ? enums[f.enum].filter((c) => c === v || rt.stepOk(st, v, c)) : enums[f.enum].slice(0, 1);
      const permitted = !st || !row ? cases : cases.filter((c) => canW(e, { ...row, [f.name]: c }));
      return h("select", {}, permitted.map((c) => h("option", { value: c, selected: v === c }, c)));
    }
    if (f.type === "ref") return h("select", {}, h("option", { value: "" }, "— select —"), D[f.ref].map((r) => h("option", { value: String(r.id), selected: v === r.id }, labelOf(f.ref, r.id))));
    if (f.type === "int" || f.type === "float") return h("input", { type: "number", step: f.type === "int" ? "1" : "any", value: v === undefined ? "" : String(v) });
    if (f.type === "string" && /body|details|description|text|note/.test(f.name)) return h("textarea", { rows: 4, value: v ?? "" });
    return h("input", { type: "text", value: v ?? "" });
  };
  const fields = e.fields.map((f) => {
    ctl[f.name] = inputFor(f); errBox[f.name] = h("div", { class: "err" });
    return h("label", {}, f.name, ctl[f.name], errBox[f.name]);
  });
  const pw = isAcc && h("input", { type: "password", autocomplete: "new-password", placeholder: row ? "leave empty to keep" : "" });
  if (pw) { errBox.password = h("div", { class: "err" }); fields.push(h("label", {}, "password", pw, errBox.password)); }
  const general = h("div", { class: "err" });
  const show = (errors) => {
    general.replaceChildren(); Object.values(errBox).forEach((b) => b.replaceChildren());
    for (const er of errors) {
      const box = errBox[er.field];
      if (box) box.append(h("div", {}, er.message)); else general.append(h("div", {}, (er.field ? er.field + ": " : "") + er.message));
    }
  };
  const submit = async (ev) => {
    ev.preventDefault();
    const raw = Object.create(null);
    for (const f of e.fields) raw[f.name] = f.type === "bool" ? ctl[f.name].checked : ctl[f.name].value;
    const { row: r, errors, bad } = rt.decodeRow(e, enums, raw);
    for (const f of e.fields) if (f.type === "ref" && !bad.has(f.name) && !I[f.ref].has(r[f.name])) errors.push({ field: f.name, message: `${f.name} must reference an existing ${f.ref} row` });
    errors.push(...rt.evalRules(e, r, bad));
    if (row) r.id = row.id;
    errors.push(...rt.keyErrors(rt.keysOf(e, ACC), r, D[e.path], bad), ...rt.stepErrors(e, enums, r, row, bad));
    if (pw && pw.value && rt.len(pw.value) < 8n) errors.push({ field: "password", message: "must be at least 8 bytes" });
    if (pw && !pw.value && !row && !SETUP) errors.push({ field: "password", message: "is required" });
    if (!errors.length && !canW(e, r)) errors.push({ field: "", message: `you are not allowed to save this ${e.name}` });
    show(errors);
    if (errors.length) return;
    let body = rt.toJSON(e, { ...r, id: undefined });
    if (pw && pw.value) body = body.slice(0, -1) + ',"password":' + JSON.stringify(pw.value) + "}";
    const res = await api(row ? "PUT" : "POST", `/api/${e.path}${row ? "/" + row.id : ""}`, body);
    if (res.status === 200 || res.status === 201) return void (location.hash = `#/${e.path}/${row ? row.id : rt.decodeRow(e, enums, res.data, true).row.id}`);
    if (res.status === 401) return void route();
    if (res.data && res.data.errors) show(res.data.errors);
    else show([{ field: "", message: (res.data && res.data.error) || "request failed (" + res.status + ")" }]);
  };
  return h("div", {}, h("h1", {}, row ? `Edit ${e.name} ${labelOf(e.path, row.id)}` : "New " + e.name),
    h("form", { onsubmit: submit, novalidate: true }, fields, general,
      h("div", { class: "bar" }, h("button", { class: "pri", type: "submit" }, row ? "Save" : "Create"), link(row ? `#/${e.path}/${row.id}` : "#/" + e.path, "Cancel"))));
}

async function del(e, row) {
  if (!confirm(`Delete ${e.name} ${labelOf(e.path, row.id)}?`)) return;
  const res = await api("DELETE", `/api/${e.path}/${row.id}`);
  if (res.status === 204) location.hash = "#/" + e.path;
  await route();
  if (res.status !== 204) flash((res.data && res.data.error) || "delete failed (" + res.status + ")");
}

// ---- accounts ----
function signIn() {
  $nav.replaceChildren(); $who.replaceChildren();
  const login = h("input", { type: "text", autocomplete: "username" }), pw = h("input", { type: "password", autocomplete: "current-password" }), err = h("div", { class: "err", role: "alert" });
  const submit = async (ev) => {
    ev.preventDefault();
    const res = await api("POST", "/api/session", JSON.stringify({ login: login.value, password: pw.value }));
    if (res.status === 200) return void route();
    err.textContent = (res.data && res.data.error) || "sign-in failed (" + res.status + ")";
  };
  $main.replaceChildren(h("div", { class: "card signin" }, h("h1", {}, "Sign in to " + app.title),
    h("form", { onsubmit: submit, novalidate: true }, h("label", {}, ACC.login, login), h("label", {}, "password", pw), err,
      h("div", { class: "bar" }, h("button", { class: "pri", type: "submit" }, "Sign in")))));
  login.focus();
}
async function signOut() {
  await api("DELETE", "/api/session");
  ME = null; location.hash = "#/"; route();
}

// ---- router ----
function drawNav(cur) {
  $nav.replaceChildren(link("#/", "Dashboard", cur === "" ? "on" : ""), ...entities.map((e) => link("#/" + e.path, e.name, cur === e.path ? "on" : "")),
    canAudit() ? link("#/~audit", "Audit", cur === "~audit" ? "on" : "") : null);
  $who.replaceChildren(...(ME ? [h("span", {}, ents[ACC.entity].label === "id" ? `#${ME.id}` : String(ME[ents[ACC.entity].label])), h("button", { onclick: signOut }, "Sign out")] : []));
}
let seq = 0;
async function route() {
  const my = ++seq;
  const [p, id, act] = location.hash.replace(/^#\/?/, "").split("/");
  try {
    if (ACC) {
      const s = await api("GET", "/api/session");
      if (my !== seq) return;
      if (s.status === 401) return signIn();
      SETUP = !!(s.data && s.data.setup === true);
      ME = SETUP ? null : decode(ents[ACC.entity], s.data);
    }
    if (!(await loadAll())) return void (my === seq && signIn());
  } catch (err) { $main.replaceChildren(h("div", { class: "flash" }, "Cannot reach the server: " + err.message)); return; }
  if (my !== seq) return;
  drawNav(p || "");
  const e = ents[p];
  let view;
  if (!p) view = dashboard();
  else if (p === "~audit" && canAudit()) view = h("div", {}, h("h1", {}, "Audit log"), historyBox("/api/audit", true));
  else if (!e) view = h("p", {}, "Page not found. ", link("#/", "Dashboard"));
  else if (!id) view = listView(e);
  else if (id === "new") view = canNew(e) ? formView(e) : h("p", {}, `You may not create ${e.name} rows. `, link("#/" + e.path, "Back to list"));
  else if (!/^[1-9]\d*$/.test(id) || !I[e.path].has(BigInt(id))) view = h("p", {}, `${e.name} ${id} not found. `, link("#/" + e.path, "Back to list"));
  else {
    const row = I[e.path].get(BigInt(id));
    view = act !== "edit" ? detailView(e, row) : canW(e, row) ? formView(e, row) : h("p", {}, `You may not edit this ${e.name}. `, link(`#/${e.path}/${id}`, "Back"));
  }
  $main.replaceChildren(...(SETUP ? [h("div", { class: "banner", role: "status" }, "Setup mode: no account has a password yet. Create the first " + ents[ACC.entity].name + " with a password, then sign in.")] : []), view);
}
window.addEventListener("hashchange", route);
route();
