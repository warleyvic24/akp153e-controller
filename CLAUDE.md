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
- Release usa `panic = "abort"`; o gancho de pânico em `service.rs` registra o erro e reinicia o serviço (até 3 quedas seguidas).

## Convenções

- Comentários e mensagens ao usuário em português; nomes de código em inglês. Constantes nomeadas em vez de números soltos.
- Nenhum arquivo acima de 800 linhas (hoje: `main.rs` ~610, `schema.rs` ~730, `stage.rs` ~460). Painéis em `panes.rs`, editor de ações em `action_ui.rs`, inspetor em `inspector.rs`, barras em `bars.rs`, palco em `stage.rs`, preferências em `settings.rs`, avisos em `toast.rs`, textos/idiomas em `strings.rs` (texto novo = `t`/`tf` + entrada em `EN`), CLI em `cli.rs`, áudio do Windows em `audio.rs`, página de mixer em `mixer.rs` (dados puros, testáveis) e `mixer_runtime.rs` (thread de leitura, teclas, repetição); ação `open-mixer`.
- Unsafe só em `shell.rs` e `audio.rs` (Core Audio do mixer), sempre com `// SAFETY:`.
- Interface: siga `docs/DESIGN.md` (tokens em `tokens.rs`, cores em `theme::Palette`, texto mínimo 12 px, alvo mínimo 24 px, contraste imposto por testes em `theme.rs`). Confira nos dois temas e, para testar clicando sem risco, use em debug `AKP153E_PREVIEW_THEME=light|dark` e `AKP153E_CONFIG_DIR=<pasta com cópia do config.json>`.

## Hardware

O aparelho do usuário enumera como HID `0300:3010` (usage page FFA0), tratado como protocolo v3 (JPEG 100 px, soltura reportada, pacote de 1024 com recuo para 512). O protocolo vem de documentação pública, não de engenharia reversa própria.
**Verificado:** desenho das teclas (pelo log e pelo aparelho conectado), interface, ações de app/webhook/atalho, autostart, bandeja.
**Não verificado:** aperto e soltura físicos das teclas, pressão longa pelo hardware, as 3 teclas laterais (ignoradas).
Não afirme que "funciona" no aparelho sem prova do usuário. Os passos para provar cada item estão em `evals/hardware-checklist.md`.

## Guarda do config real

`.claude/hooks/guard-config.js` (hook `PreToolUse` em `.claude/settings.json`) bloqueia comandos Bash/PowerShell que gravam ou apagam o `config.json` real do usuário. Ler é permitido. Testado em `tests/guard-config.test.js` (`node --test "tests/**/*.test.js"`). Para alterar a config use o editor do programa ou peça confirmação ao usuário.
