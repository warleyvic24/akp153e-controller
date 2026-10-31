//! Biblioteca de ícones Lucide (licença ISC, `assets/LUCIDE-LICENSE.txt`) embutida no programa:
//! cerca de 2000 ícones de traço, buscáveis por nome e por palavras-chave. Cada linha do arquivo é
//! `nome<TAB>palavras-chave<TAB>elementos SVG`; o ícone é montado na cor do texto da tecla e
//! desenhado pelo `resvg`, então a prévia do editor e o aparelho mostram a mesma imagem.
use crate::config::schema::RGBAColor;
use image::RgbaImage;
use std::sync::OnceLock;

const DATA: &str = include_str!("../../assets/lucide-icons.tsv");
/// Prefixo do id gravado na config (`lucide:camera`); sem ele o nome é um ícone embutido antigo.
pub const PREFIX: &str = "lucide:";
/// Ícones mostrados antes de qualquer busca: os que mais aparecem num controlador de live e mesa.
pub const SUGGESTED: [&str; 40] = [
    "mic",
    "mic-off",
    "video",
    "video-off",
    "camera",
    "circle-dot",
    "radio",
    "cast",
    "monitor",
    "screen-share",
    "gamepad-2",
    "headphones",
    "disc-3",
    "volume-2",
    "volume-x",
    "clock",
    "timer",
    "sun",
    "moon",
    "house",
    "folder",
    "terminal",
    "message-circle",
    "heart",
    "star",
    "zap",
    "bell",
    "scissors",
    "save",
    "undo-2",
    "lock",
    "power",
    "settings",
    "search",
    "trash",
    "image",
    "layers",
    "chevron-right",
    "chevron-left",
    "rocket",
];

/// Palavras em português que viram o termo em inglês da biblioteca (a busca procura os dois).
const ALIASES: [(&str, &str); 44] = [
    ("microfone", "mic"),
    ("câmera", "camera"),
    ("camera", "camera"),
    ("webcam", "webcam"),
    ("vídeo", "video"),
    ("video", "video"),
    ("gravar", "record"),
    ("gravação", "record"),
    ("ao vivo", "live"),
    ("live", "live"),
    ("transmitir", "cast"),
    ("tela", "monitor"),
    ("jogo", "gamepad"),
    ("fone", "headphones"),
    ("música", "music"),
    ("musica", "music"),
    ("som", "volume"),
    ("volume", "volume"),
    ("mudo", "mute"),
    ("relógio", "clock"),
    ("relogio", "clock"),
    ("tempo", "timer"),
    ("brilho", "sun"),
    ("luz", "light"),
    ("noite", "moon"),
    ("casa", "house"),
    ("início", "house"),
    ("pasta", "folder"),
    ("arquivo", "file"),
    ("mensagem", "message"),
    ("conversa", "chat"),
    ("coração", "heart"),
    ("estrela", "star"),
    ("raio", "zap"),
    ("sino", "bell"),
    ("tesoura", "scissors"),
    ("salvar", "save"),
    ("desfazer", "undo"),
    ("cadeado", "lock"),
    ("desligar", "power"),
    ("configurações", "settings"),
    ("lixeira", "trash"),
    ("seta", "arrow"),
    ("foto", "image"),
];

pub struct LibIcon {
    pub name: &'static str,
    tags: &'static str,
    body: &'static str,
}

/// Todos os ícones, em ordem alfabética (lidos uma vez do arquivo embutido).
pub fn all() -> &'static [LibIcon] {
    static ICONS: OnceLock<Vec<LibIcon>> = OnceLock::new();
    ICONS.get_or_init(|| {
        DATA.lines()
            .filter_map(|line| {
                let mut parts = line.splitn(3, '\t');
                Some(LibIcon {
                    name: parts.next()?,
                    tags: parts.next()?,
                    body: parts.next()?,
                })
            })
            .collect()
    })
}

/// O ícone de um id da config (`lucide:nome`), se existir na biblioteca.
pub fn find(id: &str) -> Option<&'static LibIcon> {
    let name = id.strip_prefix(PREFIX)?;
    let icons = all();
    icons
        .binary_search_by(|i| i.name.cmp(name))
        .ok()
        .map(|i| &icons[i])
}

/// Ícones cujo nome ou palavras-chave contêm todas as palavras da busca (o nome conta mais).
pub fn search(query: &str, limit: usize) -> Vec<&'static LibIcon> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return SUGGESTED
            .iter()
            .filter_map(|n| find(&format!("{PREFIX}{n}")))
            .collect();
    }
    let english = ALIASES
        .iter()
        .find(|(pt, _)| *pt == query)
        .map(|(_, en)| *en);
    let words: Vec<&str> =
        english.map_or_else(|| query.split_whitespace().collect(), |en| vec![en]);
    let mut hits: Vec<(u8, &LibIcon)> = all()
        .iter()
        .filter_map(|icon| {
            let all_match = words
                .iter()
                .all(|w| icon.name.contains(w) || icon.tags.contains(w));
            // Nome igual primeiro, depois nome que começa com a busca, depois o resto.
            let rank = match () {
                _ if icon.name == words[0] => 0,
                _ if icon.name.starts_with(words[0]) => 1,
                _ if icon.name.contains(words[0]) => 2,
                _ => 3,
            };
            all_match.then_some((rank, icon))
        })
        .collect();
    hits.sort_by_key(|(rank, icon)| (*rank, icon.name.len(), icon.name));
    hits.into_iter().take(limit).map(|(_, i)| i).collect()
}

/// SVG completo do ícone, com o traço na cor pedida.
fn svg(icon: &LibIcon, color: RGBAColor) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\" fill=\"none\" \
         stroke=\"#{:02x}{:02x}{:02x}\" stroke-width=\"2\" stroke-linecap=\"round\" \
         stroke-linejoin=\"round\">{}</svg>",
        color.r, color.g, color.b, icon.body
    )
}

/// Desenha o ícone `id` (`lucide:nome`) num quadrado de `px`, transparente em volta.
pub fn render(id: &str, px: u32, color: RGBAColor) -> Option<RgbaImage> {
    let icon = find(id)?;
    let tree =
        resvg::usvg::Tree::from_str(&svg(icon, color), &resvg::usvg::Options::default()).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(px.max(1), px.max(1))?;
    let scale = px as f32 / tree.size().width().max(tree.size().height());
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    // O tiny-skia guarda cores pré-multiplicadas; a imagem quer as normais.
    let pixels = pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect();
    RgbaImage::from_raw(px, px, pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: RGBAColor = RGBAColor {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };

    #[test]
    fn the_library_is_sorted_and_large() {
        let icons = all();
        assert!(icons.len() > 1500, "{}", icons.len());
        assert!(icons.windows(2).all(|w| w[0].name < w[1].name));
    }

    #[test]
    fn every_suggested_icon_exists() {
        for name in SUGGESTED {
            assert!(find(&format!("{PREFIX}{name}")).is_some(), "{name}");
        }
    }

    #[test]
    fn every_alias_finds_something() {
        for (pt, _) in ALIASES {
            assert!(!search(pt, 5).is_empty(), "{pt}");
        }
    }

    #[test]
    fn an_exact_name_comes_first_and_portuguese_works() {
        assert_eq!(search("mic", 10)[0].name, "mic");
        assert_eq!(search("microfone", 10)[0].name, "mic");
        assert!(search("câmera", 10).iter().any(|i| i.name == "camera"));
        assert!(search("zzzz-nada", 10).is_empty());
    }

    #[test]
    fn an_icon_renders_with_ink_in_the_requested_color() {
        let img = render("lucide:mic", 64, WHITE).expect("desenha");
        assert_eq!(img.dimensions(), (64, 64));
        let inked = img.pixels().filter(|p| p[3] > 200).count();
        assert!(inked > 100, "{inked}");
        assert!(img.pixels().filter(|p| p[3] > 200).all(|p| p[0] > 200));
        assert!(render("lucide:nao-existe", 64, WHITE).is_none());
        assert!(
            render("mic", 64, WHITE).is_none(),
            "sem prefixo é ícone antigo"
        );
    }
}
