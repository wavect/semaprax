'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const shared = path.resolve(root, '..', '..', 'ui', 'semantic-explorer');
const packaged = path.join(root, 'explorer-assets');
const assets = ['explorer.css', 'model.js', 'layout.js', 'cache.js', 'changes.js', 'evidence.js', 'hosts.js', 'view.js'];

fs.mkdirSync(packaged, { recursive: true });
const manifest = {};
for (const file of assets) {
  const source = fs.readFileSync(path.join(shared, file));
  fs.writeFileSync(path.join(packaged, file), source);
  manifest[file] = crypto.createHash('sha256').update(source).digest('hex');
}
for (const file of fs.readdirSync(packaged)) if (!assets.includes(file)) fs.unlinkSync(path.join(packaged, file));
fs.writeFileSync(path.join(root, 'explorer-assets.manifest.json'), JSON.stringify({ schema: 'semaprax.vscode-explorer-assets.v1', assets: manifest }, null, 2) + '\n');
