# Build do Keyforge para Windows

O Keyforge compila e roda no Windows, mas **não há máquina Windows disponível para testes** com o hardware AKP153E físico. O código está pronto (`cfg(windows)` em todos os pontos específicos), mas você precisará validar em uma máquina Windows real.

## Pré-requisitos

### No Windows (recomendado)
1. Instale [Rust](https://rustup.rs/)
2. Instale o driver USB do AKP153E (WinUSB ou libusbK — o dispositivo é reconhecido como "Ajazz HOTSPOTEKUSB HID DEMO", VID:PID 0300:3010)
3. Instale o Visual C++ Build Tools (para o linker MSVC)

### Cross-compile a partir do Linux
1. `rustup target add x86_64-pc-windows-msvc`
2. `sudo apt install clang llvm lld` (ou `mingw-w64`)

## Como gerar o .exe

### No Windows:
```powershell
git clone https://github.com/warleyvic24/akp153e-controller.git
cd akp153e-controller
git checkout linux-port
cargo build --release
# O .exe estará em: target/release/keyforge.exe
```

### Do Linux (cross-compile):
```bash
rustup target add x86_64-pc-windows-msvc
sudo apt install clang llvm lld
cargo build --release --target x86_64-pc-windows-msvc
# O .exe estará em: target/x86_64-pc-windows-msvc/release/keyforge.exe
```

## Como gerar o instalador .msi

Instale o `cargo-winstaller`:
```bash
cargo install cargo-winstaller
```

### No Windows:
```powershell
cargo build --release
cargo wininstaller --config windows/wininstaller.toml
# Gera: keyforge-setup-0.1.0.msi
```

### Do Linux (cross-compile):
```bash
cargo build --release --target x86_64-pc-windows-msvc
cargo wininstaller --config windows/wininstaller.toml
```

## Scripts de build

- `tools/build-windows.sh` — cross-compile a partir do Linux
- `tools/build-windows.ps1` — build nativo no Windows (PowerShell)

## O que está incluído no build Windows

| Recurso | Status |
|---------|--------|
| Ícone no .exe | ✅ Embutido via `build.rs` + `winresource` |
| Ícone na bandeja (tray) | ✅ `tray-icon` 0.26 |
| Mixer de áudio por app | ✅ Core Audio API |
| Apps instalados (Menu Iniciar) | ✅ PowerShell + atalhos `.lnk` |
| Troca automática de perfil | ✅ Polling `GetForegroundWindow` |
| Autostart | ✅ Registro do Windows (`HKCU\...\Run`) |
| USB/HID (AKP153E) | ✅ Código pronto, **não testado** |

## ⚠️ Limitações conhecidas no Windows

1. **Driver USB**: O AKP153E não é reconhecido automaticamente no Windows. Você precisará instalar o driver WinUSB ou libusbK manualmente.
2. **enigo**: A simulação de teclado no Windows usa `enigo` 0.6 — pode haver diferenças de comportamento vs Linux.
3. **Nunca testado**: Não há máquina Windows disponível para validar com o hardware físico. Se encontrar bugs, reporte com logs para correção.

## Comandos rápidos

```bash
# Cross-compile do Linux
rustup target add x86_64-pc-windows-msvc
sudo apt install clang llvm lld
cargo build --release --target x86_64-pc-windows-msvc
cargo install cargo-winstaller
cargo wininstaller --config windows/wininstaller.toml
```

## Arquivos relacionados

- `build.rs` — embute o ícone no .exe Windows
- `windows/keyforge.rc` — recursos do Windows (versão, ícone)
- `windows/wininstaller.toml` — configuração do instalador MSI
- `tools/build-windows.sh` — script de build cross-platform
- `tools/build-windows.ps1` — script de build nativo Windows