# AKP153E Open Controller

[English](README.md) · [![CI](https://github.com/warleyvic24/akp153e-controller/actions/workflows/ci.yml/badge.svg)](https://github.com/warleyvic24/akp153e-controller/actions/workflows/ci.yml) · [Licença MIT](LICENSE)

Software de controle não oficial e de código aberto, para Windows e Linux, para o stream controller AJAZZ AKP153E (15 teclas LCD), escrito em Rust com egui/eframe. Substitui o aplicativo do fabricante e não contém código, firmware nem arquivos dele.

> **Status: inicial (v0.1).** O desenho das teclas, o editor, as ações de app/webhook/atalho, o autostart e a bandeja estão verificados no aparelho real. Aperto e soltura físicos e pressão longa **ainda não foram verificados**; a faixa lateral é uma tela sem botões no AKP153E e não é usada — veja [`evals/hardware-checklist.md`](evals/hardware-checklist.md). Este projeto não tem ligação com a AJAZZ.

> **Sem afiliação.** Projeto independente, não feito, endossado nem suportado pela AJAZZ. "AJAZZ" e "AKP153E" são marcas ou nomes de produto dos respectivos donos e aparecem aqui só para dizer com qual aparelho o programa funciona.

## Funções

- **Editor** com prévia ao vivo de cada tecla (a mesma imagem que vai ao aparelho), arrastar e soltar, copiar/colar, desfazer/refazer, páginas e perfis.
- **Ações:** atalho de teclado, texto, abrir app ou site, comando no shell, webhook, teclas de mídia, multi-ação com pausas, navegação entre páginas e perfis, mixer de volume no aparelho, teclas de dois estados e pressão longa.
- **OBS Studio** (obs-websocket 5, embutido no OBS 28+): trocar cena, transmitir, gravar, pausar, mudo de entrada, câmera virtual, buffer de replay. As teclas acompanham o OBS ao vivo: vermelhas gravando, no ar ou mudas; a cena atual acende.
- **Teclas vivas:** relógio com hora e data; teclas do **aparelho** para subir e baixar o brilho.
- **Visual:** 2000 ícones com busca ([Lucide](https://lucide.dev), em português ou inglês), suas imagens PNG/JPG/SVG/ICO (solte o arquivo sobre a tecla), rótulo de até três linhas no topo, no meio ou embaixo.
- **Perfis automáticos:** um perfil entra quando o app dele ganha o foco (Windows, Hyprland, Sway e X11) e o anterior volta quando você sai.
- **Exportar/importar perfil** num arquivo só, com as imagens das teclas dentro.
- **Descanso da tela** após o tempo escolhido sem uso; o primeiro toque só acende.
- Temas escuro, claro e Omarchy; português e inglês.

## Como funciona

Um executável, dois modos:

| Modo | Comando | Função |
|------|---------|--------|
| Serviço | `akp153e --background` | Bandeja, sem janela. Cuida do USB/HID, das ações e do desenho das teclas. |
| Editor | `akp153e --editor` | Janela egui. Edita o `config.json` (`%APPDATA%\AJazz\AKP153E` no Windows, `~/.config/AJazz/AKP153E` no Linux) e avisa o serviço por `127.0.0.1:47153`. |

Outras opções: `--probe` (grava `akp153e_probe.txt`), `--run-action <json>`, `--autostart on|off`.

O aparelho enumera como HID `0300:3010` e é tratado como protocolo v3. O protocolo segue documentação publicamente disponível; este projeto não fez engenharia reversa do aplicativo oficial.

## Compilar

Precisa de Rust estável recente (1.89 ou mais novo) e:

- **Windows:** as ferramentas de build MSVC (o compilador de recursos do Windows SDK embute o ícone).
- **Linux:** GTK 3, libayatana-appindicator, libudev, libxkbcommon e `pactl`; uma regra udev dá ao seu usuário acesso ao aparelho. Pacotes, script de instalação e o que foi testado (Omarchy / Hyprland) estão em [`docs/LINUX.md`](docs/LINUX.md). No Arch: `cd packaging/arch && makepkg -si`.

```
cargo build --release   # target/release/akp153e (akp153e.exe no Windows)
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

No Windows, `cargo build --release` falha com "Acesso negado" enquanto o serviço roda; pare antes o processo `--background`.

## Segurança

A porta de controle escuta só em localhost (no Linux, conexões de outros usuários são recusadas), e o `config.json` guarda URLs de webhook, caminhos de programas e a senha do WebSocket do OBS em texto puro (no Linux, legível só pelo dono). Detalhes e como reportar estão em [SECURITY.md](SECURITY.md).

## Contribuir

Issues e pull requests são bem-vindos. Leia o [`CLAUDE.md`](CLAUDE.md) (convenções) e o [`docs/DESIGN.md`](docs/DESIGN.md) (regras da interface). O CI roda formatação, clippy e testes a cada push.

## Licença

[MIT](LICENSE). As licenças de terceiros estão em [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md); gere de novo com `python tools/third_party_notices.py` ao mudar dependências.
