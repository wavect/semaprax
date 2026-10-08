import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import net from 'node:net';
import path from 'node:path';
export async function unusedPort(){const server=net.createServer();await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(0,'127.0.0.1',resolve);});const port=server.address().port;await new Promise(resolve=>server.close(resolve));return port;}
export function localUrl(value){const url=new URL(value);assert.equal(url.protocol,'http:');assert.ok(['127.0.0.1','localhost','[::1]'].includes(url.hostname),'only explicit loopback authority');assert.ok(url.port);assert.equal(url.username,'');assert.equal(url.password,'');assert.equal(url.pathname,'/');assert.equal(url.search,'');assert.equal(url.hash,'');return url.href;}
export function start(command,args,{cwd,env,log,timeout=120000}){
  const child=spawn(command,args,{cwd,env,detached:process.platform!=='win32',stdio:['ignore','pipe','pipe']});let stdout='',stderr='',closed=false;
  const append=(kind,data)=>{const text=String(data);if(kind==='stdout')stdout+=text;else stderr+=text;if(stdout.length+stderr.length>8*1024*1024){kill();throw new Error('bounded child output exceeded');}log.write(`[${kind}] ${text}`);};
  child.stdout.on('data',data=>append('stdout',data));child.stderr.on('data',data=>append('stderr',data));
  const kill=(signal='SIGTERM')=>{if(closed)return;try{if(process.platform==='win32')child.kill(signal);else process.kill(-child.pid,signal);}catch(error){if(error.code!=='ESRCH')throw error;}};
  const done=new Promise((resolve,reject)=>{child.once('error',reject);child.once('close',(code,signal)=>{closed=true;resolve({code,signal,stdout,stderr});});});
  const timer=setTimeout(()=>kill('SIGKILL'),timeout);done.finally(()=>clearTimeout(timer));
  return {child,done,kill,get closed(){return closed;},get stdout(){return stdout;}};
}
export async function finish(process){process.kill();await Promise.race([process.done,new Promise(resolve=>setTimeout(resolve,5000))]);if(!process.closed){process.kill('SIGKILL');await process.done;}}
export function readinessPath(arm) {
  assert.ok(['semaprax', 'typescript'].includes(arm), 'known application arm');
  return arm === 'semaprax' ? 'api/session' : 'api/me';
}
export async function readyResponse(response) {
  if (![200, 401].includes(response.status)) return false;
  try { const body = await response.json(); return body !== null && typeof body === 'object' && !Array.isArray(body); }
  catch { return false; }
}
export async function launch({candidate,env,log}){
  const process=start('/bin/sh',[path.join(candidate,'run.sh')],{cwd:candidate,env,log,timeout:45*60*1000});let descriptor;
  const deadline=Date.now()+30000;while(Date.now()<deadline&&!process.closed){for(const line of process.stdout.split('\n')){try{const value=JSON.parse(line);if(value.api_base_url&&value.ui_base_url){descriptor={api:localUrl(value.api_base_url),ui:localUrl(value.ui_base_url)};break;}}catch{}}
    if(descriptor){try{const result=await fetch(descriptor.api+readinessPath(env.TEAMDESK_ARM),{signal:AbortSignal.timeout(1000),redirect:'manual'});if(await readyResponse(result))return {...process,...descriptor};}catch{}}
    await new Promise(resolve=>setTimeout(resolve,50));}
  await finish(process);throw new Error('run.sh did not publish a ready loopback launch descriptor within30s');
}
export async function tree(root){const rows=[];async function visit(directory){for(const entry of(await fs.readdir(directory,{withFileTypes:true})).sort((a,b)=>a.name.localeCompare(b.name))){if(['.git','node_modules','target'].includes(entry.name))continue;const name=path.join(directory,entry.name);assert.ok(!entry.isSymbolicLink(),`no symlink evidence: ${name}`);if(entry.isDirectory())await visit(name);else if(entry.isFile())rows.push([path.relative(root,name),await fs.readFile(name)]);}}await visit(root);return rows;}
