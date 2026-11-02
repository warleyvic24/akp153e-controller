# =============================================================================
# Script de build para Windows (nativo no Windows)
# Execute no PowerShell: .\tools\build-windows.ps1
# =============================================================================

$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot\..

$TARGET = "x86_64-pc-windows-msvc"
$APP_NAME = "keyforge"
$VERSION = "0.1.0"

Write-Host "🔧 Verificando toolchain..." -ForegroundColor Cyan
$targets = rustup target list --installed
if ($targets -notcontains $TARGET) {
    Write-Host "❌ Target $TARGET não instalado. Rode: rustup target add $TARGET" -ForegroundColor Red
    exit 1
}

Write-Host "🔨 Compilando para Windows ($TARGET)..." -ForegroundColor Cyan
cargo build --release --target $TARGET

Write-Host "📦 Criando instalador MSI..." -ForegroundColor Cyan

if (Get-Command cargo-winstaller -ErrorAction SilentlyContinue) {
    cargo wininstaller --config windows/wininstaller.toml
} else {
    Write-Host "⚠️  cargo-winstaller não instalado. Instale com: cargo install cargo-winstaller" -ForegroundColor Yellow
    Write-Host "📦 O .exe está em: target\$TARGET\release\$APP_NAME.exe" -ForegroundColor Green
}

Write-Host "✅ Build concluído!" -ForegroundColor Green
Write-Host ""
Write-Host "📁 Arquivos gerados:" -ForegroundColor Green
Write-Host "   - target\$TARGET\release\$APP_NAME.exe"
Write-Host "   - keyforge-setup-$VERSION.msi (se cargo-winstaller estiver instalado)"