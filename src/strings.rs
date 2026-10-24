//! Textos da interface em português (idioma do código) e inglês. O texto em português é a própria
//! chave: `t("Salvo")` devolve o original no idioma padrão, ou a tradução se existir; o que ainda
//! não foi traduzido continua em português em vez de quebrar.
use crate::config::schema::Language;
use std::collections::HashMap;
use std::fmt::Display;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

static LANGUAGE: AtomicU8 = AtomicU8::new(0);

/// Define o idioma da interface (chamado quando a preferência muda).
pub fn set_language(lang: Language) {
    LANGUAGE.store(lang as u8, Ordering::Relaxed);
}

fn english() -> bool {
    LANGUAGE.load(Ordering::Relaxed) == Language::En as u8
}

fn table() -> &'static HashMap<&'static str, &'static str> {
    static TABLE: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    TABLE.get_or_init(|| EN.iter().copied().collect())
}

/// Texto no idioma atual. `pt` é o texto original, em português.
pub fn t(pt: &str) -> &str {
    if !english() {
        return pt;
    }
    table().get(pt).copied().unwrap_or(pt)
}

/// Nome de página ou perfil para exibir. Os nomes que o próprio programa cria ("Default", "Page 1",
/// "Página 2", "Perfil 3") aparecem no idioma atual; o que o usuário escreveu passa intacto.
/// Só muda a exibição: o nome guardado na config não é alterado.
pub fn display_name(name: &str) -> String {
    localize_name(name, english())
}

fn localize_name(name: &str, english: bool) -> String {
    if name == "Default" || name == "Padrão" {
        return if english { "Default" } else { "Padrão" }.to_string();
    }
    let numbered = [
        ("Page ", "Página "),
        ("Página ", "Página "),
        ("Profile ", "Perfil "),
        ("Perfil ", "Perfil "),
    ];
    for (prefix, pt) in numbered {
        let Some(n) = name.strip_prefix(prefix) else {
            continue;
        };
        if n.is_empty() || !n.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let word = match (pt.trim_end(), english) {
            ("Página", true) => "Page ",
            ("Perfil", true) => "Profile ",
            _ => pt,
        };
        return format!("{word}{n}");
    }
    name.to_string()
}

/// Como `t`, mas preenche cada `{}` do modelo, em ordem, com os argumentos.
pub fn tf(pt: &str, args: &[&dyn Display]) -> String {
    let mut out = String::new();
    let mut rest = t(pt);
    for arg in args {
        match rest.split_once("{}") {
            Some((head, tail)) => {
                out.push_str(head);
                out.push_str(&arg.to_string());
                rest = tail;
            }
            None => break,
        }
    }
    out.push_str(rest);
    out
}

/// Traduções para inglês: (português, inglês).
const EN: &[(&str, &str)] = &[
    ("(escolha)", "(choose)"),
    ("(vazio)", "(empty)"),
    ("AKP153E", "AKP153E"),
    ("Abrir editor", "Open editor"),
    ("Abrir pasta de logs", "Open logs folder"),
    ("Abrir site", "Open website"),
    (
        "Ajuste texto, cor e ícone no painel à direita",
        "Adjust text, color and icon in the panel on the right",
    ),
    ("Alternar", "Toggle"),
    ("Alternar entre dois estados", "Toggle between two states"),
    ("Anterior", "Previous"),
    ("Ao segurar (opcional)", "On hold (optional)"),
    ("Ao tocar", "On press"),
    ("Aparelho conectado.", "Device connected."),
    ("Aparelho desconectado", "Device disconnected"),
    ("Aparelho desconectado.", "Device disconnected."),
    (
        "Aparelho não encontrado. Confira o cabo USB.",
        "Device not found. Check the USB cable.",
    ),
    ("Aparência", "Appearance"),
    (
        "Aplica no aparelho ao salvar.",
        "Applies to the device when saved.",
    ),
    ("Aplicativo", "Application"),
    ("Aplicativo...", "Application..."),
    ("Aplicativos", "Applications"),
    ("App", "App"),
    ("Apps instalados", "Installed apps"),
    ("Argumentos", "Arguments"),
    ("Arraste uma ação", "Drag an action"),
    ("Atalho", "Shortcut"),
    ("Atalhos", "Shortcuts"),
    ("Ativar ação ao segurar", "Enable action on hold"),
    ("Ação", "Action"),
    ("Ação personalizada", "Custom action"),
    ("BOTÃO {}", "BUTTON {}"),
    ("Bloco de notas", "Notepad"),
    ("Bloquear", "Lock"),
    ("Brilho do aparelho", "Device brightness"),
    ("Buscar app", "Search apps"),
    ("Buscar ação", "Search actions"),
    ("COMECE POR AQUI", "START HERE"),
    ("CONECTADO", "CONNECTED"),
    ("Cada toque troca a aparência", "Each press swaps the look"),
    ("Calculadora", "Calculator"),
    ("Captura", "Screenshot"),
    ("Claro", "Light"),
    ("Clique numa tecla do aparelho", "Click a key on the device"),
    (
        "Clique para editar · arraste uma ação da bandeja · arraste entre teclas para trocar",
        "Click to edit · drag an action from the tray · drag between keys to swap",
    ),
    ("Colado na tecla {}", "Pasted on key {}"),
    ("Colar", "Paste"),
    ("Copiar", "Copy"),
    ("Cor de destaque", "Accent color"),
    ("Cor do botão", "Button color"),
    ("Corpo da requisição", "Request body"),
    ("Ctrl+Shift+Z", "Ctrl+Shift+Z"),
    ("Ctrl+Z", "Ctrl+Z"),
    ("DESCONECTADO", "DISCONNECTED"),
    ("Desfazer", "Undo"),
    ("Diagnóstico", "Diagnostics"),
    ("Digitar (ex: meta+d)", "Type (e.g. meta+d)"),
    (
        "Duplo clique renomeia · clique direito abre o menu",
        "Double-click to rename · right-click for the menu",
    ),
    (
        "Escolha uma ação na bandeja abaixo, ou arraste-a até a tecla",
        "Pick an action in the tray below, or drag it onto the key",
    ),
    (
        "Escolha uma ação na bandeja ou comece pelo rótulo.",
        "Pick an action from the tray, or start with the label.",
    ),
    ("Escolher", "Choose"),
    ("Escolher app instalado", "Choose installed app"),
    ("Escuro", "Dark"),
    ("Estado da conexão", "Connection status"),
    ("Esvazia este botão", "Empties this button"),
    ("Excluir perfil", "Delete profile"),
    ("Excluir página", "Delete page"),
    ("Explorador", "Explorer"),
    ("Falha ao salvar", "Save failed"),
    ("Fundo", "Background"),
    ("Gerenciar", "Manage"),
    ("Idioma", "Language"),
    ("Imagem (PNG ou JPG)", "Image (PNG or JPG)"),
    ("Imagem (PNG, JPG)", "Image (PNG, JPG)"),
    ("Inicialização", "Startup"),
    ("Iniciar com o Windows", "Start with Windows"),
    ("Ligado:", "On:"),
    ("Limpar", "Clear"),
    ("Limpar atalho", "Clear shortcut"),
    ("Limpar botão", "Clear button"),
    ("Limpar imagem", "Clear image"),
    ("Mover para a direita", "Move right"),
    ("Mover para a esquerda", "Move left"),
    ("Mudo", "Mute"),
    ("Método", "Method"),
    ("Mídia", "Media"),
    ("Nada encontrado para “{}”.", "Nothing found for “{}”."),
    ("Navegação", "Navigation"),
    (
        "Nenhum programa encontrado. Use “Procurar .exe”.",
        "No programs found. Use “Browse .exe”.",
    ),
    ("Nenhuma", "None"),
    (
        "Nenhuma ação encontrada para “{}”. Limpe a busca para ver todas.",
        "No action found for “{}”. Clear the search to see them all.",
    ),
    ("Mostra no aparelho os apps com som: ícone com a porcentagem em cima, + no meio e − embaixo. Esta tecla vira SAIR (segure para sair se houver mais de 5 apps).",
        "Shows the apps playing sound on the device: icon with the percentage on top, + in the middle and − below. This key becomes EXIT (hold it to exit when there are more than 5 apps).",
    ),
    ("MAIS", "MORE"),
    ("SAIR", "EXIT"),
    ("SEGURE: SAIR", "HOLD: EXIT"),
    ("Mixer", "Mixer"),
    ("Nome da página", "Page name"),
    ("Nome do perfil", "Profile name"),
    ("Nova página", "New page"),
    ("Nova tentativa em instantes.", "Retrying shortly."),
    ("Novo nome da página", "New page name"),
    ("Novo perfil", "New profile"),
    ("Outra", "Other"),
    ("Pasta", "Folder"),
    ("Perfil", "Profile"),
    ("Perfil de destino", "Target profile"),
    ("Perfil excluído", "Profile deleted"),
    ("Perfil {}", "Profile {}"),
    ("Personalizada", "Custom"),
    ("Play", "Play"),
    ("Play/Pause", "Play/Pause"),
    ("Preferências", "Preferences"),
    (
        "Procurando programas instalados…",
        "Looking for installed programs…",
    ),
    ("Procurar .exe", "Browse .exe"),
    ("Programa (caminho ou nome)", "Program (path or name)"),
    ("Próxima", "Next"),
    ("Página", "Page"),
    ("Página de destino", "Target page"),
    ("Página excluída", "Page deleted"),
    ("Página {}", "Page {}"),
    ("Reconectar", "Reconnect"),
    ("Recortar", "Cut"),
    ("Refazer", "Redo"),
    ("Renomear", "Rename"),
    ("Rótulo", "Label"),
    ("Sair", "Quit"),
    ("Salvando…", "Saving…"),
    ("Salvar", "Save"),
    ("Salvo", "Saved"),
    ("Segurar a partir de", "Hold after"),
    (
        "Sem aparelho conectado: vale a partir da próxima conexão.",
        "No device connected: applies from the next connection.",
    ),
    ("Sem nome", "Unnamed"),
    ("Sem ícone", "No icon"),
    (
        "Suas alterações ficam salvas e vão ao aparelho ao reconectar.",
        "Your changes are saved and reach the device when it reconnects.",
    ),
    ("Tamanho da fonte", "Font size"),
    ("Tecla de mídia", "Media key"),
    ("Tecla desconhecida: {}", "Unknown key: {}"),
    ("Tecla {} copiada", "Key {} copied"),
    ("Tecla {} limpa", "Key {} cleared"),
    ("Tecla {} testada", "Key {} tested"),
    ("Tecla {}, {}", "Key {}, {}"),
    ("Teclas trocadas", "Keys swapped"),
    ("Tema", "Theme"),
    ("Testar", "Test"),
    ("Texto", "Text"),
    ("Texto na tecla (ex.: Mudo)", "Text on the key (e.g. Mute)"),
    ("URL", "URL"),
    (
        "Ver o estado ligado na prévia",
        "Show the on state in the preview",
    ),
    ("Versão {}", "Version {}"),
    ("Vol +", "Vol +"),
    ("Vol -", "Vol -"),
    ("Vol −", "Vol −"),
    ("Volume +", "Volume +"),
    ("Volume −", "Volume −"),
    ("Web", "Web"),
    ("Webhook", "Webhook"),
    ("sem texto", "no text"),
    ("vazia", "empty"),
    ("Área de trabalho", "Desktop"),
    ("Ícone", "Icon"),
    ("● Gravar atalho", "● Record shortcut"),
    (
        "● Pressione as teclas… (Esc cancela)",
        "● Press the keys… (Esc cancels)",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Primeiro literal de texto depois de `from`, sem passar do fim da instrução (`;`).
    fn literal_after(src: &str, from: usize) -> Option<String> {
        let rest = &src[from..];
        let seg = &rest[..rest.find(';').unwrap_or(rest.len())];
        let start = seg.find('"')?;
        let mut out = String::new();
        let mut chars = seg[start + 1..].chars();
        while let Some(c) = chars.next() {
            match c {
                '\\' => out.push(match chars.next()? {
                    'n' => '\n',
                    other => other,
                }),
                '"' => return Some(out),
                c => out.push(c),
            }
        }
        None
    }

    /// Todo texto que os arquivos-fonte passam por `t`, `tf` ou pelos componentes que traduzem.
    fn keys_used_in_sources() -> Vec<(String, String)> {
        let calls = [
            "widgets::section(",
            "widgets::pill(",
            "widgets::tab(",
            "widgets::status_chip(",
            "widgets::hint(",
            "widgets::name(",
        ];
        let mut found = Vec::new();
        for (name, src) in crate::test_support::sources_without_tests() {
            if name == "strings.rs" {
                continue;
            }
            let src = src.as_str();
            let mut at = 0;
            while let Some(i) = src[at..].find("t(\"").map(|i| i + at) {
                at = i + 3;
                let before = src[..i].chars().last().unwrap_or(' ');
                if !(before.is_alphanumeric() || before == '_') {
                    found.extend(literal_after(src, i).map(|k| (name.clone(), k)));
                }
            }
            for call in calls.iter().chain(&["tf("]) {
                let mut at = 0;
                while let Some(i) = src[at..].find(call).map(|i| i + at) {
                    at = i + call.len();
                    found.extend(literal_after(src, at).map(|k| (name.clone(), k)));
                }
            }
        }
        found.retain(|(_, k)| k.chars().filter(|c| c.is_alphabetic()).count() >= 2);
        found
    }

    #[test]
    fn every_text_used_by_the_interface_has_an_english_translation() {
        let missing: Vec<String> = keys_used_in_sources()
            .into_iter()
            .filter(|(_, k)| !table().contains_key(k.as_str()))
            .map(|(file, k)| format!("{file}: {k:?}"))
            .collect();
        assert!(missing.is_empty(), "sem tradução:\n{}", missing.join("\n"));
    }

    #[test]
    fn the_scan_actually_finds_texts() {
        assert!(keys_used_in_sources().len() > 60);
    }

    #[test]
    fn placeholders_match_between_languages() {
        for (pt, en) in EN {
            assert_eq!(pt.matches("{}").count(), en.matches("{}").count(), "{pt}");
        }
    }

    #[test]
    fn default_names_follow_the_language_and_user_names_are_untouched() {
        assert_eq!(localize_name("Default", false), "Padrão");
        assert_eq!(localize_name("Padrão", true), "Default");
        assert_eq!(localize_name("Page 1", false), "Página 1");
        assert_eq!(localize_name("Página 12", true), "Page 12");
        assert_eq!(localize_name("Perfil 2", true), "Profile 2");
        assert_eq!(localize_name("Profile 3", false), "Perfil 3");
        for mine in ["Jogos", "Page", "Page x", "Page 1 extra", "Default 2", ""] {
            assert_eq!(localize_name(mine, true), mine);
            assert_eq!(localize_name(mine, false), mine);
        }
    }

    #[test]
    fn the_table_has_no_duplicate_keys() {
        assert_eq!(EN.len(), table().len());
    }

    #[test]
    fn portuguese_is_returned_untouched_and_placeholders_are_filled_in_order() {
        assert_eq!(t("Salvo"), "Salvo");
        assert_eq!(tf("Tecla {} copiada", &[&3]), "Tecla 3 copiada");
        assert_eq!(tf("Tecla {}, {}", &[&1, &"vazia"]), "Tecla 1, vazia");
    }
}
