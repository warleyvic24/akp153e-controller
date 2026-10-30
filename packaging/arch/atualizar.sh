#!/bin/sh
# Compila, instala e reinicia o AKP153E pedindo a senha do sudo uma vez só, logo no começo.
# Uso: packaging/arch/atualizar.sh
# O makepkg leva minutos e o sudo esquece a senha em 5; sem isto a senha era pedida no fim,
# quando ninguém mais estava olhando, e expirava.
set -eu
cd "$(dirname "$0")"

echo "Senha do sudo (só agora; o resto roda sozinho):"
sudo -v

# Renova o sudo a cada minuto enquanto o script roda.
(while sudo -n -v 2>/dev/null; do sleep 60; done) &
keep_alive=$!
trap 'kill "$keep_alive" 2>/dev/null || true' EXIT INT TERM

makepkg -sf --noconfirm

# O pacote principal (o "-debug" fica de fora).
pkg=$(ls -t akp153e-[0-9]*.pkg.tar.* | head -n 1)
sudo -n pacman -U --noconfirm "$pkg"

# Reinicia serviço e editor com o binário novo (o editor salva sozinho, nada se perde).
pkill -x akp153e || true
sleep 1
setsid -f akp153e >/dev/null 2>&1

echo "Pronto: AKP153E atualizado e aberto."
