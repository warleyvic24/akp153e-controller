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

## Feito em 2026-10-31 (segunda parte)

Os passos 1 a 5 do plano: `Monitor`/`Metric`, `gradient` e `SideScreen::System` na config;
`graphics/live_art.rs` (volume, tocando agora, monitor, lateral "Sistema"); `service/runtime/live.rs`
(thread de 2 s, redesenha só as teclas afetadas; o editor liga a mesma thread sem `tx` para a prévia);
editor com a ação "Monitor" (`editor/monitor_ui.rs`), modo "Sistema" da lateral, caixa "Fundo em degradê"
e o botão "Embelezar página" (`editor/beautify.rs`). Prévia do aparelho conferida em PNG
(`device_preview`); a foto do editor não foi tirada porque a tela estava bloqueada.

## Falta

- Foto do editor (rodapé com "Embelezar página", aba Aparência) quando a tela estiver desbloqueada.
- Instalar (`packaging/arch/atualizar.sh`) e conferir os itens 17 a 21 da checklist no aparelho.

## Ainda pendente de antes (não depende de código)

Teste no aparelho, itens 10 a 16 de `evals/hardware-checklist.md`: tecla que afunda, descanso da tela,
brilho, relógio, OBS, perfil automático e tela lateral (geometria vinda de outro projeto, nunca vista
neste aparelho). Nota atual honesta: 9,5; só vira 10 depois desse teste.
