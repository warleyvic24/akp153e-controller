//! Editores das ações de som: arquivo avulso (`PlaySound`), banco de sons (`SoundBank`), som de
//! um banco (`PlaySoundBank`) e o botão de pânico (`StopAllSounds`).
//!
//! Os bancos vêm da própria config (`AppConfig::sound_banks`, via `Targets`), então a lista do
//! editor é sempre a mesma que o serviço usa ao tocar.
use super::action_ui::PaneCtx;
use super::widgets::{self, ButtonKind};
use crate::strings::{t, tf};
use eframe::egui::{self, RichText};

/// Formatos que o tocador do sistema lê (no Windows, só WAV).
#[cfg(windows)]
const SOUND_EXTENSIONS: [&str; 1] = ["wav"];
#[cfg(not(windows))]
const SOUND_EXTENSIONS: [&str; 5] = ["wav", "ogg", "oga", "flac", "mp3"];
#[cfg(windows)]
const SOUND_HINT: &str = "Arquivo WAV. Tocado sem esperar: a tecla responde na hora.";
#[cfg(not(windows))]
const SOUND_HINT: &str = "WAV, OGG, FLAC ou MP3, pelo pw-play (ou paplay). Tocado sem esperar.";
const NO_BANKS_HINT: &str =
    "Nenhum banco de sons ainda: configure uma tecla \"Banco de sons\" e escolha-o aqui.";
/// Largura do campo de caminho de cada som do banco.
const SOUND_FIELD_WIDTH: f32 = 260.0;

/// Nome do arquivo de `path`, para mostrar no lugar do caminho inteiro.
fn file_name(path: &str) -> Option<String> {
    std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
}

/// Abre o seletor de arquivos filtrado pelos formatos que o tocador lê.
fn pick_sound_file() -> Option<String> {
    rfd::FileDialog::new()
        .add_filter(t("Som"), &SOUND_EXTENSIONS)
        .pick_file()
        .map(|f| f.display().to_string())
}

/// `PlaySound`: um arquivo, escolhido pelo seletor.
pub(super) fn sound_file_editor(ui: &mut egui::Ui, cx: &PaneCtx, path: &mut String) -> bool {
    let mut changed = false;
    widgets::section(ui, "Arquivo de som", cx.pal);
    ui.horizontal(|ui| {
        match file_name(path) {
            Some(name) => {
                ui.label(name).on_hover_text(path.as_str());
            }
            None => {
                ui.label(RichText::new(t("Nenhum")).color(cx.pal.muted));
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let kind = ButtonKind::Secondary;
            if widgets::button(ui, Some("folder-open"), "Escolher", kind, true, 0.0, cx.pal)
                .clicked()
            {
                if let Some(file) = pick_sound_file() {
                    *path = file;
                    changed = true;
                }
            }
        });
    });
    ui.label(widgets::hint(t(SOUND_HINT), cx.pal));
    changed
}

/// `SoundBank`: nome do banco e a lista de arquivos. Outro banco pode servir de ponto de partida.
pub(super) fn bank_editor(
    ui: &mut egui::Ui,
    cx: &PaneCtx,
    name: &mut String,
    sounds: &mut Vec<String>,
) -> bool {
    let label = widgets::section(ui, "Banco de sons", cx.pal);
    let mut changed = widgets::text_field_width(ui, name, "", cx.pal, false, ui.available_width())
        .labelled_by(label)
        .changed();

    let others: Vec<&str> = cx
        .targets
        .sound_banks()
        .keys()
        .map(String::as_str)
        .filter(|other| *other != name.as_str())
        .collect();
    if !others.is_empty() {
        let mut source = String::new();
        let combo = widgets::combo_box(
            ui,
            "soundbank-copy",
            "Copiar sons de",
            &mut source,
            &others,
            cx.pal,
        );
        if combo.changed() {
            if let Some(copied) = cx.targets.sound_banks().get(&source) {
                sounds.clone_from(copied);
                changed = true;
            }
        }
    }

    widgets::section(ui, "Sons", cx.pal);
    let mut remove = None;
    for (i, sound) in sounds.iter_mut().enumerate() {
        ui.push_id(("bank-sound", i), |ui| {
            ui.horizontal(|ui| {
                let field =
                    widgets::text_field_width(ui, sound, "", cx.pal, false, SOUND_FIELD_WIDTH);
                // Cada linha tem nome próprio: "Som 2", não um campo anônimo entre vários iguais.
                widgets::name(&field, &tf("Som {}", &[&(i + 1)]));
                changed |= field.changed();
                if widgets::icon_button(ui, "folder-open", "Escolher arquivo", true, cx.pal)
                    .clicked()
                {
                    if let Some(file) = pick_sound_file() {
                        *sound = file;
                        changed = true;
                    }
                }
                if widgets::icon_button(ui, "trash", "Remover som", true, cx.pal).clicked() {
                    remove = Some(i);
                }
            });
        });
    }
    if let Some(i) = remove {
        sounds.remove(i);
        changed = true;
    }
    let kind = ButtonKind::Secondary;
    if widgets::button(ui, Some("plus"), "Adicionar som", kind, true, 0.0, cx.pal).clicked() {
        sounds.push(String::new());
        changed = true;
    }
    ui.label(widgets::hint(t(SOUND_HINT), cx.pal));
    changed
}

/// Rótulo de um som do banco na lista: número e nome do arquivo.
fn sound_label(index: usize, path: &str) -> String {
    let name = file_name(path).unwrap_or_else(|| path.to_string());
    tf("Som {}: {}", &[&(index + 1), &name])
}

/// `PlaySoundBank`: banco, som dentro dele e volume.
pub(super) fn bank_sound_editor(
    ui: &mut egui::Ui,
    cx: &PaneCtx,
    bank: &mut String,
    index: &mut usize,
    volume: &mut f32,
) -> bool {
    let mut changed = false;
    let banks = cx.targets.sound_banks();
    widgets::section(ui, "Som do banco", cx.pal);
    if banks.is_empty() {
        ui.label(widgets::hint(t(NO_BANKS_HINT), cx.pal));
    } else {
        let names: Vec<&str> = banks.keys().map(String::as_str).collect();
        if widgets::combo_box(ui, "soundbank-bank", "Banco", bank, &names, cx.pal).changed() {
            *index = 0;
            changed = true;
        }
    }
    if let Some(sounds) = banks.get(bank.as_str()) {
        let labels: Vec<String> = sounds
            .iter()
            .enumerate()
            .map(|(i, path)| sound_label(i, path))
            .collect();
        let options: Vec<&str> = labels.iter().map(String::as_str).collect();
        let mut chosen = labels.get(*index).cloned().unwrap_or_default();
        if widgets::combo_box(ui, "soundbank-sound", "Som", &mut chosen, &options, cx.pal).changed()
        {
            if let Some(i) = labels.iter().position(|l| *l == chosen) {
                *index = i;
                changed = true;
            }
        }
    }
    let volume_label = widgets::section(ui, "Volume", cx.pal);
    let mut percent = *volume * 100.0;
    if widgets::slider(ui, &mut percent, 0.0..=100.0, " %")
        .labelled_by(volume_label)
        .changed()
    {
        *volume = (percent / 100.0).clamp(0.0, 1.0);
        changed = true;
    }
    changed
}

/// `StopAllSounds`: sem campos, só a explicação.
pub(super) fn stop_sounds_hint(ui: &mut egui::Ui, cx: &PaneCtx) -> bool {
    widgets::section(ui, "Parar todos os sons", cx.pal);
    ui.label(widgets::hint(
        t("Mata todos os processos de som em execução"),
        cx.pal,
    ));
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sounds_are_listed_by_number_and_file_name() {
        assert_eq!(sound_label(0, "/sons/aplausos.ogg"), "Som 1: aplausos.ogg");
        assert_eq!(sound_label(2, ""), "Som 3: ");
    }
}
