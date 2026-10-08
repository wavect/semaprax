import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { COVERAGE, PASSWORD, ENTITIES, INVALID, KEYS, WORKFLOWS, ENUMS } from './contract.mjs';
import { tree } from './process.mjs';
export const sha256=value=>createHash('sha256').update(value).digest('hex');
// This obligation inventory is fixed before candidates run. A missing check is
// a failure; a narrowed runtime/schema cannot shrink the obligations.
export function requiredCases(){const ids=['bootstrap','all-computed-and-rollups','rollup-mutation-refresh','sign-in-out-inactive','password-edit','audit-create-update-delete','csv-quotation-and-row-hiding','deleted-ids-never-reused','restart-accounts-rows-audit','password-kdf-storage','browser.auth.pages-header-signout','browser.dashboard.entity-and-enum-counts','browser.history-old-new','browser.delete-confirm-and-reference-409','browser.csv-all-twenty-entities'];
  for(const [entity,fields]of Object.entries(ENTITIES)){ids.push(`${entity}.shape`,`${entity}.csv`,`browser.${entity}.navigation-detail-fields`,`browser.${entity}.typed-form`,`browser.${entity}.list-search-sort-pagination-filter-csv`);for(const [field,type]of Object.entries(fields)){ids.push(`${entity}.${field}.type`);if(type.startsWith('ref:'))ids.push(`${entity}.${field}.reference`,`browser.${entity}.${field}.links-backrefs`);if(ENUMS[type]&&WORKFLOWS[entity]?.[0]!==field)for(const value of ENUMS[type])ids.push(`${entity}.${field}.${value}.enum`);}for(const role of ['Admin','Manager','Agent','Viewer'])ids.push(`${role}.${entity}.matrix`,`browser.${role}.${entity}.hidden-actions`);}
  for(const [entity,name]of INVALID){ids.push(`${entity}.${name}`,`browser.${entity}.${name}.pre-submit`);if(name.endsWith('.short')||name.endsWith('.long'))ids.push(`${entity}.${name}.valid-endpoint`);}
  for(let i=0;i<19;i++)ids.push(`numeric-valid-${i}`);
  for(const entity of Object.keys(KEYS))ids.push(`${entity}.keys`,`browser.${entity}.unique-key`);
  for(const [entity,[field]]of Object.entries(WORKFLOWS)){const states=ENUMS[ENTITIES[entity][field]];for(const state of states.slice(1))ids.push(`${entity}.create.${state}`);for(const from of states){ids.push(`browser.${entity}.${from}.workflow-options`);for(const to of states)ids.push(`${entity}.${from}.${to}`);}}
  for(const entity of ['Task','Comment','TimeEntry','Ticket','TicketReply','Leave','Expense'])ids.push(`${entity}.own-other`);
  ids.push('account.Manager','account.Agent','account.Viewer','Sprint.signed64','Project.signed64','Ticket.signed64');return ids.sort();
}
export function qualify(rows){assert.equal(new Set(rows.map(row=>row.id)).size,rows.length,'duplicate case identities');const missingCases=requiredCases().filter(id=>!rows.some(row=>row.id===id));const missingGroups=COVERAGE.filter(group=>!rows.some(row=>row.group===group));const failures=rows.filter(row=>row.status!=='passed');return {passed:missingCases.length===0&&missingGroups.length===0&&failures.length===0,missingCases,missingGroups,failures:failures.map(row=>row.id),cases:rows.length};}
export async function passwordChecks({data,ledger}){
  const files=await tree(data),bytes=Buffer.concat(files.map(([,bytes])=>bytes));assert.ok(!bytes.includes(Buffer.from(PASSWORD)),'plaintext account password is absent from all data files');
  let ledgerText;try{ledgerText=await fs.readFile(ledger,'utf8');}catch(error){if(error.code==='ENOENT')return {verified:false,reason:'No observed supported KDF. Independent review of the actual KDF/storage is required; this is not a SPEC failure.'};throw error;}
  const records=ledgerText.trim().split('\n').filter(Boolean).map(line=>JSON.parse(line));const passwordDigest=sha256(PASSWORD);const actual=records.filter(row=>row.password_sha256===passwordDigest);
  const contains=hex=>bytes.includes(Buffer.from(hex))||bytes.includes(Buffer.from(hex,'hex'))||bytes.includes(Buffer.from(Buffer.from(hex,'hex').toString('base64')));
  const stored=actual.filter(row=>contains(row.key_hex)&&contains(row.salt_hex));if(actual.length<4||new Set(stored.map(row=>row.salt_hex)).size<4)return {verified:false,reason:'Observed KDF/storage proof is insufficient. Review unsupported KDF or encoding independently; no success is inferred.'};
  for(const row of stored){assert.ok(/^[0-9a-f]+$/.test(row.salt_hex)&&row.salt_hex.length>=32);assert.ok(row.key_hex.length>=64);assert.ok(row.algorithm==='scrypt'||row.algorithm==='pbkdf2');if(row.algorithm==='scrypt')assert.ok(row.parameters.N>=16384&&row.parameters.r>=8&&row.parameters.p>=1,'scrypt retains ordinary slow default work factors');else assert.ok(row.parameters.iterations>=100000&&['sha256','sha512'].includes(row.parameters.digest),'PBKDF2 has explicit slow work');}
  return {verified:true,algorithm:[...new Set(stored.map(row=>row.algorithm))],distinctStoredSalts:new Set(stored.map(row=>row.salt_hex)).size,dataFiles:files.map(([name,bytes])=>({name,sha256:sha256(bytes)}))};
}
