//! Ícones reais na lista de apps. São procurados numa thread à parte, só para as linhas que
//! aparecem na tela; até o ícone chegar (ou se o app não tiver um) a linha mostra a inicial
//! colorida. No Windows a extração usa o PowerShell e é lenta: por isso a fila e o limite.
use crate::platform::apps::{self, InstalledApp};
use eframe::egui;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, Sender};

/// Ícones em fila ao mesmo tempo (as linhas visíveis cabem com folga).
const MAX_QUEUED: usize = 24;

type Found = (String, Option<image::RgbaImage>);

#[derive(Default)]
pub(super) struct AppIcons {
    textures: HashMap<String, egui::TextureHandle>,
    /// Pedidos feitos (com ou sem ícone no fim): cada app é procurado uma vez só.
    asked: HashSet<String>,
    queued: usize,
    worker: Option<(Sender<InstalledApp>, Receiver<Found>)>,
}

impl AppIcons {
    /// Textura do ícone do app; na primeira vez, pede a busca e devolve `None`.
    pub fn get(&mut self, ctx: &egui::Context, app: &InstalledApp) -> Option<egui::TextureId> {
        self.collect(ctx);
        if let Some(tex) = self.textures.get(&app.target) {
            return Some(tex.id());
        }
        if self.queued < MAX_QUEUED && self.asked.insert(app.target.clone()) {
            let (jobs, _) = self.worker.get_or_insert_with(|| spawn(ctx.clone()));
            if jobs.send(app.clone()).is_ok() {
                self.queued += 1;
            }
        }
        None
    }

    fn collect(&mut self, ctx: &egui::Context) {
        let Some((_, done)) = &self.worker else {
            return;
        };
        for (target, image) in done.try_iter() {
            self.queued = self.queued.saturating_sub(1);
            let Some(img) = image else { continue };
            let size = [img.width() as usize, img.height() as usize];
            let pixels = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
            let tex = ctx.load_texture(
                format!("app-icon:{target}"),
                pixels,
                egui::TextureOptions::LINEAR,
            );
            self.textures.insert(target, tex);
        }
    }
}

fn spawn(ctx: egui::Context) -> (Sender<InstalledApp>, Receiver<Found>) {
    let (jobs, inbox) = channel::<InstalledApp>();
    let (outbox, done) = channel::<Found>();
    crate::telemetry::spawn_named("app-icons", move || {
        for app in inbox {
            let image = apps::extract_icon(&app)
                .and_then(|path| image::open(path).ok())
                .map(|img| img.to_rgba8());
            if outbox.send((app.target, image)).is_err() {
                return;
            }
            ctx.request_repaint();
        }
    });
    (jobs, done)
}
