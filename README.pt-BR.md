# AKP153E Open Controller

[English](README.md) · [![CI](https://github.com/warleyvic24/akp153e-controller/actions/workflows/ci.yml/badge.svg)](https://github.com/warleyvic24/akp153e-controller/actions/workflows/ci.yml) · [Licença MIT](LICENSE)

Software de controle não oficial, de código aberto e só para Windows, para o stream controller AJAZZ AKP153E (15 teclas LCD), escrito em Rust com egui/eframe. Substitui o aplicativo do fabricante e não contém código, firmware nem arquivos dele.

> **Status: inicial (v0.1).** O desenho das teclas, o editor, as ações de app/webhook/atalho, o autostart e a bandeja estão verificados no aparelho real. Aperto e soltura físicos e pressão longa **ainda não foram verificados**; a faixa lateral é uma tela sem botões no AKP153E e não é usada — veja [`evals/hardware-checklist.md`](evals/hardware-checklist.md). Este projeto não tem ligação com a AJAZZ.

> **Sem afiliação.** Projeto independente, não feito, endossado nem suportado pela AJAZZ. "AJAZZ" e "AKP153E" são marcas ou nomes de produto dos respectivos donos e aparecem aqui só para dizer com qual aparelho o programa funciona.

## Como funciona

Um executável, dois modos:

| Modo | Comando | Função |
|------|---------|--------|
| Serviço | `akp153e.exe --background` | Bandeja, sem janela. Cuida do USB/HID, das ações e do desenho das teclas. |
| Editor | `akp153e.exe --editor` | Janela egui. Edita `%APPDATA%\AJazz\AKP153E\config.json` e avisa o serviço por `127.0.0.1:47153`. |

Outras opções: `--probe` (grava `akp153e_probe.txt`), `--run-action <json>`, `--autostart on|off`.

O aparelho enumera como HID `0300:3010` e é tratado como protocolo v3. O protocolo segue documentação publicamente disponível; este projeto não fez engenharia reversa do aplicativo oficial.

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

[MIT](LICENSE). As licenças de terceiros estão em [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md); gere de novo com `python tools/third_party_notices.py` ao mudar dependências.
