//! Ícones vetoriais embutidos, desenhados numa grade de 24x24. A mesma geometria é usada
//! na interface (egui) e no display do aparelho, então a prévia é fiel ao que sai no botão.

pub type Polyline = Vec<(f32, f32)>;

/// (id gravado na configuração, nome exibido)
pub const ICONS: [(&str, &str); 16] = [
    ("play", "Play"),
    ("next", "Próxima"),
    ("prev", "Anterior"),
    ("volup", "Volume +"),
    ("voldown", "Volume −"),
    ("mute", "Mudo"),
    ("copy", "Copiar"),
    ("paste", "Colar"),
    ("calc", "Calculadora"),
    ("page", "Página"),
    ("globe", "Web"),
    ("bolt", "Atalho"),
    ("lock", "Bloquear"),
    ("folder", "Pasta"),
    ("app", "Aplicativo"),
    ("user", "Perfil"),
];

fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Polyline {
    vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)]
}

fn arc(cx: f32, cy: f32, rx: f32, ry: f32, a0: f32, a1: f32) -> Polyline {
    const STEPS: usize = 20;
    (0..=STEPS)
        .map(|i| {
            let a = (a0 + (a1 - a0) * i as f32 / STEPS as f32).to_radians();
            (cx + rx * a.cos(), cy + ry * a.sin())
        })
        .collect()
}

fn speaker() -> Polyline {
    vec![
        (4.0, 9.0),
        (8.0, 9.0),
        (13.0, 5.0),
        (13.0, 19.0),
        (8.0, 15.0),
        (4.0, 15.0),
        (4.0, 9.0),
    ]
}

/// Traços do ícone `name` (vazio se o nome for desconhecido).
pub fn strokes(name: &str) -> Vec<Polyline> {
    match name {
        "play" => vec![vec![(7.0, 4.0), (20.0, 12.0), (7.0, 20.0), (7.0, 4.0)]],
        "next" => vec![
            vec![(5.0, 4.0), (16.0, 12.0), (5.0, 20.0), (5.0, 4.0)],
            vec![(19.0, 4.0), (19.0, 20.0)],
        ],
        "prev" => vec![
            vec![(19.0, 4.0), (8.0, 12.0), (19.0, 20.0), (19.0, 4.0)],
            vec![(5.0, 4.0), (5.0, 20.0)],
        ],
        "volup" => vec![
            speaker(),
            arc(12.0, 12.0, 5.0, 5.0, -45.0, 45.0),
            arc(12.0, 12.0, 8.5, 8.5, -45.0, 45.0),
        ],
        "voldown" => vec![speaker(), arc(12.0, 12.0, 5.0, 5.0, -45.0, 45.0)],
        "mute" => vec![
            speaker(),
            vec![(17.0, 9.0), (22.0, 15.0)],
            vec![(22.0, 9.0), (17.0, 15.0)],
        ],
        "copy" => vec![
            rect(8.0, 8.0, 20.0, 20.0),
            vec![
                (16.0, 8.0),
                (16.0, 5.0),
                (15.0, 4.0),
                (5.0, 4.0),
                (4.0, 5.0),
                (4.0, 15.0),
                (5.0, 16.0),
                (8.0, 16.0),
            ],
        ],
        "paste" => vec![rect(5.0, 5.0, 19.0, 21.0), rect(9.0, 3.0, 15.0, 7.0)],
        "calc" => vec![
            rect(5.0, 3.0, 19.0, 21.0),
            vec![(8.0, 7.0), (16.0, 7.0)],
            vec![(8.0, 12.0), (10.0, 12.0)],
            vec![(14.0, 12.0), (16.0, 12.0)],
            vec![(8.0, 16.0), (10.0, 16.0)],
            vec![(14.0, 16.0), (16.0, 16.0)],
        ],
        "page" => vec![
            rect(4.0, 5.0, 20.0, 19.0),
            vec![(9.0, 10.0), (15.0, 10.0)],
            vec![(9.0, 14.0), (13.0, 14.0)],
        ],
        "globe" => vec![
            arc(12.0, 12.0, 9.0, 9.0, 0.0, 360.0),
            vec![(3.0, 12.0), (21.0, 12.0)],
            arc(12.0, 12.0, 4.5, 9.0, 0.0, 360.0),
        ],
        "bolt" => vec![vec![
            (13.0, 3.0),
            (6.0, 14.0),
            (12.0, 14.0),
            (11.0, 21.0),
            (18.0, 10.0),
            (12.0, 10.0),
            (13.0, 3.0),
        ]],
        "lock" => vec![
            rect(6.0, 11.0, 18.0, 21.0),
            arc(12.0, 11.0, 4.0, 4.0, 180.0, 360.0),
        ],
        "folder" => vec![vec![
            (3.0, 7.0),
            (9.0, 7.0),
            (11.0, 9.0),
            (21.0, 9.0),
            (21.0, 19.0),
            (3.0, 19.0),
            (3.0, 7.0),
        ]],
        "app" => vec![rect(4.0, 4.0, 20.0, 20.0), rect(9.0, 9.0, 15.0, 15.0)],
        "user" => vec![
            arc(12.0, 8.0, 4.0, 4.0, 0.0, 360.0),
            arc(12.0, 21.0, 8.0, 8.0, 180.0, 360.0),
        ],
        _ => ui_strokes(name),
    }
}

/// Ícones só da interface do editor (não aparecem na escolha de ícone da tecla).
#[cfg(test)]
pub const UI_ICONS: [&str; 13] = [
    "undo", "redo", "settings", "trash", "search", "grip", "plus", "keyboard", "ban", "music",
    "mixer", "chevron", "image",
];

fn dot(x: f32, y: f32) -> Polyline {
    vec![(x, y - 0.6), (x, y + 0.6)]
}

fn ui_strokes(name: &str) -> Vec<Polyline> {
    match name {
        "undo" => vec![
            vec![(9.0, 14.0), (4.0, 9.0), (9.0, 4.0)],
            vec![(4.0, 9.0), (14.0, 9.0)],
            arc(14.0, 14.5, 5.5, 5.5, -90.0, 90.0),
            vec![(14.0, 20.0), (11.0, 20.0)],
        ],
        "redo" => vec![
            vec![(15.0, 14.0), (20.0, 9.0), (15.0, 4.0)],
            vec![(20.0, 9.0), (10.0, 9.0)],
            arc(10.0, 14.5, 5.5, 5.5, 270.0, 90.0),
            vec![(10.0, 20.0), (13.0, 20.0)],
        ],
        "settings" => vec![
            vec![(20.0, 7.0), (11.0, 7.0)],
            vec![(13.0, 17.0), (4.0, 17.0)],
            arc(17.0, 17.0, 3.0, 3.0, 0.0, 360.0),
            arc(7.0, 7.0, 3.0, 3.0, 0.0, 360.0),
        ],
        "trash" => vec![
            vec![(3.0, 6.0), (21.0, 6.0)],
            vec![
                (19.0, 6.0),
                (19.0, 20.0),
                (17.0, 22.0),
                (7.0, 22.0),
                (5.0, 20.0),
                (5.0, 6.0),
            ],
            vec![
                (8.0, 6.0),
                (8.0, 4.0),
                (10.0, 2.0),
                (14.0, 2.0),
                (16.0, 4.0),
                (16.0, 6.0),
            ],
        ],
        "search" => vec![
            arc(11.0, 11.0, 7.0, 7.0, 0.0, 360.0),
            vec![(16.0, 16.0), (21.0, 21.0)],
        ],
        "grip" => [6.0, 12.0, 18.0]
            .iter()
            .flat_map(|&y| [dot(9.0, y), dot(15.0, y)])
            .collect(),
        "plus" => vec![
            vec![(5.0, 12.0), (19.0, 12.0)],
            vec![(12.0, 5.0), (12.0, 19.0)],
        ],
        "keyboard" => vec![
            rect(2.0, 5.0, 22.0, 19.0),
            vec![(7.0, 15.0), (17.0, 15.0)],
            dot(6.0, 9.5),
            dot(10.0, 9.5),
            dot(14.0, 9.5),
            dot(18.0, 9.5),
        ],
        "ban" => vec![
            arc(12.0, 12.0, 9.0, 9.0, 0.0, 360.0),
            vec![(5.6, 5.6), (18.4, 18.4)],
        ],
        "music" => vec![
            vec![(9.0, 18.0), (9.0, 5.0), (21.0, 3.0), (21.0, 16.0)],
            arc(6.0, 18.0, 3.0, 3.0, 0.0, 360.0),
            arc(18.0, 16.0, 3.0, 3.0, 0.0, 360.0),
        ],
        "mixer" => vec![
            vec![(4.0, 21.0), (4.0, 14.0)],
            vec![(4.0, 10.0), (4.0, 3.0)],
            vec![(12.0, 21.0), (12.0, 12.0)],
            vec![(12.0, 8.0), (12.0, 3.0)],
            vec![(20.0, 21.0), (20.0, 16.0)],
            vec![(20.0, 12.0), (20.0, 3.0)],
            vec![(2.0, 14.0), (6.0, 14.0)],
            vec![(10.0, 8.0), (14.0, 8.0)],
            vec![(18.0, 16.0), (22.0, 16.0)],
        ],
        "chevron" => vec![vec![(6.0, 9.0), (12.0, 15.0), (18.0, 9.0)]],
        "image" => vec![
            rect(3.0, 3.0, 21.0, 21.0),
            arc(9.0, 9.0, 2.0, 2.0, 0.0, 360.0),
            vec![(21.0, 15.0), (16.0, 10.0), (6.0, 20.0)],
        ],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_icon_has_strokes_inside_the_grid() {
        for id in ICONS.iter().map(|(id, _)| *id).chain(UI_ICONS) {
            let s = strokes(id);
            assert!(!s.is_empty(), "{id} sem traços");
            for (x, y) in s.iter().flatten() {
                assert!(
                    (0.0..=24.0).contains(x) && (0.0..=24.0).contains(y),
                    "{id}: ({x},{y}) fora da grade"
                );
            }
        }
    }

    #[test]
    fn unknown_icon_is_empty() {
        assert!(strokes("nao-existe").is_empty());
    }
}
