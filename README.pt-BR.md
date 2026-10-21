# AJAZZ AKP153E Controller

[English](README.md) · [![CI](https://github.com/warleyvic24/akp153e-controller/actions/workflows/ci.yml/badge.svg)](https://github.com/warleyvic24/akp153e-controller/actions/workflows/ci.yml) · [Licença MIT](LICENSE)

Substituto de código aberto, só para Windows, do aplicativo oficial do AJAZZ AKP153E (stream controller com 15 teclas LCD), escrito em Rust com egui/eframe.

> **Status: inicial (v0.1).** O desenho das teclas, o editor, as ações de app/webhook/atalho, o autostart e a bandeja estão verificados no aparelho real. Aperto e soltura físicos, pressão longa e as 3 teclas laterais **ainda não foram verificados** — veja [`evals/hardware-checklist.md`](evals/hardware-checklist.md). Este projeto não tem ligação com a AJAZZ.

## Como funciona

Um executável, dois modos:

| Modo | Comando | Função |
|------|---------|--------|
| Serviço | `akp153e.exe --background` | Bandeja, sem janela. Cuida do USB/HID, das ações e do desenho das teclas. |
| Editor | `akp153e.exe --editor` | Janela egui. Edita `%APPDATA%\AJazz\AKP153E\config.json` e avisa o serviço por `127.0.0.1:47153`. |

Outras opções: `--probe` (grava `akp153e_probe.txt`), `--run-action <json>`, `--autostart on|off`.

O aparelho enumera como HID `0300:3010` e é tratado como protocolo v3. O protocolo vem de documentação pública, não de engenharia reversa própria.

## Compilar

Precisa de Windows, Rust estável recente e as ferramentas de build MSVC (o compilador de recursos do Windows SDK embute o ícone).

```
cargo build --release   # target\release\akp153e.exe
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

`cargo build --release` falha com "Acesso negado" enquanto o serviço roda; pare antes o processo `--background`.

## Segurança

A porta de controle aceita qualquer processo local, e o `config.json` guarda URLs de webhook e caminhos de programas em texto puro. Detalhes e como reportar estão em [SECURITY.md](SECURITY.md).

## Contribuir

Issues e pull requests são bem-vindos. Leia o [`CLAUDE.md`](CLAUDE.md) (convenções) e o [`docs/DESIGN.md`](docs/DESIGN.md) (regras da interface). O CI roda formatação, clippy e testes a cada push.

## Licença

[MIT](LICENSE)
