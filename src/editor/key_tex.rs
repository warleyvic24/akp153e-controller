//! Texturas das teclas no editor: a imagem exata que vai ao display, refeita só quando a tecla
//! muda. A primeira imagem de cada tecla sai na hora; as seguintes são desenhadas numa thread à
//! parte e a imagem anterior continua na tela até a nova ficar pronta (a janela não engasga com
//! uma capa de música ou uma página inteira mudando).
use super::{current_minute, is_blank};
use crate::config::schema::{ButtonAction, ButtonConfig};
use crate::graphics::{live_art, renderer};
use crate::platform::live::Live;
use eframe::egui;
use image::RgbaImage;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};

/// Tudo de que a imagem de uma tecla depende.
#[derive(Clone, PartialEq)]
struct Stamp {
    button: ButtonConfig,
    px: u32,
    /// Minuto atual nas teclas de relógio; 0 nas outras.
    minute: u64,
    /// Só nas teclas vivas.
    live: Option<Live>,
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
        std::thread::spawn(move || {
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

fn upload(ctx: &egui::Context, index: usize, img: &RgbaImage) -> egui::TextureHandle {
    let size = [img.width() as usize, img.height() as usize];
    let image = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
    ctx.load_texture(format!("key{index}"), image, egui::TextureOptions::LINEAR)
}

impl KeyTextures {
    /// Aplica as imagens que a thread terminou (só as que ainda são as esperadas).
    fn collect(&mut self, ctx: &egui::Context) {
        let Some(worker) = &self.worker else { return };
        for (index, stamp, img) in worker.done.try_iter() {
            let Some(Some(slot)) = self.keys.get_mut(index) else {
                continue;
            };
            if slot.pending.as_ref() == Some(&stamp) {
                slot.tex = upload(ctx, index, &img);
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
        self.collect(ctx);
        if self.keys.len() <= index {
            self.keys.resize_with(index + 1, || None);
        }
        if is_blank(b) {
            return None;
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
