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
- Nenhum arquivo acima de 800 linhas (hoje: `main.rs` ~690, `schema.rs` ~700). Painéis em `panes.rs`, barras em `bars.rs`, palco em `stage.rs`, CLI em `cli.rs`.
- Unsafe só em `shell.rs`, sempre com `// SAFETY:`.

## Hardware

O aparelho do usuário enumera como HID `0300:3010` (usage page FFA0), tratado como protocolo v3 (JPEG 100 px, soltura reportada, pacote de 1024 com recuo para 512). O protocolo vem de documentação pública, não de engenharia reversa própria.
**Verificado:** desenho das teclas (pelo log e pelo aparelho conectado), interface, ações de app/webhook/atalho, autostart, bandeja.
**Não verificado:** aperto e soltura físicos das teclas, pressão longa pelo hardware, as 3 teclas laterais (ignoradas).
Não afirme que "funciona" no aparelho sem prova do usuário.
