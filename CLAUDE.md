# AJAZZ AKP153E (Rust)

Substituto do aplicativo oficial do AJAZZ AKP153E (stream controller com 15 teclas LCD). Rust + egui/eframe, só Windows.
Um executável, dois modos:

- **Serviço** (`akp153e.exe --background`, bandeja, sem janela): dono do USB/HID, das ações e do desenho das teclas.
- **Editor** (`akp153e.exe --editor`, janela egui): edita `%APPDATA%\AJazz\AKP153E\config.json` e avisa o serviço por TCP em `127.0.0.1:47153` (`reload`, `status`, `reconnect`, `show`, `calib`, `pattern`). Trava do editor: porta 47154.
- Outros: `--probe` (grava `akp153e_probe.txt`), `--run-action <json>`, `--autostart on|off`.
- Esconder uma janela eframe gasta 100% de CPU; por isso são dois processos.

## Comandos

```
cargo test                      # rápido; não toca na config real (ver abaixo)
cargo clippy --all-targets      # deve ficar sem avisos
cargo fmt                       # obrigatório antes de concluir
cargo build --release           # -> target\release\akp153e.exe
```

## Armadilhas

- `cargo build --release` falha com "Acesso negado" enquanto o serviço roda. Pare só o processo com `--background`, compile, reinicie. Avise o usuário antes de parar o serviço; não feche o editor dele sem avisar.
- Testes **nunca** podem escrever na config real: `AppConfig::path()` devolve uma pasta temporária em `cfg!(test)`. Mantenha isso ao criar testes que gravam.
- Para regravar a config use `AppConfig::load_existing()` (devolve `None` se o arquivo não puder ser lido). `AppConfig::load()` cai na config padrão e a gravaria por cima do arquivo do usuário.
- O serviço só grava perfil/página ativos (`runtime::persist`); o resto da config é do editor.
- A porta de controle aceita qualquer processo local: valide e limite toda entrada nova (veja `service::read_command`, `parse_px`).
- Segure o mutex `SharedState` só o tempo de copiar dados; desenhar, gravar e codificar JPEG ficam fora do lock (`runtime::Snapshot`, `Followup`).
- Release usa `panic = "abort"`; o gancho de pânico em `service/mod.rs` registra o erro e reinicia o serviço (até 3 quedas seguidas).

## Convenções

- Comentários e mensagens ao usuário em português; nomes de código em inglês. Constantes nomeadas em vez de números soltos.
- Nenhum arquivo acima de 800 linhas (hoje o maior é `config/schema.rs`, ~770). Testes grandes vão para `tests.rs` ao lado (ex.: `mixer/tests.rs`).
- Camadas em `src/`, dependência só para baixo. O editor usa do serviço apenas `service::runtime` (estado compartilhado); o serviço nunca usa o editor.
  - `main.rs`: lê os argumentos e escolhe o modo. `cli.rs`: log, `--run-action`, abertura do editor.
  - `editor/`: janela egui. `mod.rs` (`App`), painéis `panes.rs`, ações `action_ui.rs`, inspetor `inspector.rs`, barras `bars.rs`, palco `stage.rs`, preferências `settings.rs`, avisos `toast.rs`, design `tokens.rs`/`theme.rs`/`widgets.rs`.
  - `service/`: serviço em segundo plano (`mod.rs`), estado compartilhado `runtime.rs`, thread do mixer `mixer_runtime.rs`.
  - `mixer/`: página de mixer, dados puros e testáveis; ação `open-mixer`.
  - `platform/`: Windows (`shell.rs`, `audio.rs` Core Audio, `apps.rs` apps instalados).
  - `config/`, `engine/` (ações), `graphics/` (desenho das teclas, ícones, ícone do app), `usb/` (protocolo), `strings.rs` (texto novo = `t`/`tf` + entrada em `EN`).
- Unsafe só em `platform/` (`shell.rs`, `audio.rs`), sempre com `// SAFETY:`.
- Testes que leem o código usam `test_support::sources_without_tests()`, que percorre todas as subpastas.
- Interface: siga `docs/DESIGN.md` (tokens em `editor/tokens.rs`, cores em `theme::Palette`, texto mínimo 12 px, alvo mínimo 24 px, contraste imposto por testes em `editor/theme.rs`). Confira nos dois temas e, para testar clicando sem risco, use em debug `AKP153E_PREVIEW_THEME=light|dark` e `AKP153E_CONFIG_DIR=<pasta com cópia do config.json>`.

## Hardware

O aparelho do usuário enumera como HID `0300:3010` (usage page FFA0), tratado como protocolo v3 (JPEG 100 px, soltura reportada, pacote de 1024 com recuo para 512). O protocolo vem de documentação pública, não de engenharia reversa própria.
**Verificado:** desenho das teclas (pelo log e pelo aparelho conectado), interface, ações de app/webhook/atalho, autostart, bandeja.
**Não verificado:** aperto e soltura físicos das teclas, pressão longa pelo hardware, as 3 teclas laterais (ignoradas).
Não afirme que "funciona" no aparelho sem prova do usuário. Os passos para provar cada item estão em `evals/hardware-checklist.md`.

## Guarda do config real

`.claude/hooks/guard-config.js` (hook `PreToolUse` em `.claude/settings.json`) bloqueia comandos Bash/PowerShell que gravam ou apagam o `config.json` real do usuário. Ler é permitido. Testado em `tests/guard-config.test.js` (`node --test "tests/**/*.test.js"`). Para alterar a config use o editor do programa ou peça confirmação ao usuário.
