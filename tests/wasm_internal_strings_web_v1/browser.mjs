// Explicit provisioned browser evidence. No installer, external server, retry,
// or runtime-source modification. The test-only engine counter observes real
// compiled adapters; it does not replace their implementation or imports.
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {readFileSync,lstatSync} from 'node:fs';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';

const [root,modulePath,chromiumPath]=process.argv.slice(2).map(value=>resolve(value));
const {chromium}=await import(pathToFileURL(modulePath));
assert(chromium&&typeof chromium.launch==='function','provisioned Playwright module required');
const names=['app.wasm','semaprax.js','semaprax.d.ts','semaprax.internal-strings.json',
  'semaprax.manifest.json','package.json','index.html','app.js'];
const files=new Map(names.map(name=>{
  const path=resolve(root,name),metadata=lstatSync(path);
  assert(metadata.isFile()&&!metadata.isSymbolicLink());return[name,readFileSync(path)];
}));
const hostile='web."</script>λ';
let mode='normal';
const requests=[];
const server=createServer((request,response)=>{
  if(request.method!=='GET'){response.writeHead(405).end();return}
  const name=request.url==='/'?'index.html':request.url?.slice(1);
  if(!files.has(name)){response.writeHead(404).end();return}
  requests.push(name);
  response.setHeader('Cache-Control','no-store');
  response.setHeader('Content-Type',name.endsWith('.wasm')?'application/wasm':name.endsWith('.js')?'text/javascript':name.endsWith('.html')?'text/html':'application/json');
  if((mode==='descriptor-oversize'&&name==='semaprax.internal-strings.json')||
     (mode==='wasm-oversize'&&name==='app.wasm')){
    const total=(mode==='descriptor-oversize'?1048576:16777216)+1;
    // Separate writes force chunked streaming without trusting Content-Length.
    response.write(Buffer.alloc(8192));response.end(Buffer.alloc(total-8192));return;
  }
  let bytes=files.get(name);
  if((mode==='descriptor-tamper'&&name==='semaprax.internal-strings.json')||
     (mode==='wasm-tamper'&&name==='app.wasm')){
    bytes=Buffer.from(bytes);bytes[bytes.length-1]^=1;
  }
  response.end(bytes);
});
await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(0,'127.0.0.1',resolve)});
const origin=`http://127.0.0.1:${server.address().port}`;
let browser;
try{
  browser=await chromium.launch({executablePath:chromiumPath,headless:true});
  const context=await browser.newContext();
  await context.route('**/*',route=>{
    const url=route.request().url();
    assert(url.startsWith(origin+'/'),'browser attempted a non-loopback request');
    return route.continue();
  });
  await context.addInitScript(()=>{
    globalThis.__webEntries=0;globalThis.__webInstantiations=0;globalThis.__webCompiles=0;
    const compile=WebAssembly.compile,instantiate=WebAssembly.instantiate;
    WebAssembly.compile=async(...args)=>{globalThis.__webCompiles++;return Reflect.apply(compile,WebAssembly,args)};
    WebAssembly.instantiate=async(...args)=>{
      globalThis.__webInstantiations++;
      const result=await Reflect.apply(instantiate,WebAssembly,args);
      const instance=result instanceof WebAssembly.Instance?result:result.instance;
      const exports={...instance.exports};
      for(const [name,fn] of Object.entries(exports))if(name.startsWith('__spx_call_')){
        exports[name]=(...values)=>{globalThis.__webEntries++;return fn(...values)};
      }
      return result instanceof WebAssembly.Instance?{exports}:{...result,instance:{exports}};
    };
  });
  const page=await context.newPage();
  const pageErrors=[];page.on('pageerror',error=>pageErrors.push(String(error)));
  await page.goto(origin+'/');
  await page.waitForFunction(()=>document.querySelector('#status').textContent==='Ready · 8 exports');
  assert.equal(await page.evaluate(()=>globalThis.__webEntries),0,'package called an export before user action');
  assert.equal(await page.evaluate(()=>globalThis.__webInstantiations),1);
  assert.equal(await page.locator('script:not([src])').count(),0);
  assert.deepEqual(await page.locator('#export-select option').allTextContents(),
    ['-web.constant', '__proto__',hostile,'web.bool','web.capacity','web.content','web.divide','web.required']);
  async function invoke(id,input,expected){
    await page.getByLabel('Export identity').selectOption({value:id});
    if(typeof input==='boolean'){
      const checkbox=page.getByLabel('Argument 1 · bool');
      assert.equal(await checkbox.getAttribute('type'),'checkbox');
      await checkbox.setChecked(input);
    }else if(typeof input==='string'){
      await page.getByLabel('Argument 1 · i64').fill(input);
    }
    const before=await page.evaluate(()=>globalThis.__webEntries);
    const button=page.getByRole('button',{name:'Invoke function'});
    await button.focus();await button.press('Enter');
    await page.waitForFunction(previous=>globalThis.__webEntries===previous+1,before);
    assert.deepEqual(JSON.parse(await page.locator('#transcript').textContent()),expected);
    assert.equal(await button.isEnabled(),true);
  }
  await invoke('web.content',undefined,{kind:'success',value:'42'});
  await invoke('-web.constant',undefined,{kind:'success',value:'17'});
  await invoke('web.bool',false,{kind:'success',value:false});
  await invoke('web.bool',true,{kind:'success',value:true});
  await invoke('web.divide','0',{kind:'failure',domain:'semaprax.arithmetic.v1',code:4});
  await invoke('web.divide','2',{kind:'success',value:'21'});
  await invoke('web.required',false,{kind:'failure',domain:'semaprax.contract.v1',code:1});
  await invoke('web.capacity','5000',{kind:'capacity',cause:'cumulative_bytes'});
  await invoke('web.content',undefined,{kind:'success',value:'42'});
  await invoke('__proto__',undefined,{kind:'success',value:'11'});
  await invoke(hostile,undefined,{kind:'success',value:'13'});
  assert.equal(await page.locator('#signature').textContent(),hostile+'() → i64');
  assert.equal(await page.locator('#signature script').count(),0);
  await page.getByLabel('Export identity').selectOption('web.divide');
  await page.getByLabel('Argument 1 · i64').fill('9223372036854775808');
  const before=await page.evaluate(()=>globalThis.__webEntries);
  await page.getByRole('button',{name:'Invoke function'}).click();
  assert.match(await page.locator('#transcript').textContent(),/outside i64/);
  assert.equal(await page.evaluate(()=>globalThis.__webEntries),before);
  assert.deepEqual(pageErrors,[]);
  await page.close();

  for(const fault of ['descriptor-tamper','descriptor-oversize','wasm-tamper','wasm-oversize']){
    mode=fault;requests.length=0;
    const faultPage=await context.newPage();
    await faultPage.goto(origin+'/');
    await faultPage.waitForFunction(()=>document.querySelector('#status').textContent.startsWith('Package unavailable:'));
    const status=await faultPage.locator('#status').textContent();
    assert.match(status,fault.endsWith('oversize')?/exceeds byte limit/:fault.startsWith('descriptor')?/descriptor digest/:/authentication/);
    assert.equal(await faultPage.getByRole('button',{name:'Invoke function'}).isDisabled(),true);
    assert.equal(await faultPage.getByLabel('Export identity').isDisabled(),true);
    assert.equal(await faultPage.locator('#export-select option').count(),0);
    assert.deepEqual(await faultPage.evaluate(()=>[globalThis.__webCompiles,globalThis.__webInstantiations,globalThis.__webEntries]),[0,0,0]);
    if(fault.startsWith('descriptor'))assert(!requests.includes('app.wasm'));
    await faultPage.close();
  }
  await context.close();
}finally{
  if(browser)await browser.close();
  await new Promise((resolve,reject)=>server.close(error=>error?reject(error):resolve()));
}
console.log('string-web-chromium-ok');
