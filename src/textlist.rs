//! Campos de texto que editam uma lista (argumentos, teclas) sem perder o separador enquanto se digita.
use eframe::egui;

/// Separa por espaços; aspas agrupam (`"a b"` é um argumento) e `""` é um argumento vazio.
/// Aspas dentro de um argumento não são suportadas.
pub fn parse_args(text: &str) -> Vec<String> {
    let (mut out, mut current, mut quoted, mut started) = (Vec::new(), String::new(), false, false);
    for c in text.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            c if c.is_whitespace() && !quoted => {
                if started {
                    out.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            c => {
                current.push(c);
                started = true;
            }
        }
    }
    if started {
        out.push(current);
    }
    out
}

pub fn join_args(args: &[String]) -> String {
    let shown: Vec<String> = args
        .iter()
        .map(|a| {
            if a.is_empty() || a.chars().any(char::is_whitespace) {
                format!("\"{a}\"")
            } else {
                a.clone()
            }
        })
        .collect();
    shown.join(" ")
}

pub fn parse_keys(text: &str) -> Vec<String> {
    text.split('+')
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .map(String::from)
        .collect()
}

pub fn join_keys(keys: &[String]) -> String {
    keys.join("+")
}

/// Campo de uma linha que edita `items`. O texto digitado fica na memória do egui: reconstruí-lo
/// a cada quadro a partir da lista apagaria o separador recém-digitado (`meta+`, `a `).
/// Se a lista mudar por outro caminho (outro botão, desfazer), o texto é refeito a partir dela.
pub fn list_field(
    ui: &mut egui::Ui,
    id: egui::Id,
    items: &mut Vec<String>,
    parse: fn(&str) -> Vec<String>,
    join: fn(&[String]) -> String,
) -> bool {
    let mut raw: String = ui.data_mut(|d| d.get_temp(id)).unwrap_or_default();
    if parse(&raw) != *items {
        raw = join(items);
    }
    let changed = ui
        .add(egui::TextEdit::singleline(&mut raw).desired_width(ui.available_width()))
        .changed();
    if changed {
        *items = parse(&raw);
    }
    ui.data_mut(|d| d.insert_temp(id, raw));
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn args_split_on_spaces_and_honor_quotes() {
        assert_eq!(
            parse_args(r#"--dir "C:\Arquivos de Programas" -v"#),
            v(&["--dir", r"C:\Arquivos de Programas", "-v"])
        );
        assert_eq!(parse_args(r#"a "" b"#), v(&["a", "", "b"]));
        assert_eq!(parse_args("   "), Vec::<String>::new());
    }

    #[test]
    fn args_roundtrip_through_text() {
        for args in [
            v(&["/C", "start", "", "x"]),
            v(&["--dir", "a b", "-v"]),
            v(&[]),
        ] {
            assert_eq!(parse_args(&join_args(&args)), args);
        }
    }

    #[test]
    fn typing_a_separator_keeps_the_list_and_the_text() {
        assert_eq!(parse_args("a "), v(&["a"]));
        assert_eq!(parse_keys("meta+"), v(&["meta"]));
        assert_eq!(parse_keys(" ctrl + shift+s "), v(&["ctrl", "shift", "s"]));
        assert_eq!(join_keys(&v(&["ctrl", "c"])), "ctrl+c");
    }
}
