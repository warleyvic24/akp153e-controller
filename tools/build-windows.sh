#!/bin/bash
# =============================================================================
# Script de build para Windows a partir do Linux (cross-compilation)
# Requer: rustup target add x86_64-pc-windows-msvc
#         sudo apt install clang llvm lld (ou mingw-w64)
# =============================================================================
set -euo pipefail

cd "$(dirname "$0")/.."

TARGET="x86_64-pc-windows-msvc"
APP_NAME="keyforge"
VERSION="0.1.0"

echo "🔧 Verificando toolchain Windows..."
rustup target list --installed 2>/dev/null | grep -q "$TARGET" || {
    echo "❌ Target $TARGET não instalado. Rode: rustup target add $TARGET"
    exit 1
}

echo "🔧 Verificando linker..."
which clang >/dev/null 2>&1 || which x86_64-w64-mingw32-gcc >/dev/null 2>&1 || {
    echo "❌ Nenhum linker Windows encontrado. Instale clang/llvm ou mingw-w64"
    exit 1
}

echo "🔨 Compilando para Windows ($TARGET)..."
cargo build --release --target "$TARGET"

echo "📦 Criando instalador MSI..."

# Se cargo-winstaller estiver disponível, usar ele
if command -v cargo-winstaller >/dev/null 2>&1; then
    cargo wininstaller --config windows/wininstaller.toml
else
    echo "⚠️  cargo-winstaller não instalado. Instale com: cargo install cargo-winstaller"
    echo "📦 O .exe está em: target/$TARGET/release/$APP_NAME.exe"
fi

echo "✅ Build concluído!"
echo ""
echo "📁 Arquivos gerados:"
echo "   - target/$TARGET/release/$APP_NAME.exe"
echo "   - keyforge-setup-$VERSION.msi (se cargo-winstaller estiver instalado)"