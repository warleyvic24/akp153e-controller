# AKP153E no Linux

O Linux é compilado e testado pelo CI (Ubuntu), mas **ainda não foi provado com o aparelho**.
Os passos para provar estão no fim; se algo falhar, anote a mensagem do log.

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
`99-akp153e.rules` em `/etc/udev/rules.d/` (pede `sudo`). Depois, desconecte e reconecte o aparelho.

```
akp153e --background     # serviço (bandeja, aparelho)
akp153e --editor         # editor
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

- **Iniciar com a sessão:** o Hyprland não executa o autostart XDG por conta própria. Acrescente em
  `~/.config/hypr/autostart.conf`:
  `exec-once = uwsm app -- akp153e --background`
- **Janela do editor:** o `app_id` é `akp153e`. Para ela abrir flutuando, em `~/.config/hypr/windows.conf`:
  `windowrule = float, class:^(akp153e)$`
- **Bandeja:** o Waybar mostra o ícone pelo StatusNotifierItem. No Omarchy a bandeja pode ficar
  recolhida atrás do ícone de expandir.
- **Trazer o editor para a frente:** feito pelo `hyprctl` (já instalado com o Hyprland).
- **Atalhos de teclado:** a build do Linux usa o protocolo de teclado virtual do Wayland, que o Hyprland
  aceita, para as teclas chegarem também aos apps nativos do Wayland. Isso ainda não foi provado.
- **Mixer:** o `pactl` vem do `libpulse`; o `pipewire-pulse` do Omarchy atende os mesmos comandos.

## Diferenças em relação ao Windows

- Os ícones dos apps não são extraídos (o mixer mostra o nome).
- "Escolher app instalado" lê os atalhos `.desktop`.
- As ações Calculadora, Bloco de notas e Explorador da biblioteca não existem; use "Aplicativo...".
- Atalhos de teclado usam o `enigo`; no Wayland alguns compositores bloqueiam teclas simuladas.

## Provar no aparelho

1. `akp153e --probe` e abra `akp153e_probe.txt`: o aparelho deve aparecer em "Interfaces HID".
   Se não aparecer, a regra udev não valeu (confira `ls -l /dev/hidraw*`).
2. `akp153e --background`: as teclas devem desenhar.
3. Aperte uma tecla com ação de atalho ou de app.
4. Ação Mixer: abra um player e confira o volume na tela do aparelho.
