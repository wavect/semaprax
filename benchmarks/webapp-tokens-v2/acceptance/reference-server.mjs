// Qualification-only adapter: original reference API and browser bytes remain
// unchanged. This static server/proxy supplies a common browser origin.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createServer,request } from 'node:http';
import fs from 'node:fs/promises';
import path from 'node:path';
const arm=process.env.TEAMDESK_ARM,root=process.cwd(),port=Number(process.env.TEAMDESK_PORT),uiPort=Number(process.env.TEAMDESK_UI_PORT);
assert.ok(['typescript','semaprax'].includes(arm));let child,staticServer;
const stop=()=>{child?.kill('SIGTERM');staticServer?.close();};process.on('SIGTERM',stop);process.on('SIGINT',stop);
if(arm==='semaprax'){
  child=spawn(process.execPath,[path.join(root,'generated','server.mjs'),'--host','127.0.0.1','--port',String(port),'--data',process.env.TEAMDESK_DATA_DIR,'--setup'],{stdio:['ignore','pipe','inherit'],env:process.env});child.stdout.pipe(process.stderr);
  const deadline=Date.now()+20000;while(Date.now()<deadline){try{const response=await fetch(`http://127.0.0.1:${port}/api/session`);if(response.status===200)break;}catch{}await new Promise(resolve=>setTimeout(resolve,50));}
  console.log(JSON.stringify({api_base_url:`http://127.0.0.1:${port}/`,ui_base_url:`http://127.0.0.1:${port}/`}));
}else{
  child=spawn(process.execPath,['--experimental-strip-types','server/index.ts'],{env:{...process.env,PORT:String(port),DATA_DIR:process.env.TEAMDESK_DATA_DIR},stdio:['ignore','inherit','inherit']});
  staticServer=createServer(async(req,res)=>{
    if(req.url.startsWith('/api/')){const proxy=request({hostname:'127.0.0.1',port,path:req.url,method:req.method,headers:req.headers},incoming=>{res.writeHead(incoming.statusCode,incoming.headers);incoming.pipe(res);});proxy.on('error',()=>{res.writeHead(502);res.end('upstream unavailable');});req.pipe(proxy);return;}
    try{const pathname=decodeURIComponent(new URL(req.url,'http://localhost').pathname),candidate=path.resolve(root,'dist','.'+pathname);assert.ok(candidate.startsWith(path.join(root,'dist')+path.sep));let file;try{file=await fs.readFile(candidate);}catch{file=await fs.readFile(path.join(root,'dist','index.html'));}res.writeHead(200,{'content-type':({'js':'text/javascript','css':'text/css','svg':'image/svg+xml'}[path.extname(candidate).slice(1)])??'text/html'});res.end(file);}catch{res.writeHead(400);res.end();}
  });await new Promise(resolve=>staticServer.listen(uiPort,'127.0.0.1',resolve));console.log(JSON.stringify({api_base_url:`http://127.0.0.1:${port}/`,ui_base_url:`http://127.0.0.1:${uiPort}/`}));
}
child.once('exit',code=>{staticServer?.close();process.exitCode=code??1;});
