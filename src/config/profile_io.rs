//! Exportar e importar um perfil (cópia de segurança, levar para outro PC, compartilhar). O arquivo
//! `.akp153e.json` leva o perfil e as imagens das teclas dentro dele (em hexadecimal), porque os
//! caminhos das imagens só valem no PC de origem. Ao importar, as imagens vão para a pasta de ícones.
use super::schema::{AppConfig, ButtonAction, ButtonAppearance, Profile};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const FORMAT: &str = "akp153e-profile";
const VERSION: u32 = 1;
/// Extensão sugerida no diálogo de salvar.
pub const EXTENSION: &str = "akp153e.json";
/// Maior imagem levada no arquivo; acima disso a tecla vai sem ela (um ícone não precisa disso).
const MAX_IMAGE_BYTES: u64 = 2 * 1024 * 1024;
/// Maior arquivo aceito na importação.
pub const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
struct Bundle {
    format: String,
    version: u32,
    profile: Profile,
    /// Caminho original da imagem -> bytes em hexadecimal.
    #[serde(default)]
    images: BTreeMap<String, String>,
}

fn appearances(p: &mut Profile) -> impl Iterator<Item = &mut ButtonAppearance> {
    p.pages
        .iter_mut()
        .flat_map(|pg| pg.buttons.iter_mut())
        .flat_map(|b| std::iter::once(&mut b.appearance).chain(b.toggled_appearance.as_mut()))
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn from_hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

/// Texto do arquivo de exportação do perfil `p`, com as imagens que puderem ser lidas.
pub fn export(p: &Profile) -> String {
    let mut profile = p.clone();
    let mut images = BTreeMap::new();
    for ap in appearances(&mut profile) {
        let Some(path) = ap.icon_path.clone() else {
            continue;
        };
        let small = std::fs::metadata(&path).is_ok_and(|m| m.len() <= MAX_IMAGE_BYTES);
        match std::fs::read(&path).ok().filter(|_| small) {
            Some(bytes) => {
                images.entry(path).or_insert_with(|| to_hex(&bytes));
            }
            None => ap.icon_path = None,
        }
    }
    let bundle = Bundle {
        format: FORMAT.into(),
        version: VERSION,
        profile,
        images,
    };
    serde_json::to_string_pretty(&bundle).unwrap_or_default()
}

/// Lê um arquivo exportado e o acrescenta a `cfg` como perfil novo (com id novo e nome sem
/// repetir). As imagens vão para `icon_dir`. Devolve o índice do perfil novo.
pub fn import(
    cfg: &mut AppConfig,
    text: &str,
    icon_dir: &std::path::Path,
) -> Result<usize, String> {
    let bundle: Bundle = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if bundle.format != FORMAT || bundle.version > VERSION {
        return Err("não é um perfil do AKP153E (ou é de uma versão mais nova)".into());
    }
    let mut profile = bundle.profile;
    let mut saved: BTreeMap<String, Option<String>> = BTreeMap::new();
    for ap in appearances(&mut profile) {
        let Some(original) = ap.icon_path.take() else {
            continue;
        };
        let local = saved.entry(original.clone()).or_insert_with(|| {
            let bytes = from_hex(bundle.images.get(&original)?)?;
            let ext = std::path::Path::new(&original)
                .extension()
                .and_then(|e| e.to_str())
                .filter(|e| e.chars().all(|c| c.is_ascii_alphanumeric()))
                .unwrap_or("png")
                .to_ascii_lowercase();
            let name = crate::platform::icon_file_name(&original);
            let file = icon_dir.join(name).with_extension(ext);
            std::fs::create_dir_all(icon_dir).ok()?;
            std::fs::write(&file, bytes).ok()?;
            Some(file.display().to_string())
        });
        ap.icon_path = local.clone();
    }
    // Ações para outros perfis apontam para ids do PC de origem: desligadas para não ir ao perfil errado.
    for b in profile
        .pages
        .iter_mut()
        .flat_map(|pg| pg.buttons.iter_mut())
    {
        for a in std::iter::once(&mut b.action)
            .chain(b.long_press_action.as_mut())
            .chain(b.toggled_action.as_mut())
        {
            if matches!(a, ButtonAction::SwitchProfile { .. }) {
                *a = ButtonAction::None;
            }
        }
    }
    profile.id = cfg.profiles.iter().map(|p| p.id).max().unwrap_or(0) + 1;
    let base = profile.name.clone();
    let mut n = 2;
    while cfg.profiles.iter().any(|p| p.name == profile.name) {
        profile.name = format!("{base} ({n})");
        n += 1;
    }
    cfg.profiles.push(profile);
    let sane = std::mem::take(cfg).sanitized();
    *cfg = sane;
    Ok(cfg.profiles.len() - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("akp_profile_io_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn hex_round_trips_and_rejects_garbage() {
        let bytes = [0u8, 1, 127, 128, 255];
        assert_eq!(from_hex(&to_hex(&bytes)).unwrap(), bytes);
        assert!(from_hex("abc").is_none());
        assert!(from_hex("zz").is_none());
    }

    #[test]
    fn a_profile_travels_with_its_images_and_gets_a_new_id_and_name() {
        let dir = temp_dir("roundtrip");
        let image = dir.join("logo.png");
        std::fs::write(&image, b"\x89PNG fake").unwrap();
        let mut source = AppConfig::default();
        source.profiles[0].name = "Live".into();
        source.profiles[0].auto_app = Some("obs".into());
        let key = &mut source.profiles[0].pages[0].buttons[0];
        key.appearance.icon_path = Some(image.display().to_string());
        key.action = ButtonAction::SwitchProfile { profile: 1 };
        let text = export(&source.profiles[0]);
        std::fs::remove_file(&image).unwrap();

        let mut target = AppConfig::default();
        target.profiles[0].name = "Live".into();
        let icons = dir.join("icons");
        let index = import(&mut target, &text, &icons).unwrap();
        let imported = &target.profiles[index];
        assert_eq!(imported.name, "Live (2)");
        assert_eq!(imported.auto_app.as_deref(), Some("obs"));
        assert_ne!(imported.id, target.profiles[0].id);
        let key = &imported.pages[0].buttons[0];
        assert_eq!(key.action, ButtonAction::None, "perfil de outro PC");
        let local = key.appearance.icon_path.as_deref().unwrap();
        assert!(local.starts_with(&icons.display().to_string()));
        assert_eq!(std::fs::read(local).unwrap(), b"\x89PNG fake");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn other_files_are_refused() {
        let mut cfg = AppConfig::default();
        let dir = temp_dir("refuse");
        assert!(import(&mut cfg, "{}", &dir).is_err());
        assert!(import(&mut cfg, "lixo", &dir).is_err());
        let wrong = r#"{"format":"outro","version":1,"profile":{"name":"x","pages":[]}}"#;
        assert!(import(&mut cfg, wrong, &dir).is_err());
        assert_eq!(cfg.profiles.len(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }
}
