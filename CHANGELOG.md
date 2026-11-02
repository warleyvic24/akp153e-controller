# Changelog — Keyforge (AKP153E Controller)

Todas as mudanças notáveis deste projeto serão documentadas neste arquivo.

O formato é baseado em [Keep a Changelog](https://keepachangelog.com/pt-BR/1.0.0/),
e este projeto adota [Versionamento Semântico](https://semver.org/lang/pt-BR/).

## [0.1.1] — 2026-11-02

### ✨ Adicionado
- **Segurança (P0):** Confirmação de importação de perfil com listagem de ações de risco (shell, webhook, texto digitado) antes de aplicar
- **Segurança (P0):** Tokens de webhook removidos dos logs — apenas `scheme://host` são registrados
- **Segurança (P0):** Porta de controle Unix socket (`$XDG_RUNTIME_DIR/keyforge/`) / named pipe (Windows) com autenticação de usuário
- **Segurança (P0):** Exportação com opção de redação de segredos ("Exportar sem segredos…")
- **Observabilidade:** `tracing` com spans estruturados para USB, render, save e controle
- **Observabilidade:** Endpoint `/health` na porta 47153 (HTTP e comando de controle)
- **Observabilidade:** Métricas Prometheus (frames enviados, tempo de render, erros USB, reconexões)
- **Observabilidade:** Rotação de log (10 MB × 5 arquivos, permissão 0600)
- **Observabilidade:** Nomes de threads (`usb-poller`, `usb-events`, `control`, `obs`, etc.)
- **Observabilidade:** Suporte opcional a `puffin` profiling via feature `profiling`
- **Performance:** Cache de imagem LRU (50 MB, `RwLock`) para imagens decodificadas
- **Performance:** Cache de GIF com frames em memória e correção de bug de escala de cinza
- **Performance:** Save assíncrono via canal `mpsc` — não trava a thread da UI
- **Performance:** Hash JPEG BLAKE3 por tecla para pular redesenho de teclas inalteradas
- **Performance:** `busctl` com timeout de 5 segundos
- **Performance:** TTL de 1 hora para cache de capas de música
- **Documentação:** `CONTRIBUTING.md` com guia de contribuição
- **Documentação:** Release notes detalhados na tag `v0.1.1`

### 🐛 Corrigido
- CI/CD Windows: release era criada *depois* dos uploads (upload_url não existia)
- CI/CD Windows: `actions/upload-release-asset@v1` (depreciado) substituído por `softprops/action-gh-release@v2`
- Erro de compilação E0382 em `src/telemetry/health.rs` (borrow de valor movido)
- Warns de código morto em `src/graphics/cache.rs` e `src/graphics/gif.rs`

### 🔧 Alterado
- Tipos de erro tipados com `thiserror`: `UsbError`, `ConfigError`, `AudioError`, `ShellError`
- Mutex padrão usa `crate::sync::lock()` com recuperação de mutex envenenado
- 7 chamadas `unwrap()`/`expect()` removidas dos caminhos de produção
- 20 blocos `unsafe` com comentário `// SAFETY:` documentado

### 📊 Métricas
- 413 testes unitários + 6 de CLI passando (0 falhas)
- `cargo clippy --all-targets -- -D warnings`: zero warnings
- `cargo fmt --check`: limpo
- `cargo deny check`: advisories, bans, licenses e fontes OK
- Render de página: ~2,7 ms com cache (release)
- Save de config: ~10 ms (assíncrono, fora da UI)

---

## [0.1.0] — 2026-10-31

### ✨ Adicionado
- Versão inicial do Keyforge para AKP153E
- Suporte a Linux (service + editor)
- Suporte a Windows (build CI/CD, instalador MSI)
- Protocolo USB versionado para o aparelho
- Editor GUI com egui/eframe
- Mixer de áudio (pactl Linux / Core Audio Windows)
- Controle de OBS via WebSocket
- Configuração atômica com migração de esquema (v1→v5)

---

*Versões anteriores não seguem este formato de changelog.*
