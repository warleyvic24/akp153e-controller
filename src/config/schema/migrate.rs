//! Migrações: pasta antiga (AKP153E -> Keyforge) e versões antigas do formato da config.
use super::*;
use std::path::Path;

/// Grava um arquivo novo que só o dono lê (a config guarda URLs de webhook com tokens).
fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, CONFIG_FILE_MODE);
    options.open(path)?.write_all(text.as_bytes())
}

/// Cópia guardada antes de migrar uma config da versão `v`.
pub(super) fn version_backup(config: &Path, v: u32) -> PathBuf {
    config.with_file_name(format!("config.v{v}.bak.json"))
}

impl AppConfig {
    /// Antes de uma versão antiga ser migrada (e depois gravada no formato novo), guarda o arquivo
    /// original como `config.v{N}.bak.json`. Só a primeira vez: nunca escreve por cima do backup.
    pub(super) fn backed_up(self, path: &Path, text: &str) -> Self {
        if self.schema_version < SCHEMA_VERSION {
            let backup = version_backup(path, self.schema_version);
            if !backup.exists() {
                match write_private(&backup, text) {
                    Ok(()) => log::info!(
                        "config v{} guardada em {}",
                        self.schema_version,
                        backup.display()
                    ),
                    Err(e) => log::warn!("backup da config antiga falhou: {e}"),
                }
            }
        }
        self
    }

    /// Pasta de antes da troca de nome (AKP153E -> Keyforge). Nunca é apagada: fica de cópia de segurança.
    fn legacy_dir() -> Option<PathBuf> {
        directories::BaseDirs::new().map(|d| d.config_dir().join("AJazz").join("AKP153E"))
    }

    /// Na primeira abertura com o nome novo, copia a pasta antiga (config, ícones) para a nova.
    /// Só copia se a nova ainda não existe: nunca escreve por cima de uma config do Keyforge.
    /// Roda antes do log existir (o arquivo de log fica na pasta nova): devolve o que registrar.
    pub fn migrate_legacy_dir() -> Option<String> {
        if cfg!(test) || (cfg!(debug_assertions) && dev_var("CONFIG_DIR").is_some()) {
            return None;
        }
        let old = Self::legacy_dir()?;
        let new = Self::path()?.parent()?.to_path_buf();
        migrate_dir(&old, &new)
    }

    /// v3 -> v4: o "abrir site" antigo (o abridor do sistema com uma URL) vira `OpenUrl`.
    pub(super) fn migrate_open_url(&mut self) {
        for b in self
            .profiles
            .iter_mut()
            .flat_map(|p| p.pages.iter_mut())
            .flat_map(|pg| pg.buttons.iter_mut())
        {
            for a in std::iter::once(&mut b.action)
                .chain(b.long_press_action.as_mut())
                .chain(b.toggled_action.as_mut())
            {
                to_open_url(a);
            }
        }
    }

    /// v1 -> v2: ações que apontavam para a posição passam a apontar para o id.
    pub(super) fn migrate_index_targets(&mut self) {
        let profile_ids: Vec<u64> = self.profiles.iter().map(|p| p.id).collect();
        for p in &mut self.profiles {
            let page_ids: Vec<u64> = p.pages.iter().map(|pg| pg.id).collect();
            for b in p.pages.iter_mut().flat_map(|pg| pg.buttons.iter_mut()) {
                for a in std::iter::once(&mut b.action)
                    .chain(b.long_press_action.as_mut())
                    .chain(b.toggled_action.as_mut())
                {
                    retarget_by_index(a, &page_ids, &profile_ids);
                }
            }
        }
    }
}

/// Programas que o "abrir site" antigo chamava (Linux e Windows).
const LEGACY_URL_OPENERS: [&str; 2] = ["xdg-open", "explorer"];

fn to_open_url(a: &mut ButtonAction) {
    match a {
        ButtonAction::LaunchApp { path, args }
            if LEGACY_URL_OPENERS.contains(&path.as_str())
                && args.len() == 1
                && args[0].contains("://") =>
        {
            *a = ButtonAction::OpenUrl {
                url: args[0].clone(),
            };
        }
        ButtonAction::MultiAction { steps } => steps.iter_mut().for_each(to_open_url),
        _ => {}
    }
}

pub(super) fn migrate_control_ids(config: &mut AppConfig) {
    use crate::device::find_by_id;

    for profile in &mut config.profiles {
        for page in &mut profile.pages {
            // Carrega o modelo do aparelho para esta página
            let Some(model) = find_by_id(&page.model) else {
                log::warn!(
                    "modelo '{}' não encontrado para migrar control_ids",
                    page.model
                );
                continue;
            };

            // Mapeia índice do botão -> control_id do modelo
            // Teclas: k0, k1, k2... (ordem de `model.keys()`)
            // Lateral: side
            let keys = model.keys();
            let strip = model.strip();

            for (idx, button) in page.buttons.iter_mut().enumerate() {
                if button.control_id.is_some() {
                    continue; // já tem control_id (config v5+)
                }
                if idx < keys.len() {
                    button.control_id = Some(keys[idx].id.clone());
                } else if let Some(strip) = strip.filter(|_| idx == keys.len()) {
                    button.control_id = Some(strip.id.clone());
                } else {
                    // Índice além do que o modelo suporta: deixa vazio (sanitized vai limpar)
                    button.control_id = None;
                }
            }
        }
    }
}

/// Copia `old` para `new` se `new` não existe e `old` tem uma config; devolve o que registrar.
pub(super) fn migrate_dir(old: &std::path::Path, new: &std::path::Path) -> Option<String> {
    if new.exists() || !old.join("config.json").is_file() {
        return None;
    }
    Some(match copy_dir(old, new) {
        Ok(()) => format!("config copiada de {} para {}", old.display(), new.display()),
        Err(e) => {
            // Pasta pela metade: melhor nenhuma (a próxima abertura tenta de novo).
            let _ = std::fs::remove_dir_all(new);
            format!("não foi possível copiar a config antiga: {e}")
        }
    })
}

fn copy_dir(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// v1 -> v2 de uma ação: posição de página/perfil vira id (0 = destino que não existe mais).
/// Entra nos passos de uma multi-ação, que também guardavam a posição.
fn retarget_by_index(a: &mut ButtonAction, page_ids: &[u64], profile_ids: &[u64]) {
    match a {
        ButtonAction::SwitchPage { page } => {
            *page = page_ids.get(*page as usize).copied().unwrap_or(0)
        }
        ButtonAction::SwitchProfile { profile } => {
            *profile = profile_ids.get(*profile as usize).copied().unwrap_or(0)
        }
        ButtonAction::MultiAction { steps } => {
            for step in steps {
                retarget_by_index(step, page_ids, profile_ids);
            }
        }
        _ => {}
    }
}
