//! As camadas do `CLAUDE.md`, conferidas no código: dependência só para baixo. Antes isto era só regra
//! escrita; uma importação no lugar errado agora quebra o teste.

/// Camadas de cima: só elas (e quem as liga, `main.rs`/`cli.rs`) podem usar umas às outras.
const TOP: [&str; 2] = ["editor/", "service/"];

fn uses(src: &str, module: &str) -> bool {
    src.contains(&format!("crate::{module}::")) || src.contains(&format!("crate::{module}}}"))
}

#[test]
fn lower_layers_never_reach_the_editor_the_service_or_the_cli() {
    let mut wrong = Vec::new();
    for (file, src) in crate::test_support::sources_without_tests() {
        // Este arquivo cita os módulos nas próprias buscas.
        let top = TOP.iter().any(|t| file.starts_with(t))
            || ["main.rs", "cli.rs", "architecture_tests.rs"].contains(&file.as_str());
        if top {
            continue;
        }
        for module in ["editor", "service", "cli"] {
            if uses(&src, module) {
                wrong.push(format!("{file} usa crate::{module}"));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "camada de baixo usando a de cima:\n{}",
        wrong.join("\n")
    );
}

#[test]
fn the_editor_uses_only_the_shared_runtime_of_the_service() {
    let mut wrong = Vec::new();
    for (file, src) in crate::test_support::sources_without_tests() {
        if !file.starts_with("editor/") {
            continue;
        }
        let mut rest = src.as_str();
        while let Some(at) = rest.find("crate::service::") {
            let after = &rest[at + "crate::service::".len()..];
            if !after.starts_with("runtime") {
                wrong.push(format!(
                    "{file}: crate::service::{}",
                    after
                        .split(|c: char| !c.is_alphanumeric() && c != '_')
                        .next()
                        .unwrap_or("")
                ));
            }
            rest = after;
        }
    }
    assert!(
        wrong.is_empty(),
        "o editor só pode usar service::runtime:\n{}",
        wrong.join("\n")
    );
}

#[test]
fn the_service_never_uses_the_editor() {
    let wrong: Vec<String> = crate::test_support::sources_without_tests()
        .into_iter()
        .filter(|(file, src)| file.starts_with("service/") && uses(src, "editor"))
        .map(|(file, _)| file)
        .collect();
    assert!(wrong.is_empty(), "o serviço usa o editor: {wrong:?}");
}

#[test]
fn the_editor_opens_dialogs_only_through_the_shared_modal() {
    // Janela crua do egui fica presa a um canto, sem fundo escurecido nem Esc: usar `widgets::modal`.
    let raw: Vec<String> = crate::test_support::sources_without_tests()
        .into_iter()
        .filter(|(file, src)| file.starts_with("editor/") && src.contains("egui::Window::new"))
        .map(|(file, _)| file)
        .collect();
    assert!(raw.is_empty(), "janela crua no editor: {raw:?}");
}
