//! Texturas das teclas no editor: a imagem exata que vai ao display, refeita só quando a tecla
//! muda. A primeira imagem de cada tecla sai na hora; as seguintes são desenhadas numa thread à
//! parte e a imagem anterior continua na tela até a nova ficar pronta (a janela não engasga com
//! uma capa de música ou uma página inteira mudando).
use super::{current_minute, is_blank};
use crate::config::schema::{ButtonAction, ButtonConfig};
use crate::graphics::{gif, live_art, renderer};
use crate::platform::live::Live;
use eframe::egui;
use image::RgbaImage;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

/// Tudo de que a imagem de uma tecla depende.
#[derive(Clone, PartialEq)]
struct Stamp {
    button: ButtonConfig,
    px: u32,
    /// Minuto atual nas teclas de relógio; 0 nas outras.
    minute: u64,
    /// Só nas teclas vivas.
    live: Option<Live>,
    /// Quadro do GIF animado mostrado (só nas teclas com GIF).
    gif_frame: Option<usize>,
}

impl Stamp {
    fn draw(&self) -> RgbaImage {
        match &self.live {
            Some(data) => live_art::render_live(&self.button, self.px, self.px, data),
            None => renderer::render_rgba(&self.button, self.px, self.px),
        }
    }
}

struct CachedKey {
    stamp: Stamp,
    tex: egui::TextureHandle,
    /// Pedido já mandado à thread e ainda sem resposta.
    pending: Option<Stamp>,
}

type Job = (usize, Stamp);
type Done = (usize, Stamp, RgbaImage);

struct Worker {
    jobs: Sender<Job>,
    done: Receiver<Done>,
}

impl Worker {
    fn start(ctx: egui::Context) -> Self {
        let (jobs, inbox) = channel::<Job>();
        let (outbox, done) = channel::<Done>();
        crate::telemetry::spawn_named("key-textures", move || {
            while let Ok(first) = inbox.recv() {
                // Digitando um rótulo chegam vários pedidos da mesma tecla: só o último vale.
                let mut latest: HashMap<usize, Stamp> = HashMap::from([first]);
                latest.extend(inbox.try_iter());
                for (index, stamp) in latest {
                    let img = stamp.draw();
                    if outbox.send((index, stamp, img)).is_err() {
                        return;
                    }
                }
                ctx.request_repaint();
            }
        });
        Self { jobs, done }
    }
}

#[derive(Default)]
pub(super) struct KeyTextures {
    keys: Vec<Option<CachedKey>>,
    /// Dados ao vivo do quadro atual (copiados de `Shared::live`), para as teclas vivas.
    pub live: Live,
    worker: Option<Worker>,
}

fn color_image(img: &RgbaImage) -> egui::ColorImage {
    let size = [img.width() as usize, img.height() as usize];
    egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw())
}

fn upload(ctx: &egui::Context, index: usize, img: &RgbaImage) -> egui::TextureHandle {
    ctx.load_texture(
        format!("key{index}"),
        color_image(img),
        egui::TextureOptions::LINEAR,
    )
}

/// Troca a imagem de uma textura que já existe (sem alocar outra na GPU a cada mudança).
fn replace(tex: &mut egui::TextureHandle, img: &RgbaImage) {
    tex.set(color_image(img), egui::TextureOptions::LINEAR);
}

impl KeyTextures {
    /// Aplica as imagens que a thread terminou (só as que ainda são as esperadas).
    fn collect(&mut self) {
        let Some(worker) = &self.worker else { return };
        for (index, stamp, img) in worker.done.try_iter() {
            let Some(Some(slot)) = self.keys.get_mut(index) else {
                continue;
            };
            if slot.pending.as_ref() == Some(&stamp) {
                replace(&mut slot.tex, &img);
                slot.stamp = stamp;
                slot.pending = None;
            }
        }
    }

    pub(super) fn get(
        &mut self,
        ctx: &egui::Context,
        index: usize,
        b: &ButtonConfig,
        px: u32,
    ) -> Option<egui::TextureId> {
        self.collect();
        if self.keys.len() <= index {
            self.keys.resize_with(index + 1, || None);
        }
        if is_blank(b) {
            return None;
        }

        // GIF animado: os quadros já decodificados vêm do cache; a tecla só é redesenhada quando
        // o quadro muda, e a janela acorda na hora da troca (não a cada quadro da tela).
        let gif = b
            .appearance
            .icon_path
            .as_deref()
            .filter(|p| p.to_lowercase().ends_with(".gif"))
            .and_then(|p| gif::frame_at(p, None));
        if let Some((frame, _, remaining)) = gif {
            ctx.request_repaint_after(Duration::from_millis(remaining));
            let stamp = Stamp {
                button: b.clone(),
                px,
                minute: 0,
                live: None,
                gif_frame: Some(frame),
            };
            let slot = &mut self.keys[index];
            match slot {
                Some(cached) if cached.stamp == stamp => {}
                Some(cached) => {
                    replace(&mut cached.tex, &stamp.draw());
                    cached.stamp = stamp;
                    cached.pending = None;
                }
                None => {
                    let tex = upload(ctx, index, &stamp.draw());
                    *slot = Some(CachedKey {
                        stamp,
                        tex,
                        pending: None,
                    });
                }
            }
            return slot.as_ref().map(|c| c.tex.id());
        }

        let stamp = Stamp {
            button: b.clone(),
            px,
            minute: if b.action == ButtonAction::Clock {
                current_minute()
            } else {
                0
            },
            live: live_art::wants(b).any().then(|| self.live.clone()),
            gif_frame: None,
        };
        let slot = &mut self.keys[index];
        match slot {
            None => {
                let tex = upload(ctx, index, &stamp.draw());
                *slot = Some(CachedKey {
                    stamp,
                    tex,
                    pending: None,
                });
            }
            Some(cached) if cached.stamp != stamp && cached.pending.as_ref() != Some(&stamp) => {
                let worker = self
                    .worker
                    .get_or_insert_with(|| Worker::start(ctx.clone()));
                if worker.jobs.send((index, stamp.clone())).is_ok() {
                    cached.pending = Some(stamp);
                }
            }
            Some(_) => {}
        }
        slot.as_ref().map(|c| c.tex.id())
    }
}
