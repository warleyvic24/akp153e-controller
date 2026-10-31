# AJAZZ AKP153E (Rust)

Substituto do aplicativo oficial do AJAZZ AKP153E (stream controller com 15 teclas LCD). Rust + egui/eframe, Windows e Linux. O Linux (`platform/*_linux.rs`, `shell.rs` com `cfg`) é compilado pelo CI (`.github/workflows/ci.yml`) e foi rodado no Omarchy (Hyprland 0.56, config em Lua) em 2026-10-29: instalação, dependências e o que foi provado em `docs/LINUX.md`; precisa de regra udev (`packaging/linux/`) para o HID, `pactl` para o mixer e GTK/appindicator para a bandeja.
Um executável, dois modos:

- **Serviço** (`akp153e.exe --background`, bandeja, sem janela): dono do USB/HID, das ações e do desenho das teclas.
- **Editor** (`akp153e.exe --editor`, janela egui): edita `%APPDATA%\AJazz\AKP153E\config.json` (Linux: `~/.config/AJazz/AKP153E/config.json`) e avisa o serviço por TCP em `127.0.0.1:47153` (`reload`, `status`, `reconnect`, `show`, `calib`, `pattern`, `apps`, `obs-info`, `obs-run`). Trava do editor: porta 47154.
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
- Para regravar a config use `AppConfig::load_existing()` (devolve `None` se o arquivo não puder ser lido). `AppConfig::load()` cai na config padrão e a gravaria por cima do arquivo do usuário. O `reload` do serviço também usa `load_existing()` e mantém a config atual se falhar.
- Ler, mudar e gravar a config (editor e serviço) vai dentro de `AppConfig::locked` (trava `config.lock`); senão um processo desfaz a gravação do outro.
- O serviço só grava perfil/página ativos (`runtime::persist`); o resto da config é do editor.
- A porta de controle aceita qualquer processo local (no Linux só do mesmo usuário, `platform::same_user`): valide e limite toda entrada nova (veja `service::read_command`, `parse_px`).
- Desenhos no aparelho passam por `DRAW_ORDER` (`runtime::change_and_draw`, `runtime::press`): um caminho novo que desenha fora dele pode deixar teclas com a imagem antiga.
- Segure o mutex `SharedState` só o tempo de copiar dados; desenhar, gravar e codificar JPEG ficam fora do lock (`runtime::Snapshot`, `Followup`).
- Release usa `panic = "abort"`; o gancho de pânico em `service/mod.rs` registra o erro e reinicia o serviço (até 3 quedas seguidas).

## Convenções

- Comentários e mensagens ao usuário em português; nomes de código em inglês. Constantes nomeadas em vez de números soltos.
- Nenhum arquivo acima de 800 linhas (hoje o maior é `service/runtime.rs`, ~765; o próximo teste grande vai para `service/runtime/tests.rs`). Testes grandes vão para `tests.rs` ao lado (ex.: `mixer/tests.rs`).
- Camadas em `src/`, dependência só para baixo. O editor usa do serviço apenas `service::runtime` (estado compartilhado); o serviço nunca usa o editor.
  - `main.rs`: lê os argumentos e escolhe o modo. `cli.rs`: log, `--run-action`, abertura do editor.
  - `editor/`: janela egui. `mod.rs` (`App`), painéis `panes.rs`, ações `action_ui.rs`, inspetor `inspector.rs`, barras `bars.rs`, palco `stage.rs`, preferências `settings.rs`, avisos `toast.rs`, design `tokens.rs`/`theme.rs`/`widgets.rs`.
  - `service/`: serviço em segundo plano (`mod.rs`), estado compartilhado `runtime.rs` (testes em `runtime/tests.rs`), thread do mixer `mixer_runtime.rs`, perfil automático `auto_profile.rs`, conexão com o OBS `obs_runtime.rs`, descanso da tela `sleep.rs`.
  - `obs/`: protocolo do obs-websocket 5 e estado do OBS, sem rede (testável); a rede fica em `service/obs_runtime.rs`.
  - `mixer/`: página de mixer, dados puros e testáveis; ação `open-mixer`.
  - `platform/`: `shell.rs` (os dois sistemas, com `cfg`), `mod.rs` (ícones, processos, `same_user`), `omarchy.rs`, `focus.rs` (app em foco: Hyprland pelo socket de eventos, Windows pela janela da frente). Windows: `audio.rs` (Core Audio), `apps.rs`. Linux: `audio_linux.rs` (`pactl`), `apps_linux.rs` (atalhos `.desktop`), `icon_theme.rs`. Comum: `audio_model.rs`.
  - `config/` (`schema.rs`, exportar/importar perfil `profile_io.rs`), `engine/` (ações), `graphics/` (desenho das teclas, ícones, biblioteca Lucide `icon_library.rs` sobre `assets/lucide-icons.tsv`, ícone do app), `usb/` (protocolo), `strings.rs` (texto novo = `t`/`tf` + entrada em `EN`).
- Unsafe só em `platform/` (`shell.rs`, `audio.rs`, `focus.rs`), sempre com `// SAFETY:`.
- Testes que leem o código usam `test_support::sources_without_tests()`, que percorre todas as subpastas.
- Interface: siga `docs/DESIGN.md` (tokens em `editor/tokens.rs`, cores em `theme::Palette`, texto mínimo 12 px, alvo mínimo 24 px, contraste imposto por testes em `editor/theme.rs`). Confira nos dois temas e, para testar clicando sem risco, use em debug `AKP153E_PREVIEW_THEME=light|dark` e `AKP153E_CONFIG_DIR=<pasta com cópia do config.json>` (com ele um segundo editor abre ao lado do real) e `AKP153E_PREVIEW_UI=key=5,tab=1,settings,picker,library` para abrir já numa tela. O OBS se testa de verdade com `AKP_OBS_PORT=.. AKP_OBS_PASSWORD=.. cargo test real_obs -- --ignored`.

## Hardware

O aparelho do usuário enumera como HID `0300:3010` (usage page FFA0), tratado como protocolo v3 (JPEG 100 px, soltura reportada, pacote de 1024 com recuo para 512). O protocolo vem de documentação pública, não de engenharia reversa própria.
**Verificado:** desenho das teclas (pelo log e pelo aparelho conectado), interface, ações de app/webhook/atalho, autostart, bandeja.
**Não verificado:** aperto e soltura físicos das teclas, pressão longa pelo hardware.
**Lateral:** no AKP153E é uma tela decorativa sem botões (as 3 teclas extras do protocolo são só do AKP153R; os índices delas seguem ignorados). Imagem nela só pelo `LOG`, que grava o logo de inicialização na flash e não está documentado para o protocolo v3: não implemente sem fonte nova. Detalhes em `evals/hardware-checklist.md`.
Não afirme que "funciona" no aparelho sem prova do usuário. Os passos para provar cada item estão em `evals/hardware-checklist.md`.

## Guarda do config real

`.claude/hooks/guard-config.js` (hook `PreToolUse` em `.claude/settings.json`) bloqueia comandos Bash/PowerShell que gravam ou apagam o `config.json` real do usuário. Ler é permitido. Testado em `tests/guard-config.test.js` (`node --test "tests/**/*.test.js"`). Para alterar a config use o editor do programa ou peça confirmação ao usuário.
