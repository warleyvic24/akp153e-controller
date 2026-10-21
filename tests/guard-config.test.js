// Testa o hook .claude/hooks/guard-config.js: bloqueia gravar/apagar o config.json real, permite o resto.
const test = require('node:test');
const assert = require('node:assert/strict');
const { decide } = require('../.claude/hooks/guard-config.js');

const CONFIG = String.raw`C:\Users\x\AppData\Roaming\AJazz\AKP153E\config.json`;
const blocked = (command) => decide({ tool_input: { command } }).block;

test('bloqueia comandos que gravam ou apagam o config real', () => {
  assert.equal(blocked(`Set-Content -Path "${CONFIG}" -Value x`), true);
  assert.equal(blocked(`Add-Content "${CONFIG}" y`), true);
  assert.equal(blocked(`Remove-Item "${CONFIG}" -Force`), true);
  assert.equal(blocked(`rm "${CONFIG}"`), true);
  assert.equal(blocked(`echo x > "${CONFIG}"`), true);
  assert.equal(blocked(`echo x >> ${CONFIG}`), true);
  assert.equal(blocked(`sed -i s/a/b/ "${CONFIG}"`), true);
});

test('permite ler o config real', () => {
  assert.equal(blocked(`Get-Content "${CONFIG}"`), false);
  assert.equal(blocked(`cat "${CONFIG}"`), false);
  assert.equal(blocked(`cat "${CONFIG}" > backup.txt`), false);
  assert.equal(blocked(`Get-Content "${CONFIG}" | Out-File saida.txt`), false);
  assert.equal(blocked(`grep launch-app "${CONFIG}" 2>&1`), false);
});

test('permite comandos que não tocam no config real', () => {
  assert.equal(blocked('cargo test'), false);
  assert.equal(blocked('Set-Content -Path nota.txt -Value x'), false);
  assert.equal(blocked('rm -rf target'), false);
});

test('entrada sem comando passa', () => {
  assert.equal(decide({}).block, false);
  assert.equal(decide(undefined).block, false);
  assert.equal(decide({ tool_input: { file_path: 'x' } }).block, false);
});
