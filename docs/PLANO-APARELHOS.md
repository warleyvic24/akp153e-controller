# Plano: Keyforge, controlador para qualquer "stream deck"

Data: 2026-10-31. Base: pesquisa sobre Bitfocus Companion (módulos de superfície), VIA/QMK (definição
JSON + catálogo da comunidade), mirajazz/OpenDeck (família AJAZZ/Mirabox em versões de protocolo) e
StreamController (selo "experimental"). Princípio: **o aparelho é descrito por dados; o código só traduz.**

**Ordem geral:** `docs/ROTEIRO.md` (intercala este plano com o outro e a checklist do aparelho).

Cada fase tem: passos, como saber que terminou (aceite) e o que pode dar errado (risco → correção).
Nenhuma fase começa sem a anterior estar verde no CI (Windows e Linux).

---

## Fase 0: renomear para Keyforge (feita em 2026-10-31; migração conferida neste PC)

**Passos**
1. `Cargo.toml` `name = "keyforge"` (binário `keyforge`), `build.rs` (ProductName), título da janela,
   `APP_ID`, nome no topo do editor, user agent, log `keyforge.log`.
2. Pasta da config: `~/.config/Keyforge/` (Windows `%APPDATA%\Keyforge\`).
3. Migração: na abertura, se a pasta nova não existe e a antiga (`AJazz/AKP153E`) existe, **copiar** (não mover)
   config, ícones e trava; registrar no log. A antiga fica como cópia de segurança.
4. Início automático: grava a entrada nova e **apaga a antiga** (`akp153e.desktop` / valor `AKP153E` no registro).
5. Variáveis de debug `KEYFORGE_*` (aceitar `AKP153E_*` por um tempo).
6. Perfil exportado: `keyforge-profile` / `.keyforge.json`; importar aceita também o formato antigo.
7. Empacotamento: `packaging/linux/keyforge.desktop`, `70-keyforge.rules`, PKGBUILD `pkgname=keyforge` com
   `replaces=(akp153e) conflicts=(akp153e)`, `keyforge.install` (encerra o serviço antigo), `atualizar.sh`, CI (artefato), `build_release.ps1`.
8. Guarda da config (`.claude/hooks/guard-config.js`): proteger as **duas** pastas; atualizar os testes.
9. Docs: README (nome, "compatível com" em vez de usar a marca Stream Deck), CLAUDE.md, LINUX.md.

**Aceite:** instalação nova abre com a config migrada; o serviço antigo some; o pacote novo substitui o antigo
sem conflito de arquivos; `cargo test` e o teste do hook passam.

**Riscos → correção**
- Serviço antigo (`akp153e --background`) ainda rodando segura o USB e a porta 47153 → o pacote novo para o
  processo antigo no `post_upgrade`; o `atualizar.sh` avisa e mata pelo nome antigo também.
- Dois inícios automáticos (antigo e novo) → passo 4; teste unitário do texto da entrada.
- Migração copia por cima de uma config nova → só copia se a nova **não existe**.
- Caminhos de ícones gravados na config apontam para a pasta antiga → continuam válidos (a antiga não é apagada);
  "Embelezar"/importar novos já gravam na pasta nova.
- Regra udev com nome novo e a antiga ainda instalada → o pacote remove a antiga (é dono dela via `replaces`).
- Marca: não usar "Stream Deck" no nome, ícone ou título; só "funciona com Elgato Stream Deck" no texto.
- Repositório no GitHub continua `akp153e-controller` até o dono renomear (o GitHub redireciona o endereço antigo).

---

## Fase 1: descrição do aparelho (dados) (feita em 2026-10-31)

**Formato** (`devices/<fabricante>/<modelo>.json`, embutidos no binário e lidos também da pasta da config):
```json
{
  "id": "ajazz-akp153e",
  "name": "AJAZZ AKP153E",
  "usb": [{ "vid": "0300", "pid": "3010", "usage_page": "FFA0" }],
  "protocol": { "family": "mirabox", "version": 3, "packet": [1024, 512], "release_events": true },
  "image": { "px": 100, "format": "jpeg", "rotate": 270, "max_bytes": 3072 },
  "controls": [
    { "id": "k0", "kind": "key", "x": 0, "y": 0, "hw": 12 },
    { "id": "side", "kind": "strip", "x": 5, "y": 0, "w": 1, "h": 3,
      "px": [82, 400], "slots": [{ "hw": 15, "top": 0 }, { "hw": 16, "top": 146 }, { "hw": 17, "top": 219 }] }
  ],
  "status": "verified"
}
```
- `kind`: `key`, `strip` (tela sem toque), `dial` (giratório: girar e apertar), `touch` (faixa de toque),
  `led`. `hw` = índice no protocolo (substitui `OPENDECK_TO_DEVICE`).
- `status`: `verified` (alguém provou no aparelho) ou `experimental`.

**Passos**
1. `src/device/model.rs`: tipos + leitura + validação (ids únicos, `hw` sem repetir, imagem dentro dos limites,
   grade sem sobreposição).
2. Converter o AKP153E atual para o primeiro arquivo; os números de hoje passam a vir dele.
3. Testes: o arquivo do AKP153E gera **exatamente** os mesmos índices e tamanhos de hoje (teste de ouro).

**Aceite:** nenhuma mudança visível; todos os testes de hoje passam; arquivo inválido é recusado com mensagem clara.

**Riscos → correção**
- Arquivo da comunidade malformado ou malicioso → só dados (sem código), limites de tamanho, validação estrita;
  nunca aceitar comando bruto vindo do arquivo.
- Divergência sutil de mapeamento (tecla trocada) → teste de ouro compara a tabela antiga com a nova.

---

## Fase 2: tradutores de protocolo (drivers) (feita em 2026-10-31)

**Passos**
1. Trait `Driver`: `open`, `set_image(control, bytes)`, `clear`, `brightness`, `poll() -> Event`
   (`Press/Release(control)`, `Turn(control, delta)`, `Touch(x)`), `keepalive`.
2. `drivers/mirabox.rs`: o código de `usb/mod.rs` de hoje, parametrizado por `protocol.version`, `packet`,
   `release_events`. Lista de comandos permitidos fixa no código (**nunca** `LOG`/gravação na flash).
3. **Decidido:** `mirajazz` 0.16 é MPL-2.0 e só assíncrono (tokio); fica como referência de fatos do protocolo
   (versões 0 a 3: 293S, AKP153, N3/AKP03, N4), sem copiar código. Estudar o crate `mirajazz` (licença, cobertura de AKP03/N3/N4, qualidade) e decidir: usar, copiar ideias ou não.
4. Aparelho falso (`FakeDriver`) para testes: grava o que recebeu e emite eventos programados.
5. Detecção: ao plugar, casar VID/PID/usage page com o catálogo; sem casar, oferecer o assistente (Fase 5).

**Aceite:** AKP153E funciona igual (checklist 1 a 16 de novo no aparelho); testes do serviço rodam no `FakeDriver`.

**Riscos → correção**
- Regressão no único aparelho que temos → refatorar em passos pequenos, cada um com o teste de ouro e um
  `atualizar.sh` + conferência no aparelho antes do próximo.
- Clone com dialeto desconhecido trava no logo → o driver para no primeiro erro e pede para religar o cabo;
  nada é gravado na memória do aparelho.
- Licença incompatível (mirajazz) → conferir antes de copiar qualquer linha; na dúvida, reimplementar a partir
  da documentação pública.

---

## Fase 3: config por controle (não por índice) (feita em 2026-10-31)

**Feito:** config versão 3; `buttons[i]` é a tecla `keys()[i]` do modelo (no AKP153E, `k{i}`: a migração é
identidade) e a página guarda `model`; a lateral é o controle `side` (`Page::side`). Antes de migrar grava
`config.v{N}.bak.json` (uma vez, só o dono lê). Config mais nova que o programa é lida mas nunca gravada.
`Page::split_for` divide uma página para um aparelho menor (15 em 6 = 3 páginas, testado); quem chama é a
Fase 4. Ações de giratório ficam para quando houver um aparelho com giratório (Fase 7, Stream Deck Plus):
sem evento de giro no `Driver`, seriam campos sem uso. Conferido com uma cópia da config real
(`AKP_CONFIG_COPY=.. cargo test real_config -- --ignored`).

**Passos**
1. Botão passa a ser ligado ao **id do controle** (`"k0"`, `"side"`, `"dial1"`), não à posição no vetor.
2. Página guarda o modelo para o qual foi feita; ação de giratório: `on_turn_left/right`, `on_press`.
3. Migração da config: índice `i` da grade 3x5 → `k{i}` do AKP153E; lateral atual → controle `side`.
4. Perfil em outro aparelho: teclas que sobram viram páginas novas; controles que faltam ficam guardados (não
   apagados) e reaparecem se o aparelho voltar.

**Aceite:** a config antiga abre igual; um perfil de 15 teclas aberto num modelo de 6 vira 3 páginas, sem perda.

**Riscos → correção**
- Perda de dados na migração → gravar `config.v1.bak.json` antes; teste com configs reais (cópia) e antigas.
- Editor antigo gravando por cima do formato novo → campo `version` na config; versão velha recusa gravar uma
  config mais nova (hoje não existe isso: entra nesta fase).
- Dois aparelhos ao mesmo tempo → fora do escopo agora; arquitetura deixa um `device_id` por perfil para depois.

---

## Fase 4: editor desenha qualquer aparelho (feita em parte em 2026-10-31)

**Feito:** o palco desenha as teclas e a faixa lateral a partir do arquivo do modelo da página
(`editor/device_view.rs`, com testes de geometria: o AKP153E sai idêntico; um modelo de 2 x 3 sem lateral).
Selo "Experimental" à direita das páginas quando o modelo não é verificado. Prévias conferidas nos dois temas.
**Falta:** giratórios e faixa de toque (junto com a Fase 7, que traz o primeiro aparelho com eles);
escolher o modelo à mão sem aparelho e dividir as páginas com `Page::split_for` ao trocar de modelo
(só faz sentido quando houver um segundo modelo de outro tamanho no catálogo).

**Passos**
1. O palco desenha os controles do arquivo (teclas, faixa lateral, giratórios redondos, faixa de toque).
2. Inspetor por tipo de controle (giratório: duas ações e o aperto).
3. Selo "experimental" no topo quando o modelo não é verificado.
4. Sem aparelho conectado: escolher o modelo à mão para montar perfis.

**Aceite:** prévias nos dois temas para AKP153E, um modelo de 6 teclas e um com giratórios; contraste e alvos
mínimos (testes de `theme.rs`) passam.

**Riscos → correção**
- Arquivos acima de 800 linhas (`stage.rs`, `inspector.rs`) → separar `stage/controls.rs` antes de começar.
- Layout estranho (aparelho muito largo) → o palco já encolhe a tecla; testar com 8x4 (Elgato XL).

---

## Fase 5: assistente "Ensinar meu aparelho"

**Passos**
1. Lista as interfaces HID desconhecidas (o `--probe` de hoje vira tela).
2. Escolher a família (AJAZZ/Mirabox v1/v3, Elgato) e o desenho (linhas, colunas, extras).
3. **Ordem das teclas:** "aperte a tecla acesa" para cada uma → aprende `hw`.
4. **Imagem:** seta + régua (a calibração de hoje) → rotação e tamanho; reduz `max_bytes` até a tela não cair.
5. Salva em `~/.config/Keyforge/devices/` e oferece "Compartilhar" (abre um PR/issue com o JSON pronto).

**Aceite:** com o AKP153E fingindo ser desconhecido, o assistente gera um arquivo igual ao embutido.

**Riscos → correção**
- Aparelho não responde a nenhuma família → dizer isso claramente e sugerir abrir um pedido com o `--probe`;
  não tentar comandos às cegas.
- Usuário erra a ordem → cada passo tem "voltar"; ao fim, teste de todas as teclas acendendo em sequência.

---

## Fase 6: catálogo da comunidade

**Passos**
1. Pasta `devices/` no repositório + validação no CI (schema, ids únicos, VID/PID sem conflito).
2. O programa baixa o catálogo (com hash do commit) e usa só arquivos válidos; sem internet, usa os embutidos.
3. Guia `docs/ADICIONAR-APARELHO.md`.

**Riscos → correção**
- Arquivo ruim derruba o aparelho de outra pessoa → novos entram como `experimental`; só viram `verified` com
  prova (foto/vídeo + checklist).
- Ataque pelo catálogo → só JSON validado, baixado por HTTPS do repositório oficial, sem comandos crus.

---

## Fase 7: Elgato

**Passos:** driver `elgato.rs` (protocolo público: Mini, MK.2, XL, Plus, Neo), arquivos dos modelos, regra udev
`0fd9`. Avaliar o crate `elgato-streamdeck` (licença).
**Riscos → correção:** sem aparelho para testar → tudo `experimental` até alguém provar; o Plus tem tela de
toque e giratórios: depende das fases 3 e 4.

---

## Fase 8: plugins de protocolo (depois)

Um programa externo fala com o Keyforge por JSON (stdin/stdout) e implementa o `Driver`. Instalado só pelo
usuário, com aviso de que roda código de terceiros.

---

## Fora do escopo por enquanto

Dois aparelhos ao mesmo tempo; importar perfis do app da Elgato; deck virtual no celular (depois da Fase 4,
vira só mais um `Driver`).

## Ordem e checagem

0 → 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8. Ao fim de cada fase: `cargo fmt`, `clippy -D warnings`, `cargo test`, CI
verde, prévias nos dois temas, `atualizar.sh` e a checklist do aparelho quando a fase toca o USB.
