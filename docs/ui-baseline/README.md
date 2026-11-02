# Galeria de referência de capturas de UI

Esta pasta contém as capturas de referência (`docs/ui-baseline/`) usadas pelo teste
de regressão visual (`tools/ui-diff.py`) no CI (`ui-regressao` job).

## Como gerar a baseline pela primeira vez

```bash
# 1. Build em debug
cargo build

# 2. Copiar a config real para uma pasta temporária (não grava na real)
mkdir -p /tmp/ui-config
cp ~/.config/Keyforge/config.json /tmp/ui-config/config.json  # Linux
# ou %APPDATA%\Keyforge\config.json  # Windows

# 3. Rodar o script de capturas (Hyprland no Omarchy)
KEYFORGE_PREVIEW_THEME=dark sh tools/ui-shots.sh docs/ui-baseline /tmp/ui-config
KEYFORGE_PREVIEW_THEME=light sh tools/ui-shots.sh docs/ui-baseline /tmp/ui-config
```

No CI (GitHub Actions Ubuntu), o job `ui-regressao` roda com `Xvfb` e compara
automaticamente as capturas com esta pasta. Qualquer diferença faz o job falhar.

## Como atualizar a baseline (após revisar diferenças)

```bash
cp <pasta-de-capturas-atual>/*.png docs/ui-baseline/
git add docs/ui-baseline/
git commit -m "ui: atualiza galeria de referência"
```

## Telas capturadas (por tema: dark, light)

- `palco-<theme>.png` — aparelho com teclas (1 vazia, 1 configurada)
- `biblioteca-<theme>.png` — aba Biblioteca de ações
- `aparencia-<theme>.png` — aba Aparência do inspetor
- `preferencias-<theme>.png` — janela Preferências (modal)
- `apps-<theme>.png` — Lista de apps (modal)
- `vazia-<theme>.png` — Página vazia com pontos de partida
- `primeira_execucao-<theme>.png` — Tela de boas-vindas
