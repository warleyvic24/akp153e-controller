#!/bin/sh
# Capturas do editor (debug) para regressão visual.
# Uso: tools/ui-shots.sh <pasta de saída> <pasta com cópia do config.json>
#
# Roda o editor em debug com KEYFORGE_PREVIEW_UI para cada tela,
# nos dois temas, e captura PNG.
#
# No Hyprland (máquina de desenvolvimento): cria um monitor headless e
# usa `grim` para capturar.
# No CI (Xvfb): usa o próprio Xvfb do job e captura com `import` (ImageMagick)
# ou `scrot`.
#
# As imagens vão para o PR como galeria de referência (antes/depois).
set -eu

OUT=${1:?pasta de saída}
CFG=${2:?pasta com a cópia do config.json}
BIN=$(dirname "$0")/../target/debug/keyforge
WAIT=${WAIT:-4}

mkdir -p "$OUT"

# -------------------------------------------------------------------
# Detecta o ambiente: Hyprland (dev) ou Xvfb (CI)
# -------------------------------------------------------------------
IS_HYPRLAND=0
if [ -n "${HYPRLAND_INSTANCE_SIGNATURE:-}" ]; then
    IS_HYPRLAND=1
elif hyprctl monitors -j >/dev/null 2>&1; then
    IS_HYPRLAND=1
fi

if [ "$IS_HYPRLAND" -eq 1 ]; then
    echo "Ambiente detectado: Hyprland"
    # Garante um monitor headless para capturas isoladas
    hyprctl monitors -j | grep -q '"HEADLESS' || hyprctl output create headless >/dev/null 2>&1 || true
    MON=$(hyprctl monitors -j | python3 -c "import json,sys;print([m['name'] for m in json.load(sys.stdin) if m['name'].startswith('HEADLESS')][0])" 2>/dev/null || echo "")
    if [ -z "$MON" ]; then
        echo "Aviso: não foi possível criar monitor headless; capturas podem falhar"
    fi
    WS=$(hyprctl monitors -j | python3 -c "import json,sys;print([m['activeWorkspace']['id'] for m in json.load(sys.stdin) if m['name'].startswith('HEADLESS')][0])" 2>/dev/null || echo "1")
else
    echo "Ambiente detectado: Xvfb/genérico (CI)"
    if ! pgrep -f "Xvfb" >/dev/null 2>&1; then
        Xvfb :99 -screen 0 1920x1080x24 &
        sleep 2
    fi
    export DISPLAY=:99
fi

pkill -f "[t]arget/debug/keyforge --editor" 2>/dev/null || true

# -------------------------------------------------------------------
# Captura uma tela
#   $1 = nome do arquivo (sem extensão)
#   $2 = valor de KEYFORGE_PREVIEW_UI
#   $3 = tema (dark|light)
# -------------------------------------------------------------------
shot() {
    name="$1"
    preview="$2"
    theme="$3"

    if [ "$IS_HYPRLAND" -eq 1 ]; then
        hyprctl dispatch "hl.dsp.dpms({ action = \"enable\", monitor = \"$MON\" })" >/dev/null 2>&1 || true
        hyprctl dispatch "hl.dsp.exec_cmd(\"env KEYFORGE_CONFIG_DIR=$CFG KEYFORGE_PREVIEW_UI=$preview KEYFORGE_PREVIEW_THEME=$theme $BIN --editor\", { workspace = \"$WS silent\" })" >/dev/null 2>&1 || true
        sleep "$WAIT"
        grim -o "$MON" "$OUT/${name}-${theme}.png" 2>/dev/null || echo "Aviso: grim falhou para $name-$theme"
    else
        # CI com Xvfb
        env KEYFORGE_CONFIG_DIR="$CFG" KEYFORGE_PREVIEW_UI="$preview" KEYFORGE_PREVIEW_THEME="$theme" "$BIN" --editor &
        KPID=$!
        sleep "$WAIT"
        # Captura com import (ImageMagick) se disponível
        if command -v import >/dev/null 2>&1; then
            import -window root "$OUT/${name}-${theme}.png" 2>/dev/null || echo "Aviso: import falhou para $name-$theme"
        elif command -v scrot >/dev/null 2>&1; then
            scrot "$OUT/${name}-${theme}.png" 2>/dev/null || echo "Aviso: scrot falhou para $name-$theme"
        else
            echo "Aviso: nenhuma ferramenta de captura disponível (install imagemagick/scrot)"
        fi
        kill "$KPID" 2>/dev/null || true
        wait "$KPID" 2>/dev/null || true
    fi
}

# Todas as telas do PLANO-UX.md, em ordem:
#  - palco: aparelho com teclas (1 tecla vazia, 1 configurada)
#  - biblioteca: aba da biblioteca de ações
#  - aparencia: aba Aparência do inspetor
#  - preferencias: janela Preferências (modal)
#  - apps: lista de apps (modal)
#  - topo: barra do topo (visível em todas, capturada junto com palco)
#  - vazio: página vazia com pontos de partida
#  - primeira_execucao: tela de boas-vindas

for theme in dark light; do
    shot palco "key=1" "$theme"
    shot biblioteca "library" "$theme"
    shot aparencia "key=1,tab=1,settings" "$theme"
    shot preferencias "settings" "$theme"
    shot apps "key=1,picker" "$theme"
    shot vazia "key=15" "$theme"
    shot primeira_execucao "first_run" "$theme"
done

pkill -f "[t]arget/debug/keyforge --editor" 2>/dev/null || true

echo "Capturas em $OUT"
ls -la "$OUT"