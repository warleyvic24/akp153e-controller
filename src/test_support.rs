//! Apoio dos testes que leem o próprio código-fonte.
use std::path::{Path, PathBuf};

/// Todo arquivo `.rs` de `src/`, em qualquer subpasta, como `(caminho relativo, conteúdo sem os
/// testes)`. O caminho usa `/` para as mensagens serem iguais em qualquer sistema.
pub(crate) fn sources_without_tests() -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect_rs(&root, &mut files);
    files.sort();
    files
        .into_iter()
        .map(|path| {
            let name = path
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let src = std::fs::read_to_string(&path).unwrap();
            let end = src.find("#[cfg(test)]").unwrap_or(src.len());
            (name, src[..end].to_string())
        })
        .collect()
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}
