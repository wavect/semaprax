// Explicit, fail-closed field migration and whole-candidate pairwise constraints.
import * as rt from "./runtime.js";
export const stateSchema = (entities, enums) => ({ entities: entities.map((e) => ({ path: e.path, fields: e.fields })), enums });
export function constraintErrors(tables, change = null) {
  const rows = new Map();
  for (const [path, t] of tables) {
    const values = new Map(t.rows);
    if (change?.path === path) { if (change.row === null) values.delete(change.id); else values.set(change.id, change.row); }
    rows.set(path, values);
  }
  const errors = [];
  for (const [path, t] of tables) for (const rule of t.ent.constraints || []) {
    const other = rows.get(rule.other);
    if (!other) throw new Error("constraint references unknown entity " + rule.other);
    for (const row of rows.get(path).values()) for (const candidate of other.values()) {
      if (path === rule.other && row.id === candidate.id) continue;
      let ok; try { ok = rule.test(row, candidate) === true; } catch { ok = false; }
      if (!ok) { errors.push({ field: !change || change.path === path ? rule.fields[0] || "" : "", message: `${t.ent.name}: ${rule.name} cross-row constraint failed` }); return errors; }
    }
  }
  return errors;
}
const numberText = (v) => String(v?.source ?? v);
export function loadState(bytes, tables, enums, account, migrate) {
  const db = rt.parseJSON(bytes);
  if (!["1", "2"].includes(numberText(db.version)) || !db.rows || !db.next) throw new Error("invalid database envelope");
  const schema = stateSchema([...tables.values()].map((t) => t.ent), enums);
  // parseJSON preserves numeric spellings; schema contains only strings/arrays/objects.
  const changed = db.schema ? JSON.stringify(db.schema) !== JSON.stringify(schema) : false;
  if (changed && !migrate) throw new Error("database schema changed; review migrations and restart with --migrate");
  const sourceTables = db.schema?.entities || [];
  const sourceEnums = db.schema?.enums || enums;
  const staged = new Map();
  for (const [path, table] of tables) {
    const source = sourceTables.find((e) => e.path === path);
    const rows = new Map(); let next = 1n;
    for (const original of db.rows[path] || []) {
      let input = original;
      const mismatch = table.ent.fields.some((f) => !Object.hasOwn(original, f.name)) || Object.keys(original).some((f) => f !== "id" && !table.ent.fields.some((d) => d.name === f));
      if ((changed || mismatch) && !migrate) throw new Error(`database fields changed in ${path}; restart with --migrate`);
      if (migrate && (changed || mismatch)) {
        input = { id: original.id };
        for (const field of table.ent.fields) {
          const steps = (table.ent.migrations || []).filter((m) => m.field === field.name);
          if (steps.length > 1) throw new Error(`duplicate migration for ${path}.${field.name}`);
          if (steps.length) {
            const migration = steps[0], old = {};
            for (const descriptor of migration.inputs) {
              if (!Object.hasOwn(original, descriptor.name)) throw new Error(`missing migration input ${path}.${descriptor.name}`);
              const historical = source?.fields.find((f) => f.name === descriptor.name);
              if (historical && ((historical.type === "ref" ? "int" : historical.type) !== descriptor.type || historical.enum !== descriptor.enum)) throw new Error(`migration input type changed: ${path}.${descriptor.name}`);
              const [value, error] = rt.decodeValue(descriptor, sourceEnums, original[descriptor.name]);
              if (error) throw new Error(`invalid migration input ${path}.${descriptor.name}: ${error}`);
              old[descriptor.name] = value;
            }
            let value; try { value = migration.value(old); } catch (e) { throw new Error(`migration failed ${path}.${field.name}: ${e.code || e.message}`); }
            // Encode through the ordinary runtime so BigInt/char/enum checks remain authoritative.
            input[field.name] = rt.parseJSON(rt.encValue(field, value));
          } else {
            const historical = source?.fields.find((f) => f.name === field.name);
            if (!Object.hasOwn(original, field.name) || historical && JSON.stringify(historical) !== JSON.stringify(field)) throw new Error(`explicit migration required for ${path}.${field.name}`);
            input[field.name] = original[field.name];
          }
        }
      }
      const { row, errors } = rt.decodeRow(table.ent, enums, input, true);
      if (errors.length || row.id === undefined || row.id <= 0n || rows.has(row.id)) throw new Error(`invalid stored row ${path}: ${JSON.stringify(errors)}`);
      rows.set(row.id, row); if (row.id >= next) next = row.id + 1n;
    }
    const storedNext = BigInt(numberText(db.next[path] ?? "1"));
    if (storedNext < 1n || storedNext > 9223372036854775807n) throw new Error(`invalid next id ${path}`);
    if (storedNext > next) next = storedNext;
    staged.set(path, { ent: table.ent, rows, next });
  }
  // Removing populated entities needs a dedicated data export instead of silent data loss.
  for (const [path, rows] of Object.entries(db.rows)) if (!staged.has(path) && rows.length) throw new Error(`cannot remove populated entity ${path}`);
  for (const [path, t] of staged) for (const row of t.rows.values()) {
    const errors = [...rt.evalRules(t.ent, row), ...rt.keyErrors(rt.keysOf(t.ent, account), row, [...t.rows.values()])];
    for (const field of t.ent.fields) if (field.type === "ref" && !staged.get(field.ref)?.rows.has(row[field.name])) errors.push({ field: field.name, message: "missing reference" });
    if (errors.length) throw new Error(`stored constraints fail ${path}: ${JSON.stringify(errors)}`);
  }
  const errors = constraintErrors(staged);
  if (errors.length) throw new Error(`stored cross-row constraints fail: ${JSON.stringify(errors)}`);
  // No caller-visible state changes until every migrated row and all constraints pass.
  for (const [path, t] of staged) { tables.get(path).rows = t.rows; tables.get(path).next = t.next; }
  return migrate && (changed || !db.schema);
}
