import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {readFileSync, writeFileSync, mkdtempSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';

const runner = resolve('run.sh');
const temp = mkdtempSync(join(tmpdir(), 'loglens-test-'));
let checks = 0;
function invoke(args, status = 0) {
  const r = spawnSync(runner, args, {encoding: 'utf8'});
  assert.equal(r.status, status, `${args.join(' ')}: ${r.stderr}`);
  if (status === 0) assert.equal(r.stderr, '');
  else { assert.equal(r.stdout, ''); assert.match(r.stderr, /^[^\r\n]+\n$/); }
  checks++;
  return r.stdout;
}
function request(path, status = 200, bytes = '1000', hour = '10', ip = 'a') {
  return `${ip} - - [10/Oct/2026:${hour}:00:00 +0000] "GET ${path} HTTP/1.1" ${status} ${bytes}`;
}
const goldenText = `lines: 12
requests: 10
malformed: 2
unique_ips: 4
status: 2xx=7 3xx=1 4xx=1 5xx=1
error_rate: 20.0%
bytes: 10450
avg_bytes: 1045
top_paths:
  1. /index.html 4
  2. /api/items 3
  3. /login 2
hours:
  09 3
  10 7
busiest_hour: 10
`;
const goldenJSON = '{"lines":12,"requests":10,"malformed":2,"unique_ips":4,"status":{"2xx":7,"3xx":1,"4xx":1,"5xx":1},"error_rate":20.0,"bytes":10450,"avg_bytes":1045,"top_paths":[{"path":"/index.html","count":4},{"path":"/api/items","count":3},{"path":"/login","count":2}],"hours":{"09":3,"10":7},"busiest_hour":"10"}\n';
// The inline golden is a smaller example than the provided public sample.
const goldenLines = [
  ...Array.from({length:4}, (_, i) => request('/index.html', 200, '1000', i < 3 ? '09' : '10', 'a')),
  ...Array.from({length:3}, () => request('/api/items', 200, '1000', '10', 'b')),
  request('/login', 404, '1000', '10', 'c'), request('/login', 500, '1000', '10', 'c'),
  request('/other', 301, '1450', '10', 'd'), 'bad', 'also bad',
];

// Independent test reference; arbitrary precision totals, bytewise order.
function expected(input, top = 5, json = false) {
  const lines = input.split(/\r\n|\r|\n/).filter(x => x !== '');
  const valid = /^([^\s]+) - - \[(\d{2})\/([A-Za-z]{3})\/(\d{4}):(\d{2}):(\d{2}):(\d{2}) \+(\d{4})\] "([A-Z]+) (\/[^\s"\\]*) ([^\s]+)" ([1-5]\d{2}) (\d+|-)$/;
  let count = 0, bytes = 0n;
  const ips = new Set(), paths = new Map(), hours = new Map(), statuses = [0,0,0,0];
  for (const line of lines) {
    const m = valid.exec(line);
    if (!m || Number(m[5]) > 23) continue;
    count++; ips.add(m[1]);
    paths.set(m[10], (paths.get(m[10]) ?? 0) + 1);
    hours.set(m[5], (hours.get(m[5]) ?? 0) + 1);
    const group = Math.floor(Number(m[12]) / 100);
    if (group >= 2) statuses[group-2]++;
    bytes += m[13] === '-' ? 0n : BigInt(m[13]);
  }
  const pathsSorted = [...paths].sort((a,b) => b[1]-a[1] || Buffer.compare(Buffer.from(a[0]), Buffer.from(b[0]))).slice(0,top);
  const hourList = [...hours].sort((a,b) => a[0].localeCompare(b[0]));
  const busiest = [...hourList].sort((a,b) => b[1]-a[1])[0]?.[0] ?? '-';
  const tenths = count === 0 ? 0 : Math.floor(((statuses[2]+statuses[3])*2000+count)/(count*2));
  const rate = `${Math.floor(tenths/10)}.${tenths%10}`;
  const avg = count === 0 ? 0n : bytes/BigInt(count);
  if (json) return `{"lines":${lines.length},"requests":${count},"malformed":${lines.length-count},"unique_ips":${ips.size},"status":{"2xx":${statuses[0]},"3xx":${statuses[1]},"4xx":${statuses[2]},"5xx":${statuses[3]}},"error_rate":${rate},"bytes":${bytes},"avg_bytes":${avg},"top_paths":${JSON.stringify(pathsSorted.map(([path,count]) => ({path,count})))},"hours":{${hourList.map(([h,n]) => `${JSON.stringify(h)}:${n}`).join(',')}},"busiest_hour":${JSON.stringify(busiest)}}\n`;
  return `lines: ${lines.length}\nrequests: ${count}\nmalformed: ${lines.length-count}\nunique_ips: ${ips.size}\nstatus: 2xx=${statuses[0]} 3xx=${statuses[1]} 4xx=${statuses[2]} 5xx=${statuses[3]}\nerror_rate: ${rate}%\nbytes: ${bytes}\navg_bytes: ${avg}\ntop_paths:\n${pathsSorted.map(([p,n],i) => `  ${i+1}. ${p} ${n}\n`).join('')}hours:\n${hourList.map(([h,n]) => `  ${h} ${n}\n`).join('')}busiest_hour: ${busiest}\n`;
}
function compare(input, top = 5) {
  const file = join(temp, 'input.log');
  writeFileSync(file, input);
  assert.equal(invoke([file, '--top', String(top)]), expected(input, top));
  assert.equal(invoke([file, '--json', '--top', String(top)]), expected(input, top, true));
}
try {
  const golden = join(temp, 'golden.log');
  writeFileSync(golden, goldenLines.join('\n')+'\n');
  assert.equal(invoke([golden, '--top', '3']), goldenText);
  assert.equal(invoke([golden, '--top', '3', '--json']), goldenJSON);
  const sample = readFileSync('../sample.log', 'utf8');
  assert.equal(invoke(['../sample.log']), expected(sample));
  assert.equal(invoke(['../sample.log', '--top', '3', '--json']), expected(sample,3,true));
  for (const input of ['', '\n\r\n\r', ' \ninvalid\r\n', request('/')]) compare(input);
  for (const sep of ['\n', '\r', '\r\n']) compare(goldenLines.join(sep)+sep);
  compare([request('/z',100,'-','23'), request('/a',599,'000005','00'), request('/é',200,'11','00'), request('/a',200,'9','23')].join('\r'),50);
  compare([request('/b',400,'1','01'),request('/a',200,'2','00'),request('/c',200,'4','02')].join('\n'),1);
  compare([request('/b',400),...Array.from({length:15}, () => request('/a'))].join('\n')); // 6.25 -> 6.3
  compare([request('/huge',200,'9'.repeat(500)),request('/huge',500,'1')].join('\n'));
  compare(request('/maximum',200,'9'.repeat(33000)));
  const good = request('/valid');
  const mutations = [
    good+' extra', ' '+good, good+' ', good.replace('GET','get'), good.replace('/valid','valid'),
    good.replace('/valid','/a b'), good.replace('/valid','/a"b'), good.replace('/valid','/a\\b'),
    good.replace(':10:',':24:'), good.replace('200','600'), good.replace('200','099'),
    good.replace('1000','12kb'), good.replace('1000','+12'), good.replace('1000','-12'),
    good.replace(' - - [',' - ['), good.replace('+0000','-0000'), good.replace('Oct','October'),
    good.replace('HTTP/1.1',''), good.replace('GET',''), good.replace('1000',''),
    good.replace(':00:00',':a0:00'), good.replace('/2026','/20x6'), good.replace('a -','a\t -'), good.replace('Oct','éct'), good.replace('200','é00'), good.replace('1000','é000'),
  ];
  compare(mutations.join('\n'));
  compare(good.replace('HTTP/1.1','HTTP\"1'));
  compare(good.replace(':00:00',':99:99'));
  for (let i=0;i<good.length;i++) compare(good.slice(0,i));
  let seed = 123456;
  function random(n) { seed = (Math.imul(seed,1664525)+1013904223) >>> 0; return seed%n; }
  for (let batch=0;batch<12;batch++) {
    const rows = Array.from({length:40}, () => random(7) === 0 ? mutations[random(mutations.length)] : request(['/a','/b','/é','/Z','/'][random(5)], [100,200,301,404,500][random(5)], random(4) === 0 ? '-' : String(random(10000)), String(random(24)).padStart(2,'0'), String(random(8))));
    compare(rows.join(['\n','\r','\r\n'][random(3)]), 1+random(50));
  }
  const missing = join(temp,'absent.log');
  invoke([missing],1);
  invoke([temp],1);
  invoke([],2);
  for (const args of [['--json'],[golden,'--wat'],[golden,'extra'],[golden,'--top'],[golden,'--top','0'],[golden,'--top','51'],[golden,'--top','-1'],[golden,'--top','1.5'],[golden,'--top','x'],[golden,'--top',''],[golden,'--top','99999999999999999999999'],[missing,'--wat'],[join(temp,'absent','file'),'--top','bad']]) invoke(args,2);
  assert.equal(invoke([golden,'--top','0003','--json']),goldenJSON);
  assert.equal(invoke([golden,...Array(30).fill('--json'),'--top','3']),goldenJSON);
  assert.equal(invoke([golden,'--top','1','--top','3','--json']),goldenJSON);
  console.log(`Passed ${checks} CLI checks, including exact inline goldens and exit statuses 0, 1, 2.`);
} finally { rmSync(temp,{recursive:true,force:true}); }
