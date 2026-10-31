# Próximos passos: widgets vivos e visual das teclas

Retomada após `/clear`. Leia antes `CLAUDE.md` (convenções, camadas, armadilhas) e `docs/DESIGN.md`.
Estado em 2026-10-31: branch `linux-port`, último push `9d75ab8`; local, sem push, o commit
`583c734` (módulo `platform/live.rs`, ainda sem uso: gera avisos de código morto e o
`cargo clippy -- -D warnings` falha até ele ser ligado ao resto). O pacote instalado no Arch
(`/usr/bin/akp153e`) é o de `d7fbe4e`+, rodando como serviço.

## Pedido do usuário

"Melhorar os widgets para algo mais bonito e funcional". Ele escolheu as 4 opções:

1. **Volume ao vivo**: Vol+, Vol− e Mudo mostram a % atual e uma barra; Mudo fica vermelho quando mutado.
2. **Tocando agora**: Play/Pause mostra a capa e o título da música e o estado (tocando/pausado). Só Linux (MPRIS).
3. **Monitor do sistema**: tecla nova (ação "Monitor") e modo "Sistema" na tela lateral, com CPU, RAM e temperatura.
4. **Visual mais bonito**: fundo em degradê, ícone com rótulo pequeno, cor por categoria, e um botão
   "Embelezar página" que aplica isso à página toda (com Desfazer).

## Já feito (commit 583c734)

`src/platform/live.rs` + `master_volume()` em `platform/audio.rs` (Windows, Core Audio) e
`platform/audio_linux.rs` (`pactl get-sink-volume/mute @DEFAULT_SINK@`).

- `Wants { volume, playing, system }`, `Live { volume, playing, system }`, `Sampler::sample(wants)`.
- Volume: `Volume { percent, muted }`.
- Tocando: `NowPlaying { title, artist, playing, art }`, lido pelo `busctl --user --json=short`
  (sem `playerctl`, que não está instalado). A capa (`mpris:artUrl`, `file://` ou https) é baixada
  uma vez e guardada em PNG por `platform::save_icon`; `art` é o caminho local.
- Sistema: `SystemStats { cpu, ram, temp }`; CPU é a diferença entre duas consultas (a primeira dá 0).
  Linux: `/proc/stat`, `/proc/meminfo`, `hwmon` (`k10temp` neste PC, 33 °C). Windows: `GetSystemTimes`,
  `GlobalMemoryStatusEx` (feature `Win32_System_SystemInformation` já no Cargo.toml); sem temperatura.
- 4 testes passam. A saída de volume deste PC (HyperX) foi lida de verdade: 40 %.

## Falta fazer, nesta ordem

### 1. Config (`src/config/schema.rs`)
- `ButtonAppearance`: `#[serde(default)] pub gradient: bool` (e `gradient: false` em `ButtonConfig::blank`).
- `ButtonAction::Monitor { #[serde(default)] metric: Metric }` e
  `enum Metric { #[default] Overview, Cpu, Ram, Temp }` (kebab-case).
- `SideScreen::System` (CPU/RAM/temp na faixa lateral).
- Os `match` que reclamarem: `engine/actions.rs` (Monitor não faz nada ao tocar, como `Clock`),
  `editor/action_ui.rs` (nomes, ícone `lucide:activity`, editor com pílulas Visão geral/CPU/RAM/Temperatura,
  `STEP_KINDS` não), `editor/summary.rs`, `editor/inspector.rs::key_title`, `graphics/side.rs`.

### 2. Desenho (novo `src/graphics/live_art.rs`)
`render_live(button, w, h, &Live) -> RgbaImage`, que cai em `renderer::render_rgba` quando não há dado:
- `MediaKey volup/voldown/mute` com `live.volume`: rótulo vira "40%" (se a tecla não tiver rótulo),
  barra fina de nível no pé (verde; vermelha mudo); Mudo mutado com fundo vermelho e rótulo "MUDO".
- `MediaKey playpause` com `live.playing`: capa cobrindo a tecla, escurecida (~50 %) com degradê no pé;
  glifo play/pause pequeno no canto; título (negrito) e artista embaixo. Sem capa: fundo da tecla.
- `Monitor`: Overview = 3 linhas (rótulo à esquerda, barra, valor à direita); métrica única = número
  grande, rótulo e barra. Cor da barra: verde < 60, âmbar < 85, vermelho acima. Sem dado: "--".
- `gradient`: em `render_rgba`, preencher de `bg` clareado (~+18 %) em cima a escurecido (~−18 %) embaixo.
- `graphics/side.rs`: `render(side, info, live)` com o modo `System` (3 blocos em pé).
- Para alinhar texto à esquerda, tornar `renderer::draw_glyphs` `pub(super)`.
- Toda imagem continua passando por `encode_frame` (limite de 3 KB: ver `MAX_JPEG_BYTES`).

### 3. Serviço (`src/service/runtime.rs`)
- `Shared.live: Live`; função `spawn_live(state, Option<Sender<UsbCmd>>)` (fica em `runtime.rs`, porque o
  editor só pode usar `service::runtime`): a cada 2 s calcula `Wants` da página ativa (teclas de volume,
  playpause, Monitor; lateral `System`), consulta com o `Sampler`, e se `Live` mudou guarda e, no serviço,
  redesenha só as teclas afetadas (`redraw_where`) e a lateral (`redraw_side`). Sem nada vivo na página:
  só dorme.
- `Snapshot`/`redraw_where`/`show_pressed`: trocar `render_button_frame(b)` pelo desenho com `Live`
  (guardar uma cópia de `sh.live` no `Snapshot`). O mixer aberto não usa.
- Testes que contam quadros usam `key_frames(&rx)` em `runtime/tests.rs`.

### 4. Editor
- Prévia ao vivo: o editor chama `runtime::spawn_live(sh, None)` e as texturas (`KeyTextures` em
  `editor/mod.rs`) entram com `Live` na chave de cache só para teclas vivas.
- Ação "Monitor" na grade de tipos e na biblioteca (categoria "Aparelho"); modo "Sistema" na janela da
  tela lateral (`editor/side_ui.rs`).
- Aparência: caixa "Fundo em degradê".
- Botão "Embelezar página" (no rodapé do palco, perto das dicas de atalho, ou no menu da página):
  função pura em `editor/beautify.rs`, com testes: para cada tecla configurada, cor da categoria
  (as constantes de `editor/library.rs`: MEDIA, KEYS, APPS, WEB, NAV, DEVICE, OBS), degradê ligado,
  rótulo curto quando vazio (`inspector::key_title`), ícone quando faltar; `touch()` e toast com Desfazer.
- Textos novos em `strings.rs` (o teste recusa chave repetida).

### 5. Fechamento
- `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test` sem falhas.
- Imagem de como fica: `AKP_PREVIEW_CONFIG=... AKP_PREVIEW_OUT=... cargo test device_preview -- --ignored`
  (em `graphics/side.rs`); mandar ao usuário com SendUserFile.
- Prévia do editor: script de monitor virtual do Hyprland (ver `CLAUDE.md`, `AKP153E_PREVIEW_UI` e
  `AKP153E_CONFIG_DIR` com uma cópia da config; nunca a config real).
- Atualizar README (Funções), `docs/DESIGN.md`, `CLAUDE.md` (módulos novos) e `evals/hardware-checklist.md`
  (itens novos de volume, tocando agora, monitor e lateral "Sistema").
- Commits pequenos com mensagem em português, push no `linux-port` e conferir o CI (Windows e Linux).
- O usuário instala com `packaging/arch/atualizar.sh` (pede sudo; não guardar senha em lugar nenhum).

## Ainda pendente de antes (não depende de código)

Teste no aparelho, itens 10 a 16 de `evals/hardware-checklist.md`: tecla que afunda, descanso da tela,
brilho, relógio, OBS, perfil automático e tela lateral (geometria vinda de outro projeto, nunca vista
neste aparelho). Nota atual honesta: 9,5; só vira 10 depois desse teste.
