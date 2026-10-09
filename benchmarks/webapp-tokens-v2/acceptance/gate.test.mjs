import test from 'node:test';
import {createServer} from 'node:http';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { createRequire } from 'node:module';
import { Client,rowShape,lossless } from './client.mjs';
import { ENTITIES,ENUMS,COVERAGE,SPEC_SHA256,integer,seed,canRead,canWrite,computed } from './contract.mjs';
import { sha256,requiredCases,qualify,passwordChecks } from './qualification.mjs';
import { localUrl,readinessPath,readyResponse,launch,finish,unusedPort } from './process.mjs';
import {direction} from './ordering.mjs';
import { parseCsv,csvColumns,auditChangeValues,auditEventKind,Probe,deniedWriteStatuses,auditRowId } from './api.mjs';
test('denied writes keep readable targets strict and audit IDs resolve the affected row',()=>{assert.deepEqual(deniedWriteStatuses(true),[403]);assert.deepEqual(deniedWriteStatuses(false),[403,404]);for(const status of [200,201,204]){assert.equal(deniedWriteStatuses(true).includes(status),false);assert.equal(deniedWriteStatuses(false).includes(status),false);}assert.equal(auditRowId({id:7,row_id:31,record_id:32}),31);assert.equal(auditRowId({id:7,record_id:32}),32);assert.equal(auditRowId({id:7}),7);});
test('frozen SPEC byte identity and exactly20 independently named entities',async()=>{assert.equal(sha256(await fs.readFile(new URL('../SPEC.md',import.meta.url))),SPEC_SHA256);assert.equal(Object.keys(ENTITIES).length,20);assert.equal(Object.keys(ENUMS).length,12);});
test('omitting any stored field fails independent row shape',()=>{const refs=Object.fromEntries(Object.keys(ENTITIES).map(name=>[name,1]));for(const entity of Object.keys(ENTITIES)){const row={...seed(entity,refs,17),id:1};delete row.password;rowShape(entity,row);for(const field of Object.keys(ENTITIES[entity])){const missing={...row};delete missing[field];assert.throws(()=>rowShape(entity,missing),`${entity}.${field}`);}}});
test('exact signed64 witness preserves unsafe numeric lexemes',()=>{const row=lossless('{"id":1,"start_day":-9223372036854775808,"due_day":9223372036854775807,"age_hours":9007199254740993}');assert.equal(integer(row.start_day),-(1n<<63n));assert.equal(integer(row.due_day),(1n<<63n)-1n);assert.equal(integer(row.age_hours),9007199254740993n);assert.throws(()=>integer(9007199254740993));assert.throws(()=>integer('9223372036854775808'));for(const field of ['id','start_day','due_day','age_hours','member_id'])assert.throws(()=>lossless(JSON.stringify({[field]:'1'})),'quoted integer JSON cannot pass');assert.equal(integer(lossless('{\"id\":1.0}').id),1n);assert.equal(integer(lossless('{\"age_hours\":9007199254740993e0}').age_hours),9007199254740993n);});
test('weakened cross-owner and approval policies cannot satisfy independent oracle',()=>{const row={member_id:1,state:'Draft'};assert.equal(canWrite('Agent','Expense',row,2),false);assert.equal(canRead('Agent','Expense',row,2),false);assert.equal(canWrite('Agent','Expense',{...row,state:'Approved'},1),false);assert.equal(canWrite('Agent','Leave',{...row,state:'Approved'},1),false);assert.equal(canWrite('Manager','Team',row,1),false);assert.equal(canWrite('Viewer','Task',row,1),false);assert.equal(canRead('Viewer','Invoice',row,1),false);});
test('dropping any mandatory case or coverage family prevents qualification',()=>{const inventory=requiredCases();assert.equal(new Set(inventory).size,inventory.length);const rows=inventory.map(id=>({id,status:'passed',group:COVERAGE[0]}));for(const group of COVERAGE.slice(1))rows.push({id:`fixture-${group}`,group,status:'passed'});assert.equal(qualify(rows).passed,true);for(const id of inventory)assert.equal(qualify(rows.filter(row=>row.id!==id)).passed,false,id);for(const group of COVERAGE)assert.equal(qualify(rows.filter(row=>row.group!==group)).passed,false,group);const changed=structuredClone(rows);changed[0].status='unverified';assert.equal(qualify(changed).passed,false);});
test('independent computed branch and rollup witnesses reject omitted semantics',()=>{const all=Object.fromEntries(Object.keys(ENTITIES).map(name=>[name,[]]));all.Task=[{project_id:1,status:'Done',spent:'7'}];all.Expense=[{project_id:1,amount:11}];assert.deepEqual(computed('Project',{id:1,start_day:'0',due_day:'101',status:'Active',budget:10},all),{duration:'101',late:false,tasks:'1',open_tasks:'0',spent:'7',expenses:11,over_budget:true});assert.equal(computed('Task',{status:'Done',estimate:'10',spent:'12',priority:'Urgent'},all).remaining,'0');assert.equal(computed('Ticket',{state:'Open',age_hours:'20',sla_hours:'1',severity:'Major'},all).escalation,'watch');});
test('CSV preserves commas quotes newlines, launch excludes remote authority',()=>{assert.deepEqual(parseCsv('id,body\r\n1,"a, ""b""\nc"\r\n'),[['id','body'],['1','a, "b"\nc']]);assert.throws(()=>parseCsv('"unterminated'));assert.equal(localUrl('http://127.0.0.1:8123/'),'http://127.0.0.1:8123/');for(const url of ['https://127.0.0.1:8123/','http://example.com:8123/','http://127.0.0.1:8123/path'])assert.throws(()=>localUrl(url));});
test('failed and missing cases stay failures, unsupported KDF remains unverified',async()=>{const probe=new Probe();await probe.check('negative','auth',()=>assert.fail('deliberately omitted behavior'));assert.equal(probe.rows[0].status,'failed');assert.equal(qualify(probe.rows).passed,false);const root=await fs.mkdtemp(path.join(os.tmpdir(),'teamdesk-gate-test-'));try{await fs.writeFile(path.join(root,'state.sqlite'),'opaque independent-proof-required data');const result=await passwordChecks({data:root,ledger:path.join(root,'missing-ledger')});assert.equal(result.verified,false);}finally{await fs.rm(root,{recursive:true,force:true});}});

test('hostile API missing fields and weakened write responses are observed externally',async()=>{const server=createServer((req,res)=>{res.setHeader('content-type','application/json');if(req.method==='POST'){res.writeHead(201);res.end(JSON.stringify({id:1}));}else{res.writeHead(200);res.end(JSON.stringify({id:1,name:'Team without mandatory description'}));}});await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));try{const client=new Client(`http://127.0.0.1:${server.address().port}/`,'typescript');const row=await client.entity('GET','Team',1,undefined,200);assert.throws(()=>rowShape('Team',row.json));const body={member_id:2};assert.equal(canWrite('Agent','Task',body,1),false);await assert.rejects(client.entity('POST','Task',undefined,body,403));}finally{await new Promise(resolve=>server.close(resolve));}});

test('actual sort order and pagination omissions cannot satisfy ordering oracle',()=>{const rows=Array.from({length:27},(_,n)=>({id:String(n+1),value:String(n+1)})),ids=rows.map(row=>row.id);assert.equal(direction(ids,rows,'value','int'),-1);assert.equal(direction(ids.slice().reverse(),rows,'value','int'),1);const wrong=ids.slice();[wrong[24],wrong[25]]=[wrong[25],wrong[24]];assert.throws(()=>direction(wrong,rows,'value','int'),'wrong boundary order');assert.throws(()=>direction(ids.slice(0,25),rows,'value','int'),'dropped page');const duplicate=ids.slice();duplicate[26]=duplicate[25];assert.throws(()=>direction(duplicate,rows,'value','int'),'duplicate row');});

import {actionControl,numericEditor,directPage,entityHeadingPattern,enumFilterSelectors,formControl,formFieldLabel,historyValueProof,renderedErrorMessages,searchControl,signInLabel,uniqueControl,visibleErrorMessageCount} from './browser-support.mjs';
test('exact numeric editors are accepted without accepting untyped strings',()=>{numericEditor('int',{type:'text',inputmode:'numeric'});numericEditor('int',{type:'number',step:'1'});numericEditor('float',{type:'number',step:'any'});assert.throws(()=>numericEditor('int',{type:'text'}));assert.throws(()=>numericEditor('float',{type:'text',inputmode:'numeric'}));assert.throws(()=>numericEditor('int',{type:'number',step:'0.1'}));});
test('direct route awaits a fresh document rather than earlier hash networkidle',async()=>{const calls=[],page={goto:async(...args)=>calls.push(args)};await directPage(page,'http://127.0.0.1:1234/#/task/1/edit');assert.deepEqual(calls,[['about:blank'],['http://127.0.0.1:1234/#/task/1/edit',{waitUntil:'networkidle'}]]);});


test('CSV header representation preserves exact columns and rejects malformed data',()=>{
  for (const header of ['id,name', '"id","name"', '\uFEFF"id","name"'])
    assert.deepEqual(csvColumns(header+'\r\n1,"a, ""b""\nc"\r\n',['id','name']),[['id','name'],['1','a, "b"\nc']]);
  assert.deepEqual(parseCsv('id,name\n1,\n'),[['id','name'],['1','']]);
  assert.deepEqual(parseCsv('id,name\n1,""'),[['id','name'],['1','']]);
  for (const invalid of ['"id"junk,name\n1,x', 'i"d,name\n1,x', 'id,name\n1', 'id,name\n1,x,y', 'id,name\r1,x', 'id,id\n1,2', 'name\nx', ''])
    assert.throws(()=>csvColumns(invalid,['id','name']),invalid);
});
test('audit old/new representations preserve exact values and reject wrong changes',()=>{
  for (const change of [['before','after'],{old:'before',new:'after'}]) assert.deepEqual(auditChangeValues(change),['before','after']);
  for (const change of [null, 'before', ['before'], ['before','after','extra'], {old:'before'}, {new:'after'}, {old:'before',new:'after',extra:1}])
    assert.throws(()=>auditChangeValues(change));
  for (const change of [['after','before'],{old:'after',new:'before'},{old:'before',new:'wrong'}])
    assert.throws(()=>assert.deepEqual(auditChangeValues(change),['before','after']));
});
test('audit event kind accepts representation freedom only when old/new evidence is complete',()=>{
  const fields={name:'string',email:'string'};
  const event=(changes,action)=>action===undefined?{changes}:{changes,action};
  assert.equal(auditEventKind(event({name:[null,'Vendor'],email:[null,'vendor@example.test']}),'Vendor',fields),'create');
  assert.equal(auditEventKind(event({name:['Vendor',null],email:['vendor@example.test',null]}),'Vendor',fields),'delete');
  assert.equal(auditEventKind(event({name:['Vendor','Renamed'],email:['vendor@example.test','next@example.test']},'update'),'Vendor',fields),'update');
  assert.equal(auditEventKind(event({name:[null,'Renamed']},'update'),'Vendor',fields),'update');
  assert.equal(auditEventKind(event({name:['Vendor','Renamed'],email:['vendor@example.test','vendor@example.test']}),'Vendor',fields),'update');
  for(const hostile of [
    event({}),
    event({name:[null,'Vendor']}),
    event({name:[null,'Vendor'],email:['vendor@example.test','next@example.test']}),
    event({name:['Vendor','Renamed'],email:['vendor@example.test','next@example.test']},'create'),
    event({name:['Vendor','Renamed'],email:['vendor@example.test','next@example.test']},'merge'),
  ]) assert.throws(()=>auditEventKind(hostile,'Vendor',fields));
});
test('explicit update no-ops remain auditable witnesses while malformed events fail',()=>{
  const fields={title:'string',status:'enum'};
  const entries=[
    {action:'update',changes:{}},
    {action:'update',changes:{title:['Task','Task'],status:['Todo','Todo']}},
    {action:'update',changes:{title:[null,'Renamed']}},
    {action:'create',changes:{title:[null,'Task'],status:[null,'Todo']}},
    {action:'delete',changes:{title:['Task',null],status:['Todo',null]}},
  ];
  assert.deepEqual(entries.map(entry=>auditEventKind(entry,'Task',fields)),['update','update','update','create','delete']);
  for(const hostile of [
    {action:'create',changes:{}},
    {changes:{}},
    {action:'update',changes:null},
    {action:'create',changes:{title:['Task','Task'],status:['Todo','Doing']}},
  ]) assert.throws(()=>auditEventKind(hostile,'Task',fields));
});
test('readiness uses current member without setup and rejects unhealthy responses',async()=>{
  assert.equal(readinessPath('semaprax'),'api/session'); assert.equal(readinessPath('typescript'),'api/me');
  assert.throws(()=>readinessPath('unknown'));
  for (const status of [200,401]) assert.equal(await readyResponse(new Response('{}',{status})),true);
  for (const status of [302,403,404,500,503]) assert.equal(await readyResponse(new Response('{}',{status})),false);
  for (const body of ['not JSON','null','[]']) assert.equal(await readyResponse(new Response(body,{status:200})),false);
});


test('initialized TypeScript setup refusal does not prevent restart readiness',async()=>{
  const candidate=await fs.mkdtemp(path.join(os.tmpdir(),'teamdesk-ready-test-'));
  const env={...process.env,NODE_BINARY:process.execPath,TEAMDESK_ARM:'typescript',TEAMDESK_PORT:String(await unusedPort())};
  const log={write(){}}; let running;
  try {
    await fs.writeFile(path.join(candidate,'run.sh'),'#!/bin/sh\nexec "$NODE_BINARY" server.mjs\n');
    await fs.writeFile(path.join(candidate,'server.mjs'),`
      import {createServer} from 'node:http';
      import {existsSync} from 'node:fs';
      const server=createServer((req,res)=>{
        res.setHeader('content-type','application/json');
        const status=req.url==='/api/me'?401:req.url==='/api/setup'?(existsSync('initialized')?403:200):404;
        res.writeHead(status);res.end(JSON.stringify({error:status===403?'Setup is already complete':'sign in required'}));
      });
      server.listen(Number(process.env.TEAMDESK_PORT),'127.0.0.1',()=>{
        const url='http://127.0.0.1:'+server.address().port+'/';
        console.log(JSON.stringify({api_base_url:url,ui_base_url:url}));
      });
      process.on('SIGTERM',()=>server.close(()=>process.exit(0)));
    `);
    running=await launch({candidate,env,log});
    assert.equal((await fetch(running.api+'api/setup')).status,200);
    await fs.writeFile(path.join(candidate,'initialized'),'yes');
    await finish(running);running=null;
    running=await launch({candidate,env,log});
    assert.equal((await fetch(running.api+'api/setup')).status,403);
    assert.equal((await fetch(running.api+'api/me')).status,401);
  } finally {if(running)await finish(running);await fs.rm(candidate,{recursive:true,force:true});}
});


test('sign-in label capitalization preserves exact accessible field identity',()=>{
  for (const label of ['email','Email','EMAIL',' email ','\nEmail\t']) assert.equal(signInLabel('email').test(label),true);
  for (const label of ['password','Password','PASSWORD',' password ','\tPassword\n']) assert.equal(signInLabel('password').test(label),true);
  for (const label of ['recovery email','email address of another member','new password','password confirmation'])
    for (const field of ['email','password']) assert.equal(signInLabel(field).test(label),false);
  assert.throws(()=>signInLabel('.*'));
});

test('enum filters accept named controls or their legacy field-qualified clear option',()=>{
  const status=enumFilterSelectors('status'),priority=enumFilterSelectors('priority');
  for(const label of ['Status',' status ','STATUS'])assert.equal(status.name.test(label),true);
  for(const label of ['status: all',' Status: All '])assert.equal(status.legacyAll.test(label),true);
  for(const label of ['Priority','priority: all','All','ticket status'])assert.equal(status.name.test(label)||status.legacyAll.test(label),false);
  assert.equal(priority.name.test('Priority'),true);assert.equal(priority.legacyAll.test('priority: all'),true);
  assert.throws(()=>enumFilterSelectors('status: all'));
});

test('form fields accept admitted prefix labels on native controls while action names remain role-neutral',()=>{
  const contact=formFieldLabel('Contact'),teamId=formFieldLabel('team_id');
  for(const label of ['Contact',' contact ','CONTACT','Contact navigation','Contact details'])assert.equal(contact.test(label),true);
  for(const label of ['Amount (USD)',' amount: USD'])assert.equal(formFieldLabel('amount').test(label),true);
  for(const label of ['team id','team_id',' Team ID ','Team identifier'])assert.equal(teamId.test(label),true);
  assert.throws(()=>formFieldLabel('Contact details'));
});

test('actions union exact links and buttons while native form controls exclude navigation and duplicates',async()=>{
  const calls=[];
  const link={kind:'link',or(other){calls.push(['or',other.kind]);return {count:async()=>1};}};
  const button={kind:'button'};
  const field={kind:'label',and(other){calls.push(['and',other.selector]);return {count:async()=>1};}};
  const page={getByRole(role,options){calls.push(['role',role,options.name]);return role==='link'?link:button;},getByLabel(label){calls.push(['label',label]);return field;},locator(selector){return {selector};}};
  await uniqueControl(actionControl(page,'Edit'),'Edit');
  await uniqueControl(actionControl(page,'New','Task'),'New Task');
  await uniqueControl(formControl(page,'Contact'),'Contact');
  assert.equal(calls.filter(call=>call[0]==='role'&&call[1]==='link').length,2);
  assert.equal(calls.filter(call=>call[0]==='role'&&call[1]==='button').length,2);
  assert.deepEqual(calls.find(call=>call[0]==='and'),['and','input,select,textarea']);
  await assert.rejects(()=>uniqueControl({count:async()=>2},'Edit'),/one Edit control/);
});
test('search controls accept exact accessible Search inputs while refusing ambiguity',async()=>{
  const calls=[];
  const legacy={kind:'legacy',or(other){calls.push(['or',other.kind]);return {count:async()=>1};}};
  const textbox={kind:'textbox',and(other){calls.push(['and',other.selector]);return {kind:'native-input'};}};
  const page={
    locator(selector){return selector==='input[type="search"]:visible,input[placeholder="Search"]:visible'?legacy:{selector};},
    getByRole(role,options){calls.push(['role',role,options.name]);return textbox;},
  };
  await uniqueControl(searchControl(page),'Search');
  const name=calls.find(call=>call[0]==='role')[2];
  for(const label of ['Search',' search ','SEARCH'])assert.equal(name.test(label),true);
  for(const label of ['Search records','New Search'])assert.equal(name.test(label),false);
  assert.deepEqual(calls.find(call=>call[0]==='and'),['and','input:visible']);
  await assert.rejects(()=>uniqueControl({count:async()=>2},'Search'),/one Search control/);
});

test('visible error messages count distinct nonempty lines without splitting collapsed text',()=>{
  assert.equal(visibleErrorMessageCount(['Required name\nInvalid email','Invalid email','  ']),2);
  assert.equal(visibleErrorMessageCount(['Required name\r\nInvalid email','Required name','Invalid email']),2);
  assert.equal(visibleErrorMessageCount(['Required name Invalid email']),1);
  assert.throws(()=>visibleErrorMessageCount(['Required name',null]));
});

test('local browser fixture counts only rendered error leaves and exact Search controls',async()=>{
  const require=createRequire(path.join(process.env.PLAYWRIGHT_PACKAGE_ROOT??path.dirname(new URL(import.meta.url).pathname),'package.json')),{chromium}=require('@playwright/test');
  const browser=await chromium.launch({headless:true}),page=await browser.newPage();
  try {
    await page.setContent(`<label>Search<input type="text" aria-label="Search" placeholder="Search"></label>
      <div class="err"><span>Required name</span><span hidden>Hidden error</span></div>
      <div role="alert" style="white-space:normal">Collapsed one
Collapsed two</div>
      <div role="alert" style="white-space:pre-wrap">Invalid email
Bad phone</div>
      <div class="err"><div role="alert">Duplicate wrapper</div></div>`);
    const search=await uniqueControl(searchControl(page),'Search');
    assert.equal(await search.getAttribute('aria-label'),'Search');
    assert.equal(visibleErrorMessageCount(await renderedErrorMessages(page)),5);
    await page.setContent('<input aria-label="Search"><input aria-label="Search">');
    await assert.rejects(()=>uniqueControl(searchControl(page),'Search'),/one Search control/);
  } finally {await browser.close();}
});

test('history proof requires distinct old and new values',()=>{
  const oldValue='audit-old-marker-659',newValue='audit-new-marker-659';
  historyValueProof(`History\nname: ${oldValue} → ${newValue}`,oldValue,newValue);
  assert.throws(()=>historyValueProof(`History\nname: ${newValue}`,oldValue,newValue));
  assert.throws(()=>historyValueProof(`History\nname: ${oldValue}`,oldValue,oldValue));
});

test('local browser fixture accepts colon destination headings without accepting prefixes',async()=>{
  const require=createRequire(path.join(process.env.PLAYWRIGHT_PACKAGE_ROOT??path.dirname(new URL(import.meta.url).pathname),'package.json')),{chromium}=require('@playwright/test');
  const browser=await chromium.launch({headless:true}),page=await browser.newPage(),pattern=entityHeadingPattern('Customer');
  try {
    await page.setContent('<h1>Customer: Acme</h1>');
    assert.equal(await page.getByRole('heading',{name:pattern}).count(),1);
    await page.setContent('<h1>Customer Acme</h1>');
    assert.equal(await page.getByRole('heading',{name:pattern}).count(),1);
    await page.setContent('<h1>CustomerOther</h1><h1 hidden>Customer: Hidden</h1>');
    assert.equal(await page.getByRole('heading',{name:pattern}).count(),0);
  } finally {await browser.close();}
});
