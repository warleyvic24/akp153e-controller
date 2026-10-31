#!/bin/sh
# Compila, instala e reinicia o Keyforge pedindo a senha do sudo uma vez só, logo no começo.
# Uso: packaging/arch/atualizar.sh
# O makepkg leva minutos e o sudo esquece a senha em 5; sem isto a senha era pedida no fim,
# quando ninguém mais estava olhando, e expirava.
set -eu
cd "$(dirname "$0")"

echo "Senha do sudo (só agora; o resto roda sozinho):"
sudo -v

# Renova o sudo enquanto este script existir. Pedaços curtos: se o script cair de qualquer jeito, o
# laço percebe e sai sozinho, e o `sleep` que estiver rodando no fim acaba em até 10 s.
main=$$
(while kill -0 "$main" 2>/dev/null && sudo -n -v 2>/dev/null; do sleep 10; done) &
keep_alive=$!
trap 'kill "$keep_alive" 2>/dev/null || true' EXIT INT TERM

makepkg -sf --noconfirm

# O pacote principal (o "-debug" fica de fora).
pkg=$(ls -t keyforge-[0-9]*.pkg.tar.* | head -n 1)
# Pacote do nome antigo: o `pacman -U --noconfirm` recusa o conflito (o `replaces` só vale para
# repositórios), então ele sai antes. A config fica (está na pasta do usuário).
if pacman -Q akp153e >/dev/null 2>&1; then
  sudo -n pacman -R --noconfirm akp153e
fi
sudo -n pacman -U --noconfirm "$pkg"

# Reinicia serviço e editor com o binário novo (o editor salva sozinho, nada se perde).
# O nome antigo também: um serviço `akp153e` velho seguraria o USB e a porta de controle.
pkill -x akp153e || true
pkill -x keyforge || true
sleep 1
setsid -f keyforge >/dev/null 2>&1

echo "Pronto: Keyforge atualizado e aberto."
