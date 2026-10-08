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
const safeRelative=value=>{assert.equal(typeof value,'string','capture path string');assert.ok(value&&value!=='.'&&!path.isAbsolute(value)&&!value.split(path.sep).includes('..')&&path.posix.normalize(value)===value,'capture path is safe');return value;};
// Optional evidence capture never replaces the mandatory build, tests or gate.
const rawManifest=async root=>{
  const rows=[];
  const visit=async directory=>{
    const entries=await fs.readdir(directory,{withFileTypes:true});
    entries.sort((a,b)=>a.name<b.name?-1:a.name>b.name?1:0);
    for(const entry of entries){
      const file=path.join(directory,entry.name);
      assert.ok(!entry.isSymbolicLink(),'raw compiler evidence has no symlinks');
      if(entry.isDirectory())await visit(file);
      else{assert.ok(entry.isFile(),'raw compiler evidence contains only regular files');rows.push({raw_path:path.relative(root,file).split(path.sep).join('/'),sha256:sha256(await fs.readFile(file))});}
    }
  };
  await visit(root);return rows;
};
const archiveExcluded=new Set(['.git','.cache','.mypy_cache','.pytest_cache','__pycache__','dist','node_modules','out','target']);
const retainedInputs=async()=> (await manifests(candidate)).filter(row=>row.name!=='compiler-output-capture.json'&&!row.name.split(path.sep).some(part=>archiveExcluded.has(part))).map(row=>({path:row.name.split(path.sep).join('/'),sha256:row.sha256}));
const capture=async()=>{
  const declarationPath=path.join(candidate,'compiler-output-capture.json');let declaration;
  try{assert.ok(!(await fs.lstat(declarationPath)).isSymbolicLink(),'capture declaration is regular');declaration=JSON.parse(await fs.readFile(declarationPath,'utf8'));}
  catch(error){if(error?.code==='ENOENT')return;throw error;}
  assert.ok(values.compiler&&values.arm==='semaprax','only a trusted Semaprax compiler can capture raw output');
  assert.deepEqual(Object.keys(declaration).sort(),['argv','input_files','output_directory','schema']);
  assert.equal(declaration.schema,'semaprax.compiler-output-capture.v1');
  assert.ok(Array.isArray(declaration.argv)&&[4,6].includes(declaration.argv.length));
  assert.equal(declaration.argv[0],'webapp');assert.equal(declaration.argv[2],'-o');assert.equal(declaration.argv[3],'{output}');
  const source=safeRelative(declaration.argv[1]);
  if(declaration.argv.length===6){assert.equal(declaration.argv[4],'--title');assert.equal(typeof declaration.argv[5],'string');}
  const inputs=await retainedInputs();assert.ok(inputs.some(row=>row.path===source),'source belongs to retained inputs');
  assert.deepEqual(declaration.input_files,inputs,'capture closes the retained input inventory; declaration and generated/cache directories are excluded');
  const outputDirectory=safeRelative(declaration.output_directory);
  const compile=async basename=>{
    const root=path.join(output,basename);await fs.mkdir(root);
    assert.equal(sha256(await fs.readFile(report.compiler.path)),report.compiler.sha256,'compiler bytes remain pinned');
    const argv=[...declaration.argv];argv[3]=root;
    const process=start(report.compiler.path,argv,{cwd:candidate,env:{PATH:process.env.PATH??''},log,timeout:180000});
    assert.equal((await process.done).code,0,'direct compiler capture failed; see process.log');
    assert.equal(sha256(await fs.readFile(report.compiler.path)),report.compiler.sha256,'compiler bytes remain pinned');
    assert.deepEqual(await retainedInputs(),inputs,'compiler capture leaves candidate inputs unchanged');
    return await rawManifest(root);
  };
  const raw=await compile('compiler-output-raw');assert.ok(raw.length>0,'compiler produced raw files');
  const repeat=await compile('compiler-output-repeat');assert.deepEqual(repeat,raw,'fresh compiler reproduction matches raw bytes');
  const rawOutputs=raw.map(row=>({...row,final_path:path.posix.join(outputDirectory,row.raw_path)}));
  const receipt={schema:'semaprax.compiler-output-provenance.v1',compiler:{source_sha:values['compiler-source-sha'],binary_sha256:report.compiler.sha256},cwd:'.',argv:declaration.argv,input_files:inputs,raw_root:'compiler-output-raw',raw_outputs:rawOutputs,repeat_outputs:repeat};
  const receiptPath=path.join(output,'compiler-output-receipt.json');await fs.writeFile(receiptPath,JSON.stringify(receipt,null,2)+'\n');
  report.compiler_output_receipt={path:receiptPath,sha256:sha256(await fs.readFile(receiptPath)),compiler:receipt.compiler,raw_outputs:rawOutputs};
};
const log=createWriteStream(path.join(output,'process.log'),{flags:'wx'}),probe=new Probe();let server;
try{
  await capture();
  for(const script of ['build.sh','test.sh']){const process=start('/bin/sh',[path.join(candidate,script)],{cwd:candidate,env,log,timeout:180000});const result=await process.done;assert.equal(result.code,0,`${script} failed; see process.log`);}
  server=await launch({candidate,env,log});const first={api:server.api,ui:server.ui};report.launch=first;
  const restart=async()=>{await finish(server);server=await launch({candidate,env,log});assert.deepEqual({api:server.api,ui:server.ui},first,'restart preserves allocated gate endpoints');};
  const state=await apiChecks({base:server.api,arm:values.arm,restart,probe});
  if(state.ready)await browserChecks({base:server.ui,arm:values.arm,state,probe,artifacts:output});
  await finish(server);server=undefined;
  await probe.check('password-kdf-storage','password.slow-salted-hash',async()=>{report.password_evidence=await passwordChecks({data,ledger:env.TEAMDESK_KDF_LEDGER});});
  if(report.password_evidence?.verified===false){const row=probe.rows.find(row=>row.id==='password-kdf-storage');row.status='unverified';row.error=report.password_evidence.reason;}
}catch(error){probe.rows.push({id:'runner-fatal',group:'runner',status:'failed',error:String(error.stack??error)});}finally{if(server)await finish(server);await new Promise(resolve=>log.end(resolve));report.checks=probe.rows;report.qualification=qualify(probe.rows);report.candidate_after=await manifests(candidate);report.finished_at=new Date().toISOString();await fs.writeFile(path.join(output,'report.json'),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify({report:path.join(output,'report.json'),passed:report.qualification.passed,cases:report.qualification.cases,failed:report.qualification.failures.length,missingCases:report.qualification.missingCases.length,missingGroups:report.qualification.missingGroups.length}));process.exitCode=report.qualification.passed?0:1;}
