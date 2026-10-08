// Test-only observer: invoke the original KDF with identical operands/result.
// This records actual host crypto use without trusting a candidate's claims.
import crypto from 'node:crypto';
import { appendFileSync } from 'node:fs';
import { syncBuiltinESMExports } from 'node:module';
const ledger=process.env.TEAMDESK_KDF_LEDGER;
const digest=value=>crypto.createHash('sha256').update(value).digest('hex');
function record(name,password,salt,parameters,key) {
  if(!ledger)return;
  appendFileSync(ledger,JSON.stringify({algorithm:name,password_sha256:digest(password),salt_hex:Buffer.from(salt).toString('hex'),parameters,key_hex:Buffer.from(key).toString('hex')})+'\n',{mode:0o600});
}
const originalScrypt=crypto.scryptSync;
crypto.scryptSync=function(password,salt,keylen,options){const result=originalScrypt(password,salt,keylen,options);record('scrypt',password,salt,{N:options?.N??options?.cost??16384,r:options?.r??options?.blockSize??8,p:options?.p??options?.parallelization??1},result);return result;};
const originalPbkdf=crypto.pbkdf2Sync;
crypto.pbkdf2Sync=function(password,salt,iterations,keylen,algorithm){const result=originalPbkdf(password,salt,iterations,keylen,algorithm);record('pbkdf2',password,salt,{iterations,digest:algorithm},result);return result;};
const originalScryptAsync=crypto.scrypt;
crypto.scrypt=function(password,salt,keylen,options,callback){if(typeof options==='function'){callback=options;options=undefined;}return originalScryptAsync(password,salt,keylen,options,(error,result)=>{if(!error)record('scrypt',password,salt,{N:options?.N??options?.cost??16384,r:options?.r??options?.blockSize??8,p:options?.p??options?.parallelization??1},result);callback(error,result);});};
const originalPbkdfAsync=crypto.pbkdf2;
crypto.pbkdf2=function(password,salt,iterations,keylen,algorithm,callback){return originalPbkdfAsync(password,salt,iterations,keylen,algorithm,(error,result)=>{if(!error)record('pbkdf2',password,salt,{iterations,digest:algorithm},result);callback(error,result);});};
syncBuiltinESMExports();
