#!/usr/bin/env node
'use strict';
// Runs test/extension-host-compiler in a real VS Code Extension Host with a
// clean editor profile, an installed release compiler and a generated starter.
//   node scripts/run-compiler-host-journey.js --vscode-app "/Applications/Visual Studio Code.app" \
//        (--archive semaprax-vX-<target>.tar.gz | --compiler /abs/semaprax) [--work /abs/dir]
const { spawnSync } = require('node:child_process');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const args = new Map();
for (let i = 2; i < process.argv.length; i += 2) args.set(process.argv[i].replace(/^--/, ''), process.argv[i + 1]);
const fail = message => { console.error(`error: ${message}`); process.exit(2); };
const run = (command, argv, options = {}) => {
  const r = spawnSync(command, argv, { encoding: 'utf8', ...options });
  if (r.error) fail(`${command}: ${r.error.message}`);
  return r;
};

const app = args.get('vscode-app') || fail('--vscode-app is required');
const work = path.resolve(args.get('work') || fs.mkdtempSync(path.join(os.tmpdir(), 'semaprax-host-journey-')));
fs.mkdirSync(work, { recursive: true });
let compiler = args.get('compiler');
if (!compiler) {
  const archive = args.get('archive') || fail('--archive or --compiler is required');
  const out = path.join(work, 'release');
  fs.mkdirSync(out, { recursive: true });
  const t = run('tar', ['-xzf', path.resolve(archive), '-C', out]);
  if (t.status !== 0) fail(`tar failed: ${t.stderr}`);
  const found = fs.readdirSync(out).map(name => path.join(out, name, 'semaprax')).find(candidate => fs.existsSync(candidate));
  compiler = found || fail('archive holds no semaprax binary');
}
compiler = fs.realpathSync(compiler);
const version = JSON.parse(run(compiler, ['version', '--json']).stdout).version;
const project = path.join(work, 'projects', 'first-semaprax');
fs.mkdirSync(path.dirname(project), { recursive: true });
fs.rmSync(project, { recursive: true, force: true });
const created = run(compiler, ['new', 'first-semaprax'], { cwd: path.dirname(project) });
if (created.status !== 0) fail(`semaprax new failed: ${created.stderr}`);

// Clean editor profile: empty user data and extensions directories.
const user = path.join(work, 'user'), extensions = path.join(work, 'extensions');
for (const dir of [user, extensions]) fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(path.join(user, 'User'), { recursive: true });
fs.mkdirSync(extensions, { recursive: true });
fs.writeFileSync(path.join(user, 'User', 'settings.json'), '{}\n');

const code = ['Code', 'Electron'].map(n => path.join(app, 'Contents/MacOS', n)).find(fs.existsSync) || fail('no VS Code executable in the app bundle');
const root = path.resolve(__dirname, '..');
const host = run(code, [
  `--user-data-dir=${user}`, `--extensions-dir=${extensions}`,
  '--disable-workspace-trust', '--disable-gpu', '--disable-updates', '--skip-welcome', '--skip-release-notes',
  `--extensionDevelopmentPath=${root}`, `--extensionTestsPath=${path.join(root, 'test/extension-host-compiler/index.js')}`, project
], {
  env: { ...process.env, SEMAPRAX_HOST_COMPILER: compiler, SEMAPRAX_HOST_PROJECT: fs.realpathSync(project), SEMAPRAX_HOST_USER_SETTINGS: path.join(user, 'User', 'settings.json'), SEMAPRAX_HOST_VERSION: version },
  timeout: 5 * 60 * 1000, maxBuffer: 64 * 1024 * 1024
});
process.stdout.write(host.stdout || '');
process.stderr.write(host.stderr || '');
console.log(`compiler=${compiler} version=${version} project=${project} exit=${host.status}`);
process.exit(host.status === 0 ? 0 : 1);
