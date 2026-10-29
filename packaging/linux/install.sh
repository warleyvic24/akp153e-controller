#!/bin/sh
# Instala o AKP153E no Linux: binário em ~/.local/bin, atalho no menu e regra udev (pede sudo).
# Uso: packaging/linux/install.sh [caminho/do/akp153e]   (padrão: target/release/akp153e)
set -eu

here=$(cd "$(dirname "$0")" && pwd)
binary=${1:-"$here/../../target/release/akp153e"}

if [ ! -x "$binary" ]; then
    echo "Binário não encontrado: $binary" >&2
    echo "Compile com 'cargo build --release' ou passe o caminho como argumento." >&2
    exit 1
fi

bin_dir="$HOME/.local/bin"
apps_dir="$HOME/.local/share/applications"
mkdir -p "$bin_dir" "$apps_dir"
install -m 755 "$binary" "$bin_dir/akp153e"
install -m 644 "$here/akp153e.desktop" "$apps_dir/akp153e.desktop"

echo "Instalando a regra udev (precisa de sudo)..."
sudo install -m 644 "$here/99-akp153e.rules" /etc/udev/rules.d/99-akp153e.rules
sudo udevadm control --reload
sudo udevadm trigger

echo "Pronto. Desconecte e reconecte o aparelho, depois rode: akp153e --background"
case ":$PATH:" in
    *":$bin_dir:"*) ;;
    *) echo "Aviso: $bin_dir não está no PATH." ;;
esac
