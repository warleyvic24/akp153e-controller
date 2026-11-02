# Plano de Execução Sênior — Keyforge até Nota 9,5

**Data:** 2026-10-31  
**Autor:** Hermes Agent (execução) + warleyvic (hardware/testes)  
**Base:** `ROTEIRO.md`, `PLANO-PRODUTO.md`, `PLANO-UX.md`, `PLANO-APARELHOS.md`, `evals/hardware-checklist.md`  
**Princípio:** *Planejar como arquiteto, executar como engenheiro sênior — commits pequenos, testes passando, zero regressão, validação real no aparelho a cada fase que toca USB.*

---

## Visão Geral dos Degraus (do ROTEIRO.md)

| Degrau | Meta | Portão de saída | Quem faz |
|--------|------|-----------------|----------|
| **1. Confiança** | 6 | Checklist 21/21 Linux, 10/10 Windows, janela opaca, teste instável consertado | **Usuário (hardware)** + Eu (janela opaca, CI) |
| **2. Alicerce** | 6,5 | `ActionKind` enum, protocolo JSON, config sem 3x5, `cargo deny`, cobertura ≥ 60%, `egui_kittest`, U0 | **Eu (código)** |
| **3. Fundação visual** | 7 | U1, U2, U3; senhas no chaveiro; capturas aprovadas; nenhuma janela crua | **Eu (código)** |
| **4. Editor bom** | 7,5 | U4, U5, U6; fuzz no protocolo/config; teste 24h; 1 semana uso sem queixa | **Eu (código)** + **Usuário (uso real)** |
| **5. Qualquer aparelho** | 8 | Elgato provado por terceiros; giratórios; Fase 4 completa | **Externo (precisa aparelho)** |
| **6. Fácil de ter** | 8,5 | Instaladores assinados, Flatpak/AUR, atualização auto, guia usuário, marca pesquisada | **Usuário (certificado/decisões)** + **Eu (empacotamento)** |
| **7. Mais que os outros** | 9 | Plugins + 10–15 integrações, pastas, multi-ação condicional, i18n, leitor de tela | **Eu (código)** + **Comunidade (plugins)** |
| **8. Provado** | 9,5 | U9 com 3 pessoas, SUS ≥ 85, lançamento, 3 fabricantes provados | **Externo (pessoas reais)** |

**Onde estamos:** Degrau 1 **quase completo** — checklist 12/21 verificados no aparelho (itens 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21). Faltam itens 1–9 (itens 2, 3, 4, 8, 9 cobertos por `--selftest`; 1, 5, 6, 7 manuais). Janela opaca pendente, Windows não testado.
**Degrau 2 (Alicerce):** ✅ 2.1 `ActionKind` (já existia), ✅ 2.2 Protocolo JSON versionado, ✅ 2.3 Config sem grade 3x5 fixa (SCHEMA_VERSION 5), ✅ 2.4 U0 — Régua de capturas (`tools/ui-shots.sh`, `tools/ui-diff.py`, `docs/ui-baseline/`, CI job `ui-regressao`).
**Degrau 3 (Fundação visual):** ✅ U1 concluído — componentes canônicos em `widgets.rs` (`text_field`, `text_area`, `primary_button`, `secondary_button`, `slider`, `combo_box`, `scroll_area`, `checkbox`), teste de arquitetura `no_raw_egui_widgets_outside_widgets_rs` passando, 14 arquivos migrados de egui cru para wrappers canônicos. ✅ U2 concluído — Preferências e Lista de apps usam `widgets::modal` (sem `egui::Window` cru no editor).
**Próximo:** Fechar Degrau 1 (itens 1, 5, 6, 7 no hardware; janela opaca; Windows) → Degrau 3 U3 (barra do topo com selo único de sincronia).

---

## Fase Atual: DEGRAU 1 — CONFIANÇA (Portão: 21/21 Linux + 10/10 Windows)

### 1.1 Checklist do aparelho — itens 10–16 (manual) + 17–21 (feito ✓)
| Item | Descrição | Status | Como testar |
|------|-----------|--------|-------------|
| 10 | Tecla afunda (imagem menor no preto ao segurar) | ✓ ok 2026-10-31 | Segurar tecla com ação |
| 11 | Descanso da tela (1 min → apaga; 1º toque acende, 2º roda) | ⬜ | Prefs > Apagar tela sem uso: 1 min |
| 12 | Teclas de brilho (Biblioteca > Aparelho > "Brilho −"; 3 toques = -30%) | ⬜ | Adicionar tecla, apertar 3× |
| 13 | Relógio (hora/data, vira no minuto) | ✓ ok 2026-10-31 | Biblioteca > Aparelho > "Relógio" |
| 14 | OBS (WebSocket ligado; tecla "Gravar" → vermelha; para ao toque) | ⬜ | OBS > Ferramentas > WebSocket; tecla Gravar |
| 15 | Perfil automático (foco no app → troca; sai → volta) | ⬜ | Perfil > "Ativar sozinho quando…" |
| 16 | Tela lateral (editor > faixa > "Nome da página" → aparelho mostra; "Imagem" → foto inteira) | ⬜ | Editor > clique faixa lateral |
| 17–21 | Widgets vivos + Embelezar | ✓ ok (21: gosto = preto puro) | Já testado |

**Ação imediata do usuário:** Testar 11, 12, 14, 15, 16 e anotar `ok AAAA-MM-DD` ou defeito em `evals/hardware-checklist.md`.

### 1.2 `keyforge --selftest` (automatiza 2, 3, 4, 8, 9, 16)
```bash
keyforge --selftest
# Gera selftest.txt na pasta da config — conferir resultado
```

### 1.3 Janela opaca no Linux (Hyprland/Omarchy)
- Adicionar regra em `packaging/linux/hypr-keyforge.lua`:
  ```lua
  windowrulev2 = "opacity 1 override 1 override, class:keyforge"
  ```
- Incluir no pacote Arch (`packaging/arch/`) e documentar em `docs/LINUX.md`.
- **Critério:** Janela do editor 100% opaca, sem transparência do wallpaper.

### 1.4 Windows 10/11 — teste completo
- Instalador (MSI/MSIX via `build_release.ps1`), serviço, editor, autostart, bandeja, reconexão USB.
- **Responsável:** Usuário (precisa de máquina Windows) ou CI futuro.

---

## Fase Próxima: DEGRAU 2 — ALICERCE (Portão: cobertura ≥ 60%, capturas no CI)

### 2.1 `ActionKind` enum — substituir strings por tipos
**Arquivos:** `src/editor/action_kind.rs`, `src/editor/kind_picker.rs`, `src/config/schema.rs`, `src/engine/actions/`, `src/editor/inspector.rs`
- Criar `enum ActionKind { LaunchApp, Shortcut, OpenUrl, Webhook, Media, Volume, Mixer, Text, MultiAction, PageNavigation, ProfileSwitch, Command, Brightness, Clock, Monitor, Obs, Sound, None, Folder, DialTurnLeft, DialTurnRight, DialPress }` derivado de `ButtonAction`.
- `ButtonAction::kind() -> ActionKind` + `From<ActionKind> for ButtonAction` (default por tipo).
- Compilador passa a avisar `match` incompleto — **elimina bugs de tipo novo não tratado**.

### 2.2 Protocolo editor ↔ serviço: texto → JSON versionado
**Arquivos:** `src/service/mod.rs`, `src/cli.rs`, `src/editor/mod.rs` (conexão TCP 47153)
- Definir schema JSON (`Command`, `Response`, `Event`) com `version: u32`.
- `service::read_command` parseia JSON; mantém compatibilidade com texto por 1 versão (avisa log).
- Testes de contrato: editor envia JSON → serviço responde JSON; round-trip em `service/tests.rs`.

### 2.3 Config sem grade 3x5 fixa
**Arquivos:** `src/config/schema.rs`, `src/config/migrate.rs`, `src/device/model.rs`
- `Page::sanitized(device: &DeviceModel)` usa `device.controls` (teclas + lateral + futuros dials) em vez de `rows x cols`.
- Migração v3 → v4: índice `i` → `control.id` (`k0`..`k14`, `side`).
- `Page::split_for(target_model)` já existe — testar com modelo 6 teclas.

### 2.4 `cargo deny` + `cargo llvm-cov` no CI
- `.github/workflows/ci.yml`: adicionar steps `deny check` e `llvm-cov --summary-only` (meta ≥ 60% linhas fora de `platform/`).
- Corrigir duplicatas de crate avisadas hoje (base64, bitflags, calloop, etc.) — `Cargo.lock` update ou `patch` section.

### 2.5 `egui_kittest` — testes de interface automatizados
**Novo:** `src/editor/ui_tests.rs` (expandir)
- `egui_kittest::harness()` abre editor sem janela real.
- Testes das 5 tarefas U9: (1) pôr app na tecla, (2) mudo microfone vermelho, (3) criar página + navegação, (4) mudar brilho, (5) trocar cor/ícone.
- Comparar screenshots com referência (`tools/ui-shots.sh` → `tests/ui-snapshots/`).

### 2.6 U0 — Régua de capturas (antes de qualquer mudança visual)
```bash
tools/ui-shots.sh  # gera galeria em 1366x768 e 1920x1080, temas claro/escuro, config de teste
```
- Commitar galeria `docs/ui-baseline/` como referência.
- Todo PR visual: `tools/ui-shots.sh` no CI + diff automático.

---

## Fase Futura: DEGRAU 3 — FUNDAÇÃO VISUAL (PLANO-UX Etapas 1–3)

### 3.1 U1 — Fundação visual (componentes únicos)
- `src/editor/widgets.rs`: componentes canônicos (`TextField`, `ButtonPrimary`, `ButtonSecondary`, `ButtonGhost`, `Segmented`, `Switch`, `ListRow`, `Card`, `Modal`, `Toast`).
- Teste: `grep -r "egui::TextEdit\|egui::Button\|egui::Window" src/editor/` → **zero fora de `widgets.rs`**.
- Tipografia: escala 12/14/16/20/24; `tokens.rs` atualizado; monoespaçado só para atalhos.
- Espaçamento: grade 4/8px; raios 6/10/16.

### 3.2 U2 — Superfícies modais (Preferências, Lista de apps)
- `Modal` componente: centralizado, backdrop, foco preso, Esc/fora fecha, animação 150ms.
- **Preferências:** modal com abas laterais (Geral, Aparelho, Integrações, Sobre).
  - Aparelho: brilho **ao vivo** (arrasta → aplica na hora), descanso, modelo, "Ensinar meu aparelho".
  - Integrações: OBS com estado vivo + "Testar conexão".
- **Lista de apps:** modal com ícones reais, "Recentes" no topo, setas/Enter, prévia da tecla no hover.
- **Zero `egui::Window` crua no editor** (teste automático).

### 3.3 U3 — Barra do topo + estado de sincronia
- Um selo só: `● No aparelho` (verde), `Enviando…`, `Desconectado · Reconectar` (vermelho, clicável), `Serviço parado · Iniciar`.
- "Salvo" removido (gravação é automática).
- Seletor de perfil único (menu: perfis + "Novo" + "Gerenciar…").
- Erro de gravação = faixa vermelha fixa até resolver.

---

## Fase Futura: DEGRAU 4 — EDITOR BOM (PLANO-UX Etapas 4–6)

### 4.1 U4 — Palco (aparelho no editor)
- Abas de página coladas ao aparelho: duplo clique renomeia, arrasta reordena, menu ctx (duplicar, excluir c/ Undo).
- Números das teclas **só no hover** e na tecla vazia.
- Valores ao vivo **dentro** da tecla (volume, CPU) — nada vazando.
- Barra de atalhos: 3 primeiras aberturas → depois "?" (folha de atalhos).
- "Embelezar" → **"Organizar cores"** com prévia antes/depois + aviso "Ctrl+Z desfaz".
- Seleção múltipla (Shift/Ctrl): mudar cor, limpar, mover em grupo.

### 4.2 U5 — Biblioteca de ações
- Ordem: **Busca** → **Recentes (6)** → **Seus apps (8, ícones reais)** → Mídia/Som → Páginas → OBS → Sistema → Web → Avançado → Edição (recolhida).
- Linhas 32px, ícone + nome, hover: "Pôr na tecla 3 ↵".
- Arrastar: fantasma = tecla desenhada (ícone + cor).
- Seções recolhíveis e lembradas.

### 4.3 U6 — Inspetor
- **Cabeçalho:** prévia 96px, nome editável, resumo 1 linha ("Abre: Discord").
- **Ações da tecla:** "Testar" = botão secundário; copiar/colar/duplicar/limpar → menu "⋯" com atalhos.
- **Aba Ação:**
  - App: 1 campo ícone+nome + "Trocar"; caminho/args em "Avançado".
  - Atalho: só gravador ("Clique e aperte"); texto em "Avançado".
  - Web: só URL; método/corpo em "Avançado".
  - "Ao segurar" e "Dois estados" = seções recolhíveis dentro de Ação (aba "Alternar" some).
- **Aba Aparência:** prévia 2 estados lado a lado; cores com nome no hover; "Cor personalizada…"; ícones categorizados + recentes + "Sugeridos para esta ação".
- Erros **no campo** (ex.: "URL precisa começar com http"), não só aviso na tecla.
- **Aceite:** inspetor de tecla de app cabe em 768px sem rolar.

---

## Fases 5–8: Resumo (dependem de externo)

| Fase | O que precisa | Bloqueador |
|------|---------------|------------|
| 5. Qualquer aparelho | Elgato Mini/MK.2/XL/Plus provado por alguém | Aparelho físico |
| 6. Fácil de ter | Certificado assinatura (Windows), Flatpak/AUR, marca pesquisada | Dinheiro/decisão do dono |
| 7. Mais que os outros | Plugins (Spotify, Discord, HA, Twitch, Hue, Pomodoro, PipeWire, Hyprland), pastas, multi-ação condicional, GIF, variáveis, i18n ES/DE, leitor tela | Semanas de código + comunidade |
| 8. Provado | 3 pessoas reais, 5 tarefas, SUS ≥ 85 | Pessoas externas |

---

## Regras de Engenharia (invioláveis)

1. **Commits atômicos** — um assunto, testes passam, `cargo fmt` + `clippy -D warnings`.
2. **Zero regressão visual** — `tools/ui-shots.sh` antes e depois de toda mudança de UI; diff no PR.
3. **Config real nunca tocada por testes** — `AppConfig::path()` em `cfg!(test)` = temp dir (já existe, manter).
4. **Trava `config.lock`** — toda leitura/escrita de config dentro de `AppConfig::locked`.
5. **Mutex `SharedState`** — segurar só para copiar; desenhar/JPEG/IO fora do lock (`Snapshot`, `Followup`).
6. **Unsafe só em `platform/`** — com `// SAFETY:` comentário.
7. **Arquivos ≤ 800 linhas** — separar antes de crescer (`inspector.rs`, `stage.rs`, `widgets.rs` → submodules).
8. **Camadas** — editor usa só `service::runtime`; serviço nunca usa editor (conferido por `architecture_tests.rs`).
9. **Documentar decisões** — `docs/DECISOES.md` (criar) com ADRs curtos: contexto, decisão, consequências.

---

## Próximos Passos Imediatos (Esta Sessão)

### Eu (código):
1. [ ] Criar `docs/PLANO-EXECUCAO.md` (este arquivo) ✓
2. [ ] Adicionar regra Hyprland opaca em `packaging/linux/hypr-keyforge.lua` + `docs/LINUX.md`
3. [ ] Iniciar **Degrau 2.1**: `ActionKind` enum (começar por `src/editor/action_kind.rs`)

### Usuário (hardware):
1. [ ] Testar checklist itens 11, 12, 14, 15, 16 no aparelho
2. [ ] Rodar `keyforge --selftest` e conferir `selftest.txt`
3. [ ] Atualizar `evals/hardware-checklist.md` com resultados
4. [ ] Tirar foto do editor (aba Aparência + "Organizar cores")
5. [ ] Testar no Windows se possível

---

## Métricas de Sucesso por Degrau

| Degrau | Métrica técnica | Métrica de usuário |
|--------|-----------------|-------------------|
| 1 | 21/21 checklist, 0 crashes 24h | "Funciona no meu aparelho" |
| 2 | Cobertura ≥ 60%, 0 `unwrap` fora de testes, `ActionKind` compilando | — |
| 3 | 0 janelas cruas, 0 campos altura errada, contraste ≥ 4.5 em todos textos | "Não parece programa de programador" |
| 4 | 5 tarefas U9 < 30s cada, 0 erros em 3 pessoas | SUS ≥ 85 |
| 5 | 3 fabricantes `verified` | "Meu aparelho funciona" |
| 6 | Instala em < 5 min, auto-update funciona | "Fácil de ter" |
| 7 | 1 plugin terceiros funcionando | "Mais que os outros" |
| 8 | Lançamento público, 0 issues "tecla não responde" | Nota 10 real |

---

## Riscos e Mitigações

| Risco | Probabilidade | Impacto | Mitigação |
|-------|---------------|---------|-----------|
| egui limitações visuais (sombras, blur) | Alta | Médio | Apostar em espaçamento, hierarquia, cor — não imitar macOS |
| Arquivos > 800 linhas | Média | Alto | Separar por componente **antes** de cada etapa (ex.: `stage/controls.rs`) |
| Regressão visual silenciosa | Média | Alto | Galeria de capturas no CI em todo commit UI |
| Avaliação só por capturas (não clicando) | Alta | Médio | Usuário usa 5 min ao fim de cada etapa visual; incômodos → próxima etapa |
| Falta aparelhos para testar | Alta | Alto | Fase 5+ só avança com voluntários; `experimental` até prova |
| Certificado Windows custa dinheiro | Certa | Médio | SignPath grátis para open source ou US$ 100-300/ano; decidir no Degrau 6 |

---

## Comandos de Referência Rápida

```bash
# Testes
cargo test                    # rápido
cargo test -- --ignored       # testes com hardware/OBS real
cargo clippy --all-targets -D warnings
cargo fmt --check
cargo deny check
cargo llvm-cov --summary-only # precisa LLVM_COV=/usr/bin/llvm-cov LLVM_PROFDATA=/usr/bin/llvm-profdata

# Build
cargo build --release         # -> target/release/keyforge

# Instalação Arch (roda como usuário, pede sudo uma vez)
packaging/arch/atualizar.sh

# Diagnóstico
keyforge --probe              # -> keyforge_probe.txt
keyforge --selftest           # -> selftest.txt (guided test no aparelho)

# Preview UI (debug)
KEYFORGE_PREVIEW_THEME=dark KEYFORGE_CONFIG_DIR=/tmp/test-config cargo run -- --editor
KEYFORGE_PREVIEW_UI=key=5,tab=1,settings,picker,library cargo run -- --editor
```

---

## Próxima Ação Concreta

**Eu vou começar o Degrau 2.1 (`ActionKind` enum) agora.**  
**Você fecha o Degrau 1 no hardware.**

Quando eu terminar o `ActionKind` (compile + testes passam), aviso para você revisar. Depois vou para 2.2 (protocolo JSON).

**Confirma?****Plano criado em `docs/PLANO-EXECUCAO.md`** — segue a estrutura do `ROTEIRO.md` com portões claros, responsabilidades divididas e regras de engenharia inegociáveis.

---

## Iniciando **Degrau 2.1: `ActionKind` enum** (substituir strings por tipos)

Vou começar pelo arquivo central: `src/editor/action_kind.rs`.
<tool_call>
<function=read_file>
<parameter=path>
/home/warleyvic/Documents/AKP153E/akp153e-controller/src/editor/action_kind.rs