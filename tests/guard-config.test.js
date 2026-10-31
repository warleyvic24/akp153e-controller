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

test('bloqueia cópia, truncamento e gravação por script no config real', () => {
  const LINUX = '~/.config/AJazz/AKP153E/config.json';
  assert.equal(blocked(`cp /tmp/x ${LINUX}`), true);
  assert.equal(blocked(`install -m644 x "${LINUX}"`), true);
  assert.equal(blocked(`cp x ${LINUX} && echo ok`), true);
  assert.equal(blocked(`truncate -s0 ${LINUX}`), true);
  assert.equal(blocked(`dd if=/dev/null of=${LINUX}`), true);
  assert.equal(blocked(`Copy-Item x "${CONFIG}"`), true);
  assert.equal(blocked('cd ~/.config/AJazz/AKP153E && rm config.json'), true);
  assert.equal(blocked('cd "~/.config/AJazz/AKP153E"; echo {} > config.json'), true);
  assert.equal(blocked(`python3 -c "open('/home/u/.config/AJazz/AKP153E/config.json','w')"`), true);
  assert.equal(blocked(`python3 -c "import pathlib; pathlib.Path('${LINUX}').write_text('x')"`), true);
  assert.equal(blocked(`node -e "require('fs').writeFileSync('${LINUX}', 'x')"`), true);
});

test('bloqueia Write/Edit no config real e permite em cópias', () => {
  const edit = (file_path) => decide({ tool_input: { file_path } }).block;
  assert.equal(edit('/home/u/.config/AJazz/AKP153E/config.json'), true);
  assert.equal(edit(CONFIG), true);
  assert.equal(edit('/tmp/akp153e-test-42/config.json'), false);
  assert.equal(edit('/home/u/Documents/AKP153E/akp153e-controller/src/config/mod.rs'), false);
});

test('permite ler o config real', () => {
  assert.equal(blocked('cp ~/.config/AJazz/AKP153E/config.json /tmp/copia.json'), false);
  assert.equal(blocked('cd ~/.config/AJazz/AKP153E && cat config.json'), false);
  assert.equal(blocked(`python3 -c "print(open('/h/.config/AJazz/AKP153E/config.json').read())"`), false);
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
