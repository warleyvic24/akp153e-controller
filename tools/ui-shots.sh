#!/bin/sh
# Capturas do editor (debug) numa cópia da config, num monitor virtual do Hyprland, sem tocar na
# config real. Uso: tools/ui-shots.sh <pasta de saída> <pasta com cópia do config.json>
# Gera as telas principais nos dois temas; rode antes e depois de mudar a interface e compare.
set -eu
OUT=${1:?pasta de saída}
CFG=${2:?pasta com a cópia do config.json}
BIN=$(dirname "$0")/../target/debug/keyforge
mkdir -p "$OUT"
hyprctl monitors -j | grep -q '"HEADLESS' || hyprctl output create headless >/dev/null
MON=$(hyprctl monitors -j | python3 -c "import json,sys;print([m['name'] for m in json.load(sys.stdin) if m['name'].startswith('HEADLESS')][0])")
WS=$(hyprctl monitors -j | python3 -c "import json,sys;print([m['activeWorkspace']['id'] for m in json.load(sys.stdin) if m['name'].startswith('HEADLESS')][0])")
shot() { # nome, tela, tema
  pkill -f "[t]arget/debug/keyforge --editor" || true
  hyprctl dispatch "hl.dsp.dpms({ action = \"enable\", monitor = \"$MON\" })" >/dev/null
  hyprctl dispatch "hl.dsp.exec_cmd(\"env KEYFORGE_CONFIG_DIR=$CFG KEYFORGE_PREVIEW_UI=$2 KEYFORGE_PREVIEW_THEME=$3 $BIN --editor\", { workspace = \"$WS silent\" })" >/dev/null
  sleep "${WAIT:-5}"
  grim -o "$MON" "$OUT/$1-$3.png"
}
for theme in dark light; do
  shot palco key=1 "$theme"
  shot aparencia key=1,tab=1 "$theme"
  shot vazia key=15 "$theme"
  shot preferencias settings "$theme"
  shot apps key=1,picker "$theme"
  shot lateral side "$theme"
done
pkill -f "[t]arget/debug/keyforge --editor" || true
echo "capturas em $OUT"
