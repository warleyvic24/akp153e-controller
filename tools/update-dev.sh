#!/bin/sh
# Atualização rápida de desenvolvimento: builda e instala em ~/.local/bin (sem sudo)
# Uso: ./tools/update-dev.sh
set -eu

cd "$(dirname "$0")/.."

echo "🔨 Compilando..."
cargo build --release

echo "📦 Instalando em ~/.local/bin..."
mkdir -p ~/.local/bin
cp target/release/keyforge ~/.local/bin/keyforge

echo "🔄 Reiniciando..."
pkill -x keyforge || true
sleep 1
setsid -f ~/.local/bin/keyforge >/dev/null 2>&1

echo "✅ Pronto! Editor aberto com as últimas mudanças."