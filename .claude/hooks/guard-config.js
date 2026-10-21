#!/usr/bin/env node
// PreToolUse (Bash/PowerShell): bloqueia comandos que gravam ou apagam o config.json REAL do usuário.
// Já aconteceu de testes e scripts sobrescreverem esse arquivo. Ler é permitido. Se a entrada não puder
// ser entendida, o hook deixa passar (falha aberta) para nunca travar o trabalho por engano.
const CONFIG_PATH = /AKP153E[\\/]+config\.json/i;
const WRITES_TO_CONFIG = new RegExp(
  [
    String.raw`(Set-Content|Add-Content|Out-File|Remove-Item|Move-Item|Rename-Item|Clear-Content)\b[^|;]*AKP153E[\\/]+config\.json`,
    String.raw`\b(rm|del|erase|mv|ren)\b[^|;]*AKP153E[\\/]+config\.json`,
    String.raw`\bsed\s+-i\b[^|;]*AKP153E[\\/]+config\.json`,
    String.raw`\btee\b[^|;]*AKP153E[\\/]+config\.json`,
    String.raw`>{1,2}\s*["']?[^|;&]*AKP153E[\\/]+config\.json`,
  ].join('|'),
  'i',
);

function decide(input) {
  const command = String(input?.tool_input?.command ?? '');
  if (CONFIG_PATH.test(command) && WRITES_TO_CONFIG.test(command)) {
    return {
      block: true,
      message:
        'Bloqueado: este comando grava ou apaga o config.json real do usuário (%APPDATA%\\AJazz\\AKP153E). ' +
        'Use o editor do programa ou peça confirmação ao usuário.',
    };
  }
  return { block: false };
}

if (require.main === module) {
  let raw = '';
  process.stdin.on('data', (chunk) => (raw += chunk)).on('end', () => {
    let input;
    try {
      input = JSON.parse(raw);
    } catch {
      process.exit(0);
    }
    const verdict = decide(input);
    if (verdict.block) {
      process.stderr.write(verdict.message + '\n');
      process.exit(2);
    }
    process.exit(0);
  });
}

module.exports = { decide };
