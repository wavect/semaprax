import assert from 'node:assert/strict';
import { Client, rowShape, route, mutations } from './client.mjs';
import { ENTITIES, ENUMS, KEYS, WORKFLOWS, COMPUTED, PASSWORD, INVALID, integer, equalId, seed, computed, canRead, canWrite, AGENT_WRITES } from './contract.mjs';
export function parseCsv(text){const rows=[];let row=[],field='',quoted=false;for(let i=0;i<text.length;i++){const c=text[i];if(quoted){if(c==='\"'&&text[i+1]==='\"'){field+='\"';i++;}else if(c==='\"')quoted=false;else field+=c;}else if(c==='\"')quoted=true;else if(c===','){row.push(field);field='';}else if(c==='\n'){row.push(field.replace(/\r$/,''));rows.push(row);row=[];field='';}else field+=c;}assert.equal(quoted,false,'unterminated CSV quote');if(field||row.length){row.push(field.replace(/\r$/,''));rows.push(row);}return rows;}
const errors = response => {assert.equal(response.status,400,response.text);const values=response.json?.errors;assert.ok(Array.isArray(values)&&values.length,'400 reports errors');return values;};
export class Probe {
  constructor() {this.rows=[];}
  async check(id, group, operation) {try {await operation();this.rows.push({id,group,status:'passed'});return true;}catch(error){this.rows.push({id,group,status:group.startsWith('browser.')&&/locator|column .* visible|Timeout/.test(String(error))?'unverified':'failed',error:String(error.stack??error).slice(0,2500)});return false;}}
}
export async function apiChecks({base,arm,restart,probe}) {
  const admin=new Client(base,arm),refs={},initial={},roles={},inputs={};let ordinal=100;
  const make = (entity, changes={}) => ({...seed(entity,refs,++ordinal),...changes});
  const create = async(entity,body,client=admin) => {const response=await client.entity('POST',entity,undefined,body,201);rowShape(entity,response.json);for(const field of Object.keys(ENTITIES[entity]))assert.equal(String(response.json[field]),String(body[field]?.rawJSON??body[field]),`${entity}.${field} stored input unchanged`);return response.json;};
  const replace = async(entity,row,body,client=admin,status=200) => {const result=await client.entity('PUT',entity,row.id,body,status);if(status===200){rowShape(entity,result.json);for(const field of Object.keys(ENTITIES[entity]))assert.equal(String(result.json[field]),String(body[field]?.rawJSON??body[field]),`${entity}.${field} replacement retained`);}return result;};
  const stored = (entity,row) => ({...Object.fromEntries(Object.keys(ENTITIES[entity]).map(field=>[field,row[field]])),...(entity==='Member'?{password:''}:{})});
  const allRows = async()=>Object.fromEntries(await Promise.all(Object.keys(ENTITIES).map(async entity=>[entity,(await admin.entity('GET',entity,undefined,undefined,200)).json])));
  const setup=await probe.check('bootstrap','auth',async()=>{
    const email='admin@example.test';let account;
    if(arm==='typescript') {const response=await admin.request('POST','setup',{name:'Benchmark Admin',email,password:PASSWORD});assert.equal(response.status,201,response.text);account=response.json;refs.Team=account.team_id;}
    else {const status=await admin.me();assert.equal(status.status,200,status.text);assert.equal(status.json.setup,true);const team=await create('Team',make('Team'));refs.Team=team.id;account=await create('Member',make('Member',{team_id:team.id,name:'Benchmark Admin',email,role:'Admin'}));}
    account=await admin.login(email,PASSWORD);refs.Member=account.id;roles.Admin={client:admin,account,email};
    for(const entity of Object.keys(ENTITIES)) {
      let row;if(entity==='Member')row=account;else if(entity==='Team')row=(await admin.entity('GET',entity,refs.Team,undefined,200)).json;else row=await create(entity,make(entity));
      refs[entity]=row.id;initial[entity]=row;inputs[entity]=stored(entity,row);
    }
  });
  if(!setup)return {ready:false};
  for(const entity of Object.keys(ENTITIES)) await probe.check(`${entity}.shape`,'entities.fields.types',async()=>{
    const row=(await admin.entity('GET',entity,refs[entity],undefined,200)).json;rowShape(entity,row);
    for(const [field,type]of Object.entries(ENTITIES[entity]))if(ENUMS[type])assert.ok(ENUMS[type].includes(row[field]),`${entity}.${field} enum`);
    for(const field of COMPUTED[entity]??[])assert.ok(Object.hasOwn(row,field),`${entity}.${field} computed omitted`);
    const list=(await admin.entity('GET',entity,undefined,undefined,200)).json;assert.ok(list.some(item=>equalId(item.id,row.id)));await admin.entity('GET',entity,'9223372036854775807',undefined,404);
  });
  for(const entity of Object.keys(ENTITIES))await probe.check(`${entity}.crud-roundtrip`,'entities.fields.types',async()=>{const row=await create(entity,make(entity)),body=stored(entity,row);for(const [field,type]of Object.entries(ENTITIES[entity])){if(type.startsWith('ref:')){const target=type.slice(4),foreign=await create(target,make(target));body[field]=foreign.id;}else if(type==='string')body[field]=String(body[field])+'X';else if(type==='bool')body[field]=!body[field];else if(type==='int')body[field]=String(integer(body[field])+1n);else if(type==='float')body[field]+=1;else if(WORKFLOWS[entity]?.[0]===field)body[field]=WORKFLOWS[entity][1].find(([from])=>from===row[field])[1];else body[field]=ENUMS[type][(ENUMS[type].indexOf(row[field])+1)%ENUMS[type].length];}await replace(entity,row,body);const read=(await admin.entity('GET',entity,row.id,undefined,200)).json;for(const field of Object.keys(ENTITIES[entity]))assert.equal(String(read[field]),String(body[field]));await admin.entity('DELETE',entity,row.id,undefined,204);await admin.entity('GET',entity,row.id,undefined,404);});
  let boundaryIndex=0;for(const [entity,changes] of [['Customer',{seats:1,tier:'Free'}],['Customer',{seats:100,tier:'Pro'}],['Project',{budget:0,start_day:0,due_day:0,status:'Done'}],['Project',{start_day:100,due_day:100}],['Milestone',{due_day:0,done:false}],['Sprint',{start_day:-30,end_day:0}],['Sprint',{start_day:0,end_day:0}],['Task',{estimate:0,spent:0}],['Task',{estimate:1000,spent:3000}],['TimeEntry',{hours:1,rate:0,billable:false}],['TimeEntry',{hours:24}],['Ticket',{sla_hours:1,age_hours:0,severity:'Minor'}],['Invoice',{amount:0}],['Payment',{amount:0.01,day:0}],['Expense',{amount:0.01,day:0}],['Asset',{cost:0}],['Leave',{start_day:0,end_day:0}],['Leave',{start_day:0,end_day:30}],['Release',{day:0}]])await probe.check(`numeric-valid-${boundaryIndex++}`,'validation.every-rule',async()=>{const row=await create(entity,make(entity,changes));for(const [field,value]of Object.entries(changes))assert.equal(String(row[field]),String(value));});
  for(const entity of Object.keys(ENTITIES)) for(const [field,type]of Object.entries(ENTITIES[entity])) {
    await probe.check(`${entity}.${field}.type`,'entities.fields.types',async()=>{
      const bad=type==='string'?17:type==='bool'?'wrong':type==='float'?'wrong':ENUMS[type]?'NotACase':1.5;
      errors(await admin.entity('POST',entity,undefined,make(entity,{[field]:bad})));const absent=make(entity);delete absent[field];errors(await admin.entity('POST',entity,undefined,absent));if(type==='int'||type.startsWith('ref:'))for(const value of ['9223372036854775808','-9223372036854775809'])errors(await admin.entity('POST',entity,undefined,make(entity,{[field]:JSON.rawJSON(value)})));const before=(await admin.entity('GET',entity,refs[entity],undefined,200)).text;
      errors(await admin.entity('PUT',entity,refs[entity],{...inputs[entity],[field]:bad}));assert.equal((await admin.entity('GET',entity,refs[entity],undefined,200)).text,before);
    });
    if(type.startsWith('ref:'))await probe.check(`${entity}.${field}.reference`,'references',async()=>{
      await create(entity,make(entity,{[field]:refs[type.slice(4)]}));errors(await admin.entity('POST',entity,undefined,make(entity,{[field]:JSON.rawJSON('9223372036854775807')})));
      errors(await admin.entity('PUT',entity,refs[entity],{...inputs[entity],[field]:JSON.rawJSON('9223372036854775807')}));await admin.entity('DELETE',type.slice(4),refs[type.slice(4)],undefined,409);
    });
  }
  for(const [entity,name,changes,count]of INVALID) await probe.check(`${entity}.${name}`,'validation.'+(name==='all-errors'?'all-errors':name.endsWith('.utf8')?'utf8':'every-rule'),async()=>{
    const before=(await admin.entity('GET',entity,refs[entity],undefined,200)).text;
    assert.ok(errors(await admin.entity('POST',entity,undefined,make(entity,changes))).length>=count,'all violated rules reported on create');
    assert.ok(errors(await admin.entity('PUT',entity,refs[entity],{...inputs[entity],...changes})).length>=count,'all violated rules reported on update');assert.equal((await admin.entity('GET',entity,refs[entity],undefined,200)).text,before);
  });
  // Valid endpoints are independently exercised, so rejecting every mutation
  // cannot satisfy the negative-validation corpus.
  for(const [entity,name,changes]of INVALID.filter(([,name])=>name.endsWith('.short')||name.endsWith('.long')))await probe.check(`${entity}.${name}.valid-endpoint`,'validation.every-rule',async()=>{
    const field=Object.keys(changes)[0],text=changes[field],length=name.endsWith('.short')?text.length+1:text.length-1;
    const valid='x'.repeat(length),body=make(entity,{[field]:valid});
    if(field==='serial')body.serial='S'+String(++ordinal).padStart(length-1,'0');
    if(field==='code')body.code=String(++ordinal).slice(-length);if(field==='version')body.version=String(++ordinal).slice(-length);
    await create(entity,body);
  });
  for(const [entity,fields]of Object.entries(KEYS))await probe.check(`${entity}.keys`,'keys',async()=>{
    const body=make(entity);for(const field of fields)body[field]=initial[entity][field];errors(await admin.entity('POST',entity,undefined,body));
    const other=await create(entity,make(entity));const before=(await admin.entity('GET',entity,other.id,undefined,200)).text;
    const updated=stored(entity,other);for(const field of fields)updated[field]=initial[entity][field];errors(await replace(entity,other,updated,admin,400));assert.equal((await admin.entity('GET',entity,other.id,undefined,200)).text,before);
  });
  for(const [entity,[field,edges]]of Object.entries(WORKFLOWS)) {
    const states=ENUMS[ENTITIES[entity][field]],path=target=>{const queue=[[states[0]]];while(queue.length){const route=queue.shift();if(route.at(-1)===target)return route;for(const [from,to]of edges)if(from===route.at(-1)&&!route.includes(to))queue.push([...route,to]);}throw new Error('unreachable workflow fixture');};
    for(const state of states.slice(1))await probe.check(`${entity}.create.${state}`,'workflows',async()=>errors(await admin.entity('POST',entity,undefined,make(entity,{[field]:state}))));
    for(const from of states)for(const to of states)await probe.check(`${entity}.${from}.${to}`,'workflows',async()=>{
      let row=await create(entity,make(entity));for(const next of path(from).slice(1))row=(await replace(entity,row,{...stored(entity,row),[field]:next})).json;
      const body={...stored(entity,row),[field]:to},allowed=from===to||edges.some(pair=>pair[0]===from&&pair[1]===to);
      const response=await replace(entity,row,body,admin,allowed?200:400);if(!allowed)assert.equal((await admin.entity('GET',entity,row.id,undefined,200)).json[field],from);else assert.equal(response.json[field],to);
    });
  }
  for(const entity of Object.keys(ENTITIES))for(const [field,type]of Object.entries(ENTITIES[entity]))if(ENUMS[type]&&WORKFLOWS[entity]?.[0]!==field)for(const value of ENUMS[type])await probe.check(`${entity}.${field}.${value}.enum`,'entities.fields.types',async()=>{const row=await create(entity,make(entity,{[field]:value}));assert.equal(row[field],value);});
  await probe.check('all-computed-and-rollups','computed',async()=>{const rows=await allRows();for(const [entity,values]of Object.entries(rows))for(const row of values){rowShape(entity,row);for(const [field,value]of Object.entries(computed(entity,row,rows)))assert.deepEqual(row[field],value,`${entity}.${field}`);}});
  await probe.check('rollup-mutation-refresh','rollups',async()=>{
    const assertTotals=async phase=>{const rows=await allRows();for(const entity of ['Team','Project','Customer','Invoice','Member'])for(const row of rows[entity])for(const [field,value]of Object.entries(computed(entity,row,rows)))assert.deepEqual(row[field],value,`${phase} ${entity}.${field}`);};
    for(const [entity,changes]of [['Member',{name:'Renamed account'}],['Task',{spent:9}],['Expense',{amount:23}],['Ticket',{state:'Resolved',age_hours:0}],['Invoice',{amount:230}],['Payment',{amount:13}],['TimeEntry',{hours:5}]]){const row=await create(entity,make(entity));await assertTotals(entity+' create');await replace(entity,row,{...stored(entity,row),...changes});await assertTotals(entity+' update');await admin.entity('DELETE',entity,row.id,undefined,204);await assertTotals(entity+' delete');}
  });
  for(const [entity,changes]of [['Sprint',{start_day:JSON.rawJSON('-9223372036854775808'),end_day:JSON.rawJSON('-9223372036854775807')}],['Project',{start_day:JSON.rawJSON('9223372036854775807'),due_day:JSON.rawJSON('9223372036854775807')}],['Ticket',{age_hours:JSON.rawJSON('9007199254740993')}]])await probe.check(`${entity}.signed64`,'i64.exact',async()=>{
    const row=await create(entity,make(entity,changes));for(const [field,raw]of Object.entries(changes))assert.equal(integer(row[field]).toString(),raw.rawJSON);const read=(await admin.entity('GET',entity,row.id,undefined,200)).json;for(const [field,raw]of Object.entries(changes))assert.equal(integer(read[field]).toString(),raw.rawJSON);
  });
  for(const role of ['Manager','Agent','Viewer'])await probe.check(`account.${role}`,'auth',async()=>{
    const email=`${role.toLowerCase()}@example.test`,account=await create('Member',make('Member',{role,email})),client=new Client(base,arm);await client.login(email,PASSWORD);assert.equal((await client.me()).status,200);roles[role]={client,account,email};
  });
  const second=await create('Member',make('Member',{role:'Agent',email:'agent-other@example.test'}));roles.OtherAgent={client:new Client(base,arm),account:second,email:second.email};await roles.OtherAgent.client.login(second.email,PASSWORD);
  for(const role of ['Admin','Manager','Agent','Viewer'])for(const entity of Object.keys(ENTITIES))await probe.check(`${role}.${entity}.matrix`,'roles',async()=>{
    const {client,account}=roles[role],body=make(entity,ENTITIES[entity].member_id?{member_id:account.id}:{}),readable=canRead(role,entity,initial[entity],account.id);
    const list=(await client.entity('GET',entity,undefined,undefined,200)).json,all=(await admin.entity('GET',entity,undefined,undefined,200)).json;assert.deepEqual(list.map(row=>String(row.id)).sort(),all.filter(row=>canRead(role,entity,row,account.id)).map(row=>String(row.id)).sort(),'every row obeys independent visibility predicate');assert.equal(list.some(row=>equalId(row.id,refs[entity])),readable,'list visibility');await client.entity('GET',entity,refs[entity],undefined,readable?200:404);
    const writable=canWrite(role,entity,body,account.id),response=await client.entity('POST',entity,undefined,body,writable?201:403);
    if(writable){const row=response.json;await replace(entity,row,stored(entity,row),client);await client.entity('DELETE',entity,row.id,undefined,204);}
    else {await client.entity('PUT',entity,refs[entity],inputs[entity],readable?403:404);await client.entity('DELETE',entity,refs[entity],undefined,readable?403:404);}
  });
  for(const entity of AGENT_WRITES)await probe.check(`${entity}.own-other`,'own-other-rows',async()=>{
    const a=roles.Agent,b=roles.OtherAgent,own=await create(entity,make(entity,{member_id:a.account.id}),a.client),other=await create(entity,make(entity,{member_id:b.account.id}),b.client);
    await a.client.entity('GET',entity,own.id,undefined,200);await a.client.entity('GET',entity,other.id,undefined,entity==='Expense'?404:200);await replace(entity,own,stored(entity,own),a.client);
    await replace(entity,own,{...stored(entity,own),member_id:b.account.id},a.client,403);
    await replace(entity,other,{...stored(entity,other),member_id:a.account.id},a.client,entity==='Expense'?404:403);await a.client.entity('DELETE',entity,other.id,undefined,entity==='Expense'?404:403);
    if(entity==='Expense'||entity==='Leave'){const field='state';await replace(entity,own,{...stored(entity,own),[field]:'Approved'},a.client,403);await a.client.entity('POST',entity,undefined,make(entity,{member_id:a.account.id,[field]:'Approved'}),403);let approved=other;if(entity==='Expense')approved=(await replace(entity,approved,{...stored(entity,approved),state:'Submitted'})).json;approved=(await replace(entity,approved,{...stored(entity,approved),state:'Approved'})).json;await replace(entity,approved,{...stored(entity,approved),state:entity==='Expense'?'Draft':'Requested'},b.client,403);await b.client.entity('DELETE',entity,approved.id,undefined,403);}
    await a.client.entity('DELETE',entity,own.id,undefined,204);
  });
  await probe.check('password-edit','auth',async()=>{const account=await create('Member',make('Member',{email:'password-edit@example.test'})),body={...stored('Member',account),password:PASSWORD+'-changed'};await replace('Member',account,body);const client=new Client(base,arm);assert.equal((await client.request('POST','session',{[arm==='semaprax'?'login':'email']:account.email,password:PASSWORD})).status,401);await client.login(account.email,PASSWORD+'-changed');});
  for(const entity of Object.keys(ENTITIES))await probe.check(`${entity}.anonymous-all-routes`,'auth',async()=>{const anonymous=new Client(base,arm),before=(await admin.entity('GET',entity,undefined,undefined,200)).text;for(const [method,id,body]of [['GET',undefined,undefined],['GET',refs[entity],undefined],['POST',undefined,make(entity)],['PUT',refs[entity],inputs[entity]],['DELETE',refs[entity],undefined]])await anonymous.entity(method,entity,id,body,401);for(const suffix of ['?format=csv',`/${refs[entity]}/history`])assert.equal((await anonymous.request('GET',route(arm,entity)+suffix)).status,401);assert.equal((await anonymous.request('GET','audit')).status,401);assert.equal((await admin.entity('GET',entity,undefined,undefined,200)).text,before,'anonymous requests never mutate rows');});
  await probe.check('sign-in-out-inactive','auth',async()=>{
    const anonymous=new Client(base,arm);for(const entity of Object.keys(ENTITIES))await anonymous.entity('GET',entity,undefined,undefined,401);const wrong=await anonymous.request('POST','session',{[arm==='semaprax'?'login':'email']:'admin@example.test',password:'wrong'});assert.equal(wrong.status,401);
    const inactive=await create('Member',make('Member',{active:false}));const response=await anonymous.request('POST','session',{[arm==='semaprax'?'login':'email']:inactive.email,password:PASSWORD});assert.equal(response.status,401);assert.equal((await roles.Viewer.client.request('DELETE','session')).status,204);assert.equal((await roles.Viewer.client.me()).status,401);await roles.Viewer.client.login(roles.Viewer.email,PASSWORD);
  });
  await probe.check('audit-create-update-delete','audit',async()=>{
    const body=make('Vendor'),row=await create('Vendor',body),changed={...body,name:'Changed vendor'};await replace('Vendor',row,changed);await admin.entity('DELETE','Vendor',row.id,undefined,204);
    const log=(await admin.request('GET','audit')).json;assert.ok(Array.isArray(log));const entries=log.filter(e=>equalId(e.id,row.id)&&String(e.entity).replaceAll('_','').toLowerCase()==='vendor');assert.equal(entries.length,3);assert.deepEqual(entries.map(e=>e.action),['create','update','delete']);for(const entry of entries){assert.ok(entry.time??entry.at);assert.ok(equalId(entry.member_id??entry.by??entry.member,roles.Admin.account.id));assert.ok(entry.changes&&Object.keys(entry.changes).length);}
    const update=entries[1].changes.name;assert.deepEqual(update,[body.name,changed.name]);for(const role of ['Manager','Agent','Viewer'])assert.equal((await roles[role].client.request('GET','audit')).status,403);
    for(const mutation of mutations){const entries=log.filter(e=>equalId(e.id,mutation.id)&&String(e.entity).replaceAll('_','').toLowerCase()===mutation.entity.toLowerCase()&&e.action===mutation.action);assert.ok(entries.length,`every ${mutation.action} ${mutation.entity} is audited`);if(mutation.actor)assert.ok(entries.some(e=>equalId(e.member_id??e.by??e.member,mutation.actor)),'audit actual actor');}
    const existing=(await admin.request('GET',`${route(arm,'Vendor')}/${refs.Vendor}/history`)).json;assert.ok(Array.isArray(existing)&&existing.length>=1);
  });
  for(const entity of Object.keys(ENTITIES))await probe.check(`${entity}.csv`,'csv',async()=>{
    const response=await admin.request('GET',`${route(arm,entity)}?format=csv`);assert.equal(response.status,200);assert.match(response.headers.get('content-type'),/text\/csv/);for(const field of ['id',...Object.keys(ENTITIES[entity]),...(COMPUTED[entity]??[])])assert.ok(response.text.split(/\r?\n/,1)[0].split(',').includes(field),`${entity} CSV column ${field}`);
    assert.ok(response.text.includes(String(refs[entity])));const history=await admin.request('GET',`${route(arm,entity)}/${refs[entity]}/history`);assert.equal(history.status,200);assert.ok(Array.isArray(history.json)&&history.json.length>0,'every entity has row history');for(const role of ['Manager','Agent','Viewer']){const actor=roles[role],actual=await actor.client.request('GET',`${route(arm,entity)}?format=csv`);assert.equal(actual.status,200);const rows=parseCsv(actual.text),index=rows[0].indexOf('id'),all=(await admin.entity('GET',entity,undefined,undefined,200)).json;assert.deepEqual(rows.slice(1).map(row=>row[index]).sort(),all.filter(row=>canRead(role,entity,row,actor.account.id)).map(row=>String(row.id)).sort(),'every CSV row uses independent visibility');}const absent=await admin.request('GET',`${route(arm,entity)}?format=csv&q=__absent_search_659__`);assert.equal(absent.status,200);assert.equal(absent.text.trim().split(/\r?\n/).length,1);
    for(const [field,type]of Object.entries(ENTITIES[entity]))if(ENUMS[type]){const chosen=ENUMS[type][0],filtered=await admin.request('GET',`${route(arm,entity)}?format=csv&${field}=${chosen}`);assert.equal(filtered.status,200);assert.ok(parseCsv(filtered.text).slice(1).every(row=>row[parseCsv(filtered.text)[0].indexOf(field)]===chosen));}
  });
  await probe.check('csv-quotation-and-row-hiding','csv',async()=>{
    const comments=await admin.request('GET',`${route(arm,'Comment')}?format=csv`);assert.ok(comments.text.includes('"Comment, ""quoted""\n😀"'));const viewer=await roles.Viewer.client.request('GET',`${route(arm,'Expense')}?format=csv`);assert.equal(viewer.status,200);assert.equal(viewer.text.trim().split(/\r?\n/).length,1);const agent=await roles.Agent.client.request('GET',`${route(arm,'Expense')}?format=csv`);assert.ok(!agent.text.includes(String(refs.Expense)+','));
  });
  await probe.check('deleted-ids-never-reused','nonreused-id',async()=>{const row=await create('Vendor',make('Vendor'));await admin.entity('DELETE','Vendor',row.id,undefined,204);const next=await create('Vendor',make('Vendor'));assert.ok(integer(next.id)>integer(row.id));refs.deletedVendor=row.id;refs.lastVendor=next.id;});
  await probe.check('restart-accounts-rows-audit','durability',async()=>{
    const before=await allRows(),history=(await admin.request('GET','audit')).json;await restart();await admin.login('admin@example.test',PASSWORD);assert.deepEqual(await allRows(),before);assert.deepEqual((await admin.request('GET','audit')).json,history);await admin.entity('GET','Vendor',refs.deletedVendor,undefined,404);const next=await create('Vendor',make('Vendor'));assert.ok(integer(next.id)>integer(refs.lastVendor));
  });
  return {ready:true,admin,refs,initial,roles,make,create,allRows,stored};
}
