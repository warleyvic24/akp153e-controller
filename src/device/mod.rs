//! Aparelhos descritos por dados (`devices/<fabricante>/<modelo>.json`, embutidos no programa): ids USB,
//! protocolo, imagem e os controles (teclas, faixa lateral...) com o índice de cada um no protocolo.
//! O código de USB só traduz; o que muda de um modelo para outro mora no arquivo.
//! Plano completo: `docs/PLANO-APARELHOS.md`.
mod model;
pub use model::*;

use std::sync::OnceLock;

/// Os arquivos embutidos. Um modelo novo entra aqui e na pasta `devices/`.
const BUILT_IN: [&str; 2] = [
    include_str!("../../devices/ajazz/akp153e.json"),
    include_str!("../../devices/ajazz/mirabox-15-v1.json"),
];

/// Todos os modelos conhecidos: primeiro os da pasta do usuário (`<config>/devices/*.json`, que
/// ganham de um embutido com o mesmo VID/PID), depois os embutidos. Inválidos ficam de fora (no log).
pub fn catalog() -> &'static [DeviceModel] {
    static CATALOG: OnceLock<Vec<DeviceModel>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let user = user_files();
        user.iter()
            .map(|(name, text)| (name.as_str(), text.as_str()))
            .chain(BUILT_IN.iter().map(|text| ("embutido", *text)))
            .filter_map(|(origin, text)| match DeviceModel::parse(text) {
                Ok(m) => Some(m),
                Err(e) => {
                    log::error!("descrição de aparelho inválida ({origin}): {e}");
                    None
                }
            })
            .collect()
    })
}

/// Arquivos `.json` da pasta `devices` ao lado da config (nunca nos testes).
fn user_files() -> Vec<(String, String)> {
    if cfg!(test) {
        return Vec::new();
    }
    let Some(dir) = crate::config::schema::AppConfig::path()
        .and_then(|p| p.parent().map(|d| d.join("devices")))
    else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut files: Vec<(String, String)> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .filter_map(|p| Some((p.display().to_string(), std::fs::read_to_string(&p).ok()?)))
        .collect();
    files.sort();
    files
}

/// O modelo do aparelho com esse VID/PID.
pub fn find(vid: u16, pid: u16) -> Option<&'static DeviceModel> {
    catalog().iter().find(|m| m.matches(vid, pid))
}

/// O modelo do aparelho pelo id (ex.: \"ajazz-akp153e\").
pub fn find_by_id(id: &str) -> Option<&'static DeviceModel> {
    catalog().iter().find(|m| m.id == id)
}

/// Todos os pares VID/PID conhecidos.
pub fn known_ids() -> Vec<(u16, u16)> {
    catalog()
        .iter()
        .flat_map(|m| m.usb.iter().map(|u| (u.vid, u.pid)))
        .collect()
}

#[cfg(test)]
mod tests;
