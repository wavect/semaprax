import assert from 'node:assert/strict';
import { ENTITIES, integer } from './contract.mjs';
const INT_FIELDS = new Set(['id','duration','days','length','remaining','weight','tasks','open_tasks','members','open_tickets','hours', ...Object.values(ENTITIES).flatMap(fields => Object.entries(fields).filter(([,type])=>type==='int'||type.startsWith('ref:')).map(([field])=>field))]);
export const route = (arm, entity) => arm === 'semaprax' ? entity.replace(/[A-Z]/g,(letter,index)=>(index?'_':'')+letter.toLowerCase()) : entity.toLowerCase();
export function numericInteger(source){const match=/^(-?)([0-9]+)(?:\.([0-9]+))?(?:[eE]([+-]?[0-9]+))?$/.exec(source);assert.ok(match,'numeric integer JSON source');const digits=BigInt(match[2]+(match[3]??'')),power=Number(match[4]??0)-(match[3]?.length??0);if(digits===0n)return '0';assert.ok(Number.isSafeInteger(power)&&Math.abs(power)<=30,'bounded signed64 integer exponent');const scale=10n**BigInt(Math.abs(power));assert.ok(power>=0||digits%scale===0n,'JSON value is integral');return integer(((power>=0?digits*scale:digits/scale)*(match[1]? -1n:1n)).toString()).toString();}
export const lossless = text => JSON.parse(text,(key,value,context)=>{if(INT_FIELDS.has(key)){assert.notEqual(typeof value,'string',`JSON ${key} must be numeric, not quoted`);if(typeof value==='number')return numericInteger(context.source);}return value;});
export const mutations=[];
export class Client {
  constructor(base, arm) {this.base=base;this.arm=arm;this.cookies=new Map();}
  get cookie() {return [...this.cookies].map(([key,value])=>`${key}=${value}`).join('; ');}
  async request(method, path, body) {
    if (!['GET','HEAD'].includes(method) && this.arm==='semaprax') {
      const seed=await this.raw('GET','session/csrf');
      assert.equal(seed.status,200,'ordinary CSRF acquisition');
      assert.equal(typeof seed.json.token,'string');this.csrf=seed.json.token;
    }
    return this.raw(method,path,body);
  }
  async raw(method,path,body) {
    const headers={cookie:this.cookie};if(this.csrf)headers['x-csrf-token']=this.csrf;
    if(body!==undefined)headers['content-type']='application/json';
    const response=await fetch(new URL(`api/${path}`,this.base),{method,headers,body:body===undefined?undefined:JSON.stringify(body,(key,value)=>INT_FIELDS.has(key)&&typeof value==='string'&&/^-?\d+$/.test(value)?JSON.rawJSON(value):value),signal:AbortSignal.timeout(10000)});
    for(const header of response.headers.getSetCookie()) {const pair=header.split(';')[0],index=pair.indexOf('=');this.cookies.set(pair.slice(0,index),pair.slice(index+1));}
    const text=await response.text();return {status:response.status,text,json:text&&response.headers.get('content-type')?.includes('json')?lossless(text):null,headers:response.headers};
  }
  async entity(method,entity,id,body,expected) {
    const response=await this.request(method,route(this.arm,entity)+(id===undefined?'':`/${id}`),body);
    if(expected!==undefined)assert.equal(response.status,expected,`${method} ${entity}/${id??''}: ${response.text}`);if(['POST','PUT','DELETE'].includes(method)&&[200,201,204].includes(response.status))mutations.push({entity,id:method==='POST'?response.json.id:id,action:{POST:'create',PUT:'update',DELETE:'delete'}[method],actor:this.account?.id});return response;
  }
  async login(email,password) {
    const response=await this.request('POST','session',{[this.arm==='semaprax'?'login':'email']:email,password});assert.equal(response.status,200,response.text);this.account=response.json;return response.json;
  }
  async me() {return this.request('GET',this.arm==='semaprax'?'session':'me');}
}
export function rowShape(entity,row) {
  assert.ok(row&&typeof row==='object');assert.ok(integer(row.id)>0n);assert.equal(Object.hasOwn(row,'password'),false,'password is never returned');
  for(const [field,type]of Object.entries(ENTITIES[entity])) {assert.ok(Object.hasOwn(row,field),`${entity}.${field} omitted`);if(type==='int'||type.startsWith('ref:'))integer(row[field]);else if(type==='float')assert.ok(typeof row[field]==='number'&&Number.isFinite(row[field]),`${entity}.${field} f64`);else if(type==='string'||type==='bool')assert.equal(typeof row[field],type==='bool'?'boolean':'string');}
}
