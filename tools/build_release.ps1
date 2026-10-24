# Compila o release sem caminhos pessoais e gera dist\<nome>.zip com o checksum SHA-256.
# Uso (na raiz do projeto):  powershell -File tools\build_release.ps1 -Version 0.1.0
#
# O Rust grava no .exe os caminhos dos arquivos de código (para mensagens de erro), o que revelaria
# o nome de usuário de quem compilou. `--remap-path-prefix` troca esses caminhos por nomes neutros.
# A compilação usa a pasta `target-release`, então funciona mesmo com o serviço rodando.
param([string]$Version = "0.1.0")

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE ".cargo" }

# Em caso de coincidência vence a regra mais à direita; por isso a mais geral (perfil do usuário) vai primeiro.
# CARGO_ENCODED_RUSTFLAGS separa por 0x1F; RUSTFLAGS quebraria caminhos com espaço.
$env:RUSTFLAGS = $null
$env:CARGO_ENCODED_RUSTFLAGS = @(
    "--remap-path-prefix=$env:USERPROFILE=~",
    "--remap-path-prefix=$cargoHome=cargo",
    "--remap-path-prefix=$root=src"
) -join [char]0x1F
$env:CARGO_TARGET_DIR = Join-Path $root "target-release"

Push-Location $root
try {
    cargo build --release --locked
    if ($LASTEXITCODE -ne 0) { throw "cargo build falhou" }

    $name = "akp153e-open-controller-v$Version-windows-x64"
    $dist = Join-Path $root "dist"
    $folder = Join-Path $dist $name
    Remove-Item -Recurse -Force $folder -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force $folder | Out-Null

    $exe = Join-Path $env:CARGO_TARGET_DIR "release\akp153e.exe"
    # Confere que o nome de usuário não ficou gravado no executável.
    $bytes = [System.IO.File]::ReadAllBytes($exe)
    $text = [System.Text.Encoding]::ASCII.GetString($bytes)
    if ($text.Contains($env:USERPROFILE)) { throw "o executável ainda contém $env:USERPROFILE" }

    Copy-Item $exe $folder
    foreach ($file in "LICENSE", "README.md", "README.pt-BR.md", "THIRD_PARTY_NOTICES.md") {
        Copy-Item (Join-Path $root $file) $folder
    }

    $zip = Join-Path $dist "$name.zip"
    Remove-Item -Force $zip -ErrorAction SilentlyContinue
    Compress-Archive -Path (Join-Path $folder "*") -DestinationPath $zip
    $hash = (Get-FileHash -Algorithm SHA256 $zip).Hash.ToLower()
    "$hash  $name.zip" | Set-Content -Encoding ascii (Join-Path $dist "$name.zip.sha256")

    Write-Host "Pronto: $zip"
    Write-Host "SHA-256: $hash"
}
finally {
    Pop-Location
}
