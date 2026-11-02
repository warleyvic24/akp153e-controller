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

/// Componentes de interface cruas do egui só podem viver em `widgets.rs`. Todo o resto do editor
/// usa os wrappers canônicos (`text_field`, `secondary_button`, `slider`,
/// `combo_box`, `scroll_area`, `checkbox`, `pill`, `icon_button`, `segment`, `status_chip`,
/// `swatch`, `fold`, `modal`), que padronizam altura, raio, contraste e rótulo falado.
#[test]
fn no_raw_egui_widgets_outside_widgets_rs() {
    let raw_widgets = [
        "egui::TextEdit",
        "egui::Button::new",
        "egui::Slider::new",
        "egui::ComboBox::from",
        "egui::ScrollArea::",
        "egui::CollapsingHeader::new",
        "egui::Checkbox::new",
    ];
    let mut wrong = Vec::new();
    for (file, src) in crate::test_support::sources_without_tests() {
        if file == "editor/widgets.rs" || !file.starts_with("editor/") {
            continue;
        }
        for rw in &raw_widgets {
            if src.contains(rw) {
                wrong.push(format!("{file} usa {rw}"));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "componentes cruos do egui fora de widgets.rs:\\n{}",
        wrong.join("\\n")
    );
}

/// Todo bloco `unsafe` (só há chamadas de FFI/COM do Windows) tem logo acima um `// SAFETY:`
/// dizendo por que a chamada é sólida. Lê o arquivo inteiro, inclusive o código só de Windows.
#[test]
fn every_unsafe_block_explains_its_safety() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    let mut dirs = vec![root.clone()];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|e| e == "rs")
                // Este arquivo cita `unsafe` nas próprias buscas.
                && !path.ends_with("architecture_tests.rs")
            {
                files.push(path);
            }
        }
    }
    let (mut blocks, mut wrong) = (0, Vec::new());
    for path in files {
        let src = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = src.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            if !code.contains("unsafe {")
                && !code.contains("unsafe fn")
                && !code.contains("unsafe impl")
            {
                continue;
            }
            blocks += 1;
            let explained = lines[..i]
                .iter()
                .rev()
                .take_while(|l| l.trim_start().starts_with("//"))
                .any(|l| l.contains("SAFETY:"));
            if !explained {
                let file = path.strip_prefix(&root).unwrap().display();
                wrong.push(format!("{file}:{}", i + 1));
            }
        }
    }
    assert!(blocks > 0, "a busca por `unsafe` não achou nada");
    assert!(
        wrong.is_empty(),
        "`unsafe` sem `// SAFETY:` acima:\n{}",
        wrong.join("\n")
    );
}
