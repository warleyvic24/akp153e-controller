#!/usr/bin/env python3
"""
Compara capturas atuais com a galeria de referência (docs/ui-baseline).
Uso: tools/ui-diff.py <pasta atual> <pasta baseline>
Sai com código diferente de zero se houver diferença visível.
"""
import sys
import os
import hashlib
from pathlib import Path

def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(8192), b""):
            h.update(chunk)
    return h.hexdigest()

def main():
    if len(sys.argv) != 3:
        print(f"Uso: {sys.argv[0]} <pasta atual> <pasta baseline>", file=sys.stderr)
        return 2

    current = Path(sys.argv[1])
    baseline = Path(sys.argv[2])

    if not current.is_dir():
        print(f"ERRO: pasta atual não existe: {current}", file=sys.stderr)
        return 2
    if not baseline.is_dir():
        print(f"ERRO: pasta baseline não existe: {baseline}", file=sys.stderr)
        return 2

    current_files = sorted(current.glob("*.png"))
    baseline_files = sorted(baseline.glob("*.png"))

    if not current_files:
        print("ERRO: nenhuma captura na pasta atual", file=sys.stderr)
        return 2

    current_names = {f.name for f in current_files}
    baseline_names = {f.name for f in baseline_files}

    # Verifica arquivos faltando na pasta atual
    missing_in_current = baseline_names - current_names
    if missing_in_current:
        print(f"FALTANDO na pasta atual: {', '.join(sorted(missing_in_current))}", file=sys.stderr)
        return 1

    # Verifica arquivos extras na pasta atual
    extra_in_current = current_names - baseline_names
    if extra_in_current:
        print(f"EXTRAS na pasta atual: {', '.join(sorted(extra_in_current))}", file=sys.stderr)

    # Compara hash de cada arquivo comum
    diffs = []
    for name in sorted(current_names & baseline_names):
        current_hash = sha256_file(current / name)
        baseline_hash = sha256_file(baseline / name)
        if current_hash != baseline_hash:
            diffs.append(name)

    if diffs:
        print(f"DIFERENÇAS detectadas ({len(diffs)} arquivo(s)):", file=sys.stderr)
        for name in diffs:
            print(f"  - {name}", file=sys.stderr)
        print("\nPara atualizar a baseline (após revisar as diferenças):", file=sys.stderr)
        print(f"  cp {current}/*.png {baseline}/", file=sys.stderr)
        return 1

    print(f"Todas as {len(current_files)} capturas coincidem com a baseline.")
    return 0

if __name__ == "__main__":
    sys.exit(main())
