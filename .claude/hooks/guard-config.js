#!/usr/bin/env node
// PreToolUse (Bash/PowerShell/Write/Edit): bloqueia o que grava ou apaga o config.json REAL do usuário.
// Já aconteceu de testes e scripts sobrescreverem esse arquivo. Ler é permitido. Se a entrada não puder
// ser entendida, o hook deixa passar (falha aberta) para nunca travar o trabalho por engano.
const PATH = String.raw`AKP153E[\\/]+config\.json`;
// Write/Edit recebem o caminho inteiro: exige a pasta `AJazz` para não pegar cópias de teste.
const REAL_FILE = /AJazz[\\/]+AKP153E[\\/]+config\.json$/i;
const WRITES_TO_CONFIG = new RegExp(
  [
    String.raw`(Set-Content|Add-Content|Out-File|Remove-Item|Move-Item|Rename-Item|Clear-Content|Copy-Item)\b[^|;]*${PATH}`,
    String.raw`\b(rm|del|erase|mv|ren|truncate|shred)\b[^|;]*${PATH}`,
    // `cp`/`install`/`ln` só gravam no último argumento; o config como origem é leitura.
    String.raw`\b(cp|install|ln)\b[^|;&]*${PATH}["']?\s*($|[|;&])`,
    String.raw`\bdd\b[^|;&]*\bof=["']?[^\s|;&]*${PATH}`,
    String.raw`\bsed\s+-i\b[^|;]*${PATH}`,
    String.raw`\btee\b[^|;]*${PATH}`,
    String.raw`>{1,2}\s*["']?[^|;&]*${PATH}`,
    // Entrando na pasta e gravando pelo nome curto: `cd .../AKP153E && rm config.json`.
    String.raw`AKP153E[\\/]*["']?\s*(&&|;)[^|]*(\b(rm|del|erase|mv|cp|install|truncate|tee|Set-Content|Out-File|Remove-Item)\b[^|;&]*|\bsed\s+-i\b[^|;&]*|>{1,2}\s*["']?)\bconfig\.json`,
    // Linhas de Python/Node que gravam ou apagam o arquivo.
    String.raw`\bopen\(\s*[rRbBuU]?["'][^"']*${PATH}["']\s*,\s*["'][^"']*[wax+]`,
    String.raw`(writeFileSync|writeFile|appendFileSync|appendFile|unlinkSync|unlink|rmSync|copyFileSync|renameSync|os\.remove|os\.unlink|os\.replace|os\.rename|shutil\.\w+)\s*\(\s*[^)]*${PATH}`,
    String.raw`${PATH}["']\s*\)\s*\.(write_text|write_bytes|unlink|touch|rename|replace)\b`,
  ].join('|'),
  'i',
);

function decide(input) {
  const command = String(input?.tool_input?.command ?? '');
  const file = String(input?.tool_input?.file_path ?? input?.tool_input?.notebook_path ?? '');
  if (WRITES_TO_CONFIG.test(command) || REAL_FILE.test(file)) {
    return {
      block: true,
      message:
        'Bloqueado: isto grava ou apaga o config.json real do usuário (%APPDATA%\\AJazz\\AKP153E ou ~/.config/AJazz/AKP153E). ' +
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
