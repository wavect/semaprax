import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import { createWriteStream } from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { parseArgs } from 'node:util';
import { Probe,apiChecks } from './api.mjs';
import { browserChecks } from './browser.mjs';
import { SPEC_SHA256 } from './contract.mjs';
import { sha256,qualify,passwordChecks } from './qualification.mjs';
import { unusedPort,launch,finish,start,tree } from './process.mjs';
const directory=path.dirname(fileURLToPath(import.meta.url));
const {values}=parseArgs({options:{candidate:{type:'string'},arm:{type:'string'},output:{type:'string'},compiler:{type:'string'},'compiler-source-sha':{type:'string'},'playwright-root':{type:'string'}}});
assert.ok(values.candidate&&values.output&&['semaprax','typescript'].includes(values.arm),'--candidate --output --arm semaprax|typescript required');
assert.ok(Number(process.versions.node.split('.')[0])>=24,'Node24 lossless JSON source/rawJSON support required');
const candidate=await fs.realpath(values.candidate),output=path.resolve(values.output);assert.ok(!output.startsWith(candidate+path.sep),'artifacts are outside candidate');await fs.mkdir(output,{recursive:true});
assert.equal((await fs.readdir(output)).length,0,'fresh evidence directory');const data=path.join(output,'data');await fs.mkdir(data);
for(const script of ['build.sh','run.sh','test.sh'])assert.ok((await fs.stat(path.join(candidate,script))).isFile(),`${script} public launch script`);
const spec=await fs.readFile(path.join(directory,'..','SPEC.md'));assert.equal(sha256(spec),SPEC_SHA256,'frozen SPEC exact bytes');
const manifests=async root=>(await tree(root)).map(([name,bytes])=>({name,sha256:sha256(bytes)}));
const report={schema:'semaprax.teamdesk.acceptance.v1',arm:values.arm,spec_sha256:SPEC_SHA256,node:process.version,started_at:new Date().toISOString(),candidate_before:await manifests(candidate),gate:await manifests(directory),checks:[],qualification:{passed:false},authority:{network:'explicit loopback candidate URLs',files:[candidate,output],processes:'build.sh test.sh run.sh and browser'}};
if(values.compiler){assert.ok(/^[a-f0-9]{40}$/.test(values['compiler-source-sha']??''),'compiler exact source SHA required');report.compiler={path:await fs.realpath(values.compiler),sha256:sha256(await fs.readFile(values.compiler)),source_sha:values['compiler-source-sha']};}
const env={...process.env,TEAMDESK_ARM:values.arm,TEAMDESK_DATA_DIR:data,TEAMDESK_HOST:'127.0.0.1',TEAMDESK_PORT:String(await unusedPort()),TEAMDESK_UI_PORT:String(await unusedPort()),TEAMDESK_KDF_LEDGER:path.join(output,'kdf.jsonl'),SEMAPRAX_BIN:values.compiler??process.env.SEMAPRAX_BIN??'',NODE_OPTIONS:`${process.env.NODE_OPTIONS??''} --import=${pathToFileURL(path.join(directory,'kdf-observer.mjs')).href}`};if(values['playwright-root'])process.env.PLAYWRIGHT_PACKAGE_ROOT=path.resolve(values['playwright-root']);
const log=createWriteStream(path.join(output,'process.log'),{flags:'wx'}),probe=new Probe();let server;
try{
  for(const script of ['build.sh','test.sh']){const process=start('/bin/sh',[path.join(candidate,script)],{cwd:candidate,env,log,timeout:180000});const result=await process.done;assert.equal(result.code,0,`${script} failed; see process.log`);}
  server=await launch({candidate,env,log});const first={api:server.api,ui:server.ui};report.launch=first;
  const restart=async()=>{await finish(server);server=await launch({candidate,env,log});assert.deepEqual({api:server.api,ui:server.ui},first,'restart preserves allocated gate endpoints');};
  const state=await apiChecks({base:server.api,arm:values.arm,restart,probe});
  if(state.ready)await browserChecks({base:server.ui,arm:values.arm,state,probe,artifacts:output});
  await finish(server);server=undefined;
  await probe.check('password-kdf-storage','password.slow-salted-hash',async()=>{report.password_evidence=await passwordChecks({data,ledger:env.TEAMDESK_KDF_LEDGER});});
  if(report.password_evidence?.verified===false){const row=probe.rows.find(row=>row.id==='password-kdf-storage');row.status='unverified';row.error=report.password_evidence.reason;}
}catch(error){probe.rows.push({id:'runner-fatal',group:'runner',status:'failed',error:String(error.stack??error)});}finally{if(server)await finish(server);await new Promise(resolve=>log.end(resolve));report.checks=probe.rows;report.qualification=qualify(probe.rows);report.candidate_after=await manifests(candidate);report.finished_at=new Date().toISOString();await fs.writeFile(path.join(output,'report.json'),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify({report:path.join(output,'report.json'),...report.qualification}));process.exitCode=report.qualification.passed?0:1;}
