# AKP153E no Linux

O Linux é compilado e testado pelo CI (Ubuntu). No aparelho (Omarchy, 2026-10-29) já foram provados:
o acesso USB pela regra udev, a imagem numa tecla e o aperto e a soltura de uma tecla. Falta o resto
da lista no fim; se algo falhar, anote a mensagem do log.

## Dependências

Para compilar (Debian/Ubuntu):

```
sudo apt install pkg-config fonts-dejavu-core libudev-dev libgtk-3-dev \
  libayatana-appindicator3-dev libxdo-dev libxkbcommon-dev libwayland-dev libx11-dev libxcb1-dev
```

Para usar:

| Recurso | Precisa de |
|---|---|
| Aparelho (USB) | regra udev (abaixo) |
| Texto nas teclas | uma fonte: DejaVu, Liberation ou Noto |
| Mixer de volume | `pactl` (PulseAudio ou PipeWire com `pipewire-pulse`) |
| Bandeja | GTK 3 e AppIndicator (no GNOME, a extensão "AppIndicator") |
| Trazer o editor para a frente | `wmctrl` (opcional) |
| Abrir sites | `xdg-open` |

## Instalar

```
cargo build --release
packaging/linux/install.sh
```

O script copia o programa para `~/.local/bin`, cria o atalho no menu e instala a regra
`70-akp153e.rules` em `/etc/udev/rules.d/` (pede `sudo`). Depois, desconecte e reconecte o aparelho.

```
akp153e                  # inicia o serviço (se parado) e abre o editor: é o que o atalho do menu usa
akp153e --background     # só o serviço (bandeja, aparelho)
akp153e --editor         # só o editor; sem o serviço ele mostra "Serviço parado" e o botão para iniciá-lo
akp153e --autostart on   # inicia com a sessão (~/.config/autostart)
```

A configuração fica em `~/.config/AJazz/AKP153E/config.json`.

## Omarchy (Arch Linux + Hyprland)

Omarchy usa Wayland (Hyprland), PipeWire e Waybar. O que muda:

```
sudo pacman -S --needed base-devel cargo gtk3 libayatana-appindicator libxkbcommon libpulse ttf-dejavu
cd packaging/arch && makepkg -si      # compila, instala o binário, a regra udev e o atalho
```

Depois reconecte o aparelho (a regra udev vale para o usuário logado).

- **Iniciar com a sessão:** no Omarchy basta Preferências > "Iniciar com o sistema" (ou
  `akp153e --autostart on`): a sessão roda pelo `uwsm`, que executa o autostart XDG
  (`~/.config/autostart`). Confira com `systemctl --user is-active xdg-desktop-autostart.target`.
  Num Hyprland sem `uwsm`, que ignora o autostart XDG, use na config em Lua
  `o.launch_on_start("akp153e --background")` ou, na antiga, `exec-once = akp153e --background`.
- **Cores do Omarchy:** em Preferências > Tema, a opção "Omarchy" (que já vem escolhida na primeira
  abertura dentro do Omarchy) usa as cores, o destaque e o modo claro/escuro do tema ativo, lidos de
  `~/.local/state/omarchy/current/theme/colors.toml`. Ao trocar o tema no Omarchy, o editor acompanha
  em até 2 segundos, sem reabrir. O contraste do texto e dos controles é corrigido se o tema for fraco
  (a regra WCAG do projeto vale para qualquer tema). As fontes de texto e monoespaçada vêm do `fc-match`.
- **Janela do editor:** o `app_id` é `akp153e`. Para ela abrir flutuando, na config em Lua:
  `o.window("^(akp153e)$", { tag = "+floating-window" })`
  Na config antiga (`windows.conf`): `windowrule = float, class:^(akp153e)$`
- **Bandeja:** o Waybar mostra o ícone pelo StatusNotifierItem. No Omarchy a bandeja pode ficar
  recolhida atrás do ícone de expandir.
- **Trazer o editor para a frente:** feito pelo `hyprctl` (já instalado com o Hyprland), na sintaxe
  antiga ou, se o Hyprland a recusar (config em Lua), na nova `hl.dsp.focus`.
- **Atalhos de teclado:** a build do Linux usa o protocolo de teclado virtual do Wayland, que o Hyprland
  aceita, para as teclas chegarem também aos apps nativos do Wayland. Isso ainda não foi provado.
- **Mixer:** o `pactl` vem do `libpulse`; o `pipewire-pulse` do Omarchy atende os mesmos comandos.

## Diferenças em relação ao Windows

- Os ícones dos apps não são extraídos (o mixer mostra o nome).
- "Escolher app instalado" lê os atalhos `.desktop`.
- As ações Calculadora, Bloco de notas, Explorador e Área de trabalho da biblioteca não existem;
  para programas, use "Aplicativo...".
- "Captura" aperta a tecla Print (GNOME, KDE e Omarchy a ligam à captura de tela) e "Bloquear" roda
  `loginctl lock-session`, em vez dos atalhos com a tecla Windows.
- Atalhos de teclado usam o `enigo`; no Wayland alguns compositores bloqueiam teclas simuladas.

## Provar no aparelho

1. `akp153e --probe` e abra `akp153e_probe.txt`: o aparelho deve aparecer em "Interfaces HID".
   Se aparecer com "Permission denied", a regra udev não valeu: confira `getfacl /dev/hidrawN` (deve
   listar o seu usuário). A regra precisa ter número menor que 73 (por isso `70-`); com `99-` a
   marca `uaccess` chega depois do `73-seat-late.rules` do systemd e não dá acesso.
   **ok 2026-10-29**: acesso, imagem na tecla 1 e aperto/soltura da tecla 1.
2. `akp153e --background`: as teclas devem desenhar.
3. Aperte uma tecla com ação de atalho ou de app. Rode o `--probe` e aperte as 15 teclas: cada uma
   deve aparecer como `IN[...] 41 43 4B ...`. Em 2026-10-29 só a tecla 1 apareceu, e numa segunda
   rodada o aparelho caiu do USB 8 vezes enquanto as teclas eram apertadas (`dmesg`: `USB disconnect`)
   sem a tela reiniciar; falta repetir em outra porta e cabo antes de concluir se é defeito.
4. Ação Mixer: abra um player e confira o volume na tela do aparelho.
